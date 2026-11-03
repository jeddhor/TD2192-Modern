//! SDL3 platform layer: window, presentation of the indexed framebuffer, input devices, audio
//! output and the per-user data directory.
//!
//! SDL types never leave this crate (MDG §8.2): keys are reported as USB HID scancode numbers
//! ([`Key`]), audio is pulled through a plain callback, and frames are taken as
//! [`td_render::Framebuffer`]s. Everything here is thin glue; game logic lives elsewhere.

mod present;

use std::path::PathBuf;

use sdl3::audio::{AudioFormat, AudioSpec, AudioStreamOwner};
use sdl3::event::Event;
use sdl3::render::WindowCanvas;
use sdl3::{EventPump, Sdl};
use td_render::Framebuffer;
use thiserror::Error;

pub use present::{ScaleMode, WindowCapture};
pub use td_input::{InputSnapshot, Key};

/// Errors raised by the platform layer.
#[derive(Debug, Error)]
pub enum PlatformError {
    /// SDL reported an error.
    #[error("SDL error: {0}")]
    Sdl(String),
}

fn sdl_err(e: impl std::fmt::Display) -> PlatformError {
    PlatformError::Sdl(e.to_string())
}

/// Version of the SDL library linked into the program, as `major.minor.patch`.
pub fn sdl_version() -> String {
    let v = sdl3::version::version();
    format!("{}.{}.{}", v.major, v.minor, v.patch)
}

/// The per-user writable directory for settings, saves and screenshots.
pub fn user_data_dir() -> Result<PathBuf, PlatformError> {
    sdl3::filesystem::get_pref_path("TD2192-Modern", "TD2192-Modern").map_err(sdl_err)
}

/// Shows a modal error box (falls back to stderr if SDL cannot show it).
pub fn show_error(title: &str, message: &str) {
    if sdl3::messagebox::show_simple_message_box(
        sdl3::messagebox::MessageBoxFlag::ERROR,
        title,
        message,
        None,
    )
    .is_err()
    {
        eprintln!("{title}: {message}");
    }
}

/// Shows a modal information box.
pub fn show_info(title: &str, message: &str) {
    if sdl3::messagebox::show_simple_message_box(
        sdl3::messagebox::MessageBoxFlag::INFORMATION,
        title,
        message,
        None,
    )
    .is_err()
    {
        println!("{title}: {message}");
    }
}

/// Shows the system folder picker and waits for the user. Returns `None` if they cancel.
pub fn pick_folder() -> Result<Option<PathBuf>, PlatformError> {
    let sdl = sdl3::init().map_err(sdl_err)?;
    let _video = sdl.video().map_err(sdl_err)?;
    let mut events = sdl.event_pump().map_err(sdl_err)?;
    let (tx, rx) = std::sync::mpsc::channel();
    sdl3::dialog::show_open_folder_dialog(
        None::<&std::path::Path>,
        false,
        None,
        Box::new(move |result, _| {
            let _ = tx.send(result.ok().and_then(|mut v| v.pop()));
        }),
    );
    loop {
        for _ in events.poll_iter() {}
        match rx.try_recv() {
            Ok(choice) => return Ok(choice),
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                std::thread::sleep(std::time::Duration::from_millis(15));
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => return Ok(None),
        }
    }
}

/// Window configuration.
#[derive(Debug, Clone)]
pub struct WindowConfig {
    pub title: String,
    /// Initial client size in pixels.
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
    pub scale_mode: ScaleMode,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "Traffic Department 2192".into(),
            width: 1280,
            height: 960,
            fullscreen: false,
            scale_mode: ScaleMode::Aspect4x3,
        }
    }
}

/// An open window with input and (optionally) audio.
pub struct Platform {
    _sdl: Sdl,
    canvas: WindowCanvas,
    presenter: present::Presenter,
    events: EventPump,
    input: td_input::InputState,
    audio: Option<AudioStreamOwner>,
    fullscreen: bool,
}

impl std::fmt::Debug for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Platform")
            .field("fullscreen", &self.fullscreen)
            .field("audio", &self.audio.is_some())
            .finish_non_exhaustive()
    }
}

impl Platform {
    /// Opens the window.
    pub fn new(cfg: &WindowConfig) -> Result<Self, PlatformError> {
        let sdl = sdl3::init().map_err(sdl_err)?;
        let video = sdl.video().map_err(sdl_err)?;
        let mut builder = video.window(&cfg.title, cfg.width, cfg.height);
        builder.position_centered().resizable().high_pixel_density();
        let mut window = builder.build().map_err(sdl_err)?;
        if cfg.fullscreen {
            window.set_fullscreen(true).map_err(sdl_err)?;
        }
        let mut canvas = window.into_canvas();
        let _ = canvas.window_mut(); // keep the canvas mutable for later resizes
        let presenter = present::Presenter::new(&canvas, cfg.scale_mode)?;
        let events = sdl.event_pump().map_err(sdl_err)?;
        Ok(Self {
            _sdl: sdl,
            canvas,
            presenter,
            events,
            input: td_input::InputState::default(),
            audio: None,
            fullscreen: cfg.fullscreen,
        })
    }

    /// Opens audio output at `rate` Hz, stereo `f32`. Audio is pushed from the main thread with
    /// [`Self::queue_audio`], which keeps it in lockstep with the game.
    pub fn open_audio(&mut self, rate: u32) -> Result<(), PlatformError> {
        let audio = self._sdl.audio().map_err(sdl_err)?;
        let spec = AudioSpec {
            freq: Some(rate as i32),
            channels: Some(2),
            format: Some(AudioFormat::f32_sys()),
        };
        let stream = audio
            .default_playback_device()
            .open_device_stream(Some(&spec))
            .map_err(sdl_err)?;
        stream.resume().map_err(sdl_err)?;
        self.audio = Some(stream);
        Ok(())
    }

    /// How many stereo frames to queue now to keep `target_frames` buffered (0 without audio).
    pub fn audio_frames_wanted(&self, target_frames: u32) -> u32 {
        let Some(stream) = &self.audio else {
            return 0;
        };
        let queued = stream.queued_bytes().unwrap_or(0).max(0) as u32 / 8;
        target_frames.saturating_sub(queued)
    }

    /// Queues interleaved stereo `f32` samples for playback.
    pub fn queue_audio(&mut self, samples: &[f32]) -> Result<(), PlatformError> {
        match &self.audio {
            Some(stream) => stream.put_data_f32(samples).map_err(sdl_err),
            None => Ok(()),
        }
    }

    /// Processes pending OS events and returns the input state since the last call.
    pub fn poll(&mut self) -> InputSnapshot {
        for event in self.events.poll_iter() {
            match event {
                Event::Quit { .. } => self.input.quit = true,
                Event::KeyDown {
                    scancode: Some(sc),
                    repeat,
                    ..
                } => self.input.key_down(Key(sc as u16), repeat),
                Event::KeyUp {
                    scancode: Some(sc), ..
                } => self.input.key_up(Key(sc as u16)),
                Event::TextInput { text, .. } => self.input.text.push_str(&text),
                _ => {}
            }
        }
        self.input.snapshot()
    }

    /// Toggles fullscreen (desktop resolution).
    pub fn toggle_fullscreen(&mut self) -> Result<(), PlatformError> {
        self.fullscreen = !self.fullscreen;
        self.canvas
            .window_mut()
            .set_fullscreen(self.fullscreen)
            .map_err(sdl_err)
    }

    /// Changes how the framebuffer is scaled to the window.
    pub fn set_scale_mode(&mut self, mode: ScaleMode) {
        self.presenter.mode = mode;
    }

    /// Current scale mode.
    pub fn scale_mode(&self) -> ScaleMode {
        self.presenter.mode
    }

    /// Shows a frame (waits for vertical sync).
    pub fn present(&mut self, fb: &Framebuffer) -> Result<(), PlatformError> {
        self.presenter
            .present(&mut self.canvas, fb, false)
            .map(|_| ())
    }

    /// Shows a frame and returns what the window displays (developer checks of scaling).
    pub fn present_and_capture(
        &mut self,
        fb: &Framebuffer,
    ) -> Result<WindowCapture, PlatformError> {
        self.presenter
            .present(&mut self.canvas, fb, true)?
            .ok_or_else(|| PlatformError::Sdl("capture failed".into()))
    }
}
