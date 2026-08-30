//! Split Friction — a split-screen game in the shape of Gauntlet.
//!
//! So far: a generated dungeon, drawn with Kenney's Tiny Dungeon atlas. Players, monsters, and the
//! device-selection screen this game exists to demonstrate all arrive in later stages.
//!
//! Pass a seed on the command line to see a different layout: `cargo run --example split_friction --
//! 7`. With none given, the layout is the same every run.

#![allow(missing_docs)]

use bevy::image::TextureAtlasTemplate;
use bevy::prelude::*;

mod dungeon;
mod tileset;

const WIDTH: usize = 64;
const HEIGHT: usize = 64;

fn main() {
    let seed = std::env::args()
        .nth(1)
        .and_then(|arg| arg.parse().ok())
        .unwrap_or(42);

    App::new()
        .add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()))
        .insert_resource(Seed(seed))
        .add_systems(Startup, setup)
        .run();
}

#[derive(Resource)]
struct Seed(u64);

fn setup(mut commands: Commands, mut layouts: ResMut<Assets<TextureAtlasLayout>>, seed: Res<Seed>) {
    let layout = layouts.add(tileset::layout());
    commands.spawn(Camera2d);
    commands.spawn_scene(map(layout, seed.0));
}

/// The generated dungeon, one sprite per cell.
fn map(layout: Handle<TextureAtlasLayout>, seed: u64) -> impl Scene {
    let dungeon = dungeon::generate(seed, WIDTH, HEIGHT);
    let roles = dungeon.resolve(seed);

    let tile_px = tileset::TILE_SIZE as f32;
    let width = WIDTH as f32;
    let height = HEIGHT as f32;

    let tiles: Vec<_> = roles
        .into_iter()
        .enumerate()
        .map(|(i, (tile_index, rotation))| {
            let (col, row) = (i % WIDTH, i / WIDTH);
            let x = (col as f32 - (width - 1.0) / 2.0) * tile_px;
            let y = ((height - 1.0) / 2.0 - row as f32) * tile_px;
            tile(
                layout.clone(),
                tile_index as usize,
                Vec2::new(x, y),
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
