//! The main loop: fixed-timestep ticks at the current screen's rate, decoupled presentation,
//! and audio rendered on the main thread in step with the game.
//!
//! Presentation takes one of two paths each frame (ADR-0006): the Classic path presents the
//! indexed 320 × 200 framebuffer as before; when any Enhanced feature is on, the frame goes
//! through the Enhanced pipeline ([`crate::enhanced`]) and is presented as RGBA.
//!
//! The game data belongs to the [`DataHost`], which answers the game-data page and switches to
//! a new choice when the start screen asks (M11-03, M13-02).
//!
//! F12 saves the indexed frame as an indexed PNG and, when the Enhanced pipeline presented the
//! frame, also the pipeline's RGBA output at its own size, (W·k) × (200·k), before the window
//! scaling (`*.enhanced.png`).
//!
//! Developer options for headless checks (M12-06): `--dev-stats` logs, once a second, the tick
//! rate the loop runs at and the ticks and frames it ran ([`Stats`]); `--dev-keys` presses keys
//! at given ticks ([`parse_dev_keys`]).

use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use td_audio::{AudioCommand, Mixer, SfxPolicy};
use td_flow::front::{OptionsScreen, SettingsPort, UiAssets};
use td_flow::settings::{Scale, SfxMode};
use td_flow::{FixedStep, Flow, FlowContext, Settings};
use td_input::TickInput;
use td_platform::{InputSnapshot, Key, Platform, ScaleMode, WindowConfig};
use td_render::Framebuffer;
use td_render::enhanced::lighting::LightStage;

use crate::data::DataHost;
use crate::enhanced::{Enhanced, RgbaImage, SharedView};

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
    /// Developer: log the tick and frame rates once a second (`--dev-stats`).
    pub stats: bool,
    /// Developer: keys pressed at given ticks (`--dev-keys`), sorted by tick.
    pub keys: Vec<(u64, Key)>,
}

/// Parses `--dev-keys`: comma-separated `TICK=KEY` or `FIRST-LAST/STEP=KEY` (every STEP ticks
/// from FIRST to LAST), ticks counted from 1 over the whole run, key names as in `[controls]`
/// (`td_input::Key::from_name`, for example `escape`, `enter`, `1`, `space`).
pub fn parse_dev_keys(spec: &str) -> Result<Vec<(u64, Key)>> {
    let mut out = Vec::new();
    for item in spec.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let (when, name) = item
            .split_once('=')
            .with_context(|| format!("{item:?}: expected TICK=KEY"))?;
        let key = Key::from_name(name.trim()).with_context(|| format!("unknown key {name:?}"))?;
        let num = |s: &str| -> Result<u64> {
            s.trim()
                .parse()
                .with_context(|| format!("{item:?}: bad tick {s:?}"))
        };
        match when.split_once('-') {
            None => out.push((num(when)?, key)),
            Some((first, rest)) => {
                let (last, step) = rest.split_once('/').unwrap_or((rest, "1"));
                let (first, last, step) = (num(first)?, num(last)?, num(step)?.max(1));
                let mut t = first;
                while t <= last {
                    out.push((t, key));
                    t += step;
                }
            }
        }
    }
    out.sort_by_key(|&(t, k)| (t, k.0));
    Ok(out)
}

/// `--dev-stats`: ticks and frames counted over each second of wall time, and the average rate
/// since the tick rate last changed (the run loop's own clock, `FixedStep`).
#[derive(Debug)]
struct Stats {
    second: Instant,
    ticks: u64,
    frames: u64,
    frame_time: Duration,
    rate_since: Instant,
    rate_ticks: u64,
}

impl Stats {
    fn new(now: Instant) -> Self {
        Self {
            second: now,
            ticks: 0,
            frames: 0,
            frame_time: Duration::ZERO,
            rate_since: now,
            rate_ticks: 0,
        }
    }

    /// The loop changed its tick rate: start a new average.
    fn rate_changed(&mut self, now: Instant) {
        self.rate_since = now;
        self.rate_ticks = 0;
    }

    /// Logs and restarts the counts once a second.
    fn maybe_log(&mut self, now: Instant, hz: f64, total_ticks: u64) {
        let dt = now - self.second;
        if dt < Duration::from_secs(1) {
            return;
        }
        let span = (now - self.rate_since).as_secs_f64();
        tracing::info!(
            target: "td2192::stats",
            "rate {hz:.4} Hz: {:.2} ticks/s this second; {:.4} ticks/s over {span:.1} s at this rate; \
             {:.1} frames/s, {:.3} ms per frame (draw + present); {total_ticks} ticks in all",
            self.ticks as f64 / dt.as_secs_f64(),
            self.rate_ticks as f64 / span.max(1e-9),
            self.frames as f64 / dt.as_secs_f64(),
            self.frame_time.as_secs_f64() * 1e3 / self.frames.max(1) as f64,
        );
        self.second = now;
        self.ticks = 0;
        self.frames = 0;
        self.frame_time = Duration::ZERO;
    }
}

/// Shared data a run needs (the game data itself is the [`DataHost`]'s).
#[derive(Debug)]
pub struct RunData<'a> {
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

/// Runs `flow` on the game data of `host` until it ends or the window closes.
pub fn run(
    mut flow: Flow,
    rd: &RunData<'_>,
    mut host: DataHost,
    options: &RunOptions,
) -> Result<()> {
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
        // The lighting and bloom stage (M12-11, M12-12).
        Box::new(LightStage::default()),
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
    let mut stats = Stats::new(last);
    let mut dev_keys = options.keys.iter().peekable();
    while flow.running() {
        let mut snap: InputSnapshot = platform.poll();
        if snap.quit {
            break;
        }
        let shot = handle_host_keys(&snap, &mut platform)?;
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
                UiAssets::load(host.data()),
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
            let mut input = std::mem::replace(
                &mut pending,
                TickInput {
                    held: snap.held.clone(),
                    pressed: Vec::new(),
                    pad: snap.pad,
                },
            );
            // --dev-keys: a press (and hold) during this tick.
            while let Some(&&(t, k)) = dev_keys.peek()
                && t <= total_ticks + 1
            {
                if t == total_ticks + 1 {
                    input.pressed.push(k);
                    input.held.insert(k);
                }
                dev_keys.next();
            }
            let mut ctx = FlowContext {
                data: host.data(),
                resources: host.resources(),
                settings: &session,
                audio: &mut commands,
            };
            flow.tick(&mut ctx, &input);
            for c in commands.drain(..) {
                mixer.command(c);
            }
            let path = rd.user_dir.join("settings.toml");
            if let Some(new) = rd.port.take() {
                // The options screen changed something: apply it live, save it (M11-03).
                apply_settings(&session, &new, &mut platform, &mut mixer)?;
                saved = saved.with_changes(&session, &new);
                if let Err(e) = saved.save(&path) {
                    tracing::warn!("settings not saved: {e}");
                }
                enhanced.apply(&new);
                session = new;
            }
            total_ticks += 1;
            stats.ticks += 1;
            stats.rate_ticks += 1;
            if options.exit_after_ticks == Some(total_ticks) {
                return dev_capture(&flow, &mut fb, &mut platform, &mut enhanced, rd.user_dir);
            }
            if rd.port.take_data_switch() && host.switch() {
                // The start screen is on top: the new game data from the next tick on.
                rd.port.data_switched();
                rd.port.set_data_info(host.info().clone());
            }
            if let Some(request) = rd.port.take_data_request() {
                // The game-data page: a dialog or an extraction, then a check (M13-02).
                if let Some(chosen) = host.handle(request, &mut platform) {
                    saved.data_dir = Some(chosen.clone());
                    session.data_dir = Some(chosen);
                    if let Err(e) = saved.save(&path) {
                        tracing::warn!("settings not saved: {e}");
                    }
                }
                rd.port.set_data_info(host.info().clone());
                // The dialog took wall time that must not turn into ticks.
                last = Instant::now();
                step = FixedStep::new(hz);
                stats.rate_changed(last);
                break;
            }
            // A screen change may change the pace; restart the clock at the new rate.
            if rate_of(&flow) != hz {
                hz = rate_of(&flow);
                step = FixedStep::new(hz);
                stats.rate_changed(now);
                if options.stats {
                    tracing::info!(target: "td2192::stats", "tick rate now {hz:.4} Hz at tick {total_ticks}");
                }
                break;
            }
        }
        let frames = platform.audio_frames_wanted(AUDIO_TARGET_FRAMES);
        if frames > 0 {
            audio_buf.resize(frames as usize * 2, 0.0);
            mixer.render(&mut audio_buf);
            platform.queue_audio(&audio_buf)?;
        }
        let frame_start = Instant::now();
        let enhanced_frame = enhanced.begin_frame(platform.output_size(), platform.scale_mode());
        flow.draw(&mut fb);
        if enhanced_frame {
            enhanced.keep_output(shot);
            enhanced.present(&fb, &mut platform, false)?;
        } else {
            platform.present(&fb)?;
        }
        if options.stats {
            let now = Instant::now();
            stats.frames += 1;
            stats.frame_time += now - frame_start;
            stats.maybe_log(now, hz, total_ticks);
        }
        if shot {
            save_screenshots(
                &fb,
                enhanced_frame.then(|| enhanced.take_output()).flatten(),
                rd.user_dir,
            );
        }
    }
    Ok(())
}

/// F12: the indexed frame, and the Enhanced pipeline's output when it presented this frame.
fn save_screenshots(fb: &Framebuffer, enhanced: Option<RgbaImage>, user_dir: &Path) {
    let path = match screenshot(fb, user_dir) {
        Ok(p) => {
            tracing::info!("screenshot saved to {}", p.display());
            p
        }
        Err(e) => {
            tracing::warn!("screenshot failed: {e:#}");
            return;
        }
    };
    if let Some(img) = enhanced {
        let epath = path.with_extension("enhanced.png");
        match save_rgba(&epath, img.width, img.height, &img.rgba) {
            Ok(()) => tracing::info!(
                "enhanced screenshot ({}x{}) saved to {}",
                img.width,
                img.height,
                epath.display()
            ),
            Err(e) => tracing::warn!("enhanced screenshot failed: {e:#}"),
        }
    }
}

/// The SFX policy of `s`: the original's under Classic, the preset's or the choice under
/// Enhanced (D5).
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
/// mode, F11 fullscreen, F12 shot (returned: the run loop saves the frame it presents next).
fn handle_host_keys(snap: &InputSnapshot, platform: &mut Platform) -> Result<bool> {
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
    Ok(snap.was_pressed(Key::F12))
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
        enhanced.keep_output(true);
        let cap = enhanced
            .present(fb, platform, true)?
            .context("window capture")?;
        if let Some(img) = enhanced.take_output() {
            let epath = path.with_extension("enhanced.png");
            save_rgba(&epath, img.width, img.height, &img.rgba)?;
            println!(
                "enhanced output: {} ({}x{})",
                epath.display(),
                img.width,
                img.height
            );
        }
        cap
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dev_keys_parse_single_ticks_and_ranges() {
        let k = parse_dev_keys("100=1, 300=enter,400-460/20=escape").unwrap();
        assert_eq!(
            k,
            vec![
                (100, Key::DIGIT_1),
                (300, Key::ENTER),
                (400, Key::ESCAPE),
                (420, Key::ESCAPE),
                (440, Key::ESCAPE),
                (460, Key::ESCAPE),
            ]
        );
        assert!(parse_dev_keys("100").is_err());
        assert!(parse_dev_keys("x=enter").is_err());
        assert!(parse_dev_keys("5=nokey").is_err());
        assert!(parse_dev_keys("").unwrap().is_empty());
    }
}
