//! Contexts: declaring them, and the live state of one.
//!
//! A context groups the bindings that are active together — on foot, in a vehicle, in a menu. You
//! declare one with [`ActionMapAppExt::add_context`] and give it to an entity; that entity then
//! carries an [`InputContextState`] holding the current state of every action in the context.
//!
//! Put the context on whatever the input belongs to. One entity for a single-player game, one per
//! player for local multiplayer, or a bare entity for input that is not tied to anything in
//! particular. Each carries its own state, so two players never share one.
//!
//! A context is live from the moment an entity carries it unless you say otherwise. Say otherwise
//! with [`active_in_state`](InputContextBuilder::active_in_state) for a context that comes and goes
//! with a game state, [`active_if`](InputContextBuilder::active_if) for one that follows any other
//! run condition, or [`activate`](InputContextState::activate) and
//! [`deactivate`](InputContextState::deactivate) to drive one instance yourself.
//!
//! Read the actions back with [`Actions`] where there is one instance of the context, and with
//! [`ActionsQuery`] where there may be several — one per player, or none at all because whatever
//! carried it was destroyed.

#[cfg(feature = "keyboard")]
use alloc::collections::BTreeSet;
use alloc::vec::Vec;
use core::marker::PhantomData;

use bevy_app::{App, FixedPreUpdate, PreUpdate};
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::lifecycle::HookContext;
use bevy_ecs::prelude::{Query, Resource};
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::system::SystemParam;
use bevy_ecs::world::{DeferredWorld, World};
use bevy_platform::sync::Arc;

use crate::action::{
    ActionId, ActionOutput, ActionState, InputAction, InputContext, Phase, Scratch, TickDomain,
};
use crate::binding::InputContextBuilder;
use crate::eval::{Transition, dispatch_class_fires, dispatch_transitions, evaluate_context};
use crate::frame::{InputFrame, Timestamp};
use crate::plan::Plan;
use crate::{ActionMapPlugin, ActionMapSystems};
#[cfg(feature = "gamepad")]
use bevy_platform::collections::HashMap;
#[cfg(feature = "mouse")]
use bevy_platform::collections::HashSet;

/// The compiled bindings for one context, shared by every instance of it.
// Instances hold an `Arc` to this rather than a copy: ten local players sharing one binding set
// hold one plan and ten small state tables. The hook needs somewhere to read it from on insertion,
// which is why it is also a resource.
//
// **This resource is the defaults, permanently.** Applying an override never writes to it — that is
// what keeps R17.1's diff-against-defaults possible after the first apply, since a diff needs
// something to diff against. The result of applying goes in `AppliedPlan<C>` instead.
#[derive(Resource)]
pub(crate) struct InputContextPlan<C> {
    plan: Arc<Plan<C>>,
    // The bindings as authored, kept so that an override can be applied as a diff against them.
    // Cloned and rewritten per apply rather than mutated, for the reason above.
    bindings: alloc::vec::Vec<crate::binding::BindingSpec>,
    // The presentation view of the same bindings, empty unless some were declared mappable.
    mappings: alloc::vec::Vec<crate::mapping::Mapping>,
    // The tunables view of the same bindings, empty unless some were declared tunable.
    tunables: alloc::vec::Vec<crate::mapping::Tunable>,
    // Whether an instance is live the moment it is spawned. False for a context whose activation
    // follows something else, so that it does not fire for one frame before the something else
    // has had a chance to say otherwise.
    starts_active: bool,
}

/// What one context's bindings currently are, once an override has been applied to them.
///
/// Absent until something applies one, which is what makes its presence the answer to "has anything
/// been overridden here". Everything that asks what is bound *now* — a spawning instance, the
/// presentation mapping list — reads this and falls back to `InputContextPlan<C>`.
#[derive(Resource)]
pub(crate) struct AppliedPlan<C> {
    pub(crate) plan: Arc<Plan<C>>,
    pub(crate) mappings: alloc::vec::Vec<crate::mapping::Mapping>,
    pub(crate) tunables: alloc::vec::Vec<crate::mapping::Tunable>,
}

/// Both views of one button-shaped control.
// A trigger has an analog position and a pressed sense, and the two are not derivable from each
// other on demand: `pressed` is hysteretic, so it depends on what it was last time this control
// was seen. Keeping it beside the value is what lets the button view be settled once per event
// rather than recomputed per binding, and is why two bindings on one trigger cannot disagree.
#[cfg(feature = "gamepad")]
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ButtonReading {
    pub(crate) value: f32,
    pub(crate) pressed: bool,
}

/// The live state for one declared context.
///
/// This holds the current state of every action in a context. It is a component, so an entity
/// carrying a context type gets one automatically — a player entity, one per local player, or a
/// bare entity for a context that is not tied to anything in particular.
///
/// One bit per action slot, for the ticks on which that action's state moved.
///
/// A bitset rather than a `Vec<bool>` so that "did anything move" is one word test for the context
/// sizes that occur — a plan with 64 actions or fewer is a single word — and so that a snapshot
/// carries it for the cost of rounding up.
#[derive(Clone, Default)]
pub(crate) struct DirtySet {
    words: Vec<u64>,
}

impl DirtySet {
    fn with_capacity(slots: usize) -> Self {
        Self {
            words: alloc::vec![0; slots.div_ceil(u64::BITS as usize)],
        }
    }

    pub(crate) fn clear(&mut self) {
        self.words.fill(0);
    }

    pub(crate) fn set(&mut self, slot: usize) {
        self.words[slot / u64::BITS as usize] |= 1 << (slot % u64::BITS as usize);
    }

    // The per-slot read has no caller outside the tests yet: what reads these bits one at a time
    // is a rollback snapshot, and that is still on the deferred table. `any` below is what the
    // change tick needs, and it is the whole of today's use.
    #[cfg(test)]
    pub(crate) fn contains(&self, slot: usize) -> bool {
        self.words
            .get(slot / u64::BITS as usize)
            .is_some_and(|word| word & (1 << (slot % u64::BITS as usize)) != 0)
    }

    pub(crate) fn any(&self) -> bool {
        self.words.iter().any(|&word| word != 0)
    }
}

/// The struct holds no references into the ECS, so tests and replays can drive one directly
/// without a `World`.
///
/// # Change detection
///
/// This component is marked changed on a tick where one of its actions actually moved, and not on
/// the ticks where evaluation ran and found nothing — so `Changed<InputContextState<C>>` is a
/// subscription rather than a per-frame wake-up. Keeping up with the devices, advancing the read
/// cursor, and lifting a shadow are all invisible to it, because none of them is an action
/// changing.
#[derive(Component)]
pub struct InputContextState<C> {
    pub(crate) plan: Arc<Plan<C>>,
    pub(crate) actions: Vec<ActionState>,
    // Parallel to `actions`: which of them moved since evaluation last cleared this. Per action
    // rather than per context because the component's own change tick cannot distinguish them
    // (R23.4), and because a rollback snapshot is the two tables plus these bits (Design §6).
    pub(crate) dirty: DirtySet,
    pub(crate) active: bool,
    // Set and cleared only by `shadow`/`unshadow` (R7.8), never by `activate`/`deactivate`: `active`
    // is what this context's own condition or lifecycle wants, `shadowed` is what a higher-priority
    // exclusive context is currently forcing regardless of that. Kept apart so the two do not fight
    // each other — see `is_active`.
    pub(crate) shadowed: bool,
    // Working memory for every modifier and condition in the plan, indexed as the plan says.
    pub(crate) scratch: Vec<Scratch>,
    // One cell per group of bindings sharing a tunable (`Plan::tunable_scratch_count`), rather than
    // each binding's own private slot in `scratch` above — the mechanism `hold_or_toggle` needs so
    // that pressing any control it reaches agrees with every other about the latch.
    pub(crate) tunable_scratch: Vec<Scratch>,
    // Reused between folds: the longest satisfied chord found on each control. Kept here rather
    // than allocated per fold, since a plan that uses chords uses them every tick (R23.2).
    pub(crate) chord_claims: Vec<(crate::binding::Control, u8)>,
    // Parallel to `actions`: this action may not fire until it has been seen at rest once. Set when
    // a context activates, so a control the player was already holding does not read as a fresh
    // press (R7.5).
    pub(crate) require_reset: Vec<bool>,
    // Every phase change since the last dispatch, in order. Evaluation appends and the dispatcher
    // drains, which is what keeps observers — arbitrary code with `&mut World` — outside the
    // evaluator (R10.2).
    pub(crate) transitions: Vec<Transition>,
    // A class binding's counterpart to `transitions`: logged by evaluation, drained by dispatch, for
    // the same reason.
    pub(crate) class_fires: Vec<crate::eval::ClassFire>,
    // The last event this context has read. Seeded at spawn rather than left empty, so a context
    // added mid-session starts from the present instead of replaying whatever is still queued.
    pub(crate) read_through: Option<Timestamp>,
    #[cfg(feature = "keyboard")]
    pub(crate) held_buttons: BTreeSet<bevy_input::keyboard::KeyCode>,
    // A `HashSet` rather than the `BTreeSet` the keys get, because `MouseButton` is `Hash` but not
    // `Ord` upstream.
    #[cfg(feature = "mouse")]
    pub(crate) held_mouse_buttons: HashSet<bevy_input::mouse::MouseButton>,
    #[cfg(feature = "gamepad")]
    pub(crate) held_gamepad_buttons: HashMap<bevy_input::gamepad::GamepadButton, ButtonReading>,
    #[cfg(feature = "gamepad")]
    pub(crate) held_gamepad_axes: HashMap<bevy_input::gamepad::GamepadAxis, f32>,
    _marker: PhantomData<C>,
}

impl<C: InputContext> InputContextState<C> {
    pub(crate) fn new(plan: Arc<Plan<C>>, read_through: Option<Timestamp>) -> Self {
        let slots = plan.slot_count();
        let scratch_slots = plan.scratch_count();
        let tunable_scratch_slots = plan.tunable_scratch_count();
        let actions = alloc::vec![ActionState::default(); slots];

        Self {
            plan,
            actions,
            dirty: DirtySet::with_capacity(slots),
            active: true,
            shadowed: false,
            scratch: alloc::vec![Scratch::default(); scratch_slots],
            tunable_scratch: alloc::vec![Scratch::default(); tunable_scratch_slots],
            chord_claims: Vec::new(),
            require_reset: alloc::vec![false; slots],
            transitions: Vec::new(),
            class_fires: Vec::new(),
            read_through,
            #[cfg(feature = "keyboard")]
            held_buttons: BTreeSet::new(),
            #[cfg(feature = "mouse")]
            held_mouse_buttons: HashSet::default(),
            #[cfg(feature = "gamepad")]
            held_gamepad_buttons: HashMap::default(),
            #[cfg(feature = "gamepad")]
            held_gamepad_axes: HashMap::default(),
            _marker: PhantomData,
        }
    }

    fn action_state<A>(&self) -> Option<&ActionState>
    where
        A: InputAction,
    {
        let slot = self.plan.slot_for_action(A::id())?;
        Some(&self.actions[slot])
    }

    /// Whether this context binds an action at all.
    ///
    /// Reading an unbound action is not an error — it reads as though nobody is touching the
    /// control — so this is here for code that would rather ask than infer it from a rest value.
    pub fn is_bound<A>(&self) -> bool
    where
        A: InputAction,
    {
        self.plan.slot_for_action(A::id()).is_some()
    }

    /// Reads the typed action value.
    ///
    /// An action this context does not bind reads as though untouched — `false`, `0.0`, or a zero
    /// vector, depending on the action's output — and says so in the log once. Use
    /// [`try_value`](Self::try_value) where the difference matters.
    pub fn value<A>(&self) -> A::Output
    where
        A: InputAction,
        A::Output: ActionOutput,
    {
        match self.action_state::<A>() {
            Some(state) => A::Output::from_action_value(state.value),
            None => {
                self.warn_unbound::<A>();
                A::Output::REST
            }
        }
    }

    /// Reads the typed action value, or `None` when this context does not bind the action.
    pub fn try_value<A>(&self) -> Option<A::Output>
    where
        A: InputAction,
        A::Output: ActionOutput,
    {
        self.action_state::<A>()
            .map(|state| A::Output::from_action_value(state.value))
    }

    /// Returns the current phase for an action.
    ///
    /// An action this context does not bind is always [`Idle`](Phase::Idle), on the same terms as
    /// [`value`](Self::value).
    pub fn phase<A>(&self) -> Phase
    where
        A: InputAction,
    {
        match self.action_state::<A>() {
            Some(state) => state.phase,
            None => {
                self.warn_unbound::<A>();
                Phase::Idle
            }
        }
    }

    /// Returns `true` when the action was pressed this tick.
    pub fn fired<A>(&self) -> bool
    where
        A: InputAction<Output = bool>,
    {
        self.phase::<A>() == Phase::Fired
    }

    /// Says once that an action was read here but never bound here.
    ///
    /// Once rather than every time because the caller is a system, and a system that reads the
    /// wrong action reads it every tick. Which action and which context is the whole of the
    /// mistake, so both are named, along with what this context does bind — the answer is usually
    /// visible in that list, as a neighbouring action or the same action in another context.
    #[cold]
    fn warn_unbound<A>(&self)
    where
        A: InputAction,
    {
        bevy_utils::once!(log::warn!(
            "`{}` is not bound in `{}`, so it reads as though untouched. Bound here: {}.",
            A::PATH,
            C::PATH,
            BoundPaths(self.plan.bound_paths()),
        ));
    }

    /// Explains why an action is not firing.
    ///
    /// ```ignore
    /// // Why is the ship not thrusting?
    /// info!("{:?}", input.why_not::<Thrust>());
    /// // Consumed { control: Key(KeyW), by: "gameplay.shell" }
    /// ```
    ///
    /// Checked in the order the obstacles apply, so what comes back is the first thing in the way
    /// rather than a list. Clearing it may reveal another.
    pub fn why_not<A>(&self, consumed: &crate::eval::ConsumedControls) -> Obstacle
    where
        A: InputAction,
    {
        self.why_not_id(A::id(), consumed)
    }

    /// Explains why an action named at run time is not firing.
    ///
    /// As [`why_not`](Self::why_not), for a debug overlay or an editor that walks a context's
    /// actions rather than naming one.
    pub fn why_not_id(
        &self,
        action: ActionId,
        consumed: &crate::eval::ConsumedControls,
    ) -> Obstacle {
        let Some(slot) = self.plan.slot_for_action(action) else {
            return Obstacle::Unbound;
        };
        if !self.is_active() {
            return Obstacle::ContextInactive;
        }
        if self.actions[slot].phase == Phase::Fired || self.actions[slot].phase == Phase::Ongoing {
            if self.actions[slot].value.to_bool() {
                return Obstacle::None;
            }
            return Obstacle::ConditionPending;
        }
        if self.actions[slot].phase == Phase::Started {
            return Obstacle::ConditionPending;
        }
        // A control someone else holds is the most useful answer available, so both of these
        // outrank the catch-all below even though all three are "the binding read nothing".
        for binding in self.plan.bindings().iter().filter(|b| b.slot == slot) {
            let mut taken = None;
            let mut outranked = None;
            binding.source.for_each_control(|control| {
                if taken.is_none()
                    && let Some(by) = consumed.claimant(control)
                {
                    taken = Some(Obstacle::Consumed { control, by });
                }
                if outranked.is_none()
                    && let Some(&(_, chord)) = self
                        .chord_claims
                        .iter()
                        .find(|&&(seen, best)| seen == control && best > binding.chord_len)
                {
                    outranked = Some(Obstacle::Outranked { control, chord });
                }
            });
            if let Some(obstacle) = taken.or(outranked) {
                return obstacle;
            }
        }
        if self.require_reset[slot] {
            return Obstacle::AwaitingRelease;
        }
        Obstacle::NoInput
    }

    /// Walks every action this context binds, without naming any of them.
    ///
    /// For code that has to work with whatever actions it is given rather than actions it was
    /// compiled against — a debug overlay, an editor, a settings screen. The typed reads are the
    /// ones to use where the action is known.
    ///
    /// The order is stable for a given set of bindings: actions appear in the order they were first
    /// bound.
    pub fn iter(&self) -> impl Iterator<Item = ActionReading<'_>> {
        self.plan
            .slot_actions()
            .iter()
            .zip(self.plan.bound_paths())
            .zip(&self.actions)
            .map(|((&action, &path), state)| ActionReading {
                action,
                path,
                state,
            })
    }

    /// Whether this context is currently driving its actions.
    ///
    /// An inactive context keeps up with its devices but stops resolving them into actions, so
    /// reactivating it is immediate and costs no rebuilding. `false` either because this context's
    /// own activation says so, or because a higher-priority exclusive context currently shadows it —
    /// the two look the same from here, and everywhere else that asks.
    pub fn is_active(&self) -> bool {
        self.active && !self.shadowed
    }

    /// Starts driving actions again, ignoring controls the player is already holding.
    ///
    /// This is the behaviour you almost always want. Closing a menu with the same key that
    /// interacts with the world would otherwise interact with whatever is in front of the player
    /// the instant the menu disappears, because the key is still down. Each action here waits until
    /// it has been seen at rest once before it can fire again.
    ///
    /// Use [`activate_including_held`](Self::activate_including_held) for the other behaviour.
    ///
    /// "At rest" means the value the action reads after its modifiers have run, so an analog
    /// control needs a deadzone for this to ever be satisfied — a stick that idles at 0.02 is
    /// never exactly at rest, and an action waiting for it would stay quiet indefinitely. Give
    /// sticks a [`DeadZone`](crate::binding::DeadZone), which they want regardless.
    pub fn activate(&mut self) {
        self.activate_with_reset(true);
    }

    /// Starts driving actions again, letting controls already held fire immediately.
    ///
    /// Right when a context takes over from another that was driving the same controls — swapping
    /// a walking context for a sprinting one should not make the player let go of the stick and
    /// push it again.
    pub fn activate_including_held(&mut self) {
        self.activate_with_reset(false);
    }

    fn activate_with_reset(&mut self, require_reset: bool) {
        if self.active {
            return;
        }
        self.active = true;
        self.require_reset.fill(require_reset);
    }

    /// Takes a new set of compiled bindings, which is what applying an override does.
    ///
    /// Whatever was in flight is canceled and every action waits to be seen at rest once — the same
    /// work `deactivate` and `activate` do, and for the same reasons. A hold on a control that is no
    /// longer bound has to resolve rather than stay held for good, and a player still holding the
    /// key they just rebound must not get a fresh press out of the swap.
    ///
    /// The variant keeps the declared plan's slot allocation, so the action table and the
    /// require-reset flags stay aligned and only the scratch has to be rebuilt.
    pub(crate) fn adopt(&mut self, plan: Arc<Plan<C>>) {
        let was_active = self.active;
        self.deactivate();

        self.scratch.clear();
        self.scratch
            .resize(plan.scratch_count(), Scratch::default());
        self.tunable_scratch.clear();
        self.tunable_scratch
            .resize(plan.tunable_scratch_count(), Scratch::default());
        self.chord_claims.clear();
        self.plan = plan;

        // Set directly rather than through `activate`, which returns early on a context that is
        // already live — and this one was just switched off to cancel what it held.
        if was_active {
            self.active = true;
            self.require_reset.fill(true);
        }
    }

    /// Stops driving actions, canceling anything in flight.
    ///
    /// Every action currently held is reported as [`Canceled`](Phase::Canceled) rather than left
    /// where it was, so a hold interrupted by a menu opening resolves instead of staying held for
    /// as long as the menu is up.
    pub fn deactivate(&mut self) {
        if !self.active {
            return;
        }
        self.active = false;
        self.cancel_in_flight();
    }

    /// Suppresses this context for as long as a higher-priority exclusive context is active,
    /// without touching what its own activation thinks — that stays `active`'s business, so the two
    /// do not fight each other once the shadow lifts.
    ///
    /// Cancels in-flight actions exactly as `deactivate` does, because a control held through a
    /// modal opening must not stay "held forever" any more than one held through an ordinary
    /// deactivation would.
    pub(crate) fn shadow(&mut self) {
        if self.shadowed {
            return;
        }
        self.shadowed = true;
        self.cancel_in_flight();
    }

    /// Lifts a shadow, re-arming require-reset so a control still held when it lifts does not
    /// read as a fresh press.
    pub(crate) fn unshadow(&mut self) {
        if !self.shadowed {
            return;
        }
        self.shadowed = false;
        self.require_reset.fill(true);
    }

    /// Reports every `Fired`/`Ongoing` action as `Canceled` rather than left where it was — the one
    /// piece `deactivate` and `shadow` share.
    fn cancel_in_flight(&mut self) {
        for (slot, state) in self.actions.iter_mut().enumerate() {
            if !matches!(state.phase, Phase::Fired | Phase::Ongoing) {
                continue;
            }
            state.phase = Phase::Canceled;
            state.value = rest_like(state.value);
            self.dirty.set(slot);
            self.transitions.push(Transition {
                slot,
                phase: Phase::Canceled,
                value: state.value,
            });
        }
    }
}

/// Formats a context's bound action paths for a diagnostic, without building a string to do it.
struct BoundPaths<'a>(&'a [&'static str]);

impl core::fmt::Display for BoundPaths<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.0.is_empty() {
            return f.write_str("nothing — this context binds no actions at all");
        }
        for (index, path) in self.0.iter().enumerate() {
            if index > 0 {
                f.write_str(", ")?;
            }
            f.write_str(path)?;
        }
        Ok(())
    }
}

/// Zero, in whatever shape the value already had.
fn rest_like(value: crate::action::ActionValue) -> crate::action::ActionValue {
    use crate::action::ActionValue;
    match value {
        ActionValue::Bool(_) => ActionValue::Bool(false),
        ActionValue::Axis1(_) => ActionValue::Axis1(0.0),
        ActionValue::Axis2(_) => ActionValue::Axis2(bevy_math::Vec2::ZERO),
        ActionValue::Axis3(_) => ActionValue::Axis3(bevy_math::Vec3::ZERO),
    }
}

/// One action's identity and current state, as read by code that did not name it.
///
/// Produced by [`InputContextState::iter`].
#[derive(Clone, Copy, Debug)]
pub struct ActionReading<'a> {
    /// The action's runtime identity.
    pub action: ActionId,
    /// Its declared path, which is what to show a human.
    pub path: &'static str,
    /// What it is currently doing.
    pub state: &'a ActionState,
}

/// Why an action is not firing.
///
/// When an action does not fire, the cause is invisible from the call site: an inactive context, a
/// higher-priority context that took the control, a condition still counting, and a control nobody
/// touched all look exactly alike. This names which it was.
///
/// Meant for a debug overlay, a log line, or a breakpoint condition — not for game logic. What it
/// reports is the *first* obstacle found, so clearing one may reveal another.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Obstacle {
    /// Nothing is in the way: the action is firing.
    None,
    /// This action is not bound in this context.
    ///
    /// Usually a typo, or reading the right action from the wrong context.
    Unbound,
    /// The context is not active, so none of its bindings are being read.
    ContextInactive,
    /// The context has just activated and this control was already held.
    ///
    /// It will fire once the player has let go and pressed again. See
    /// [`activate`](InputContextState::activate).
    AwaitingRelease,
    /// A higher-priority context took one of the controls this action reads.
    Consumed {
        /// The control that was taken.
        control: crate::binding::Control,
        /// The `PATH` of the context that took it.
        by: &'static str,
    },
    /// A longer chord on one of this action's controls took it.
    ///
    /// `Ctrl+S` firing is why a plain `S` binding did not. See
    /// [`with`](crate::binding::BindingHandle::with).
    Outranked {
        /// The control the longer chord took.
        control: crate::binding::Control,
        /// How many controls that chord requires held, this one included.
        chord: u8,
    },
    /// A condition has begun but has not been satisfied — a hold part way through.
    ConditionPending,
    /// Nothing has touched any control this action is bound to.
    NoInput,
}

/// System parameter for polling the actions of a context with exactly one instance.
///
/// Most games have one of a given context — one on-foot context, one menu context — and this reads
/// it directly: [`value`](Self::value), [`phase`](Self::phase) and [`fired`](Self::fired) need no
/// entity, because there is only one it could mean.
///
/// **A system taking this does not run unless exactly one entity carries the context.** No
/// instance yet, the entity despawned, or several at once, and the system is skipped for that run
/// rather than failing — the same rule Bevy's [`Single`] follows, and for the same reason: a system
/// about the player's ship has nothing to do while there is no ship.
///
/// Use [`ActionsQuery`] instead where a context is per-player, or where the system has work to do
/// whether or not an instance exists.
///
/// [`Single`]: bevy_ecs::system::Single
#[derive(SystemParam)]
pub struct Actions<'w, 's, C: InputContext + Component> {
    state: bevy_ecs::system::Single<'w, 's, (Entity, &'static InputContextState<C>)>,
    consumed: bevy_ecs::system::Res<'w, crate::eval::ConsumedControls>,
}

impl<C: InputContext + Component> Actions<'_, '_, C> {
    /// Returns the entity carrying this context.
    pub fn entity(&self) -> Entity {
        self.state.0
    }

    /// Returns the state of the one instance.
    pub fn state(&self) -> &InputContextState<C> {
        self.state.1
    }

    /// Reads the typed action value.
    ///
    /// See [`InputContextState::value`] for what an unbound action reads as.
    pub fn value<A>(&self) -> A::Output
    where
        A: InputAction,
        A::Output: ActionOutput,
    {
        self.state().value::<A>()
    }

    /// Reads the typed action value, or `None` when this context does not bind the action.
    pub fn try_value<A>(&self) -> Option<A::Output>
    where
        A: InputAction,
        A::Output: ActionOutput,
    {
        self.state().try_value::<A>()
    }

    /// Returns the current phase for an action.
    pub fn phase<A>(&self) -> Phase
    where
        A: InputAction,
    {
        self.state().phase::<A>()
    }

    /// Returns `true` when the action was pressed this tick.
    pub fn fired<A>(&self) -> bool
    where
        A: InputAction<Output = bool>,
    {
        self.state().fired::<A>()
    }

    /// Explains why an action is not firing.
    ///
    /// See [`InputContextState::why_not`].
    pub fn why_not<A>(&self) -> Obstacle
    where
        A: InputAction,
    {
        self.state().why_not::<A>(&self.consumed)
    }
}

/// System parameter for polling every instance of a context.
///
/// For a context that is per-player, or any other case where there is not exactly one: unlike
/// [`Actions`], a system taking this runs whether or not any instance exists, and reads them by
/// entity with [`get`](Self::get) or all at once with [`iter`](Self::iter).
///
/// ```ignore
/// fn drive(all: ActionsQuery<Piloting>, ships: Query<&mut Transform>) {
///     for (player, input) in all.iter() {
///         let turn = input.value::<Turn>();
///     }
/// }
/// ```
#[derive(SystemParam)]
pub struct ActionsQuery<'w, 's, C: InputContext + Component> {
    states: Query<'w, 's, (Entity, &'static InputContextState<C>)>,
    consumed: bevy_ecs::system::Res<'w, crate::eval::ConsumedControls>,
}

impl<C: InputContext + Component> ActionsQuery<'_, '_, C> {
    /// Returns the instance carried by an entity, if it has one.
    pub fn get(&self, entity: Entity) -> Option<&InputContextState<C>> {
        self.states.get(entity).ok().map(|(_, state)| state)
    }

    /// Iterates every instance of this context and the entity carrying it.
    pub fn iter(&self) -> impl Iterator<Item = (Entity, &InputContextState<C>)> {
        self.states.iter()
    }

    /// How many entities carry this context.
    pub fn len(&self) -> usize {
        self.states.iter().len()
    }

    /// Whether no entity carries this context.
    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }

    /// Explains why an action is not firing on one instance.
    ///
    /// Returns [`Obstacle::Unbound`] when the entity carries no such context, since from the call
    /// site that is indistinguishable from an action nobody bound.
    ///
    /// See [`InputContextState::why_not`].
    pub fn why_not<A>(&self, entity: Entity) -> Obstacle
    where
        A: InputAction,
    {
        self.get(entity).map_or(Obstacle::Unbound, |state| {
            state.why_not::<A>(&self.consumed)
        })
    }
}

/// Gives a newly added context entity the state tables for its bindings.
///
/// Registered by [`add_context`](ActionMapAppExt::add_context), which is what lets an entity
/// spawned from a scene or a template work with no setup call of its own.
fn attach_context_state<C: InputContext + Component>(
    mut world: DeferredWorld<'_>,
    context: HookContext,
) {
    // `add_context` inserts the plan before registering this hook, so the resource is present
    // whenever the hook can run.
    let Some(declared) = world.get_resource::<InputContextPlan<C>>() else {
        return;
    };

    let starts_active = declared.starts_active;
    // The current bindings rather than the declared ones, so that an instance arriving after a
    // rebind — a player joining, a context respawned with a game state — is bound the way the
    // player left it rather than silently reverting to what the game shipped.
    let plan = world
        .get_resource::<AppliedPlan<C>>()
        .map_or_else(|| declared.plan.clone(), |applied| applied.plan.clone());
    // Whatever is already queued happened before this context existed, so it is not this
    // context's input to react to (R7.5).
    let read_through = world
        .get_resource::<InputFrame>()
        .and_then(InputFrame::latest);
    let mut state = InputContextState::<C>::new(plan, read_through);
    state.active = starts_active;
    world.commands().entity(context.entity).insert(state);
}

/// Says that a prompt naming this context's controls may now say something else.
///
/// Registered on the *state* rather than on `C`, because it is the state that says whether the
/// context is being carried at all: a prompt depends on whether any instance exists, so it
/// changes when the first one appears and when the last one goes away, with nothing calling
/// `activate` in either case. Not generic — the hook is the same code for every context, and one
/// copy of it is enough.
fn invalidate_prompts(mut world: DeferredWorld<'_>, _context: HookContext) {
    crate::present::PromptGeneration::invalidate(&mut world.commands());
}

/// Installs whatever decides when a context is live, once the context itself is declared.
///
/// Boxed because the condition's type is only known inside the closure `add_context` hands to the
/// caller, and it has to outlive that closure to reach the `App`.
pub(crate) type Activation = alloc::boxed::Box<dyn FnOnce(&mut App)>;

/// Brings every instance of `C` in step with what its condition just answered.
///
/// The one mechanism behind both [`active_if`](InputContextBuilder::active_if) and
/// [`active_in_state`](InputContextBuilder::active_in_state): the condition is an ordinary system
/// piped into this one, so it gets the same dependency injection as anything else and needs no
/// exclusive access to the world.
fn apply_active<C: InputContext + Component>(
    bevy_ecs::system::In(live): bevy_ecs::system::In<bool>,
    contexts: Query<'_, '_, &mut InputContextState<C>>,
    mut commands: bevy_ecs::system::Commands<'_, '_>,
    mut was_empty: bevy_ecs::system::Local<'_, bool>,
) {
    // Something said this context should be live and there is nothing to make live, which is the
    // shape of a context declared but never spawned: every action in it is dead and the symptom is
    // that a key does nothing. Only after two runs, because an entity spawned from `OnEnter` does
    // not exist yet on the frame its state became current.
    let empty = contexts.is_empty();
    if live && empty && *was_empty {
        bevy_utils::once!(log::warn!(
            "context `{}` is active, but no entity carries it — none of its bindings can fire. \
             Spawn an entity with the `{}` component.",
            C::PATH,
            core::any::type_name::<C>(),
        ));
    }
    *was_empty = empty;

    let mut changed = false;
    for mut context in contexts {
        // `activate` and `deactivate` both return immediately when there is nothing to do; the
        // check here is what keeps the mutable deref, and with it the change tick, off the frames
        // where nothing happened.
        if context.is_active() == live {
            continue;
        }
        if live {
            context.activate();
        } else {
            context.deactivate();
        }
        changed = true;
    }

    // Once for the edge, not once per instance: a prompt is the same answer however many entities
    // carry the context.
    if changed {
        crate::present::PromptGeneration::invalidate(&mut commands);
    }
}

impl<C: InputContext + Component> InputContextBuilder<C> {
    /// Makes a run condition decide whether this context is live.
    ///
    /// The condition is polled every frame, ahead of the evaluation that reads the bindings, and
    /// its answer is applied to every instance of the context: true activates, false deactivates.
    /// Any Bevy run condition works, including combinations of them.
    ///
    /// ```ignore
    /// app.add_context::<Piloting>(|controls| {
    ///     controls.active_if(any_with_component::<InVehicle>);
    ///     controls.bind::<Throttle>(GamepadButton::RightTrigger2);
    /// });
    /// ```
    ///
    /// A context with a condition starts inactive and stays that way until the condition first
    /// says otherwise, so it never fires for a frame before the thing it follows has been asked.
    /// Activation ignores controls the player is already holding, exactly as
    /// [`activate`](InputContextState::activate) describes.
    ///
    /// Use [`active_in_state`](Self::active_in_state) for a context that follows a game state:
    /// `in_state` would work here, but it is worth a frame to a fixed-tick context.
    ///
    /// Leave both off for a context you drive yourself, per instance, with
    /// [`activate`](InputContextState::activate) and
    /// [`deactivate`](InputContextState::deactivate) — a condition answers for every instance at
    /// once, so the two do not mix.
    ///
    /// # Panics
    ///
    /// Panics if this context has already been given a condition.
    pub fn active_if<M: 'static>(
        &mut self,
        condition: impl bevy_ecs::schedule::SystemCondition<M> + 'static,
    ) -> &mut Self {
        self.set_activation(alloc::boxed::Box::new(move |app: &mut App| {
            app.add_systems(
                PreUpdate,
                condition
                    .pipe(apply_active::<C>)
                    .before(ActionMapSystems::Evaluate),
            );
        }))
    }

    /// Makes this context live exactly while the app is in one state.
    ///
    /// Most contexts come and go with the game's state — flying while playing, a menu while
    /// paused — and keeping the two in step by hand is a bug waiting to happen, because it is the
    /// kind of thing that stays correct until someone adds a third way to reach the menu.
    ///
    /// ```ignore
    /// app.add_context::<Flying>(|controls| {
    ///     controls.active_in_state(GameState::Playing);
    ///     controls.bind::<Thrust>(KeyCode::KeyW);
    /// });
    /// app.add_context::<PauseMenu>(|controls| {
    ///     controls.active_in_state(GameState::Paused);
    ///     controls.bind::<Resume>(KeyCode::Escape);
    /// });
    /// ```
    ///
    /// Entering the state activates the context, and leaving it deactivates it — which cancels
    /// whatever was in flight, and means a control the player is already holding when the state
    /// changes does not read as a fresh press. That last part is what stops one key both closing a
    /// menu and acting on the world behind it.
    ///
    /// An instance spawned while the state is already current is activated too, so this works for
    /// a context that arrives with a player rather than at startup.
    ///
    /// A [`SubStates`](bevy_state::prelude::SubStates) or a computed state works here too: while
    /// its parent does not select it there is no such state to be in, and the context is inactive.
    /// A state the app never initialized reads the same way, so a context following one stays
    /// quiet rather than bringing the app down.
    ///
    /// This is `active_if(in_state(state))` placed where the state has just changed rather than
    /// where the frame started, which is what lets a fixed-tick context stand down in time for the
    /// same frame's simulation, and lets an `OnEnter` system find the context already in step.
    ///
    /// # Panics
    ///
    /// Panics if this context has already been given a condition.
    #[cfg(feature = "state")]
    #[cfg_attr(docsrs, doc(cfg(feature = "state")))]
    pub fn active_in_state(&mut self, state: impl bevy_state::prelude::States) -> &mut Self {
        self.set_activation(alloc::boxed::Box::new(move |app: &mut App| {
            use bevy_ecs::system::IntoSystem;
            use bevy_state::prelude::in_state;
            use bevy_state::state::{StateTransition, StateTransitionSystems};

            // After the transition is computed and before the exit and enter schedules run, so a
            // context is already in step by the time an `OnEnter` system looks at it.
            app.add_systems(
                StateTransition,
                in_state(state)
                    .pipe(apply_active::<C>)
                    .after(StateTransitionSystems::DependentTransitions)
                    .before(StateTransitionSystems::ExitSchedules),
            );
        }))
    }

    fn set_activation(&mut self, activation: Activation) -> &mut Self {
        assert!(
            self.activation.is_none(),
            "context {} already has a condition deciding when it is active",
            C::PATH
        );
        self.activation = Some(activation);
        self
    }
}

/// Extension methods for setting up contexts.
pub trait ActionMapAppExt {
    /// Declares one context and the bindings that drive it.
    ///
    /// This compiles the bindings once and arranges for any entity carrying `C` to receive the
    /// state for them. Spawn that entity wherever it belongs — on the player, one per local
    /// player, or on its own — and the context is live from then on with no further setup:
    ///
    /// ```ignore
    /// app.add_context::<OnFoot>(|controls| {
    ///     controls.bind::<Jump>(KeyCode::Space);
    /// });
    /// // ...then, in a startup system or a scene:
    /// commands.spawn((Player, OnFoot));
    /// ```
    ///
    /// The context's tick domain decides where it is evaluated: a `Render` context runs in
    /// `PreUpdate` and a `Fixed` context in `FixedPreUpdate`, both before the schedule you would
    /// normally read the actions from.
    ///
    /// A context declared this way is live as soon as an entity carries it. Give it an
    /// [`active_if`](InputContextBuilder::active_if) or an
    /// [`active_in_state`](InputContextBuilder::active_in_state) when it should follow something
    /// else instead, or drive it yourself with [`activate`](InputContextState::activate) and
    /// [`deactivate`](InputContextState::deactivate).
    ///
    /// # Panics
    ///
    /// Panics if [`ActionMapPlugin`] has not been added, if the same context is declared twice, or
    /// if an entity already carries `C`.
    fn add_context<C: InputContext + Component>(
        &mut self,
        configure: impl FnOnce(&mut InputContextBuilder<C>),
    ) -> &mut Self;
}

impl ActionMapAppExt for App {
    fn add_context<C: InputContext + Component>(
        &mut self,
        configure: impl FnOnce(&mut InputContextBuilder<C>),
    ) -> &mut Self {
        declare_context(self, configure);
        self
    }
}

/// Where one priority's contexts evaluate, relative to the others in the same schedule.
///
/// A value-typed set rather than a marker, so that the priority a context declares becomes the
/// ordering directly. Higher priorities run first, which is what gives them the chance to claim a
/// control before anyone else reads it.
#[derive(bevy_ecs::schedule::SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct EvaluateAt(i32);

/// Which priorities have been seen, so a new one can be ordered against them.
#[derive(Resource, Default)]
struct DeclaredPriorities {
    render: alloc::collections::BTreeSet<i32>,
    fixed: alloc::collections::BTreeSet<i32>,
}

/// Orders a newly seen priority against every other in its schedule.
///
/// Done once per distinct priority at app build rather than per frame, keeping the single
/// deterministic evaluation pass free of any run-time ordering decision. The number of distinct
/// priorities is small — a handful of layers, not a handful per context.
fn order_by_priority(
    app: &mut App,
    schedule: impl bevy_ecs::schedule::ScheduleLabel + Clone,
    domain: TickDomain,
    priority: i32,
) {
    let others: alloc::vec::Vec<i32> = {
        let mut declared = app
            .world_mut()
            .get_resource_or_insert_with(DeclaredPriorities::default);
        let seen = match domain {
            TickDomain::Render => &mut declared.render,
            TickDomain::Fixed => &mut declared.fixed,
        };
        if !seen.insert(priority) {
            return;
        }
        seen.iter().copied().filter(|&p| p != priority).collect()
    };

    for other in others {
        if priority > other {
            app.configure_sets(
                schedule.clone(),
                EvaluateAt(priority).before(EvaluateAt(other)),
            );
        } else {
            app.configure_sets(
                schedule.clone(),
                EvaluateAt(priority).after(EvaluateAt(other)),
            );
        }
    }
    app.configure_sets(
        schedule,
        EvaluateAt(priority).in_set(ActionMapSystems::Evaluate),
    );
}

/// Reads the mappings of one context back out once its type is no longer known.
///
/// Registered per context by `add_context`, which is the last place `C` is available.
fn read_mappings<C: InputContext + Component>(
    world: &World,
) -> alloc::vec::Vec<crate::mapping::Mapping> {
    // What is bound now, so a settings screen and a conflict check both read the controls the
    // player is actually using. `read_declared_mappings` is the one that answers about defaults.
    if let Some(applied) = world.get_resource::<AppliedPlan<C>>() {
        return applied.mappings.clone();
    }
    world
        .get_resource::<InputContextPlan<C>>()
        .map(|declared| declared.mappings.clone())
        .unwrap_or_default()
}

/// Reads the mappings this context *declared*, whatever has since been applied over them.
///
/// The other half of `read_mappings`, which answers about current values. Registered separately
/// rather than taking a flag, because the two are asked by different callers for different reasons.
fn read_declared_mappings<C: InputContext + Component>(
    world: &World,
) -> alloc::vec::Vec<crate::mapping::Mapping> {
    world
        .get_resource::<InputContextPlan<C>>()
        .map(|declared| declared.mappings.clone())
        .unwrap_or_default()
}

/// Reads the tunables of one context back out once its type is no longer known.
///
/// The tunable half of [`read_mappings`], for the same reason and the same caller.
fn read_tunables<C: InputContext + Component>(
    world: &World,
) -> alloc::vec::Vec<crate::mapping::Tunable> {
    if let Some(applied) = world.get_resource::<AppliedPlan<C>>() {
        return applied.tunables.clone();
    }
    world
        .get_resource::<InputContextPlan<C>>()
        .map(|declared| declared.tunables.clone())
        .unwrap_or_default()
}

/// Reads the tunables this context *declared*, whatever has since been applied over them.
fn read_declared_tunables<C: InputContext + Component>(
    world: &World,
) -> alloc::vec::Vec<crate::mapping::Tunable> {
    world
        .get_resource::<InputContextPlan<C>>()
        .map(|declared| declared.tunables.clone())
        .unwrap_or_default()
}

/// Rewrites one context's bindings for an override set, and swaps the result into every instance.
///
/// Registered per context by `add_context`, like the readers above, and for the same reason: this is
/// the last place `C` is available. `preset` names the rows a preset authorized, exempting exactly
/// those from the "not rebindable here" refusal that would otherwise stop a preset moving a `Fixed`
/// row — see [`apply_overrides_with_preset`](crate::overrides::apply_overrides_with_preset).
fn apply_to_context<C: InputContext + Component>(
    world: &mut World,
    overrides: &crate::overrides::Overrides,
    preset: Option<&crate::overrides::Overrides>,
) -> alloc::vec::Vec<crate::overrides::OverrideProblem> {
    let Some(declared) = world.get_resource::<InputContextPlan<C>>() else {
        return alloc::vec::Vec::new();
    };
    // Read out before anything is written, so the compile below borrows nothing from the world.
    let bindings = declared.bindings.clone();
    let rows = declared.mappings.clone();
    let tunables = declared.tunables.clone();
    let template = declared.plan.clone();
    let reserved: alloc::vec::Vec<crate::binding::Control> = world
        .get_resource::<crate::capture::ReservedControls>()
        .map(|reserved| reserved.iter().map(|entry| entry.control).collect())
        .unwrap_or_default();

    let (variant, mappings, tunables, problems) = crate::overrides::rewrite(
        &bindings,
        &rows,
        &tunables,
        overrides,
        preset,
        &reserved,
        C::PATH,
    );
    let plan = Arc::new(Plan::variant_of(&template, variant));

    world.insert_resource(AppliedPlan::<C> {
        plan: plan.clone(),
        mappings,
        tunables,
    });

    let mut instances = world.query::<&mut InputContextState<C>>();
    for mut state in instances.iter_mut(world) {
        state.adopt(plan.clone());
    }

    problems
}

/// Like `apply_to_context`, but reaches one named entity's own instance rather than every one.
///
/// Deliberately does not touch `AppliedPlan<C>` — that resource is what a freshly spawned instance
/// inherits at spawn, and a per-entity apply must not change what the *next* new instance gets, only
/// what this one already-spawned instance has. The diff is still computed against
/// `InputContextPlan<C>`'s pristine declaration, the same baseline `apply_to_context` diffs against,
/// so two entities can diverge independently without either becoming the new default.
fn apply_to_entity<C: InputContext + Component>(
    world: &mut World,
    entity: Entity,
    overrides: &crate::overrides::Overrides,
    preset: Option<&crate::overrides::Overrides>,
) -> alloc::vec::Vec<crate::overrides::OverrideProblem> {
    if world.get::<InputContextState<C>>(entity).is_none() {
        return alloc::vec::Vec::new();
    }
    let Some(declared) = world.get_resource::<InputContextPlan<C>>() else {
        return alloc::vec::Vec::new();
    };
    let bindings = declared.bindings.clone();
    let rows = declared.mappings.clone();
    let tunables = declared.tunables.clone();
    let template = declared.plan.clone();
    let reserved: alloc::vec::Vec<crate::binding::Control> = world
        .get_resource::<crate::capture::ReservedControls>()
        .map(|reserved| reserved.iter().map(|entry| entry.control).collect())
        .unwrap_or_default();

    let (variant, _mappings, _tunables, problems) = crate::overrides::rewrite(
        &bindings,
        &rows,
        &tunables,
        overrides,
        preset,
        &reserved,
        C::PATH,
    );
    let plan = Arc::new(Plan::variant_of(&template, variant));

    if let Some(mut state) = world.get_mut::<InputContextState<C>>(entity) {
        state.adopt(plan);
    }

    problems
}

/// Reads one context's bindings back out for a reverse lookup, once its type is no longer known.
///
/// Registered beside `read_mappings`, and answering a different question: this one is about what
/// would fire now, so it reads the compiled plan rather than the presentation rows, and it asks
/// whether anything is carrying the context at all.
fn read_bindings<C: InputContext + Component>(world: &World) -> crate::present::ContextBindings {
    use crate::present::{BoundControl, ContextBindings};

    // What is bound now rather than what was declared: a prompt names the control that would fire
    // the action, and after a rebind that is the control the player chose.
    let plan = match world.get_resource::<AppliedPlan<C>>() {
        Some(applied) => &applied.plan,
        None => match world.get_resource::<InputContextPlan<C>>() {
            Some(declared) => &declared.plan,
            None => return ContextBindings::default(),
        },
    };

    // A context nobody carries, or one that is switched off, fires nothing — and a prompt naming
    // its controls would be telling the player to press a key that does nothing. Read-only, which
    // is what keeps a lookup callable from an ordinary system rather than an exclusive one.
    let active = world
        .try_query::<&InputContextState<C>>()
        .is_some_and(|mut instances| instances.iter(world).any(InputContextState::is_active));

    let mut prompts = alloc::vec::Vec::new();
    let mut claims = alloc::vec::Vec::new();
    for binding in plan.bindings() {
        let action = plan.slot_actions()[binding.slot];
        #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
        let chord: alloc::vec::Vec<crate::binding::Control> = binding
            .chord
            .iter()
            .copied()
            .map(crate::binding::Control::from)
            .collect();
        #[cfg(not(any(feature = "keyboard", feature = "mouse", feature = "gamepad")))]
        let chord: alloc::vec::Vec<crate::binding::Control> = alloc::vec::Vec::new();
        let condition = crate::condition::describe(&binding.conditions);

        // By part rather than by control, so that a composite answers once per direction and a
        // stick answers once rather than twice — the same view the presentation model takes.
        binding.source.for_each_part(|part, control| {
            prompts.push(BoundControl {
                action,
                part,
                control,
                chord: chord.clone(),
                condition,
            });
        });
        // Claims are by control, because taking a composite takes every control in it.
        if binding.consume {
            binding
                .source
                .for_each_control(|control| claims.push((control, action)));
        }
    }

    ContextBindings {
        active,
        prompts,
        claims,
    }
}

fn read_instances<C: InputContext + Component>(
    world: &mut World,
) -> alloc::vec::Vec<crate::inspect::InstanceDump> {
    use crate::inspect::{ActionDump, InstanceDump};

    // Stands in when the plugin is absent, which is only reachable from a test that built the
    // world by hand. Held here so the borrow below has something to point at.
    let nothing_consumed = crate::eval::ConsumedControls::default();

    let mut instances = world.query::<(Entity, &InputContextState<C>)>();
    let consumed = world
        .get_resource::<crate::eval::ConsumedControls>()
        .unwrap_or(&nothing_consumed);

    instances
        .iter(world)
        .map(|(entity, state)| InstanceDump {
            entity,
            active: state.is_active(),
            actions: state
                .iter()
                .map(|reading| ActionDump {
                    action: reading.action,
                    path: reading.path,
                    state: *reading.state,
                    obstacle: state.why_not_id(reading.action, consumed),
                })
                .collect(),
        })
        .collect()
}

/// Warns about every suspicious binding in a context, and refuses one that cannot work.
///
/// All of them at once: a context with three mistakes should cost one run to find all three, not
/// three runs to find them one at a time. Refusing is a panic because this is app-build code —
/// unreachable in a shipped game, and Bevy's own convention for a plugin that has been set up
/// wrongly.
fn report_diagnostics<C: InputContext + Component>(builder: &InputContextBuilder<C>) {
    use crate::plan::Severity;

    let found = builder.diagnostics();
    let errors = found
        .iter()
        .filter(|diagnostic| diagnostic.severity() == Severity::Error)
        .count();

    for diagnostic in found.iter().filter(|d| d.severity() == Severity::Warning) {
        log::warn!("in context `{}`: {diagnostic}", C::PATH);
    }

    assert!(
        errors == 0,
        "context `{}` has {errors} binding {} that cannot work:\n{}",
        C::PATH,
        if errors == 1 { "problem" } else { "problems" },
        Listed(&found),
    );
}

/// Refuses a mapping name another context has already taken.
///
/// The within-a-context case is a plan-build diagnostic like any other, but this one cannot be:
/// a context is compiled without seeing the others, and one action bound in two of them derives
/// the same key twice. What makes it findable is the registry of what has already been declared.
fn report_mapping_collisions<C: InputContext + Component>(
    app: &App,
    mappings: &[crate::mapping::Mapping],
) {
    let Some(declared) = app
        .world()
        .get_resource::<crate::inspect::DeclaredContexts>()
    else {
        return;
    };

    for context in &declared.0 {
        for taken in (context.mappings)(app.world()) {
            // Per scheme, like the within-a-context check: one action mappable on both the keyboard
            // and the pad is two rows in two tables, not a collision.
            //
            // Unlike the within-a-context check, the *action* is not consulted, and the asymmetry
            // is the point. Two mappable bindings of one action inside one context are a primary
            // and a secondary and merge into one row. The same two in two different contexts are
            // two rows, in two contexts that may be active at different times — and the overrides
            // store is keyed by mapping alone (§10.1), so a rebind of one still lands on the other.
            // Same action, and still a collision.
            //
            // Rebindable rows only, for the reason the within-a-context check gives: the hazard is a
            // *saved* rebind landing on the wrong row, and a fixed row is never saved. Since listing
            // is the default, anything stricter would fail the build of any game binding one action
            // in two contexts — which is ordinary, and which R19.13 promises keeps working for a
            // game that offers no rebinding at all.
            if let Some(clash) = mappings.iter().find(|mapping| {
                mapping.key == taken.key
                    && mapping.scheme == taken.scheme
                    && (mapping.rebinding.is_rebindable() || taken.rebinding.is_rebindable())
            }) {
                panic!(
                    "context `{}` declares a mapping named `{}`, which context `{}` already \
                     uses. A saved rebinding of one would land on the other; give one of them a \
                     name with `mappable_as`.\n  here:  {}\n  there: {}",
                    C::PATH,
                    clash.key,
                    context.path,
                    clash.action_path,
                    taken.action_path,
                );
            }
        }
    }
}

/// Formats diagnostics one per line, for a panic message that has to carry several.
struct Listed<'a>(&'a [crate::plan::BindingDiagnostic]);

impl core::fmt::Display for Listed<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        use crate::plan::Severity;

        for diagnostic in self.0.iter().filter(|d| d.severity() == Severity::Error) {
            writeln!(f, "  - {diagnostic}")?;
        }
        Ok(())
    }
}

fn declare_context<C: InputContext + Component>(
    app: &mut App,
    configure: impl FnOnce(&mut InputContextBuilder<C>),
) {
    // The plugin owns the set ordering, so a context added before it would evaluate
    // unordered against the sampler.
    assert!(
        app.is_plugin_added::<ActionMapPlugin>(),
        "add ActionMapPlugin before calling add_context"
    );

    let mut builder = InputContextBuilder::<C>::default();
    configure(&mut builder);

    report_diagnostics::<C>(&builder);

    let mappings = builder.mappings(C::PATH);
    report_mapping_collisions::<C>(app, &mappings);
    let tunables = builder.tunables(C::PATH);

    // Flat and global, unlike mappings: reserving withholds a control from every capture in its
    // scheme, including captures for mappings declared in other contexts.
    app.world_mut()
        .get_resource_or_insert_with(crate::capture::ReservedControls::default)
        .0
        .extend(builder.reserved(C::PATH));

    // Recorded while `C` is still available: after this, nothing can name the type, so a tool that
    // walks every context has to be handed the way in now.
    app.world_mut()
        .get_resource_or_insert_with(crate::inspect::DeclaredContexts::default)
        .0
        .push(crate::inspect::DeclaredContext {
            path: C::PATH,
            tick: C::TICK,
            priority: C::PRIORITY,
            read: read_instances::<C>,
            mappings: read_mappings::<C>,
            bindings: read_bindings::<C>,
            declared_mappings: read_declared_mappings::<C>,
            tunables: read_tunables::<C>,
            declared_tunables: read_declared_tunables::<C>,
            apply: apply_to_context::<C>,
            apply_for_entity: apply_to_entity::<C>,
        });

    // A context whose activation follows something else starts inactive and waits to be asked.
    // That is also what catches an instance spawned once the answer is already yes.
    let activation = builder.activation.take();
    let starts_active = activation.is_none();

    let (bindings, class_bindings) = builder.finish();
    let plan = Arc::new(Plan::from_bindings(bindings.clone(), class_bindings));
    app.insert_resource(InputContextPlan::<C> {
        plan,
        bindings,
        mappings,
        tunables,
        starts_active,
    });

    // The hook can only be attached while no entity carries `C` yet, so declaring a context
    // has to precede spawning into it. Bevy's own assertion here says nothing about
    // `add_context`, which is why this one exists.
    let world = app.world_mut();
    let mut existing = world.query::<&C>();
    assert!(
        existing.iter(world).next().is_none(),
        "declare context {} with add_context before spawning an entity that carries it",
        C::PATH
    );

    assert!(
        world
            .register_component_hooks::<C>()
            .try_on_add(attach_context_state::<C>)
            .is_some(),
        "context {} is already declared, or its component already has an on_add hook",
        C::PATH
    );

    // `InputContextState<C>` is ours alone, so these can only fail the way the assertion above
    // fails: the same context declared twice.
    assert!(
        world
            .register_component_hooks::<InputContextState<C>>()
            .try_on_add(invalidate_prompts)
            .and_then(|hooks| hooks.try_on_remove(invalidate_prompts))
            .is_some(),
        "context {} is already declared",
        C::PATH
    );

    let dispatch = dispatch_transitions::<C>.in_set(ActionMapSystems::Dispatch);
    let dispatch_classes = dispatch_class_fires::<C>.in_set(ActionMapSystems::Dispatch);
    let order = EvaluateAt(C::PRIORITY);
    match C::TICK {
        TickDomain::Render => {
            app.add_systems(
                PreUpdate,
                (
                    evaluate_context::<C, PreUpdate>.in_set(order),
                    dispatch,
                    dispatch_classes,
                ),
            );
            order_by_priority(app, PreUpdate, TickDomain::Render, C::PRIORITY);
        }
        TickDomain::Fixed => {
            app.add_systems(
                FixedPreUpdate,
                (
                    evaluate_context::<C, FixedPreUpdate>.in_set(order),
                    dispatch,
                    dispatch_classes,
                ),
            );
            order_by_priority(app, FixedPreUpdate, TickDomain::Fixed, C::PRIORITY);
        }
    }

    // Last, so that the condition's system is ordered against evaluation that already exists.
    if let Some(install) = activation {
        install(app);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{InputAction, InputContext};
    use bevy_app::{App, FixedUpdate, Update};
    use bevy_ecs::prelude::{Component, Resource};
    use bevy_input::{
        ButtonState, InputPlugin, keyboard::Key, keyboard::KeyCode, keyboard::KeyboardInput,
        mouse::MouseMotion,
    };
    use bevy_math::Vec2;

    #[cfg(feature = "gamepad")]
    use crate::binding::Stick;
    use crate::binding::{AxisButtons, ButtonThreshold, DeadZone, DirectionalButtons, MouseMove};
    use crate::frame::InputFrame;
    #[cfg(feature = "gamepad")]
    use bevy_input::gamepad::{
        GamepadAxis, GamepadButton, RawGamepadAxisChangedEvent, RawGamepadButtonChangedEvent,
        RawGamepadEvent,
    };

    #[derive(InputAction)]
    #[action(path = "tests.jump", output = bool, intent = Button)]
    struct Jump;

    #[derive(InputContext)]
    #[context(path = "tests.on_foot", tick = Fixed)]
    struct OnFoot;

    #[derive(Resource, Default)]
    struct Probe {
        value: bool,
        phase: Phase,
    }

    fn probe_jump(input: Actions<OnFoot>, mut probe: bevy_ecs::system::ResMut<'_, Probe>) {
        probe.value = input.value::<Jump>();
        probe.phase = input.phase::<Jump>();
    }

    fn press(key_code: KeyCode, logical_key: Key, state: ButtonState) -> KeyboardInput {
        KeyboardInput {
            key_code,
            logical_key,
            state,
            text: None,
            repeat: false,
            window: bevy_ecs::entity::Entity::PLACEHOLDER,
        }
    }

    // `bevy_time` is not a dependency, so `RunFixedMainLoop` never accumulates enough time to
    // step on its own. Run the fixed schedules directly instead: it is what the loop would do,
    // and it makes the tick count in each test explicit rather than dependent on wall time.
    fn run_fixed_tick(app: &mut App) {
        app.world_mut().run_schedule(bevy_app::FixedPreUpdate);
        // Evaluation happens above; a test that reads state directly registers nothing here.
        let _ = app.world_mut().try_run_schedule(FixedUpdate);
    }

    #[test]
    fn pressing_and_releasing_a_key_updates_the_action_state() {
        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<OnFoot>(|context| {
            context.bind::<Jump>(KeyCode::Space);
        });
        app.world_mut().spawn(OnFoot);
        app.init_resource::<Probe>();
        app.add_systems(FixedUpdate, probe_jump);

        app.world_mut()
            .write_message(press(KeyCode::Space, Key::Space, ButtonState::Pressed));
        app.update();
        run_fixed_tick(&mut app);

        let probe = app.world().resource::<Probe>();
        assert!(probe.value);
        assert_eq!(probe.phase, Phase::Fired);

        app.world_mut()
            .write_message(press(KeyCode::Space, Key::Space, ButtonState::Released));
        app.update();
        run_fixed_tick(&mut app);

        let probe = app.world().resource::<Probe>();
        assert!(!probe.value);
        assert_eq!(probe.phase, Phase::Completed);
    }

    /// The other half of the keyboard-and-mouse scheme, which until now the crate only claimed to
    /// support: a mouse button drives an action exactly as a key does.
    #[cfg(feature = "mouse")]
    #[test]
    fn pressing_and_releasing_a_mouse_button_updates_the_action_state() {
        use bevy_input::mouse::{MouseButton, MouseButtonInput};

        let click = |state| MouseButtonInput {
            button: MouseButton::Left,
            state,
            window: Entity::PLACEHOLDER,
        };

        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<OnFoot>(|context| {
            context.bind::<Jump>(MouseButton::Left);
        });
        app.world_mut().spawn(OnFoot);
        app.init_resource::<Probe>();
        app.add_systems(FixedUpdate, probe_jump);

        app.world_mut().write_message(click(ButtonState::Pressed));
        app.update();
        run_fixed_tick(&mut app);

        let probe = app.world().resource::<Probe>();
        assert!(probe.value);
        assert_eq!(probe.phase, Phase::Fired);

        app.world_mut().write_message(click(ButtonState::Released));
        app.update();
        run_fixed_tick(&mut app);

        let probe = app.world().resource::<Probe>();
        assert!(!probe.value);
        assert_eq!(probe.phase, Phase::Completed);
    }

    /// A mouse button is a button, so it serves as a part of a composite — which is what
    /// `ButtonControl` gaining a variant is for, rather than only `Control`.
    #[cfg(feature = "mouse")]
    #[test]
    fn a_mouse_button_can_be_part_of_a_composite() {
        use bevy_input::mouse::{MouseButton, MouseButtonInput};

        #[derive(InputAction)]
        #[action(path = "tests.lean", output = f32, intent = Analog1)]
        struct Lean;

        #[derive(Resource, Default)]
        struct LeanProbe(f32);

        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<FreeLook>(|context| {
            context.bind::<Lean>(AxisButtons::new(MouseButton::Left, MouseButton::Right));
        });
        app.world_mut().spawn(FreeLook);
        app.init_resource::<LeanProbe>();
        app.add_systems(
            Update,
            |input: Actions<FreeLook>, mut probe: bevy_ecs::system::ResMut<'_, LeanProbe>| {
                probe.0 = input.value::<Lean>();
            },
        );

        let click = |app: &mut App, button, state| {
            app.world_mut().write_message(MouseButtonInput {
                button,
                state,
                window: Entity::PLACEHOLDER,
            });
        };

        click(&mut app, MouseButton::Right, ButtonState::Pressed);
        app.update();
        assert_eq!(app.world().resource::<LeanProbe>().0, 1.0);

        click(&mut app, MouseButton::Left, ButtonState::Pressed);
        app.update();
        assert_eq!(app.world().resource::<LeanProbe>().0, 0.0, "both held");
    }

    #[derive(InputAction)]
    #[action(path = "tests.move", output = Vec2, intent = Directional2)]
    struct Move;

    #[derive(InputAction)]
    #[action(path = "tests.look", output = Vec2, intent = Delta2)]
    struct Look;

    #[cfg(feature = "gamepad")]
    #[derive(InputAction)]
    #[action(path = "tests.turn", output = f32, intent = Analog1)]
    struct Turn;

    #[derive(InputContext)]
    #[context(path = "tests.free_look", tick = Render)]
    struct FreeLook;

    #[derive(Resource, Default)]
    struct MotionProbe {
        movement: Vec2,
        look: Vec2,
    }

    fn probe_motion(
        input: Actions<FreeLook>,
        mut probe: bevy_ecs::system::ResMut<'_, MotionProbe>,
    ) {
        probe.movement = input.value::<Move>();
        probe.look = input.value::<Look>();
    }

    #[cfg(feature = "gamepad")]
    #[derive(Resource, Default)]
    struct GamepadProbe {
        movement: Vec2,
        turn: f32,
        jump: bool,
        jump_phase: Phase,
    }

    #[cfg(feature = "gamepad")]
    fn probe_gamepad(
        input: Actions<OnFoot>,
        mut probe: bevy_ecs::system::ResMut<'_, GamepadProbe>,
    ) {
        probe.movement = input.value::<Move>();
        probe.turn = input.value::<Turn>();
        probe.jump = input.value::<Jump>();
        probe.jump_phase = input.phase::<Jump>();
    }

    #[test]
    fn directional_composites_and_mouse_motion_stay_live_across_frames() {
        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<FreeLook>(|context| {
            context.bind::<Move>(DirectionalButtons::wasd());
            context.bind::<Look>(MouseMove);
        });
        app.world_mut().spawn(FreeLook);
        app.init_resource::<MotionProbe>();
        app.add_systems(Update, probe_motion);

        app.world_mut().write_message(press(
            KeyCode::KeyW,
            Key::Character("w".into()),
            ButtonState::Pressed,
        ));
        app.world_mut().write_message(press(
            KeyCode::KeyD,
            Key::Character("d".into()),
            ButtonState::Pressed,
        ));
        app.world_mut().write_message(MouseMotion {
            delta: Vec2::new(4.0, -1.5),
        });
        app.update();

        let probe = app.world().resource::<MotionProbe>();
        assert_eq!(probe.movement, Vec2::new(1.0, 1.0));
        assert_eq!(probe.look, Vec2::new(4.0, -1.5));

        app.update();

        let probe = app.world().resource::<MotionProbe>();
        assert_eq!(probe.movement, Vec2::new(1.0, 1.0));
        assert_eq!(probe.look, Vec2::ZERO);
    }

    #[cfg(feature = "gamepad")]
    #[derive(InputAction)]
    #[action(path = "tests.thrust", output = f32, intent = Analog1)]
    struct Thrust;

    #[cfg(feature = "gamepad")]
    #[derive(Resource, Clone, Copy, Default)]
    struct TriggerProbe {
        travel: f32,
        pressed: bool,
        phase: Phase,
    }

    /// One trigger, bound twice: once to an analog action and once to a button action. The two
    /// views are independent, and the assertions below only hold if they are — at 0.42 the travel
    /// is live while the press has not yet happened.
    #[cfg(feature = "gamepad")]
    #[test]
    fn a_trigger_serves_an_analog_and_a_button_action_at_once() {
        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<FreeLook>(|context| {
            context.bind::<Thrust>(GamepadButton::LeftTrigger2);
            context.bind::<Jump>(GamepadButton::LeftTrigger2);
        });
        app.world_mut().spawn(FreeLook);
        app.init_resource::<TriggerProbe>();
        app.add_systems(
            Update,
            |input: Actions<FreeLook>, mut probe: bevy_ecs::system::ResMut<'_, TriggerProbe>| {
                probe.travel = input.value::<Thrust>();
                probe.pressed = input.value::<Jump>();
                probe.phase = input.phase::<Jump>();
            },
        );

        let pull_to = |app: &mut App, value: f32| {
            app.world_mut().write_message(RawGamepadEvent::Button(
                RawGamepadButtonChangedEvent::new(
                    bevy_ecs::entity::Entity::PLACEHOLDER,
                    GamepadButton::LeftTrigger2,
                    value,
                ),
            ));
            app.update();
            *app.world().resource::<TriggerProbe>()
        };

        // Short of the press threshold: the travel is real, the button has not fired.
        let probe = pull_to(&mut app, 0.42);
        assert_eq!(probe.travel, 0.42);
        assert!(!probe.pressed);

        // Past it, both views move.
        let probe = pull_to(&mut app, 0.8);
        assert_eq!(probe.travel, 0.8);
        assert!(probe.pressed);
        assert_eq!(probe.phase, Phase::Fired);
    }

    /// The edges of a press, delivered as events rather than polled. `Ongoing` is deliberately not
    /// among them — an observer that fired every tick a key was held would be reporting the absence
    /// of news.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_press_and_a_release_reach_an_observer_as_two_events() {
        use crate::event::{Completed, Fired};
        use bevy_ecs::observer::On;

        #[derive(Resource, Default)]
        struct Heard(Vec<&'static str>);

        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<FreeLook>(|context| {
            context.bind::<Jump>(KeyCode::Space);
        });
        app.init_resource::<Heard>();
        app.add_observer(
            |_: On<Fired<Jump>>, mut heard: bevy_ecs::system::ResMut<'_, Heard>| {
                heard.0.push("fired");
            },
        );
        app.add_observer(
            |_: On<Completed<Jump>>, mut heard: bevy_ecs::system::ResMut<'_, Heard>| {
                heard.0.push("completed");
            },
        );
        app.world_mut().spawn(FreeLook);

        app.update();
        assert!(
            app.world().resource::<Heard>().0.is_empty(),
            "idle is silent"
        );

        app.world_mut()
            .write_message(press(KeyCode::Space, Key::Space, ButtonState::Pressed));
        app.update();
        assert_eq!(app.world().resource::<Heard>().0, ["fired"]);

        // Held, not newly pressed: no further news.
        app.update();
        assert_eq!(app.world().resource::<Heard>().0, ["fired"]);

        app.world_mut()
            .write_message(press(KeyCode::Space, Key::Space, ButtonState::Released));
        app.update();
        assert_eq!(app.world().resource::<Heard>().0, ["fired", "completed"]);
    }

    /// The full handover, both ways. Pausing works by hand and unpausing does not, if the two
    /// directions differ in a way the pause direction happens to tolerate.
    #[cfg(all(feature = "state", feature = "keyboard"))]
    #[test]
    fn a_state_driven_context_hands_control_back_again() {
        use bevy_ecs::observer::On;
        use bevy_state::app::AppExtStates;
        use bevy_state::prelude::{NextState, State, States};

        use crate::event::Fired;

        #[derive(States, Default, Clone, Copy, PartialEq, Eq, Hash, Debug)]
        enum Game {
            #[default]
            Playing,
            Paused,
        }

        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin, bevy_state::app::StatesPlugin));
        app.init_state::<Game>();
        // `OnFoot` is a fixed-tick context and `FreeLook` a render-tick one, which is the pairing
        // a real pause menu has.
        app.add_context::<OnFoot>(|context| {
            context.active_in_state(Game::Playing);
            context.bind::<Jump>(KeyCode::Space);
        });
        app.add_context::<FreeLook>(|context| {
            context.active_in_state(Game::Paused);
            context.bind::<Jump>(KeyCode::Space);
        });
        app.world_mut().spawn(OnFoot);
        app.world_mut().spawn(FreeLook);
        app.add_observer(
            |_: On<Fired<Jump>>,
             game: bevy_ecs::system::Res<'_, State<Game>>,
             mut next: bevy_ecs::system::ResMut<'_, NextState<Game>>| {
                next.set(match game.get() {
                    Game::Playing => Game::Paused,
                    Game::Paused => Game::Playing,
                });
            },
        );

        let tick = |app: &mut App| {
            app.update();
            run_fixed_tick(app);
        };
        let key = |app: &mut App, state: ButtonState| {
            app.world_mut()
                .write_message(press(KeyCode::Space, Key::Space, state));
        };

        // Settle, then tap once to pause.
        tick(&mut app);
        tick(&mut app);
        key(&mut app, ButtonState::Pressed);
        tick(&mut app);
        key(&mut app, ButtonState::Released);
        tick(&mut app);
        tick(&mut app);
        tick(&mut app);
        assert_eq!(*app.world().resource::<State<Game>>().get(), Game::Paused);

        // And again to unpause.
        key(&mut app, ButtonState::Pressed);
        tick(&mut app);
        key(&mut app, ButtonState::Released);
        tick(&mut app);
        tick(&mut app);
        tick(&mut app);
        assert_eq!(
            *app.world().resource::<State<Game>>().get(),
            Game::Playing,
            "the menu never handed control back"
        );
    }

    /// A substate has no `State` resource at all while its parent does not select it. Reading that
    /// resource unconditionally panics the moment anyone reads a nested state, which is a
    /// perfectly ordinary thing to want — pause is often a substate of playing.
    #[cfg(all(feature = "state", feature = "keyboard"))]
    #[test]
    fn a_context_can_follow_a_substate_that_does_not_exist_yet() {
        use bevy_state::app::AppExtStates;
        use bevy_state::prelude::{NextState, States, SubStates};

        #[derive(States, Default, Clone, Copy, PartialEq, Eq, Hash, Debug)]
        enum Session {
            #[default]
            Menu,
            InGame,
        }

        // Two variants, because a substate with one is not a substate anyone would write — and
        // the second is what makes `Running` a value the context can fail to match.
        #[derive(SubStates, Default, Clone, Copy, PartialEq, Eq, Hash, Debug)]
        #[source(Session = Session::InGame)]
        #[expect(
            dead_code,
            reason = "the point is that `Running` is not the only value"
        )]
        enum Play {
            #[default]
            Running,
            Paused,
        }

        fn active(app: &mut App) -> bool {
            app.world_mut()
                .query::<&InputContextState<FreeLook>>()
                .iter(app.world())
                .any(InputContextState::is_active)
        }

        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin, bevy_state::app::StatesPlugin));
        app.init_state::<Session>();
        app.add_sub_state::<Play>();
        app.add_context::<FreeLook>(|context| {
            context.active_in_state(Play::Running);
            context.bind::<Jump>(KeyCode::Space);
        });
        app.world_mut().spawn(FreeLook);

        // In the menu there is no `Play` state to be in, and asking must not be fatal.
        app.update();
        assert!(!active(&mut app));

        app.world_mut()
            .resource_mut::<NextState<Session>>()
            .set(Session::InGame);
        app.update();
        assert!(
            active(&mut app),
            "the substate exists now, and says Running"
        );

        app.world_mut()
            .resource_mut::<NextState<Session>>()
            .set(Session::Menu);
        app.update();
        assert!(!active(&mut app), "and it has gone away again");
    }

    /// A context that follows a state must not be live before the state says so, must come and go
    /// with it, and — the part `OnEnter`/`OnExit` alone would miss — must catch up an instance
    /// spawned while the state is already current.
    #[cfg(all(feature = "state", feature = "keyboard"))]
    #[test]
    fn a_context_follows_the_state_it_was_declared_in() {
        use bevy_state::app::AppExtStates;
        use bevy_state::prelude::{NextState, States};

        #[derive(States, Default, Clone, Copy, PartialEq, Eq, Hash, Debug)]
        enum Screen {
            #[default]
            Menu,
            Playing,
        }

        fn active(app: &mut App) -> bool {
            app.world_mut()
                .query::<&InputContextState<FreeLook>>()
                .iter(app.world())
                .any(InputContextState::is_active)
        }

        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin, bevy_state::app::StatesPlugin));
        app.init_state::<Screen>();
        app.add_context::<FreeLook>(|context| {
            context.active_in_state(Screen::Playing);
            context.bind::<Jump>(KeyCode::Space);
        });
        app.world_mut().spawn(FreeLook);

        app.update();
        assert!(
            !active(&mut app),
            "the state says menu, so the context stands down"
        );

        app.world_mut()
            .resource_mut::<NextState<Screen>>()
            .set(Screen::Playing);
        app.update();
        assert!(active(&mut app), "entering the state brings it up");

        // A second instance arriving after the transition has already happened.
        let latecomer = app.world_mut().spawn(FreeLook).id();
        app.update();
        assert!(
            app.world()
                .get::<InputContextState<FreeLook>>(latecomer)
                .unwrap()
                .is_active(),
            "an instance spawned mid-state is brought up too"
        );

        app.world_mut()
            .resource_mut::<NextState<Screen>>()
            .set(Screen::Menu);
        app.update();
        assert!(!active(&mut app), "leaving the state stands it down again");
    }

    #[derive(Resource)]
    struct AtTheControls;

    /// The general case of the two above: any run condition, not only a state, decides whether a
    /// context is live — including for an instance that arrives after the answer was already yes.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_context_can_follow_an_ordinary_run_condition() {
        use bevy_ecs::schedule::common_conditions::resource_exists;

        fn active(app: &mut App) -> bool {
            app.world_mut()
                .query::<&InputContextState<FreeLook>>()
                .iter(app.world())
                .any(InputContextState::is_active)
        }

        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<FreeLook>(|context| {
            context.active_if(resource_exists::<AtTheControls>);
            context.bind::<Jump>(KeyCode::Space);
        });
        app.world_mut().spawn(FreeLook);

        app.update();
        assert!(
            !active(&mut app),
            "a context with a condition waits to be asked"
        );

        app.world_mut().insert_resource(AtTheControls);
        app.update();
        assert!(active(&mut app), "the condition says yes now");

        let latecomer = app.world_mut().spawn(FreeLook).id();
        app.update();
        assert!(
            app.world()
                .get::<InputContextState<FreeLook>>(latecomer)
                .unwrap()
                .is_active(),
            "an instance spawned while the answer was already yes is brought up too"
        );

        app.world_mut().remove_resource::<AtTheControls>();
        app.update();
        assert!(!active(&mut app), "and stands down again when it says no");
    }

    /// Whatever brings a context up through `active_if`, a control the player was already holding
    /// must not read as a fresh press. Otherwise the button that satisfies the condition is also
    /// the button that acts on what the condition just enabled.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_condition_bringing_a_context_up_ignores_a_control_already_held() {
        use bevy_ecs::schedule::common_conditions::resource_exists;
        use bevy_ecs::schedule::{SystemCondition, common_conditions::not};

        #[derive(Resource)]
        struct Grounded;

        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<OnFoot>(|context| {
            context.active_if(
                resource_exists::<AtTheControls>.and_then(not(resource_exists::<Grounded>)),
            );
            context.bind::<Jump>(KeyCode::Space);
        });
        app.world_mut().spawn(OnFoot);
        app.init_resource::<Probe>();
        app.add_systems(FixedUpdate, probe_jump);

        let tick = |app: &mut App| {
            app.update();
            run_fixed_tick(app);
        };
        let key = |app: &mut App, state: ButtonState| {
            app.world_mut()
                .write_message(press(KeyCode::Space, Key::Space, state));
        };

        // Held down before the context has any interest in it.
        key(&mut app, ButtonState::Pressed);
        tick(&mut app);
        assert_eq!(app.world().resource::<Probe>().phase, Phase::Idle);

        app.world_mut().insert_resource(AtTheControls);
        tick(&mut app);
        assert_eq!(
            app.world().resource::<Probe>().phase,
            Phase::Idle,
            "the key was already down when the condition brought the context up"
        );

        key(&mut app, ButtonState::Released);
        tick(&mut app);
        key(&mut app, ButtonState::Pressed);
        tick(&mut app);
        assert_eq!(
            app.world().resource::<Probe>().phase,
            Phase::Fired,
            "released and pressed again, so it counts"
        );
    }

    /// Two answers to one question, and no rule for which wins. An app-build mistake, so it says so
    /// rather than picking one.
    #[test]
    #[should_panic(expected = "already has a condition")]
    fn a_context_cannot_be_given_two_conditions() {
        use bevy_ecs::schedule::common_conditions::resource_exists;

        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<FreeLook>(|context| {
            context.active_if(resource_exists::<AtTheControls>);
            context.active_if(resource_exists::<Probe>);
        });
    }

    /// A runtime failure rather than a developer mistake: the entity carrying the context is
    /// gone, because whatever it belonged to was destroyed. The system that reads it stands down
    /// for the run instead of bringing the game down with it.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_reader_is_skipped_rather_than_broken_when_its_context_is_gone() {
        let mut app = jump_app();
        let player = app
            .world_mut()
            .query_filtered::<Entity, bevy_ecs::prelude::With<OnFoot>>()
            .single(app.world())
            .expect("one context instance");

        app.world_mut()
            .write_message(press(KeyCode::Space, Key::Space, ButtonState::Pressed));
        app.update();
        run_fixed_tick(&mut app);
        assert_eq!(app.world().resource::<FireCount>().0, 1);

        // The ship dies mid-game. Reading its actions is now a question with no answer.
        app.world_mut().despawn(player);
        app.world_mut()
            .write_message(press(KeyCode::Space, Key::Space, ButtonState::Released));
        app.update();
        run_fixed_tick(&mut app);
        app.world_mut()
            .write_message(press(KeyCode::Space, Key::Space, ButtonState::Pressed));
        app.update();
        run_fixed_tick(&mut app);

        assert_eq!(
            app.world().resource::<FireCount>().0,
            1,
            "the reader was skipped, so it counted nothing further"
        );
    }

    /// The other half of the same rule. Two instances is not one, so a reader that named no entity
    /// has no answer either — and guessing one of them would be worse than standing down.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_reader_is_skipped_when_several_instances_exist() {
        let mut app = jump_app();
        app.world_mut().spawn(OnFoot);

        app.world_mut()
            .write_message(press(KeyCode::Space, Key::Space, ButtonState::Pressed));
        app.update();
        run_fixed_tick(&mut app);

        assert_eq!(app.world().resource::<FireCount>().0, 0);
    }

    /// And what to use instead: the query form runs regardless and reads each instance by entity.
    #[cfg(feature = "keyboard")]
    #[test]
    fn the_query_form_reads_every_instance() {
        #[derive(Resource, Default)]
        struct Jumping(usize, usize);

        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<OnFoot>(|context| {
            context.bind::<Jump>(KeyCode::Space);
        });
        let first = app.world_mut().spawn(OnFoot).id();
        let second = app.world_mut().spawn(OnFoot).id();
        app.init_resource::<Jumping>();
        app.add_systems(
            FixedUpdate,
            move |all: ActionsQuery<OnFoot>, mut count: bevy_ecs::system::ResMut<'_, Jumping>| {
                count.0 = all
                    .iter()
                    .filter(|(_, state)| state.value::<Jump>())
                    .count();
                count.1 = all.len();
                // Reading one by name is the same answer as reading it in the walk.
                assert_eq!(
                    all.get(first).map(InputContextState::value::<Jump>),
                    Some(all.iter().next().unwrap().1.value::<Jump>())
                );
                assert!(all.get(second).is_some());
            },
        );

        app.world_mut()
            .write_message(press(KeyCode::Space, Key::Space, ButtonState::Pressed));
        app.update();
        run_fixed_tick(&mut app);

        let count = app.world().resource::<Jumping>();
        assert_eq!((count.0, count.1), (2, 2));
    }

    /// Another runtime failure rather than a developer mistake: an action read where it was never
    /// bound. It reads as though nobody is touching the control, rather than taking the game down
    /// over a mistake that is the developer's and not the player's.
    #[cfg(feature = "keyboard")]
    #[test]
    fn an_unbound_action_reads_as_untouched() {
        #[derive(InputAction)]
        #[action(path = "tests.unbound", output = f32, intent = Analog1)]
        struct NeverBound;

        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<OnFoot>(|context| {
            context.bind::<Jump>(KeyCode::Space);
        });
        let player = app.world_mut().spawn(OnFoot).id();
        app.update();

        let state = app
            .world()
            .get::<InputContextState<OnFoot>>(player)
            .expect("the context is on the entity");

        assert!(state.is_bound::<Jump>());
        assert!(!state.is_bound::<NeverBound>());

        // Reading it is not fatal, and it reads as rest for its own shape.
        assert_eq!(state.value::<NeverBound>(), 0.0);
        assert_eq!(state.phase::<NeverBound>(), Phase::Idle);

        // And the difference is available to code that wants it.
        assert_eq!(state.try_value::<NeverBound>(), None);
        assert_eq!(state.try_value::<Jump>(), Some(false));

        // The diagnostic still distinguishes the two, which is what it is for.
        assert_eq!(
            state.why_not::<NeverBound>(app.world().resource()),
            Obstacle::Unbound
        );
    }

    /// Counts warnings about a context nobody carries, so the test below can watch for one rather
    /// than assume it.
    ///
    /// A global logger, which `log` allows exactly one of per process — so the two halves of that
    /// test share one, and live in one test rather than racing each other from two.
    mod capture {
        use bevy_platform::sync::atomic::{AtomicUsize, Ordering};

        pub(super) static SEEN: AtomicUsize = AtomicUsize::new(0);

        struct Counting;

        impl log::Log for Counting {
            fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
                metadata.level() <= log::Level::Warn
            }

            fn log(&self, record: &log::Record<'_>) {
                if alloc::format!("{}", record.args()).contains("no entity carries it") {
                    SEEN.fetch_add(1, Ordering::Relaxed);
                }
            }

            fn flush(&self) {}
        }

        static COUNTING: Counting = Counting;

        pub(super) fn install() {
            // Another test may have installed it already; either way it is ours by the time this
            // returns, because nothing else in this crate installs one.
            let _ = log::set_logger(&COUNTING);
            log::set_max_level(log::LevelFilter::Warn);
        }

        pub(super) fn seen() -> usize {
            SEEN.load(Ordering::Relaxed)
        }
    }

    /// A context declared and never spawned is the failure that looks like "that key does nothing":
    /// the bindings compile, the systems run, and no entity is carrying the state they would write.
    ///
    /// Both halves live in one test because they share the process-wide logger above.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_context_nobody_carries_says_so_once_it_is_sure() {
        use bevy_ecs::schedule::common_conditions::resource_exists;

        #[derive(InputContext)]
        #[context(path = "tests.carried", tick = Render)]
        struct Carried;

        #[derive(InputContext)]
        #[context(path = "tests.never_spawned", tick = Render)]
        struct NeverSpawned;

        capture::install();
        let before = capture::seen();

        // An entity carries this one, so there is nothing to say however long it runs.
        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.insert_resource(AtTheControls);
        app.add_context::<Carried>(|context| {
            context.active_if(resource_exists::<AtTheControls>);
            context.bind::<Jump>(KeyCode::Space);
        });
        app.world_mut().spawn(Carried);
        for _ in 0..3 {
            app.update();
        }
        assert_eq!(
            capture::seen(),
            before,
            "a carried context is not a mistake"
        );

        // This one nobody carries.
        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.insert_resource(AtTheControls);
        app.add_context::<NeverSpawned>(|context| {
            context.active_if(resource_exists::<AtTheControls>);
            context.bind::<Jump>(KeyCode::Space);
        });

        // Not on the first run: an entity spawned by an `OnEnter` does not exist yet on the frame
        // its state became current, and warning about that would be crying wolf.
        app.update();
        assert_eq!(capture::seen(), before, "too early to be sure");

        app.update();
        assert_eq!(capture::seen(), before + 1, "and now it is sure");

        // Said once, not once per frame.
        for _ in 0..3 {
            app.update();
        }
        assert_eq!(capture::seen(), before + 1);
    }

    /// The list in that warning is the useful half of it — often a neighbouring action or the
    /// same one in another context — and it reads as a plain sentence.
    #[test]
    fn the_unbound_warning_lists_what_is_bound() {
        use alloc::format;

        assert_eq!(
            format!("{}", BoundPaths(&["tests.turn", "tests.fire"])),
            "tests.turn, tests.fire"
        );
        assert_eq!(format!("{}", BoundPaths(&["tests.turn"])), "tests.turn");
        assert_eq!(
            format!("{}", BoundPaths(&[])),
            "nothing — this context binds no actions at all"
        );
    }

    /// The same rule reached the other way: an action that always claims what it reads says so
    /// once, on itself, rather than on each of its bindings — and a binding can still make an
    /// exception of itself in either direction.
    #[cfg(feature = "keyboard")]
    #[test]
    fn an_action_can_consume_by_declaration_and_a_binding_can_opt_out() {
        #[derive(InputAction)]
        #[action(path = "tests.back", output = bool, intent = Button, consume)]
        struct Back;

        #[derive(InputAction)]
        #[action(path = "tests.crouch", output = bool, intent = Button)]
        struct Crouch;

        #[derive(InputContext)]
        #[context(path = "tests.over", tick = Render, priority = 10)]
        struct Over;

        #[derive(InputContext)]
        #[context(path = "tests.under", tick = Render, priority = 0)]
        struct Under;

        #[derive(Resource, Default)]
        struct Seen {
            under_saw_escape: bool,
            under_saw_backspace: bool,
        }

        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<Over>(|context| {
            // Neither says `consume`; the action already did.
            context.bind::<Back>(KeyCode::Escape);
            context.bind::<Back>(KeyCode::Backspace).without_consuming();
        });
        app.add_context::<Under>(|context| {
            context.bind::<Jump>(KeyCode::Escape);
            context.bind::<Crouch>(KeyCode::Backspace);
        });
        app.world_mut().spawn(Over);
        app.world_mut().spawn(Under);
        app.init_resource::<Seen>();
        app.add_systems(
            Update,
            |under: Actions<Under>, mut seen: bevy_ecs::system::ResMut<'_, Seen>| {
                seen.under_saw_escape = under.value::<Jump>();
                seen.under_saw_backspace = under.value::<Crouch>();
            },
        );

        app.world_mut()
            .write_message(press(KeyCode::Escape, Key::Escape, ButtonState::Pressed));
        app.world_mut().write_message(press(
            KeyCode::Backspace,
            Key::Backspace,
            ButtonState::Pressed,
        ));
        app.update();

        let seen = app.world().resource::<Seen>();
        assert!(
            !seen.under_saw_escape,
            "the action asked to consume, so its binding did"
        );
        assert!(
            seen.under_saw_backspace,
            "and the binding that opted out let this one through"
        );
    }

    /// A menu consumes `Escape` so the game behind it does not also act on it, while a global
    /// screenshot key on `F12` goes on working. Consumption is per binding rather than per context
    /// precisely so those two can differ.
    ///
    /// `Menu` is declared at a higher priority than `Behind`, and both are render-tick here
    /// because that is the only way one schedule can order them.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_consuming_binding_takes_only_the_control_it_named() {
        #[derive(InputAction)]
        #[action(path = "tests.dismiss", output = bool, intent = Button)]
        struct Dismiss;

        #[derive(InputAction)]
        #[action(path = "tests.screenshot", output = bool, intent = Button)]
        struct Screenshot;

        #[derive(InputContext)]
        #[context(path = "tests.menu", tick = Render, priority = 10)]
        struct Menu;

        #[derive(InputContext)]
        #[context(path = "tests.behind", tick = Render, priority = 0)]
        struct Behind;

        #[derive(Resource, Default)]
        struct Seen {
            dismissed: bool,
            behind_saw_escape: bool,
            screenshot: bool,
        }

        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<Menu>(|context| {
            context.bind::<Dismiss>(KeyCode::Escape).consume();
        });
        app.add_context::<Behind>(|context| {
            context.bind::<Jump>(KeyCode::Escape);
            context.bind::<Screenshot>(KeyCode::F12);
        });
        app.world_mut().spawn(Menu);
        app.world_mut().spawn(Behind);
        app.init_resource::<Seen>();
        app.add_systems(
            Update,
            |menu: Actions<Menu>,
             behind: Actions<Behind>,
             mut seen: bevy_ecs::system::ResMut<'_, Seen>| {
                seen.dismissed = menu.value::<Dismiss>();
                seen.behind_saw_escape = behind.value::<Jump>();
                seen.screenshot = behind.value::<Screenshot>();
            },
        );

        app.world_mut()
            .write_message(press(KeyCode::Escape, Key::Escape, ButtonState::Pressed));
        app.world_mut()
            .write_message(press(KeyCode::F12, Key::F12, ButtonState::Pressed));
        app.update();

        let seen = app.world().resource::<Seen>();
        assert!(seen.dismissed, "the menu acted on escape");
        assert!(!seen.behind_saw_escape, "and took it from the game behind");
        assert!(
            seen.screenshot,
            "but f12 was never claimed, so it still works"
        );
    }

    /// An exclusive context shadows every lower-priority one exactly as `deactivate` would —
    /// canceling what was in flight — and releases it exactly as `activate` would, honoring
    /// require-reset so a control held through the whole transition does not fire again on its
    /// own. `Menu` is render-tick and `OnFoot` fixed, which is the direction a settings screen
    /// actually uses.
    #[cfg(feature = "keyboard")]
    #[test]
    fn an_exclusive_context_shadows_and_releases_everything_below_it() {
        #[derive(InputContext)]
        #[context(path = "tests.exclusive_menu", tick = Render, priority = 10, exclusive)]
        struct Menu;

        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<OnFoot>(|context| {
            context.bind::<Jump>(KeyCode::Space);
        });
        app.add_context::<Menu>(|context| {
            context.bind::<Jump>(KeyCode::Escape);
        });
        app.world_mut().spawn(OnFoot);
        app.init_resource::<Probe>();
        app.add_systems(FixedUpdate, probe_jump);

        let tick = |app: &mut App| {
            app.update();
            run_fixed_tick(app);
        };
        let key = |app: &mut App, state: ButtonState| {
            app.world_mut()
                .write_message(press(KeyCode::Space, Key::Space, state));
        };

        key(&mut app, ButtonState::Pressed);
        tick(&mut app);
        assert_eq!(app.world().resource::<Probe>().phase, Phase::Fired);
        tick(&mut app);
        assert_eq!(
            app.world().resource::<Probe>().phase,
            Phase::Ongoing,
            "held, and nothing has shadowed it yet"
        );

        let menu = app.world_mut().spawn(Menu).id();
        tick(&mut app);
        assert_eq!(
            app.world().resource::<Probe>().phase,
            Phase::Canceled,
            "the exclusive context shadows it exactly as deactivate would"
        );

        // Still held, and the exclusive context is still up — no fresh fire hides behind the cancel.
        tick(&mut app);
        assert_ne!(app.world().resource::<Probe>().phase, Phase::Fired);

        app.world_mut().despawn(menu);
        tick(&mut app);
        assert_ne!(
            app.world().resource::<Probe>().phase,
            Phase::Fired,
            "the key never left the control, so require-reset (R7.5) holds it back"
        );

        key(&mut app, ButtonState::Released);
        tick(&mut app);
        key(&mut app, ButtonState::Pressed);
        tick(&mut app);
        assert_eq!(
            app.world().resource::<Probe>().phase,
            Phase::Fired,
            "released and pressed again, now it fires"
        );
    }

    /// The other half of the worked example: a context above the exclusive one's priority is
    /// never touched, which is how a global hotkey survives a modal without an opt-out list —
    /// settled by placement rather than a second mechanism.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_context_above_the_exclusive_ones_priority_is_untouched() {
        #[derive(InputAction)]
        #[action(path = "tests.screenshot", output = bool, intent = Button)]
        struct Screenshot;

        #[derive(InputContext)]
        #[context(path = "tests.exclusive_menu", tick = Render, priority = 10, exclusive)]
        struct Menu;

        #[derive(InputContext)]
        #[context(path = "tests.system", tick = Render, priority = 20)]
        struct System;

        #[derive(Resource, Default)]
        struct Seen(bool);

        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<Menu>(|context| {
            context.bind::<Jump>(KeyCode::Escape);
        });
        app.add_context::<System>(|context| {
            context.bind::<Screenshot>(KeyCode::F12);
        });
        app.world_mut().spawn(Menu);
        app.world_mut().spawn(System);
        app.init_resource::<Seen>();
        app.add_systems(
            Update,
            |system: Actions<System>, mut seen: bevy_ecs::system::ResMut<'_, Seen>| {
                seen.0 = system.value::<Screenshot>();
            },
        );

        app.world_mut()
            .write_message(press(KeyCode::F12, Key::F12, ButtonState::Pressed));
        app.update();

        assert!(
            app.world().resource::<Seen>().0,
            "priority 20 is above the exclusive context's 10, so it was never shadowed"
        );
    }

    /// The tick after the one it fired on, which is where consumption used to let go.
    ///
    /// A menu navigating by direction fires once per direction entered and says nothing on the
    /// ticks between, so a claim that lasted only as long as the fire would hand the key back to
    /// the game underneath for every tick the player kept holding it. What the claim follows is the
    /// binding having something to say, which includes a condition part way through.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_claim_outlasts_the_fire_that_made_it() {
        #[derive(InputAction)]
        #[action(path = "tests.navigate", output = bool, intent = Button)]
        struct Navigate;

        #[derive(InputAction)]
        #[action(path = "tests.walk", output = bool, intent = Button)]
        struct Walk;

        #[derive(InputContext)]
        #[context(path = "tests.screen", tick = Render, priority = 10)]
        struct Screen;

        #[derive(InputContext)]
        #[context(path = "tests.world", tick = Render, priority = 0)]
        struct World;

        #[derive(Resource, Default)]
        struct Seen {
            navigated: bool,
            walked: bool,
        }

        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<Screen>(|context| {
            // One fire when the key goes down, and nothing but `Ongoing` for as long as it is held.
            context
                .bind::<Navigate>(KeyCode::ArrowUp)
                .on_change()
                .consume();
        });
        app.add_context::<World>(|context| {
            context.bind::<Walk>(KeyCode::ArrowUp);
        });
        app.world_mut().spawn(Screen);
        app.world_mut().spawn(World);
        app.init_resource::<Seen>();
        app.add_systems(
            Update,
            |screen: Actions<Screen>,
             world: Actions<World>,
             mut seen: bevy_ecs::system::ResMut<'_, Seen>| {
                seen.navigated = screen.value::<Navigate>();
                seen.walked = world.value::<Walk>();
            },
        );

        app.world_mut()
            .write_message(press(KeyCode::ArrowUp, Key::ArrowUp, ButtonState::Pressed));
        app.update();
        assert!(app.world().resource::<Seen>().navigated, "the change fired");
        assert!(!app.world().resource::<Seen>().walked, "and took the key");

        // The key is still down and nothing has changed, so the screen has nothing to report — but
        // it has not let go either.
        app.update();
        let seen = app.world().resource::<Seen>();
        assert!(!seen.navigated, "no second fire from one press");
        assert!(!seen.walked, "and the game behind still does not see it");
    }

    /// Each obstacle the query can currently reach, provoked one at a time. The point of the type
    /// is that these are five different situations that look identical from the call site, so the
    /// test is worth as much as the feature.
    #[cfg(feature = "keyboard")]
    #[test]
    fn the_diagnostic_names_which_thing_is_in_the_way() {
        use crate::binding::Control;

        #[derive(InputAction)]
        #[action(path = "tests.never_bound", output = bool, intent = Button)]
        struct NeverBound;

        #[derive(InputAction)]
        #[action(path = "tests.charged", output = bool, intent = Button)]
        struct Charged;

        #[derive(InputContext)]
        #[context(path = "tests.taker", tick = Render, priority = 10)]
        struct Taker;

        #[derive(InputContext)]
        #[context(path = "tests.asker", tick = Render, priority = 0)]
        struct Asker;

        #[derive(Resource, Default)]
        struct Report(Option<Obstacle>, Option<Obstacle>, Option<Obstacle>);

        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<Taker>(|context| {
            context.bind::<Jump>(KeyCode::Escape).consume();
        });
        app.add_context::<Asker>(|context| {
            context.bind::<Jump>(KeyCode::Escape);
            context.bind::<Charged>(KeyCode::Space).hold(10.0);
        });
        app.world_mut().spawn(Taker);
        app.world_mut().spawn(Asker);
        app.init_resource::<Report>();
        app.add_systems(
            Update,
            |asker: Actions<Asker>, mut report: bevy_ecs::system::ResMut<'_, Report>| {
                report.0 = Some(asker.why_not::<NeverBound>());
                report.1 = Some(asker.why_not::<Jump>());
                report.2 = Some(asker.why_not::<Charged>());
            },
        );

        // Nothing pressed at all.
        app.update();
        let report = app.world().resource::<Report>();
        assert_eq!(
            report.0,
            Some(Obstacle::Unbound),
            "reading the wrong context"
        );
        assert_eq!(report.1, Some(Obstacle::NoInput), "nobody touched it");

        // Escape taken by the higher-priority context; space held but nowhere near ten seconds.
        app.world_mut()
            .write_message(press(KeyCode::Escape, Key::Escape, ButtonState::Pressed));
        app.world_mut()
            .write_message(press(KeyCode::Space, Key::Space, ButtonState::Pressed));
        app.update();

        let report = app.world().resource::<Report>();
        assert_eq!(
            report.1,
            Some(Obstacle::Consumed {
                control: Control::Key(KeyCode::Escape),
                by: "tests.taker",
            }),
            "and it says who took it"
        );
        assert_eq!(report.2, Some(Obstacle::ConditionPending), "still charging");
    }

    /// The two obstacles that need a context to change state under them.
    #[cfg(feature = "keyboard")]
    #[test]
    fn the_diagnostic_covers_inactive_and_awaiting_release() {
        use crate::eval::ConsumedControls;

        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<FreeLook>(|context| {
            context.bind::<Jump>(KeyCode::Space);
        });
        let entity = app.world_mut().spawn(FreeLook).id();
        app.update();

        let nothing_taken = ConsumedControls::default();
        let mut world = app.world_mut();

        {
            let mut state = world
                .get_mut::<InputContextState<FreeLook>>(entity)
                .unwrap();
            state.deactivate();
            assert_eq!(
                state.why_not::<Jump>(&nothing_taken),
                Obstacle::ContextInactive
            );
            // Coming back while the control is already held is the R7.5 case, and it has its own
            // answer rather than looking like nobody pressed anything.
            state.activate();
            assert_eq!(
                state.why_not::<Jump>(&nothing_taken),
                Obstacle::AwaitingRelease
            );
        }

        let _ = &mut world;
    }

    /// The longest chord wins, and nothing has to be declared for it. Three bindings on one key,
    /// distinguished only by what is held alongside.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_longer_chord_takes_the_control_from_a_shorter_one() {
        #[derive(InputAction)]
        #[action(path = "tests.save", output = bool, intent = Button)]
        struct Save;

        #[derive(InputAction)]
        #[action(path = "tests.save_as", output = bool, intent = Button)]
        struct SaveAs;

        #[derive(InputAction)]
        #[action(path = "tests.type_s", output = bool, intent = Button)]
        struct TypeS;

        #[derive(Resource, Default, Debug, PartialEq)]
        struct Fired {
            typed: bool,
            save: bool,
            save_as: bool,
        }

        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<FreeLook>(|context| {
            context.bind::<TypeS>(KeyCode::KeyS);
            context
                .bind::<Save>(KeyCode::KeyS)
                .with(KeyCode::ControlLeft);
            context
                .bind::<SaveAs>(KeyCode::KeyS)
                .with(KeyCode::ControlLeft)
                .with(KeyCode::ShiftLeft);
        });
        app.world_mut().spawn(FreeLook);
        app.init_resource::<Fired>();
        app.add_systems(
            Update,
            |input: Actions<FreeLook>, mut fired: bevy_ecs::system::ResMut<'_, Fired>| {
                *fired = Fired {
                    typed: input.value::<TypeS>(),
                    save: input.value::<Save>(),
                    save_as: input.value::<SaveAs>(),
                };
            },
        );

        let hold = |app: &mut App, key: KeyCode, logical: Key, state: ButtonState| {
            app.world_mut().write_message(press(key, logical, state));
        };

        // S alone.
        hold(
            &mut app,
            KeyCode::KeyS,
            Key::Character("s".into()),
            ButtonState::Pressed,
        );
        app.update();
        assert_eq!(
            *app.world().resource::<Fired>(),
            Fired {
                typed: true,
                save: false,
                save_as: false
            }
        );

        // Ctrl joins: the two-key chord takes the S from the one-key binding.
        hold(
            &mut app,
            KeyCode::ControlLeft,
            Key::Control,
            ButtonState::Pressed,
        );
        app.update();
        assert_eq!(
            *app.world().resource::<Fired>(),
            Fired {
                typed: false,
                save: true,
                save_as: false
            }
        );

        // Shift joins: the three-key chord takes it from both.
        hold(
            &mut app,
            KeyCode::ShiftLeft,
            Key::Shift,
            ButtonState::Pressed,
        );
        app.update();
        assert_eq!(
            *app.world().resource::<Fired>(),
            Fired {
                typed: false,
                save: false,
                save_as: true
            }
        );

        // And the diagnostic says so, rather than leaving "S is held but nothing happened".
        let mut probe = app.world_mut().query::<&InputContextState<FreeLook>>();
        let state = probe.single(app.world()).unwrap();
        let consumed = crate::eval::ConsumedControls::default();
        assert_eq!(
            state.why_not::<TypeS>(&consumed),
            Obstacle::Outranked {
                control: crate::binding::Control::Key(KeyCode::KeyS),
                chord: 3,
            }
        );
    }

    /// A tap faster than the tick rate, end to end. Polling can only report where the tick ended —
    /// the key back up — so an observer is the only way to learn it happened at all.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_tap_within_one_frame_reaches_observers_as_both_edges() {
        use bevy_ecs::observer::On;

        use crate::event::{Completed, Fired};

        #[derive(Resource, Default)]
        struct Heard(Vec<&'static str>);

        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<FreeLook>(|context| {
            context.bind::<Jump>(KeyCode::Space);
        });
        app.init_resource::<Heard>();
        app.add_observer(
            |_: On<Fired<Jump>>, mut heard: bevy_ecs::system::ResMut<'_, Heard>| {
                heard.0.push("fired");
            },
        );
        app.add_observer(
            |_: On<Completed<Jump>>, mut heard: bevy_ecs::system::ResMut<'_, Heard>| {
                heard.0.push("completed");
            },
        );
        app.world_mut().spawn(FreeLook);

        // Both edges in one frame, which is what a fast tap looks like from here.
        app.world_mut()
            .write_message(press(KeyCode::Space, Key::Space, ButtonState::Pressed));
        app.world_mut()
            .write_message(press(KeyCode::Space, Key::Space, ButtonState::Released));
        app.update();

        assert_eq!(app.world().resource::<Heard>().0, ["fired", "completed"]);
    }

    /// The events target the context entity, which is what makes them usable per player. An
    /// observer attached to one entity must not hear another's input.
    #[cfg(feature = "keyboard")]
    #[test]
    fn an_entity_observer_hears_only_its_own_context() {
        use crate::event::Fired;
        use bevy_ecs::observer::On;

        #[derive(Component, Default)]
        struct Count(usize);

        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<FreeLook>(|context| {
            context.bind::<Jump>(KeyCode::Space);
        });

        let watched = app
            .world_mut()
            .spawn((FreeLook, Count::default()))
            .observe(
                |fired: On<Fired<Jump>>, mut counts: Query<'_, '_, &mut Count>| {
                    if let Ok(mut count) = counts.get_mut(fired.entity) {
                        count.0 += 1;
                    }
                },
            )
            .id();
        let ignored = app.world_mut().spawn((FreeLook, Count::default())).id();

        app.world_mut()
            .write_message(press(KeyCode::Space, Key::Space, ButtonState::Pressed));
        app.update();

        // Both contexts fired — they read the same frame — but only one had an observer.
        assert_eq!(app.world().get::<Count>(watched).unwrap().0, 1);
        assert_eq!(app.world().get::<Count>(ignored).unwrap().0, 0);
    }

    /// Two keys pushing one number in opposite directions. Holding both has to cancel: a turn
    /// control that spun one way because that key was declared first would be a bug the player
    /// could feel.
    #[cfg(feature = "keyboard")]
    #[test]
    fn two_buttons_make_a_signed_axis_that_cancels() {
        #[derive(InputAction)]
        #[action(path = "tests.turn_keys", output = f32, intent = Analog1)]
        struct TurnKeys;

        #[derive(Resource, Default)]
        struct TurnProbe(f32);

        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<FreeLook>(|context| {
            context.bind::<TurnKeys>(AxisButtons::ad());
        });
        app.world_mut().spawn(FreeLook);
        app.init_resource::<TurnProbe>();
        app.add_systems(
            Update,
            |input: Actions<FreeLook>, mut probe: bevy_ecs::system::ResMut<'_, TurnProbe>| {
                probe.0 = input.value::<TurnKeys>();
            },
        );

        let key = |app: &mut App, code: KeyCode, character: &'static str, state: ButtonState| {
            app.world_mut()
                .write_message(press(code, Key::Character(character.into()), state));
        };

        key(&mut app, KeyCode::KeyD, "d", ButtonState::Pressed);
        app.update();
        assert_eq!(app.world().resource::<TurnProbe>().0, 1.0);

        key(&mut app, KeyCode::KeyA, "a", ButtonState::Pressed);
        app.update();
        assert_eq!(app.world().resource::<TurnProbe>().0, 0.0, "both held");

        key(&mut app, KeyCode::KeyD, "d", ButtonState::Released);
        app.update();
        assert_eq!(app.world().resource::<TurnProbe>().0, -1.0);
    }

    /// A stick never rests at exactly zero, so a button action driven by one has to ask the
    /// threshold rather than ask whether the axis is off centre.
    #[cfg(feature = "gamepad")]
    #[test]
    fn an_axis_driving_a_button_action_asks_the_threshold() {
        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<FreeLook>(|context| {
            context.bind::<Jump>(GamepadAxis::LeftStickY);
        });
        app.world_mut().spawn(FreeLook);
        app.init_resource::<TriggerProbe>();
        app.add_systems(
            Update,
            |input: Actions<FreeLook>, mut probe: bevy_ecs::system::ResMut<'_, TriggerProbe>| {
                probe.pressed = input.value::<Jump>();
            },
        );

        let push_to = |app: &mut App, value: f32| {
            app.world_mut()
                .write_message(RawGamepadEvent::Axis(RawGamepadAxisChangedEvent::new(
                    bevy_ecs::entity::Entity::PLACEHOLDER,
                    GamepadAxis::LeftStickY,
                    value,
                )));
            app.update();
            app.world().resource::<TriggerProbe>().pressed
        };

        // A stick at rest sits a little off centre. That is not a press.
        assert!(!push_to(&mut app, 0.03));
        assert!(push_to(&mut app, 0.9));
        // And it is a press however the stick is pushed, since a threshold measures distance.
        assert!(push_to(&mut app, -0.9));
    }

    /// A finger resting near the threshold makes the value wobble. Without a release threshold
    /// below the press one, every wobble would be another `Fired`.
    #[cfg(feature = "gamepad")]
    #[test]
    fn a_trigger_held_near_the_threshold_does_not_chatter() {
        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<FreeLook>(|context| {
            context.bind::<Jump>(GamepadButton::RightTrigger2);
        });
        app.world_mut().spawn(FreeLook);
        app.init_resource::<TriggerProbe>();
        app.add_systems(
            Update,
            |input: Actions<FreeLook>, mut probe: bevy_ecs::system::ResMut<'_, TriggerProbe>| {
                probe.pressed = input.value::<Jump>();
                probe.phase = input.phase::<Jump>();
            },
        );

        let threshold = *app.world().resource::<ButtonThreshold>();
        let midband = (threshold.press + threshold.release) / 2.0;

        let pull_to = |app: &mut App, value: f32| {
            app.world_mut().write_message(RawGamepadEvent::Button(
                RawGamepadButtonChangedEvent::new(
                    bevy_ecs::entity::Entity::PLACEHOLDER,
                    GamepadButton::RightTrigger2,
                    value,
                ),
            ));
            app.update();
            *app.world().resource::<TriggerProbe>()
        };

        assert!(pull_to(&mut app, 0.9).pressed);
        // Backing off into the band holds the press rather than dropping it.
        assert!(pull_to(&mut app, midband).pressed);
        assert_eq!(pull_to(&mut app, midband).phase, Phase::Ongoing);

        // Only past the release threshold does it let go, and re-entering the band keeps it let go.
        assert!(!pull_to(&mut app, 0.1).pressed);
        assert!(!pull_to(&mut app, midband).pressed);
    }

    /// The D-pad has no axis pair anywhere below us, so it becomes a direction the same way WASD
    /// does. Both composites drive one action, and the two are asserted against the same expected
    /// vectors so that a divergence between the keyboard and gamepad paths fails here.
    #[cfg(all(feature = "keyboard", feature = "gamepad"))]
    #[test]
    fn a_dpad_and_four_keys_drive_one_composite_alike() {
        fn movement_after(app: &mut App, drive: impl FnOnce(&mut App)) -> Vec2 {
            drive(app);
            app.update();
            app.world().resource::<MotionProbe>().movement
        }

        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<FreeLook>(|context| {
            context.bind::<Move>(DirectionalButtons::wasd());
            context.bind::<Move>(DirectionalButtons::dpad());
        });
        app.world_mut().spawn(FreeLook);
        app.init_resource::<MotionProbe>();
        app.add_systems(
            Update,
            |input: Actions<FreeLook>, mut probe: bevy_ecs::system::ResMut<'_, MotionProbe>| {
                probe.movement = input.value::<Move>();
            },
        );

        let by_keys = movement_after(&mut app, |app| {
            app.world_mut().write_message(press(
                KeyCode::KeyW,
                Key::Character("w".into()),
                ButtonState::Pressed,
            ));
            app.world_mut().write_message(press(
                KeyCode::KeyA,
                Key::Character("a".into()),
                ButtonState::Pressed,
            ));
        });
        assert_eq!(by_keys, Vec2::new(-1.0, 1.0));

        // Release the keys, then push the same direction on the D-pad.
        let by_dpad = movement_after(&mut app, |app| {
            app.world_mut().write_message(press(
                KeyCode::KeyW,
                Key::Character("w".into()),
                ButtonState::Released,
            ));
            app.world_mut().write_message(press(
                KeyCode::KeyA,
                Key::Character("a".into()),
                ButtonState::Released,
            ));
            for button in [GamepadButton::DPadUp, GamepadButton::DPadLeft] {
                app.world_mut().write_message(RawGamepadEvent::Button(
                    RawGamepadButtonChangedEvent::new(
                        bevy_ecs::entity::Entity::PLACEHOLDER,
                        button,
                        1.0,
                    ),
                ));
            }
        });
        assert_eq!(by_dpad, by_keys);
    }

    /// Stage 1 corrects the hardware; the binding's own deadzone still decides what the mechanic
    /// wants. The two are separate stages, and this is what that buys: a stick resting off centre
    /// stops turning the ship, and a player who wants a smaller deadzone than the drift can still
    /// have one, because the drift was removed underneath rather than clamped over.
    #[cfg(feature = "gamepad")]
    #[test]
    fn calibration_corrects_a_drifting_axis_before_a_binding_reads_it() {
        use crate::device::{AxisCalibration, GamepadCalibration};

        let pad = bevy_ecs::entity::Entity::PLACEHOLDER;
        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<OnFoot>(|context| {
            // No deadzone of its own: this is stage 1 on trial, and a stage-2 deadzone wide enough
            // to swallow the drift would prove nothing about it.
            context.bind::<Turn>(GamepadAxis::RightStickX);
        });
        app.world_mut().spawn(OnFoot);
        app.init_resource::<GamepadProbe>();
        app.add_systems(FixedUpdate, probe_gamepad);

        let push_to =
            |app: &mut App, value: f32| {
                app.world_mut().write_message(RawGamepadEvent::Axis(
                    RawGamepadAxisChangedEvent::new(pad, GamepadAxis::RightStickX, value),
                ));
                app.update();
                run_fixed_tick(app);
                app.world().resource::<GamepadProbe>().turn
            };

        // Uncalibrated, a worn stick resting at 0.1 turns the ship on its own.
        assert!((push_to(&mut app, 0.1) - 0.1).abs() < 1e-6);

        app.world_mut().resource_mut::<GamepadCalibration>().set(
            pad,
            GamepadAxis::RightStickX,
            AxisCalibration {
                center: 0.1,
                rest: 0.03,
            },
        );

        // Calibrated, the same reading is the stick doing nothing.
        assert_eq!(push_to(&mut app, 0.1), 0.0);
        // A real push still arrives, recentred rather than rescaled — 0.5 on the wire is 0.4 of
        // travel away from where this stick actually rests.
        assert!((push_to(&mut app, 0.5) - 0.4).abs() < 1e-6);
        // And the correction is this unit's alone: another pad reporting the same value is not
        // silenced by what this one needed.
        let other = bevy_ecs::entity::Entity::from_bits(0xDEAD);
        app.world_mut()
            .write_message(RawGamepadEvent::Axis(RawGamepadAxisChangedEvent::new(
                other,
                GamepadAxis::RightStickX,
                0.1,
            )));
        app.update();
        run_fixed_tick(&mut app);
        assert!((app.world().resource::<GamepadProbe>().turn - 0.1).abs() < 1e-6);
    }

    /// A player may turn their own deadzone all the way off, and a worn stick still holds still.
    ///
    /// This is the whole reason the two are separate stages. The preference stage adjusts what the
    /// mechanic asked for, and it is free to ask for nothing, because it is not the thing keeping a
    /// drifting stick quiet — calibration already removed the drift underneath it. There is no
    /// clamp anywhere enforcing a floor; the floor is that stage 1 ran first.
    #[cfg(feature = "gamepad")]
    #[test]
    fn a_deadzone_turned_all_the_way_down_still_rests_on_calibration() {
        use crate::device::{AxisCalibration, GamepadCalibration};
        use crate::mapping::{Scheme, TunableValue};
        use crate::overrides::{Overrides, apply_overrides};

        let pad = bevy_ecs::entity::Entity::PLACEHOLDER;
        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<OnFoot>(|context| {
            context
                .bind::<Turn>(GamepadAxis::RightStickX)
                .dead_zone(DeadZone::radial(0.15))
                .tunable_dead_zone("tests.turn.stick_deadzone", 0.0..=0.5);
        });
        app.world_mut().spawn(OnFoot);
        app.init_resource::<GamepadProbe>();
        app.add_systems(FixedUpdate, probe_gamepad);

        app.world_mut().resource_mut::<GamepadCalibration>().set(
            pad,
            GamepadAxis::RightStickX,
            AxisCalibration {
                center: 0.1,
                rest: 0.03,
            },
        );

        let push_to =
            |app: &mut App, value: f32| {
                app.world_mut().write_message(RawGamepadEvent::Axis(
                    RawGamepadAxisChangedEvent::new(pad, GamepadAxis::RightStickX, value),
                ));
                app.update();
                run_fixed_tick(app);
                app.world().resource::<GamepadProbe>().turn
            };

        // The declared 0.15 swallows a small push, which is the mechanic's own choice.
        assert_eq!(push_to(&mut app, 0.2), 0.0);

        let mut overrides = Overrides::default();
        overrides.tune(
            Scheme::Gamepad,
            "tests.turn.stick_deadzone",
            TunableValue::Range {
                value: 0.0,
                min: 0.0,
                max: 0.5,
            },
        );
        assert!(apply_overrides(app.world_mut(), &overrides).is_empty());

        // Applying cancels what was in flight and re-arms require-reset, so the stick has to be
        // seen at rest once before it counts again. It reads as rest at 0.1, which is the point:
        // with the deadzone now at zero, calibration is the only thing that could be saying so.
        assert_eq!(push_to(&mut app, 0.1), 0.0);
        // And that same small push now arrives — the player asked for a stick that answers sooner.
        assert!(push_to(&mut app, 0.2) > 0.0);
    }

    /// An analog action cannot be wedged by an axis that never reads rest.
    ///
    /// Applying an override re-arms require-reset, which holds an action back until it is seen at
    /// rest once. An axis is under no obligation to ever be: a stick drifting at 0.05, with a
    /// player who has taken their own deadzone to zero, reads non-rest forever. Before this was
    /// restricted to button intents the action never recovered — not on a real push, not ever.
    #[cfg(feature = "gamepad")]
    #[test]
    fn an_analog_action_survives_an_axis_that_never_rests() {
        use crate::mapping::{Scheme, TunableValue};
        use crate::overrides::{Overrides, apply_overrides};

        let pad = bevy_ecs::entity::Entity::PLACEHOLDER;
        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<OnFoot>(|context| {
            context
                .bind::<Turn>(GamepadAxis::RightStickX)
                .dead_zone(DeadZone::radial(0.15))
                .tunable_dead_zone("tests.turn.stick_deadzone", 0.0..=0.5);
        });
        app.world_mut().spawn(OnFoot);
        app.init_resource::<GamepadProbe>();
        app.add_systems(FixedUpdate, probe_gamepad);

        let push_to =
            |app: &mut App, value: f32| {
                app.world_mut().write_message(RawGamepadEvent::Axis(
                    RawGamepadAxisChangedEvent::new(pad, GamepadAxis::RightStickX, value),
                ));
                app.update();
                run_fixed_tick(app);
                app.world().resource::<GamepadProbe>().turn
            };

        // A worn stick, drifting, with no calibration measured — so nothing but the binding's own
        // deadzone is holding it still, and that is what the player is about to remove.
        assert_eq!(push_to(&mut app, 0.05), 0.0);

        let mut overrides = Overrides::default();
        overrides.tune(
            Scheme::Gamepad,
            "tests.turn.stick_deadzone",
            TunableValue::Range {
                value: 0.0,
                min: 0.0,
                max: 0.5,
            },
        );
        assert!(apply_overrides(app.world_mut(), &overrides).is_empty());

        // The drift now leaks through, which is exactly what "no deadzone" means and is the
        // player's own choice — measuring the stick is what would answer it, not a clamp here.
        assert!((push_to(&mut app, 0.05) - 0.05).abs() < 1e-6);
        // The point of the test: the action still works. A require-reset that only lifts at exact
        // rest would have wedged it here permanently.
        assert!(
            push_to(&mut app, 0.9) > 0.5,
            "an analog action was held back by require-reset it can never satisfy"
        );
    }

    /// The sampling step, end to end: the crate feeds the sampler while it exists, and what it
    /// measured is what silences the stick afterwards.
    #[cfg(feature = "gamepad")]
    #[test]
    fn a_calibration_step_measures_the_pad_that_reported_during_it() {
        use crate::device::{CalibrationSampling, GamepadCalibration};

        let pad = bevy_ecs::entity::Entity::PLACEHOLDER;
        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<OnFoot>(|context| {
            context.bind::<Turn>(GamepadAxis::RightStickX);
        });
        app.world_mut().spawn(OnFoot);
        app.init_resource::<GamepadProbe>();
        app.add_systems(FixedUpdate, probe_gamepad);

        // The game puts up its "let go of the sticks" screen.
        app.world_mut().init_resource::<CalibrationSampling>();
        for value in [0.09, 0.11, 0.10] {
            app.world_mut()
                .write_message(RawGamepadEvent::Axis(RawGamepadAxisChangedEvent::new(
                    pad,
                    GamepadAxis::RightStickX,
                    value,
                )));
            app.update();
            run_fixed_tick(&mut app);
        }

        let sampling = app.world_mut().remove_resource::<CalibrationSampling>();
        let sampling = sampling.expect("the sampling resource outlives the step");
        assert_eq!(sampling.axes_seen(), 1);
        sampling.finish(&mut app.world_mut().resource_mut::<GamepadCalibration>());

        // What the player was told to hold still now reads as still.
        app.world_mut()
            .write_message(RawGamepadEvent::Axis(RawGamepadAxisChangedEvent::new(
                pad,
                GamepadAxis::RightStickX,
                0.10,
            )));
        app.update();
        run_fixed_tick(&mut app);
        assert_eq!(app.world().resource::<GamepadProbe>().turn, 0.0);
    }

    #[cfg(feature = "gamepad")]
    #[test]
    fn raw_gamepad_events_drive_sticks_and_buttons() {
        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<OnFoot>(|context| {
            context
                .bind::<Move>(Stick::Left)
                .dead_zone(DeadZone::radial(0.2));
            context.bind::<Jump>(GamepadButton::South);
            // A single axis, not the whole stick: an `Analog1` action wants one signed number,
            // and reading the stick as a whole would give it an unsigned magnitude instead.
            context.bind::<Turn>(GamepadAxis::RightStickX);
        });
        app.world_mut().spawn(OnFoot);
        app.init_resource::<GamepadProbe>();
        app.add_systems(FixedUpdate, probe_gamepad);

        app.world_mut()
            .write_message(RawGamepadEvent::Axis(RawGamepadAxisChangedEvent::new(
                bevy_ecs::entity::Entity::PLACEHOLDER,
                GamepadAxis::LeftStickX,
                0.0,
            )));
        app.world_mut()
            .write_message(RawGamepadEvent::Axis(RawGamepadAxisChangedEvent::new(
                bevy_ecs::entity::Entity::PLACEHOLDER,
                GamepadAxis::LeftStickY,
                0.5,
            )));
        app.world_mut()
            .write_message(RawGamepadEvent::Axis(RawGamepadAxisChangedEvent::new(
                bevy_ecs::entity::Entity::PLACEHOLDER,
                GamepadAxis::RightStickX,
                -0.5,
            )));
        app.world_mut()
            .write_message(RawGamepadEvent::Axis(RawGamepadAxisChangedEvent::new(
                bevy_ecs::entity::Entity::PLACEHOLDER,
                GamepadAxis::RightStickY,
                0.25,
            )));
        app.world_mut()
            .write_message(RawGamepadEvent::Button(RawGamepadButtonChangedEvent::new(
                bevy_ecs::entity::Entity::PLACEHOLDER,
                GamepadButton::South,
                1.0,
            )));
        app.update();
        run_fixed_tick(&mut app);

        let probe = app.world().resource::<GamepadProbe>();
        assert_eq!(probe.movement, Vec2::new(0.0, 0.375));
        assert_eq!(probe.turn, -0.5);
        assert!(probe.jump);
        assert_eq!(probe.jump_phase, Phase::Fired);

        app.update();
        run_fixed_tick(&mut app);

        let probe = app.world().resource::<GamepadProbe>();
        assert_eq!(probe.movement, Vec2::new(0.0, 0.375));
        assert_eq!(probe.turn, -0.5);
        assert!(probe.jump);
        assert_eq!(probe.jump_phase, Phase::Ongoing);
    }

    #[test]
    fn fixed_tick_contexts_do_not_evaluate_in_the_render_schedule() {
        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<OnFoot>(|context| {
            context.bind::<Jump>(KeyCode::Space);
        });
        app.world_mut().spawn(OnFoot);
        app.init_resource::<Probe>();
        app.add_systems(FixedUpdate, probe_jump);

        app.world_mut()
            .write_message(press(KeyCode::Space, Key::Space, ButtonState::Pressed));
        app.update();

        // Sampling has happened, but `OnFoot` is a fixed-tick context and no fixed tick has run.
        assert_eq!(app.world().resource::<InputFrame>().events().len(), 1);
        let probe = app.world().resource::<Probe>();
        assert!(!probe.value);
        assert_eq!(probe.phase, Phase::Idle);

        run_fixed_tick(&mut app);

        let probe = app.world().resource::<Probe>();
        assert!(probe.value);
        assert_eq!(probe.phase, Phase::Fired);
    }

    #[cfg(feature = "gamepad")]
    #[test]
    fn either_of_two_bindings_fires_one_action() {
        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<OnFoot>(|context| {
            context.bind::<Jump>(KeyCode::Space);
            context.bind::<Jump>(GamepadButton::South);
        });
        app.world_mut().spawn(OnFoot);
        app.init_resource::<Probe>();
        app.add_systems(FixedUpdate, probe_jump);

        app.world_mut()
            .write_message(press(KeyCode::Space, Key::Space, ButtonState::Pressed));
        app.update();
        run_fixed_tick(&mut app);
        assert!(app.world().resource::<Probe>().value, "keyboard binding");

        app.world_mut()
            .write_message(press(KeyCode::Space, Key::Space, ButtonState::Released));
        app.update();
        run_fixed_tick(&mut app);
        assert!(!app.world().resource::<Probe>().value);

        app.world_mut()
            .write_message(RawGamepadEvent::Button(RawGamepadButtonChangedEvent::new(
                bevy_ecs::entity::Entity::PLACEHOLDER,
                GamepadButton::South,
                1.0,
            )));
        app.update();
        run_fixed_tick(&mut app);
        assert!(app.world().resource::<Probe>().value, "gamepad binding");
    }

    /// R15.3's mixed case: a keyboard-paired instance and a gamepad-paired instance of the same
    /// context, sharing no code path, so a routing bug cannot hide behind symmetry the way it could
    /// between two identical pads.
    #[cfg(feature = "gamepad")]
    #[test]
    fn a_keyboard_paired_and_a_gamepad_paired_instance_are_deaf_to_each_other() {
        use crate::device::DeviceHandle;
        use crate::player::Paired;

        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<OnFoot>(|context| {
            context.bind::<Jump>(KeyCode::Space);
            context.bind::<Jump>(GamepadButton::South);
        });
        let kb_player = app
            .world_mut()
            .spawn((OnFoot, Paired::to(DeviceHandle::KeyboardMouse)))
            .id();
        let pad_player = app
            .world_mut()
            .spawn((
                OnFoot,
                Paired::to(DeviceHandle::Gamepad(bevy_ecs::entity::Entity::from_bits(
                    1,
                ))),
            ))
            .id();

        app.world_mut()
            .write_message(press(KeyCode::Space, Key::Space, ButtonState::Pressed));
        app.update();
        run_fixed_tick(&mut app);
        assert_eq!(
            app.world()
                .get::<InputContextState<OnFoot>>(kb_player)
                .unwrap()
                .phase::<Jump>(),
            Phase::Fired,
            "the keyboard-paired instance saw its own device"
        );
        assert_eq!(
            app.world()
                .get::<InputContextState<OnFoot>>(pad_player)
                .unwrap()
                .phase::<Jump>(),
            Phase::Idle,
            "the gamepad-paired instance never sees the keyboard"
        );

        app.world_mut()
            .write_message(RawGamepadEvent::Button(RawGamepadButtonChangedEvent::new(
                bevy_ecs::entity::Entity::from_bits(1),
                GamepadButton::South,
                1.0,
            )));
        app.update();
        run_fixed_tick(&mut app);
        assert_eq!(
            app.world()
                .get::<InputContextState<OnFoot>>(pad_player)
                .unwrap()
                .phase::<Jump>(),
            Phase::Fired,
            "the gamepad-paired instance saw its own device"
        );
    }

    /// R15.3's identity case: two pads of the same model, where kind alone cannot tell them apart
    /// and only the device handle does. This is the test that fails without routing — every context
    /// reads the whole frame today, so an unpaired build sees both presses as its own.
    #[cfg(feature = "gamepad")]
    #[test]
    fn two_identically_bound_gamepads_do_not_drive_each_others_instance() {
        use crate::device::DeviceHandle;
        use crate::player::Paired;

        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<OnFoot>(|context| {
            context.bind::<Jump>(GamepadButton::South);
        });
        let pad_a = bevy_ecs::entity::Entity::from_bits(1);
        let pad_b = bevy_ecs::entity::Entity::from_bits(2);
        let player_a = app
            .world_mut()
            .spawn((OnFoot, Paired::to(DeviceHandle::Gamepad(pad_a))))
            .id();
        let player_b = app
            .world_mut()
            .spawn((OnFoot, Paired::to(DeviceHandle::Gamepad(pad_b))))
            .id();

        app.world_mut()
            .write_message(RawGamepadEvent::Button(RawGamepadButtonChangedEvent::new(
                pad_a,
                GamepadButton::South,
                1.0,
            )));
        app.update();
        run_fixed_tick(&mut app);

        assert_eq!(
            app.world()
                .get::<InputContextState<OnFoot>>(player_a)
                .unwrap()
                .phase::<Jump>(),
            Phase::Fired,
            "the pad that pressed drives its own paired instance"
        );
        assert_eq!(
            app.world()
                .get::<InputContextState<OnFoot>>(player_b)
                .unwrap()
                .phase::<Jump>(),
            Phase::Idle,
            "a sibling pad's press must not reach an instance paired to a different pad"
        );
    }

    #[cfg(feature = "gamepad")]
    #[test]
    fn a_directional_action_takes_its_strongest_binding() {
        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<FreeLook>(|context| {
            context.bind::<Move>(DirectionalButtons::wasd());
            context.bind::<Move>(Stick::Left);
            context.bind::<Look>(MouseMove);
        });
        app.world_mut().spawn(FreeLook);
        app.init_resource::<MotionProbe>();
        app.add_systems(Update, probe_motion);

        // A half-deflected stick loses to a fully held key...
        app.world_mut().write_message(press(
            KeyCode::KeyW,
            Key::Character("w".into()),
            ButtonState::Pressed,
        ));
        app.world_mut()
            .write_message(RawGamepadEvent::Axis(RawGamepadAxisChangedEvent::new(
                bevy_ecs::entity::Entity::PLACEHOLDER,
                GamepadAxis::LeftStickX,
                0.5,
            )));
        app.update();
        assert_eq!(app.world().resource::<MotionProbe>().movement, Vec2::Y);

        // ...and wins once it is pushed further than the key can reach.
        app.world_mut()
            .write_message(RawGamepadEvent::Axis(RawGamepadAxisChangedEvent::new(
                bevy_ecs::entity::Entity::PLACEHOLDER,
                GamepadAxis::LeftStickX,
                1.0,
            )));
        app.world_mut()
            .write_message(RawGamepadEvent::Axis(RawGamepadAxisChangedEvent::new(
                bevy_ecs::entity::Entity::PLACEHOLDER,
                GamepadAxis::LeftStickY,
                0.5,
            )));
        app.update();
        assert_eq!(
            app.world().resource::<MotionProbe>().movement,
            Vec2::new(1.0, 0.5)
        );
    }

    #[test]
    fn a_delta_action_sums_its_bindings() {
        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<FreeLook>(|context| {
            context.bind::<Move>(DirectionalButtons::wasd());
            context.bind::<Look>(MouseMove);
            context.bind::<Look>(MouseMove).scale(2.0);
        });
        app.world_mut().spawn(FreeLook);
        app.init_resource::<MotionProbe>();
        app.add_systems(Update, probe_motion);

        app.world_mut().write_message(MouseMotion {
            delta: Vec2::new(3.0, -1.0),
        });
        app.update();

        assert_eq!(
            app.world().resource::<MotionProbe>().look,
            Vec2::new(9.0, -3.0)
        );
    }

    #[test]
    fn two_entities_carry_independent_state_for_one_context() {
        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<OnFoot>(|context| {
            context.bind::<Jump>(KeyCode::Space);
        });
        let first = app.world_mut().spawn(OnFoot).id();
        let second = app.world_mut().spawn(OnFoot).id();

        app.world_mut()
            .write_message(press(KeyCode::Space, Key::Space, ButtonState::Pressed));
        app.update();
        run_fixed_tick(&mut app);

        // Both evaluate; each owns its own tables rather than sharing one.
        let world = app.world_mut();
        assert!(
            world
                .get::<InputContextState<OnFoot>>(first)
                .unwrap()
                .value::<Jump>()
        );
        assert!(
            world
                .get::<InputContextState<OnFoot>>(second)
                .unwrap()
                .value::<Jump>()
        );

        // Despawning one leaves the other untouched, which a single shared store could not do.
        world.despawn(second);
        app.update();
        run_fixed_tick(&mut app);

        let world = app.world();
        assert_eq!(
            world
                .get::<InputContextState<OnFoot>>(first)
                .unwrap()
                .phase::<Jump>(),
            Phase::Ongoing
        );
        assert!(world.get::<InputContextState<OnFoot>>(second).is_none());
    }

    #[test]
    #[should_panic(expected = "at most one may rescale")]
    fn stacking_two_rescaling_dead_zones_is_rejected() {
        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<FreeLook>(|context| {
            context
                .bind::<Look>(MouseMove)
                .dead_zone(DeadZone::radial(0.05))
                .dead_zone(DeadZone::radial(0.15));
        });
    }

    #[test]
    fn a_trimming_dead_zone_composes_with_a_rescaling_one() {
        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<FreeLook>(|context| {
            context
                .bind::<Look>(MouseMove)
                .dead_zone(DeadZone::radial(0.05).without_rescale())
                .dead_zone(DeadZone::radial(0.15));
        });
        app.world_mut().spawn(FreeLook);
        app.update();
    }

    #[derive(Resource, Default)]
    struct FireCount(u32);

    fn count_jump_fires(
        input: Actions<OnFoot>,
        mut count: bevy_ecs::system::ResMut<'_, FireCount>,
    ) {
        if input.fired::<Jump>() {
            count.0 += 1;
        }
    }

    fn jump_app() -> App {
        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<OnFoot>(|context| {
            context.bind::<Jump>(KeyCode::Space);
        });
        app.world_mut().spawn(OnFoot);
        app.init_resource::<FireCount>();
        app.add_systems(FixedUpdate, count_jump_fires);
        app
    }

    #[test]
    fn one_press_fires_once_however_many_fixed_ticks_run() {
        let mut app = jump_app();

        app.world_mut()
            .write_message(press(KeyCode::Space, Key::Space, ButtonState::Pressed));
        app.update();
        for _ in 0..3 {
            run_fixed_tick(&mut app);
        }

        // Three ticks over one press. Re-reading the queue each tick would fire three times.
        assert_eq!(app.world().resource::<FireCount>().0, 1);
    }

    #[test]
    fn a_press_survives_a_frame_with_no_fixed_tick() {
        let mut app = jump_app();

        // Two rendered frames go by with the simulation never stepping.
        app.world_mut()
            .write_message(press(KeyCode::Space, Key::Space, ButtonState::Pressed));
        app.update();
        app.update();
        assert_eq!(app.world().resource::<FireCount>().0, 0);

        // The press was queued, not discarded, so the tick that finally runs still sees it.
        run_fixed_tick(&mut app);
        assert_eq!(app.world().resource::<FireCount>().0, 1);
    }

    #[test]
    fn a_delta_is_delivered_once_across_several_fixed_ticks() {
        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<OnFoot>(|context| {
            context.bind::<Look>(MouseMove);
        });
        let context = app.world_mut().spawn(OnFoot).id();

        app.world_mut().write_message(MouseMotion {
            delta: Vec2::new(9.0, 0.0),
        });
        app.update();

        let mut total = Vec2::ZERO;
        for _ in 0..3 {
            run_fixed_tick(&mut app);
            total += app
                .world()
                .get::<InputContextState<OnFoot>>(context)
                .unwrap()
                .value::<Look>();
        }

        // A delta is a displacement, so seeing it in three windows would move the camera three
        // times as far as the mouse actually moved.
        assert_eq!(total, Vec2::new(9.0, 0.0));
    }

    #[test]
    fn a_context_does_not_react_to_input_that_predates_it() {
        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<OnFoot>(|context| {
            context.bind::<Jump>(KeyCode::Space);
        });

        // Space goes down and stays queued while no context exists to read it.
        app.world_mut()
            .write_message(press(KeyCode::Space, Key::Space, ButtonState::Pressed));
        app.update();

        let late = app.world_mut().spawn(OnFoot).id();
        run_fixed_tick(&mut app);

        assert_eq!(
            app.world()
                .get::<InputContextState<OnFoot>>(late)
                .unwrap()
                .phase::<Jump>(),
            Phase::Idle,
            "a context should not fire for input that happened before it existed"
        );
    }

    #[test]
    fn evaluation_precedes_the_systems_that_read_it() {
        // The evaluator writes in `PreUpdate`/`FixedPreUpdate` so that a reader in `Update` or
        // `FixedUpdate` cannot be scheduled ahead of it. Registering the reader first is the
        // arrangement that would expose an ordering ambiguity if both shared one schedule.
        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.init_resource::<MotionProbe>();
        app.add_systems(Update, probe_motion);
        app.add_context::<FreeLook>(|context| {
            context.bind::<Move>(DirectionalButtons::wasd());
            context.bind::<Look>(MouseMove);
        });
        app.world_mut().spawn(FreeLook);

        app.world_mut().write_message(press(
            KeyCode::KeyW,
            Key::Character("w".into()),
            ButtonState::Pressed,
        ));
        app.update();

        assert_eq!(app.world().resource::<MotionProbe>().movement, Vec2::Y);
    }

    #[derive(InputAction)]
    #[action(path = "tests.never_bound_anywhere", output = bool, intent = Button)]
    struct NeverBoundAnywhere;

    #[test]
    fn an_action_the_context_does_not_bind_reads_as_unbound() {
        // Both halves of the slot map's miss: `NeverBoundAnywhere` is interned by the read itself,
        // so its id lands past the end of a map compiled before it existed, while `Jump` is
        // interned by other tests and lands inside the map holding the sentinel.
        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<FreeLook>(|context| {
            context.bind::<Look>(MouseMove);
        });
        let entity = app.world_mut().spawn(FreeLook).id();

        let state = app
            .world()
            .entity(entity)
            .get::<InputContextState<FreeLook>>()
            .unwrap();
        assert!(state.is_bound::<Look>());
        assert!(!state.is_bound::<NeverBoundAnywhere>());
        assert!(!state.is_bound::<Jump>());
    }

    /// What a subscriber sees: how many instances the `Changed` filter offered on the last tick.
    #[derive(Resource, Default)]
    struct Woken(usize);

    fn count_woken(
        woken: Query<'_, '_, (), bevy_ecs::prelude::Changed<InputContextState<OnFoot>>>,
        mut probe: bevy_ecs::system::ResMut<'_, Woken>,
    ) {
        probe.0 = woken.iter().count();
    }

    #[test]
    fn change_detection_follows_the_actions_rather_than_the_tick() {
        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<OnFoot>(|context| {
            context.bind::<Jump>(KeyCode::Space);
        });
        app.world_mut().spawn(OnFoot);
        app.init_resource::<Woken>();
        app.add_systems(FixedUpdate, count_woken);

        // The spawn itself is a change; settle it, then run a tick where nothing happened at all.
        app.update();
        run_fixed_tick(&mut app);
        app.update();
        run_fixed_tick(&mut app);
        assert_eq!(
            app.world().resource::<Woken>().0,
            0,
            "an idle tick woke every subscriber"
        );

        app.world_mut()
            .write_message(press(KeyCode::Space, Key::Space, ButtonState::Pressed));
        app.update();
        run_fixed_tick(&mut app);
        assert_eq!(app.world().resource::<Woken>().0, 1, "a press said nothing");

        // Still held. `Fired` becomes `Ongoing`, which is one more change, and then the action is
        // quiet for as long as the key stays down.
        app.update();
        run_fixed_tick(&mut app);
        app.update();
        run_fixed_tick(&mut app);
        assert_eq!(
            app.world().resource::<Woken>().0,
            0,
            "a key held still was reported as moving"
        );

        app.world_mut()
            .write_message(press(KeyCode::Space, Key::Space, ButtonState::Released));
        app.update();
        run_fixed_tick(&mut app);
        assert_eq!(
            app.world().resource::<Woken>().0,
            1,
            "a release said nothing"
        );
    }

    #[test]
    fn cancelling_on_deactivation_marks_the_action_dirty() {
        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<OnFoot>(|context| {
            context.bind::<Jump>(KeyCode::Space);
        });
        let entity = app.world_mut().spawn(OnFoot).id();

        app.world_mut()
            .write_message(press(KeyCode::Space, Key::Space, ButtonState::Pressed));
        app.update();
        run_fixed_tick(&mut app);

        let mut state = app
            .world_mut()
            .entity_mut(entity)
            .into_mut::<InputContextState<OnFoot>>()
            .unwrap();
        state.dirty.clear();
        state.deactivate();
        assert!(
            state.dirty.contains(0),
            "a hold canceled by deactivation left no trace"
        );
    }

    // OQ-3's two evaluation criteria, as facts about the layout rather than a wall-clock
    // comparison: the numbers a timing run would produce follow from these, and these do not
    // depend on the machine that ran them.

    #[test]
    fn activation_moves_no_entity_between_archetypes() {
        // R23.3. Under action-as-entity each of these actions is a component insert or removal per
        // activation, and every one of them is an archetype move; here activation is a `bool` and
        // a `fill`, so the entity never leaves the archetype it spawned in and no new archetype is
        // created to receive it.
        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<FreeLook>(|context| {
            context.bind::<Move>(DirectionalButtons::wasd());
            context.bind::<Look>(MouseMove);
            context.bind::<Turn>(KeyCode::KeyQ);
        });
        let entity = app.world_mut().spawn(FreeLook).id();
        app.update();

        let archetype = app.world().entity(entity).archetype().id();
        let archetypes = app.world().archetypes().len();
        let entities = app.world().entities().len();

        for _ in 0..8 {
            let mut state = app
                .world_mut()
                .entity_mut(entity)
                .into_mut::<InputContextState<FreeLook>>()
                .unwrap();
            state.deactivate();
            state.activate();
            app.update();
        }

        assert_eq!(app.world().entity(entity).archetype().id(), archetype);
        assert_eq!(app.world().archetypes().len(), archetypes);
        assert_eq!(app.world().entities().len(), entities);
    }

    #[test]
    fn a_snapshot_is_a_fixed_number_of_bytes_of_copy_data() {
        // R10.3/R23.5. Restoring a context is a memcpy of the two tables and the dirty words —
        // there is nothing here to traverse, nothing to reflect over, and nothing to allocate,
        // which is the claim that has to hold for a rollback to afford it once per tick.
        fn assert_copy<T: Copy>() {}
        assert_copy::<ActionState>();
        assert_copy::<Scratch>();

        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<FreeLook>(|context| {
            context.bind::<Move>(DirectionalButtons::wasd());
            context.bind::<Look>(MouseMove);
            context.bind::<Turn>(KeyCode::KeyQ);
        });
        let entity = app.world_mut().spawn(FreeLook).id();
        app.update();

        let state = app
            .world()
            .entity(entity)
            .get::<InputContextState<FreeLook>>()
            .unwrap();
        let bytes = size_of_val(&*state.actions)
            + size_of_val(&*state.scratch)
            + size_of_val(&*state.tunable_scratch)
            + size_of_val(&*state.dirty.words);

        // Three actions and seven bindings' worth of working memory. The bound is generous, and
        // the point of it is the order of magnitude: a rollback window of sixty ticks over four
        // players is kilobytes, not megabytes.
        assert!(
            bytes < 512,
            "a three-action context snapshots {bytes} bytes"
        );
    }
}
