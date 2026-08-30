//! Split Friction — a split-screen game in the shape of Gauntlet.
//!
//! This example grows in stages, each landing on its own. So far it generates a Gauntlet-sized
//! dungeon from a seed (see [`dungeon`]) and draws it with Kenney's Tiny Dungeon atlas (see
//! [`tileset`]). Nothing here reads an action yet — the device-selection screen this game exists to
//! demonstrate, plus players, monsters and missiles, all arrive in later stages, none of which
//! should have to touch the generator or the tile names this one got right.
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

/// The generated arena, one sprite per cell — every cell resolves to something, so there is no
/// "skip this one" case here the way chunk 56's static room never needed either.
///
/// Wrong atlas indices would be one kind of failure here, same as chunk 56's hand-placed room; a
/// generator bug is a different one, and looks like a different thing on screen — an obstacle with
/// a hole in it, a gap too narrow to walk through, floor that never connects. Both are visible at a
/// glance.
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

// /// This chunk's only tie between the generator's vocabulary and this atlas's — everywhere else,
// /// each stays free to change without the other noticing.
// fn tile_index(role: TileRole) -> usize {
//     match role {
//         TileRole::Floor(0) => tileset::FLOOR,
//         TileRole::Floor(n) => tileset::FLOOR_VARIANTS[n as usize - 1],
//         TileRole::FloorShadowEdge(_) => tileset::FLOOR_SHADOW_EDGE,
//         TileRole::FloorShadowCorner(_) => tileset::FLOOR_SHADOW_CORNER,
//         TileRole::FloorShadowNub(_) => tileset::FLOOR_SHADOW_NUB,
//         // Open never rolls a Prop (see resolve_cell), but the match stays exhaustive rather than
//         // leaning on that invariant here too.
//         TileRole::Prop(RegionAspect::Open) => tileset::FLOOR,
//         TileRole::Prop(RegionAspect::Vault) => tileset::CHEST,
//         TileRole::Prop(RegionAspect::Ruins) => tileset::RUBBLE,
//         TileRole::Prop(RegionAspect::Shrine) => tileset::SHRINE,
//         TileRole::WallCapFar => tileset::WALL_CAP_FAR,
//         TileRole::WallCapFarWest => tileset::WALL_CAP_FAR_WEST,
//         TileRole::WallCapFarEast => tileset::WALL_CAP_FAR_EAST,
//         TileRole::WallBaseFar(0) => tileset::WALL_BASE_FAR,
//         TileRole::WallBaseFar(_) => tileset::WALL_BASE_FAR_VARIANT,
//         TileRole::WallCapNear => tileset::WALL_CAP_NEAR,
//         TileRole::WallCapNearWest => tileset::WALL_CAP_NEAR_WEST,
//         TileRole::WallCapNearEast => tileset::WALL_CAP_NEAR_EAST,
//         TileRole::WallSideWest => tileset::WALL_SIDE_WEST,
//         TileRole::WallSideEast => tileset::WALL_SIDE_EAST,
//         TileRole::WallNub(_) => tileset::WALL_NUB,
//         TileRole::WallFill(0) => tileset::WALL_FILL,
//         TileRole::WallFill(_) => tileset::WALL_FILL_VARIANT,
//     }
// }

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
