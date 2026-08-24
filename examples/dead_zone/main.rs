//! Dead Zone — an asteroids-like game, playable on the keyboard or a gamepad.
//!
//! The point of the example is [`actions`], which holds the entire input layer: six actions, two
//! contexts, and the bindings that drive them from either device. Nothing else in the game mentions
//! a key or a button.
//!
//! Fly with `W`/arrow-up and `A`/`D` or the arrow keys, fire with space, jump with left shift, and
//! pause with escape. On a pad, the right trigger is the throttle and it is analog — the ship burns
//! as hard as you pull it.
//!
//! The two contexts are the arrangement worth copying. Flying is live only while the game is
//! playing, so pausing stands it down and whatever the player was holding is canceled rather than
//! left running. Pause itself is in a context with no condition at all, because the control that
//! unpauses has to be heard by something that pausing did not switch off.

#![allow(missing_docs)]

use bevy::prelude::*;

mod actions;
mod asteroids;
mod field;
mod overlay;
mod pause;
mod ship;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Dead Zone".into(),
                    resolution: (
                        field::HALF_EXTENT.x as u32 * 2,
                        field::HALF_EXTENT.y as u32 * 2,
                    )
                        .into(),
                    ..default()
                }),
                ..default()
            }),
            bevy_action_map::ActionMapPlugin,
        ))
        .add_plugins((
            actions::plugin,
            field::plugin,
            ship::plugin,
            asteroids::plugin,
            pause::plugin,
            overlay::plugin,
        ))
        .insert_resource(ClearColor(Color::srgb(0.02, 0.02, 0.05)))
        .add_systems(Startup, camera.spawn())
        .run();
}

/// The camera, as a one-entity scene.
///
/// Every spawn in this example is written this way: a plain function returning `impl Scene`, and
/// either `.spawn()` to make a `Startup` system out of it or `Commands::spawn_scene` to spawn one
/// mid-game. `bsn!` is doing very little here, but the shape is the same at every size.
fn camera() -> impl Scene {
    bsn! { Camera2d }
}
