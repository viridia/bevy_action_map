//! A seeded arena layout, and what each cell needs drawn over it.
//!
//! No Bevy here, deliberately: a layout is a pure function of its seed, so it can be checked with
//! plain unit tests — same seed in, same grid out, every open cell reachable from every other one —
//! faster and more thoroughly than watching a window across fifty seeds ever could.
//!
//! The shape is Gauntlet's: one open floor filling almost the whole grid, with a handful of solid
//! obstacle blocks punched back out of it, each kept clear of the others and of the outer wall by at
//! least [`CLEARANCE`] cells of open floor. [`resolve`] then looks at each cell's neighbors to decide
//! what belongs there: a wall cap or side where open floor borders a solid cell in the right
//! direction, and a plain filled wall anywhere solid that isn't — so every cell draws *something*,
//! and nothing reads as a hole in the world.

/// Minimum open floor width between two obstacles, or between an obstacle and the outer wall.
const CLEARANCE: usize = 2;

/// One cell of the layout, before it has been resolved to anything drawable.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Cell {
    Solid,
    Floor,
}

/// What a resolved cell should look like, independent of any particular tileset.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TileRole {
    /// Open floor, plain or one of a handful of decorative variants (0 is always plain).
    Floor(u8),
    /// Floor immediately south of a north wall, shaded to sit beneath it.
    FloorShadow,
    WallTop,
    WallTopLeft,
    WallTopRight,
    WallSideLeft,
    WallSideRight,
    /// Solid with no floor in a direction any of the above pieces render — the obstacle's own
    /// interior, and any solid cell too far from floor to need a specific piece.
    WallFill,
}

pub struct Dungeon {
    pub width: usize,
    pub height: usize,
    cells: Vec<Cell>,
}

impl Dungeon {
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

    fn set_solid(&mut self, x: usize, y: usize) {
        if x < self.width && y < self.height {
            self.cells[y * self.width + x] = Cell::Solid;
        }
    }

    /// Every cell, resolved to what belongs there — row-major, same order as the grid itself.
    pub fn resolve(&self, seed: u64) -> Vec<TileRole> {
        (0..self.height)
            .flat_map(|y| (0..self.width).map(move |x| (x, y)))
            .map(|(x, y)| self.resolve_cell(seed, x as isize, y as isize))
            .collect()
    }

    fn resolve_cell(&self, seed: u64, x: isize, y: isize) -> TileRole {
        if self.floor_at(x, y) {
            return if !self.floor_at(x, y - 1) {
                TileRole::FloorShadow
            } else {
                TileRole::Floor(decorate(seed, x, y))
            };
        }

        let s = self.floor_at(x, y + 1);
        let e = self.floor_at(x + 1, y);
        let w = self.floor_at(x - 1, y);
        let se = self.floor_at(x + 1, y + 1);
        let sw = self.floor_at(x - 1, y + 1);

        if se && !s && !e {
            TileRole::WallTopLeft
        } else if sw && !s && !w {
            TileRole::WallTopRight
        } else if s {
            TileRole::WallTop
        } else if e {
            TileRole::WallSideLeft
        } else if w {
            TileRole::WallSideRight
        } else {
            TileRole::WallFill
        }
    }
}

/// A pick from 0..6 for which floor variant to scatter in, stable for a given cell and seed.
fn decorate(seed: u64, x: isize, y: isize) -> u8 {
    let h = splitmix64(
        seed ^ (x as u64).wrapping_mul(0x9E3779B97F4A7C15)
            ^ (y as u64).wrapping_mul(0xC2B2AE3D27D4EB4F),
    );
    // Five in six plain, so variants read as texture rather than a checkerboard.
    if h.is_multiple_of(6) {
        (h % 5) as u8 + 1
    } else {
        0
    }
}

/// A rectangle of solid ground, in cell coordinates.
#[derive(Clone, Copy)]
struct Rect {
    x: usize,
    y: usize,
    w: usize,
    h: usize,
}

impl Rect {
    /// Whether this rectangle, and `other`, would leave less than `clearance` cells of floor
    /// between them if both stood.
    fn crowds(&self, other: &Rect, clearance: usize) -> bool {
        self.x < other.x + other.w + clearance
            && other.x < self.x + self.w + clearance
            && self.y < other.y + other.h + clearance
            && other.y < self.y + self.h + clearance
    }
}

/// Carves a Gauntlet-sized arena: one open floor with a handful of solid obstacles punched out of
/// it, deterministic in `seed`.
pub fn generate(seed: u64, width: usize, height: usize) -> Dungeon {
    let mut rng = SplitMix64(seed);
    let border = 1;
    let mut dungeon = Dungeon {
        width,
        height,
        cells: vec![Cell::Solid; width * height],
    };

    for y in border..height - border {
        for x in border..width - border {
            dungeon.set_floor(x, y);
        }
    }

    let target_obstacles = 9;
    let lo = border + CLEARANCE;
    let mut obstacles: Vec<Rect> = Vec::new();
    for _ in 0..target_obstacles * 20 {
        if obstacles.len() >= target_obstacles {
            break;
        }
        let w = 3 + (rng.next_u32() % 4) as usize;
        let h = 3 + (rng.next_u32() % 4) as usize;
        let Some(x_span) = (width.saturating_sub(lo + CLEARANCE + w)).checked_sub(lo) else {
            continue;
        };
        let Some(y_span) = (height.saturating_sub(lo + CLEARANCE + h)).checked_sub(lo) else {
            continue;
        };
        if x_span == 0 || y_span == 0 {
            continue;
        }
        let x = lo + (rng.next_u32() as usize) % x_span;
        let y = lo + (rng.next_u32() as usize) % y_span;
        let candidate = Rect { x, y, w, h };
        if obstacles.iter().any(|o| o.crowds(&candidate, CLEARANCE)) {
            continue;
        }
        obstacles.push(candidate);
    }

    for obstacle in &obstacles {
        for y in obstacle.y..obstacle.y + obstacle.h {
            for x in obstacle.x..obstacle.x + obstacle.w {
                dungeon.set_solid(x, y);
            }
        }
    }

    dungeon
}

/// A small, dependency-free PRNG — deterministic and good enough for obstacle placement, not
/// cryptography.
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
        for seed in [0, 1, 42, 12345, u64::MAX] {
            let dungeon = generate(seed, 64, 64);
            let roles = dungeon.resolve(seed);
            for y in 0..dungeon.height {
                for x in 0..dungeon.width {
                    let role = roles[y * dungeon.width + x];
                    // WallFill is deliberately the fallback for solid ground with no floor in
                    // reach — only the directional pieces promise a floor neighbor.
                    let is_directional = matches!(
                        role,
                        TileRole::WallTop
                            | TileRole::WallTopLeft
                            | TileRole::WallTopRight
                            | TileRole::WallSideLeft
                            | TileRole::WallSideRight
                    );
                    if !is_directional {
                        continue;
                    }
                    let (x, y) = (x as isize, y as isize);
                    let has_floor_neighbor = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, 1)]
                        .iter()
                        .any(|&(dx, dy)| dungeon.floor_at(x + dx, y + dy));
                    assert!(
                        has_floor_neighbor,
                        "seed {seed} placed a directional wall at ({x},{y}) touching no floor"
                    );
                }
            }
        }
    }

    #[test]
    fn obstacles_keep_their_clearance() {
        for seed in [0, 1, 42, 12345, u64::MAX] {
            let dungeon = generate(seed, 64, 64);
            // A cheap proxy for "every gap is at least CLEARANCE wide": scan every row and column
            // for a run of solid cells shorter than CLEARANCE sitting between two floor cells,
            // which is exactly what a too-narrow gap between obstacles would leave behind.
            for y in 0..dungeon.height {
                let mut run = 0;
                for x in 0..dungeon.width {
                    if dungeon.floor_at(x as isize, y as isize) {
                        run = 0;
                    } else {
                        run += 1;
                        let before = x >= run && dungeon.floor_at((x - run) as isize, y as isize);
                        let after = dungeon.floor_at(x as isize + 1, y as isize);
                        assert!(
                            !(before && after && run < CLEARANCE),
                            "seed {seed} left a gap narrower than {CLEARANCE} at row {y}, col {x}"
                        );
                    }
                }
            }
        }
    }
}
