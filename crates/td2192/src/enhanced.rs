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
use td_render::enhanced::guidance::Mark;
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
    /// The colour-blind guidance line (accessibility, M12-04).
    pub guidance: bool,
}

/// The Enhanced frame of the current draw.
#[derive(Debug)]
pub struct EnhancedView {
    request: Option<Request>,
    /// The composed W × 200 frame.
    fb: Framebuffer,
    scene: Option<LightScene>,
    emissive: Emissive,
    /// The colour-blind guidance line's marks, when requested.
    marks: Vec<Mark>,
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
            marks: Vec::new(),
            filled: false,
        }
    }
}

impl EnhancedView {
    /// The frame the flight screen composed in this draw, if it did.
    #[cfg(test)]
    pub(crate) fn composed(&self) -> Option<&Framebuffer> {
        self.filled.then_some(&self.fb)
    }

    /// Starts a frame: what to compose (`None`: the Classic path, nothing to compose).
    fn begin(&mut self, request: Option<Request>) {
        self.request = request;
        self.filled = false;
    }

    /// Composes the flight frame: `draw` draws the picture into the W-wide framebuffer, then
    /// the light inputs and, when requested, the colour-blind guidance marks (from the margin
    /// m and the width W) are stored with it.
    pub fn compose(
        &mut self,
        draw: impl FnOnce(&mut Framebuffer),
        scene: impl FnOnce(i32, LightMode) -> Option<LightScene>,
        emissive: &Emissive,
        marks: impl FnOnce(i32, i32) -> Vec<Mark>,
    ) {
        let Some(req) = self.request else {
            return;
        };
        if self.fb.width() != req.width {
            self.fb = Framebuffer::with_width(req.width);
        }
        draw(&mut self.fb);
        let margin = enhanced::margin(req.width) as i32;
        self.scene = req.lighting.and_then(|mode| scene(margin, mode));
        self.emissive.clone_from(emissive);
        self.marks = if req.guidance {
            marks(margin, req.width as i32)
        } else {
            Vec::new()
        };
        self.filled = true;
    }
}

/// An RGBA8 image (4 bytes per pixel, row-major).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbaImage {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// The live Enhanced settings and the pipeline, owned by the run loop.
pub struct Enhanced {
    profile: Profile,
    settings: EnhancedSettings,
    /// The accessibility setting `colorblind_guidance` (not governed by the profile).
    guidance: bool,
    view: SharedView,
    pipeline: Pipeline,
    post: Box<dyn PostStage>,
    reload: Option<Reload>,
    /// Keep a copy of the next presented output (F12).
    keep: bool,
    output: Option<RgbaImage>,
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
            settings: settings.enhanced_values(),
            guidance: settings.accessibility.colorblind_guidance,
            view,
            pipeline: Pipeline::default(),
            post,
            reload,
            keep: false,
            output: None,
        }
    }

    /// Whether the next [`Self::present`] keeps a copy of its output for
    /// [`Self::take_output`] (the F12 screenshot).
    pub fn keep_output(&mut self, keep: bool) {
        self.keep = keep;
    }

    /// The output kept by the last [`Self::present`]: the pipeline's (W·k) × (200·k) RGBA frame,
    /// before the window scaling.
    pub fn take_output(&mut self) -> Option<RgbaImage> {
        self.output.take()
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

    /// Takes the profile, `[enhanced]` and the colour-blind guidance setting from `settings` at
    /// once (the options screen, M11-03). The settings file the host has just saved is not
    /// read again.
    pub fn apply(&mut self, settings: &Settings) {
        self.profile = settings.profile;
        self.settings = settings.enhanced_values();
        self.guidance = settings.accessibility.colorblind_guidance;
        if let Some(r) = &mut self.reload {
            r.modified = modified(&r.path);
        }
    }

    /// Starts a frame for a window of `window` pixels; returns false for the Classic path. The
    /// colour-blind guidance line needs the RGBA stage, so it takes the Enhanced path (with
    /// every `[enhanced]` feature off that path gives the Classic picture plus the marks).
    pub fn begin_frame(&mut self, window: (u32, u32), mode: ScaleMode) -> bool {
        let s = self.effective();
        let request = (!s.is_off() || self.guidance).then(|| Request {
            width: s.widescreen.width().unwrap_or_else(|| {
                enhanced::auto_width(window.0, window.1, mode == ScaleMode::IntegerSquare)
            }),
            lighting: match s.lighting {
                Lighting::Off => None,
                Lighting::Subtle => Some(LightMode::Subtle),
                Lighting::Full => Some(LightMode::Full),
            },
            guidance: self.guidance,
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
        let out = self.render(classic)?;
        Ok(if capture {
            Some(platform.present_rgba_and_capture(out.rgba, out.size, out.base)?)
        } else {
            platform.present_rgba(out.rgba, out.size, out.base)?;
            None
        })
    }

    /// Runs the pipeline on the frame begun by [`Self::begin_frame`].
    pub(crate) fn render(&mut self, classic: &Framebuffer) -> Result<Output<'_>> {
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
        self.pipeline.set_marks(if view.filled {
            view.marks.as_slice()
        } else {
            &[]
        });
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
        if std::mem::take(&mut self.keep) {
            self.output = Some(RgbaImage {
                rgba: out.rgba.to_vec(),
                width: size.0,
                height: size.1,
            });
        }
        Ok(Output {
            rgba: out.rgba,
            size,
            base,
        })
    }
}

/// The pipeline's output for one frame.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Output<'a> {
    /// RGBA8 rows.
    pub rgba: &'a [u8],
    /// (W·k, 200·k).
    pub size: (u32, u32),
    /// The base frame, (W, 200).
    pub base: (u32, u32),
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
        settings.enhanced.upscaler = Some(Upscaler::Xbr4);
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
                lighting: Some(LightMode::Subtle),
                guidance: false,
            })
        );
    }

    #[test]
    fn colour_blind_guidance_takes_the_rgba_path_in_either_profile() {
        let view = SharedView::default();
        let mut settings = Settings::default();
        let mut e = Enhanced::new(&settings, view.clone(), None, Box::new(enhanced::NoPost));
        assert!(
            !e.begin_frame((1920, 1080), ScaleMode::Aspect4x3),
            "off: Classic"
        );
        settings.accessibility.colorblind_guidance = true;
        e.apply(&settings);
        assert!(e.begin_frame((1920, 1080), ScaleMode::Aspect4x3));
        assert_eq!(
            view.borrow().request,
            Some(Request {
                width: 320,
                lighting: None,
                guidance: true,
            }),
            "the Classic picture, with the marks"
        );
    }

    #[test]
    fn the_output_is_kept_only_when_asked() {
        let settings = Settings {
            profile: Profile::Enhanced,
            enhanced: td_flow::settings::EnhancedChoices::chosen(EnhancedSettings {
                upscaler: Upscaler::Scale2x,
                ..EnhancedSettings::default()
            }),
            ..Settings::default()
        };
        let mut e = Enhanced::new(
            &settings,
            SharedView::default(),
            None,
            Box::new(enhanced::NoPost),
        );
        let mut classic = Framebuffer::new();
        classic.put(5, 5, 7);
        assert!(e.begin_frame((1280, 960), ScaleMode::Aspect4x3));
        e.render(&classic).unwrap();
        assert_eq!(e.take_output(), None);
        e.keep_output(true);
        let (rgba, size, base) = e
            .render(&classic)
            .map(|o| (o.rgba.to_vec(), o.size, o.base))
            .unwrap();
        assert_eq!(
            (size, base),
            ((640, 400), (320, 200)),
            "before the window scaling"
        );
        let kept = e.take_output().unwrap();
        assert_eq!((kept.width, kept.height, kept.rgba), (640, 400, rgba));
        e.render(&classic).unwrap();
        assert_eq!(e.take_output(), None, "one frame only");
    }

    #[test]
    fn compose_sizes_the_frame_and_keeps_the_inputs() {
        let mut v = EnhancedView::default();
        v.compose(
            |_| panic!("nothing requested"),
            |_, _| None,
            &Emissive::default(),
            |_, _| panic!("nothing requested"),
        );
        assert!(!v.filled);
        v.begin(Some(Request {
            width: 560,
            lighting: Some(LightMode::Full),
            guidance: false,
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
            |_, _| panic!("guidance not requested"),
        );
        assert!(v.filled);
        assert_eq!((v.fb.width(), v.fb.get(559, 0)), (560, Some(9)));
        assert!(v.scene.is_some() && v.emissive.0[0x3D]);
        assert!(v.marks.is_empty());
        v.begin(Some(Request {
            width: 384,
            lighting: None,
            guidance: true,
        }));
        let mark = Mark {
            x: 1,
            y: 2,
            rgb: [3, 4, 5],
        };
        v.compose(
            |_| {},
            |_, _| panic!("lighting off"),
            &emissive,
            |m, w| {
                assert_eq!((m, w), (32, 384));
                vec![mark]
            },
        );
        assert_eq!(v.marks, [mark]);
    }
}
