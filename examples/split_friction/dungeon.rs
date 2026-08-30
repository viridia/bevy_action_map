//! A seeded arena layout, and what each cell needs drawn over it.
//!
//! No Bevy here, deliberately: a layout is a pure function of its seed, so it can be checked with
//! plain unit tests — same seed in, same grid out, every open cell reachable from every other one —
//! faster and more thoroughly than watching a window across fifty seeds ever could.
//!
//! The shape: rooms of varying size, connected by passages wide enough for two characters to stand
//! side by side ([`PASSAGE_WIDTH`]), grown outward from a single seed room by picking a random point
//! on the unclaimed perimeter of what exists so far and trying to dig a new room beyond it — the
//! [`Frontier`] set. A second pass then looks for rooms that ended up facing each other across a
//! narrow gap and, at random, connects some of them too, so the result has loops rather than being a
//! single tree.
//!
//! [`Dungeon::resolve`] then looks at each cell's neighbors to decide what belongs there — see
//! [`TileRole`] for the vocabulary that comes out of it, and a plain filled wall for any solid cell
//! none of it reaches — so every cell draws *something*, and nothing reads as a hole in the world.

use bevy_math::Quat;

use crate::tileset::*;

/// How wide a passage must be for two characters to stand side by side in it.
const PASSAGE_WIDTH: usize = 2;
/// Room footprint, in cells, along each axis.
const ROOM_MIN: usize = 5;
const ROOM_MAX: usize = 10;
/// How far a passage runs before the next room starts.
const PASSAGE_LEN_MIN: usize = 1;
const PASSAGE_LEN_MAX: usize = 3;
/// Growth stops once this many rooms exist, or sooner if the frontier runs dry first.
const TARGET_ROOMS: usize = 40;
/// Outer wall thickness.
const BORDER: usize = 1;
/// A facing gap wider than this is too far to read as a connection between two rooms.
const MAX_LOOP_GAP: usize = 3;
/// One in this many qualifying facing gaps gets a connecting passage.
const LOOP_CHANCE_DEN: u32 = 2;
/// One in this many cells of a themed room's floor gets a prop, so a room reads as a handful of set
/// dressing rather than either bare or wall-to-wall clutter.
const PROP_CHANCE_DEN: u64 = 18;
/// One in this many rooms gets each non-`Open` aspect (so half of all rooms stay plain).
const ASPECT_CHANCE_DEN: u64 = 6;

/// One cell of the layout, before it has been resolved to anything drawable.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Cell {
    Solid,
    SolidFront, // Solid, but with a floor in front of it
    Floor,
}

/// A room's thematic character — which prop, if any, its floor rarely scatters in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RegionAspect {
    /// No prop; the common case.
    Open,
    /// A chest sits here.
    Vault,
    /// Rubble litters the floor.
    Ruins,
    /// A shrine stands here.
    Shrine,
}

/// A quarter-turn, clockwise. [`TileRole::WallNub`] and the floor-shadow roles each have only one
/// drawn orientation — Kenney's sheet leaves the other three to be produced by rotating the tile
/// itself, the way [`main`](../main) already has to for every rendered tile's `Transform`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rotation {
    R0,
    R90,
    R180,
    R270,
}

impl Rotation {
    pub(crate) fn as_quat(&self) -> Quat {
        let clockwise_turns = match self {
            Rotation::R0 => 0.0,
            Rotation::R90 => 1.0,
            Rotation::R180 => 2.0,
            Rotation::R270 => 3.0,
        };
        Quat::from_rotation_z(-clockwise_turns * core::f32::consts::FRAC_PI_2)
    }
}

pub struct Dungeon {
    pub width: usize,
    pub height: usize,
    cells: Vec<Cell>,
    /// Which room (an index into `aspects`) owns a given floor cell — `None` for passages, and for
    /// solid cells.
    room_of: Vec<Option<usize>>,
    aspects: Vec<RegionAspect>,
}

impl Dungeon {
    fn cell_at(&self, x: isize, y: isize) -> Cell {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return Cell::Solid;
        }
        self.cells[y as usize * self.width + x as usize]
    }

    fn floor_at(&self, x: isize, y: isize) -> bool {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return false;
        }
        self.cells[y as usize * self.width + x as usize] == Cell::Floor
    }

    fn set_floor(&mut self, x: usize, y: usize) {
        if x < self.width && y < self.height {
            self.cells[y * self.width + x] = Cell::Floor;
        }
    }

    fn set_room(&mut self, x: usize, y: usize, room: usize) {
        if x < self.width && y < self.height {
            self.room_of[y * self.width + x] = Some(room);
        }
    }

    fn aspect_at(&self, x: isize, y: isize) -> Option<RegionAspect> {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return None;
        }
        self.room_of[y as usize * self.width + x as usize].map(|i| self.aspects[i])
    }

    /// Every cell, resolved to what belongs there — row-major, same order as the grid itself.
    pub fn resolve(&self, seed: u64) -> Vec<(u8, Rotation)> {
        (0..self.height)
            .flat_map(|y| (0..self.width).map(move |x| (x, y)))
            .map(|(x, y)| self.resolve_cell(seed, x as isize, y as isize))
            .collect()
    }

    fn resolve_cell(&self, seed: u64, x: isize, y: isize) -> (u8, Rotation) {
        match self.cell_at(x, y) {
            Cell::Floor => self.resolve_floor(seed, x, y),
            Cell::SolidFront => self.resolve_wall_front(seed, x, y),
            Cell::Solid => self.resolve_wall_cap(seed, x, y),
        }
    }

    fn resolve_floor(&self, seed: u64, x: isize, y: isize) -> (u8, Rotation) {
        // Shadows
        let wall_n = !self.floor_at(x, y - 1);
        let wall_e = !self.floor_at(x + 1, y);
        let wall_ne = !self.floor_at(x + 1, y - 1);

        match (wall_n, wall_ne, wall_e) {
            (true, true, true) => (FLOOR_SHADOW_CORNER, Rotation::R0),
            (true, _, false) => (FLOOR_SHADOW_EDGE, Rotation::R0),
            (false, _, true) => (FLOOR_SHADOW_EDGE, Rotation::R90),
            (false, true, false) => (FLOOR_SHADOW_NUB, Rotation::R90),
            _ => {
                let aspect = self.aspect_at(x, y);

                match aspect {
                    Some(RegionAspect::Ruins) => choose_weighted(
                        seed,
                        x,
                        y,
                        &[(FLOOR, Rotation::R0, 10), (FLOOR_STONE, Rotation::R0, 3)],
                    ),

                    _ => choose_weighted(
                        seed,
                        x,
                        y,
                        &[(FLOOR, Rotation::R0, 10), (FLOOR_PEBBLES, Rotation::R0, 3)],
                    ),
                }
            }
        }
    }

    fn resolve_wall_front(&self, seed: u64, x: isize, y: isize) -> (u8, Rotation) {
        let wall_w = !self.floor_at(x - 1, y);
        let wall_e = !self.floor_at(x + 1, y);
        match (wall_e, wall_w) {
            (false, true) => (WALL_FILL_EAST, Rotation::R0),
            (true, false) => (WALL_FILL_WEST, Rotation::R0),
            (false, false) => (WALL_FILL_NARROW, Rotation::R0),
            (true, true) => {
                let aspect = self.aspect_at(x, y);
                match aspect {
                    Some(RegionAspect::Shrine) => choose_weighted(
                        seed,
                        x,
                        y,
                        &[
                            (WALL_FRONT, Rotation::R0, 8),
                            (WALL_FRONT_BANNER, Rotation::R0, 1),
                        ],
                    ),

                    Some(RegionAspect::Ruins) => choose_weighted(
                        seed,
                        x,
                        y,
                        &[
                            (WALL_FRONT, Rotation::R0, 8),
                            (WALL_FRONT_GRATING, Rotation::R0, 1),
                            (WALL_FRONT_VARIANT, Rotation::R0, 1),
                        ],
                    ),

                    _ => choose_weighted(
                        seed,
                        x,
                        y,
                        &[
                            (WALL_FRONT, Rotation::R0, 8),
                            (WALL_FRONT_VARIANT, Rotation::R0, 1),
                        ],
                    ),
                }
            }
        }
    }

    fn resolve_wall_cap(&self, seed: u64, x: isize, y: isize) -> (u8, Rotation) {
        let wall_n = self.cell_at(x, y - 1);
        let wall_e = self.cell_at(x + 1, y);
        let wall_s = self.cell_at(x, y + 1);
        let wall_w = self.cell_at(x - 1, y);

        let wall_ne = self.cell_at(x + 1, y - 1);
        let wall_nw = self.cell_at(x - 1, y - 1);
        let wall_se = self.cell_at(x + 1, y + 1);
        let wall_sw = self.cell_at(x - 1, y + 1);

        match (wall_n, wall_e, wall_s, wall_w) {
            (_, Cell::Solid, Cell::SolidFront, Cell::Floor | Cell::SolidFront) => {
                (WALL_CAP_NEAR_SE, Rotation::R0)
            }
            (Cell::Solid, Cell::Solid, _, Cell::Floor | Cell::SolidFront) => {
                (WALL_SIDE_EAST, Rotation::R0)
            }
            (_, Cell::Floor | Cell::SolidFront, Cell::SolidFront, Cell::Solid) => {
                (WALL_CAP_NEAR_SW, Rotation::R0)
            }
            (Cell::Solid, Cell::Floor | Cell::SolidFront, _, Cell::Solid) => {
                (WALL_SIDE_WEST, Rotation::R0)
            }
            (Cell::Floor, Cell::Floor, Cell::Solid, Cell::Solid) => {
                (WALL_CAP_FAR_EAST, Rotation::R0)
            }
            (Cell::Floor, Cell::Solid, Cell::Solid, Cell::Floor) => {
                (WALL_CAP_FAR_WEST, Rotation::R0)
            }
            (
                Cell::Floor,
                Cell::Solid | Cell::SolidFront,
                Cell::Solid,
                Cell::Solid | Cell::SolidFront,
            ) => (WALL_CAP_FAR, Rotation::R0),
            (Cell::Solid, Cell::Solid, Cell::SolidFront, Cell::Solid) => {
                (WALL_CAP_NEAR, Rotation::R0)
            }
            (Cell::Solid, Cell::Solid, Cell::Solid, Cell::Solid) if wall_se == Cell::SolidFront => {
                (WALL_CAP_NEAR_EAST, Rotation::R0)
            }
            (Cell::Solid, Cell::Solid, Cell::Solid, Cell::Solid) if wall_sw == Cell::SolidFront => {
                (WALL_CAP_NEAR_WEST, Rotation::R0)
            }
            (Cell::Solid, Cell::Solid, Cell::Solid, Cell::Solid) if wall_nw == Cell::Floor => {
                (WALL_CAP_FAR_NW, Rotation::R0)
            }
            (Cell::Solid, Cell::Solid, Cell::Solid, Cell::Solid) if wall_ne == Cell::Floor => {
                (WALL_CAP_FAR_NE, Rotation::R0)
            }
            (Cell::Floor, _, Cell::Floor, _) | (_, Cell::Floor, _, Cell::Floor) => {
                // There's no combo that works for a wall this thin, so just put in a solid block.
                (WALL_FRONT_VARIANT, Rotation::R0)
            }
            _ => choose_weighted(
                seed,
                x,
                y,
                &[
                    (WALL_FILL, Rotation::R0, 6),
                    (WALL_FILL_VARIANT, Rotation::R0, 1),
                ],
            ),
        }
    }
}

fn choose_weighted(
    seed: u64,
    x: isize,
    y: isize,
    choices: &[(u8, Rotation, usize)],
) -> (u8, Rotation) {
    let h = splitmix64(
        seed ^ (x as u64).wrapping_mul(0xBF58476D1CE4E5B9)
            ^ (y as u64).wrapping_mul(0x94D049BB133111EB)
            ^ 0xA24BAED4963EE407,
    ) as usize;

    let total = choices.iter().fold(0, |prev, (_, _, w)| prev + w);
    let mut choice = h.rem_euclid(total);
    for (tile, rotation, weight) in choices {
        if choice < *weight {
            return (*tile, *rotation);
        }
        choice -= weight;
    }
    unreachable!();
}

/// A room's aspect, stable for a given seed and room origin.
fn pick_aspect(seed: u64, rect: Rect) -> RegionAspect {
    let h = splitmix64(
        seed ^ (rect.x as u64).wrapping_mul(0xD1B54A32D192ED03)
            ^ (rect.y as u64).wrapping_mul(0x9E3779B97F4A7C15),
    );
    match h % ASPECT_CHANCE_DEN {
        0 => RegionAspect::Vault,
        1 => RegionAspect::Ruins,
        2 => RegionAspect::Shrine,
        _ => RegionAspect::Open,
    }
}

/// A rectangle of floor, in cell coordinates — a room or a passage between two rooms.
#[derive(Clone, Copy, PartialEq)]
struct Rect {
    x: usize,
    y: usize,
    w: usize,
    h: usize,
}

impl Rect {
    /// Whether this rectangle, and `other`, would leave less than `clearance` cells of solid ground
    /// between them if both stood — used to keep unrelated rooms and passages from touching.
    fn crowds(&self, other: &Rect, clearance: usize) -> bool {
        self.x < other.x + other.w + clearance
            && other.x < self.x + self.w + clearance
            && self.y < other.y + other.h + clearance
            && other.y < self.y + self.h + clearance
    }
}

/// The side of a room a [`Frontier`] entry grows away from.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Dir {
    N,
    S,
    E,
    W,
}

impl Dir {
    const ALL: [Dir; 4] = [Dir::N, Dir::S, Dir::E, Dir::W];

    fn opposite(self) -> Dir {
        match self {
            Dir::N => Dir::S,
            Dir::S => Dir::N,
            Dir::E => Dir::W,
            Dir::W => Dir::E,
        }
    }
}

/// An unclaimed `PASSAGE_WIDTH`-wide slot on a room's edge, from which a new room might grow.
struct Frontier {
    from: Rect,
    dir: Dir,
    seg_start: usize,
}

/// Every frontier slot along `rect`'s edges, skipping `exclude` (the edge a passage already
/// occupies, for a room that was itself just grown from another one).
fn edge_frontiers(rect: Rect, exclude: Option<Dir>) -> Vec<Frontier> {
    let mut out = Vec::new();
    for &dir in &Dir::ALL {
        if Some(dir) == exclude {
            continue;
        }
        let (start, end) = match dir {
            Dir::N | Dir::S => (rect.x, rect.x + rect.w),
            Dir::E | Dir::W => (rect.y, rect.y + rect.h),
        };
        let mut seg = start;
        while seg + PASSAGE_WIDTH <= end {
            out.push(Frontier {
                from: rect,
                dir,
                seg_start: seg,
            });
            seg += PASSAGE_WIDTH;
        }
    }
    out
}

/// The passage and room a frontier slot would produce, or `None` if it runs off the grid — overlap
/// with other rooms is checked separately, since that needs the full room list.
fn candidate(
    f: &Frontier,
    passage_len: usize,
    new_w: usize,
    new_h: usize,
    offset: usize,
) -> Option<(Rect, Rect)> {
    Some(match f.dir {
        Dir::N => {
            let passage_y = f.from.y.checked_sub(passage_len)?;
            let room_y = passage_y.checked_sub(new_h)?;
            let room_x = f.seg_start.checked_sub(offset)?;
            (
                Rect {
                    x: f.seg_start,
                    y: passage_y,
                    w: PASSAGE_WIDTH,
                    h: passage_len,
                },
                Rect {
                    x: room_x,
                    y: room_y,
                    w: new_w,
                    h: new_h,
                },
            )
        }
        Dir::S => {
            let passage_y = f.from.y + f.from.h;
            let room_y = passage_y + passage_len;
            let room_x = f.seg_start.checked_sub(offset)?;
            (
                Rect {
                    x: f.seg_start,
                    y: passage_y,
                    w: PASSAGE_WIDTH,
                    h: passage_len,
                },
                Rect {
                    x: room_x,
                    y: room_y,
                    w: new_w,
                    h: new_h,
                },
            )
        }
        Dir::E => {
            let passage_x = f.from.x + f.from.w;
            let room_x = passage_x + passage_len;
            let room_y = f.seg_start.checked_sub(offset)?;
            (
                Rect {
                    x: passage_x,
                    y: f.seg_start,
                    w: passage_len,
                    h: PASSAGE_WIDTH,
                },
                Rect {
                    x: room_x,
                    y: room_y,
                    w: new_w,
                    h: new_h,
                },
            )
        }
        Dir::W => {
            let passage_x = f.from.x.checked_sub(passage_len)?;
            let room_x = passage_x.checked_sub(new_w)?;
            let room_y = f.seg_start.checked_sub(offset)?;
            (
                Rect {
                    x: passage_x,
                    y: f.seg_start,
                    w: passage_len,
                    h: PASSAGE_WIDTH,
                },
                Rect {
                    x: room_x,
                    y: room_y,
                    w: new_w,
                    h: new_h,
                },
            )
        }
    })
}

fn in_bounds(r: Rect, width: usize, height: usize) -> bool {
    r.x >= BORDER && r.y >= BORDER && r.x + r.w + BORDER <= width && r.y + r.h + BORDER <= height
}

fn carve(dungeon: &mut Dungeon, rect: Rect) {
    for y in rect.y..rect.y + rect.h {
        for x in rect.x..rect.x + rect.w {
            dungeon.set_floor(x, y);
        }
    }
}

/// Carves a room (unlike a passage, a room owns its cells for `aspect_at` and gets an aspect of its
/// own), adds its edges to the frontier, and records it in `rooms`.
fn place_room(
    dungeon: &mut Dungeon,
    rooms: &mut Vec<Rect>,
    frontier: &mut Vec<Frontier>,
    seed: u64,
    rect: Rect,
    exclude: Option<Dir>,
) {
    let index = rooms.len();
    for y in rect.y..rect.y + rect.h {
        for x in rect.x..rect.x + rect.w {
            dungeon.set_floor(x, y);
            dungeon.set_room(x, y, index);
        }
    }
    dungeon.aspects.push(pick_aspect(seed, rect));
    frontier.extend(edge_frontiers(rect, exclude));
    rooms.push(rect);
}

/// If `a` and `b` sit on the same horizontal band with solid ground directly between them, the gap
/// found: (the x where it starts, its width, and the y-range where both rooms have floor).
fn horizontal_gap(a: Rect, b: Rect) -> Option<(usize, usize, usize, usize)> {
    let (left, right) = if a.x + a.w <= b.x {
        (a, b)
    } else if b.x + b.w <= a.x {
        (b, a)
    } else {
        return None;
    };
    let gap = right.x - (left.x + left.w);
    let y_lo = left.y.max(right.y);
    let y_hi = (left.y + left.h).min(right.y + right.h);
    if y_hi.saturating_sub(y_lo) < PASSAGE_WIDTH {
        return None;
    }
    Some((left.x + left.w, gap, y_lo, y_hi))
}

/// As [`horizontal_gap`], for two rooms stacked vertically.
fn vertical_gap(a: Rect, b: Rect) -> Option<(usize, usize, usize, usize)> {
    let (top, bottom) = if a.y + a.h <= b.y {
        (a, b)
    } else if b.y + b.h <= a.y {
        (b, a)
    } else {
        return None;
    };
    let gap = bottom.y - (top.y + top.h);
    let x_lo = top.x.max(bottom.x);
    let x_hi = (top.x + top.w).min(bottom.x + bottom.w);
    if x_hi.saturating_sub(x_lo) < PASSAGE_WIDTH {
        return None;
    }
    Some((top.y + top.h, gap, x_lo, x_hi))
}

/// Looks for rooms that ended up facing each other across a narrow gap and, at random, digs a
/// passage between some of them — the loops that keep this from being a single tree of rooms.
fn add_loop_connections(
    rng: &mut SplitMix64,
    dungeon: &mut Dungeon,
    rooms: &[Rect],
    passages: &mut Vec<Rect>,
) {
    for i in 0..rooms.len() {
        for j in (i + 1)..rooms.len() {
            let (a, b) = (rooms[i], rooms[j]);
            let slot = horizontal_gap(a, b)
                .filter(|&(_, gap, _, _)| (1..=MAX_LOOP_GAP).contains(&gap))
                .map(|(x, gap, y_lo, y_hi)| {
                    let span = y_hi - y_lo - PASSAGE_WIDTH;
                    let y = y_lo + (rng.next_u32() as usize) % (span + 1);
                    Rect {
                        x,
                        y,
                        w: gap,
                        h: PASSAGE_WIDTH,
                    }
                })
                .or_else(|| {
                    vertical_gap(a, b)
                        .filter(|&(_, gap, _, _)| (1..=MAX_LOOP_GAP).contains(&gap))
                        .map(|(y, gap, x_lo, x_hi)| {
                            let span = x_hi - x_lo - PASSAGE_WIDTH;
                            let x = x_lo + (rng.next_u32() as usize) % (span + 1);
                            Rect {
                                x,
                                y,
                                w: PASSAGE_WIDTH,
                                h: gap,
                            }
                        })
                });
            let Some(passage) = slot else { continue };
            if !rng.next_u32().is_multiple_of(LOOP_CHANCE_DEN) {
                continue;
            }
            let blocked = rooms
                .iter()
                .chain(passages.iter())
                .filter(|r| **r != a && **r != b)
                .any(|r| r.crowds(&passage, 1));
            if blocked {
                continue;
            }
            carve(dungeon, passage);
            passages.push(passage);
        }
    }
}

/// Grows a dungeon of rooms and wide passages outward from a single seed room, deterministic in
/// `seed`: pull a random frontier slot, try to dig a room beyond it, and either add its own edges to
/// the frontier or drop the slot for good. A second pass then adds loops between rooms that ended up
/// facing each other.
pub fn generate(seed: u64, width: usize, height: usize) -> Dungeon {
    let mut rng = SplitMix64(seed);
    let mut dungeon = Dungeon {
        width,
        height,
        cells: vec![Cell::Solid; width * height],
        room_of: vec![None; width * height],
        aspects: Vec::new(),
    };

    let mut rooms: Vec<Rect> = Vec::new();
    let mut passages: Vec<Rect> = Vec::new();
    let mut frontier: Vec<Frontier> = Vec::new();

    let w = ROOM_MIN + (rng.next_u32() as usize) % (ROOM_MAX - ROOM_MIN + 1);
    let h = ROOM_MIN + (rng.next_u32() as usize) % (ROOM_MAX - ROOM_MIN + 1);
    let x = BORDER + (rng.next_u32() as usize) % (width - 2 * BORDER - w);
    let y = BORDER + (rng.next_u32() as usize) % (height - 2 * BORDER - h);
    let first = Rect { x, y, w, h };
    place_room(&mut dungeon, &mut rooms, &mut frontier, seed, first, None);

    while !frontier.is_empty() && rooms.len() < TARGET_ROOMS {
        let idx = (rng.next_u32() as usize) % frontier.len();
        let f = frontier.swap_remove(idx);

        let passage_len =
            PASSAGE_LEN_MIN + (rng.next_u32() as usize) % (PASSAGE_LEN_MAX - PASSAGE_LEN_MIN + 1);
        let new_w = ROOM_MIN + (rng.next_u32() as usize) % (ROOM_MAX - ROOM_MIN + 1);
        let new_h = ROOM_MIN + (rng.next_u32() as usize) % (ROOM_MAX - ROOM_MIN + 1);
        let along = if matches!(f.dir, Dir::N | Dir::S) {
            new_w
        } else {
            new_h
        };
        let offset = (rng.next_u32() as usize) % (along - PASSAGE_WIDTH + 1);

        let Some((passage, room)) = candidate(&f, passage_len, new_w, new_h, offset) else {
            continue;
        };
        if !in_bounds(passage, width, height) || !in_bounds(room, width, height) {
            continue;
        }
        let blocked = rooms
            .iter()
            .chain(passages.iter())
            .filter(|r| **r != f.from)
            .any(|r| r.crowds(&passage, 1) || r.crowds(&room, 1));
        if blocked {
            continue;
        }

        carve(&mut dungeon, passage);
        passages.push(passage);
        place_room(
            &mut dungeon,
            &mut rooms,
            &mut frontier,
            seed,
            room,
            Some(f.dir.opposite()),
        );
    }

    add_loop_connections(&mut rng, &mut dungeon, &rooms, &mut passages);

    // Make all `Solid` with a floor in front of them `SolidFront`.
    for y in 0..height - 1 {
        for x in 0..width {
            let idx = y * width + x;
            if dungeon.cells[idx] == Cell::Solid && dungeon.floor_at(x as isize, y as isize + 1) {
                dungeon.cells[idx] = Cell::SolidFront;
            }
        }
    }

    dungeon
}

/// A small, dependency-free PRNG
struct SplitMix64(u64);

impl SplitMix64 {
    fn next_u32(&mut self) -> u32 {
        (splitmix64_step(&mut self.0) >> 32) as u32
    }
}

fn splitmix64_step(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E3779B97F4A7C15);
    splitmix64(*state)
}

fn splitmix64(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `#`/`.` render of a layout, for eyeballing shape during tuning — run with
    /// `cargo test --example split_friction -- --ignored --nocapture dump_layout`.
    fn ascii_art(dungeon: &Dungeon) -> String {
        let mut out = String::new();
        for y in 0..dungeon.height {
            for x in 0..dungeon.width {
                out.push(if dungeon.floor_at(x as isize, y as isize) {
                    '.'
                } else {
                    '#'
                });
            }
            out.push('\n');
        }
        out
    }

    #[test]
    #[ignore]
    fn dump_layout() {
        let seed = std::env::var("SEED")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(42);
        println!("{}", ascii_art(&generate(seed, 64, 64)));
    }

    fn floor_positions(dungeon: &Dungeon) -> Vec<(usize, usize)> {
        (0..dungeon.height)
            .flat_map(|y| (0..dungeon.width).map(move |x| (x, y)))
            .filter(|&(x, y)| dungeon.floor_at(x as isize, y as isize))
            .collect()
    }

    #[test]
    fn same_seed_same_layout() {
        let a = generate(1234, 64, 64).resolve(1234);
        let b = generate(1234, 64, 64).resolve(1234);
        assert_eq!(a, b);
    }

    #[test]
    fn different_seeds_differ() {
        let a = generate(1, 64, 64).resolve(1);
        let b = generate(2, 64, 64).resolve(2);
        assert_ne!(a, b);
    }

    #[test]
    fn every_floor_cell_is_reachable_from_every_other() {
        for seed in [0, 1, 42, 12345, u64::MAX] {
            let dungeon = generate(seed, 64, 64);
            let floors = floor_positions(&dungeon);
            assert!(!floors.is_empty(), "seed {seed} produced no floor at all");

            let mut seen = vec![false; dungeon.width * dungeon.height];
            let mut stack = vec![floors[0]];
            seen[floors[0].1 * dungeon.width + floors[0].0] = true;
            let mut visited = 0;
            while let Some((x, y)) = stack.pop() {
                visited += 1;
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let (nx, ny) = (x as isize + dx, y as isize + dy);
                    if dungeon.floor_at(nx, ny) {
                        let idx = ny as usize * dungeon.width + nx as usize;
                        if !seen[idx] {
                            seen[idx] = true;
                            stack.push((nx as usize, ny as usize));
                        }
                    }
                }
            }
            assert_eq!(
                visited,
                floors.len(),
                "seed {seed} left part of the arena unreachable"
            );
        }
    }

    #[test]
    fn directional_walls_always_stand_beside_floor() {
        // Everything except WallFill (the fallback for solid ground with no floor in reach) and the
        // far-wall caps promises a floor neighbor directly: a far-wall cap sits one cell further out
        // than that, bordering its own base or a side wall instead.
        for seed in [0, 1, 42, 12345, u64::MAX] {
            let dungeon = generate(seed, 64, 64);
            let roles = dungeon.resolve(seed);
            for y in 0..dungeon.height {
                for x in 0..dungeon.width {
                    let role = roles[y * dungeon.width + x];
                    let (x, y) = (x as isize, y as isize);
                    let touches_floor = || {
                        [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, 1)]
                            .iter()
                            .any(|&(dx, dy)| dungeon.floor_at(x + dx, y + dy))
                    };
                    let touches_base_or_side = || {
                        matches!(
                            roles[(y + 1) as usize * dungeon.width + x as usize],
                            TileRole::WallBaseFar(_)
                                | TileRole::WallSideWest
                                | TileRole::WallSideEast
                        )
                    };
                    match role {
                        TileRole::WallCapNear
                        | TileRole::WallCapNearWest
                        | TileRole::WallCapNearEast
                        | TileRole::WallBaseFar(_)
                        | TileRole::WallSideWest
                        | TileRole::WallSideEast
                        | TileRole::WallNub(_) => assert!(
                            touches_floor(),
                            "seed {seed} placed a {role:?} at ({x},{y}) touching no floor"
                        ),
                        TileRole::WallCapFar
                        | TileRole::WallCapFarWest
                        | TileRole::WallCapFarEast => {
                            assert!(
                                touches_base_or_side(),
                                "seed {seed} placed a {role:?} at ({x},{y}) with no base or side wall beneath it"
                            );
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    #[test]
    fn props_only_appear_in_their_own_aspects_rooms() {
        for seed in [0, 1, 42, 12345, u64::MAX] {
            let dungeon = generate(seed, 64, 64);
            let roles = dungeon.resolve(seed);
            for y in 0..dungeon.height {
                for x in 0..dungeon.width {
                    let TileRole::Prop(aspect) = roles[y * dungeon.width + x] else {
                        continue;
                    };
                    assert_ne!(
                        aspect,
                        RegionAspect::Open,
                        "seed {seed} rolled a prop for an open room at ({x},{y})"
                    );
                    assert_eq!(
                        dungeon.aspect_at(x as isize, y as isize),
                        Some(aspect),
                        "seed {seed} placed a prop that disagrees with its own cell's room at ({x},{y})"
                    );
                }
            }
        }
    }

    #[test]
    fn passages_are_never_one_cell_wide() {
        // A floor cell with solid directly on both sides across one axis is only ever produced by
        // a 1-cell-tall or 1-cell-wide room or passage — which ROOM_MIN and PASSAGE_WIDTH should
        // both rule out. Catches an off-by-one in the growth math sooner than a screenshot would.
        for seed in [0, 1, 42, 12345, u64::MAX] {
            let dungeon = generate(seed, 64, 64);
            for y in 0..dungeon.height {
                for x in 0..dungeon.width {
                    let (x, y) = (x as isize, y as isize);
                    if !dungeon.floor_at(x, y) {
                        continue;
                    }
                    let pinched_ns = !dungeon.floor_at(x, y - 1) && !dungeon.floor_at(x, y + 1);
                    let pinched_ew = !dungeon.floor_at(x - 1, y) && !dungeon.floor_at(x + 1, y);
                    assert!(
                        !pinched_ns,
                        "seed {seed} left a 1-cell-tall passage at ({x},{y})"
                    );
                    assert!(
                        !pinched_ew,
                        "seed {seed} left a 1-cell-wide passage at ({x},{y})"
                    );
                }
            }
        }
    }
}
