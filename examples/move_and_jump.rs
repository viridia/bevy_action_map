#![allow(missing_docs)]

use bevy::prelude::*;
use bevy_action_map::prelude::*;
use bevy_input::{gamepad::GamepadButton, keyboard::KeyCode};

#[derive(InputAction)]
#[action(path = "gameplay.move", output = Vec2, intent = Directional2)]
struct Move;

#[derive(InputAction)]
#[action(path = "gameplay.look", output = Vec2, intent = Delta2)]
struct Look;

#[derive(InputAction)]
#[action(path = "gameplay.jump", output = bool, intent = Button)]
struct Jump;

#[derive(InputContext, Component)]
#[context(path = "gameplay.on_foot", tick = Fixed)]
struct OnFoot;

#[derive(InputContext, Component)]
#[context(path = "gameplay.free_look", tick = Render)]
struct FreeLook;

fn main() {
    let mut app = App::new();
    app.add_plugins((DefaultPlugins, ActionMapPlugin));
    app.add_context::<OnFoot, _>(|context| {
        context.bind::<Move>(DirectionalButtons::wasd());
        context
            .bind::<Move>(Stick::Left)
            .dead_zone(DeadZone::radial(0.15));
        context.bind::<Jump>(KeyCode::Space);
        context.bind::<Jump>(GamepadButton::South);
    });
    app.add_context::<FreeLook, _>(|context| {
        // Look is a delta: the mouse reports how far the view has already turned. The right stick
        // reports how fast to turn instead, so it needs a rate-to-delta conversion this crate does
        // not offer yet, and binding it here would add a rate to a displacement.
        context.bind::<Look>(MouseMove);
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
