//! Time per stage of the Enhanced pipeline (`docs/spec/enhanced/presentation.md` rev 1 §7).
//!
//! Ignored by default. Run with
//! `cargo test -p td-render --release --test enhanced_timing -- --ignored --nocapture`.
//!
//! The frame is the C-0002 playfield at 21:9 (560 × 200) when TD2192_DATA is set, otherwise a
//! synthetic frame of the same size. In release builds the test fails if the upscalers exceed
//! their part of the 6 ms budget. The budget assumes rayon's default pool on a multi-core
//! desktop: xBR 4× needs about 7 ms on one thread, about 1.7 ms on six (measured 2026-11-05 on
//! the development machine while other builds were running).

use std::fs;
use std::time::{Duration, Instant};

use td_formats::map::TileMap;
use td_formats::palette::{Palette, Rgb6};
use td_formats::sprites::SpriteSheet;
use td_formats::tileset::Tileset;
use td_render::Framebuffer;
use td_render::enhanced::scene::Emissive;
use td_render::enhanced::{Config, FlightInput, FrameInput, NoPost, Pipeline, Upscaler};
use td_render::flight::{self, ViewPos};

const WIDTH: usize = 560;
const RUNS: u32 = 60;

/// Upscaling may take at most this much of the 6 ms frame budget, leaving the rest for
/// lighting and bloom (M12-11, M12-12).
const UPSCALE_BUDGET: Duration = Duration::from_micros(3500);

fn frame() -> (Framebuffer, &'static str) {
    if let Some(data) = td_test_support::data_dir() {
        let map = TileMap::parse_episode(&fs::read(data.join("td.map")).unwrap()).unwrap();
        let tiles = Tileset::parse(&fs::read(data.join("td.blk")).unwrap()).unwrap();
        let shp = SpriteSheet::parse_base(&fs::read(data.join("td.shp")).unwrap()).unwrap();
        let mut fb = Framebuffer::with_width(WIDTH);
        fb.palette = Palette::parse(&fs::read(data.join("td.pal")).unwrap()).unwrap();
        let view = ViewPos {
            tile_x: 102,
            tile_y: 104,
            sub_x: 32,
            sub_y: 16,
        };
        flight::draw_tile_window(&mut fb, &map, &tiles, view);
        flight::draw_player_ship(&mut fb, shp.frame(4).unwrap(), 32);
        flight::redraw_overpasses(&mut fb, &map, &tiles, view);
        let win = fs::read(data.join("td.win")).unwrap();
        flight::draw_hud_bar(&mut fb, &win);
        return (fb, "C-0002 playfield");
    }
    // Blocks of flat colour with dithered and diagonal detail, like pixel art.
    let mut fb = Framebuffer::with_width(WIDTH);
    for (i, c) in fb.palette.colors.iter_mut().enumerate() {
        let v = i as u8;
        *c = Rgb6 {
            r: v & 63,
            g: (v >> 1) & 63,
            b: (v >> 2) & 63,
        };
    }
    for y in 0..200 {
        for x in 0..WIDTH {
            let block = ((x / 24) * 7 + (y / 20) * 13) % 200;
            let detail = if (x + y) % 9 == 0 || (x / 3 + y) % 17 == 0 {
                50
            } else {
                0
            };
            fb.pixels[y * WIDTH + x] = (block + detail) as u8;
        }
    }
    (fb, "synthetic")
}

fn time(mut f: impl FnMut()) -> Duration {
    f(); // warm-up: buffers, rayon pool, tables
    let start = Instant::now();
    for _ in 0..RUNS {
        f();
    }
    start.elapsed() / RUNS
}

#[test]
#[ignore = "timing; run with --release -- --ignored --nocapture"]
fn stage_timings() {
    let (fb, source) = frame();
    let emissive = Emissive::default();
    let input = FrameInput {
        indexed: &fb,
        width: WIDTH,
        flight: Some(FlightInput {
            margin: 120,
            scene: None,
            emissive: &emissive,
        }),
    };
    println!(
        "Enhanced pipeline, {source}, {WIDTH} x 200, {} threads (RAYON_NUM_THREADS):",
        rayon::current_num_threads()
    );
    let mut pipe = Pipeline::default();
    let base = time(|| {
        pipe.render(&input, &Config::default(), &mut NoPost);
    });
    println!(
        "  palette to RGBA        {:>8.3} ms",
        base.as_secs_f64() * 1e3
    );
    let mut worst = Duration::ZERO;
    for kind in Upscaler::ALL.into_iter().skip(1) {
        let cfg = Config {
            upscaler: kind,
            ..Config::default()
        };
        let total = time(|| {
            pipe.render(&input, &cfg, &mut NoPost);
        });
        let k = kind.factor();
        let up = total.saturating_sub(base);
        worst = worst.max(up);
        println!(
            "  {:<8} -> {:>4} x {:<4} {:>8.3} ms upscale ({:.3} ms with palette)",
            format!("{kind:?}"),
            WIDTH * k,
            200 * k,
            up.as_secs_f64() * 1e3,
            total.as_secs_f64() * 1e3
        );
    }
    if !cfg!(debug_assertions) {
        assert!(
            worst <= UPSCALE_BUDGET,
            "slowest upscaler {worst:?} exceeds {UPSCALE_BUDGET:?}"
        );
    }
}
