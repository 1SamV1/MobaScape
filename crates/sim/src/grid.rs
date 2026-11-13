#[derive(Debug, Clone)]
pub struct Grid {
    width: i32,
    height: i32,
    blocked: Vec<bool>,
}

impl Grid {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width: width as i32,
            height: height as i32,
            blocked: vec![false; width * height],
        }
    }

    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn height(&self) -> i32 {
        self.height
    }

    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.width && y < self.height
    }

    pub fn is_blocked(&self, x: i32, y: i32) -> bool {
        self.in_bounds(x, y) && self.blocked[(y * self.width + x) as usize]
    }

    pub fn is_walkable(&self, x: i32, y: i32) -> bool {
        self.in_bounds(x, y) && !self.blocked[(y * self.width + x) as usize]
    }

    pub fn set_blocked(&mut self, x: i32, y: i32, blocked: bool) {
        if self.in_bounds(x, y) {
            self.blocked[(y * self.width + x) as usize] = blocked;
        }
    }

    pub fn block_rect(&mut self, x: i32, y: i32, width: i32, height: i32) {
        for yy in y..y + height {
            for xx in x..x + width {
                self.set_blocked(xx, yy, true);
            }
        }
    }

    pub fn nearest_walkable(&self, x: i32, y: i32, max_radius: i32) -> Option<(i32, i32)> {
        if self.is_walkable(x, y) {
            return Some((x, y));
        }
        for radius in 1..=max_radius {
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    if dx.abs() != radius && dy.abs() != radius {
                        continue;
                    }
                    let nx = x + dx;
                    let ny = y + dy;
                    if self.is_walkable(nx, ny) {
                        return Some((nx, ny));
                    }
                }
            }
        }
        None
    }

    /// Bresenham line of sight; returns false if any intermediate tile is
    /// blocked. Used to simplify A* paths and for hold-to-steer checks.
    pub fn line_of_sight(&self, a: (i32, i32), b: (i32, i32)) -> bool {
        let (mut x0, mut y0) = a;
        let (x1, y1) = b;
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        loop {
            if !self.is_walkable(x0, y0) {
                return false;
            }
            if x0 == x1 && y0 == y1 {
                return true;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_and_blocking() {
        let mut grid = Grid::new(8, 8);
        assert!(grid.is_walkable(0, 0));
        grid.set_blocked(0, 0, true);
        assert!(!grid.is_walkable(0, 0));
        assert!(!grid.is_walkable(-1, 0));
        assert!(!grid.is_walkable(8, 0));
        assert_eq!(grid.nearest_walkable(0, 0, 3), Some((1, 0)));
    }

    #[test]
    fn line_of_sight_detects_walls() {
        let mut grid = Grid::new(8, 8);
        assert!(grid.line_of_sight((0, 0), (7, 7)));
        for y in 0..8 {
            grid.set_blocked(4, y, true);
        }
        assert!(!grid.line_of_sight((0, 0), (7, 7)));
    }
}
