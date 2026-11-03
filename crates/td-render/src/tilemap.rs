//! Drawing the scrolling city tile map.
//!
//! Fidelity: T0 for tile pixels. The in-flight viewport geometry and camera rules are U-025 /
//! `docs/spec/sim/camera.md`; this routine takes them as parameters.

use td_formats::map::TileMap;
use td_formats::tileset::{TILE_SIZE, Tileset};

use crate::framebuffer::{Framebuffer, Image, Rect};

/// Draws the part of `map` visible through `viewport`, with the world pixel (`cam_x`, `cam_y`)
/// at the viewport's top-left corner. Cells outside the map, or tile indices outside the
/// tileset, are drawn with `fallback_color`.
pub fn draw_tile_map(
    fb: &mut Framebuffer,
    map: &TileMap,
    tiles: &Tileset,
    cam_x: i32,
    cam_y: i32,
    viewport: Rect,
    fallback_color: u8,
) {
    let viewport = viewport.intersect(Rect::SCREEN);
    if viewport.is_empty() {
        return;
    }
    let ts = TILE_SIZE as i32;
    let first_tx = cam_x.div_euclid(ts);
    let first_ty = cam_y.div_euclid(ts);
    let last_tx = (cam_x + viewport.w - 1).div_euclid(ts);
    let last_ty = (cam_y + viewport.h - 1).div_euclid(ts);
    let fallback = vec![fallback_color; TILE_SIZE * TILE_SIZE];
    for ty in first_ty..=last_ty {
        for tx in first_tx..=last_tx {
            let cell = (tx >= 0 && ty >= 0)
                .then(|| map.get(tx as usize, ty as usize))
                .flatten();
            let pixels = cell
                .and_then(|i| tiles.tiles.get(i as usize))
                .map(AsRef::as_ref)
                .unwrap_or(fallback.as_slice());
            let sx = viewport.x + tx * ts - cam_x;
            let sy = viewport.y + ty * ts - cam_y;
            fb.blit(
                Image::new(pixels, TILE_SIZE, TILE_SIZE),
                sx,
                sy,
                viewport,
                None,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrolls_with_sub_tile_offsets() {
        let mut blk = vec![0u8; 80 * 1024];
        for (i, tile) in blk.chunks_exact_mut(1024).enumerate() {
            tile.fill(i as u8);
        }
        let tiles = Tileset::parse(&blk).unwrap();
        let mut cells = vec![0u8; 201 * 201];
        cells[0] = 1; // (0, 0)
        cells[201] = 2; // (1, 0): the file is column-major
        cells[1] = 3; // (0, 1)
        let map = TileMap::parse_episode(&cells).unwrap();
        let mut fb = Framebuffer::new();
        draw_tile_map(&mut fb, &map, &tiles, 16, 16, Rect::SCREEN, 99);
        assert_eq!(fb.get(0, 0), Some(1), "world (16,16) is in tile (0,0)");
        assert_eq!(fb.get(16, 0), Some(2), "world (32,16) is in tile (1,0)");
        assert_eq!(fb.get(0, 16), Some(3), "world (16,32) is in tile (0,1)");
        draw_tile_map(&mut fb, &map, &tiles, -10, 0, Rect::SCREEN, 99);
        assert_eq!(fb.get(0, 0), Some(99), "outside the map uses the fallback");
        assert_eq!(fb.get(10, 0), Some(1));
    }
}
