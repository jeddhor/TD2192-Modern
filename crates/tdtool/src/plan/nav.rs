//! Distances on the tile grid for the planner's heuristic.
//!
//! The player's ship sits on the cell (viewX + 5, viewY + 3) of the map when its view offsets
//! are at rest (flight-model §4.3: the collision tests look at the cells around it). A
//! breadth-first search over open cells (`World::solid` false) from goal cells gives every cell
//! its distance in 4-neighbour steps; solid cells and cells outside the map stay unreachable.

use std::collections::VecDeque;

use td_sim::World;

/// Map size in cells per axis.
pub const MAP: usize = 201;
/// Distance of an unreachable cell.
pub const FAR: u16 = u16::MAX;

/// Distances to the nearest of some goal cells.
#[derive(Debug, Clone)]
pub struct Field {
    dist: Vec<u16>,
}

fn inside(x: i16, y: i16) -> bool {
    (0..MAP as i16).contains(&x) && (0..MAP as i16).contains(&y)
}

fn idx(x: i16, y: i16) -> usize {
    x as usize * MAP + y as usize
}

impl Field {
    /// Breadth-first distances from `goal` over open cells.
    pub fn new(world: &World, goal: (i16, i16)) -> Self {
        Self::from_cells(world, &[goal])
    }

    /// Breadth-first distances from the nearest of `goals` over open cells. A solid goal cell
    /// is still a source, so ships parked on odd tiles stay reachable.
    pub fn from_cells(world: &World, goals: &[(i16, i16)]) -> Self {
        let mut dist = vec![FAR; MAP * MAP];
        let mut q = VecDeque::new();
        for &(x, y) in goals.iter().filter(|&&(x, y)| inside(x, y)) {
            dist[idx(x, y)] = 0;
            q.push_back((x, y));
        }
        while let Some((x, y)) = q.pop_front() {
            let d = dist[idx(x, y)];
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (x + dx, y + dy);
                if !inside(nx, ny) || world.solid(nx, ny) || dist[idx(nx, ny)] != FAR {
                    continue;
                }
                dist[idx(nx, ny)] = d + 1;
                q.push_back((nx, ny));
            }
        }
        Self { dist }
    }

    /// Distance of cell (x, y), [`FAR`] if unreachable or outside the map.
    pub fn at(&self, x: i16, y: i16) -> u16 {
        if !inside(x, y) {
            return FAR;
        }
        self.dist[idx(x, y)]
    }
}
