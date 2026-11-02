//! The main loop: fixed-timestep ticks, decoupled presentation, audio pulled by the device.

use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result};
use td_audio::{AudioCommand, Mixer, SfxPolicy};
use td_flow::settings::{Scale, SfxMode};
use td_flow::{DEFAULT_TICK_HZ, FixedStep, Settings};
use td_platform::{InputSnapshot, Key, Platform, ScaleMode, WindowConfig};
use td_render::Framebuffer;

/// Audio device rate.
pub const AUDIO_RATE: u32 = 48_000;
/// Audio kept queued ahead of playback (about 43 ms).
const AUDIO_TARGET_FRAMES: u32 = 2048;

/// Input delivered to one simulation tick.
#[derive(Debug, Default)]
pub struct TickInput {
    pub held: std::collections::BTreeSet<Key>,
    /// Keys pressed since the previous tick (edges are delivered to exactly one tick).
    pub pressed: Vec<Key>,
}

impl TickInput {
    pub fn held(&self, k: Key) -> bool {
        self.held.contains(&k)
    }
    pub fn pressed(&self, k: Key) -> bool {
        self.pressed.contains(&k)
    }
}

/// What an application asks the loop to do after a tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Control {
    Continue,
    Quit,
}

/// Something the loop can run.
pub trait App {
    /// Advances one tick; audio commands are queued in `audio`.
    fn tick(&mut self, input: &TickInput, audio: &mut Vec<AudioCommand>) -> Control;
    /// Draws the current state.
    fn draw(&mut self, fb: &mut Framebuffer);
}

fn scale_mode(s: Scale) -> ScaleMode {
    match s {
        Scale::Aspect => ScaleMode::Aspect4x3,
        Scale::Integer => ScaleMode::IntegerSquare,
        Scale::Stretch => ScaleMode::Stretch,
    }
}

/// Options for a run.
#[derive(Debug, Clone, Default)]
pub struct RunOptions {
    /// Developer: save a screenshot and quit after this many ticks.
    pub exit_after_ticks: Option<u64>,
}

/// Runs `app` until it quits or the window closes.
pub fn run(
    mut app: impl App,
    settings: &Settings,
    user_dir: &Path,
    options: &RunOptions,
) -> Result<()> {
    let mut platform = Platform::new(&WindowConfig {
        fullscreen: settings.video.fullscreen,
        scale_mode: scale_mode(settings.video.scale),
        ..WindowConfig::default()
    })?;

    let policy = match settings.audio.sfx_mode {
        SfxMode::Classic => SfxPolicy::Classic,
        SfxMode::Polyphonic => SfxPolicy::Polyphonic(8),
    };
    let mut mixer = Mixer::new(AUDIO_RATE, policy);
    mixer.command(AudioCommand::SetVolumes {
        music: settings.audio.music_volume,
        sfx: settings.audio.sfx_volume,
    });
    if let Err(e) = platform.open_audio(AUDIO_RATE) {
        tracing::warn!("audio unavailable: {e}");
    }
    let mut audio_buf: Vec<f32> = Vec::new();

    let mut step = FixedStep::new(settings.dev.tick_hz.unwrap_or(DEFAULT_TICK_HZ));
    let mut fb = Framebuffer::new();
    let mut pending = TickInput::default();
    let mut commands = Vec::new();
    let mut last = Instant::now();
    loop {
        let snap: InputSnapshot = platform.poll();
        if snap.quit {
            break;
        }
        if snap.was_pressed(Key::F11) {
            platform.toggle_fullscreen()?;
        }
        if snap.was_pressed(Key::F10) {
            let next = match platform.scale_mode() {
                ScaleMode::Aspect4x3 => ScaleMode::IntegerSquare,
                ScaleMode::IntegerSquare => ScaleMode::Stretch,
                ScaleMode::Stretch => ScaleMode::Aspect4x3,
            };
            platform.set_scale_mode(next);
        }
        if snap.was_pressed(Key::F12) {
            match crate::run::screenshot(&fb, user_dir) {
                Ok(p) => tracing::info!("screenshot saved to {}", p.display()),
                Err(e) => tracing::warn!("screenshot failed: {e:#}"),
            }
        }
        pending.held = snap.held.clone();
        pending.pressed.extend(snap.pressed);

        let now = Instant::now();
        let ticks = step.advance(now - last);
        last = now;
        for _ in 0..ticks {
            let input = std::mem::replace(
                &mut pending,
                TickInput {
                    held: snap.held.clone(),
                    pressed: Vec::new(),
                },
            );
            let control = app.tick(&input, &mut commands);
            for c in commands.drain(..) {
                mixer.command(c);
            }
            if control == Control::Quit {
                return Ok(());
            }
            if options.exit_after_ticks == Some(step.total_ticks()) {
                app.draw(&mut fb);
                let path = screenshot(&fb, user_dir)?;
                println!("screenshot: {}", path.display());
                let cap = platform.present_and_capture(&fb)?;
                let wpath = path.with_extension("window.png");
                save_rgba(&wpath, cap.width, cap.height, &cap.rgba)?;
                println!(
                    "window capture: {} ({}x{})",
                    wpath.display(),
                    cap.width,
                    cap.height
                );
                return Ok(());
            }
        }
        let frames = platform.audio_frames_wanted(AUDIO_TARGET_FRAMES);
        if frames > 0 {
            audio_buf.resize(frames as usize * 2, 0.0);
            mixer.render(&mut audio_buf);
            platform.queue_audio(&audio_buf)?;
        }
        app.draw(&mut fb);
        platform.present(&fb)?;
    }
    Ok(())
}

fn save_rgba(path: &Path, w: u32, h: u32, rgba: &[u8]) -> Result<()> {
    let file = std::io::BufWriter::new(std::fs::File::create(path)?);
    let mut enc = png::Encoder::new(file, w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(rgba)?;
    Ok(())
}

/// Saves the framebuffer as an indexed PNG under `<user_dir>/screenshots`.
pub fn screenshot(fb: &Framebuffer, user_dir: &Path) -> Result<std::path::PathBuf> {
    let dir = user_dir.join("screenshots");
    std::fs::create_dir_all(&dir)?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let path = dir.join(format!("td2192-{stamp}.png"));
    let file = std::io::BufWriter::new(std::fs::File::create(&path)?);
    let mut enc = png::Encoder::new(file, td_render::WIDTH as u32, td_render::HEIGHT as u32);
    enc.set_color(png::ColorType::Indexed);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_palette(fb.palette.to_rgb8().concat());
    enc.write_header()
        .context("png header")?
        .write_image_data(&fb.pixels)
        .context("png data")?;
    Ok(path)
}
