//! One context instance's live state, and the system parameters that read it.

#[cfg(feature = "keyboard")]
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;
use core::marker::PhantomData;
use fixedbitset::FixedBitSet;

use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::Query;
use bevy_ecs::system::SystemParam;
use bevy_platform::sync::Arc;

use crate::action::{
    ActionId, ActionOutput, ActionPhase, ActionState, InputAction, InputContext, Scratch,
};
use crate::condition::BindingCondition;
use crate::eval::Transition;
use crate::frame::FrameTimestamp;
use crate::plan::Plan;
#[cfg(feature = "gamepad")]
use bevy_platform::collections::HashMap;
#[cfg(feature = "mouse")]
use bevy_platform::collections::HashSet;

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

/// The live state of one context on one entity: what every action it binds is currently doing.
///
/// It holds no references into the ECS, so a test or a replay harness can drive one directly
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
    // Parallel to `actions`: which ones changed state since evaluation last cleared this. Per
    // action rather than per context because the component's own change tick cannot distinguish
    // them (R23.4), and because a rollback snapshot is the two tables plus these bits (TD6).
    pub(crate) dirty: FixedBitSet,
    pub(crate) active: bool,
    // Set and cleared only by `shadow`/`unshadow` (R7.8), never by `activate`/`deactivate`:
    // `active` is what this context's own condition or lifecycle wants, `shadowed` is what a
    // higher-priority exclusive context is currently forcing regardless of that. Kept apart so the
    // two do not fight each other — see `is_active`.
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
    // press.
    pub(crate) require_reset: FixedBitSet,
    // Parallel to `actions`: switched off one at a time by the game (R3.7). Evaluation skips these
    // slots on the same terms as it skips an inactive context.
    pub(crate) disabled: FixedBitSet,
    // Every phase change since the last dispatch, in order. Evaluation appends and the dispatcher
    // drains, which is what keeps observers — arbitrary code with `&mut World` — outside the
    // evaluator (R10.2).
    pub(crate) transitions: Vec<Transition>,
    // A class binding's counterpart to `transitions`: logged by evaluation, drained by dispatch,
    // for the same reason.
    pub(crate) class_fires: Vec<crate::eval::ClassFire>,
    // The last event this context has read. Seeded at spawn rather than left empty, so a context
    // added mid-session starts from the present instead of replaying whatever is still queued.
    pub(crate) read_through: Option<FrameTimestamp>,
    #[cfg(feature = "keyboard")]
    pub(crate) held_buttons: BTreeSet<bevy_input::keyboard::KeyCode>,
    // The character each held key reported, for the logical bindings to read. Keyed by position
    // rather than by character so that a release always finds its press: holding a key and then
    // pressing shift changes the character the platform reports, and matching on that would leave
    // the entry stranded. Empty in the overwhelmingly common plan, which binds nothing logically.
    #[cfg(feature = "keyboard")]
    pub(crate) held_characters: BTreeMap<bevy_input::keyboard::KeyCode, char>,
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
    pub(crate) fn new(plan: Arc<Plan<C>>, read_through: Option<FrameTimestamp>) -> Self {
        let slots = plan.slot_count();
        let scratch_slots = plan.scratch_count();
        let tunable_scratch_slots = plan.tunable_scratch_count();
        let actions = alloc::vec![ActionState::default(); slots];

        Self {
            plan,
            actions,
            dirty: FixedBitSet::with_capacity(slots),
            active: true,
            shadowed: false,
            scratch: alloc::vec![Scratch::default(); scratch_slots],
            tunable_scratch: alloc::vec![Scratch::default(); tunable_scratch_slots],
            chord_claims: Vec::new(),
            require_reset: FixedBitSet::with_capacity(slots),
            disabled: FixedBitSet::with_capacity(slots),
            transitions: Vec::new(),
            class_fires: Vec::new(),
            read_through,
            #[cfg(feature = "keyboard")]
            held_buttons: BTreeSet::new(),
            #[cfg(feature = "keyboard")]
            held_characters: BTreeMap::new(),
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
    /// Reading an unbound action is not an error, so this is here for code that would rather ask
    /// than infer it from a rest value. See [`value`](Self::value).
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
    /// An action this context does not bind is always [`Idle`](ActionPhase::Idle), on the same
    /// terms as [`value`](Self::value).
    pub fn phase<A>(&self) -> ActionPhase
    where
        A: InputAction,
    {
        match self.action_state::<A>() {
            Some(state) => state.phase,
            None => {
                self.warn_unbound::<A>();
                ActionPhase::Idle
            }
        }
    }

    /// Returns `true` when the action was pressed this tick.
    pub fn fired<A>(&self) -> bool
    where
        A: InputAction<Output = bool>,
    {
        self.phase::<A>() == ActionPhase::Fired
    }

    /// How long the control behind a `hold` or `hold_and_release` binding has been down, in the
    /// context's own seconds.
    ///
    /// Zero when the action has no such binding, or none of them is currently held. Where more than
    /// one binding qualifies — a keyboard hold and a gamepad hold on the same action — this follows
    /// whichever is furthest along.
    pub fn elapsed<A>(&self) -> f32
    where
        A: InputAction,
    {
        self.holding::<A>().map_or(0.0, |(time, _)| time)
    }

    /// Progress toward firing a `hold` or `hold_and_release` binding, from `0.0` to `1.0`.
    ///
    /// Meant for a charge meter or a hold-to-confirm indicator. Zero on the same terms as
    /// [`elapsed`](Self::elapsed), and clamped at `1.0` rather than continuing to climb once the
    /// control has been held past the duration.
    pub fn progress<A>(&self) -> f32
    where
        A: InputAction,
    {
        self.holding::<A>()
            .map_or(0.0, |(time, duration)| (time / duration).clamp(0.0, 1.0))
    }

    // The elapsed time and duration of whichever `hold`-shaped binding is furthest along.
    //
    // `Scratch::time` already accumulates exactly this while `Hold` or `HoldAndRelease` is
    // building or satisfied, so this is a read path over existing working memory rather than new
    // bookkeeping (R3.4, R3.5).
    fn holding<A>(&self) -> Option<(f32, f32)>
    where
        A: InputAction,
    {
        let slot = self.plan.slot_for_action(A::id())?;
        self.plan
            .bindings()
            .iter()
            .filter(|binding| binding.slot == slot)
            .filter_map(|binding| {
                binding
                    .conditions
                    .iter()
                    .enumerate()
                    .find_map(|(index, condition)| match condition {
                        BindingCondition::Hold { duration, .. }
                        | BindingCondition::HoldAndRelease { duration } => {
                            let scratch_index =
                                binding.scratch_base + binding.modifiers.len() + index;
                            Some((self.scratch[scratch_index].time, *duration))
                        }
                        _ => None,
                    })
            })
            .max_by(|(a, _), (b, _)| a.total_cmp(b))
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
    /// rather than a list.
    pub fn why_not<A>(
        &self,
        consumed: &crate::eval::ConsumedControls,
        pairing: Option<&crate::player::Paired>,
    ) -> ActionObstacle
    where
        A: InputAction,
    {
        self.why_not_id(A::id(), consumed, pairing)
    }

    /// Explains why an action named at run time is not firing.
    ///
    /// As [`why_not`](Self::why_not), for a debug overlay or an editor that walks a context's
    /// actions rather than naming one.
    pub fn why_not_id(
        &self,
        action: ActionId,
        consumed: &crate::eval::ConsumedControls,
        pairing: Option<&crate::player::Paired>,
    ) -> ActionObstacle {
        let Some(slot) = self.plan.slot_for_action(action) else {
            return ActionObstacle::Unbound;
        };
        if !self.is_active() {
            return ActionObstacle::ContextInactive;
        }
        if self.disabled[slot] {
            return ActionObstacle::Disabled;
        }
        match self.actions[slot].phase {
            ActionPhase::Fired | ActionPhase::Firing => return ActionObstacle::None,
            ActionPhase::Started | ActionPhase::Building => {
                return ActionObstacle::ConditionPending;
            }
            _ => {}
        }
        // A control someone else holds is the most useful answer available, so both of these
        // outrank the catch-all below even though all three are "the binding read nothing".
        // No pairing owns every device (R15.3's default), so `reachable` starts true in that case
        // and the loop below only ever narrows it when there is a `Paired` to narrow it against.
        let mut reachable = pairing.is_none();
        for binding in self.plan.bindings().iter().filter(|b| b.slot == slot) {
            let mut taken = None;
            let mut outranked = None;
            binding.input.for_each_control(|control| {
                if pairing.is_some_and(|p| p.owner_for(control.family()).is_some()) {
                    reachable = true;
                }
                if taken.is_none()
                    && let Some(by) = consumed.claimant(control)
                {
                    taken = Some(ActionObstacle::Consumed { control, by });
                }
                if outranked.is_none()
                    && let Some(&(_, chord)) = self
                        .chord_claims
                        .iter()
                        .find(|&&(seen, best)| seen == control && best > binding.chord_len)
                {
                    outranked = Some(ActionObstacle::Outranked { control, chord });
                }
            });
            if let Some(obstacle) = taken.or(outranked) {
                return obstacle;
            }
        }
        // A device that can never satisfy the binding beats "awaiting release": that latch can
        // only clear from an event on an owned device, so reporting it here would describe a wait
        // that never ends.
        if !reachable {
            return ActionObstacle::Unowned;
        }
        if self.require_reset[slot] {
            return ActionObstacle::AwaitingRelease;
        }
        ActionObstacle::NoInput
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
    /// own activation says so, or because a higher-priority exclusive context currently shadows it
    /// — the two look the same from here, and everywhere else that asks.
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
        self.require_reset.set_range(.., require_reset);
    }

    /// Takes a new set of compiled bindings, which is what applying an override does.
    ///
    /// Whatever was in flight is canceled and every action waits to be seen at rest once — the same
    /// work `deactivate` and `activate` do, and for the same reasons: a hold on a control that is
    /// no longer bound has to resolve, and a player still holding the key they just rebound must
    /// not get a fresh press out of the swap.
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
            self.require_reset.set_range(.., true);
        }
    }

    /// Stops driving actions, canceling anything in flight.
    ///
    /// Every action currently held is reported as [`Canceled`](ActionPhase::Canceled) rather than
    /// left where it was, so a hold interrupted by a menu opening resolves instead of staying held
    /// for as long as the menu is up.
    pub fn deactivate(&mut self) {
        if !self.active {
            return;
        }
        self.active = false;
        self.cancel_in_flight();
    }

    /// Switches one action off without unbinding it.
    ///
    /// The action stops reading its controls and stays at rest, while the rest of the context
    /// carries on. Whatever it had in flight is reported as [`Canceled`](ActionPhase::Canceled), as
    /// [`deactivate`](Self::deactivate) does for a whole context. A disabled action also stops
    /// consuming controls and stops out-ranking shorter chords, so a control it would have taken
    /// is free for the other bindings to read.
    pub fn disable<A>(&mut self)
    where
        A: InputAction,
    {
        let Some(slot) = self.plan.slot_for_action(A::id()) else {
            self.warn_unbound::<A>();
            return;
        };
        if self.disabled[slot] {
            return;
        }
        self.disabled.set(slot, true);
        self.cancel_slot(slot);
    }

    /// Switches an action back on, ignoring a control the player is already holding.
    ///
    /// A button held for the whole time the action was off does not fire the moment it comes
    /// back: the player lets go and presses again, the same rule [`activate`](Self::activate)
    /// applies to a whole context. An analog action has no press to hold back, and picks up its
    /// value straight away.
    ///
    /// Enabling an action that is already enabled does nothing, so a system can call this every
    /// tick without holding a button back forever.
    pub fn enable<A>(&mut self)
    where
        A: InputAction,
    {
        let Some(slot) = self.plan.slot_for_action(A::id()) else {
            self.warn_unbound::<A>();
            return;
        };
        if !self.disabled[slot] {
            return;
        }
        self.disabled.set(slot, false);
        self.require_reset.set(slot, true);
    }

    /// Whether an action is switched on. See [`disable`](Self::disable).
    ///
    /// An action this context does not bind reads as enabled, since it was never switched off.
    pub fn is_enabled<A>(&self) -> bool
    where
        A: InputAction,
    {
        self.plan
            .slot_for_action(A::id())
            .is_none_or(|slot| !self.disabled[slot])
    }

    /// Suppresses this context for as long as a higher-priority exclusive context is active.
    ///
    /// Cancels in-flight actions as `deactivate` does: a control held through a modal opening must
    /// not stay held forever any more than one held through an ordinary deactivation would.
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
        self.require_reset.set_range(.., true);
    }

    /// The cancellation `deactivate` and `shadow` share.
    ///
    /// `Started` is included along with `Fired`/`Firing`/`Building`: a hold canceled on the tick it
    /// began is still in flight, and leaving it at `Started` would strand it there until the
    /// context reactivates, which is the "held forever" R7.4 forbids.
    fn cancel_in_flight(&mut self) {
        for slot in 0..self.actions.len() {
            self.cancel_slot(slot);
        }
    }

    fn cancel_slot(&mut self, slot: usize) {
        let state = &mut self.actions[slot];
        if !matches!(
            state.phase,
            ActionPhase::Started | ActionPhase::Building | ActionPhase::Fired | ActionPhase::Firing
        ) {
            return;
        }
        state.phase = ActionPhase::Canceled;
        state.value = rest_like(state.value);
        self.dirty.set(slot, true);
        self.transitions.push(Transition {
            slot,
            phase: ActionPhase::Canceled,
            value: state.value,
        });
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
#[cfg_attr(feature = "bevy_reflect", derive(bevy_reflect::Reflect))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ActionObstacle {
    /// Nothing is in the way: the action is firing.
    None,
    /// This action is not bound in this context.
    ///
    /// Usually a typo, or reading the right action from the wrong context.
    Unbound,
    /// The context is not active, so none of its bindings are being read.
    ContextInactive,
    /// The game has switched this action off. See [`disable`](InputContextState::disable).
    Disabled,
    /// The context has just activated, or this action was just enabled, and its control was
    /// already held.
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
    /// [`with`](crate::binding::BindingBuilder::with).
    Outranked {
        /// The control the longer chord took.
        control: crate::binding::Control,
        /// How many controls that chord requires held, this one included.
        chord: u8,
    },
    /// A condition has begun but has not been satisfied — a hold part way through.
    ConditionPending,
    /// None of this player's paired devices can reach any binding this action has.
    ///
    /// Distinct from [`NoInput`](Self::NoInput): that is an owned device sitting idle, this is no
    /// owned device able to satisfy the binding at all. See
    /// [`Paired`](crate::player::Paired).
    Unowned,
    /// Nothing has touched any control this action is bound to.
    NoInput,
}

/// System parameter for polling the actions of a context with exactly one instance.
///
/// Most games have one of a given context (one on-foot context, one menu context), and this reads
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
pub struct ContextActions<'w, 's, C: InputContext + Component> {
    state: bevy_ecs::system::Single<
        'w,
        's,
        (
            Entity,
            &'static InputContextState<C>,
            Option<&'static crate::player::Paired>,
        ),
    >,
    consumed: bevy_ecs::system::Res<'w, crate::eval::ConsumedControls>,
}

impl<C: InputContext + Component> ContextActions<'_, '_, C> {
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
    pub fn phase<A>(&self) -> ActionPhase
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

    /// How long the control behind a `hold` or `hold_and_release` binding has been down.
    ///
    /// See [`InputContextState::elapsed`].
    pub fn elapsed<A>(&self) -> f32
    where
        A: InputAction,
    {
        self.state().elapsed::<A>()
    }

    /// Progress toward firing a `hold` or `hold_and_release` binding, from `0.0` to `1.0`.
    ///
    /// See [`InputContextState::progress`].
    pub fn progress<A>(&self) -> f32
    where
        A: InputAction,
    {
        self.state().progress::<A>()
    }

    /// Explains why an action is not firing.
    ///
    /// See [`InputContextState::why_not`].
    pub fn why_not<A>(&self) -> ActionObstacle
    where
        A: InputAction,
    {
        self.state().why_not::<A>(&self.consumed, self.state.2)
    }
}

/// System parameter for polling every instance of a context.
///
/// For a context that is per-player, or any other case where there is not exactly one: unlike
/// [`ContextActions`], a system taking this runs whether or not any instance exists, and reads
/// them by entity with [`get`](Self::get) or all at once with [`iter`](Self::iter).
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
    states: Query<
        'w,
        's,
        (
            Entity,
            &'static InputContextState<C>,
            Option<&'static crate::player::Paired>,
        ),
    >,
    consumed: bevy_ecs::system::Res<'w, crate::eval::ConsumedControls>,
}

impl<C: InputContext + Component> ActionsQuery<'_, '_, C> {
    /// Returns the instance carried by an entity, if it has one.
    pub fn get(&self, entity: Entity) -> Option<&InputContextState<C>> {
        self.states.get(entity).ok().map(|(_, state, _)| state)
    }

    /// Iterates every instance of this context and the entity carrying it.
    pub fn iter(&self) -> impl Iterator<Item = (Entity, &InputContextState<C>)> {
        self.states.iter().map(|(entity, state, _)| (entity, state))
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
    /// Returns [`ActionObstacle::Unbound`] when the entity carries no such context, since from
    /// the call site that is indistinguishable from an action nobody bound.
    ///
    /// See [`InputContextState::why_not`].
    pub fn why_not<A>(&self, entity: Entity) -> ActionObstacle
    where
        A: InputAction,
    {
        self.states
            .get(entity)
            .ok()
            .map_or(ActionObstacle::Unbound, |(_, state, pairing)| {
                state.why_not::<A>(&self.consumed, pairing)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use super::super::fixtures::*;
    use crate::action::ActionPhase;

    use crate::ActionMapPlugin;
    use crate::context::ActionMapAppExt;
    use bevy_ecs::prelude::Query;

    use crate::{InputAction, InputContext};
    use bevy_app::{App, FixedUpdate, Update};
    use bevy_ecs::prelude::{Component, Resource};
    use bevy_input::{
        ButtonState, InputPlugin, keyboard::Key, keyboard::KeyCode, mouse::MouseMotion,
    };
    use bevy_math::Vec2;

    #[cfg(feature = "gamepad")]
    use crate::binding::Stick;
    use crate::binding::{AxisButtons, ButtonThreshold, DeadZone, DirectionalButtons, MouseMove};

    #[cfg(feature = "gamepad")]
    use bevy_input::gamepad::{
        GamepadAxis, GamepadButton, RawGamepadAxisChangedEvent, RawGamepadButtonChangedEvent,
        RawGamepadEvent,
    };
    #[cfg(feature = "mouse")]
    use bevy_input::mouse::MouseButton;

    #[test]
    fn pressing_and_releasing_a_key_updates_the_action_state() {
        let mut app = jump_app();
        app.init_resource::<Probe>();
        app.add_systems(FixedUpdate, probe_jump);

        app.world_mut()
            .write_message(press(KeyCode::Space, Key::Space, ButtonState::Pressed));
        app.update();
        run_fixed_tick(&mut app);

        let probe = app.world().resource::<Probe>();
        assert!(probe.value);
        assert_eq!(probe.phase, ActionPhase::Fired);

        app.world_mut()
            .write_message(press(KeyCode::Space, Key::Space, ButtonState::Released));
        app.update();
        run_fixed_tick(&mut app);

        let probe = app.world().resource::<Probe>();
        assert!(!probe.value);
        assert_eq!(probe.phase, ActionPhase::Completed);
    }

    /// The other half of the keyboard-and-mouse family, which until now the crate only claimed to
    /// support: a mouse button drives an action exactly as a key does.
    #[cfg(feature = "mouse")]
    #[test]
    fn pressing_and_releasing_a_mouse_button_updates_the_action_state() {
        let click = |state| mouse_click(MouseButton::Left, state);

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
        assert_eq!(probe.phase, ActionPhase::Fired);

        app.world_mut().write_message(click(ButtonState::Released));
        app.update();
        run_fixed_tick(&mut app);

        let probe = app.world().resource::<Probe>();
        assert!(!probe.value);
        assert_eq!(probe.phase, ActionPhase::Completed);
    }

    /// A mouse button is a button, so it serves as a part of a composite — which is what
    /// `ButtonControl` gaining a variant is for, rather than only `Control`.
    #[cfg(feature = "mouse")]
    #[test]
    fn a_mouse_button_can_be_part_of_a_composite() {
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
            |input: ContextActions<FreeLook>,
             mut probe: bevy_ecs::system::ResMut<'_, LeanProbe>| {
                probe.0 = input.value::<Lean>();
            },
        );

        let click = |app: &mut App, button, state| {
            app.world_mut().write_message(mouse_click(button, state));
        };

        click(&mut app, MouseButton::Right, ButtonState::Pressed);
        app.update();
        assert_eq!(app.world().resource::<LeanProbe>().0, 1.0);

        click(&mut app, MouseButton::Left, ButtonState::Pressed);
        app.update();
        assert_eq!(app.world().resource::<LeanProbe>().0, 0.0, "both held");
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
            |input: ContextActions<FreeLook>,
             mut probe: bevy_ecs::system::ResMut<'_, TriggerProbe>| {
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
        assert_eq!(probe.phase, ActionPhase::Fired);
    }

    /// The edges of a press, delivered as events rather than polled. `Firing` is deliberately not
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

    /// A runtime failure rather than a developer mistake: the entity carrying the context is
    /// gone, because whatever it belonged to was destroyed. The system that reads it stands down
    /// for the run instead of bringing the game down with it.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_reader_is_skipped_rather_than_broken_when_its_context_is_gone() {
        let mut app = jump_fire_count_app();
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
        let mut app = jump_fire_count_app();
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
        assert_eq!(state.phase::<NeverBound>(), ActionPhase::Idle);

        // And the difference is available to code that wants it.
        assert_eq!(state.try_value::<NeverBound>(), None);
        assert_eq!(state.try_value::<Jump>(), Some(false));

        // The diagnostic still distinguishes the two, which is what it is for.
        assert_eq!(
            state.why_not::<NeverBound>(app.world().resource(), None),
            ActionObstacle::Unbound
        );
    }

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
            |under: ContextActions<Under>, mut seen: bevy_ecs::system::ResMut<'_, Seen>| {
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
            |menu: ContextActions<Menu>,
             behind: ContextActions<Behind>,
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
        assert_eq!(app.world().resource::<Probe>().phase, ActionPhase::Fired);
        tick(&mut app);
        assert_eq!(
            app.world().resource::<Probe>().phase,
            ActionPhase::Firing,
            "held, and nothing has shadowed it yet"
        );

        let menu = app.world_mut().spawn(Menu).id();
        tick(&mut app);
        assert_eq!(
            app.world().resource::<Probe>().phase,
            ActionPhase::Canceled,
            "the exclusive context shadows it exactly as deactivate would"
        );

        // Still held, and the exclusive context is still up — no fresh fire hides behind the
        // cancel.
        tick(&mut app);
        assert_ne!(app.world().resource::<Probe>().phase, ActionPhase::Fired);

        app.world_mut().despawn(menu);
        tick(&mut app);
        assert_ne!(
            app.world().resource::<Probe>().phase,
            ActionPhase::Fired,
            "the key never left the control, so require-reset (R7.5) holds it back"
        );

        key(&mut app, ButtonState::Released);
        tick(&mut app);
        key(&mut app, ButtonState::Pressed);
        tick(&mut app);
        assert_eq!(
            app.world().resource::<Probe>().phase,
            ActionPhase::Fired,
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
            |system: ContextActions<System>, mut seen: bevy_ecs::system::ResMut<'_, Seen>| {
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

    /// One action switched off while the rest of its context carries on, and switched back on
    /// under the same require-reset rule activation follows: a key held across the boundary is not
    /// a fresh press (R3.7).
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_disabled_action_rests_and_returns_without_a_fresh_press() {
        #[derive(InputAction)]
        #[action(path = "tests.crouch", output = bool, intent = Button)]
        struct Crouch;

        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<FreeLook>(|context| {
            context.bind::<Jump>(KeyCode::Space);
            context.bind::<Crouch>(KeyCode::KeyC);
        });
        let player = app.world_mut().spawn(FreeLook).id();

        let space = |app: &mut App, state| {
            app.world_mut()
                .write_message(press(KeyCode::Space, Key::Space, state));
        };
        fn input(
            app: &mut App,
            player: Entity,
        ) -> bevy_ecs::world::Mut<'_, InputContextState<FreeLook>> {
            app.world_mut().get_mut(player).unwrap()
        }
        let why_not = |app: &App| {
            app.world()
                .get::<InputContextState<FreeLook>>(player)
                .unwrap()
                .why_not::<Jump>(app.world().resource(), None)
        };

        space(&mut app, ButtonState::Pressed);
        app.world_mut().write_message(press(
            KeyCode::KeyC,
            Key::Character("c".into()),
            ButtonState::Pressed,
        ));
        app.update();
        assert!(input(&mut app, player).value::<Jump>());

        input(&mut app, player).disable::<Jump>();
        assert_eq!(
            input(&mut app, player).phase::<Jump>(),
            ActionPhase::Canceled
        );
        assert!(!input(&mut app, player).is_enabled::<Jump>());

        app.update();
        assert!(
            !input(&mut app, player).value::<Jump>(),
            "still held, and still off"
        );
        assert!(
            input(&mut app, player).value::<Crouch>(),
            "its neighbour is untouched"
        );
        assert_eq!(why_not(&app), ActionObstacle::Disabled);

        input(&mut app, player).enable::<Jump>();
        app.update();
        assert!(
            !input(&mut app, player).value::<Jump>(),
            "held across the enable, so not a press"
        );
        assert_eq!(why_not(&app), ActionObstacle::AwaitingRelease);

        space(&mut app, ButtonState::Released);
        app.update();
        space(&mut app, ButtonState::Pressed);
        app.update();
        assert!(
            input(&mut app, player).fired::<Jump>(),
            "released and pressed again"
        );
    }

    /// A disabled action is out of the running entirely, so what it would have claimed goes to
    /// whoever else reads the control: a context below it, and a shorter chord beside it.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_disabled_action_neither_consumes_nor_out_ranks() {
        #[derive(InputAction)]
        #[action(path = "tests.dismiss", output = bool, intent = Button)]
        struct Dismiss;

        #[derive(InputAction)]
        #[action(path = "tests.save", output = bool, intent = Button)]
        struct Save;

        #[derive(InputAction)]
        #[action(path = "tests.type_s", output = bool, intent = Button)]
        struct TypeS;

        #[derive(InputContext)]
        #[context(path = "tests.disabled_menu", tick = Render, priority = 10)]
        struct Menu;

        #[derive(InputContext)]
        #[context(path = "tests.disabled_behind", tick = Render, priority = 0)]
        struct Behind;

        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<Menu>(|context| {
            context.bind::<Dismiss>(KeyCode::Escape).consume();
            context
                .bind::<Save>(KeyCode::KeyS)
                .with(KeyCode::ControlLeft);
            context.bind::<TypeS>(KeyCode::KeyS);
        });
        app.add_context::<Behind>(|context| {
            context.bind::<Jump>(KeyCode::Escape);
        });
        let menu = app.world_mut().spawn(Menu).id();
        let behind = app.world_mut().spawn(Behind).id();

        {
            let mut state = app
                .world_mut()
                .get_mut::<InputContextState<Menu>>(menu)
                .unwrap();
            state.disable::<Dismiss>();
            state.disable::<Save>();
        }

        app.world_mut()
            .write_message(press(KeyCode::Escape, Key::Escape, ButtonState::Pressed));
        app.world_mut().write_message(press(
            KeyCode::ControlLeft,
            Key::Control,
            ButtonState::Pressed,
        ));
        app.world_mut().write_message(press(
            KeyCode::KeyS,
            Key::Character("s".into()),
            ButtonState::Pressed,
        ));
        app.update();

        let world = app.world();
        assert!(
            world
                .get::<InputContextState<Behind>>(behind)
                .unwrap()
                .value::<Jump>(),
            "the disabled binding did not take escape"
        );
        assert!(
            world
                .get::<InputContextState<Menu>>(menu)
                .unwrap()
                .value::<TypeS>(),
            "the disabled chord did not take s"
        );
    }

    /// A context whose condition keeps saying yes stays quiet the whole time it is shadowed.
    ///
    /// Comparing that answer against `is_active()`, which folds in shadowing, rather than against
    /// `active` marks the instance changed every frame the shadow lasts.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_shadowed_context_whose_condition_stays_satisfied_is_quiet() {
        use bevy_ecs::schedule::common_conditions::resource_exists;

        #[derive(InputContext)]
        #[context(path = "tests.exclusive_menu_over_condition", tick = Render, priority = 10, exclusive)]
        struct Menu;

        #[derive(Resource)]
        struct Live;

        #[derive(Resource, Default)]
        struct Quiet(usize);

        fn count_quiet(
            changed: Query<'_, '_, (), bevy_ecs::prelude::Changed<InputContextState<FreeLook>>>,
            mut quiet: bevy_ecs::system::ResMut<'_, Quiet>,
        ) {
            if changed.is_empty() {
                quiet.0 += 1;
            }
        }

        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<FreeLook>(|context| {
            context.active_if(resource_exists::<Live>);
            context.bind::<Turn>(KeyCode::KeyD);
        });
        app.add_context::<Menu>(|context| {
            context.bind::<Jump>(KeyCode::Escape);
        });
        app.world_mut().spawn(FreeLook);
        app.world_mut().insert_resource(Live);
        app.init_resource::<Quiet>();
        app.add_systems(Update, count_quiet);

        // Settle the activation itself before the exclusive context arrives.
        app.update();

        app.world_mut().spawn(Menu);
        app.update();

        // Both settling updates above are real changes — the first activation, then the shadow
        // taking hold — so only the steady state that follows is what this test is about.
        app.world_mut().resource_mut::<Quiet>().0 = 0;
        for _ in 0..5 {
            app.update();
        }

        assert_eq!(
            app.world().resource::<Quiet>().0,
            5,
            "the condition never changed its answer, so a shadowed instance should not either"
        );
    }

    /// The other direction: while shadowed, `is_active()` already reads `false`, so comparing
    /// against it makes a condition going false compare equal and skip the `deactivate`, stranding
    /// `active` at `true` until the shadow lifts and finally notices the mismatch.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_condition_going_false_while_shadowed_deactivates_on_the_same_frame() {
        use bevy_ecs::schedule::common_conditions::resource_exists;

        #[derive(InputContext)]
        #[context(path = "tests.exclusive_menu_over_condition_2", tick = Render, priority = 10, exclusive)]
        struct Menu;

        #[derive(Resource)]
        struct Live;

        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<FreeLook>(|context| {
            context.active_if(resource_exists::<Live>);
            context.bind::<Turn>(KeyCode::KeyD);
        });
        app.add_context::<Menu>(|context| {
            context.bind::<Jump>(KeyCode::Escape);
        });
        let entity = app.world_mut().spawn(FreeLook).id();
        app.world_mut().insert_resource(Live);
        app.update();

        app.world_mut().spawn(Menu);
        app.update();
        assert!(
            app.world()
                .get::<InputContextState<FreeLook>>(entity)
                .unwrap()
                .shadowed,
            "the exclusive context is up, so this instance should be shadowed"
        );

        app.world_mut().remove_resource::<Live>();
        app.update();
        assert!(
            !app.world()
                .get::<InputContextState<FreeLook>>(entity)
                .unwrap()
                .active,
            "the condition said no on this very frame, and `active` should have moved with it \
             instead of waiting for the shadow to lift"
        );
    }

    /// A claim lasts as long as the binding has something to say, not only the tick it fired on.
    ///
    /// A menu navigating by direction fires once per direction entered and says nothing on the
    /// ticks between, so a claim that lasted only as long as the fire would hand the key back to
    /// the game underneath for every tick the player kept holding it. A condition part way through
    /// counts as something to say.
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
            // One fire when the key goes down, and nothing but `Firing` for as long as it is held.
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
            |screen: ContextActions<Screen>,
             world: ContextActions<World>,
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

    /// Each obstacle the query can currently reach, provoked one at a time.
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
        struct Report(
            Option<ActionObstacle>,
            Option<ActionObstacle>,
            Option<ActionObstacle>,
        );

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
            |asker: ContextActions<Asker>, mut report: bevy_ecs::system::ResMut<'_, Report>| {
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
            Some(ActionObstacle::Unbound),
            "reading the wrong context"
        );
        assert_eq!(report.1, Some(ActionObstacle::NoInput), "nobody touched it");

        // Escape taken by the higher-priority context; space held but nowhere near ten seconds.
        app.world_mut()
            .write_message(press(KeyCode::Escape, Key::Escape, ButtonState::Pressed));
        app.world_mut()
            .write_message(press(KeyCode::Space, Key::Space, ButtonState::Pressed));
        app.update();

        let report = app.world().resource::<Report>();
        assert_eq!(
            report.1,
            Some(ActionObstacle::Consumed {
                control: Control::PhysicalKey(KeyCode::Escape),
                by: "tests.taker",
            }),
            "and it says who took it"
        );
        assert_eq!(
            report.2,
            Some(ActionObstacle::ConditionPending),
            "still charging"
        );
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
                state.why_not::<Jump>(&nothing_taken, None),
                ActionObstacle::ContextInactive
            );
            // Coming back while the control is already held is the R7.5 case, and it has its own
            // answer rather than looking like nobody pressed anything.
            state.activate();
            assert_eq!(
                state.why_not::<Jump>(&nothing_taken, None),
                ActionObstacle::AwaitingRelease
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
            |input: ContextActions<FreeLook>, mut fired: bevy_ecs::system::ResMut<'_, Fired>| {
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
            state.why_not::<TypeS>(&consumed, None),
            ActionObstacle::Outranked {
                control: crate::binding::Control::PhysicalKey(KeyCode::KeyS),
                chord: 3,
            }
        );
    }

    /// A modifier entry is satisfied by either key of its pair, and a sided entry by only its own.
    /// The pair is the whole point: a player reaching for the right-hand Control still saves.
    #[cfg(feature = "keyboard")]
    #[test]
    fn either_key_of_a_modifier_satisfies_a_chord() {
        #[derive(InputAction)]
        #[action(path = "tests.save", output = bool, intent = Button)]
        struct Save;

        #[derive(InputAction)]
        #[action(path = "tests.lean", output = bool, intent = Button)]
        struct Lean;

        #[derive(Resource, Default, Debug, PartialEq)]
        struct Fired {
            save: bool,
            lean: bool,
        }

        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<FreeLook>(|context| {
            context
                .bind::<Save>(KeyCode::KeyS)
                .with(crate::binding::ModifierKey::Ctrl);
            // The contrast: one named side, satisfied by that key and no other.
            context
                .bind::<Lean>(KeyCode::KeyQ)
                .with(KeyCode::ControlLeft);
        });
        app.world_mut().spawn(FreeLook);
        app.init_resource::<Fired>();
        app.add_systems(
            Update,
            |input: ContextActions<FreeLook>, mut fired: bevy_ecs::system::ResMut<'_, Fired>| {
                *fired = Fired {
                    save: input.value::<Save>(),
                    lean: input.value::<Lean>(),
                };
            },
        );

        let hold = |app: &mut App, key, logical: Key, state| {
            app.world_mut().write_message(press(key, logical, state));
            app.update();
        };

        // Both letters down, no modifier: neither chord is satisfied.
        hold(
            &mut app,
            KeyCode::KeyS,
            Key::Character("s".into()),
            ButtonState::Pressed,
        );
        hold(
            &mut app,
            KeyCode::KeyQ,
            Key::Character("q".into()),
            ButtonState::Pressed,
        );
        assert_eq!(
            *app.world().resource::<Fired>(),
            Fired {
                save: false,
                lean: false
            }
        );

        // The left Control satisfies both the modifier and the sided entry.
        hold(
            &mut app,
            KeyCode::ControlLeft,
            Key::Control,
            ButtonState::Pressed,
        );
        assert_eq!(
            *app.world().resource::<Fired>(),
            Fired {
                save: true,
                lean: true
            }
        );

        // Swap sides. The modifier is still held; the binding that named the left key is not.
        hold(
            &mut app,
            KeyCode::ControlLeft,
            Key::Control,
            ButtonState::Released,
        );
        hold(
            &mut app,
            KeyCode::ControlRight,
            Key::Control,
            ButtonState::Pressed,
        );
        assert_eq!(
            *app.world().resource::<Fired>(),
            Fired {
                save: true,
                lean: false
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
            |input: ContextActions<FreeLook>,
             mut probe: bevy_ecs::system::ResMut<'_, TurnProbe>| {
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
            |input: ContextActions<FreeLook>,
             mut probe: bevy_ecs::system::ResMut<'_, TriggerProbe>| {
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
            |input: ContextActions<FreeLook>,
             mut probe: bevy_ecs::system::ResMut<'_, TriggerProbe>| {
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
        assert_eq!(pull_to(&mut app, midband).phase, ActionPhase::Firing);

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
            |input: ContextActions<FreeLook>,
             mut probe: bevy_ecs::system::ResMut<'_, MotionProbe>| {
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
    /// No clamp anywhere enforces a floor; the floor is that stage 1 ran first.
    #[cfg(feature = "gamepad")]
    #[test]
    fn a_deadzone_turned_all_the_way_down_still_rests_on_calibration() {
        use crate::device::{AxisCalibration, DeviceFamily, GamepadCalibration};
        use crate::mapping::TunableValue;
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
            DeviceFamily::Gamepad,
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
    /// rest once, and an axis is under no obligation to ever be: a stick drifting at 0.05, with a
    /// player who has taken their own deadzone to zero, reads non-rest forever. Restricting the
    /// latch to button intents is what keeps such an action from being held back for good.
    #[cfg(feature = "gamepad")]
    #[test]
    fn an_analog_action_survives_an_axis_that_never_rests() {
        use crate::device::DeviceFamily;
        use crate::mapping::TunableValue;
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
            DeviceFamily::Gamepad,
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
        assert_eq!(probe.jump_phase, ActionPhase::Fired);

        app.update();
        run_fixed_tick(&mut app);

        let probe = app.world().resource::<GamepadProbe>();
        assert_eq!(probe.movement, Vec2::new(0.0, 0.375));
        assert_eq!(probe.turn, -0.5);
        assert!(probe.jump);
        assert_eq!(probe.jump_phase, ActionPhase::Firing);
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

    // The mixed case (R15.3): a keyboard-paired instance and a gamepad-paired instance of the same
    // context, sharing no code path, so a routing bug cannot hide behind symmetry the way it could
    // between two identical pads.

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
            ActionPhase::Fired,
            "the keyboard-paired instance saw its own device"
        );
        assert_eq!(
            app.world()
                .get::<InputContextState<OnFoot>>(pad_player)
                .unwrap()
                .phase::<Jump>(),
            ActionPhase::Idle,
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
            ActionPhase::Fired,
            "the gamepad-paired instance saw its own device"
        );
    }

    // The identity case (R15.3): two pads of the same model, where kind alone cannot tell them
    // apart and only the device handle does. Every context reads the whole frame, so without
    // routing each instance sees both presses as its own.

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
            ActionPhase::Fired,
            "the pad that pressed drives its own paired instance"
        );
        assert_eq!(
            app.world()
                .get::<InputContextState<OnFoot>>(player_b)
                .unwrap()
                .phase::<Jump>(),
            ActionPhase::Idle,
            "a sibling pad's press must not reach an instance paired to a different pad"
        );
    }

    // Without a `Paired` to check, a control the player pressed on a device this instance is not
    // paired to comes back from `why_not` as `NoInput` — "nobody touched it" — rather than naming
    // the pairing as the reason it never arrived.

    #[cfg(all(feature = "gamepad", feature = "keyboard"))]
    #[test]
    fn why_not_blames_the_pairing_rather_than_no_input() {
        use crate::device::DeviceHandle;
        use crate::eval::ConsumedControls;
        use crate::player::Paired;

        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<OnFoot>(|context| {
            context.bind::<Jump>(KeyCode::Space);
        });
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

        let world = app.world();
        let state = world.get::<InputContextState<OnFoot>>(pad_player).unwrap();
        let pairing = world.get::<Paired>(pad_player);
        let consumed = ConsumedControls::default();
        assert_eq!(
            state.why_not::<Jump>(&consumed, pairing),
            ActionObstacle::Unowned,
            "the keyboard press happened, it just was never this instance's to see"
        );
    }

    #[cfg(feature = "gamepad")]
    fn stick(axis: GamepadAxis, value: f32) -> RawGamepadEvent {
        RawGamepadEvent::Axis(RawGamepadAxisChangedEvent::new(
            bevy_ecs::entity::Entity::PLACEHOLDER,
            axis,
            value,
        ))
    }

    /// Holding W while the stick also pushes forward is one forward, not more; the stick's sideways
    /// push lands on the other axis untouched, so the diagonal survives.
    #[cfg(feature = "gamepad")]
    #[test]
    fn same_direction_contributions_do_not_add() {
        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<FreeLook>(|context| {
            context.bind::<Move>(DirectionalButtons::wasd());
            context.bind::<Move>(Stick::Left);
        });
        app.world_mut().spawn(FreeLook);
        app.init_resource::<MotionProbe>();
        app.add_systems(Update, probe_motion);

        app.world_mut().write_message(press(
            KeyCode::KeyW,
            Key::Character("w".into()),
            ButtonState::Pressed,
        ));
        app.world_mut()
            .write_message(stick(GamepadAxis::LeftStickY, 0.5));
        app.update();
        assert_eq!(app.world().resource::<MotionProbe>().movement, Vec2::Y);

        app.world_mut()
            .write_message(stick(GamepadAxis::LeftStickX, 0.5));
        app.update();
        assert_eq!(
            app.world().resource::<MotionProbe>().movement,
            Vec2::new(0.5, 1.0)
        );
    }

    #[cfg(feature = "gamepad")]
    #[test]
    fn opposite_contributions_cancel() {
        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<FreeLook>(|context| {
            context.bind::<Move>(DirectionalButtons::wasd());
            context.bind::<Move>(Stick::Left);
        });
        app.world_mut().spawn(FreeLook);
        app.init_resource::<MotionProbe>();
        app.add_systems(Update, probe_motion);

        // A pulled fully back against W held forward.
        app.world_mut().write_message(press(
            KeyCode::KeyW,
            Key::Character("w".into()),
            ButtonState::Pressed,
        ));
        app.world_mut()
            .write_message(stick(GamepadAxis::LeftStickY, -1.0));
        app.update();
        assert_eq!(app.world().resource::<MotionProbe>().movement, Vec2::ZERO);

        // Part way back leaves the key the rest of its reach.
        app.world_mut()
            .write_message(stick(GamepadAxis::LeftStickY, -0.25));
        app.update();
        assert_eq!(
            app.world().resource::<MotionProbe>().movement,
            Vec2::new(0.0, 0.75)
        );
    }

    #[cfg(feature = "gamepad")]
    #[test]
    fn the_fold_does_not_depend_on_declaration_order() {
        let mut values = Vec::new();
        for keys_first in [true, false] {
            let mut app = App::new();
            app.add_plugins((InputPlugin, ActionMapPlugin));
            app.add_context::<FreeLook>(move |context| {
                if keys_first {
                    context.bind::<Move>(DirectionalButtons::wasd());
                    context.bind::<Move>(Stick::Left);
                } else {
                    context.bind::<Move>(Stick::Left);
                    context.bind::<Move>(DirectionalButtons::wasd());
                }
            });
            let entity = app.world_mut().spawn(FreeLook).id();

            app.world_mut().write_message(press(
                KeyCode::KeyD,
                Key::Character("d".into()),
                ButtonState::Pressed,
            ));
            // As far as the key reaches, so no contribution is stronger than the other.
            app.world_mut()
                .write_message(stick(GamepadAxis::LeftStickY, 1.0));
            app.update();

            let state = app
                .world()
                .get::<InputContextState<FreeLook>>(entity)
                .unwrap();
            let reading = state
                .iter()
                .find(|reading| reading.path == Move::PATH)
                .unwrap();
            values.push(reading.state.value);
        }

        assert_eq!(values[0], crate::action::ActionValue::Axis2(Vec2::ONE));
        assert_eq!(values[0], values[1]);
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
            ActionPhase::Firing
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

    #[test]
    fn one_press_fires_once_however_many_fixed_ticks_run() {
        let mut app = jump_fire_count_app();

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
        let mut app = jump_fire_count_app();

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

    #[test]
    fn change_detection_follows_the_actions_rather_than_the_tick() {
        let mut app = jump_app();
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

        // Still held. `Fired` becomes `Firing`, which is one more change, and then the action is
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

    // docs/decisions.md D8's two evaluation criteria, as facts about the layout rather than a
    // wall-clock comparison: the numbers a timing run would produce follow from these, and these
    // do not depend on the machine that ran them.

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
            + size_of_val(state.dirty.as_slice());

        // Three actions and six bindings, four of them WASD, and none with a modifier, a condition
        // or a press to remember. The bound is generous, and the point of it is the order of
        // magnitude: a rollback window of sixty ticks over four players is kilobytes, not
        // megabytes.
        assert!(
            bytes < 512,
            "a three-action context snapshots {bytes} bytes"
        );
    }
}
