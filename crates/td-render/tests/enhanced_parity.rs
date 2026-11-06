//! Enhanced presentation invariants on real frames (`docs/spec/enhanced/presentation.md` rev 1
//! §8):
//! - with every Enhanced key off, the pipeline output is the Classic RGBA;
//! - the centre 320 columns of a widescreen playfield equal the Classic playfield and the
//!   original's capture (C-0002 f3500/f3543, as in `flight_parity.rs`).
//!
//! Needs TD2192_DATA and TD2192_ORACLE; skips otherwise. No game pixels are stored here.

use std::fs;

use td_formats::map::TileMap;
use td_formats::palette::Palette;
use td_formats::screen::Screen;
use td_formats::sprites::SpriteSheet;
use td_formats::tileset::Tileset;
use td_render::Framebuffer;
use td_render::enhanced::scene::Emissive;
use td_render::enhanced::{Config, FlightInput, FrameInput, NoPost, PRESET_WIDTHS, Pipeline};
use td_render::flight::{self, PLAYFIELD, ViewPos};
use td_test_support::{require_data, require_oracle, sha256_hex};

/// View and class from the C-0002 RAM dumps at flight +1 s (as in `flight_parity.rs`).
const VIEW: ViewPos = ViewPos {
    tile_x: 102,
    tile_y: 104,
    sub_x: 32,
    sub_y: 16,
};
const CLASS: usize = 4;

/// A small moving object drawn after the overpass redraw (as in `flight_parity.rs`).
fn in_known_extra(x: i32, y: i32) -> bool {
    (166..=175).contains(&x) && (60..=68).contains(&y)
}

struct Assets {
    map: TileMap,
    tiles: Tileset,
    ship: Vec<u8>,
    palette: Palette,
}

fn load(data: &std::path::Path) -> Assets {
    let shp = SpriteSheet::parse_base(&fs::read(data.join("td.shp")).unwrap()).unwrap();
    Assets {
        map: TileMap::parse_episode(&fs::read(data.join("td.map")).unwrap()).unwrap(),
        tiles: Tileset::parse(&fs::read(data.join("td.blk")).unwrap()).unwrap(),
        ship: shp.frame(CLASS).unwrap().to_vec(),
        palette: Palette::parse(&fs::read(data.join("td.pal")).unwrap()).unwrap(),
    }
}

/// The playfield at rest in a frame `width` wide.
fn playfield(a: &Assets, width: usize) -> Framebuffer {
    let mut fb = Framebuffer::with_width(width);
    fb.palette = a.palette.clone();
    flight::draw_tile_window(&mut fb, &a.map, &a.tiles, VIEW);
    flight::draw_player_ship(&mut fb, &a.ship, 32);
    flight::redraw_overpasses(&mut fb, &a.map, &a.tiles, VIEW);
    fb
}

#[test]
fn widescreen_centre_columns_equal_classic_and_the_original() {
    let data = require_data!();
    let oracle = require_oracle!();
    let a = load(&data);
    let classic = playfield(&a, 320);
    let frames: Vec<Vec<u8>> = [3500, 3543]
        .iter()
        .map(|f| {
            let name = format!("f{f:08}_320x192_flight.vga");
            fs::read(oracle.join("C-0002").join("fb").join(name)).unwrap()[768..].to_vec()
        })
        .collect();
    for width in PRESET_WIDTHS[1..].iter().copied() {
        let wide = playfield(&a, width);
        let m = (width - 320) / 2;
        let mut drawn_margin = 0;
        for y in 0..PLAYFIELD.h as usize {
            let row = &wide.pixels[y * width..(y + 1) * width];
            assert_eq!(
                &row[m..m + 320],
                &classic.pixels[y * 320..(y + 1) * 320],
                "{width}: row {y} of the centre differs from Classic"
            );
            for (f, captured) in frames.iter().enumerate() {
                for x in 0..320 {
                    if !in_known_extra(x as i32, y as i32) {
                        assert_eq!(
                            row[m + x],
                            captured[y * 320 + x],
                            "{width}: ({x}, {y}) differs from capture {f}"
                        );
                    }
                }
            }
            drawn_margin += row[..m]
                .iter()
                .chain(&row[m + 320..])
                .filter(|&&p| p != 0)
                .count();
        }
        // The margins show map tiles (the C-0002 view is in the middle of the city).
        assert!(
            drawn_margin > m * 160,
            "{width}: margins mostly blank ({drawn_margin} pixels drawn)"
        );
    }
}

#[test]
fn everything_off_gives_the_classic_rgba() {
    let data = require_data!();
    let a = load(&data);
    let mut pipe = Pipeline::default();
    let emissive = Emissive::default();
    // A flight frame (C-0002 f3543's playfield) and a front-end screen.
    let flight_fb = playfield(&a, 320);
    let mut title = Framebuffer::new();
    let scr = Screen::parse(&fs::read(data.join("command.scr")).unwrap()).unwrap();
    title.copy_full(&scr.pixels);
    title.palette = scr.palette;
    for (fb, is_flight) in [(&flight_fb, true), (&title, false)] {
        let frame = FrameInput {
            indexed: fb,
            width: 320,
            flight: is_flight.then_some(FlightInput {
                margin: 0,
                scene: None,
                emissive: &emissive,
            }),
        };
        let out = pipe.render(&frame, &Config::default(), &mut NoPost);
        assert_eq!(
            sha256_hex(out.rgba),
            sha256_hex(&fb.to_rgba()),
            "Enhanced with every key off differs from Classic (flight: {is_flight})"
        );
    }
}
