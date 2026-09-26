//! Shared scaffolding for the context tests: the action and context types they declare,
//! the input they feed in, and the probes they read back.

use super::*;

use crate::ActionMapPlugin;
use crate::action::ActionPhase;
use crate::context::ActionMapAppExt;
use bevy_ecs::prelude::Query;

use crate::{InputAction, InputContext};
use bevy_app::{App, FixedUpdate};
use bevy_ecs::prelude::Resource;
use bevy_input::{
    ButtonState, InputPlugin, keyboard::Key, keyboard::KeyCode, keyboard::KeyboardInput,
};
use bevy_math::Vec2;

#[cfg(feature = "mouse")]
use bevy_input::mouse::{MouseButton, MouseButtonInput};

// ---- event-and-step helpers, shared by every fixture below ----

pub(super) fn press(key_code: KeyCode, logical_key: Key, state: ButtonState) -> KeyboardInput {
    KeyboardInput {
        key_code,
        logical_key,
        state,
        text: None,
        repeat: false,
        window: bevy_ecs::entity::Entity::PLACEHOLDER,
    }
}

#[cfg(feature = "mouse")]
pub(super) fn mouse_click(button: MouseButton, state: ButtonState) -> MouseButtonInput {
    MouseButtonInput {
        button,
        state,
        window: bevy_ecs::entity::Entity::PLACEHOLDER,
    }
}

// `bevy_time` is not a dependency, so `RunFixedMainLoop` never accumulates enough time to
// step on its own. Run the fixed schedules directly instead: it is what the loop would do,
// and it makes the tick count in each test explicit rather than dependent on wall time.
pub(super) fn run_fixed_tick(app: &mut App) {
    app.world_mut().run_schedule(bevy_app::FixedPreUpdate);
    // Evaluation happens above; a test that reads state directly registers nothing here.
    let _ = app.world_mut().try_run_schedule(FixedUpdate);
}

// ---- Jump / OnFoot: the default single-action fixture, used all over this module ----

#[derive(InputAction)]
#[action(path = "tests.jump", output = bool, intent = Button)]
pub(super) struct Jump;

#[derive(InputContext)]
#[context(path = "tests.on_foot", tick = Fixed)]
pub(super) struct OnFoot;

/// `Jump` bound to `Space`, in a spawned `OnFoot` instance, and nothing else.
pub(super) fn jump_app() -> App {
    let mut app = App::new();
    app.add_plugins((InputPlugin, ActionMapPlugin));
    app.add_context::<OnFoot>(|context| {
        context.bind::<Jump>(KeyCode::Space);
    });
    app.world_mut().spawn(OnFoot);
    app
}

#[derive(Resource, Default)]
pub(super) struct Probe {
    pub(super) value: bool,
    pub(super) phase: ActionPhase,
}

pub(super) fn probe_jump(
    input: ContextActions<OnFoot>,
    mut probe: bevy_ecs::system::ResMut<'_, Probe>,
) {
    probe.value = input.value::<Jump>();
    probe.phase = input.phase::<Jump>();
}

#[derive(Resource, Default)]
pub(super) struct FireCount(pub(super) u32);

pub(super) fn count_jump_fires(
    input: ContextActions<OnFoot>,
    mut count: bevy_ecs::system::ResMut<'_, FireCount>,
) {
    if input.fired::<Jump>() {
        count.0 += 1;
    }
}

pub(super) fn jump_fire_count_app() -> App {
    let mut app = jump_app();
    app.init_resource::<FireCount>();
    app.add_systems(FixedUpdate, count_jump_fires);
    app
}

/// What a subscriber sees: how many instances the `Changed` filter offered on the last tick.
#[derive(Resource, Default)]
pub(super) struct Woken(pub(super) usize);

pub(super) fn count_woken(
    woken: Query<'_, '_, (), bevy_ecs::prelude::Changed<InputContextState<OnFoot>>>,
    mut probe: bevy_ecs::system::ResMut<'_, Woken>,
) {
    probe.0 = woken.iter().count();
}

#[derive(InputAction)]
#[action(path = "tests.never_bound_anywhere", output = bool, intent = Button)]
pub(super) struct NeverBoundAnywhere;

// ---- Move / Look / Turn / FreeLook: the directional-and-motion fixture ----

#[derive(InputAction)]
#[action(path = "tests.move", output = Vec2, intent = Directional2)]
pub(super) struct Move;

#[derive(InputAction)]
#[action(path = "tests.look", output = Vec2, intent = Delta2)]
pub(super) struct Look;

#[cfg(feature = "gamepad")]
#[derive(InputAction)]
#[action(path = "tests.turn", output = f32, intent = Analog1)]
pub(super) struct Turn;

#[derive(InputContext)]
#[context(path = "tests.free_look", tick = Render)]
pub(super) struct FreeLook;

#[derive(Resource, Default)]
pub(super) struct MotionProbe {
    pub(super) movement: Vec2,
    pub(super) look: Vec2,
}

pub(super) fn probe_motion(
    input: ContextActions<FreeLook>,
    mut probe: bevy_ecs::system::ResMut<'_, MotionProbe>,
) {
    probe.movement = input.value::<Move>();
    probe.look = input.value::<Look>();
}

#[cfg(feature = "gamepad")]
#[derive(Resource, Default)]
pub(super) struct GamepadProbe {
    pub(super) movement: Vec2,
    pub(super) turn: f32,
    pub(super) jump: bool,
    pub(super) jump_phase: ActionPhase,
}

#[cfg(feature = "gamepad")]
pub(super) fn probe_gamepad(
    input: ContextActions<OnFoot>,
    mut probe: bevy_ecs::system::ResMut<'_, GamepadProbe>,
) {
    probe.movement = input.value::<Move>();
    probe.turn = input.value::<Turn>();
    probe.jump = input.value::<Jump>();
    probe.jump_phase = input.phase::<Jump>();
}

// ---- Thrust: one trigger serving an analog and a button action at once ----

#[cfg(feature = "gamepad")]
#[derive(InputAction)]
#[action(path = "tests.thrust", output = f32, intent = Analog1)]
pub(super) struct Thrust;

#[cfg(feature = "gamepad")]
#[derive(Resource, Clone, Copy, Default)]
pub(super) struct TriggerProbe {
    pub(super) travel: f32,
    pub(super) pressed: bool,
    pub(super) phase: ActionPhase,
}

// ---- misc single-test fixtures ----

#[derive(Resource)]
pub(super) struct AtTheControls;
