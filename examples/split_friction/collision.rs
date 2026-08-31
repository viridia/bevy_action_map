//! Sliding AABB collision — a protagonist's own movement clamped against the dungeon's solid cells
//! and against the other protagonist, one axis at a time so a wall entered at an angle is slid
//! along rather than stopped dead.
//!
//! No physics engine: a protagonist's own box is fixed-size and axis-aligned, the dungeon's solid
//! cells are already a grid, and the only two moving things ever need testing against each other.

use bevy::prelude::*;

use crate::dungeon::Dungeon;
use crate::{cell_to_world, world_to_cell};

/// Half the side of a protagonist's own collision box, in pixels — smaller than the sprite itself
/// so a slight visual overlap with a wall doesn't read as getting stuck on nothing.
const HALF_EXTENT: f32 = 6.0;
/// Half a dungeon cell's side, in pixels.
const HALF_TILE: f32 = crate::tileset::TILE_SIZE as f32 / 2.0;

/// How far a clamped move stops short of actually touching what blocked it. Landing exactly flush
/// leaves the box one rounding error away from reading as already overlapping on the very next
/// tick — enough to jitter in place, or worse, tunnel through outright the tick after that.
const SKIN: f32 = 0.5;

/// How many times [`clamp_axis`] halves its search interval. Eight steps resolve to under a tenth
/// of a pixel at protagonist speeds, well inside [`SKIN`] — no reason to reach for the exact
/// contact distance when the back-off throws away more precision than one more halving would buy.
const ITERATIONS: u32 = 8;

/// Clamps `delta` so moving `pos` by it never overlaps a solid dungeon cell or `other`'s own box.
///
/// Resolved one axis at a time — `x` against `pos`, then `y` against `pos` already moved by the
/// clamped `x` — which is what turns a wall met at an angle into a slide along it rather than a
/// stop: one axis being blocked never costs the other axis its own full distance.
pub fn resolve(dungeon: &Dungeon, pos: Vec2, other: Vec2, delta: Vec2) -> Vec2 {
    let x = clamp_axis(dungeon, pos, other, Vec2::new(delta.x, 0.0));
    let y = clamp_axis(dungeon, pos + x, other, Vec2::new(0.0, delta.y));
    x + y
}

/// Clamps a single-axis `step` (the other component already zero) to the longest prefix of it that
/// never overlaps anything, less [`SKIN`].
fn clamp_axis(dungeon: &Dungeon, pos: Vec2, other: Vec2, step: Vec2) -> Vec2 {
    if step == Vec2::ZERO || !blocked(dungeon, pos + step, other) {
        return step;
    }

    let mut safe = 0.0_f32;
    let mut unsafe_at = 1.0_f32;
    for _ in 0..ITERATIONS {
        let mid = (safe + unsafe_at) * 0.5;
        if blocked(dungeon, pos + step * mid, other) {
            unsafe_at = mid;
        } else {
            safe = mid;
        }
    }

    let clamped = step * safe;
    let length = clamped.length();
    if length <= SKIN {
        Vec2::ZERO
    } else {
        clamped * (1.0 - SKIN / length)
    }
}

/// Whether a protagonist's own box, centered at `pos`, overlaps `other`'s box or a solid cell.
fn blocked(dungeon: &Dungeon, pos: Vec2, other: Vec2) -> bool {
    if aabb_overlap(pos, HALF_EXTENT, other, HALF_EXTENT) {
        return true;
    }

    let (col, row) = world_to_cell(pos);
    for dy in -1..=1 {
        for dx in -1..=1 {
            let (c, r) = (col + dx, row + dy);
            if dungeon.is_solid(c, r)
                && aabb_overlap(pos, HALF_EXTENT, cell_to_world(c, r), HALF_TILE)
            {
                return true;
            }
        }
    }
    false
}

fn aabb_overlap(a: Vec2, a_half: f32, b: Vec2, b_half: f32) -> bool {
    (a.x - b.x).abs() < a_half + b_half && (a.y - b.y).abs() < a_half + b_half
}
