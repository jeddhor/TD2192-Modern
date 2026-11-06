//! Enhanced presentation glue (`docs/spec/enhanced/presentation.md` rev 1, ADR-0006; M12-08).
//!
//! - [`Enhanced`] holds the live `[enhanced]` settings and runs `td_render::enhanced::Pipeline`
//!   for the run loop.
//! - [`EnhancedView`] is the frame the flight screen composes for it.
//!
//! **How a flight frame reaches the pipeline.** The flight runs nested inside the episode
//! programs and campaigns of `td_flow`, which only know `Screen::draw` into a 320 × 200
//! framebuffer. Instead of threading a second draw call through every container, the run loop
//! and the flight screens share one [`SharedView`]:
//! - each frame the run loop states what it wants ([`Request`]: the base width W and the
//!   lighting mode, or nothing when every feature is off);
//! - while drawing, the flight arena (when no modal screen is open) composes the W-wide frame
//!   and its light inputs into the view;
//! - after `Flow::draw` the run loop presents that frame if it was composed, and otherwise
//!   centres the Classic 320 × 200 frame.
//!
//! **Live settings.** Changes apply without a restart:
//! - `settings.toml` is checked for changes twice a second, and a new `profile` or
//!   `[enhanced]` section takes effect at once;
//! - debug builds also have hotkeys that change the session's settings (not saved) and switch
//!   the session to the Enhanced profile:
//!   - F7 cycles the upscaler and Shift+F7 the lighting;
//!   - F8 cycles widescreen and Shift+F8 toggles bloom.
//!
//! F7 and F8 are not used by the original, the host keys (F10..F12) or the debug "complete
//! mission" key (F9), and they are not passed on to the game.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Result, bail};
use td_flow::Settings;
use td_flow::settings::{EnhancedSettings, Lighting, Profile, Upscaler, Widescreen};
use td_platform::{InputSnapshot, Key, Platform, ScaleMode, WindowCapture};
use td_render::Framebuffer;
use td_render::enhanced::scene::{Emissive, LightMode, LightScene};
use td_render::enhanced::{self, Config, FlightInput, FrameInput, Pipeline, PostStage};

/// The view shared by the run loop and the flight screens.
pub type SharedView = Rc<RefCell<EnhancedView>>;

/// What the run loop asks the flight screen to compose this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Request {
    /// Base width W of the frame.
    pub width: usize,
    /// The lighting mode, when lighting is on.
    pub lighting: Option<LightMode>,
}

/// The Enhanced frame of the current draw.
#[derive(Debug)]
pub struct EnhancedView {
    request: Option<Request>,
    /// The composed W × 200 frame.
    fb: Framebuffer,
    scene: Option<LightScene>,
    emissive: Emissive,
    /// True once the flight screen composed this frame.
    filled: bool,
}

impl Default for EnhancedView {
    fn default() -> Self {
        Self {
            request: None,
            fb: Framebuffer::new(),
            scene: None,
            emissive: Emissive::default(),
            filled: false,
        }
    }
}

impl EnhancedView {
    /// Starts a frame: what to compose (`None`: the Classic path, nothing to compose).
    fn begin(&mut self, request: Option<Request>) {
        self.request = request;
        self.filled = false;
    }

    /// Composes the flight frame: `draw` draws the picture into the W-wide framebuffer, then
    /// the light inputs are stored with it.
    pub fn compose(
        &mut self,
        draw: impl FnOnce(&mut Framebuffer),
        scene: impl FnOnce(i32, LightMode) -> Option<LightScene>,
        emissive: &Emissive,
    ) {
        let Some(req) = self.request else {
            return;
        };
        if self.fb.width() != req.width {
            self.fb = Framebuffer::with_width(req.width);
        }
        draw(&mut self.fb);
        self.scene = req
            .lighting
            .and_then(|mode| scene(enhanced::margin(req.width) as i32, mode));
        self.emissive.clone_from(emissive);
        self.filled = true;
    }
}

/// The live Enhanced settings and the pipeline, owned by the run loop.
pub struct Enhanced {
    profile: Profile,
    settings: EnhancedSettings,
    view: SharedView,
    pipeline: Pipeline,
    post: Box<dyn PostStage>,
    reload: Option<Reload>,
}

impl std::fmt::Debug for Enhanced {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Enhanced")
            .field("profile", &self.profile)
            .field("settings", &self.settings)
            .finish_non_exhaustive()
    }
}

/// Watches the settings file.
#[derive(Debug)]
struct Reload {
    path: PathBuf,
    modified: Option<SystemTime>,
    checked: Instant,
}

/// How often the settings file is checked.
const RELOAD_PERIOD: Duration = Duration::from_millis(500);

impl Enhanced {
    /// Starts with `settings`; `settings_path` is watched for live changes when given. `post`
    /// is the lighting and bloom stage.
    pub fn new(
        settings: &Settings,
        view: SharedView,
        settings_path: Option<PathBuf>,
        post: Box<dyn PostStage>,
    ) -> Self {
        let reload = settings_path.map(|path| Reload {
            modified: modified(&path),
            path,
            checked: Instant::now(),
        });
        Self {
            profile: settings.profile,
            settings: settings.enhanced,
            view,
            pipeline: Pipeline::default(),
            post,
            reload,
        }
    }

    /// The features in effect: all off under the Classic profile.
    fn effective(&self) -> EnhancedSettings {
        match self.profile {
            Profile::Classic => EnhancedSettings::default(),
            Profile::Enhanced => self.settings,
        }
    }

    /// Debug builds: the hotkeys of the module docs. Handled keys are removed from `snap`.
    pub fn hotkeys(&mut self, snap: &mut InputSnapshot) {
        if !cfg!(debug_assertions) {
            return;
        }
        let shift = snap.held.contains(&Key::LSHIFT) || snap.held.contains(&Key::RSHIFT);
        let mut changed = false;
        for key in [KEY_F7, KEY_F8] {
            if !snap.was_pressed(key) {
                continue;
            }
            let s = &mut self.settings;
            match (key, shift) {
                (KEY_F7, false) => s.upscaler = next(&Upscaler::ALL, s.upscaler),
                (KEY_F7, true) => s.lighting = next(&Lighting::ALL, s.lighting),
                (_, false) => s.widescreen = next(&Widescreen::ALL, s.widescreen),
                (_, true) => s.bloom = !s.bloom,
            }
            changed = true;
        }
        if changed {
            snap.pressed.retain(|&k| k != KEY_F7 && k != KEY_F8);
            self.profile = Profile::Enhanced;
            tracing::info!("enhanced presentation: {:?}", self.settings);
        }
    }

    /// Re-reads the profile and `[enhanced]` from the settings file when it changed.
    pub fn poll_settings(&mut self) {
        let Some(r) = &mut self.reload else {
            return;
        };
        if r.checked.elapsed() < RELOAD_PERIOD {
            return;
        }
        r.checked = Instant::now();
        let now = modified(&r.path);
        if now == r.modified {
            return;
        }
        r.modified = now;
        match Settings::load(&r.path) {
            Ok(s) => {
                self.apply(&s);
                tracing::info!(
                    "settings reloaded: profile {:?}, {:?}",
                    s.profile,
                    s.enhanced
                );
            }
            Err(e) => tracing::warn!("settings not reloaded: {e}"),
        }
    }

    /// Takes the profile and `[enhanced]` from `settings` at once (the options screen, M11-03).
    /// The settings file the host has just saved is not read again.
    pub fn apply(&mut self, settings: &Settings) {
        self.profile = settings.profile;
        self.settings = settings.enhanced;
        if let Some(r) = &mut self.reload {
            r.modified = modified(&r.path);
        }
    }

    /// Starts a frame for a window of `window` pixels; returns false for the Classic path.
    pub fn begin_frame(&mut self, window: (u32, u32), mode: ScaleMode) -> bool {
        let s = self.effective();
        let request = (!s.is_off()).then(|| Request {
            width: s.widescreen.width().unwrap_or_else(|| {
                enhanced::auto_width(window.0, window.1, mode == ScaleMode::IntegerSquare)
            }),
            lighting: match s.lighting {
                Lighting::Off => None,
                Lighting::Subtle => Some(LightMode::Subtle),
                Lighting::Full => Some(LightMode::Full),
            },
        });
        self.view.borrow_mut().begin(request);
        request.is_some()
    }

    /// Presents the frame begun by [`Self::begin_frame`]: the flight screen's composed frame,
    /// or `classic` centred.
    pub fn present(
        &mut self,
        classic: &Framebuffer,
        platform: &mut Platform,
        capture: bool,
    ) -> Result<Option<WindowCapture>> {
        let s = self.effective();
        let cfg = Config {
            upscaler: upscaler(s.upscaler),
            lighting: self.view.borrow().request.and_then(|r| r.lighting),
            bloom: s.bloom,
        };
        let view = self.view.borrow();
        let Some(req) = view.request else {
            bail!("no Enhanced frame was begun");
        };
        let frame = if view.filled {
            FrameInput {
                indexed: &view.fb,
                width: req.width,
                flight: Some(FlightInput {
                    margin: enhanced::margin(req.width),
                    scene: view.scene.as_ref(),
                    emissive: &view.emissive,
                }),
            }
        } else {
            FrameInput {
                indexed: classic,
                width: req.width,
                flight: None,
            }
        };
        let out = self.pipeline.render(&frame, &cfg, self.post.as_mut());
        let size = (out.width as u32, out.height as u32);
        let base = (req.width as u32, td_render::HEIGHT as u32);
        Ok(if capture {
            Some(platform.present_rgba_and_capture(out.rgba, size, base)?)
        } else {
            platform.present_rgba(out.rgba, size, base)?;
            None
        })
    }
}

/// F7 and F8 (USB HID scancodes).
const KEY_F7: Key = Key(0x40);
const KEY_F8: Key = Key(0x41);

/// The item after `v` in `all`, wrapping.
fn next<T: Copy + PartialEq>(all: &[T], v: T) -> T {
    let i = all.iter().position(|&x| x == v).unwrap_or(0);
    all[(i + 1) % all.len()]
}

fn modified(path: &std::path::Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// The pipeline's filter for a settings value.
fn upscaler(u: Upscaler) -> enhanced::Upscaler {
    use enhanced::Upscaler as U;
    match u {
        Upscaler::None => U::None,
        Upscaler::Scale2x => U::Scale2x,
        Upscaler::Scale3x => U::Scale3x,
        Upscaler::Scale4x => U::Scale4x,
        Upscaler::Xbr2 => U::Xbr2,
        Upscaler::Xbr3 => U::Xbr3,
        Upscaler::Xbr4 => U::Xbr4,
    }
}

/// Parses `--dev-enhanced`: comma-separated `key=value` pairs of the `[enhanced]` section, for
/// example `widescreen=16:9,upscaler=xbr2,bloom=true`.
pub fn parse_spec(spec: &str) -> Result<EnhancedSettings> {
    let mut text = String::new();
    for pair in spec.split(',').filter(|p| !p.trim().is_empty()) {
        let Some((key, value)) = pair.split_once('=') else {
            bail!("expected key=value, found {pair:?}");
        };
        let (key, value) = (key.trim(), value.trim());
        if !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            bail!("invalid key {key:?}");
        }
        match value {
            "true" | "false" => text.push_str(&format!("{key} = {value}\n")),
            _ => text.push_str(&format!("{key} = {value:?}\n")),
        }
    }
    Ok(toml::from_str(&text)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dev_spec_parses_the_enhanced_keys() {
        let s = parse_spec("widescreen=16:9, upscaler=xbr2,lighting=full,bloom=true").unwrap();
        assert_eq!(
            s,
            EnhancedSettings {
                widescreen: Widescreen::W16x9,
                upscaler: Upscaler::Xbr2,
                lighting: Lighting::Full,
                bloom: true,
            }
        );
        assert!(parse_spec("upscaler=xbr9").is_err());
        assert!(parse_spec("widescreen").is_err());
        assert_eq!(parse_spec("").unwrap(), EnhancedSettings::default());
    }

    #[test]
    fn classic_profile_requests_nothing() {
        let view = SharedView::default();
        let mut settings = Settings::default();
        settings.enhanced.upscaler = Upscaler::Xbr4;
        let mut e = Enhanced::new(&settings, view.clone(), None, Box::new(enhanced::NoPost));
        assert!(!e.begin_frame((1920, 1080), ScaleMode::Aspect4x3));
        assert_eq!(view.borrow().request, None);
        e.profile = Profile::Enhanced;
        e.settings.widescreen = Widescreen::Auto;
        e.settings.lighting = Lighting::Subtle;
        assert!(e.begin_frame((1920, 1080), ScaleMode::Aspect4x3));
        assert_eq!(
            view.borrow().request,
            Some(Request {
                width: 428,
                lighting: Some(LightMode::Subtle)
            })
        );
    }

    #[test]
    fn compose_sizes_the_frame_and_keeps_the_inputs() {
        let mut v = EnhancedView::default();
        v.compose(
            |_| panic!("nothing requested"),
            |_, _| None,
            &Emissive::default(),
        );
        assert!(!v.filled);
        v.begin(Some(Request {
            width: 560,
            lighting: Some(LightMode::Full),
        }));
        let mut emissive = Emissive::default();
        emissive.0[0x3D] = true;
        v.compose(
            |fb| fb.put(559, 0, 9),
            |m, mode| {
                assert_eq!((m, mode), (120, LightMode::Full));
                Some(LightScene::default())
            },
            &emissive,
        );
        assert!(v.filled);
        assert_eq!((v.fb.width(), v.fb.get(559, 0)), (560, Some(9)));
        assert!(v.scene.is_some() && v.emissive.0[0x3D]);
    }
}
