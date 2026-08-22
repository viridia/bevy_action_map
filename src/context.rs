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
use crate::eval::evaluate_context;
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
    #[cfg(feature = "keyboard")]
    pub(crate) held_buttons: BTreeSet<bevy_input::keyboard::KeyCode>,
    #[cfg(feature = "gamepad")]
    pub(crate) held_gamepad_buttons: HashMap<bevy_input::gamepad::GamepadButton, f32>,
    #[cfg(feature = "gamepad")]
    pub(crate) held_gamepad_axes: HashMap<bevy_input::gamepad::GamepadAxis, f32>,
    _marker: PhantomData<C>,
}

impl<C> InputContextState<C> {
    pub(crate) fn new(plan: Arc<Plan<C>>) -> Self {
        let actions = alloc::vec![ActionState::default(); plan.slot_count()];

        Self {
            plan,
            actions,
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
            .expect("action value does not match its declared output shape")
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

    let state = InputContextState::<C>::new(plan.plan.clone());
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
    /// app.add_context::<OnFoot, _>(|context| {
    ///     context.bind::<Jump, _>(KeyCode::Space);
    /// });
    /// // ...then, in a startup system or a scene:
    /// commands.spawn((Player, OnFoot));
    /// ```
    ///
    /// The context's tick domain decides where it is evaluated: a `Render` context runs in
    /// `PreUpdate` and a `Fixed` context in `FixedPreUpdate`, both before the schedule you would
    /// normally read the actions from.
    ///
    /// # Panics
    ///
    /// Panics if [`ActionMapPlugin`] has not been added, if the same context is declared twice, or
    /// if an entity already carries `C`.
    fn add_context<C, F>(&mut self, configure: F) -> &mut Self
    where
        C: InputContext + Component,
        F: FnOnce(&mut InputContextBuilder<C>);
}

impl ActionMapAppExt for App {
    fn add_context<C, F>(&mut self, configure: F) -> &mut Self
    where
        C: InputContext + Component,
        F: FnOnce(&mut InputContextBuilder<C>),
    {
        // The plugin owns the set ordering, so a context added before it would evaluate
        // unordered against the sampler.
        assert!(
            self.is_plugin_added::<ActionMapPlugin>(),
            "add ActionMapPlugin before calling add_context"
        );

        let mut builder = InputContextBuilder::<C>::default();
        configure(&mut builder);

        let plan = Arc::new(Plan::from_bindings(builder.finish()));
        self.insert_resource(InputContextPlan::<C> { plan });

        // The hook can only be attached while no entity carries `C` yet, so declaring a context
        // has to precede spawning into it. Bevy's own assertion here says nothing about
        // `add_context`, which is why this one exists.
        let world = self.world_mut();
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
        match C::TICK {
            TickDomain::Render => self.add_systems(PreUpdate, evaluate),
            TickDomain::Fixed => self.add_systems(FixedPreUpdate, evaluate),
        };
        self
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
    use crate::binding::{DeadZone, DirectionalKeys};
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
        app.add_context::<OnFoot, _>(|context| {
            context.bind::<Jump, _>(KeyCode::Space);
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
        look: Vec2,
        jump: bool,
        jump_phase: Phase,
    }

    #[cfg(feature = "gamepad")]
    fn probe_gamepad(
        input: Actions<OnFoot>,
        mut probe: bevy_ecs::system::ResMut<'_, GamepadProbe>,
    ) {
        probe.movement = input.value::<Move>();
        probe.look = input.value::<Look>();
        probe.jump = input.value::<Jump>();
        probe.jump_phase = input.phase::<Jump>();
    }

    #[test]
    fn directional_composites_and_mouse_motion_stay_live_across_frames() {
        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<FreeLook, _>(|context| {
            context.bind_directional::<Move>(DirectionalKeys::new(
                KeyCode::KeyW,
                KeyCode::KeyS,
                KeyCode::KeyA,
                KeyCode::KeyD,
            ));
            context.bind_mouse_motion::<Look>();
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
    #[test]
    fn raw_gamepad_events_drive_sticks_and_buttons() {
        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<OnFoot, _>(|context| {
            context
                .bind::<Move, _>(Stick::Left)
                .dead_zone(DeadZone::radial(0.2));
            context.bind::<Jump, _>(GamepadButton::South);
            context.bind::<Look, _>(Stick::Right);
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
        assert_eq!(probe.look, Vec2::new(-0.5, 0.25));
        assert!(probe.jump);
        assert_eq!(probe.jump_phase, Phase::Fired);

        app.update();
        run_fixed_tick(&mut app);

        let probe = app.world().resource::<GamepadProbe>();
        assert_eq!(probe.movement, Vec2::new(0.0, 0.375));
        assert_eq!(probe.look, Vec2::new(-0.5, 0.25));
        assert!(probe.jump);
        assert_eq!(probe.jump_phase, Phase::Ongoing);
    }

    #[test]
    fn fixed_tick_contexts_do_not_evaluate_in_the_render_schedule() {
        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<OnFoot, _>(|context| {
            context.bind::<Jump, _>(KeyCode::Space);
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
        app.add_context::<OnFoot, _>(|context| {
            context.bind::<Jump, _>(KeyCode::Space);
            context.bind::<Jump, _>(GamepadButton::South);
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
        app.add_context::<FreeLook, _>(|context| {
            context.bind_directional::<Move>(DirectionalKeys::new(
                KeyCode::KeyW,
                KeyCode::KeyS,
                KeyCode::KeyA,
                KeyCode::KeyD,
            ));
            context.bind::<Move, _>(Stick::Left);
            context.bind_mouse_motion::<Look>();
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
        app.add_context::<FreeLook, _>(|context| {
            context.bind_directional::<Move>(DirectionalKeys::new(
                KeyCode::KeyW,
                KeyCode::KeyS,
                KeyCode::KeyA,
                KeyCode::KeyD,
            ));
            context.bind_mouse_motion::<Look>();
            context.bind_mouse_motion::<Look>().scale(2.0);
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
        app.add_context::<OnFoot, _>(|context| {
            context.bind::<Jump, _>(KeyCode::Space);
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
        app.add_context::<FreeLook, _>(|context| {
            context
                .bind_mouse_motion::<Look>()
                .dead_zone(DeadZone::radial(0.05))
                .dead_zone(DeadZone::radial(0.15));
        });
    }

    #[test]
    fn a_trimming_dead_zone_composes_with_a_rescaling_one() {
        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<FreeLook, _>(|context| {
            context
                .bind_mouse_motion::<Look>()
                .dead_zone(DeadZone::radial(0.05).without_rescale())
                .dead_zone(DeadZone::radial(0.15));
        });
        app.world_mut().spawn(FreeLook);
        app.update();
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
        app.add_context::<FreeLook, _>(|context| {
            context.bind_directional::<Move>(DirectionalKeys::new(
                KeyCode::KeyW,
                KeyCode::KeyS,
                KeyCode::KeyA,
                KeyCode::KeyD,
            ));
            context.bind_mouse_motion::<Look>();
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
