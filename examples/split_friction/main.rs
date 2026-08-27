//! Split Friction — a split-screen game in the shape of Gauntlet.
//!
//! This example grows in stages, each landing on its own. This file's only job so far is proving
//! that [`tileset`]'s indices into Kenney's Tiny Dungeon atlas are the tiles they claim to be, with
//! one hand-placed room. Nothing here reads an action yet — the device-selection screen this game
//! exists to demonstrate, plus generation, players, monsters and missiles, all arrive in later
//! stages, none of which should have to touch the tile names this one got right.

#![allow(missing_docs)]

use bevy::image::TextureAtlasTemplate;
use bevy::prelude::*;

mod tileset;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()))
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands, mut layouts: ResMut<Assets<TextureAtlasLayout>>) {
    let layout = layouts.add(tileset::layout());
    commands.spawn(Camera2d);
    commands.spawn_scene(room(layout));
}

/// A 7×3 room: a north wall with a door in it, and floor beneath.
///
/// One room, placed by hand from [`tileset`]'s constants, is the whole of this chunk's acceptance
/// — if the atlas indices were wrong, this would render as noise instead of a room.
fn room(layout: Handle<TextureAtlasLayout>) -> impl Scene {
    #[rustfmt::skip]
    let plan: [[usize; 7]; 3] = [
        [tileset::WALL_TOP_LEFT, tileset::WALL_TOP, tileset::WALL_TOP, tileset::DOOR_CLOSED, tileset::WALL_TOP, tileset::WALL_TOP, tileset::WALL_TOP_RIGHT],
        [tileset::WALL_SIDE_LEFT, tileset::FLOOR_SHADOW, tileset::FLOOR_SHADOW, tileset::FLOOR_SHADOW, tileset::FLOOR_SHADOW, tileset::FLOOR_SHADOW, tileset::WALL_SIDE_RIGHT],
        [tileset::WALL_SIDE_LEFT, tileset::FLOOR, tileset::FLOOR_VARIANTS[0], tileset::FLOOR, tileset::FLOOR_VARIANTS[2], tileset::FLOOR, tileset::WALL_SIDE_RIGHT],
    ];

    let scale = 4.0;
    let tile_px = tileset::TILE_SIZE as f32 * scale;
    let width = plan[0].len() as f32;
    let height = plan.len() as f32;

    let tiles: Vec<_> = plan
        .into_iter()
        .enumerate()
        .flat_map(|(row, cells)| {
            cells
                .into_iter()
                .enumerate()
                .map(move |(col, index)| (row, col, index))
        })
        .map(|(row, col, index)| {
            let x = (col as f32 - (width - 1.0) / 2.0) * tile_px;
            let y = ((height - 1.0) / 2.0 - row as f32) * tile_px;
            tile(layout.clone(), index, Vec2::new(x, y), tile_px)
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

fn tile(layout: Handle<TextureAtlasLayout>, index: usize, pos: Vec2, tile_px: f32) -> impl Scene {
    bsn! {
        Sprite {
            image: "split_friction/tilemap_packed.png",
            texture_atlas: texture_atlas_template(layout, index),
            custom_size: Vec2::splat(tile_px),
        }
        Transform::from_translation(pos.extend(0.0))
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
