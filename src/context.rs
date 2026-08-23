//! Contexts: declaring them, and the live state of one.
//!
//! A context groups the bindings that are active together — on foot, in a vehicle, in a menu. You
//! declare one with [`ActionMapAppExt::add_context`] and give it to an entity; that entity then
//! carries an [`InputContextState`] holding the current state of every action in the context.
//!
//! Put the context on whatever the input belongs to. One entity for a single-player game, one per
//! player for local multiplayer, or a bare entity for input that is not tied to anything in
//! particular. Each carries its own state, so two players never share one.

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
use bevy_ecs::world::DeferredWorld;
use bevy_platform::sync::Arc;

use crate::action::{ActionOutput, ActionState, InputAction, InputContext, Phase, TickDomain};
use crate::binding::InputContextBuilder;
use crate::eval::{Transition, dispatch_transitions, evaluate_context};
use crate::frame::{InputFrame, Timestamp};
use crate::plan::Plan;
use crate::{ActionMapPlugin, ActionMapSystems};
#[cfg(feature = "gamepad")]
use bevy_platform::collections::HashMap;

/// The compiled bindings for one context, shared by every instance of it.
// Instances hold an `Arc` to this rather than a copy: ten local players sharing one binding set
// hold one plan and ten small state tables. The hook needs somewhere to read it from on insertion,
// which is why it is also a resource.
#[derive(Resource)]
pub(crate) struct InputContextPlan<C> {
    plan: Arc<Plan<C>>,
    // Whether an instance is live the moment it is spawned. False for a context whose activation
    // follows something else, so that it does not fire for one frame before the something else
    // has had a chance to say otherwise.
    starts_active: bool,
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
/// The struct holds no references into the ECS, so tests and replays can drive one directly
/// without a `World`.
#[derive(Component)]
pub struct InputContextState<C> {
    pub(crate) plan: Arc<Plan<C>>,
    pub(crate) actions: Vec<ActionState>,
    pub(crate) active: bool,
    // Parallel to `actions`: this action may not fire until it has been seen at rest once. Set when
    // a context activates, so a control the player was already holding does not read as a fresh
    // press (R7.5).
    pub(crate) require_reset: Vec<bool>,
    // Every phase change since the last dispatch, in order. Evaluation appends and the dispatcher
    // drains, which is what keeps observers — arbitrary code with `&mut World` — outside the
    // evaluator (R10.2).
    pub(crate) transitions: Vec<Transition>,
    // The last event this context has read. Seeded at spawn rather than left empty, so a context
    // added mid-session starts from the present instead of replaying whatever is still queued.
    pub(crate) read_through: Option<Timestamp>,
    #[cfg(feature = "keyboard")]
    pub(crate) held_buttons: BTreeSet<bevy_input::keyboard::KeyCode>,
    #[cfg(feature = "gamepad")]
    pub(crate) held_gamepad_buttons: HashMap<bevy_input::gamepad::GamepadButton, ButtonReading>,
    #[cfg(feature = "gamepad")]
    pub(crate) held_gamepad_axes: HashMap<bevy_input::gamepad::GamepadAxis, f32>,
    _marker: PhantomData<C>,
}

impl<C> InputContextState<C> {
    pub(crate) fn new(plan: Arc<Plan<C>>, read_through: Option<Timestamp>) -> Self {
        let slots = plan.slot_count();
        let actions = alloc::vec![ActionState::default(); slots];

        Self {
            plan,
            actions,
            active: true,
            require_reset: alloc::vec![false; slots],
            transitions: Vec::new(),
            read_through,
            #[cfg(feature = "keyboard")]
            held_buttons: BTreeSet::new(),
            #[cfg(feature = "gamepad")]
            held_gamepad_buttons: HashMap::default(),
            #[cfg(feature = "gamepad")]
            held_gamepad_axes: HashMap::default(),
            _marker: PhantomData,
        }
    }

    fn action_state<A>(&self) -> &ActionState
    where
        A: InputAction,
    {
        let slot = self
            .plan
            .slot_for_action(A::id())
            .expect("action has not been bound in this context");
        &self.actions[slot]
    }

    /// Reads the typed action value.
    pub fn value<A>(&self) -> A::Output
    where
        A: InputAction,
        A::Output: ActionOutput,
    {
        A::Output::from_action_value(self.action_state::<A>().value)
    }

    /// Returns the current phase for an action.
    pub fn phase<A>(&self) -> Phase
    where
        A: InputAction,
    {
        self.action_state::<A>().phase
    }

    /// Returns `true` when the action was pressed this tick.
    pub fn fired<A>(&self) -> bool
    where
        A: InputAction<Output = bool>,
    {
        self.phase::<A>() == Phase::Fired
    }

    /// Whether this context is currently driving its actions.
    ///
    /// An inactive context keeps up with its devices but stops resolving them into actions, so
    /// reactivating it is immediate and costs no rebuilding.
    pub fn is_active(&self) -> bool {
        self.active
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

        for (slot, state) in self.actions.iter_mut().enumerate() {
            if !matches!(state.phase, Phase::Fired | Phase::Ongoing) {
                continue;
            }
            state.phase = Phase::Canceled;
            state.value = rest_like(state.value);
            self.transitions.push(Transition {
                slot,
                phase: Phase::Canceled,
                value: state.value,
            });
        }
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

/// System parameter for polling a context's actions.
///
/// Most games have one instance of a given context, and [`value`](Self::value),
/// [`phase`](Self::phase) and [`fired`](Self::fired) read it directly. When a context is per-player
/// there will be several, and [`get`](Self::get) reads the one belonging to an entity you name
/// while [`iter`](Self::iter) walks them all.
#[derive(SystemParam)]
pub struct Actions<'w, 's, C: InputContext + Component> {
    states: Query<'w, 's, (Entity, &'static InputContextState<C>)>,
}

impl<C: InputContext + Component> Actions<'_, '_, C> {
    /// Returns the only instance of this context.
    ///
    /// # Panics
    ///
    /// Panics unless exactly one entity carries this context. Use [`get`](Self::get) when a
    /// context is per-player and several instances exist at once.
    pub fn single(&self) -> &InputContextState<C> {
        match self.states.single() {
            Ok((_, state)) => state,
            Err(error) => panic!(
                "`Actions<{}>` expected exactly one context instance: {error}",
                C::PATH
            ),
        }
    }

    /// Returns the instance carried by an entity, if it has one.
    pub fn get(&self, entity: Entity) -> Option<&InputContextState<C>> {
        self.states.get(entity).ok().map(|(_, state)| state)
    }

    /// Iterates every instance of this context and the entity carrying it.
    pub fn iter(&self) -> impl Iterator<Item = (Entity, &InputContextState<C>)> {
        self.states.iter()
    }

    /// Reads the typed action value from the only instance of this context.
    pub fn value<A>(&self) -> A::Output
    where
        A: InputAction,
        A::Output: ActionOutput,
    {
        self.single().value::<A>()
    }

    /// Returns the current phase for an action on the only instance of this context.
    pub fn phase<A>(&self) -> Phase
    where
        A: InputAction,
    {
        self.single().phase::<A>()
    }

    /// Returns `true` when the action was pressed this tick, on the only instance of this context.
    pub fn fired<A>(&self) -> bool
    where
        A: InputAction<Output = bool>,
    {
        self.single().fired::<A>()
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
    let Some(plan) = world.get_resource::<InputContextPlan<C>>() else {
        return;
    };

    let starts_active = plan.starts_active;
    let plan = plan.plan.clone();
    // Whatever is already queued happened before this context existed, so it is not this
    // context's input to react to (R7.5).
    let read_through = world
        .get_resource::<InputFrame>()
        .and_then(InputFrame::latest);
    let mut state = InputContextState::<C>::new(plan, read_through);
    state.active = starts_active;
    world.commands().entity(context.entity).insert(state);
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
    /// A context declared this way is live as soon as an entity carries it. Use
    /// [`add_context_in_state`](Self::add_context_in_state) when it should follow a game state
    /// instead, or drive it yourself with
    /// [`activate`](InputContextState::activate) and [`deactivate`](InputContextState::deactivate).
    ///
    /// # Panics
    ///
    /// Panics if [`ActionMapPlugin`] has not been added, if the same context is declared twice, or
    /// if an entity already carries `C`.
    fn add_context<C: InputContext + Component>(
        &mut self,
        configure: impl FnOnce(&mut InputContextBuilder<C>),
    ) -> &mut Self;

    /// Declares a context that is live exactly while the app is in one state.
    ///
    /// Most contexts come and go with the game's state — flying while playing, a menu while
    /// paused — and keeping the two in step by hand is a bug waiting to happen, because it is the
    /// kind of thing that stays correct until someone adds a third way to reach the menu.
    ///
    /// ```ignore
    /// app.add_context_in_state::<Flying>(GameState::Playing, |controls| {
    ///     controls.bind::<Thrust>(KeyCode::KeyW);
    /// });
    /// app.add_context_in_state::<PauseMenu>(GameState::Paused, |controls| {
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
    ///
    /// # Panics
    ///
    /// As [`add_context`](Self::add_context), plus at run time if `S` was never initialized with
    /// `init_state` or `insert_state`.
    #[cfg(feature = "state")]
    #[cfg_attr(docsrs, doc(cfg(feature = "state")))]
    fn add_context_in_state<C: InputContext + Component>(
        &mut self,
        state: impl bevy_state::prelude::States,
        configure: impl FnOnce(&mut InputContextBuilder<C>),
    ) -> &mut Self;
}

impl ActionMapAppExt for App {
    fn add_context<C: InputContext + Component>(
        &mut self,
        configure: impl FnOnce(&mut InputContextBuilder<C>),
    ) -> &mut Self {
        declare_context(self, configure, true);
        self
    }

    #[cfg(feature = "state")]
    fn add_context_in_state<C: InputContext + Component>(
        &mut self,
        state: impl bevy_state::prelude::States,
        configure: impl FnOnce(&mut InputContextBuilder<C>),
    ) -> &mut Self {
        // Instances start inactive: the sync below is what brings them up, and it is also what
        // catches an instance spawned while the state is already current.
        declare_context(self, configure, false);
        follow_state::<C, _>(self, state);
        self
    }
}

/// The half of declaring a context that does not depend on how it is activated.
fn declare_context<C: InputContext + Component>(
    app: &mut App,
    configure: impl FnOnce(&mut InputContextBuilder<C>),
    starts_active: bool,
) {
    // The plugin owns the set ordering, so a context added before it would evaluate
    // unordered against the sampler.
    assert!(
        app.is_plugin_added::<ActionMapPlugin>(),
        "add ActionMapPlugin before calling add_context"
    );

    let mut builder = InputContextBuilder::<C>::default();
    configure(&mut builder);

    let plan = Arc::new(Plan::from_bindings(builder.finish()));
    app.insert_resource(InputContextPlan::<C> {
        plan,
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

    let evaluate = evaluate_context::<C>.in_set(ActionMapSystems::Evaluate);
    let dispatch = dispatch_transitions::<C>.in_set(ActionMapSystems::Dispatch);
    match C::TICK {
        TickDomain::Render => app.add_systems(PreUpdate, (evaluate, dispatch)),
        TickDomain::Fixed => app.add_systems(FixedPreUpdate, (evaluate, dispatch)),
    };
}

/// Keeps every instance of `C` activated exactly while the app is in `wanted`.
///
/// Placed inside `StateTransition`, after the transition is computed and before the exit and enter
/// schedules run, so a context is already in step by the time an `OnEnter` system looks at it. Both
/// `activate` and `deactivate` return immediately when there is nothing to do, so this costs a
/// comparison on the frames where nothing changed (R7.6).
#[cfg(feature = "state")]
fn follow_state<C, S>(app: &mut App, wanted: S)
where
    C: InputContext + Component,
    S: bevy_state::prelude::States,
{
    use bevy_ecs::prelude::Res;
    use bevy_state::prelude::State;
    use bevy_state::state::{StateTransition, StateTransitionSystems};

    app.add_systems(
        StateTransition,
        (move |current: Option<Res<'_, State<S>>>,
               contexts: Query<'_, '_, &mut InputContextState<C>>| {
            // Absent rather than merely different: a substate or a computed state has no `State`
            // resource at all while its parent does not select it, and that means "not now" rather
            // than being a mistake.
            let live = current.is_some_and(|current| *current.get() == wanted);
            for mut context in contexts {
                if live {
                    context.activate();
                } else {
                    context.deactivate();
                }
            }
        })
        .after(StateTransitionSystems::DependentTransitions)
        .before(StateTransitionSystems::ExitSchedules),
    );
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

    #[derive(InputContext, Component)]
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

    #[derive(InputContext, Component)]
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

    /// One trigger, bound twice: once to an analog action and once to a button action. R2.10 says
    /// the two views are independent, and the assertions below only hold if they are — at 0.42 the
    /// travel is live while the press has not yet happened.
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
        app.add_context_in_state::<OnFoot>(Game::Playing, |context| {
            context.bind::<Jump>(KeyCode::Space);
        });
        app.add_context_in_state::<FreeLook>(Game::Paused, |context| {
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
    /// resource unconditionally panics the moment anyone reaches for a nested state, which is a
    /// perfectly ordinary thing to want — pause is usually a substate of playing.
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
        app.add_context_in_state::<FreeLook>(Play::Running, |context| {
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
        app.add_context_in_state::<FreeLook>(Screen::Playing, |context| {
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

    /// R14.2: a finger resting near the threshold makes the value wobble. Without a release
    /// threshold below the press one, every wobble would be another `Fired`.
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

    /// R14.3: the D-pad has no axis pair anywhere below us, so it becomes a direction the same way
    /// WASD does. Both composites drive one action, and the two are asserted against the same
    /// expected vectors so that a divergence between the keyboard and gamepad paths fails here.
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
}
