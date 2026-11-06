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
use td_flow::front::{OptionsScreen, SettingsPort, UiAssets};
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
    /// The session's settings: the file's, with the command-line overrides.
    pub settings: &'a Settings,
    /// The settings as in the file (what the options screen's changes are saved onto).
    pub saved: Settings,
    pub user_dir: &'a Path,
    /// The Enhanced frame the flight screens compose.
    pub enhanced: SharedView,
    /// Changes from the options screens (M11-03).
    pub port: SettingsPort,
}

/// The host key that opens the options screen during play (M11-03). The original reads no
/// F1..F4; F7/F8 are the debug Enhanced hotkeys, F9 the debug mission complete, F10..F12 the
/// other host keys.
pub const OPTIONS_KEY: Key = Key(59); // F2

/// Runs `flow` until it ends or the window closes.
pub fn run(mut flow: Flow, rd: &RunData<'_>, options: &RunOptions) -> Result<()> {
    let settings = rd.settings;
    let mut platform = Platform::new(&WindowConfig {
        fullscreen: settings.video.fullscreen,
        scale_mode: scale_mode(settings.video.scale),
        ..WindowConfig::default()
    })?;

    let mut mixer = Mixer::new(AUDIO_RATE, sfx_policy(settings));
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
    let mut session = settings.clone();
    let mut saved = rd.saved.clone();
    let mut pad_buttons = 0;
    while flow.running() {
        let mut snap: InputSnapshot = platform.poll();
        if snap.quit {
            break;
        }
        handle_host_keys(&snap, &mut platform, &fb, rd.user_dir)?;
        // F10/F11 change the session only; the options screen shows them.
        session.video.fullscreen = platform.is_fullscreen();
        session.video.scale = scale_of(platform.scale_mode());
        enhanced.hotkeys(&mut snap);
        enhanced.poll_settings();
        // Bound controller buttons stand for their flight keys' keyboard keys (M11-04).
        let pad_keys = session.controls.pad_keys(snap.pad, pad_buttons);
        pad_buttons = snap.pad.map_or(0, |p| p.buttons);
        snap.held.extend(pad_keys.held);
        snap.pressed.extend(pad_keys.pressed);
        if snap.was_pressed(OPTIONS_KEY) && !rd.port.is_open() {
            flow.push(Box::new(OptionsScreen::new(
                UiAssets::load(rd.data),
                session.clone(),
                rd.port.clone(),
            )));
        }
        snap.pressed.retain(|&k| k != OPTIONS_KEY);
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
                settings: &session,
                audio: &mut commands,
            };
            flow.tick(&mut ctx, &input);
            for c in commands.drain(..) {
                mixer.command(c);
            }
            if let Some(new) = rd.port.take() {
                // The options screen changed something: apply it live, save it (M11-03).
                apply_settings(&session, &new, &mut platform, &mut mixer)?;
                saved = saved.with_changes(&session, &new);
                let path = rd.user_dir.join("settings.toml");
                if let Err(e) = saved.save(&path) {
                    tracing::warn!("settings not saved: {e}");
                }
                enhanced.apply(&new);
                session = new;
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

/// The SFX policy of `s` (polyphonic only under the Enhanced profile).
fn sfx_policy(s: &Settings) -> SfxPolicy {
    match s.effective_sfx_mode() {
        SfxMode::Classic => SfxPolicy::Classic,
        SfxMode::Polyphonic => SfxPolicy::Polyphonic(8),
    }
}

fn scale_of(m: ScaleMode) -> Scale {
    match m {
        ScaleMode::Aspect4x3 => Scale::Aspect,
        ScaleMode::IntegerSquare => Scale::Integer,
        ScaleMode::Stretch => Scale::Stretch,
    }
}

/// Applies what changed from `old` to `new` to the window and the mixer. The Enhanced
/// presentation takes `new` separately; the flight reads its bindings every tick.
fn apply_settings(
    old: &Settings,
    new: &Settings,
    platform: &mut Platform,
    mixer: &mut Mixer,
) -> Result<()> {
    platform.set_fullscreen(new.video.fullscreen)?;
    if new.video.scale != old.video.scale {
        platform.set_scale_mode(scale_mode(new.video.scale));
    }
    if (new.audio.music_volume, new.audio.sfx_volume)
        != (old.audio.music_volume, old.audio.sfx_volume)
    {
        mixer.command(AudioCommand::SetVolumes {
            music: new.audio.music_volume,
            sfx: new.audio.sfx_volume,
        });
    }
    if new.effective_sfx_mode() != old.effective_sfx_mode() {
        mixer.set_policy(sfx_policy(new));
    }
    Ok(())
}

/// Keys handled by the host rather than the game: F2 options (in the run loop), F10 scale
/// mode, F11 fullscreen, F12 shot.
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
