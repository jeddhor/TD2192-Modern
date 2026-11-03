//! The Episode 1 program (`file0001.exe` main body): script choice, main menu and the campaign.
//!
//! Spec: `docs/spec/flow/episode-program-flow.md` (rev 1) §4.1–§4.2; menu graphics per
//! `docs/re/FORMATS.md` §12.1 and `docs/re/asset-load-sites.md` §3; music per
//! `docs/spec/audio/music-cues.md` §4.2. Fidelity: T2 for the order of screens.
//!
//! Not ported (T3 or decided elsewhere): the text-mode configuration screen and command-line
//! switches (the port has its own settings), joystick calibration, `tdstatus.dat` (menu items
//! 6 and 7 show a "not yet" notice instead of halting for the launcher), save and restore
//! (stubs until D3, M2-12/M11-06).
//!
//! Provisional (ASSUMPTION(U-120)): how the menu shows the cursor. From C-0002 frame 997
//! (cursor on item 0, no game): item k is drawn in palette entry 224 + k; the selected item is
//! (63, 0, 0), the others (16, 0, 0), and "next mission" is black while no game is in progress.
//! The menu is shown without its fades.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use td_assets::{DataDir, Episode, ExeResources};
use td_audio::{AudioCommand, CueRestart, MusicCues};
use td_formats::cmf::Cmf;
use td_formats::palette::{Palette, Rgb6};
use td_input::{Key, TickInput};
use td_render::{Framebuffer, WIDTH, title_text};

use crate::campaign::{Bg, BlockResult, Campaign, FlightLauncher, Rating, Step, rating};
use crate::dialogue::{DialogueAssets, DialogueScene, ScriptChoice};
use crate::screen::{FlowContext, Screen, Transition};
use crate::story::{self, Hold, StoryAssets, StoryError, StoryScreen};

const EXE: &str = "file0001.exe";
/// Ink of the provisional text-only screens (ASSUMPTION(U-121)).
const TEXT_INK: (u8, Rgb6) = (
    255,
    Rgb6 {
        r: 63,
        g: 63,
        b: 63,
    },
);

/// Services shared by the menu and the campaign.
pub struct Program {
    pub data: DataDir,
    pub res: ExeResources,
    pub assets: StoryAssets,
    pub choice: ScriptChoice,
    dialogue: Option<DialogueAssets>,
    pub cues: MusicCues,
    songs: BTreeMap<&'static str, Arc<Cmf>>,
    pub launcher: Box<dyn FlightLauncher>,
}

impl fmt::Debug for Program {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Program")
            .field("choice", &self.choice)
            .finish_non_exhaustive()
    }
}

impl Program {
    pub fn new(
        data: &DataDir,
        res: &ExeResources,
        launcher: Box<dyn FlightLauncher>,
    ) -> Result<Self, StoryError> {
        Ok(Self {
            data: data.clone(),
            res: res.clone(),
            assets: StoryAssets::load(data)?,
            choice: ScriptChoice::Original,
            dialogue: None,
            cues: MusicCues::new(CueRestart::Always),
            songs: BTreeMap::new(),
            launcher,
        })
    }

    /// The `0000:OOOO` string of `file0001.exe` (empty if unavailable).
    pub fn text(&self, addr: u16) -> &str {
        self.res.text_at(EXE, usize::from(addr)).unwrap_or_else(|| {
            tracing::warn!("file0001.exe string 0000:{addr:04X} unavailable");
            ""
        })
    }

    fn texts(&self, id: &str) -> Vec<String> {
        self.res
            .texts(id)
            .map(<[String]>::to_vec)
            .unwrap_or_default()
    }

    fn song_loader<'a>(
        data: &'a DataDir,
        songs: &'a mut BTreeMap<&'static str, Arc<Cmf>>,
    ) -> impl FnOnce(&'static str) -> Option<Arc<Cmf>> + 'a {
        move |name| {
            if let Some(s) = songs.get(name) {
                return Some(Arc::clone(s));
            }
            let song = Arc::new(Cmf::parse(&data.read(name).ok()?).ok()?);
            songs.insert(name, Arc::clone(&song));
            Some(song)
        }
    }

    /// `Snd_PlayMusic(n)`.
    pub fn play_cue(&mut self, ctx: &mut FlowContext<'_>, n: u8) {
        let load = Self::song_loader(&self.data, &mut self.songs);
        self.cues.play_cue(n, load, ctx.audio);
    }

    /// The flight song of flight `n`.
    pub fn play_flight_song(&mut self, ctx: &mut FlowContext<'_>, n: u8) {
        let load = Self::song_loader(&self.data, &mut self.songs);
        self.cues.play_flight(n, load, ctx.audio);
    }

    /// A dialogue scene of the chosen script (an empty scene if the files are missing).
    pub fn dialogue(&mut self, id: u8) -> DialogueScene {
        if self.dialogue.is_none() {
            match DialogueAssets::load(&self.data, &self.res, Episode::E1, self.choice) {
                Ok(a) => self.dialogue = Some(a),
                Err(e) => tracing::warn!("dialogue unavailable: {e}"),
            }
        }
        match &self.dialogue {
            Some(a) => DialogueScene::new(a.clone(), Episode::E1, id),
            None => DialogueScene::new(
                DialogueAssets {
                    script: Vec::new(),
                    font: self.assets.small.clone(),
                    faces: td_formats::portraits::Portraits { faces: Vec::new() },
                    td_pal: self.assets.td_pal.clone(),
                    prompt: None,
                },
                Episode::E1,
                id,
            ),
        }
    }

    /// The screen of a story step (text card, scene card, stills, hint, orders, dossier).
    pub fn story(&self, step: Step) -> Result<StoryScreen, StoryError> {
        let a = &self.assets;
        let lines = |s: &[u16; 3]| [self.text(s[0]), self.text(s[1]), self.text(s[2])];
        Ok(match step {
            Step::T(addr) => story::text_card(a, self.text(addr)),
            Step::C(Bg::Pic4, s) => story::pic4_card(&self.data, a, lines(&s))?,
            Step::C(Bg::Td1_18, s) => story::still_18(&self.data, a, lines(&s))?,
            Step::C(bg, s) => {
                story::scene_card(&self.data, a, bg.file().unwrap_or_default(), lines(&s))?
            }
            Step::P(set, s) => story::still_set(&self.data, a, set, lines(&s))?,
            Step::P15 => story::still_15(&self.data)?,
            Step::H(n) => story::hint(&self.data, a, &self.texts("e1.hints"), n)?,
            Step::O => self.page(&self.texts("e1.assignment_orders"), Hold::Key),
            Step::A => self.page(&self.texts("e1.dossier.enemy"), Hold::Key),
            other => return Err(StoryError::Text(format!("{other:?} is not a story screen"))),
        })
    }

    /// A provisional text page: lines from (10, 10) down, 9 pixels apart (ASSUMPTION(U-121)).
    fn page(&self, lines: &[String], hold: Hold) -> StoryScreen {
        let placed: Vec<_> = lines
            .iter()
            .enumerate()
            .map(|(i, l)| ((10, 10 + 9 * i as i32), l.clone()))
            .collect();
        story::text_screen(&self.assets, &placed, TEXT_INK, hold)
    }

    /// The retry prompt (`051A:6A33`): a taunt and the two key lines; Esc declines.
    /// ASSUMPTION(U-124): which taunt the original picks is not specified; the port cycles
    /// through them by attempt (no random number is drawn).
    pub fn retry_prompt(&self, attempts: u32) -> StoryScreen {
        let t = self.texts("retry.taunts");
        let n = t.len().saturating_sub(2).max(1);
        let pick = t.get(attempts as usize % n).cloned().unwrap_or_default();
        let lines = [pick, String::new(), t.get(n).cloned().unwrap_or_default()]
            .into_iter()
            .chain(t.get(n + 1).cloned())
            .collect::<Vec<_>>();
        self.page(&lines, Hold::Key)
    }

    /// The debrief (`051A:6FF2`, §4.5): labels, kills, and the rating of
    /// T = kills to date + kills, or the failure note when the outcome is 0. 10 s or a key.
    pub fn debrief(
        &self,
        mission: u8,
        kills: i16,
        kills_to_date: i16,
        outcome: bool,
    ) -> StoryScreen {
        let l = self.texts("debrief.labels");
        let get = |i: usize| l.get(i).cloned().unwrap_or_default();
        let total = i32::from(kills_to_date) + i32::from(kills);
        let verdict = if !outcome {
            self.texts("debrief.failure_note").join(" ")
        } else {
            match rating(total) {
                Rating::Cheater => self.texts("debrief.cheater").join(" "),
                Rating::Tier(t) => self
                    .texts("debrief.tiers")
                    .get(usize::from(t) - 1)
                    .cloned()
                    .unwrap_or_default(),
            }
        };
        let lines = vec![
            get(2),
            format!("{}{}{}", get(0), mission, get(1)),
            if outcome { get(3) } else { get(4) },
            String::new(),
            format!("{}{}", get(5), kills),
            format!("{}{}", get(6), total),
            String::new(),
            verdict,
        ];
        self.page(&lines, Hold::Seconds(10))
    }

    /// The pilot dossier of ending A (`051A:74B2`).
    pub fn player_dossier(&self, total: i16) -> StoryScreen {
        let mut lines = self.texts("e1.dossier.player");
        lines.push(total.to_string());
        self.page(&lines, Hold::Key)
    }
}

/// Menu items (§4.2).
const ITEMS: u8 = 9;
const NEXT_MISSION: u8 = 3;
const SAVE: u8 = 1;
/// First palette entry of the item colours (FORMATS §12.1).
const ITEM_ENTRY: usize = 224;

enum Stage {
    Start,
    ScriptChoice(StoryScreen),
    Notice(StoryScreen),
    Menu,
    Campaign(Box<Campaign>),
    Sub(StoryScreen),
    /// A confirmation box (1 = quit, 2 = current game lost) for an item.
    Confirm(u8, u8),
    Quit,
}

/// The Episode 1 program as a screen.
pub struct Episode1 {
    prog: Program,
    stage: Stage,
    cursor: u8,
    mission: u8,
    kills_to_date: i16,
    menu: Option<MenuGraphics>,
}

/// The menu image, the four confirmation boxes (320x32 each) and `tdmen.pal`.
type MenuGraphics = (Box<[u8]>, Box<[u8]>, Palette);

impl fmt::Debug for Episode1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Episode1")
            .field("cursor", &self.cursor)
            .field("mission", &self.mission)
            .finish_non_exhaustive()
    }
}

impl Episode1 {
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
        Ok(Self {
            prog: Program::new(data, res, launcher)?,
            stage: Stage::Start,
            cursor: 0,
            mission: 0,
            kills_to_date: 0,
            menu,
        })
    }

    /// The script-choice screen (`051A:527F`). ASSUMPTION(U-121): layout provisional.
    fn script_choice(&self) -> StoryScreen {
        let w = self.prog.texts("script_choice.warning");
        let mut s = self.prog.page(&[], Hold::Key);
        let (fb, pal) = s.parts_mut();
        pal.colors[102] = Rgb6 { r: 63, g: 0, b: 0 };
        pal.colors[85] = Rgb6 { r: 0, g: 33, b: 0 };
        pal.colors[92] = Rgb6 { r: 40, g: 0, b: 0 };
        let title = w.first().cloned().unwrap_or_default();
        let x = title_text::centred_x(&title);
        title_text::draw(fb, &self.prog.assets.title, &title, (x, 20), 102);
        for (i, line) in w.iter().enumerate().skip(1) {
            let ink = if i >= 6 { 92 } else { 85 };
            let y = if i >= 6 {
                150 + 12 * (i as i32 - 6)
            } else {
                50 + 12 * (i as i32 - 1)
            };
            let x = 160 - (5 * line.chars().count() as i32 + 1) / 2;
            td_render::small_text::draw(fb, &self.prog.assets.small, line, (x, y), ink);
        }
        s
    }

    fn enter_menu(&mut self, ctx: &mut FlowContext<'_>) {
        self.prog.play_cue(ctx, td_audio::music_cues::MENU);
        self.stage = Stage::Menu;
    }

    fn in_game(&self) -> bool {
        self.mission != 0
    }

    fn notice(&self, text: &str) -> StoryScreen {
        self.prog.page(&[text.to_owned()], Hold::Key)
    }

    fn select(&mut self, ctx: &mut FlowContext<'_>) {
        match self.cursor {
            0 if self.in_game() => self.stage = Stage::Confirm(0, 2),
            0 => self.start_block(1, 0),
            NEXT_MISSION => self.start_block(self.mission, self.kills_to_date),
            4 => match story::help(&self.prog.data) {
                Ok(s) => self.stage = Stage::Sub(s),
                Err(e) => tracing::warn!("help unavailable: {e}"),
            },
            5 => {
                let t = self.prog.texts("e1.background_story");
                self.stage = Stage::Sub(self.prog.page(&t, Hold::Key));
            }
            8 => self.stage = Stage::Confirm(8, 1),
            SAVE | 2 => {
                self.stage = Stage::Sub(self.notice("save and restore are not available yet"))
            }
            _ => self.stage = Stage::Sub(self.notice("this program is not available yet")),
        }
        let _ = ctx;
    }

    fn start_block(&mut self, mission: u8, kills: i16) {
        self.mission = mission;
        self.kills_to_date = kills;
        self.stage = Stage::Campaign(Box::new(Campaign::new(mission, kills)));
    }

    fn block_done(&mut self, r: BlockResult) {
        if r.game_over {
            self.mission = 0;
            self.cursor = 0;
        } else if r.advance {
            self.kills_to_date = self.kills_to_date.saturating_add(r.mission_kills);
            self.mission += 1;
            self.cursor = NEXT_MISSION;
            if self.mission == 21 {
                // Episode finished (§4.2 item 3; §5: the cursor jump to item 7 is kept).
                self.mission = 0;
                self.cursor = 7;
            }
        }
    }

    fn menu_keys(&mut self, ctx: &mut FlowContext<'_>, input: &TickInput) {
        for &k in &input.pressed {
            let step = match k {
                Key::UP => ITEMS - 1,
                Key::DOWN => 1,
                Key::ENTER | Key::KP_ENTER | Key::SPACE => {
                    self.select(ctx);
                    return;
                }
                Key::ESCAPE => {
                    self.stage = Stage::Confirm(8, 1);
                    return;
                }
                _ => continue,
            };
            loop {
                self.cursor = (self.cursor + step) % ITEMS;
                let hidden =
                    !self.in_game() && (self.cursor == SAVE || self.cursor == NEXT_MISSION);
                if !hidden {
                    break;
                }
            }
        }
    }
}

impl Screen for Episode1 {
    fn tick(&mut self, ctx: &mut FlowContext<'_>, input: &TickInput) -> Transition {
        match &mut self.stage {
            Stage::Start => {
                // The intro program's song state does not carry over (IM-2, music-cues §4.2).
                ctx.audio.push(AudioCommand::RestartMusicDriver);
                self.stage = if self.prog.data.resolve("dialogue.dat").is_some() {
                    Stage::ScriptChoice(self.script_choice())
                } else {
                    self.prog.choice = ScriptChoice::Edited;
                    let t = self.prog.texts("script_choice.missing_dialogue");
                    Stage::Notice(self.prog.page(&t, Hold::Key))
                };
            }
            Stage::ScriptChoice(s) => {
                let keys: Vec<Key> = input
                    .pressed
                    .iter()
                    .copied()
                    .filter(|&k| Some(k) == Key::digit(1) || Some(k) == Key::digit(2))
                    .collect();
                s.step(&TickInput {
                    pressed: keys,
                    held: input.held.clone(),
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
            Stage::Notice(s) | Stage::Sub(s) => {
                s.step(input);
                if s.finished() {
                    self.enter_menu(ctx);
                }
            }
            Stage::Campaign(c) => {
                if let Some(r) = c.step(&mut self.prog, ctx, input) {
                    self.block_done(r);
                    self.enter_menu(ctx);
                }
            }
            Stage::Confirm(item, _) => {
                let item = *item;
                if let Some(&k) = input.pressed.iter().find(|&&k| story::is_keystroke(k)) {
                    self.stage = Stage::Menu;
                    if Some(k) == Key::letter('y') {
                        match item {
                            8 => self.stage = Stage::Quit,
                            _ => self.start_block(1, 0),
                        }
                    }
                }
            }
            Stage::Menu => self.menu_keys(ctx, input),
            Stage::Quit => return Transition::Quit,
        }
        Transition::None
    }

    fn draw(&self, fb: &mut Framebuffer) {
        match &self.stage {
            Stage::ScriptChoice(s) | Stage::Notice(s) | Stage::Sub(s) => s.draw(fb),
            Stage::Campaign(c) => c.draw(fb),
            Stage::Menu | Stage::Confirm(..) | Stage::Start | Stage::Quit => {
                let Some((image, boxes, pal)) = &self.menu else {
                    fb.clear(0);
                    return;
                };
                fb.pixels.copy_from_slice(image);
                fb.palette = pal.clone();
                for k in 0..ITEMS {
                    let c = if k == self.cursor {
                        Rgb6 { r: 63, g: 0, b: 0 }
                    } else if k == NEXT_MISSION && !self.in_game() {
                        Rgb6::default()
                    } else {
                        Rgb6 { r: 16, g: 0, b: 0 }
                    };
                    fb.palette.colors[ITEM_ENTRY + usize::from(k)] = c;
                }
                fb.palette.colors[ITEM_ENTRY + 9] = Rgb6 { r: 16, g: 0, b: 0 };
                if let Stage::Confirm(_, b) = self.stage {
                    // Box b of td.men over menu rows 84..115 (0000:330E).
                    let src = usize::from(b - 1) * 32 * WIDTH;
                    if let Some(rows) = boxes.get(src..src + 32 * WIDTH) {
                        fb.pixels[84 * WIDTH..116 * WIDTH].copy_from_slice(rows);
                    }
                }
            }
        }
    }
}
