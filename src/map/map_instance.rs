use crate::map::block::{Block, BLOCK_SIZE};

pub struct MapInstance {
    pub name: String,
    pub xs: i16,
    pub ys: i16,
    pub cells: Vec<u8>,
    pub blocks: Vec<Block>,
    pub b_xs: i16,
    pub b_ys: i16,
}

impl MapInstance {
    pub fn new(name: String, xs: i16, ys: i16, cells: Vec<u8>) -> Self {
        let b_xs = (xs + BLOCK_SIZE - 1) / BLOCK_SIZE;
        let b_ys = (ys + BLOCK_SIZE - 1) / BLOCK_SIZE;
        let num_blocks = (b_xs as usize) * (b_ys as usize);
        
        let mut blocks = Vec::with_capacity(num_blocks);
        for _ in 0..num_blocks {
            blocks.push(Block::new());
        }

        Self { name, xs, ys, cells, blocks, b_xs, b_ys }
    }

    /// Gets the block index for a given (x, y) coordinate
    pub fn get_block_index(&self, x: i16, y: i16) -> Option<usize> {
        if x >= 0 && x < self.xs && y >= 0 && y < self.ys {
            let bx = x / BLOCK_SIZE;
            let by = y / BLOCK_SIZE;
            Some((bx as usize) + (by as usize) * (self.b_xs as usize))
        } else {
            None
        }
    }

    /// Returns the raw cell type at (x, y) if within bounds.
    pub fn get_cell(&self, x: i16, y: i16) -> Option<u8> {
        if x >= 0 && x < self.xs && y >= 0 && y < self.ys {
            let index = (x as usize) + (y as usize) * (self.xs as usize);
            Some(self.cells[index])
        } else {
            None
        }
    }

    /// Checks if a cell is walkable based on rAthena gat rules.
    /// Types: 0 = Walkable/Shootable, 1 = Non-walkable/Non-shootable
    /// 3 = Walkable/Shootable (water), 5 = Non-walkable/Shootable
    pub fn is_walkable(&self, x: i16, y: i16) -> bool {
        match self.get_cell(x, y) {
            Some(0) | Some(3) => true,
            _ => false,
        }
    }

    pub fn is_shootable(&self, x: i16, y: i16) -> bool {
        match self.get_cell(x, y) {
            Some(0) | Some(3) | Some(5) => true,
            _ => false,
        }
    }

    /// Basic Bresenham's Line of Sight algorithm
    pub fn has_los(&self, x0: i16, y0: i16, x1: i16, y1: i16) -> bool {
        let mut x = x0;
        let mut y = y0;
        let dx = (x1 - x0).abs();
        let dy = (y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx - dy;

        while x != x1 || y != y1 {
            if !self.is_shootable(x, y) {
                return false;
            }
            let e2 = 2 * err;
            if e2 > -dy {
                err -= dy;
                x += sx;
            }
            if e2 < dx {
                err += dx;
                y += sy;
            }
        }
        self.is_shootable(x1, y1)
    }
}
