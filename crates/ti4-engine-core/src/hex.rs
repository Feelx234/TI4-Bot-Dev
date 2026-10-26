//! Axial hex coordinates (the same arithmetic as `ti4_model::hex`, without serde) and a dense grid
//! from hex to system.

use crate::ids::SystemIx;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Hex {
    pub q: i32,
    pub r: i32,
}

/// Axial directions, clockwise from east (`ti4_model::hex::DIRECTIONS`).
pub const DIRECTIONS: [(i32, i32); 6] = [(1, 0), (1, -1), (0, -1), (-1, 0), (-1, 1), (0, 1)];
/// Ring-1 direction of each hyperlane edge (`ti4_content::hyperlane::EDGE_DIRS`).
pub const EDGE_DIRS: [(i32, i32); 6] = [(-1, 1), (0, 1), (1, 0), (1, -1), (0, -1), (-1, 0)];

impl Hex {
    #[must_use]
    pub const fn new(q: i32, r: i32) -> Self {
        Self { q, r }
    }

    #[must_use]
    pub fn neighbours(self) -> [Self; 6] {
        DIRECTIONS.map(|(dq, dr)| Self::new(self.q + dq, self.r + dr))
    }

    #[must_use]
    pub const fn edge(self, edge: u8) -> Self {
        let (dq, dr) = EDGE_DIRS[edge as usize];
        Self::new(self.q + dq, self.r + dr)
    }
}

/// Hexes with `|q|, |r| <= GRID_RADIUS` are stored densely (an 8-player map has 4 rings; the
/// margin covers every template). A placement outside it is refused by [`Grid::set`].
pub const GRID_RADIUS: i32 = 12;
const SIDE: usize = (2 * GRID_RADIUS + 1) as usize;
const NONE: u16 = u16::MAX;

/// Hex -> system, dense.
#[derive(Clone, PartialEq, Eq)]
pub struct Grid {
    cells: Box<[u16; SIDE * SIDE]>,
}

impl std::fmt::Debug for Grid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Grid")
    }
}

impl Default for Grid {
    fn default() -> Self {
        Self {
            cells: Box::new([NONE; SIDE * SIDE]),
        }
    }
}

impl Grid {
    fn slot(hex: Hex) -> Option<usize> {
        let (q, r) = (hex.q + GRID_RADIUS, hex.r + GRID_RADIUS);
        let side = 2 * GRID_RADIUS + 1;
        ((0..side).contains(&q) && (0..side).contains(&r)).then(|| (q * side + r) as usize)
    }

    #[inline]
    #[must_use]
    pub fn get(&self, hex: Hex) -> Option<SystemIx> {
        let v = self.cells[Self::slot(hex)?];
        (v != NONE).then(|| crate::ids::system_at(usize::from(v)))
    }

    /// Place; `false` when the hex is outside the grid.
    pub fn set(&mut self, hex: Hex, system: Option<SystemIx>) -> bool {
        let Some(slot) = Self::slot(hex) else {
            return false;
        };
        self.cells[slot] = system.map_or(NONE, |s| s as u16);
        true
    }
}
