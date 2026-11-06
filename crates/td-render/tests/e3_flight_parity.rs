//! L5 parity: the E3 flight playfield at rest (`td3.map`/`td4.map` with `td3.blk`, the start
//! views of `docs/spec/sim/e3-mission-setup.md` rev 1 §3.1, §3.2 and the overlay tiles of
//! §3.6) compared with early flight frames of C-0036 (flight 1, `td3.map`) and C-0041 (flight 8,
//! `td4.map`, whose base box (21..23, 165..166) is in the window). The moving objects (the
//! guidance dots, flight 8's three trucks) are drawn by the simulation and not part of this
//! check. Needs TD2192_DATA and TD2192_ORACLE; skips otherwise. No captured pixels are stored.

use std::fs;
use std::path::Path;

use td_formats::map::TileMap;
use td_formats::sprites::SpriteSheet;
use td_formats::tileset::Tileset;
use td_render::Framebuffer;
use td_render::flight::{self, PLAYFIELD, ViewPos};
use td_test_support::{require_data, require_oracle};

/// Draws flight `flight`'s playfield at view (`x`, `y`), offsets (32, 16), with the player
/// ship of `class`, and lists the playfield pixels that differ from `frame` of `capture`.
fn diff(
    data: &Path,
    oracle: &Path,
    (map, flight, x, y, class): (&str, u8, i32, i32, usize),
    (capture, frame): (&str, u32),
) -> Vec<(i32, i32)> {
    let map = TileMap::parse_episode(&fs::read(data.join(map)).unwrap()).unwrap();
    let tiles = Tileset::parse(&fs::read(data.join("td3.blk")).unwrap()).unwrap();
    let shp = SpriteSheet::parse_base(&fs::read(data.join("td.shp")).unwrap()).unwrap();
    let view = ViewPos {
        tile_x: x,
        tile_y: y,
        sub_x: 32,
        sub_y: 16,
    };
    let mut fb = Framebuffer::new();
    flight::draw_tile_window(&mut fb, &map, &tiles, view);
    flight::draw_player_ship(&mut fb, shp.frame(class).unwrap(), 32);
    flight::redraw_overlays_e3(&mut fb, &map, &tiles, view, flight);
    let name = format!("f{frame:08}_320x192_flight.vga");
    let bytes = fs::read(oracle.join(capture).join("fb").join(&name)).unwrap();
    let pixels = &bytes[768..];
    let mut out = Vec::new();
    for y in 0..PLAYFIELD.h {
        for x in 0..PLAYFIELD.w {
            let i = (y * 320 + x) as usize;
            if fb.pixels[i] != pixels[i] {
                out.push((x, y));
            }
        }
    }
    out
}

#[test]
fn e3_flight_1_playfield_matches_c0036() {
    // Flight 1: view (167, 66), class 2. Only the six guidance dots and their shadows differ
    // (at most 24 pixels, all on the dot ring 20..25 pixels from (160, 85)).
    let data = require_data!();
    let oracle = require_oracle!();
    for frame in [698, 702] {
        let d = diff(
            &data,
            &oracle,
            ("td3.map", 1, 167, 66, 2),
            ("C-0036", frame),
        );
        assert!(d.len() <= 24, "C-0036 f{frame}: {} pixels differ", d.len());
        for &(x, y) in &d {
            let r2 = (x - 160).pow(2) + (y - 85).pow(2);
            assert!(
                (18 * 18..=27 * 27).contains(&r2),
                "C-0036 f{frame}: ({x}, {y})"
            );
        }
    }
}

#[test]
fn e3_flight_8_playfield_matches_c0041() {
    // Flight 8: view (17, 164), class 6, td4.map; the trucks of slots 1..3 stand in the
    // window's lower right (x 160..255, y 105..143).
    let data = require_data!();
    let oracle = require_oracle!();
    for frame in [1072, 1080] {
        let d = diff(
            &data,
            &oracle,
            ("td4.map", 8, 17, 164, 6),
            ("C-0041", frame),
        );
        let outside: Vec<_> = d
            .iter()
            .filter(|&&(x, y)| !((156..=258).contains(&x) && (100..=147).contains(&y)))
            .take(8)
            .collect();
        assert!(outside.is_empty(), "C-0041 f{frame}: {outside:?}");
    }
}
