//! The port's start screen (M11-01): shown when `td2192` runs without `--episode`. It offers
//! the installed episodes, the options and quit.
//!
//! The original has no such screen: each episode is its own program (`file0001.exe`,
//! `td2.exe`, `td3.exe`), started from DOS or the launcher. This screen stands in for that
//! choice, so it is kept plain and in the game's own style (fonts, menu palette, fades) without
//! imitating an original screen. Choosing an episode starts that episode's program as the
//! original would start it (a new program run); its quit item comes back here.

use std::fmt;

use td_assets::Episode;
use td_audio::AudioCommand;
use td_input::TickInput;
use td_render::Framebuffer;

use super::SettingsPort;
use super::options::OptionsScreen;
use super::ui::{Fader, INK_CURSOR, INK_DISABLED, INK_NOTE, INK_TEXT, Nav, PadNav, UiAssets, nav};
use crate::screen::{FlowContext, Screen, Transition};

/// Starts an episode's program for the start screen.
pub trait EpisodeLauncher: fmt::Debug {
    /// The program of `episode`, from its start (a new program run).
    fn launch(
        &mut self,
        ctx: &mut FlowContext<'_>,
        episode: Episode,
    ) -> Result<Box<dyn Screen>, String>;
}

/// The items, top to bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Item {
    Episode(Episode),
    Options,
    Quit,
}

/// Every item in order.
pub const ITEMS: [Item; 5] = [
    Item::Episode(Episode::E1),
    Item::Episode(Episode::E2),
    Item::Episode(Episode::E3),
    Item::Options,
    Item::Quit,
];

const ITEM_Y: i32 = 78;
const ITEM_STEP: i32 = 13;
const ITEM_X: i32 = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Fading in, or waiting for a choice.
    Shown,
    /// Fading out before `Item`'s action.
    Leaving(Item),
    /// An episode or the options screen is on top; this screen fades in again when it
    /// resumes.
    Away,
}

/// The start screen.
#[derive(Debug)]
pub struct StartScreen {
    ui: UiAssets,
    available: [bool; 3],
    launcher: Box<dyn EpisodeLauncher>,
    port: SettingsPort,
    cursor: usize,
    phase: Phase,
    fader: Fader,
    pad: PadNav,
    /// Why the last episode could not start.
    error: Option<String>,
}

fn episode_index(e: Episode) -> usize {
    match e {
        Episode::E1 => 0,
        Episode::E2 => 1,
        Episode::E3 => 2,
    }
}

impl StartScreen {
    /// A start screen; `available[n]` says whether episode n + 1's files are present
    /// (`td_assets::validate`).
    pub fn new(
        ui: UiAssets,
        available: [bool; 3],
        launcher: Box<dyn EpisodeLauncher>,
        port: SettingsPort,
    ) -> Self {
        let mut s = Self {
            ui,
            available,
            launcher,
            port,
            cursor: 0,
            phase: Phase::Shown,
            fader: Fader::fading_in(),
            pad: PadNav::default(),
            error: None,
        };
        if !s.enabled(0) {
            s.cursor = s.next_enabled(0, 1);
        }
        s
    }

    /// The selected item.
    pub fn selected(&self) -> Item {
        ITEMS[self.cursor]
    }

    /// True while the screen waits for a choice (faded in, nothing on top).
    pub fn waiting(&self) -> bool {
        self.phase == Phase::Shown && self.fader.done()
    }

    fn enabled(&self, i: usize) -> bool {
        match ITEMS[i] {
            Item::Episode(e) => self.available[episode_index(e)],
            Item::Options | Item::Quit => true,
        }
    }

    /// The next enabled item from `from` in `dir`, wrapping.
    fn next_enabled(&self, from: usize, dir: isize) -> usize {
        let n = ITEMS.len() as isize;
        let mut i = from as isize;
        for _ in 0..ITEMS.len() {
            i = (i + dir).rem_euclid(n);
            if self.enabled(i as usize) {
                return i as usize;
            }
        }
        from
    }

    fn choose(&mut self) {
        let item = self.selected();
        self.error = None;
        self.fader.fade_out();
        self.phase = Phase::Leaving(item);
    }

    fn act(&mut self, ctx: &mut FlowContext<'_>, item: Item) -> Transition {
        match item {
            Item::Quit => Transition::Quit,
            Item::Options => {
                self.phase = Phase::Away;
                Transition::Push(Box::new(OptionsScreen::new(
                    self.ui.clone(),
                    ctx.settings.clone(),
                    self.port.clone(),
                )))
            }
            Item::Episode(e) => match self.launcher.launch(ctx, e) {
                Ok(screen) => {
                    self.phase = Phase::Away;
                    Transition::Push(screen)
                }
                Err(msg) => {
                    tracing::error!("{e} could not start: {msg}");
                    self.error = Some(format!("episode {} could not start", episode_index(e) + 1));
                    self.phase = Phase::Shown;
                    self.fader = Fader::fading_in();
                    Transition::None
                }
            },
        }
    }
}

impl Screen for StartScreen {
    fn tick(&mut self, ctx: &mut FlowContext<'_>, input: &TickInput) -> Transition {
        let step = self.pad.update(input.pad);
        match self.phase {
            Phase::Away => {
                // Back from an episode (its quit item) or the options: the episode's song and
                // sounds end with its program.
                ctx.audio.push(AudioCommand::StopMusic);
                ctx.audio.push(AudioCommand::StopSfx);
                self.phase = Phase::Shown;
                self.fader = Fader::fading_in();
                self.pad = PadNav::starting_with(input.pad);
                Transition::None
            }
            Phase::Leaving(item) => {
                if self.fader.step() {
                    self.act(ctx, item)
                } else {
                    Transition::None
                }
            }
            Phase::Shown => {
                if !self.fader.step() {
                    return Transition::None;
                }
                match nav(input, &step) {
                    Some(Nav::Up) => self.cursor = self.next_enabled(self.cursor, -1),
                    Some(Nav::Down) => self.cursor = self.next_enabled(self.cursor, 1),
                    Some(Nav::Confirm) => self.choose(),
                    Some(Nav::Back) => {
                        // Esc goes to quit; a second Esc (or a confirm) quits.
                        let quit = ITEMS.len() - 1;
                        if self.cursor == quit {
                            self.choose();
                        } else {
                            self.cursor = quit;
                        }
                    }
                    Some(Nav::Left | Nav::Right) | None => {}
                }
                Transition::None
            }
        }
    }

    fn draw(&self, fb: &mut Framebuffer) {
        fb.clear(0);
        let ui = &self.ui;
        ui.heading(fb, "traffic department 2192", 36);
        for (i, item) in ITEMS.iter().enumerate() {
            let y = ITEM_Y + i as i32 * ITEM_STEP;
            let ink = if !self.enabled(i) {
                INK_DISABLED
            } else if i == self.cursor {
                INK_CURSOR
            } else {
                INK_TEXT
            };
            let label = match item {
                Item::Episode(e) => format!("episode {}", episode_index(*e) + 1),
                Item::Options => "options".to_string(),
                Item::Quit => "quit".to_string(),
            };
            let end = ui.text(fb, &label, (ITEM_X, y), ink);
            if !self.enabled(i) {
                ui.text(fb, "- not installed", (end + 6, y), INK_DISABLED);
            }
        }
        if let Some(e) = &self.error {
            ui.text_centred(fb, e, 156, INK_CURSOR);
        }
        ui.text_centred(
            fb,
            "arrows choose - enter starts - escape quits",
            172,
            INK_NOTE,
        );
        ui.text(
            fb,
            &format!("td2192-modern {}", env!("CARGO_PKG_VERSION")),
            (4, 190),
            INK_NOTE,
        );
        self.fader.apply(&mut fb.palette, &ui.palette);
    }

    fn catches_quit(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;
    use td_assets::{DataDir, ExeResources};
    use td_input::{Key, Pad, PadButton};

    #[derive(Debug, Default)]
    struct Recorder {
        launched: std::rc::Rc<std::cell::RefCell<Vec<Episode>>>,
    }

    #[derive(Debug)]
    struct Program;

    impl Screen for Program {
        fn tick(&mut self, _: &mut FlowContext<'_>, _: &TickInput) -> Transition {
            Transition::Quit
        }
        fn draw(&self, fb: &mut Framebuffer) {
            fb.clear(9);
        }
    }

    impl EpisodeLauncher for Recorder {
        fn launch(
            &mut self,
            _: &mut FlowContext<'_>,
            e: Episode,
        ) -> Result<Box<dyn Screen>, String> {
            self.launched.borrow_mut().push(e);
            if e == Episode::E3 {
                return Err("broken".into());
            }
            Ok(Box::new(Program))
        }
    }

    struct Rig {
        data: DataDir,
        res: ExeResources,
        settings: Settings,
        audio: Vec<AudioCommand>,
    }

    impl Rig {
        fn new() -> Self {
            Self {
                data: DataDir::open(std::env::temp_dir()).unwrap(),
                res: ExeResources::default(),
                settings: Settings::default(),
                audio: Vec::new(),
            }
        }

        fn tick(&mut self, s: &mut dyn Screen, input: &TickInput) -> Transition {
            let mut ctx = FlowContext {
                data: &self.data,
                resources: &self.res,
                settings: &self.settings,
                audio: &mut self.audio,
            };
            s.tick(&mut ctx, input)
        }
    }

    fn key(k: Key) -> TickInput {
        TickInput {
            pressed: vec![k],
            ..TickInput::default()
        }
    }

    fn screen(
        available: [bool; 3],
    ) -> (StartScreen, std::rc::Rc<std::cell::RefCell<Vec<Episode>>>) {
        let rec = Recorder::default();
        let launched = rec.launched.clone();
        (
            StartScreen::new(
                UiAssets::blank(),
                available,
                Box::new(rec),
                SettingsPort::new(),
            ),
            launched,
        )
    }

    /// Ticks until the screen waits (or 200 ticks), returning the first transition that is
    /// not `None`.
    fn settle(rig: &mut Rig, s: &mut StartScreen) -> Option<Transition> {
        for _ in 0..200 {
            if s.waiting() {
                return None;
            }
            match rig.tick(s, &TickInput::default()) {
                Transition::None => {}
                t => return Some(t),
            }
        }
        panic!("the start screen never settled");
    }

    #[test]
    fn unavailable_episodes_are_skipped() {
        let mut rig = Rig::new();
        let (mut s, _) = screen([false, true, false]);
        assert_eq!(s.selected(), Item::Episode(Episode::E2));
        assert!(settle(&mut rig, &mut s).is_none());
        rig.tick(&mut s, &key(Key::DOWN));
        assert_eq!(s.selected(), Item::Options);
        rig.tick(&mut s, &key(Key::UP));
        rig.tick(&mut s, &key(Key::UP));
        assert_eq!(s.selected(), Item::Quit, "wraps past episodes 1 and 3");
    }

    #[test]
    fn an_episode_starts_and_its_quit_comes_back() {
        let mut rig = Rig::new();
        let (mut s, launched) = screen([true, true, true]);
        settle(&mut rig, &mut s);
        rig.tick(&mut s, &key(Key::DOWN));
        rig.tick(&mut s, &key(Key::ENTER));
        let t = settle(&mut rig, &mut s);
        assert!(matches!(t, Some(Transition::Push(_))), "{t:?}");
        assert_eq!(*launched.borrow(), [Episode::E2]);
        assert!(s.catches_quit());
        // Resumed after the episode's quit: the music stops and the screen fades in again.
        rig.audio.clear();
        rig.tick(&mut s, &TickInput::default());
        assert!(
            rig.audio
                .iter()
                .any(|c| matches!(c, AudioCommand::StopMusic))
        );
        assert!(settle(&mut rig, &mut s).is_none());
        assert_eq!(s.selected(), Item::Episode(Episode::E2));
    }

    #[test]
    fn a_failed_launch_stays_here() {
        let mut rig = Rig::new();
        let (mut s, launched) = screen([true, true, true]);
        settle(&mut rig, &mut s);
        rig.tick(&mut s, &key(Key::DOWN));
        rig.tick(&mut s, &key(Key::DOWN));
        rig.tick(&mut s, &key(Key::ENTER));
        assert!(settle(&mut rig, &mut s).is_none());
        assert_eq!(*launched.borrow(), [Episode::E3]);
        assert!(s.error.is_some());
    }

    #[test]
    fn gamepad_moves_confirms_and_goes_back() {
        let mut rig = Rig::new();
        let (mut s, _) = screen([true, true, true]);
        settle(&mut rig, &mut s);
        let pad = |y: i8, buttons: u32| TickInput {
            pad: Some(Pad {
                y,
                buttons,
                ..Pad::default()
            }),
            ..TickInput::default()
        };
        rig.tick(&mut s, &pad(-1, 0));
        assert_eq!(s.selected(), Item::Episode(Episode::E2));
        rig.tick(&mut s, &pad(0, 0));
        // East goes to quit, South then quits.
        rig.tick(&mut s, &pad(0, PadButton::East.bit()));
        assert_eq!(s.selected(), Item::Quit);
        rig.tick(&mut s, &pad(0, 0));
        rig.tick(&mut s, &pad(0, PadButton::South.bit()));
        let t = settle(&mut rig, &mut s);
        assert!(matches!(t, Some(Transition::Quit)), "{t:?}");
    }

    #[test]
    fn options_open_on_top() {
        let mut rig = Rig::new();
        let (mut s, _) = screen([true, false, false]);
        settle(&mut rig, &mut s);
        rig.tick(&mut s, &key(Key::DOWN));
        assert_eq!(s.selected(), Item::Options);
        rig.tick(&mut s, &key(Key::ENTER));
        let t = settle(&mut rig, &mut s);
        assert!(matches!(t, Some(Transition::Push(_))), "{t:?}");
    }

    /// FNV-1a over the pixels and the palette.
    fn snapshot(fb: &Framebuffer) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let rgb = fb.palette.colors.iter().flat_map(|c| [c.r, c.g, c.b]);
        for b in fb.pixels.iter().copied().chain(rgb) {
            h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
        h
    }

    #[test]
    fn snapshot_hashes() {
        // The layout with solid-block fonts, faded in, cursor on episode 1 / all installed,
        // and with episode 2 missing. A change to the layout changes these on purpose.
        let mut rig = Rig::new();
        let mut fb = Framebuffer::new();
        let (mut s, _) = screen([true, true, true]);
        settle(&mut rig, &mut s);
        s.draw(&mut fb);
        let all = snapshot(&fb);
        let (mut s, _) = screen([true, false, true]);
        settle(&mut rig, &mut s);
        s.draw(&mut fb);
        let missing = snapshot(&fb);
        assert_ne!(all, missing);
        assert_eq!(
            (all, missing),
            (0x54e8_8a81_ce34_44a7, 0x42f7_ee42_fa77_d9f7),
            "start screen snapshot hashes ({all:#018x}, {missing:#018x})"
        );
    }

    #[test]
    fn snapshot_with_the_game_fonts() {
        let data = td_test_support::require_data!();
        let dir = DataDir::open(&data).unwrap();
        let mut rig = Rig::new();
        let rec = Recorder::default();
        let mut s = StartScreen::new(
            UiAssets::load(&dir),
            [true, true, true],
            Box::new(rec),
            SettingsPort::new(),
        );
        settle(&mut rig, &mut s);
        let mut fb = Framebuffer::new();
        s.draw(&mut fb);
        let h = snapshot(&fb);
        assert_eq!(h, 0x700d_b535_ea4d_434a, "game-font snapshot {h:#018x}");
    }
}
