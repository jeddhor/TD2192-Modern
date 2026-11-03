//! L5 parity: the flight playfield rebuilt from the data files compared with frames captured
//! from the original (C-0002, E1 mission 1 at rest; `docs/re/dynamic-observations.md` §13).
//! Needs TD2192_DATA and TD2192_ORACLE; skips otherwise. No captured pixels are stored here.

use std::fs;

use td_formats::map::TileMap;
use td_formats::sprites::SpriteSheet;
use td_formats::tileset::Tileset;
use td_render::Framebuffer;
use td_render::flight::{self, PLAYFIELD, ViewPos};
use td_test_support::{require_data, require_oracle};

/// View and class from the C-0002 RAM dumps at flight +1 s (flight-model vector 3).
const VIEW: ViewPos = ViewPos {
    tile_x: 102,
    tile_y: 104,
    sub_x: 32,
    sub_y: 16,
};
const CLASS: usize = 4;

/// A small moving object drawn after the overpass redraw (not part of this check, M8).
fn in_known_extra(x: i32, y: i32) -> bool {
    (166..=175).contains(&x) && (60..=68).contains(&y)
}

#[test]
fn playfield_at_rest_matches_the_original() {
    let data = require_data!();
    let oracle = require_oracle!();
    let map = TileMap::parse_episode(&fs::read(data.join("td.map")).unwrap()).unwrap();
    let tiles = Tileset::parse(&fs::read(data.join("td.blk")).unwrap()).unwrap();
    let shp = SpriteSheet::parse_base(&fs::read(data.join("td.shp")).unwrap()).unwrap();
    let mut fb = Framebuffer::new();
    flight::draw_tile_window(&mut fb, &map, &tiles, VIEW);
    flight::draw_player_ship(&mut fb, shp.frame(CLASS).unwrap(), 32);
    flight::redraw_overpasses(&mut fb, &map, &tiles, VIEW);
    for frame in [3500, 3543] {
        let name = format!("f{frame:08}_320x192_flight.vga");
        let bytes = fs::read(oracle.join("C-0002").join("fb").join(&name)).unwrap();
        let pixels = &bytes[768..];
        let mut diff = 0;
        for y in 0..PLAYFIELD.h {
            for x in 0..PLAYFIELD.w {
                let i = (y * 320 + x) as usize;
                if fb.pixels[i] != pixels[i] && !in_known_extra(x, y) {
                    diff += 1;
                }
            }
        }
        assert_eq!(
            diff, 0,
            "{name}: playfield pixels differing from the original"
        );
    }
}
