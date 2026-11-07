//! The flight screen's lighting inputs (M12-11): the light scene of the frame the last tick
//! drew, and the flight's emissive colours.
//!
//! Spec: `docs/spec/enhanced/presentation.md` rev 3 §6.1 and §6.4. The scene is built by
//! `td_render::enhanced::lights::flight_scene` from `Flight::frame` and `Flight::drawn`. It is
//! presentation only and never feeds back into the simulation.

use td_formats::sprites::SpriteSheet;
use td_render::enhanced::lighting::flight_emissive;
use td_render::enhanced::lights::{FrameContext, flight_scene};
use td_render::enhanced::scene::{Emissive, LightMode, LightScene};

use super::FlightArena;

/// Bank slots of the `td.exp` frames (after the 20 `td.shp` frames).
const EXPLOSION_SLOTS: std::ops::Range<usize> = 20..27;

/// The emissive colours of a flight whose sprite bank is `bank` (§6.1): the shot colours, the
/// HUD lamps and every colour of the explosion frames.
pub(super) fn emissive_of(bank: &SpriteSheet) -> Emissive {
    flight_emissive(EXPLOSION_SLOTS.filter_map(|i| bank.frame(i)))
}

impl FlightArena {
    /// The light sources and occluders of the frame `draw` shows for a widescreen margin of
    /// `margin` pixels, or `None` while a modal screen is shown. `mode` is the user's setting;
    /// infra-red limits it to `subtle` (§6.4).
    pub(crate) fn light_scene(&self, margin: i32, mode: LightMode) -> Option<LightScene> {
        if self.screen.is_some() {
            return None;
        }
        let ctx = FrameContext {
            world: &self.world,
            palette: self.dac.current(),
            page: self.drawn_page(),
            margin,
        };
        let mut scene = LightScene::default();
        flight_scene(&self.flight, &ctx, mode, &mut scene);
        Some(scene)
    }

    /// The palette indices that glow at full brightness (built when the flight loads).
    pub(crate) fn emissive(&self) -> &Emissive {
        &self.emissive
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use td_assets::{DataDir, ExeResources};
    use td_flow::{FlowContext, Screen, Settings};
    use td_input::{Key, TickInput};
    use td_render::Framebuffer;
    use td_render::enhanced::lighting::LightStage;
    use td_render::enhanced::{Config, FlightInput, FrameInput, Pipeline, Upscaler, margin};
    use td_sim::flight::episode::Episode;

    /// Developer tool for tuning the light table by eye (§9 Q1): flies `TD2192_LIGHT_FLIGHTS`
    /// (default E1 flight 1) firing all the time, and saves lit frames as PNG files to
    /// `<temp>/td2192-lighting`. Needs `TD2192_DATA`. Nothing is written to the repository.
    #[test]
    #[ignore = "developer tool: renders lit frames for tuning"]
    fn render_lit_frames() {
        let Ok(root) = std::env::var("TD2192_DATA") else {
            return;
        };
        let dir = DataDir::open(std::path::Path::new(&root)).unwrap();
        let resources = ExeResources::load(&dir).unwrap();
        let settings = Settings::default();
        let out_dir = std::env::temp_dir().join("td2192-lighting");
        std::fs::create_dir_all(&out_dir).unwrap();
        let flights = std::env::var("TD2192_LIGHT_FLIGHTS").unwrap_or_else(|_| "1".into());
        let ticks: Vec<u32> = std::env::var("TD2192_LIGHT_TICKS")
            .unwrap_or_else(|_| "40,80,120,160".into())
            .split(',')
            .filter_map(|t| t.parse().ok())
            .collect();
        for n in flights.split(',').filter_map(|n| n.parse::<u8>().ok()) {
            let mut arena = FlightArena::new(&dir, &resources, Episode::E1, n).unwrap();
            let mut audio = Vec::new();
            // The opening fade-in (its last frame runs tick 1).
            while arena.opening.is_some() {
                let mut ctx = FlowContext {
                    data: &dir,
                    resources: &resources,
                    settings: &settings,
                    audio: &mut audio,
                };
                arena.tick(&mut ctx, &TickInput::default());
            }
            let mut input = TickInput::default();
            input.held.insert(Key::SPACE);
            input.held.insert(Key::UP);
            for t in 1..=ticks.iter().copied().max().unwrap_or(0) {
                // Fire continuously, and turn now and then.
                input.held.remove(&Key::LEFT);
                if t % 40 < 6 {
                    input.held.insert(Key::LEFT);
                }
                input.pressed = vec![Key::SPACE];
                let mut ctx = FlowContext {
                    data: &dir,
                    resources: &resources,
                    settings: &settings,
                    audio: &mut audio,
                };
                arena.tick(&mut ctx, &input);
                audio.clear();
                if ticks.contains(&t) {
                    save_frames(&arena, &out_dir, n, t);
                }
            }
        }
        println!("lit frames in {}", out_dir.display());
    }

    /// Median, 95th percentile and maximum of `v`, in milliseconds.
    fn spread(v: &mut [std::time::Duration]) -> (f64, f64, f64) {
        v.sort();
        let ms = |d: std::time::Duration| d.as_secs_f64() * 1e3;
        let p = |q: usize| ms(v[(v.len() - 1) * q / 100]);
        (p(50), p(95), ms(v[v.len() - 1]))
    }

    /// M12-06: the CPU cost of a presented flight frame (presentation §7: at most 6 ms for the
    /// heaviest setting), measured on real flights: `TD2192_BENCH_FLIGHTS` (default E1 flights
    /// 1 and 13, `eN:N` for other episodes) flown with fire held and turns now and then, every
    /// frame from tick 40 to tick 40 + `TD2192_BENCH_FRAMES` (default 400). Per frame and
    /// setting: the composed indexed frame (`draw_frame`), the light scene, and the pipeline
    /// (palette, upscale, lighting, bloom; the platform's GPU scaling is not included). The
    /// Classic path is the 320-wide frame and its palette to RGBA (`Framebuffer::to_rgba`, the
    /// CPU side of `present`). Also the tick (td-sim and the host's flight screen). Needs
    /// `TD2192_DATA`; run in release:
    /// `cargo test --release -p td2192 frame_cost -- --ignored --nocapture`.
    #[test]
    #[ignore = "timing: run in release with --ignored --nocapture"]
    fn frame_cost() {
        use std::time::{Duration, Instant};
        use td_render::enhanced::scene::LightMode;
        let Ok(root) = std::env::var("TD2192_DATA") else {
            return;
        };
        let dir = DataDir::open(std::path::Path::new(&root)).unwrap();
        let resources = ExeResources::load(&dir).unwrap();
        let settings = Settings::default();
        let frames: u32 = std::env::var("TD2192_BENCH_FRAMES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(400);
        let flights = std::env::var("TD2192_BENCH_FLIGHTS").unwrap_or_else(|_| "1,13".into());
        // (name, width, upscaler, lighting, bloom); None = the Classic path.
        type Stages = Option<(Upscaler, Option<LightMode>, bool)>;
        let setups: [(&str, usize, Stages); 4] = [
            ("classic 320", 320, None),
            (
                "preset 16:9 xbr3 subtle",
                428,
                Some((Upscaler::Xbr3, Some(LightMode::Subtle), false)),
            ),
            (
                "16:9 xbr3 subtle + bloom",
                428,
                Some((Upscaler::Xbr3, Some(LightMode::Subtle), true)),
            ),
            (
                "heaviest 21:9 xbr4 full + bloom",
                560,
                Some((Upscaler::Xbr4, Some(LightMode::Full), true)),
            ),
        ];
        println!(
            "frame cost, {frames} frames per flight, {} logical CPUs (RAYON_NUM_THREADS: {:?})",
            std::thread::available_parallelism().map_or(0, |n| n.get()),
            std::env::var("RAYON_NUM_THREADS").ok()
        );
        let mut tick_times = Vec::new();
        let mut times: Vec<[Vec<Duration>; 3]> =
            setups.iter().map(|_| Default::default()).collect();
        let (mut pipeline, mut stage) = (Pipeline::default(), LightStage::default());
        let mut lights = 0usize;
        for spec in flights.split(',') {
            let (episode, n) = match spec.split_once(':') {
                Some(("e2", n)) => (Episode::E2, n),
                Some(("e3", n)) => (Episode::E3, n),
                Some((_, n)) => (Episode::E1, n),
                None => (Episode::E1, spec),
            };
            let n: u8 = n.parse().unwrap();
            let mut arena = FlightArena::new(&dir, &resources, episode, n).unwrap();
            let mut audio = Vec::new();
            while arena.opening.is_some() {
                let mut ctx = FlowContext {
                    data: &dir,
                    resources: &resources,
                    settings: &settings,
                    audio: &mut audio,
                };
                arena.tick(&mut ctx, &TickInput::default());
            }
            let mut input = TickInput::default();
            input.held.insert(Key::SPACE);
            input.held.insert(Key::UP);
            for t in 1..40 + frames {
                input.held.remove(&Key::LEFT);
                if t % 40 < 6 {
                    input.held.insert(Key::LEFT);
                }
                input.pressed = vec![Key::SPACE];
                let mut ctx = FlowContext {
                    data: &dir,
                    resources: &resources,
                    settings: &settings,
                    audio: &mut audio,
                };
                let t0 = Instant::now();
                arena.tick(&mut ctx, &input);
                tick_times.push(t0.elapsed());
                audio.clear();
                if t < 40 {
                    continue;
                }
                for (i, &(_, w, cfg)) in setups.iter().enumerate() {
                    let mut fb = Framebuffer::with_width(w);
                    let t0 = Instant::now();
                    arena.draw_frame(&mut fb);
                    let t1 = Instant::now();
                    let Some((upscaler, lighting, bloom)) = cfg else {
                        std::hint::black_box(fb.to_rgba());
                        let t2 = Instant::now();
                        times[i][0].push(t1 - t0);
                        times[i][2].push(t2 - t1);
                        continue;
                    };
                    let m = margin(w);
                    let scene = lighting.and_then(|mode| arena.light_scene(m as i32, mode));
                    let t2 = Instant::now();
                    lights = lights.max(scene.as_ref().map_or(0, |s| s.lights.len()));
                    let frame = FrameInput {
                        indexed: &fb,
                        width: w,
                        flight: Some(FlightInput {
                            margin: m,
                            scene: scene.as_ref(),
                            emissive: arena.emissive(),
                        }),
                    };
                    let c = Config {
                        upscaler,
                        lighting,
                        bloom,
                    };
                    std::hint::black_box(pipeline.render(&frame, &c, &mut stage).rgba.len());
                    let t3 = Instant::now();
                    times[i][0].push(t1 - t0);
                    times[i][1].push(t2 - t1);
                    times[i][2].push(t3 - t2);
                }
            }
        }
        let (tm, tp, tx) = spread(&mut tick_times);
        println!("  tick (td-sim + flight screen): median {tm:.3} ms, p95 {tp:.3}, max {tx:.3}");
        println!("  most lights in one frame: {lights}");
        for ((name, _, _), [draw, scene, pipe]) in setups.iter().zip(times.iter_mut()) {
            let total: Vec<Duration> = (0..draw.len())
                .map(|k| draw[k] + scene.get(k).copied().unwrap_or_default() + pipe[k])
                .collect();
            let (dm, _, _) = spread(draw);
            let (sm, _, _) = if scene.is_empty() {
                (0.0, 0.0, 0.0)
            } else {
                spread(scene)
            };
            let (pm, _, _) = spread(pipe);
            let (am, ap, ax) = spread(&mut total.clone());
            println!(
                "  {name:<32} compose {dm:.3} + scene {sm:.3} + pipeline {pm:.3} ms (medians); \
                 frame median {am:.3}, p95 {ap:.3}, max {ax:.3} ms"
            );
        }
    }

    fn save_frames(arena: &FlightArena, out_dir: &std::path::Path, n: u8, t: u32) {
        let w = 428;
        let mut fb = Framebuffer::with_width(w);
        arena.draw_frame(&mut fb);
        let m = margin(w);
        let (mut pipeline, mut stage) = (Pipeline::default(), LightStage::default());
        for (name, lighting, bloom) in [
            ("day", None, false),
            ("subtle", Some(LightMode::Subtle), true),
            ("full", Some(LightMode::Full), true),
        ] {
            let scene = lighting.and_then(|mode| arena.light_scene(m as i32, mode));
            let frame = FrameInput {
                indexed: &fb,
                width: w,
                flight: Some(FlightInput {
                    margin: m,
                    scene: scene.as_ref(),
                    emissive: arena.emissive(),
                }),
            };
            let cfg = Config {
                upscaler: Upscaler::Xbr2,
                lighting,
                bloom,
            };
            let out = pipeline.render(&frame, &cfg, &mut stage);
            let path = out_dir.join(format!("e1f{n:02}-t{t:04}-{name}.png"));
            let file = std::io::BufWriter::new(std::fs::File::create(&path).unwrap());
            let mut enc = png::Encoder::new(file, out.width as u32, out.height as u32);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            enc.write_header()
                .unwrap()
                .write_image_data(out.rgba)
                .unwrap();
        }
    }
}
