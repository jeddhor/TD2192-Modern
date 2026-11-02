//! City maps: a square grid of one-byte tile indices, row-major.
//!
//! Spec: `docs/re/FORMATS.md` §5 (CONFIRMED for `td.map`). Fidelity: T0.
//! Episode maps (`td.map`, `td2.map`, `td3.map`, `td4.map`) are 201x201; `intro.map` is 200x200.

use crate::{FormatError, Result};

/// Edge length of an episode map in tiles.
pub const EPISODE_MAP_SIZE: usize = 201;
/// Edge length of the intro map in tiles.
pub const INTRO_MAP_SIZE: usize = 200;

/// A square tile map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileMap {
    size: usize,
    cells: Vec<u8>,
}

impl TileMap {
    /// Parses a 201x201 episode map.
    pub fn parse_episode(data: &[u8]) -> Result<Self> {
        Self::parse_square(data, EPISODE_MAP_SIZE)
    }

    /// Parses the 200x200 intro map.
    pub fn parse_intro(data: &[u8]) -> Result<Self> {
        Self::parse_square(data, INTRO_MAP_SIZE)
    }

    /// Parses a square map whose edge length is inferred from the data length.
    pub fn parse_any(data: &[u8]) -> Result<Self> {
        let size = data.len().isqrt();
        if size == 0 || size * size != data.len() {
            return Err(FormatError::Invalid {
                format: "map",
                offset: 0,
                detail: format!("length {} is not a square", data.len()),
            });
        }
        Self::parse_square(data, size)
    }

    fn parse_square(data: &[u8], size: usize) -> Result<Self> {
        FormatError::check_size("map", data, size * size)?;
        Ok(Self {
            size,
            cells: data.to_vec(),
        })
    }

    /// Edge length in tiles.
    pub fn size(&self) -> usize {
        self.size
    }

    /// Tile index at (x, y), or `None` outside the map.
    pub fn get(&self, x: usize, y: usize) -> Option<u8> {
        (x < self.size && y < self.size).then(|| self.cells[y * self.size + x])
    }

    /// All cells, row-major.
    pub fn cells(&self) -> &[u8] {
        &self.cells
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_row_major() {
        let mut data = vec![0u8; 201 * 201];
        data[201 * 3 + 5] = 42;
        let m = TileMap::parse_episode(&data).unwrap();
        assert_eq!(m.size(), 201);
        assert_eq!(m.get(5, 3), Some(42));
        assert_eq!(m.get(201, 0), None);
        assert!(TileMap::parse_intro(&data).is_err());
        assert_eq!(TileMap::parse_any(&data).unwrap().size(), 201);
        assert!(TileMap::parse_any(&data[..1000]).is_err());
        assert!(TileMap::parse_any(&[]).is_err());
    }
}
