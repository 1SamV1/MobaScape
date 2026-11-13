use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::grid::Grid;

const STRAIGHT: i32 = 10;
const DIAGONAL: i32 = 14;

/// A* on an 8-connected grid. Diagonals may not cut through blocked corners.
/// Returns tile coordinates including the start and goal.
pub fn find_path(grid: &Grid, start: (i32, i32), goal: (i32, i32)) -> Option<Vec<(i32, i32)>> {
    let start = grid.nearest_walkable(start.0, start.1, 8)?;
    let goal = grid.nearest_walkable(goal.0, goal.1, 8)?;
    if start == goal {
        return Some(vec![start]);
    }

    let width = grid.width() as usize;
    let height = grid.height() as usize;
    let total = width * height;
    let index = |x: i32, y: i32| y as usize * width + x as usize;

    let mut g_score = vec![i32::MAX; total];
    let mut parent = vec![(-1i32, -1i32); total];
    let mut closed = vec![false; total];
    let mut open = BinaryHeap::new();

    g_score[index(start.0, start.1)] = 0;
    open.push((Reverse(heuristic(start, goal)), start.0, start.1));

    while let Some((Reverse(_), x, y)) = open.pop() {
        let current = (x, y);
        let ci = index(x, y);
        if closed[ci] {
            continue;
        }
        closed[ci] = true;

        if current == goal {
            return Some(reconstruct(&parent, width, start, goal));
        }

        for (dx, dy) in [
            (-1, 0),
            (1, 0),
            (0, -1),
            (0, 1),
            (-1, -1),
            (1, -1),
            (-1, 1),
            (1, 1),
        ] {
            let nx = x + dx;
            let ny = y + dy;
            if !grid.is_walkable(nx, ny) {
                continue;
            }
            if dx != 0 && dy != 0 {
                // Do not cut through a blocked corner.
                if !grid.is_walkable(x + dx, y) || !grid.is_walkable(x, y + dy) {
                    continue;
                }
            }

            let step = if dx != 0 && dy != 0 {
                DIAGONAL
            } else {
                STRAIGHT
            };
            let tentative = g_score[ci].saturating_add(step);
            let ni = index(nx, ny);
            if tentative < g_score[ni] {
                g_score[ni] = tentative;
                parent[ni] = current;
                let f = tentative.saturating_add(heuristic((nx, ny), goal));
                open.push((Reverse(f), nx, ny));
            }
        }
    }

    None
}

fn heuristic(a: (i32, i32), b: (i32, i32)) -> i32 {
    let dx = (a.0 - b.0).abs();
    let dy = (a.1 - b.1).abs();
    STRAIGHT * dx.max(dy) + (DIAGONAL - STRAIGHT) * dx.min(dy)
}

fn reconstruct(
    parent: &[(i32, i32)],
    width: usize,
    start: (i32, i32),
    goal: (i32, i32),
) -> Vec<(i32, i32)> {
    let mut path = Vec::new();
    let mut current = goal;
    loop {
        path.push(current);
        if current == start {
            break;
        }
        let p = parent[current.1 as usize * width + current.0 as usize];
        if p == (-1, -1) {
            break;
        }
        current = p;
    }
    path.reverse();
    path
}

/// Remove redundant waypoints when a straight line is walkable.
pub fn simplify_path(grid: &Grid, path: &[(i32, i32)]) -> Vec<(i32, i32)> {
    if path.len() <= 2 {
        return path.to_vec();
    }
    let mut out = Vec::new();
    let mut anchor = 0usize;
    out.push(path[0]);
    while anchor + 1 < path.len() {
        let mut best = anchor + 1;
        for candidate in anchor + 2..path.len() {
            if grid.line_of_sight(path[anchor], path[candidate]) {
                best = candidate;
            } else {
                break;
            }
        }
        out.push(path[best]);
        anchor = best;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_straight_path() {
        let grid = Grid::new(10, 10);
        let path = find_path(&grid, (1, 1), (8, 8)).unwrap();
        assert_eq!(path.first(), Some(&(1, 1)));
        assert_eq!(path.last(), Some(&(8, 8)));
        assert!(path.len() > 2);
    }

    #[test]
    fn routes_around_wall() {
        let mut grid = Grid::new(12, 12);
        for y in 0..10 {
            grid.set_blocked(6, y, true);
        }
        let path = find_path(&grid, (1, 5), (10, 5)).unwrap();
        assert!(path.iter().any(|&(x, y)| x == 6 && y == 10));
        assert_eq!(path.last(), Some(&(10, 5)));
    }

    #[test]
    fn finds_nearest_walkable_goal() {
        let mut grid = Grid::new(8, 8);
        grid.set_blocked(5, 5, true);
        let path = find_path(&grid, (1, 1), (5, 5)).unwrap();
        let end = path.last().copied().unwrap();
        assert!(grid.is_walkable(end.0, end.1));
        assert_ne!(end, (5, 5));
    }

    #[test]
    fn does_not_cut_blocked_corners() {
        let mut grid = Grid::new(6, 6);
        grid.set_blocked(2, 1, true);
        grid.set_blocked(1, 2, true);
        let path = find_path(&grid, (1, 1), (2, 2)).unwrap();
        // The direct diagonal is forbidden; the path must go around.
        assert!(path.len() > 2);
        assert!(path.iter().all(|&(x, y)| grid.is_walkable(x, y)));
    }

    #[test]
    fn simplifies_line() {
        let grid = Grid::new(10, 10);
        let path = vec![(0, 0), (1, 0), (2, 0), (3, 0)];
        let simplified = simplify_path(&grid, &path);
        assert_eq!(simplified, vec![(0, 0), (3, 0)]);
    }
}
