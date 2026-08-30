//! Named indices into `tilemap_packed.png`'s 12×11 grid.
//!
//! Kenney's Tiny Dungeon ships its tiles as `tile_0000.png` … `tile_0131.png` — numbered, not named
//! — so there is nothing to read a tile's purpose from except the pixels, cross-checked against
//! Kenney's own preview of the sheet in use. See `assets/split_friction/CREDITS.md` for the license.
//!
//! Only floor, wall, and the region-aspect props are named here — enough for a generated layout.
//! Doors, characters, monsters and weapons get their own names when the chunks that use them need
//! them.

use bevy::prelude::*;

pub const TILE_SIZE: u32 = 16;
pub const COLUMNS: u32 = 12;
pub const ROWS: u32 = 11;

/// Plain floor, no decoration.
pub const FLOOR: u8 = 48;
pub const FLOOR_PEBBLES: u8 = 49;
pub const FLOOR_STONE: u8 = 42;
/// The edge piece of a wall's drop shadow on the floor beside it, at `Rotation::R0` (solid to the
/// north). `Rotation::R90` turns it to face a wall to the east instead — the light is modeled as
/// coming from the northeast, so those are the only two sides that ever cast one.
pub const FLOOR_SHADOW_EDGE: u8 = 50;
/// The corner piece of a wall's drop shadow, cast where solid ground is both north and east.
pub const FLOOR_SHADOW_CORNER: u8 = 52;
pub const FLOOR_SHADOW_NUB: u8 = 53;

/// A chest — the `Vault` aspect's prop.
pub const CHEST: u8 = 89;
/// A standing shrine — the `Shrine` aspect's prop.
pub const SHRINE: u8 = 41;

/// One of Split Friction's two protagonists — the two Tiny Dungeon sprites that read female, in
/// keeping with the game this is a parody of.
pub const PROTAGONIST_1: u8 = 99;
/// The other protagonist. See [`PROTAGONIST_1`].
pub const PROTAGONIST_2: u8 = 100;

/// The far wall's cap — straight, and at its west and east ends where it meets a side wall.
pub const WALL_CAP_NEAR: u8 = 2;
pub const WALL_CAP_NEAR_EAST: u8 = 1;
pub const WALL_CAP_NEAR_WEST: u8 = 3;
pub const WALL_CAP_NEAR_SE: u8 = 16;
pub const WALL_CAP_NEAR_SW: u8 = 17;
/// The far wall's own face, one cell nearer the room than its cap.
pub const WALL_FRONT: u8 = 40;
/// A dimmer variant of [`WALL_FRONT`], scattered in rarely for texture.
pub const WALL_FRONT_VARIANT: u8 = 14;
pub const WALL_FRONT_GRATING: u8 = 28;
pub const WALL_FRONT_BANNER: u8 = 29;
/// The near wall's cap — the whole wall, since nothing stands between it and the room. Straight,
/// and at its west and east ends.
pub const WALL_CAP_FAR: u8 = 26;
pub const WALL_CAP_FAR_WEST: u8 = 4;
pub const WALL_CAP_FAR_EAST: u8 = 5;
pub const WALL_CAP_FAR_NW: u8 = 27;
pub const WALL_CAP_FAR_NE: u8 = 25;
/// The wall flanking a room or passage on its west side, one tile tall — stacked to make a wall of
/// any height.
pub const WALL_SIDE_WEST: u8 = 13;
/// As [`WALL_SIDE_WEST`], on the east side.
pub const WALL_SIDE_EAST: u8 = 15;
/// Plain solid ground, no cap or side implied — a room or passage's own interior, or any solid cell
/// too far from floor for a directional piece to make sense.
pub const WALL_FILL: u8 = 0;
/// A pebbled variant of [`WALL_FILL`], scattered in rarely for texture.
pub const WALL_FILL_VARIANT: u8 = 12;
pub const WALL_FILL_WEST: u8 = 57;
pub const WALL_FILL_EAST: u8 = 59;
pub const WALL_FILL_NARROW: u8 = 58;

/// The atlas layout matching `tilemap_packed.png`'s grid, with no padding between tiles.
pub fn layout() -> TextureAtlasLayout {
    TextureAtlasLayout::from_grid(UVec2::splat(TILE_SIZE), COLUMNS, ROWS, None, None)
}
