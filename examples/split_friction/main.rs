//! Split Friction — a split-screen game in the shape of Gauntlet.
//!
//! So far: a generated dungeon, drawn with Kenney's Tiny Dungeon atlas, and two protagonists —
//! sprites 99 and 100, the two that read female, in keeping with the game this parodies — spawned a
//! short distance apart in the dungeon's first room, each seen through its own camera. Player 1
//! moves with the arrow keys; player 2 moves with a gamepad stick once one is connected — which
//! device drives which protagonist is hardcoded for now, see [`protagonist`]. Monsters and the
//! device-selection screen this game exists to demonstrate arrive in later stages.
//!
//! Pass a seed on the command line to see a different layout: `cargo run --example split_friction --
//! 7`. With none given, the layout is the same every run.

#![allow(missing_docs)]

use bevy::image::TextureAtlasTemplate;
use bevy::prelude::*;

mod dungeon;
mod protagonist;
mod split_screen;
mod tileset;

const WIDTH: usize = 64;
const HEIGHT: usize = 64;

fn main() {
    let seed = std::env::args()
        .nth(1)
        .and_then(|arg| arg.parse().ok())
        .unwrap_or(42);

    App::new()
        .add_plugins((
            DefaultPlugins.set(ImagePlugin::default_nearest()),
            bevy_action_map::ActionMapPlugin,
        ))
        .add_plugins((protagonist::plugin, split_screen::plugin))
        .insert_resource(Seed(seed))
        .add_systems(Startup, setup)
        .run();
}

#[derive(Resource)]
struct Seed(u64);

fn setup(mut commands: Commands, mut layouts: ResMut<Assets<TextureAtlasLayout>>, seed: Res<Seed>) {
    let layout = layouts.add(tileset::layout());
    let dungeon = dungeon::generate(seed.0, WIDTH, HEIGHT);
    let spawn = dungeon
        .spawn_points()
        .map(|(col, row)| cell_to_world(col, row));

    commands.spawn_scene(map(layout.clone(), &dungeon, seed.0));
    commands.spawn_scene(protagonist::pair(layout, spawn));
}

/// The generated dungeon, one sprite per cell.
fn map(layout: Handle<TextureAtlasLayout>, dungeon: &dungeon::Dungeon, seed: u64) -> impl Scene {
    let roles = dungeon.resolve(seed);
    let tile_px = tileset::TILE_SIZE as f32;

    let tiles: Vec<_> = roles
        .into_iter()
        .enumerate()
        .map(|(i, (tile_index, rotation))| {
            let (col, row) = (i % WIDTH, i / WIDTH);
            tile(
                layout.clone(),
                tile_index as usize,
                cell_to_world(col, row),
                tile_px,
                rotation.as_quat(),
            )
        })
        .collect();

    bsn! {
        Transform::default()
        Visibility::default()
        Children [
            {tiles},
        ]
    }
}

/// A cell's center, in world units — column and row grow right and down, the grid centered on the
/// origin.
fn cell_to_world(col: usize, row: usize) -> Vec2 {
    let tile_px = tileset::TILE_SIZE as f32;
    let width = WIDTH as f32;
    let height = HEIGHT as f32;
    Vec2::new(
        (col as f32 - (width - 1.0) / 2.0) * tile_px,
        ((height - 1.0) / 2.0 - row as f32) * tile_px,
    )
}

fn tile(
    layout: Handle<TextureAtlasLayout>,
    index: usize,
    pos: Vec2,
    tile_px: f32,
    rotation: Quat,
) -> impl Scene {
    bsn! {
        Sprite {
            image: "split_friction/tilemap_packed.png",
            texture_atlas: texture_atlas_template(layout, index),
            custom_size: Vec2::splat(tile_px),
        }
        Transform {
            translation: {pos.extend(0.0)},
            rotation: {rotation},
        }
    }
}

fn texture_atlas_template(
    layout: Handle<TextureAtlasLayout>,
    index: usize,
) -> TextureAtlasTemplate {
    TextureAtlasTemplate {
        layout: layout.into(),
        index,
    }
}
