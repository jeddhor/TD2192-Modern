//! Timing of the lighting and bloom stages at the heaviest setting of
//! `docs/spec/enhanced/presentation.md` rev 2 §7: 21:9 (W = 560), 4× output (2240 × 800),
//! `full` lighting and bloom, on a busy synthetic night (25 explosions, 20 shots, flashes, the
//! flame and engines, a street grid of solid blocks).
//!
//! Run with `cargo test --release -p td-render --test enhanced_lighting_timing -- --ignored
//! --nocapture`. The budget for the whole pipeline is 6 ms per frame. xBR 4× at 560 × 200 takes
//! about 1.3 ms on 16 threads and 1.7 ms on 6 (IM-12), so this test asserts at most 4 ms for
//! lighting and bloom together (median, release builds only). Measured 2026-11-05 on an
//! i7-10700F that other builds kept fully loaded: 3.7 ms on 16 threads, 5.1 ms on 6, 23 ms on 1.

use std::time::{Duration, Instant};

use td_render::enhanced::bloom::Bloom;
use td_render::enhanced::lighting::{FRAME_ROWS, LIT_ROWS, LightMap, flight_emissive};
use td_render::enhanced::scene::{Light, LightMode, LightScene, Occluders};

const W: usize = 560;
const K: usize = 4;
const FRAMES: u32 = 60;
/// Explosions in the scene.
const BLASTS: usize = 25;

/// Centre of explosion `e` (base pixels).
fn blast(e: usize) -> (f32, f32) {
    (40.0 + 21.0 * e as f32, 20.0 + (e * 37 % 120) as f32)
}

fn heavy_scene() -> LightScene {
    let mut lights = Vec::new();
    for e in 0..BLASTS {
        lights.push(Light {
            pos: blast(e),
            color: [1.0, 0.55, 0.15],
            radius: 56.0,
            intensity: 1.6 - 0.25 * (e % 6) as f32,
            shadows: true,
        });
    }
    for s in 0..20 {
        lights.push(Light {
            pos: (30.0 + 26.0 * s as f32, 10.0 + (s * 53 % 140) as f32),
            color: [0.9, 0.2, 0.2],
            radius: 12.0 + (s % 3) as f32 * 3.0,
            intensity: 0.9,
            shadows: false,
        });
    }
    for (pos, radius, intensity, shadows) in [
        ((280.0, 84.0), 24.0, 1.2, false),
        ((350.0, 60.0), 24.0, 1.2, false),
        ((280.0, 116.0), 32.0, 1.3, true),
        ((280.0, 94.0), 20.0, 0.35, false),
    ] {
        lights.push(Light {
            pos,
            color: [0.6, 0.8, 1.0],
            radius,
            intensity,
            shadows,
        });
    }
    let (cols, rows) = (22, 8);
    let solid = (0..cols * rows)
        .map(|i| (i % cols) % 3 == 1 && (i / cols) % 2 == 0)
        .collect();
    LightScene {
        mode: LightMode::Full,
        lights,
        occluders: Occluders {
            origin: (-40, -32),
            cols,
            rows,
            solid,
        },
    }
}

/// The indexed frame: city-like colours 100..=199, explosion sprites (half their pixels in
/// explosion colours), shot dots and lit HUD lamps.
fn heavy_frame() -> Vec<u8> {
    let mut f: Vec<u8> = (0..W * FRAME_ROWS)
        .map(|i| 100 + ((i % W) / 7 + (i / W) / 5) as u8 % 100)
        .collect();
    for e in 0..BLASTS {
        let (cx, cy) = blast(e);
        let (cx, cy) = (cx as usize, cy as usize);
        for y in cy.saturating_sub(16)..(cy + 16).min(LIT_ROWS) {
            for x in cx.saturating_sub(16)..(cx + 16).min(W) {
                if (x + y) % 2 == 0 {
                    f[y * W + x] = [0x44, 0x60, 0xF7][(x / 3 + y) % 3];
                }
            }
        }
    }
    for s in 0..20 {
        let (x, y) = (30 + 26 * s, 10 + s * 53 % 140);
        for d in 0..3 {
            f[(y + d) * W + x] = 0x3D;
        }
    }
    for (i, lamp) in [0x31u8, 0x86, 0x2F, 0x26].into_iter().enumerate() {
        for y in 168..174 {
            for x in 0..6 {
                f[y * W + 194 + 12 * i + x] = lamp;
            }
        }
    }
    f
}

#[test]
#[ignore = "timing: run in release with --ignored on an idle machine"]
fn lighting_and_bloom_fit_the_budget() {
    let indexed = heavy_frame();
    let emissive = flight_emissive([[0x44u8, 0x60, 0xF7].as_slice()]);
    let mut pal = [[0u8; 4]; 256];
    for (i, p) in pal.iter_mut().enumerate() {
        *p = [
            (i * 5 % 256) as u8,
            (i * 3 % 256) as u8,
            (i * 7 % 256) as u8,
            255,
        ];
    }
    for (c, rgb) in [
        (0x44, [255, 60, 60]),
        (0x60, [255, 156, 12]),
        (0xF7, [255, 255, 255]),
        (0x3D, [255, 40, 40]),
    ] {
        pal[c] = [rgb[0], rgb[1], rgb[2], 255];
    }
    // Nearest-neighbour stands in for the upscaler's output.
    let mut base = Vec::with_capacity(W * K * FRAME_ROWS * K * 4);
    for y in 0..FRAME_ROWS * K {
        for x in 0..W * K {
            base.extend_from_slice(&pal[usize::from(indexed[(y / K) * W + x / K])]);
        }
    }
    let scene = heavy_scene();
    let (mut map, mut bloom) = (LightMap::default(), Bloom::default());
    let mut out = base.clone();
    let mut times = [vec![], vec![], vec![]];
    for i in 0..FRAMES + 5 {
        out.copy_from_slice(&base);
        let t0 = Instant::now();
        map.build(&scene, W);
        let t1 = Instant::now();
        map.apply(&mut out, W, K, &indexed, &emissive);
        let t2 = Instant::now();
        bloom.apply(&mut out, W, K, &indexed, &emissive);
        let t3 = Instant::now();
        if i >= 5 {
            for (v, d) in times.iter_mut().zip([t1 - t0, t2 - t1, t3 - t2]) {
                v.push(d);
            }
        }
    }
    // Medians: other processes share the machine.
    let [build, apply, glow] = times.map(|mut v: Vec<Duration>| {
        v.sort();
        v[v.len() / 2].as_secs_f64() * 1000.0
    });
    let total = build + apply + glow;
    println!(
        "{W}x200 -> {}x{}, {} lights, {} threads: build {build:.3} ms, apply {apply:.3} ms, \
         bloom {glow:.3} ms, total {total:.3} ms (medians of {FRAMES})",
        W * K,
        FRAME_ROWS * K,
        scene.lights.len(),
        rayon::current_num_threads(),
    );
    if !cfg!(debug_assertions) {
        assert!(
            total <= 4.0,
            "lighting + bloom took {total:.3} ms per frame"
        );
    }
}
