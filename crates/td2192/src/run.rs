//! The main loop: fixed-timestep ticks at the current screen's rate, decoupled presentation,
//! and audio rendered on the main thread in step with the game.
//!
//! Presentation takes one of two paths each frame (ADR-0006): the Classic path presents the
//! indexed 320 × 200 framebuffer as before; when any Enhanced feature is on, the frame goes
//! through the Enhanced pipeline ([`crate::enhanced`]) and is presented as RGBA.

use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result};
use td_assets::{DataDir, ExeResources};
use td_audio::{AudioCommand, Mixer, SfxPolicy};
use td_flow::settings::{Scale, SfxMode};
use td_flow::{FixedStep, Flow, FlowContext, Settings};
use td_input::TickInput;
use td_platform::{InputSnapshot, Key, Platform, ScaleMode, WindowConfig};
use td_render::Framebuffer;
use td_render::enhanced::NoPost;

use crate::enhanced::{Enhanced, SharedView};

/// Audio device rate.
pub const AUDIO_RATE: u32 = 48_000;
/// Audio kept queued ahead of playback (about 43 ms).
const AUDIO_TARGET_FRAMES: u32 = 2048;

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

/// Shared data a run needs.
#[derive(Debug)]
pub struct RunData<'a> {
    pub data: &'a DataDir,
    pub resources: &'a ExeResources,
    pub settings: &'a Settings,
    pub user_dir: &'a Path,
    /// The Enhanced frame the flight screens compose.
    pub enhanced: SharedView,
}

/// Runs `flow` until it ends or the window closes.
pub fn run(mut flow: Flow, rd: &RunData<'_>, options: &RunOptions) -> Result<()> {
    let settings = rd.settings;
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
    let mut enhanced = Enhanced::new(
        settings,
        rd.enhanced.clone(),
        Some(rd.user_dir.join("settings.toml")),
        // IM-13: the lighting and bloom stage (M12-11, M12-12) goes here.
        Box::new(NoPost),
    );

    let rate_of = |flow: &Flow| settings.dev.tick_hz.unwrap_or_else(|| flow.tick_hz());
    let mut hz = rate_of(&flow);
    let mut step = FixedStep::new(hz);
    let mut total_ticks: u64 = 0;
    let mut fb = Framebuffer::new();
    let mut pending = TickInput::default();
    let mut commands = Vec::new();
    let mut last = Instant::now();
    while flow.running() {
        let mut snap: InputSnapshot = platform.poll();
        if snap.quit {
            break;
        }
        handle_host_keys(&snap, &mut platform, &fb, rd.user_dir)?;
        enhanced.hotkeys(&mut snap);
        enhanced.poll_settings();
        pending.held = snap.held.clone();
        pending.pad = snap.pad;
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
                    pad: snap.pad,
                },
            );
            let mut ctx = FlowContext {
                data: rd.data,
                resources: rd.resources,
                settings,
                audio: &mut commands,
            };
            flow.tick(&mut ctx, &input);
            for c in commands.drain(..) {
                mixer.command(c);
            }
            total_ticks += 1;
            if options.exit_after_ticks == Some(total_ticks) {
                return dev_capture(&flow, &mut fb, &mut platform, &mut enhanced, rd.user_dir);
            }
            // A screen change may change the pace; restart the clock at the new rate.
            if rate_of(&flow) != hz {
                hz = rate_of(&flow);
                step = FixedStep::new(hz);
                break;
            }
        }
        let frames = platform.audio_frames_wanted(AUDIO_TARGET_FRAMES);
        if frames > 0 {
            audio_buf.resize(frames as usize * 2, 0.0);
            mixer.render(&mut audio_buf);
            platform.queue_audio(&audio_buf)?;
        }
        let enhanced_frame = enhanced.begin_frame(platform.output_size(), platform.scale_mode());
        flow.draw(&mut fb);
        if enhanced_frame {
            enhanced.present(&fb, &mut platform, false)?;
        } else {
            platform.present(&fb)?;
        }
    }
    Ok(())
}

/// Keys handled by the host rather than the game: F10 scale mode, F11 fullscreen, F12 shot.
fn handle_host_keys(
    snap: &InputSnapshot,
    platform: &mut Platform,
    fb: &Framebuffer,
    user_dir: &Path,
) -> Result<()> {
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
        match screenshot(fb, user_dir) {
            Ok(p) => tracing::info!("screenshot saved to {}", p.display()),
            Err(e) => tracing::warn!("screenshot failed: {e:#}"),
        }
    }
    Ok(())
}

fn dev_capture(
    flow: &Flow,
    fb: &mut Framebuffer,
    platform: &mut Platform,
    enhanced: &mut Enhanced,
    user_dir: &Path,
) -> Result<()> {
    let enhanced_frame = enhanced.begin_frame(platform.output_size(), platform.scale_mode());
    flow.draw(fb);
    let path = screenshot(fb, user_dir)?;
    println!("screenshot: {}", path.display());
    let cap = if enhanced_frame {
        enhanced
            .present(fb, platform, true)?
            .context("window capture")?
    } else {
        platform.present_and_capture(fb)?
    };
    let wpath = path.with_extension("window.png");
    save_rgba(&wpath, cap.width, cap.height, &cap.rgba)?;
    println!(
        "window capture: {} ({}x{})",
        wpath.display(),
        cap.width,
        cap.height
    );
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
