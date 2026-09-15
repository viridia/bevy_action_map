//! Declaring a context: the app wiring, and the records declaration writes.

use bevy_app::{App, FixedPreUpdate, PreUpdate};
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::lifecycle::HookContext;
use bevy_ecs::prelude::{Query, Resource};
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::world::{DeferredWorld, World};
use bevy_platform::sync::Arc;

use crate::action::{InputContext, TickDomain};
use crate::binding::InputContextBuilder;
use crate::eval::{dispatch_class_fires, dispatch_transitions, evaluate_context};
use crate::frame::InputFrame;
use crate::inspect::OverrideStage;
use crate::plan::Plan;
use crate::{ActionMapPlugin, ActionMapSystems};

use super::state::InputContextState;

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
    mappings: alloc::vec::Vec<crate::mapping::ActionMapping>,
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
    pub(crate) mappings: alloc::vec::Vec<crate::mapping::ActionMapping>,
    pub(crate) tunables: alloc::vec::Vec<crate::mapping::Tunable>,
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

/// Takes the state off again when the component goes, so a context is live exactly while its
/// entity carries it.
///
/// Without this the state evaluates on past its own declaration, and because an app dropping a
/// context usually drops `Paired` in the same breath, it does so reading *every* device instead of
/// the one it had — a character that walks on someone else's stick.
///
/// `try_remove`, because the component also goes when the entity is despawned, and then there is
/// nothing left to take it off.
fn detach_context_state<C: InputContext + Component>(
    mut world: DeferredWorld<'_>,
    context: HookContext,
) {
    world
        .commands()
        .entity(context.entity)
        .try_remove::<InputContextState<C>>();
}

/// Warns when a `#[derive(InputContext)]` component is spawned before `add_context` declared it.
///
/// `attach_context_state` cannot catch this by itself: `add_context` is the only thing that
/// installs it as `C`'s `on_add` hook, so a type nobody declared has no `on_add` hook at all, and
/// neither does anything log the miss — the symptom is just a control that does nothing, on a
/// context `dump` cannot see either, since [`DeclaredContexts`](crate::inspect::DeclaredContexts)
/// is its only source. This runs from `Component::on_insert` instead, which the derive can set at
/// compile time regardless of whether `add_context` ever runs, and is a no-op once it has.
#[doc(hidden)]
pub fn warn_if_undeclared<C: InputContext + Component>(
    world: DeferredWorld<'_>,
    context: HookContext,
) {
    if world.get_resource::<InputContextPlan<C>>().is_some() {
        return;
    }
    bevy_utils::once!(log::warn!(
        "entity {} carries context `{}` ({}), but add_context was never called for it — none of \
         its bindings can fire, and `dump` cannot see this instance either. Call add_context \
         before spawning it.",
        context.entity,
        C::PATH,
        core::any::type_name::<C>(),
    ));
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
        // Against `active`, not `is_active()`: `activate`/`deactivate` only ever move `active`, and
        // under an exclusive context `is_active()` is pinned to `false` by `shadowed` regardless of
        // what this condition says, which would either call `activate` every frame for nothing or
        // skip the `deactivate` a shadowed context still needs to catch up on once the shadow
        // lifts.
        //
        // `activate` and `deactivate` both return immediately when there is nothing to do; the
        // check here is what keeps the mutable deref, and with it the change tick, off the frames
        // where nothing happened.
        if context.active == live {
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
    /// Leave both off for a context you drive yourself, per instance, with
    /// [`activate`](InputContextState::activate) and
    /// [`deactivate`](InputContextState::deactivate) — a condition answers for every instance at
    /// once, so the two do not mix.
    ///
    /// # Following a game state
    ///
    /// Use [`active_in_state`](Self::active_in_state) instead. `in_state` works here and the
    /// context does follow the state, but a frame later than it needs to: Bevy applies state
    /// transitions *after* `PreUpdate`, so a condition polled here reads the state as it was
    /// before this frame's transition — a gameplay context on the fixed tick keeps driving the
    /// simulation for one more tick after a pause, and a system running in `OnEnter` finds the
    /// context it is about to set up still in its previous state.
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

/// Where the *n*th context declared at a priority evaluates, relative to its same-priority
/// siblings. Nested inside that priority's `EvaluateAt`, so ordering it against another priority
/// stays `EvaluateAt`'s job alone.
///
/// `PRIORITY` defaults to 0, so two contexts declaring nothing in particular still need a
/// tiebreak — declaration order is the only one `add_context`'s caller controls (R8.3).
#[derive(bevy_ecs::schedule::SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct EvaluateSeq(i32, u32);

/// How many contexts have been declared at each priority, so the next one can be numbered and
/// ordered after the last.
#[derive(Resource, Default)]
struct DeclaredPriorities {
    render: alloc::collections::BTreeMap<i32, u32>,
    fixed: alloc::collections::BTreeMap<i32, u32>,
}

/// Numbers a context within its priority and orders its `EvaluateSeq` against its predecessor and,
/// the first time this priority is seen in this schedule, against every other priority already
/// declared. Returns the index to evaluate this context at.
///
/// Done once per context at app build rather than per frame, keeping the single deterministic
/// evaluation pass free of any run-time ordering decision. The number of distinct priorities is
/// small — a handful of layers, not a handful per context.
fn order_by_priority(
    app: &mut App,
    schedule: impl bevy_ecs::schedule::ScheduleLabel + Clone,
    domain: TickDomain,
    priority: i32,
) -> u32 {
    let (index, others) = {
        let mut declared = app
            .world_mut()
            .get_resource_or_insert_with(DeclaredPriorities::default);
        let seen = match domain {
            TickDomain::Render => &mut declared.render,
            TickDomain::Fixed => &mut declared.fixed,
        };
        let index = seen.entry(priority).or_insert(0);
        let this = *index;
        *index += 1;
        let others = if this == 0 {
            declared_others(&declared, domain, priority)
        } else {
            alloc::vec::Vec::new()
        };
        (this, others)
    };

    if index > 0 {
        app.configure_sets(
            schedule.clone(),
            EvaluateSeq(priority, index).after(EvaluateSeq(priority, index - 1)),
        );
    }
    app.configure_sets(
        schedule.clone(),
        EvaluateSeq(priority, index).in_set(EvaluateAt(priority)),
    );

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
    if index == 0 {
        app.configure_sets(
            schedule,
            EvaluateAt(priority).in_set(ActionMapSystems::Evaluate),
        );
    }

    index
}

/// Every other priority already declared in this schedule's domain, for ordering a newly seen one
/// against them.
fn declared_others(
    declared: &DeclaredPriorities,
    domain: TickDomain,
    priority: i32,
) -> alloc::vec::Vec<i32> {
    let seen = match domain {
        TickDomain::Render => &declared.render,
        TickDomain::Fixed => &declared.fixed,
    };
    seen.keys().copied().filter(|&p| p != priority).collect()
}

/// Reads the mappings of one context back out once its type is no longer known.
///
/// Registered per context by `add_context`, which is the last place `C` is available. `Effective`
/// answers with what is bound now, so a settings screen and a conflict check both read the controls
/// the player is actually using; `Declared` answers with the defaults, whatever has since been
/// applied over them. A context nothing has been applied to gives the same list either way, which
/// is the fallback below rather than a case of its own.
fn read_mappings<C: InputContext + Component>(
    world: &World,
    stage: OverrideStage,
) -> alloc::vec::Vec<crate::mapping::ActionMapping> {
    if stage == OverrideStage::Effective
        && let Some(applied) = world.get_resource::<AppliedPlan<C>>()
    {
        return applied.mappings.clone();
    }
    world
        .get_resource::<InputContextPlan<C>>()
        .map(|declared| declared.mappings.clone())
        .unwrap_or_default()
}

/// Reads the tunables of one context back out, on the same terms as [`read_mappings`].
fn read_tunables<C: InputContext + Component>(
    world: &World,
    stage: OverrideStage,
) -> alloc::vec::Vec<crate::mapping::Tunable> {
    if stage == OverrideStage::Effective
        && let Some(applied) = world.get_resource::<AppliedPlan<C>>()
    {
        return applied.tunables.clone();
    }
    world
        .get_resource::<InputContextPlan<C>>()
        .map(|declared| declared.tunables.clone())
        .unwrap_or_default()
}

/// Rewrites one context's bindings for an override set, and swaps the result into every instance.
///
/// Registered per context by `add_context`, like the readers above, and for the same reason: this
/// is the last place `C` is available. `preset` names the rows a preset authorized, exempting
/// exactly those from the "not rebindable here" refusal that would otherwise stop a preset moving a
/// `Fixed` row — see
/// [`apply_overrides_with_preset`](crate::overrides::apply_overrides_with_preset).
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
/// inherits at spawn, and a per-entity apply must not change what the *next* new instance gets,
/// only what this one already-spawned instance has. The diff is still computed against
/// `InputContextPlan<C>`'s pristine declaration, the same baseline `apply_to_context` diffs
/// against, so two entities can diverge independently without either becoming the new default.
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
        binding.input.for_each_part(|part, control| {
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
                .input
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

    let mut instances = world.query::<(
        Entity,
        &InputContextState<C>,
        Option<&crate::player::Paired>,
    )>();
    let consumed = world
        .get_resource::<crate::eval::ConsumedControls>()
        .unwrap_or(&nothing_consumed);

    instances
        .iter(world)
        .map(|(entity, state, pairing)| InstanceDump {
            entity,
            active: state.is_active(),
            actions: state
                .iter()
                .map(|reading| ActionDump {
                    action: reading.action,
                    path: reading.path,
                    state: *reading.state,
                    obstacle: state.why_not_id(reading.action, consumed, pairing),
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
    mappings: &[crate::mapping::ActionMapping],
) {
    let Some(declared) = app
        .world()
        .get_resource::<crate::inspect::DeclaredContexts>()
    else {
        return;
    };

    for context in &declared.0 {
        for taken in (context.mappings)(app.world(), OverrideStage::Declared) {
            // Per family, like the within-a-context check: one action mappable on both the keyboard
            // and the pad is two rows in two tables, not a collision.
            //
            // Unlike the within-a-context check, the *action* is not consulted, and the asymmetry
            // is the point. Two mappable bindings of one action inside one context are a primary
            // and a secondary and merge into one row. The same two in two different contexts are
            // two rows, in two contexts that may be active at different times — and the overrides
            // store is keyed by mapping alone (§10.1), so a rebind of one still lands on the other.
            // Same action, and still a collision.
            //
            // Rebindable rows only, for the reason the within-a-context check gives: the hazard is
            // a *saved* rebind landing on the wrong row, and a fixed row is never saved. Since
            // listing is the default, anything stricter would fail the build of any game binding
            // one action in two contexts — which is ordinary, and which R19.13 promises keeps
            // working for a game that offers no rebinding at all.
            if let Some(clash) = mappings.iter().find(|mapping| {
                mapping.key == taken.key
                    && mapping.family == taken.family
                    && (mapping.rebind_policy.is_rebindable()
                        || taken.rebind_policy.is_rebindable())
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
    // family, including captures for mappings declared in other contexts.
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
            tunables: read_tunables::<C>,
            bindings: read_bindings::<C>,
            apply: apply_to_context::<C>,
            apply_for_entity: apply_to_entity::<C>,
        });

    // A context whose activation follows something else starts inactive and waits to be asked.
    // That is also what catches an instance spawned once the answer is already yes.
    let activation = builder.activation.take();
    let starts_active = activation.is_none();

    let (bindings, class_bindings, delegated) = builder.finish();
    let mut plan = Plan::from_bindings(bindings.clone(), class_bindings);
    plan.delegate(delegated);
    let plan = Arc::new(plan);
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
            .and_then(|hooks| hooks.try_on_remove(detach_context_state::<C>))
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
    match C::TICK {
        TickDomain::Render => {
            let index = order_by_priority(app, PreUpdate, TickDomain::Render, C::PRIORITY);
            app.add_systems(
                PreUpdate,
                (
                    evaluate_context::<C, PreUpdate>.in_set(EvaluateSeq(C::PRIORITY, index)),
                    dispatch,
                    dispatch_classes,
                ),
            );
        }
        TickDomain::Fixed => {
            let index = order_by_priority(app, FixedPreUpdate, TickDomain::Fixed, C::PRIORITY);
            app.add_systems(
                FixedPreUpdate,
                (
                    evaluate_context::<C, FixedPreUpdate>.in_set(EvaluateSeq(C::PRIORITY, index)),
                    dispatch,
                    dispatch_classes,
                ),
            );
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

    use super::super::fixtures::*;
    use crate::ActionMapPlugin;
    use crate::action::ActionPhase;
    use crate::context::ActionMapAppExt;

    use super::super::state::ContextActions;

    /// Counts warnings about a context nobody carries, so the test below can watch for one rather
    /// than assume it.
    ///
    /// A global logger, which `log` allows exactly one of per process — so the two halves of that
    /// test share one, and live in one test rather than racing each other from two.
    mod capture {
        use bevy_platform::sync::atomic::{AtomicUsize, Ordering};

        pub(super) static SEEN: AtomicUsize = AtomicUsize::new(0);
        pub(super) static UNDECLARED: AtomicUsize = AtomicUsize::new(0);

        struct Counting;

        impl log::Log for Counting {
            fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
                metadata.level() <= log::Level::Warn
            }

            fn log(&self, record: &log::Record<'_>) {
                let message = alloc::format!("{}", record.args());
                if message.contains("no entity carries it") {
                    SEEN.fetch_add(1, Ordering::Relaxed);
                }
                if message.contains("was never called for it") {
                    UNDECLARED.fetch_add(1, Ordering::Relaxed);
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

        pub(super) fn undeclared_seen() -> usize {
            UNDECLARED.load(Ordering::Relaxed)
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

    use crate::{InputAction, InputContext};
    use bevy_app::{App, FixedUpdate, Update};
    use bevy_ecs::prelude::Resource;
    use bevy_input::{ButtonState, InputPlugin, keyboard::Key, keyboard::KeyCode};
    use bevy_math::Vec2;

    use crate::binding::{DirectionalButtons, MouseMove};
    use crate::frame::InputFrame;

    /// Taking the component off takes the context off with it, which is the other half of "a
    /// declared context is live as soon as an entity carries it".
    ///
    /// Left behind, the state keeps evaluating — and because pairing usually comes off at the same
    /// time, it does so against *every* device rather than the one it had. A game dropping a player
    /// mid-session gets a character that still walks, driven by whoever else is holding a stick.
    #[cfg(feature = "keyboard")]
    #[test]
    fn removing_the_component_removes_the_context_state() {
        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<FreeLook>(|context| {
            context.bind::<Move>(DirectionalButtons::wasd());
        });

        let entity = app.world_mut().spawn(FreeLook).id();
        app.update();
        assert!(
            app.world()
                .get::<InputContextState<FreeLook>>(entity)
                .is_some(),
            "spawning should have attached the state"
        );

        app.world_mut().entity_mut(entity).remove::<FreeLook>();
        app.update();

        assert!(
            app.world()
                .get::<InputContextState<FreeLook>>(entity)
                .is_none(),
            "the context outlived the component that declared it"
        );
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
        assert_eq!(app.world().resource::<Probe>().phase, ActionPhase::Idle);

        app.world_mut().insert_resource(AtTheControls);
        tick(&mut app);
        assert_eq!(
            app.world().resource::<Probe>().phase,
            ActionPhase::Idle,
            "the key was already down when the condition brought the context up"
        );

        key(&mut app, ButtonState::Released);
        tick(&mut app);
        key(&mut app, ButtonState::Pressed);
        tick(&mut app);
        assert_eq!(
            app.world().resource::<Probe>().phase,
            ActionPhase::Fired,
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

    /// The likelier mistake behind a dead key: nobody called `add_context` at all, so there is no
    /// `InputContextPlan` to say what `Undeclared` should even do.
    #[test]
    fn a_context_nobody_declared_says_so() {
        #[derive(InputContext)]
        #[context(path = "tests.undeclared", tick = Render)]
        struct Undeclared;

        capture::install();
        let before = capture::undeclared_seen();

        let mut app = App::new();
        app.add_plugins(ActionMapPlugin);
        let entity = app.world_mut().spawn(Undeclared).id();

        assert!(
            app.world()
                .get::<InputContextState<Undeclared>>(entity)
                .is_none(),
            "no add_context call means no plan to build state from"
        );
        assert_eq!(capture::undeclared_seen(), before + 1);

        // `on_insert` fired once, on the spawn itself — nothing polls for this per frame.
        for _ in 0..3 {
            app.update();
        }
        assert_eq!(capture::undeclared_seen(), before + 1);
    }

    /// Two contexts at the same priority — both left at the default — still need a winner when
    /// they consume the same control (R8.3). `First` is declared before `Second`, so it claims
    /// `Escape`.
    #[cfg(feature = "keyboard")]
    #[test]
    fn same_priority_contexts_break_the_tie_by_declaration_order() {
        #[derive(InputAction)]
        #[action(path = "tests.first_dismiss", output = bool, intent = Button)]
        struct FirstDismiss;

        #[derive(InputAction)]
        #[action(path = "tests.second_dismiss", output = bool, intent = Button)]
        struct SecondDismiss;

        #[derive(InputContext)]
        #[context(path = "tests.first", tick = Render)]
        struct First;

        #[derive(InputContext)]
        #[context(path = "tests.second", tick = Render)]
        struct Second;

        #[derive(Resource, Default)]
        struct Seen {
            first: bool,
            second: bool,
        }

        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<First>(|context| {
            context.bind::<FirstDismiss>(KeyCode::Escape).consume();
        });
        app.add_context::<Second>(|context| {
            context.bind::<SecondDismiss>(KeyCode::Escape).consume();
        });
        app.world_mut().spawn(First);
        app.world_mut().spawn(Second);
        app.init_resource::<Seen>();
        app.add_systems(
            Update,
            |first: ContextActions<First>,
             second: ContextActions<Second>,
             mut seen: bevy_ecs::system::ResMut<'_, Seen>| {
                seen.first = first.value::<FirstDismiss>();
                seen.second = second.value::<SecondDismiss>();
            },
        );

        app.world_mut()
            .write_message(press(KeyCode::Escape, Key::Escape, ButtonState::Pressed));
        app.update();

        let seen = app.world().resource::<Seen>();
        assert!(
            seen.first,
            "First was declared before Second at the same priority"
        );
        assert!(!seen.second, "so Second never sees the control it lost");
    }

    #[test]
    fn fixed_tick_contexts_do_not_evaluate_in_the_render_schedule() {
        let mut app = jump_app();
        app.init_resource::<Probe>();
        app.add_systems(FixedUpdate, probe_jump);

        app.world_mut()
            .write_message(press(KeyCode::Space, Key::Space, ButtonState::Pressed));
        app.update();

        // Sampling has happened, but `OnFoot` is a fixed-tick context and no fixed tick has run.
        assert_eq!(app.world().resource::<InputFrame>().events().len(), 1);
        let probe = app.world().resource::<Probe>();
        assert!(!probe.value);
        assert_eq!(probe.phase, ActionPhase::Idle);

        run_fixed_tick(&mut app);

        let probe = app.world().resource::<Probe>();
        assert!(probe.value);
        assert_eq!(probe.phase, ActionPhase::Fired);
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
            ActionPhase::Idle,
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
}
