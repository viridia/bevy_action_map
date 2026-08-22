#![allow(missing_docs)]

use bevy::prelude::*;
use bevy_action_map::prelude::*;
use bevy_input::keyboard::KeyCode;

#[derive(bevy_action_map::InputAction)]
#[action(path = "gameplay.move", output = Vec2, intent = Directional2)]
struct Move;

#[derive(bevy_action_map::InputAction)]
#[action(path = "gameplay.look", output = Vec2, intent = Delta2)]
struct Look;

#[derive(bevy_action_map::InputAction)]
#[action(path = "gameplay.jump", output = bool, intent = Button)]
struct Jump;

#[derive(bevy_action_map::InputContext)]
#[context(path = "gameplay.on_foot", tick = Fixed)]
struct OnFoot;

#[derive(bevy_action_map::InputContext)]
#[context(path = "gameplay.free_look", tick = Render)]
struct FreeLook;

fn main() {
    let mut app = App::new();
    app.add_plugins((DefaultPlugins, InputFramePlugin, ActionMapPlugin));
    app.add_context::<OnFoot, _>(|context| {
        context.bind_directional::<Move>(DirectionalKeys::new(
            KeyCode::KeyW,
            KeyCode::KeyS,
            KeyCode::KeyA,
            KeyCode::KeyD,
        ));
        context.bind::<Jump>(KeyCode::Space);
    });
    app.add_context::<FreeLook, _>(|context| {
        context.bind_mouse_motion::<Look>();
    });
    app.add_systems(FixedUpdate, move_player);
    app.add_systems(Update, look_camera);

    app.run();
}

fn move_player(input: Actions<'_, OnFoot>, mut position: Local<Vec3>) {
    let movement = input.value::<Move>();
    *position += Vec3::new(movement.x, 0.0, movement.y);

    if input.fired::<Jump>() {
        info!("Jump fired");
    }
}

fn look_camera(input: Actions<'_, FreeLook>) {
    let delta = input.value::<Look>();
    if delta != Vec2::ZERO {
        info!("Look delta: {delta:?}");
    }
}
