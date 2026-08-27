//! Named indices into `tilemap_packed.png`'s 12×11 grid.
//!
//! Kenney's Tiny Dungeon ships its tiles as `tile_0000.png` … `tile_0131.png` — numbered, not named
//! — so there is nothing to read a tile's purpose from except the pixels. These constants were
//! found by decoding the sample dungeon Kenney ships alongside the sheet (a Tiled map built from
//! this same atlas) and checking each index against a render of that map, tile by tile, rather than
//! read off a preview image by eye. See `assets/split_friction/CREDITS.md` for the license.
//!
//! Only floor and wall are named here — enough for a generated layout. Doors, characters, monsters
//! and weapons get their own names when the chunks that use them need them.

use bevy::prelude::*;

pub const TILE_SIZE: u32 = 16;
pub const COLUMNS: u32 = 12;
pub const ROWS: u32 = 11;

/// Plain floor, no decoration.
pub const FLOOR: usize = 48;
/// Floor variants safe to scatter in for visual variety — none reads as a different surface.
pub const FLOOR_VARIANTS: [usize; 5] = [49, 50, 51, 52, 53];
/// Floor immediately under a north wall, shaded to sit beneath it.
pub const FLOOR_SHADOW: usize = 40;

/// Wall cap, north side: the two corners and the piece repeated between them.
pub const WALL_TOP_LEFT: usize = 1;
pub const WALL_TOP: usize = 2;
pub const WALL_TOP_RIGHT: usize = 3;
/// Wall side, one tile tall, west and east — stacked to make a wall of any height.
pub const WALL_SIDE_LEFT: usize = 13;
pub const WALL_SIDE_RIGHT: usize = 15;
/// Plain brick, no cap or side implied — an obstacle's own interior, or any solid cell too far
/// from floor for a directional piece to make sense.
pub const WALL_FILL: usize = 36;

/// The atlas layout matching `tilemap_packed.png`'s grid, with no padding between tiles.
pub fn layout() -> TextureAtlasLayout {
    TextureAtlasLayout::from_grid(UVec2::splat(TILE_SIZE), COLUMNS, ROWS, None, None)
}
