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
//! settings and shell). Save and restore (`td2.stt`/`td2.sav`, D3) show a "not yet" notice
//! until the port's save files exist; [`Episode2::restored`] applies the menu bookkeeping of a
//! restore for them.
//!
//! The menu is shown without its fades and highlight ramps (T2); the colours of entries
//! 224..233 are the ramps' end states, kept as the original keeps them: the palette buffer
//! (`g_MenuPalette`) and the DAC separately.

use std::fmt;

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
use crate::scenes_e2::{self, Bg};
use crate::screen::{FlowContext, Screen, Transition};
use crate::story::{self, Hold, StoryError, StoryScreen};

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

enum Stage {
    Start,
    ScriptChoice(StoryScreen),
    Menu,
    /// A block started from item 0 or item 3.
    Campaign(Box<CampaignE2>, u8),
    Sub(StoryScreen, Back),
    /// A confirmation box (1 = quit, 2 = current game lost) for an item.
    Confirm(u8, u8),
    Quit,
}

/// The menu image, the confirmation boxes (320 × 32 each) and `tdmen.pal`.
type MenuGraphics = (Box<[u8]>, Box<[u8]>, Palette);

/// The Episode 2 program as a screen.
pub struct Episode2 {
    prog: Program,
    stage: Stage,
    cursor: u8,
    mission: u8,
    kills_to_date: i16,
    menu: Option<MenuGraphics>,
    /// `g_MenuPalette` entries 224..233.
    buffer: [Rgb6; 10],
    /// DAC entries 224..233.
    dac: [Rgb6; 10],
    /// Keys typed after the script choice (no flush in E2, §4.1).
    typed_ahead: Vec<Key>,
}

impl fmt::Debug for Episode2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Episode2")
            .field("cursor", &self.cursor)
            .field("mission", &self.mission)
            .field("kills_to_date", &self.kills_to_date)
            .finish_non_exhaustive()
    }
}

impl Episode2 {
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
            prog: Program::for_episode(data, res, launcher, Episode::E2)?,
            stage: Stage::Start,
            cursor: 0,
            mission: 0,
            kills_to_date: 0,
            menu,
            buffer: [DIM; 10],
            dac: [DIM; 10],
            typed_ahead: Vec::new(),
        };
        ep.load_menu_colours();
        Ok(ep)
    }

    /// Mission (0 = no game), kills to date and cursor.
    pub fn state(&self) -> (u8, i16, u8) {
        (self.mission, self.kills_to_date, self.cursor)
    }

    /// DAC entries 224..233 (the item colours) as shown.
    pub fn item_colours(&self) -> [Rgb6; 10] {
        self.dac
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

    /// The script-choice screen (`UI_ScriptChoice` `04EC:689B`): E1's layout with one line
    /// fewer (screens §3.2).
    fn script_choice(&self) -> StoryScreen {
        let w = self.prog.texts("e2.script_choice.warning");
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

    /// Starts the main menu after the start-up (§4.2 entry): song 7, mission 0, cursor 0, then
    /// the loader, which starts song 7 again.
    fn enter_menu(&mut self, ctx: &mut FlowContext<'_>) {
        self.prog.play_cue(ctx, MENU);
        self.mission = 0;
        self.cursor = 0;
        self.load_menu(ctx, true);
    }

    fn sub(&mut self, s: Result<StoryScreen, StoryError>, back: Back, what: &str) {
        match s {
            Ok(s) => self.stage = Stage::Sub(s, back),
            Err(e) => tracing::warn!("{what} unavailable: {e}"),
        }
    }

    fn notice(&mut self, text: &str) {
        let s = self.prog.page(&[text.to_owned()], Hold::Key);
        self.stage = Stage::Sub(s, Back::FadeIn);
    }

    fn select(&mut self, ctx: &mut FlowContext<'_>) {
        match self.cursor {
            0 if self.in_game() => self.stage = Stage::Confirm(0, 2),
            0 => self.start_block(1, 0, 0),
            NEXT_MISSION => self.start_block(self.mission, self.kills_to_date, NEXT_MISSION),
            RESTORE if self.in_game() => self.stage = Stage::Confirm(RESTORE, 2),
            SAVE | RESTORE => self.notice("save and restore are not available yet"),
            4 => {
                let s = story::help(&self.prog.data);
                self.sub(s, Back::FadeIn, "help");
            }
            5 => {
                let t = self.prog.texts("e2.background_story");
                let s = story::story_so_far(&self.prog.data, &self.prog.assets, &t);
                self.sub(s, Back::Reload { music: false }, "story so far");
            }
            6 | 7 => {
                // In-process pictures with intro.cmf (§4.2 items 6 and 7).
                self.prog.play_cue(ctx, E2_INTRO);
                let bg = if self.cursor == 6 {
                    Bg::Aboutus
                } else {
                    Bg::Order
                };
                let s = scenes_e2::scene_card(&self.prog.data, &self.prog.assets, bg, ["", "", ""]);
                self.sub(s, Back::Reload { music: true }, "about/order");
            }
            _ => self.stage = Stage::Confirm(QUIT, 1),
        }
    }

    fn start_block(&mut self, mission: u8, kills: i16, item: u8) {
        self.mission = mission;
        self.kills_to_date = kills;
        self.stage = Stage::Campaign(Box::new(CampaignE2::new(mission, kills)), item);
    }

    /// The bookkeeping of §4.2 items 0 and 3 and §4.6, then the menu reload.
    fn block_done(&mut self, ctx: &mut FlowContext<'_>, r: BlockResult, item: u8) {
        if r.game_over {
            self.mission = 0;
            if item == NEXT_MISSION {
                self.cursor = 0;
            }
        } else if r.advance {
            if !r.replay {
                self.mission += 1;
            }
            self.kills_to_date = self.kills_to_date.saturating_add(r.mission_kills);
            if item == 0 {
                self.cursor = NEXT_MISSION;
            }
            if self.mission == 21 {
                // Episode complete (`0000:3B80`): "ordering info".
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

impl Screen for Episode2 {
    fn tick(&mut self, ctx: &mut FlowContext<'_>, input: &TickInput) -> Transition {
        match &mut self.stage {
            Stage::Start => {
                ctx.audio.push(AudioCommand::RestartMusicDriver);
                if self.prog.data.resolve("dialogu2.dat").is_some() {
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
                        // Typed after the choice: kept for the menu (no flush in E2).
                        self.typed_ahead.push(k);
                    } else if Some(k) == Key::digit(1) || Some(k) == Key::digit(2) {
                        keys.push(k);
                    }
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
            Stage::Sub(s, back) => {
                let back = *back;
                s.step(input);
                if s.finished() {
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
                            RESTORE => self.notice("save and restore are not available yet"),
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
            Stage::Quit => return Transition::Quit,
        }
        Transition::None
    }

    fn draw(&self, fb: &mut Framebuffer) {
        match &self.stage {
            Stage::ScriptChoice(s) | Stage::Sub(s, _) => s.draw(fb),
            Stage::Campaign(c, _) => c.draw(fb),
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
