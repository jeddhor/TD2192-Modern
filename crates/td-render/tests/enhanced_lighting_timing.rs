//! Timing of the lighting and bloom stages at the heaviest setting of
//! `docs/spec/enhanced/presentation.md` rev 2 §7: 21:9 (W = 560), 4× output (2240 × 800),
//! `full` lighting with a busy scene, and bloom.
//!
//! Run with `cargo test --release -p td-render --test enhanced_lighting_timing -- --ignored
//! --nocapture`. The budget for the whole pipeline is 6 ms per frame; lighting and bloom must
//! leave most of it to the upscaler, so this test asserts at most 3 ms for the two (release
//! builds only).

use std::time::{Duration, Instant};

use td_render::enhanced::bloom::Bloom;
use td_render::enhanced::lighting::{FRAME_ROWS, LightMap, flight_emissive};
use td_render::enhanced::scene::{Light, LightMode, LightScene, Occluders};

const W: usize = 560;
const K: usize = 4;
const FRAMES: u32 = 60;

/// A busy night: five ships' explosions (25 lights), twenty shots, two muzzle flashes, the
/// flame, engines, and a street grid of solid blocks.
fn heavy_scene() -> LightScene {
    let mut lights = Vec::new();
    for e in 0..25 {
        let (x, y) = (40.0 + 21.0 * e as f32, 20.0 + (e * 37 % 120) as f32);
        lights.push(Light {
            pos: (x, y),
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

#[test]
#[ignore = "timing: run in release with --ignored"]
fn lighting_and_bloom_fit_the_budget() {
    let indexed: Vec<u8> = (0..W * FRAME_ROWS).map(|i| (i * 31 % 251) as u8).collect();
    let emissive = flight_emissive([[0x40u8, 0x50, 0x60, 0x70].as_slice()]);
    let base: Vec<u8> = (0..W * K * FRAME_ROWS * K)
        .flat_map(|i| [(i % 256) as u8, (i / 7 % 256) as u8, 200, 255])
        .collect();
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
    let [build, apply, bloom] = times.map(|mut v: Vec<Duration>| {
        v.sort();
        v[v.len() / 2].as_secs_f64() * 1000.0
    });
    let total = build + apply + bloom;
    println!(
        "{W}x200 -> {}x{}, {} lights, {} threads: build {build:.3} ms, apply {apply:.3} ms, \
         bloom {bloom:.3} ms, total {total:.3} ms (medians of {FRAMES})",
        W * K,
        FRAME_ROWS * K,
        scene.lights.len(),
        rayon::current_num_threads(),
    );
    if !cfg!(debug_assertions) {
        assert!(
            total <= 3.0,
            "lighting + bloom took {total:.3} ms per frame"
        );
    }
}
