//! Pong with a countdown before every serve, to show an action switched off without unbinding it.
//!
//! After each goal the ball waits at center and both paddles freeze for three seconds. Then they
//! move again, and either player serves: `Space` on the keyboard, or the south face button on the
//! pad. See [`rally`] for the whole of it.
//!
//! Try holding `Space` through the countdown. The ball does not leave when it ends; let go and
//! press again. `F1` opens the debug panel, where a frozen action reads as disabled.

#![allow(missing_docs)]

use bevy::prelude::*;

// A variant reaches part of the base and leaves the rest alone. What it does not call is not dead
// code — it is the piece being replaced, still live in `pong`'s own build.
#[allow(dead_code)]
#[path = "../pong/mod.rs"]
mod pong;

#[path = "../common/mod.rs"]
mod common;

mod rally;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Pong — countdown".into(),
                    resolution: (
                        pong::court::HALF_EXTENT.x as u32 * 2,
                        pong::court::HALF_EXTENT.y as u32 * 2,
                    )
                        .into(),
                    ..default()
                }),
                ..default()
            }),
            common::font::plugin,
            bevy_action_map::ActionMapPlugin,
            bevy_remote_driver::RemoteDriverPlugin,
        ))
        // Every part of Pong except `paddle` and `ball`, which `rally` replaces.
        .add_plugins((
            pong::court::plugin,
            pong::score::plugin,
            pong::overlay::plugin,
            rally::plugin,
        ))
        .insert_resource(ClearColor(Color::srgb(0.02, 0.02, 0.03)))
        .add_systems(Startup, camera.spawn())
        .run();
}

fn camera() -> impl Scene {
    bsn! { Camera2d }
}
