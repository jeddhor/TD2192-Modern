//! The screen stack: every part of the game (menus, story cards, dialogue, flight, debrief) is
//! a [`Screen`] that ticks at its own rate and asks for transitions (MDG §8.5).
//!
//! Tick rates follow the original's pacing (`docs/spec/core/timing.md` rev 1): menus, story
//! sequences and fades step once per vertical retrace ([`RETRACE_HZ`]); flight steps once per
//! BIOS timer tick ([`FLIGHT_TICK_HZ`], ADR-0007: the Sound Blaster + music rate, measured by VE-3,
//! C-0074).

use td_assets::{DataDir, ExeResources};
use td_audio::AudioCommand;
use td_input::TickInput;
use td_render::Framebuffer;

use crate::settings::Settings;

/// VGA mode 13h refresh rate: the pace of retrace-synchronised screens and fades.
pub const RETRACE_HZ: f64 = 70.086;

/// In-flight game tick: one BIOS tick in the Sound Blaster + music timer state, 1,193,182 /
/// (12,428 × 5) = 19.2015 Hz (timing spec §4.2; ADR-0007, accepted 2026-11-06; measured 19.208
/// in C-0074).
pub const FLIGHT_TICK_HZ: f64 = 1_193_182.0 / 62_140.0;

/// What a screen asks the flow to do after a tick.
#[derive(Debug)]
pub enum Transition {
    /// Stay on this screen.
    None,
    /// Show another screen on top; this one resumes when it pops.
    Push(Box<dyn Screen>),
    /// Replace this screen.
    Replace(Box<dyn Screen>),
    /// Return to the screen below.
    Pop,
    /// Leave the game.
    Quit,
}

/// Shared services available to screens during a tick.
#[derive(Debug)]
pub struct FlowContext<'a> {
    pub data: &'a DataDir,
    pub resources: &'a ExeResources,
    pub settings: &'a Settings,
    /// Audio commands issued this tick (the host forwards them to the mixer).
    pub audio: &'a mut Vec<AudioCommand>,
}

/// One screen of the game.
pub trait Screen: std::fmt::Debug {
    /// Ticks per second this screen runs at.
    fn tick_hz(&self) -> f64 {
        RETRACE_HZ
    }

    /// Advances one tick.
    fn tick(&mut self, ctx: &mut FlowContext<'_>, input: &TickInput) -> Transition;

    /// Draws the current state.
    fn draw(&self, fb: &mut Framebuffer);

    /// True if a [`Transition::Quit`] from a screen above this one ends there and this screen
    /// resumes (the port's start screen: quitting an episode program returns to it).
    fn catches_quit(&self) -> bool {
        false
    }

    /// True while the screen reads typed characters (`TickInput::text`): the host then turns
    /// the platform's text input on (M12-18; the save-name entry).
    fn wants_text(&self) -> bool {
        false
    }
}

/// The stack of active screens; the top one ticks and draws.
#[derive(Debug, Default)]
pub struct Flow {
    stack: Vec<Box<dyn Screen>>,
}

impl Flow {
    /// A flow starting at `first`.
    pub fn new(first: Box<dyn Screen>) -> Self {
        Self { stack: vec![first] }
    }

    /// True while any screen remains.
    pub fn running(&self) -> bool {
        !self.stack.is_empty()
    }

    /// Tick rate of the current screen.
    pub fn tick_hz(&self) -> f64 {
        self.stack.last().map_or(RETRACE_HZ, |s| s.tick_hz())
    }

    /// True while the current screen reads typed characters ([`Screen::wants_text`]).
    pub fn wants_text(&self) -> bool {
        self.stack.last().is_some_and(|s| s.wants_text())
    }

    /// Shows `screen` on top (the host's options screen); the screen below resumes when it
    /// pops.
    pub fn push(&mut self, screen: Box<dyn Screen>) {
        self.stack.push(screen);
    }

    /// Ticks the top screen and applies its transition.
    pub fn tick(&mut self, ctx: &mut FlowContext<'_>, input: &TickInput) {
        let Some(top) = self.stack.last_mut() else {
            return;
        };
        match top.tick(ctx, input) {
            Transition::None => {}
            Transition::Push(s) => self.stack.push(s),
            Transition::Replace(s) => {
                self.stack.pop();
                self.stack.push(s);
            }
            Transition::Pop => {
                self.stack.pop();
            }
            Transition::Quit => {
                // The quitting screen ends, and so does everything down to a screen that
                // catches the quit.
                self.stack.pop();
                while self.stack.last().is_some_and(|s| !s.catches_quit()) {
                    self.stack.pop();
                }
            }
        }
    }

    /// Draws the top screen.
    pub fn draw(&self, fb: &mut Framebuffer) {
        if let Some(top) = self.stack.last() {
            top.draw(fb);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct Counter {
        id: u8,
        ticks: u32,
        then: Option<fn() -> Transition>,
    }

    impl Screen for Counter {
        fn tick(&mut self, _: &mut FlowContext<'_>, _: &TickInput) -> Transition {
            self.ticks += 1;
            self.then.map_or(Transition::None, |f| f())
        }
        fn draw(&self, fb: &mut Framebuffer) {
            fb.clear(self.id);
        }
    }

    fn ctx_parts() -> (DataDir, ExeResources, Settings) {
        let dir = std::env::temp_dir();
        (
            DataDir::open(dir).unwrap(),
            ExeResources::default(),
            Settings::default(),
        )
    }

    #[test]
    fn push_pop_replace_quit() {
        let (data, res, settings) = ctx_parts();
        let mut audio = Vec::new();
        let mut ctx = FlowContext {
            data: &data,
            resources: &res,
            settings: &settings,
            audio: &mut audio,
        };
        let input = TickInput::default();
        let mut flow = Flow::new(Box::new(Counter {
            id: 1,
            ticks: 0,
            then: Some(|| {
                Transition::Push(Box::new(Counter {
                    id: 2,
                    ticks: 0,
                    then: Some(|| Transition::Pop),
                }))
            }),
        }));
        let mut fb = Framebuffer::new();
        flow.tick(&mut ctx, &input); // pushes 2
        flow.draw(&mut fb);
        assert_eq!(fb.get(0, 0), Some(2));
        flow.tick(&mut ctx, &input); // 2 pops
        flow.draw(&mut fb);
        assert_eq!(fb.get(0, 0), Some(1));
        let mut flow = Flow::new(Box::new(Counter {
            id: 3,
            ticks: 0,
            then: Some(|| Transition::Quit),
        }));
        assert!(flow.running());
        flow.tick(&mut ctx, &input);
        assert!(!flow.running());
    }

    #[derive(Debug)]
    struct Catcher;

    impl Screen for Catcher {
        fn tick(&mut self, _: &mut FlowContext<'_>, _: &TickInput) -> Transition {
            Transition::Quit
        }
        fn draw(&self, fb: &mut Framebuffer) {
            fb.clear(7);
        }
        fn catches_quit(&self) -> bool {
            true
        }
    }

    #[test]
    fn quit_returns_to_a_screen_that_catches_it() {
        let (data, res, settings) = ctx_parts();
        let mut audio = Vec::new();
        let mut ctx = FlowContext {
            data: &data,
            resources: &res,
            settings: &settings,
            audio: &mut audio,
        };
        let input = TickInput::default();
        let mut flow = Flow::new(Box::new(Catcher));
        flow.push(Box::new(Counter {
            id: 1,
            ticks: 0,
            then: None,
        }));
        flow.push(Box::new(Counter {
            id: 2,
            ticks: 0,
            then: Some(|| Transition::Quit),
        }));
        let mut fb = Framebuffer::new();
        flow.tick(&mut ctx, &input);
        flow.draw(&mut fb);
        assert_eq!(fb.get(0, 0), Some(7), "back at the catcher");
        // The catcher's own quit ends the flow.
        flow.tick(&mut ctx, &input);
        assert!(!flow.running());
    }
}
