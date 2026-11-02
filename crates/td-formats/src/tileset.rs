//! City tilesets: 80 tiles of 32x32 8bpp pixels, no header.
//!
//! Spec: `docs/re/FORMATS.md` §4 (CONFIRMED for `td.blk`, LIKELY for `td2.blk`/`td3.blk`).
//! Fidelity: T0. `td2.blk` and `td3.blk` carry 864 trailing bytes of unknown purpose (U-013),
//! which are preserved verbatim in [`Tileset::trailing`].

use crate::{FormatError, Result};

/// Tile edge length in pixels.
pub const TILE_SIZE: usize = 32;
/// Bytes per tile.
pub const TILE_BYTES: usize = TILE_SIZE * TILE_SIZE;
/// Tiles per tileset.
pub const TILE_COUNT: usize = 80;

/// A city tileset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tileset {
    /// 80 tiles, each 32x32 row-major palette indices.
    pub tiles: Vec<Box<[u8]>>,
    /// Bytes after the 80th tile (empty for `td.blk`, 864 bytes for `td2.blk`/`td3.blk`).
    pub trailing: Vec<u8>,
}

impl Tileset {
    /// Parses a tileset of at least 80 tiles.
    pub fn parse(data: &[u8]) -> Result<Self> {
        let need = TILE_COUNT * TILE_BYTES;
        if data.len() < need {
            return Err(FormatError::Truncated {
                format: "tileset",
                offset: data.len(),
                needed: need - data.len(),
            });
        }
        Ok(Self {
            tiles: data[..need]
                .chunks_exact(TILE_BYTES)
                .map(Into::into)
                .collect(),
            trailing: data[need..].to_vec(),
        })
    }

    /// Pixel (x, y) of tile `index`, or `None` if out of range.
    pub fn pixel(&self, index: usize, x: usize, y: usize) -> Option<u8> {
        let tile = self.tiles.get(index)?;
        (x < TILE_SIZE && y < TILE_SIZE).then(|| tile[y * TILE_SIZE + x])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_tiles_and_keeps_trailing_bytes() {
        let mut data = vec![0u8; TILE_COUNT * TILE_BYTES + 3];
        data[TILE_BYTES + 33] = 9; // tile 1, (1, 1)
        data[TILE_COUNT * TILE_BYTES..].copy_from_slice(&[1, 2, 3]);
        let t = Tileset::parse(&data).unwrap();
        assert_eq!(t.tiles.len(), 80);
        assert_eq!(t.pixel(1, 1, 1), Some(9));
        assert_eq!(t.pixel(80, 0, 0), None);
        assert_eq!(t.trailing, [1, 2, 3]);
        assert!(Tileset::parse(&data[..1000]).is_err());
    }
}
