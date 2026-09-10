//! Pong against a robot, to show an action driven from outside the input map.
//!
//! Identical to [`pong`] but for the left paddle, which no device reaches. Its context binds
//! nothing and delegates its one action; a system chasing the ball writes the value instead. See
//! [`robot`] for the whole of it.
//!
//! You are the right paddle, on `W`/`S`, the arrow keys, or a gamepad. `F1` opens the debug panel,
//! which lists both contexts — the delegated action reads there like any other.

#![allow(missing_docs)]

use bevy::prelude::*;

// A variant reaches part of the base and leaves the rest alone. What it does not call is not dead
// code — it is the piece being replaced, still live in `pong`'s own build.
#[allow(dead_code)]
#[path = "../pong/mod.rs"]
mod pong;

#[path = "../common/mod.rs"]
mod common;

mod robot;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Pong — robot".into(),
                    resolution: (
                        pong::court::HALF_EXTENT.x as u32 * 2,
                        pong::court::HALF_EXTENT.y as u32 * 2,
                    )
                        .into(),
                    ..default()
                }),
                ..default()
            }),
            bevy_action_map::ActionMapPlugin,
        ))
        // Every part of Pong except `paddle`, which `robot` replaces.
        .add_plugins((
            pong::court::plugin,
            pong::ball::plugin,
            pong::score::plugin,
            pong::overlay::plugin,
            robot::plugin,
        ))
        .insert_resource(ClearColor(Color::srgb(0.02, 0.02, 0.03)))
        .add_systems(Startup, camera.spawn())
        .run();
}

fn camera() -> impl Scene {
    bsn! { Camera2d }
}
