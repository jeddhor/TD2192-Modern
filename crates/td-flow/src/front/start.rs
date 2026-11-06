//! The port's start screen (M11-01): shown when `td2192` runs without `--episode`. It offers
//! the installed episodes, the options, the credits and quit.
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
use crate::credits::{Credits, CreditsMode};
use crate::intro::IntroOptions;
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
    /// The original's credits (`docs/spec/frontend/credits.md`): the intro flyover under the
    /// Classic profile, the grouped pages under Enhanced.
    Credits,
    Quit,
}

/// Every item in order.
pub const ITEMS: [Item; 6] = [
    Item::Episode(Episode::E1),
    Item::Episode(Episode::E2),
    Item::Episode(Episode::E3),
    Item::Options,
    Item::Credits,
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
    /// The port's data generation this screen's fonts and episodes come from.
    data_generation: u32,
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
            data_generation: port.data_generation(),
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
            Item::Options | Item::Credits | Item::Quit => true,
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

    /// A new choice of game data (the game-data page, M13-02) is used from this screen: on
    /// top, it asks the host to switch; once the host has, the fonts and the installed
    /// episodes come from the new data.
    fn follow_data(&mut self, ctx: &FlowContext<'_>) {
        if self.port.data_generation() != self.data_generation {
            self.data_generation = self.port.data_generation();
            self.ui = UiAssets::load(ctx.data);
            if let Some(info) = self.port.data_info() {
                self.available = info.current.playable;
            }
            if !self.enabled(self.cursor) {
                self.cursor = self.next_enabled(self.cursor, 1);
            }
        }
        if self.port.data_pending() {
            self.port.want_data_switch();
        }
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
            Item::Credits => {
                let mode = CreditsMode::for_settings(ctx.settings);
                let options = IntroOptions::for_settings(ctx.settings);
                match Credits::load(ctx.data, ctx.resources, mode, options) {
                    Ok(c) => {
                        self.phase = Phase::Away;
                        Transition::Push(Box::new(c))
                    }
                    Err(e) => {
                        tracing::error!("credits unavailable: {e}");
                        self.error = Some("the credits could not start".into());
                        self.phase = Phase::Shown;
                        self.fader = Fader::fading_in();
                        Transition::None
                    }
                }
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
        self.follow_data(ctx);
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
                Item::Credits => "credits".to_string(),
                Item::Quit => "quit".to_string(),
            };
            let end = ui.text(fb, &label, (ITEM_X, y), ink);
            if !self.enabled(i) {
                // The pen after the label is ITEM_X + end - 1; leave a 6-pixel gap.
                ui.text(fb, "- not installed", (ITEM_X + end + 5, y), INK_DISABLED);
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
    fn credits_without_data_stay_here() {
        // The rig's data directory has no td.lt2: the credits cannot start.
        let mut rig = Rig::new();
        let (mut s, _) = screen([true, true, true]);
        settle(&mut rig, &mut s);
        for _ in 0..4 {
            rig.tick(&mut s, &key(Key::DOWN));
        }
        assert_eq!(s.selected(), Item::Credits);
        rig.tick(&mut s, &key(Key::ENTER));
        assert!(settle(&mut rig, &mut s).is_none());
        assert!(s.error.is_some());
    }

    #[test]
    fn credits_open_on_top_in_the_profile_s_mode() {
        let data = td_test_support::require_data!();
        let dir = DataDir::open(&data).unwrap();
        let res = ExeResources::load(&dir).unwrap();
        for (profile, mode) in [
            (crate::settings::Profile::Classic, CreditsMode::Flyover),
            (crate::settings::Profile::Enhanced, CreditsMode::Pages),
        ] {
            let mut rig = Rig::new();
            rig.data = dir.clone();
            rig.res = res.clone();
            rig.settings.profile = profile;
            let (mut s, _) = screen([true, true, true]);
            settle(&mut rig, &mut s);
            for _ in 0..4 {
                rig.tick(&mut s, &key(Key::DOWN));
            }
            rig.tick(&mut s, &key(Key::ENTER));
            match settle(&mut rig, &mut s) {
                Some(Transition::Push(c)) => {
                    let dbg = format!("{c:?}");
                    assert!(dbg.contains(&format!("{mode:?}")), "{profile:?}: {dbg}");
                }
                t => panic!("{profile:?}: {t:?}"),
            }
        }
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

    #[test]
    fn a_new_choice_of_game_data_is_used_from_here() {
        use crate::front::data::{DataInfo, DataSource, DataSummary};
        let mut rig = Rig::new();
        let port = SettingsPort::new();
        let summary = |playable| DataSummary {
            path: std::path::PathBuf::from("game"),
            archive: false,
            counts: td_assets::FileCounts::default(),
            playable,
        };
        let mut info = DataInfo {
            current: summary([true; 3]),
            source: DataSource::Settings,
            pending: None,
            message: None,
            import_dir: std::path::PathBuf::from("game"),
        };
        port.set_data_info(info.clone());
        let mut s = StartScreen::new(
            UiAssets::blank(),
            [true; 3],
            Box::new(Recorder::default()),
            port.clone(),
        );
        settle(&mut rig, &mut s);
        rig.tick(&mut s, &key(Key::DOWN));
        assert_eq!(s.selected(), Item::Episode(Episode::E2));
        rig.tick(&mut s, &TickInput::default());
        assert!(!port.take_data_switch(), "nothing waits");
        // The game-data page chose data without episode 2.
        info.pending = Some(summary([true, false, true]));
        port.set_data_info(info.clone());
        rig.tick(&mut s, &TickInput::default());
        assert!(port.take_data_switch(), "on top, it asks for the switch");
        // The host switched.
        info.current = info.pending.take().unwrap();
        port.set_data_info(info);
        port.data_switched();
        rig.tick(&mut s, &TickInput::default());
        assert!(!port.take_data_switch());
        assert_eq!(s.available, [true, false, true]);
        assert_eq!(
            s.selected(),
            Item::Episode(Episode::E3),
            "the cursor leaves the missing episode"
        );
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
            (0x95f9_1d46_48c2_9a27, 0x59b6_a512_a432_98d7),
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
        if let Some(out) = std::env::var_os("TD2192_UI_DUMP") {
            // For looking at the layout (as the options screen's dump): a PPM in that folder.
            let mut ppm = b"P6 320 200 255\n".to_vec();
            for &p in &fb.pixels {
                let c = fb.palette.colors[usize::from(p)];
                ppm.extend([c.r, c.g, c.b].map(|v| (u16::from(v) * 255 / 63) as u8));
            }
            std::fs::write(std::path::Path::new(&out).join("Start.ppm"), ppm).unwrap();
        }
        let h = snapshot(&fb);
        assert_eq!(h, 0x3c95_25aa_1737_820a, "game-font snapshot {h:#018x}");
    }
}
