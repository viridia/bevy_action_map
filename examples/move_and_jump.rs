#![allow(missing_docs)]

use bevy::prelude::*;
use bevy_action_map::prelude::*;
use bevy_input::{gamepad::GamepadButton, keyboard::KeyCode};

#[derive(bevy_action_map::InputAction)]
#[action(path = "gameplay.move", output = Vec2, intent = Directional2)]
struct Move;

#[derive(bevy_action_map::InputAction)]
#[action(path = "gameplay.look", output = Vec2, intent = Delta2)]
struct Look;

#[derive(bevy_action_map::InputAction)]
#[action(path = "gameplay.jump", output = bool, intent = Button)]
struct Jump;

#[derive(bevy_action_map::InputContext, Component)]
#[context(path = "gameplay.on_foot", tick = Fixed)]
struct OnFoot;

#[derive(bevy_action_map::InputContext, Component)]
#[context(path = "gameplay.free_look", tick = Render)]
struct FreeLook;

fn main() {
    let mut app = App::new();
    app.add_plugins((DefaultPlugins, ActionMapPlugin));
    app.add_context::<OnFoot, _>(|context| {
        context.bind_directional::<Move>(DirectionalKeys::new(
            KeyCode::KeyW,
            KeyCode::KeyS,
            KeyCode::KeyA,
            KeyCode::KeyD,
        ));
        context.bind::<Move, _>(Stick::Left).deadzone(0.15);
        context.bind::<Jump, _>(KeyCode::Space);
        context.bind::<Jump, _>(GamepadButton::South);
    });
    app.add_context::<FreeLook, _>(|context| {
        context.bind_mouse_motion::<Look>();
        context
            .bind::<Look, _>(Stick::Right)
            .deadzone(0.12)
            .curve(1.8);
    });
    // The player owns the on-foot bindings; the camera looks around on its own.
    app.add_systems(Startup, |mut commands: Commands| {
        commands.spawn(OnFoot);
        commands.spawn(FreeLook);
    });
    app.add_systems(FixedUpdate, move_player);
    app.add_systems(Update, look_camera);

    app.run();
}

fn move_player(input: Actions<OnFoot>, mut position: Local<Vec3>) {
    let movement = input.value::<Move>();
    *position += Vec3::new(movement.x, 0.0, movement.y);

    if input.fired::<Jump>() {
        info!("Jump fired");
    }
}

fn look_camera(input: Actions<FreeLook>) {
    let delta = input.value::<Look>();
    if delta != Vec2::ZERO {
        info!("Look delta: {delta:?}");
    }
}
