//! Dead Zone — an asteroids-like game, playable on the keyboard or a gamepad.
//!
//! The point of the example is [`actions`], which holds the entire input layer: four actions, one
//! context, and the bindings that drive them from either device. Nothing else in the game mentions
//! a key or a button.
//!
//! Fly with `W`/arrow-up and `A`/`D` or the arrow keys, fire with space, jump with left shift. On a
//! pad, the right trigger is the throttle and it is analog — the ship burns as hard as you pull it.

#![allow(missing_docs)]

use bevy::prelude::*;

mod actions;
mod asteroids;
mod field;
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
        ))
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.insert_resource(ClearColor(Color::srgb(0.02, 0.02, 0.05)));
}
