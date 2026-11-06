//! The Episode 2 program (`td2.exe` main body): script choice, main menu and the campaign.
//!
//! Spec: `docs/spec/flow/e2-program-flow.md` rev 1 §4.1 (start-up), §4.2 (menu, items and
//! bookkeeping), §4.6, §7 (music); E1 screen layouts per `docs/spec/flow/screens.md` rev 1 §3.1,
//! §3.2 with the E2 deltas of episode-deltas rev 2 §8.3. The campaign is
//! [`crate::campaign_e2`]. Fidelity: T0 for the layouts and the menu colours (OB-041 kept), T1
//! for the state between blocks, T2 for pacing.
//!
//! `td2.exe` is started directly (U-005): it reads no `tdstatus.dat`, asks for the script at
//! every start and starts at mission 0, cursor 0.
//!
//! Not ported (T3 or decided elsewhere): the text-mode configuration screen, command-line
//! switches, the memory gate, joystick calibration and the exit notice (the port has its own
//! settings and shell). Save and restore (menu items 1 and 2) are E1's screens
//! (`crate::save_screens`, save-screens rev 1 §4.5) on the port's own `td2.sav`/`td2.stt`
//! (E3: `td3.*`) in the directory given to [`LaterEpisode::with_save_dir`] (none: the restore
//! screen shows empty slots and a save is not written); [`LaterEpisode::restored`] applies the
//! menu bookkeeping of a restore.
//!
//! The menu is shown without its fades and highlight ramps (T2); the colours of entries
//! 224..233 are the ramps' end states, kept as the original keeps them: the palette buffer
//! (`g_MenuPalette`) and the DAC separately.
//!
//! The program is shared with Episode 3 (`td3.exe`, `crate::program_e3`): [`LaterEpisode`]
//! is the menu of both, parameterised by the episode's campaign ([`EpisodeCampaign`]) and its
//! [`FrontEnd`] constants (e3-program-flow rev 1 §4.1, §4.2 list the E3 deltas).

use std::fmt;
use std::path::PathBuf;

use td_assets::{DataDir, Episode, ExeResources};
use td_audio::AudioCommand;
use td_audio::music_cues::{E2_INTRO, MENU};
use td_formats::palette::{Palette, Rgb6};
use td_input::{Key, TickInput};
use td_render::{Framebuffer, WIDTH, title_text};

use crate::campaign::FlightLauncher;
use crate::campaign_e2::{BlockResult, CampaignE2};
use crate::dialogue::ScriptChoice;
use crate::flight_screens::{KEY_KP2, KEY_KP8};
use crate::program::{Program, line};
use crate::save_screens::{SaveFiles, SaveKind, SaveOutcome, SaveScreen, SaveSet};
use crate::scenes_e2::{self, Bg};
use crate::screen::{FlowContext, RETRACE_HZ, Screen, Transition};
use crate::story::{self, Hold, StoryError, StoryScreen};

/// What differs between the E2 and E3 front ends (beyond the campaign).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrontEnd {
    pub episode: Episode,
    /// Resource of the script-choice warning.
    pub script_choice: &'static str,
    /// The original script; without it script 2 is chosen silently (§4.1 step 9).
    pub dialogue_file: &'static str,
    /// Resource of the two story-so-far pages (menu item 5).
    pub background_story: &'static str,
    /// The `intro.cmf` cue of menu items 6 and 7.
    pub intro_cue: u8,
    /// The mission number after the menu's `+= 1` that completes the episode (mission := 0,
    /// cursor := 7).
    pub complete_at: u8,
    /// E2 also plays song 7 on entering the menu loop (§4.2); E3 only from the loader.
    pub menu_song_at_entry: bool,
    /// E3: `UI_LoadMainMenu` flushes the keyboard, so keys typed after the script choice do
    /// not reach the menu (e3-program-flow §4.1).
    pub loader_flushes: bool,
    /// The saved-game files (save-screens §4.5) and the resources of the two screens' labels.
    pub saves: SaveFiles,
    pub save_labels: &'static str,
    pub restore_labels: &'static str,
}

/// What the menu hands a mission block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BlockStart {
    /// `g_MissionNumber`.
    pub mission: u8,
    /// `g_KillsToDate`.
    pub kills_to_date: i16,
    /// The skip flag as the menu left it (E3 does not clear it, OB-050).
    pub skip: bool,
    /// `g_MissionKills` of the last flight flown (E3 missions without a block add it, OB-051).
    pub mission_kills: i16,
}

/// A later episode's campaign sequencer as the menu drives it.
pub trait EpisodeCampaign: Sized {
    /// The episode's front-end constants.
    const FRONT: FrontEnd;
    /// `Main_CampaignLoop` for one block.
    fn start(start: BlockStart) -> Self;
    /// Runs the block for one frame; the result when it returns to the menu.
    fn step(
        &mut self,
        prog: &mut Program,
        ctx: &mut FlowContext<'_>,
        input: &TickInput,
    ) -> Option<BlockResult>;
    fn draw(&self, fb: &mut Framebuffer);
    /// Ticks per second of the block's current screen (BUG-5).
    fn tick_hz(&self) -> f64;
    /// The in-process picture of menu item 6 (`aboutus.scr`) or 7 (`order.scr`).
    fn picture(prog: &Program, item: u8) -> Result<StoryScreen, StoryError>;
}

/// The Episode 2 front end (e2-program-flow rev 1 §4.1, §4.2).
pub const E2_FRONT: FrontEnd = FrontEnd {
    episode: Episode::E2,
    script_choice: "e2.script_choice.warning",
    dialogue_file: "dialogu2.dat",
    background_story: "e2.background_story",
    intro_cue: E2_INTRO,
    complete_at: 21,
    menu_song_at_entry: true,
    loader_flushes: false,
    saves: SaveFiles {
        sav: "td2.sav",
        stt: "td2.stt",
    },
    save_labels: "e2.saves.save",
    restore_labels: "e2.saves.restore",
};

impl EpisodeCampaign for CampaignE2 {
    const FRONT: FrontEnd = E2_FRONT;

    fn start(start: BlockStart) -> Self {
        CampaignE2::new(start.mission, start.kills_to_date)
    }

    fn step(
        &mut self,
        prog: &mut Program,
        ctx: &mut FlowContext<'_>,
        input: &TickInput,
    ) -> Option<BlockResult> {
        CampaignE2::step(self, prog, ctx, input)
    }

    fn draw(&self, fb: &mut Framebuffer) {
        CampaignE2::draw(self, fb);
    }

    fn tick_hz(&self) -> f64 {
        CampaignE2::tick_hz(self)
    }

    fn picture(prog: &Program, item: u8) -> Result<StoryScreen, StoryError> {
        let bg = if item == 6 { Bg::Aboutus } else { Bg::Order };
        scenes_e2::scene_card(&prog.data, &prog.assets, bg, ["", "", ""])
    }
}

/// Menu items (§4.2).
const ITEMS: u8 = 9;
const SAVE: u8 = 1;
const RESTORE: u8 = 2;
const NEXT_MISSION: u8 = 3;
const QUIT: u8 = 8;
/// First palette entry of the item colours (FORMATS §12.1).
const ITEM_ENTRY: usize = 224;
/// Entries the confirmation boxes keep at full brightness (screens §3.1).
const BOX_ENTRIES: std::ops::RangeInclusive<usize> = 192..=209;

const DIM: Rgb6 = Rgb6 { r: 16, g: 0, b: 0 };
const BLACK: Rgb6 = Rgb6 { r: 0, g: 0, b: 0 };
const RED: Rgb6 = Rgb6 { r: 63, g: 0, b: 0 };
const YELLOW: Rgb6 = Rgb6 { r: 63, g: 63, b: 0 };

/// Pens and inks of `e2.script_choice.warning[1..6]`: screens §3.2 with one ink-92 line fewer
/// (episode-deltas §8.3). Black-box (C-0015 f146): the four ink-92 lines keep E1's first four
/// pen x but sit 8 rows lower (y 70, 77, 84, 91); the ink-102 lines are E1's.
const CHOICE_PENS: [(i32, i32, u8); 6] = [
    (66, 70, 92),
    (54, 77, 92),
    (42, 84, 92),
    (38, 91, 92),
    (66, 110, 102),
    (90, 117, 102),
];

/// What follows a sub-screen of the menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Back {
    /// The menu fades in again from its palette buffer (help, port notices).
    FadeIn,
    /// `UI_LoadMainMenu`-style reload (story so far: without music).
    Reload { music: bool },
}

enum Stage<C> {
    Start,
    ScriptChoice(StoryScreen),
    Menu,
    /// A block started from item 0 or item 3.
    Campaign(Box<C>, u8),
    /// A sub-screen, what follows it and how its Esc reaches the skip flag.
    Sub(StoryScreen, Back, SkipRule),
    /// A confirmation box (1 = quit, 2 = current game lost) for an item.
    Confirm(u8, u8),
    /// The save or restore screen.
    Saves(Box<SaveScreen>),
    Quit,
}

/// How a menu sub-screen treats the skip flag (e3-program-flow §3.5; only E3 reads it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SkipRule {
    /// Help: not touched.
    Keep,
    /// About and order pictures: cleared on entry, set by Esc.
    Card,
    /// Story so far: not cleared on entry, set by Esc.
    Story,
}

/// The menu image, the confirmation boxes (320 × 32 each) and `tdmen.pal`.
type MenuGraphics = (Box<[u8]>, Box<[u8]>, Palette);

/// The program of a later episode (E2, E3) as a screen.
pub struct LaterEpisode<C: EpisodeCampaign> {
    prog: Program,
    stage: Stage<C>,
    cursor: u8,
    mission: u8,
    kills_to_date: i16,
    /// `g_MissionKills`, kept between blocks (OB-051).
    mission_kills: i16,
    /// `g_SkipRequested` between blocks (only E3 reads it, OB-050).
    skip: bool,
    menu: Option<MenuGraphics>,
    /// `g_MenuPalette` entries 224..233.
    buffer: [Rgb6; 10],
    /// DAC entries 224..233.
    dac: [Rgb6; 10],
    /// Keys typed after the script choice (no flush in E2, §4.1).
    typed_ahead: Vec<Key>,
    /// Where the port keeps the episode's saved games.
    save_dir: Option<PathBuf>,
}

/// The Episode 2 program.
pub type Episode2 = LaterEpisode<CampaignE2>;

impl<C: EpisodeCampaign> fmt::Debug for LaterEpisode<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LaterEpisode")
            .field("episode", &C::FRONT.episode)
            .field("cursor", &self.cursor)
            .field("mission", &self.mission)
            .field("kills_to_date", &self.kills_to_date)
            .finish_non_exhaustive()
    }
}

impl<C: EpisodeCampaign> LaterEpisode<C> {
    pub fn new(
        data: &DataDir,
        res: &ExeResources,
        launcher: Box<dyn FlightLauncher>,
    ) -> Result<Self, StoryError> {
        let menu = (|| {
            let men = data.read("td.men").ok()?;
            let pal = Palette::parse(&data.read("tdmen.pal").ok()?).ok()?;
            Some((
                men.get(16_000..80_000)?.into(),
                men.get(80_000..120_960)?.into(),
                pal,
            ))
        })();
        if menu.is_none() {
            tracing::warn!("td.men or tdmen.pal unavailable; the menu is drawn black");
        }
        let mut ep = Self {
            prog: Program::for_episode(data, res, launcher, C::FRONT.episode)?,
            stage: Stage::Start,
            cursor: 0,
            mission: 0,
            kills_to_date: 0,
            mission_kills: 0,
            skip: false,
            menu,
            buffer: [DIM; 10],
            dac: [DIM; 10],
            typed_ahead: Vec::new(),
            save_dir: None,
        };
        ep.load_menu_colours();
        Ok(ep)
    }

    /// Keeps the saved games in `dir` (the user data directory's `saves`), in the original
    /// format with the episode's file names; the game folder is never written.
    #[must_use]
    pub fn with_save_dir(mut self, dir: PathBuf) -> Self {
        self.save_dir = Some(dir);
        self
    }

    /// Mission (0 = no game), kills to date and cursor.
    pub fn state(&self) -> (u8, i16, u8) {
        (self.mission, self.kills_to_date, self.cursor)
    }

    /// DAC entries 224..233 (the item colours) as shown.
    pub fn item_colours(&self) -> [Rgb6; 10] {
        self.dac
    }

    /// The skip flag as the menu holds it (E3, OB-050).
    pub fn skip_flag(&self) -> bool {
        self.skip
    }

    /// The menu after a successful restore of (`mission`, `kills`) from item 2 (§4.2): the
    /// cursor moves to item 3, the loader runs, and the palette buffer's entry 227 is cleared
    /// (else 226's red goes back to 16). Black-box (C-0016 f750, U-314): the loader already
    /// sees cursor 3 (DAC 227 = (63, 0, 0), 226 = (16, 0, 0)).
    pub fn restored(&mut self, mission: u8, kills: i16) {
        self.mission = mission;
        self.kills_to_date = kills;
        self.cursor = if self.mission != 0 {
            NEXT_MISSION
        } else {
            RESTORE
        };
        self.load_menu_colours();
        if self.cursor == NEXT_MISSION {
            self.buffer[3] = BLACK;
        } else {
            self.buffer[2].r = 16;
        }
        self.stage = Stage::Menu;
    }

    /// `UI_LoadMainMenu` colours (§4.2 item 2 of the loader): entries 224..233 := (16, 0, 0),
    /// the cursor's red := 63 (also for cursor 3), fade in, then the cursor's red back to 16 in
    /// the buffer only.
    fn load_menu_colours(&mut self) {
        let c = usize::from(self.cursor);
        self.buffer = [DIM; 10];
        self.buffer[c].r = 63;
        self.dac = self.buffer;
        self.buffer[c].r = 16;
    }

    /// `UI_LoadMainMenu(playMusic)`.
    fn load_menu(&mut self, ctx: &mut FlowContext<'_>, music: bool) {
        if music {
            self.prog.play_cue(ctx, MENU);
        }
        self.load_menu_colours();
        self.stage = Stage::Menu;
    }

    /// The highlight ramps' end states when the cursor moves from `from` (§4.2 item 4; OB-041:
    /// item 3 un-highlights to black, highlights yellow).
    fn move_highlight(&mut self, from: u8) {
        self.dac[usize::from(from)] = if from == NEXT_MISSION { BLACK } else { DIM };
        self.dac[usize::from(self.cursor)] = if self.cursor == NEXT_MISSION {
            YELLOW
        } else {
            RED
        };
    }

    /// The script-choice screen (`UI_ScriptChoice` `04EC:689B`, E3 `04A0:4980`): E1's layout
    /// with one line fewer (screens §3.2).
    fn script_choice(&self) -> StoryScreen {
        let w = self.prog.texts(C::FRONT.script_choice);
        let mut s = StoryScreen::blank(Hold::Key);
        let (fb, pal) = s.parts_mut();
        for (e, (r, g, b)) in [
            (1, (10, 10, 10)),
            (85, (0, 33, 0)),
            (92, (40, 0, 0)),
            (102, (63, 0, 0)),
            (103, (63, 0, 0)),
        ] {
            pal.colors[e] = Rgb6 { r, g, b };
        }
        for y in (38..=50).chain(138..=150) {
            fb.pixels[y * WIDTH..(y + 1) * WIDTH].fill(1);
        }
        let title = line(&w, 0);
        let x = title_text::centred_x(&title);
        for y in [40, 140] {
            title_text::draw(fb, &self.prog.assets.title, &title, (x, y), 85);
        }
        for (i, &(x, y, ink)) in CHOICE_PENS.iter().enumerate() {
            let text = line(&w, i + 1);
            td_render::small_text::draw(fb, &self.prog.assets.small, &text, (x, y), ink);
        }
        s
    }

    fn in_game(&self) -> bool {
        self.mission != 0
    }

    /// Starts the main menu after the start-up (§4.2 entry): song 7 (E2 only), mission 0,
    /// cursor 0, then the loader, which starts song 7 (again).
    fn enter_menu(&mut self, ctx: &mut FlowContext<'_>) {
        if C::FRONT.menu_song_at_entry {
            self.prog.play_cue(ctx, MENU);
        }
        if C::FRONT.loader_flushes {
            self.typed_ahead.clear();
        }
        self.mission = 0;
        self.cursor = 0;
        self.load_menu(ctx, true);
    }

    fn sub(&mut self, s: Result<StoryScreen, StoryError>, back: Back, skip: SkipRule, what: &str) {
        match s {
            Ok(s) => {
                if skip == SkipRule::Card {
                    self.skip = false;
                }
                self.stage = Stage::Sub(s, back, skip);
            }
            Err(e) => tracing::warn!("{what} unavailable: {e}"),
        }
    }

    /// The save (item 1) or restore (item 2) screen over the files as they are now; it gets
    /// the menu's palette buffer (save-screens §4.1).
    fn open_saves(&mut self, item: u8) {
        let (kind, labels) = if item == SAVE {
            let kind = SaveKind::Save {
                mission: self.mission,
                kills: self.kills_to_date,
            };
            (kind, C::FRONT.save_labels)
        } else {
            (SaveKind::Restore, C::FRONT.restore_labels)
        };
        let mut pal = self
            .menu
            .as_ref()
            .map(|(_, _, pal)| pal.clone())
            .unwrap_or_default();
        for (i, &c) in self.buffer.iter().enumerate() {
            pal.colors[ITEM_ENTRY + i] = c;
        }
        let set = SaveSet::load(self.save_dir.as_deref(), C::FRONT.saves);
        let s = SaveScreen::open(
            kind,
            &self.prog.data,
            &self.prog.assets.small,
            &self.prog.texts(labels),
            &pal,
            set,
        );
        self.stage = Stage::Saves(Box::new(s));
    }

    /// The menu after the save or restore screen (§4.2 items 1 and 2): a restore sets the game
    /// and, whenever a game is in progress afterwards (Esc included), the cursor goes to item
    /// 3; a save writes the files and the menu comes back with the cursor on item 1.
    fn saves_done(&mut self, outcome: SaveOutcome) {
        match outcome {
            SaveOutcome::Left => self.restored(self.mission, self.kills_to_date),
            SaveOutcome::Restored { mission, kills } => self.restored(mission, kills),
            SaveOutcome::Saved(set) => {
                match &self.save_dir {
                    Some(dir) => {
                        if let Err(e) = set.store(dir, C::FRONT.saves) {
                            tracing::warn!("saving to {} failed: {e}", dir.display());
                        }
                    }
                    None => tracing::warn!("no save directory; the game was not saved"),
                }
                self.load_menu_colours();
                self.stage = Stage::Menu;
            }
        }
    }

    /// E3's timed waits flush the keyboard when they start (e3-program-flow §3.5).
    fn timed(&self, s: StoryScreen) -> StoryScreen {
        s.flushing_waits(C::FRONT.episode == Episode::E3)
    }

    fn select(&mut self, ctx: &mut FlowContext<'_>) {
        match self.cursor {
            0 if self.in_game() => self.stage = Stage::Confirm(0, 2),
            0 => self.start_block(1, 0, 0),
            NEXT_MISSION => self.start_block(self.mission, self.kills_to_date, NEXT_MISSION),
            RESTORE if self.in_game() => self.stage = Stage::Confirm(RESTORE, 2),
            SAVE | RESTORE => self.open_saves(self.cursor),
            4 => {
                let s = story::help(&self.prog.data);
                self.sub(s, Back::FadeIn, SkipRule::Keep, "help");
            }
            5 => {
                let t = self.prog.texts(C::FRONT.background_story);
                let s = story::story_so_far(&self.prog.data, &self.prog.assets, &t)
                    .map(|s| self.timed(s));
                let back = Back::Reload { music: false };
                self.sub(s, back, SkipRule::Story, "story so far");
            }
            6 | 7 => {
                // In-process pictures with intro.cmf (§4.2 items 6 and 7).
                self.prog.play_cue(ctx, C::FRONT.intro_cue);
                let s = C::picture(&self.prog, self.cursor).map(|s| self.timed(s));
                let back = Back::Reload { music: true };
                self.sub(s, back, SkipRule::Card, "about/order");
            }
            _ => self.stage = Stage::Confirm(QUIT, 1),
        }
    }

    fn start_block(&mut self, mission: u8, kills: i16, item: u8) {
        self.mission = mission;
        self.kills_to_date = kills;
        let start = BlockStart {
            mission,
            kills_to_date: kills,
            skip: self.skip,
            mission_kills: self.mission_kills,
        };
        self.stage = Stage::Campaign(Box::new(C::start(start)), item);
    }

    /// The bookkeeping of §4.2 items 0 and 3 and §4.6 (E3: e3-program-flow §4.6), then the
    /// menu reload.
    fn block_done(&mut self, ctx: &mut FlowContext<'_>, r: BlockResult, item: u8) {
        self.mission_kills = r.mission_kills;
        self.skip = r.skip;
        if r.game_over {
            self.mission = 0;
            if item == NEXT_MISSION {
                self.cursor = 0;
            }
        } else if r.advance {
            if let Some(m) = r.set_mission {
                self.mission = m;
            }
            if !r.replay {
                // A byte: E3 missions above 20 (edited saves) count on until it wraps (OB-051).
                self.mission = self.mission.wrapping_add(1);
            }
            self.kills_to_date = self.kills_to_date.saturating_add(r.mission_kills);
            if item == 0 {
                self.cursor = NEXT_MISSION;
            }
            if self.mission == C::FRONT.complete_at {
                // Episode complete (E2 `0000:3B80`, E3 `0000:369E`): "ordering info".
                self.mission = 0;
                self.cursor = 7;
            }
        }
        // Item 0: Snd_PlayMusic(7) and UI_LoadMainMenu(0); item 3: UI_LoadMainMenu(1).
        self.load_menu(ctx, true);
    }

    fn menu_key(&mut self, ctx: &mut FlowContext<'_>, k: Key) {
        let step = match k {
            Key::UP | KEY_KP8 => ITEMS - 1,
            Key::DOWN | KEY_KP2 => 1,
            k if Some(k) == Key::digit(8) => ITEMS - 1,
            k if Some(k) == Key::digit(2) => 1,
            Key::ENTER | Key::KP_ENTER | Key::SPACE => return self.select(ctx),
            Key::ESCAPE => {
                self.stage = Stage::Confirm(QUIT, 1);
                return;
            }
            _ => return,
        };
        let from = self.cursor;
        loop {
            self.cursor = (self.cursor + step) % ITEMS;
            let hidden = !self.in_game() && (self.cursor == SAVE || self.cursor == NEXT_MISSION);
            if !hidden {
                break;
            }
        }
        self.move_highlight(from);
    }

    /// After a confirmation box: the DAC fades back to the buffer, whose cursor entry was set
    /// to red 63; then the cursor's highlight runs (the box counts as a cursor change).
    fn close_box(&mut self) {
        self.buffer[usize::from(self.cursor)].r = 63;
        self.dac = self.buffer;
        self.move_highlight(self.cursor);
    }
}

impl<C: EpisodeCampaign> Screen for LaterEpisode<C> {
    /// The campaign's flight runs at the flight rate; everything else at the retrace rate.
    fn tick_hz(&self) -> f64 {
        match &self.stage {
            Stage::Campaign(c, _) => c.tick_hz(),
            _ => RETRACE_HZ,
        }
    }

    fn tick(&mut self, ctx: &mut FlowContext<'_>, input: &TickInput) -> Transition {
        match &mut self.stage {
            Stage::Start => {
                ctx.audio.push(AudioCommand::RestartMusicDriver);
                if self.prog.data.resolve(C::FRONT.dialogue_file).is_some() {
                    self.stage = Stage::ScriptChoice(self.script_choice());
                } else {
                    // No notice in E2: script 2 silently (§4.1 step 9).
                    self.prog.choice = ScriptChoice::Edited;
                    self.enter_menu(ctx);
                }
            }
            Stage::ScriptChoice(s) => {
                let chosen = s.last_key().is_some();
                let mut keys = Vec::new();
                for &k in input.pressed.iter().filter(|&&k| story::is_keystroke(k)) {
                    if chosen || !keys.is_empty() {
                        // Typed after the choice: kept for the menu (no flush in E2; the E3
                        // loader flushes them).
                        self.typed_ahead.push(k);
                    } else if Some(k) == Key::digit(1) || Some(k) == Key::digit(2) {
                        keys.push(k);
                    }
                }
                if !chosen && keys.is_empty() {
                    // A controller answers too (M11-04, port addition).
                    keys.extend(crate::front::ui::pad_script_choice(input.pad));
                }
                s.step(&TickInput {
                    pressed: keys,
                    held: input.held.clone(),
                    pad: input.pad,
                });
                if s.finished() {
                    self.prog.choice = if s.last_key() == Key::digit(2) {
                        ScriptChoice::Edited
                    } else {
                        ScriptChoice::Original
                    };
                    self.enter_menu(ctx);
                }
            }
            Stage::Sub(s, back, rule) => {
                let (back, rule) = (*back, *rule);
                s.step(input);
                if s.finished() {
                    if rule != SkipRule::Keep && s.skipped() {
                        self.skip = true;
                    }
                    match back {
                        Back::FadeIn => {
                            self.dac = self.buffer;
                            self.stage = Stage::Menu;
                        }
                        Back::Reload { music } => self.load_menu(ctx, music),
                    }
                }
            }
            Stage::Campaign(c, item) => {
                let item = *item;
                if let Some(r) = c.step(&mut self.prog, ctx, input) {
                    self.block_done(ctx, r, item);
                }
            }
            Stage::Confirm(item, _) => {
                let item = *item;
                if let Some(&k) = input.pressed.iter().find(|&&k| story::is_keystroke(k)) {
                    self.close_box();
                    self.stage = Stage::Menu;
                    if Some(k) == Key::letter('y') {
                        match item {
                            QUIT => {
                                self.cursor = QUIT;
                                self.stage = Stage::Quit;
                            }
                            RESTORE => self.open_saves(RESTORE),
                            _ => self.start_block(1, 0, 0),
                        }
                    }
                }
            }
            Stage::Menu => {
                // The menu reads one key and discards the rest of the BIOS buffer.
                let first = self
                    .typed_ahead
                    .drain(..)
                    .chain(input.pressed.iter().copied())
                    .find(|&k| story::is_keystroke(k));
                if let Some(k) = first {
                    self.menu_key(ctx, k);
                }
            }
            Stage::Saves(s) => {
                s.step(input);
                if let Some(o) = s.outcome() {
                    let o = o.clone();
                    self.saves_done(o);
                }
            }
            Stage::Quit => return Transition::Quit,
        }
        Transition::None
    }

    fn draw(&self, fb: &mut Framebuffer) {
        match &self.stage {
            Stage::ScriptChoice(s) | Stage::Sub(s, ..) => s.draw(fb),
            Stage::Campaign(c, _) => c.draw(fb),
            Stage::Saves(s) => s.draw(fb),
            Stage::Menu | Stage::Confirm(..) | Stage::Start | Stage::Quit => {
                let Some((image, boxes, pal)) = &self.menu else {
                    fb.clear(0);
                    return;
                };
                fb.pixels.copy_from_slice(image);
                fb.palette = pal.clone();
                for (i, &c) in self.dac.iter().enumerate() {
                    fb.palette.colors[ITEM_ENTRY + i] = c;
                }
                if let Stage::Confirm(_, b) = self.stage {
                    // Half brightness from the buffer (cursor entry red 63), entries 192..209
                    // at full; box b of td.men over rows 84..115 (screens §3.1).
                    let mut buffer = self.buffer;
                    buffer[usize::from(self.cursor)].r = 63;
                    for (e, c) in fb.palette.colors.iter_mut().enumerate() {
                        let full = match e.checked_sub(ITEM_ENTRY) {
                            Some(i) if i < 10 => buffer[i],
                            _ => pal.colors[e],
                        };
                        *c = if BOX_ENTRIES.contains(&e) {
                            full
                        } else {
                            Rgb6 {
                                r: full.r / 2,
                                g: full.g / 2,
                                b: full.b / 2,
                            }
                        };
                    }
                    let src = usize::from(b - 1) * 32 * WIDTH;
                    if let Some(rows) = boxes.get(src..src + 32 * WIDTH) {
                        fb.pixels[84 * WIDTH..116 * WIDTH].copy_from_slice(rows);
                    }
                }
            }
        }
    }
}
