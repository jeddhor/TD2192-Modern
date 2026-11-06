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
