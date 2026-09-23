//! Pong — a base the single-concept demos build on, rather than grafting their concept onto
//! Disasteroids or Split Friction.
//!
//! Two paddles, a ball, a court, and a score. Player one is the keyboard; player two is the first
//! gamepad the game sees, paired the moment it appears rather than through a join gesture — see
//! [`pong::paddle`]. Neither rebinds, and there is no settings screen: Disasteroids already owns
//! that lesson.
//!
//! `F1` opens the debug panel — see [`pong::overlay`].
//!
//! [`pong`] is what a variant imports — see its own doc comment for how one builds on this base and
//! swaps in the single thing it is demonstrating.

#![allow(missing_docs)]

use bevy::prelude::*;

#[path = "mod.rs"]
mod pong;

#[path = "../common/mod.rs"]
mod common;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Pong".into(),
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
        .add_plugins(pong::plugin)
        .insert_resource(ClearColor(Color::srgb(0.02, 0.02, 0.03)))
        .add_systems(Startup, camera.spawn())
        .run();
}

fn camera() -> impl Scene {
    bsn! { Camera2d }
}
