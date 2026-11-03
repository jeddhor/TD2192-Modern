//! The in-flight picture: scrolling playfield (rows 0..160) and HUD bar (rows 160..192).
//!
//! Spec: `docs/spec/sim/camera-viewport.md`, `docs/spec/render/hud.md`, tick-order step 12
//! (overpass redraw), and the Verifier's composition check (`docs/re/dynamic-observations.md`
//! §13, T0 against C-0002 f3500/f3543). The original draws an 11 × 6 tile window into a page
//! and scrolls with the CRTC start address: horizontally in 4-pixel steps, vertically per
//! pixel. The ship stays at a fixed screen position; the world moves.

use td_formats::map::TileMap;
use td_formats::tileset::{TILE_SIZE, Tileset};

use crate::framebuffer::{Framebuffer, Image, Rect, WIDTH};

/// The scrolling playfield.
pub const PLAYFIELD: Rect = Rect {
    x: 0,
    y: 0,
    w: 320,
    h: 160,
};
/// First screen row of the HUD bar (`td.win`).
pub const HUD_Y: i32 = 160;
/// HUD bar size.
pub const HUD_W: usize = 320;
pub const HUD_H: usize = 32;
/// Screen position of a 32 × 32 ship frame's top-left corner.
pub const SHIP_POS: (i32, i32) = (144, 68);
/// Map tiles redrawn over the ships each tick (overpasses).
pub const OVERPASS_TILES: [u8; 4] = [0x42, 0x44, 0x45, 0x46];

/// The view as the simulation stores it: a map tile plus a pixel offset per axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewPos {
    pub tile_x: i32,
    pub tile_y: i32,
    pub sub_x: i32,
    pub sub_y: i32,
}

impl ViewPos {
    /// Page pixel shown at the playfield's top-left corner: `4·Round(subX / 4) − 16`
    /// (Round halves away from zero) and `subY − 16`.
    pub fn page_origin(&self) -> (i32, i32) {
        (4 * (self.sub_x + 2).div_euclid(4) - 16, self.sub_y - 16)
    }
}

fn tile_at(map: &TileMap, view: ViewPos, i: i32, j: i32) -> Option<u8> {
    let (x, y) = (view.tile_x + i, view.tile_y + j);
    map.get(usize::try_from(x).ok()?, usize::try_from(y).ok()?)
}

/// Draws one window tile (column `i`, row `j` of the 11 × 6 window) opaquely, clipped to the
/// playfield.
fn draw_window_tile(
    fb: &mut Framebuffer,
    map: &TileMap,
    tiles: &Tileset,
    view: ViewPos,
    i: i32,
    j: i32,
) {
    let Some(tile) = tile_at(map, view, i, j).and_then(|t| tiles.tiles.get(usize::from(t))) else {
        return;
    };
    let (ox, oy) = view.page_origin();
    let size = TILE_SIZE as i32;
    let img = Image::new(tile, TILE_SIZE, TILE_SIZE);
    fb.blit(img, i * size - ox, j * size - oy, PLAYFIELD, None);
}

/// Draws the tile window for `view` into the playfield.
pub fn draw_tile_window(fb: &mut Framebuffer, map: &TileMap, tiles: &Tileset, view: ViewPos) {
    for i in 0..11 {
        for j in 0..6 {
            draw_window_tile(fb, map, tiles, view, i, j);
        }
    }
}

/// Redraws the overpass tiles of window columns 3..=6, rows 1..=4 over the ships (tick-order
/// step 12): tiles 42h, 44h, 45h, 46h, and every cell with map x 7Ah..=7Fh at map y 47h.
pub fn redraw_overpasses(fb: &mut Framebuffer, map: &TileMap, tiles: &Tileset, view: ViewPos) {
    for i in 3..=6 {
        for j in 1..=4 {
            let special = (0x7A..=0x7F).contains(&(view.tile_x + i)) && view.tile_y + j == 0x47;
            let overpass = tile_at(map, view, i, j).is_some_and(|t| OVERPASS_TILES.contains(&t));
            if special || overpass {
                draw_window_tile(fb, map, tiles, view, i, j);
            }
        }
    }
}

/// Draws a ship frame (colour 0 transparent) at the player's screen position. `frame` is a
/// square frame of `size` pixels centred like a 32 × 32 base frame (40 × 40 heading frames
/// have the base frame at offset (4, 4)).
pub fn draw_player_ship(fb: &mut Framebuffer, frame: &[u8], size: usize) {
    let inset = (size as i32 - 32) / 2;
    fb.blit(
        Image::new(frame, size, size),
        SHIP_POS.0 - inset,
        SHIP_POS.1 - inset,
        PLAYFIELD,
        Some(0),
    );
}

/// Copies the 320 × 32 HUD bar (`td.win`) to rows 160..192.
///
/// # Panics
/// If `win` is not 320 × 32 bytes.
pub fn draw_hud_bar(fb: &mut Framebuffer, win: &[u8]) {
    assert_eq!(win.len(), HUD_W * HUD_H, "td.win is 320x32");
    let start = HUD_Y as usize * WIDTH;
    fb.pixels[start..start + win.len()].copy_from_slice(win);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horizontal_scroll_is_quantised_to_four_pixels() {
        let v = |sub_x| ViewPos {
            tile_x: 0,
            tile_y: 0,
            sub_x,
            sub_y: 16,
        };
        assert_eq!(v(32).page_origin(), (16, 0));
        assert_eq!(v(17).page_origin(), (0, 0));
        assert_eq!(v(18).page_origin(), (4, 0), "Round(4.5) = 5");
        assert_eq!(v(21).page_origin(), (4, 0));
        assert_eq!(v(48).page_origin(), (32, 0));
    }

    #[test]
    fn window_and_overpass_layout() {
        // Tile t is filled with colour t; the map holds tile 1 everywhere except an overpass
        // (0x42) at window cell (5, 2).
        let mut blk = vec![0u8; 80 * 1024];
        for t in 0..80 {
            blk[t * 1024..(t + 1) * 1024].fill(t as u8);
        }
        let tiles = Tileset::parse(&blk).unwrap();
        let mut cells = vec![1u8; 201 * 201];
        let view = ViewPos {
            tile_x: 102,
            tile_y: 104,
            sub_x: 32,
            sub_y: 16,
        };
        cells[107 * 201 + 106] = 0x42;
        let map = TileMap::parse_episode(&cells).unwrap();
        let mut fb = Framebuffer::new();
        draw_tile_window(&mut fb, &map, &tiles, view);
        // Window cell (5, 2) spans page x 160..192, y 64..96 → screen x 144..176, y 64..96.
        assert_eq!(fb.get(144, 64), Some(0x42));
        assert_eq!(fb.get(143, 64), Some(1));
        let ship = vec![9u8; 32 * 32];
        draw_player_ship(&mut fb, &ship, 32);
        assert_eq!(fb.get(150, 70), Some(9));
        assert_eq!(fb.get(150, 99), Some(9));
        redraw_overpasses(&mut fb, &map, &tiles, view);
        assert_eq!(fb.get(150, 70), Some(0x42), "overpass covers the ship");
        assert_eq!(
            fb.get(150, 99),
            Some(9),
            "below the overpass the ship shows"
        );
        assert_eq!(fb.get(150, 160), Some(0), "playfield clip");
    }
}
