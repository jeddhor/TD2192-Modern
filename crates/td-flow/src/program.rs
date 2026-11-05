//! The Episode 1 program (`file0001.exe` main body): script choice, main menu and the campaign.
//!
//! Spec: `docs/spec/flow/episode-program-flow.md` (rev 2) §4.1–§4.2; screen layouts per
//! `docs/spec/flow/screens.md` (rev 1) §3; menu graphics per `docs/re/FORMATS.md` §12.1 and
//! `docs/re/asset-load-sites.md` §3; music per `docs/spec/audio/music-cues.md` §4.2.
//! Fidelity: T0 for the layouts, T1 for the retry prompt's `Random(10)`, T2 for the order and
//! pacing of screens.
//!
//! Not ported (T3 or decided elsewhere): the text-mode configuration screen and command-line
//! switches (the port has its own settings), joystick calibration, `tdstatus.dat` (menu items
//! 6 and 7 show a "not yet" notice instead of halting for the launcher), save and restore
//! (stubs until D3, M2-12/M11-06).
//!
//! The menu screen itself (fades, pulses, keys, boxes) is [`crate::main_menu`].

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use td_assets::{DataDir, Episode, ExeResources};
use td_audio::{AudioCommand, CueRestart, MusicCues};
use td_formats::cmf::Cmf;
use td_formats::palette::Rgb6;
use td_input::{Key, TickInput};
use td_render::dialogue::Side;
use td_render::{Framebuffer, WIDTH, title_text};

use crate::campaign::{Bg, BlockResult, Campaign, FlightLauncher, Rating, Step, rating};
use crate::dialogue::{DialogueAssets, DialogueScene, ScriptChoice};
use crate::main_menu::{MainMenu, MenuGraphics, NEXT_MISSION, QUIT, Reentry, SAVE};
use crate::screen::{FlowContext, Screen, Transition};
use crate::story::{self, Hold, StoryAssets, StoryError, StoryScreen, TextItem};

const EXE: &str = "file0001.exe";
/// Ink of the port's own notices ("not available yet").
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
            Step::O => self.orders(),
            Step::A => self.enemy_dossier(),
            other => return Err(StoryError::Text(format!("{other:?} is not a story screen"))),
        })
    }

    /// A port notice: lines from (10, 10) down, 9 pixels apart, in white.
    fn page(&self, lines: &[String], hold: Hold) -> StoryScreen {
        let placed: Vec<_> = lines
            .iter()
            .enumerate()
            .map(|(i, l)| ((10, 10 + 9 * i as i32), l.clone()))
            .collect();
        story::text_screen(&self.assets, &placed, TEXT_INK, hold)
    }

    /// A single text resource (`pstring`), or a list joined (empty if unavailable).
    fn text_of(&self, id: &str) -> String {
        self.res
            .text(id)
            .map_or_else(|| self.texts(id).join(" "), str::to_owned)
    }

    /// `Dlg_LoadPortrait(side 1, face, " ", 0)`: the face in the right half of rows 0..99,
    /// colours 112..223 (screens §3.5).
    fn portrait(&self, s: &mut StoryScreen, face: u8) {
        let faces = self
            .data
            .read("newfaces.td")
            .ok()
            .and_then(|b| td_formats::portraits::Portraits::parse(&b).ok());
        let face = crate::dialogue::remap_face(Episode::E1, face);
        let Some(p) = faces.as_ref().and_then(|f| f.by_face_byte(face)) else {
            tracing::warn!("portrait {face} unavailable");
            return;
        };
        let (fb, pal) = s.parts_mut();
        td_render::dialogue::draw_portrait(fb, Side::Right, &p.pixels);
        td_render::dialogue::load_face_palette(pal, Side::Right, &p.palette);
    }

    /// Assignment orders (`UI_AssignmentOrders` `051A:62FD`, screens §3.4): a frame of
    /// one-pixel rectangles, the lines typed in green; 5 s.
    fn orders(&self) -> StoryScreen {
        let t = self.texts("e1.assignment_orders");
        let ys = [45, 51, 57, 69, 75, 81, 93, 99, 117];
        let items = ys
            .iter()
            .enumerate()
            .map(|(i, &y)| TextItem::typed((40, y), line(&t, i), 40))
            .collect();
        let mut s = story::typed_page(
            &self.assets,
            &self.assets.td_pal,
            &[(40, GREEN)],
            items,
            Hold::Seconds(5),
        );
        let (fb, _) = s.parts_mut();
        for k in 0..=25 {
            let colour = match k {
                0..=5 => 5 * k,
                6..=19 => 25,
                _ => 5 * (25 - k),
            };
            outline(fb, (k, k), (319 - k, 199 - k), colour as u8);
        }
        outline(fb, (275, 185), (285, 190), 16);
        outline(fb, (279, 187), (281, 188), 40);
        s
    }

    /// The enemy dossier (`UI_EnemyDossier` `051A:6653`, screens §3.5): face 6; 10 s.
    fn enemy_dossier(&self) -> StoryScreen {
        let t = self.texts("e1.dossier.enemy");
        let ys = [75, 91, 97, 103, 109, 115, 121, 137, 143];
        let mut items: Vec<TextItem> = ys
            .iter()
            .enumerate()
            .map(|(i, &y)| TextItem::typed((40, y), line(&t, i), 40))
            .collect();
        items.push(TextItem::typed((40, 109), line(&t, 9), 39));
        items.push(TextItem::typed((40, 160), line(&t, 10), 40));
        let mut s = story::typed_page(
            &self.assets,
            &self.assets.td_pal,
            &[(40, GREEN), (39, RED)],
            items,
            Hold::Seconds(10),
        );
        self.portrait(&mut s, 6);
        s
    }

    /// The retry prompt (`UI_RetryPrompt` `051A:6A33`, screens §3.7): `Random(10)` from the
    /// program's generator picks the taunt, typed before the fade-in; the two key lines are
    /// drawn after it. Any key but Esc flies again.
    pub fn retry_prompt(&mut self) -> StoryScreen {
        /// Pen x of taunt r (screens §3.7).
        const TAUNT_X: [i32; 10] = [78, 46, 54, 82, 58, 46, 130, 30, 70, 42];
        let r = usize::from(self.launcher.random(10).min(9));
        let t = self.texts("retry.taunts");
        let items = vec![
            TextItem::drawn((86, 110), line(&t, 10), 39),
            TextItem::drawn((82, 117), line(&t, 11), 39),
        ];
        let mut s = story::typed_page(
            &self.assets,
            &self.assets.td_pal,
            &[(40, DEBRIEF_GREEN), (39, DEBRIEF_DIM)],
            items,
            Hold::Key,
        );
        story::type_now(
            &mut s,
            &self.assets.small,
            &line(&t, r),
            (TAUNT_X[r], 90),
            40,
        );
        s
    }

    /// The debrief (`UI_Debrief` `051A:6FF2`, screens §3.6, episode-program-flow §4.5): face 1;
    /// the rating of T = kills to date + kills, or the failure note when the outcome is 0;
    /// 10 s.
    pub fn debrief(
        &self,
        mission: u8,
        kills: i16,
        kills_to_date: i16,
        outcome: bool,
    ) -> StoryScreen {
        let l = self.texts("debrief.labels");
        let get = |i: usize| line(&l, i);
        let total = i32::from(kills_to_date) + i32::from(kills);
        let verdict = if !outcome {
            self.text_of("debrief.failure_note")
        } else {
            match rating(total) {
                Rating::Cheater => self.text_of("debrief.cheater"),
                Rating::Tier(t) => line(&self.texts("debrief.tiers"), usize::from(t) - 1),
            }
        };
        let items = vec![
            TextItem::typed((20, 45), format!("{}{}{}", get(0), mission, get(1)), 40),
            TextItem::typed((20, 65), get(2), 40),
            TextItem::typed((20, 75), if outcome { get(3) } else { get(4) }, 39).after(500),
            TextItem::typed((20, 95), format!("{}{}", get(5), kills), 40),
            TextItem::typed((20, 105), format!("{}{}", get(6), total), 40).after(500),
            TextItem::typed((20, 130), self.text_of("debrief.note_label"), 40).after(500),
            TextItem::typed((20, 140), verdict, 39).after(500),
        ];
        let mut s = story::typed_page(
            &self.assets,
            &self.assets.td_pal,
            &[(40, DEBRIEF_GREEN), (39, DEBRIEF_DIM)],
            items,
            Hold::Seconds(10),
        );
        self.portrait(&mut s, 1);
        s
    }

    /// The pilot dossier of ending A (`UI_PlayerDossier` `051A:74B2`, screens §3.5): face 1;
    /// 25 s.
    pub fn player_dossier(&self, total: i16) -> StoryScreen {
        let t = self.texts("e1.dossier.player");
        let items = vec![
            TextItem::typed((20, 45), line(&t, 0), 40),
            TextItem::typed((20, 91), line(&t, 1), 40).after(500),
            TextItem::typed((20, 97), line(&t, 2), 40).after(500),
            TextItem::typed((20, 109), line(&t, 3), 40).after(500),
            TextItem::typed((20, 121), format!("{}{}", line(&t, 4), total), 40).after(500),
            TextItem::typed((20, 109), line(&t, 5), 39).after(2000),
            TextItem::typed((20, 160), line(&t, 6), 40).after(500),
        ];
        let mut s = story::typed_page(
            &self.assets,
            &self.assets.td_pal,
            &[(40, GREEN), (39, RED)],
            items,
            Hold::Seconds(25),
        );
        self.portrait(&mut s, 1);
        s
    }
}

/// Line `i` of a resource list ("" if missing).
fn line(list: &[String], i: usize) -> String {
    list.get(i).cloned().unwrap_or_default()
}

/// A one-pixel rectangle outline (`Gfx_DrawRect` `051A:609D`).
fn outline(fb: &mut Framebuffer, (x0, y0): (i32, i32), (x1, y1): (i32, i32), c: u8) {
    for x in x0..=x1 {
        fb.put(x, y0, c);
        fb.put(x, y1, c);
    }
    for y in y0..=y1 {
        fb.put(x0, y, c);
        fb.put(x1, y, c);
    }
}

/// Inks of the typed pages (screens §3.4–§3.7).
const GREEN: Rgb6 = Rgb6 { r: 0, g: 63, b: 0 };
const RED: Rgb6 = Rgb6 { r: 63, g: 0, b: 0 };
const DEBRIEF_GREEN: Rgb6 = Rgb6 {
    r: 15,
    g: 55,
    b: 15,
};
const DEBRIEF_DIM: Rgb6 = Rgb6 { r: 0, g: 40, b: 0 };

enum Stage {
    Start,
    ScriptChoice(StoryScreen),
    Notice(StoryScreen),
    /// The main menu ([`MainMenu`] animates it).
    Menu,
    Campaign(Box<Campaign>),
    /// A screen started from the menu, and how the menu comes back after it.
    Sub(StoryScreen, Reentry),
    Quit,
}

/// The Episode 1 program as a screen.
pub struct Episode1 {
    prog: Program,
    stage: Stage,
    menu: MainMenu,
    mission: u8,
    kills_to_date: i16,
}

impl fmt::Debug for Episode1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Episode1")
            .field("cursor", &self.menu.cursor())
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
        let gfx = MenuGraphics::load(data);
        if gfx.is_none() {
            tracing::warn!("td.men or tdmen.pal unavailable; the menu is drawn black");
        }
        Ok(Self {
            prog: Program::new(data, res, launcher)?,
            stage: Stage::Start,
            menu: MainMenu::new(gfx),
            mission: 0,
            kills_to_date: 0,
        })
    }

    /// The main menu (cursor, palette buffer and screen).
    pub fn menu(&self) -> &MainMenu {
        &self.menu
    }

    /// The palette of the script choice and the missing-dialogue notice (screens §3.2): all
    /// black but 1 (grey), 85 (green), 92 (dark red), 102 and 103 (red).
    fn choice_screen() -> StoryScreen {
        let mut s = StoryScreen::blank(Hold::Key);
        let (_, pal) = s.parts_mut();
        for (e, (r, g, b)) in [
            (1, (10, 10, 10)),
            (85, (0, 33, 0)),
            (92, (40, 0, 0)),
            (102, (63, 0, 0)),
            (103, (63, 0, 0)),
        ] {
            pal.colors[e] = Rgb6 { r, g, b };
        }
        s
    }

    /// The script-choice screen (`UI_ScriptChoice` `051A:527F`, screens §3.2).
    fn script_choice(&self) -> StoryScreen {
        let w = self.prog.texts("script_choice.warning");
        let mut s = Self::choice_screen();
        let (fb, _) = s.parts_mut();
        for y in (38..=50).chain(138..=150) {
            fb.pixels[y * WIDTH..(y + 1) * WIDTH].fill(1);
        }
        let title = line(&w, 0);
        let x = title_text::centred_x(&title);
        for y in [40, 140] {
            title_text::draw(fb, &self.prog.assets.title, &title, (x, y), 85);
        }
        let pens = [
            (66, 62, 92),
            (54, 69, 92),
            (42, 76, 92),
            (38, 83, 92),
            (66, 90, 92),
            (66, 110, 102),
            (90, 117, 102),
        ];
        for (i, &(x, y, ink)) in pens.iter().enumerate() {
            let text = line(&w, i + 1);
            td_render::small_text::draw(fb, &self.prog.assets.small, &text, (x, y), ink);
        }
        s
    }

    /// The missing-dialogue notice (`051A:56D5`, screens §3.2): any key.
    fn missing_dialogue(&self) -> StoryScreen {
        let t = self.prog.texts("script_choice.missing_dialogue");
        let mut s = Self::choice_screen();
        let (fb, _) = s.parts_mut();
        let pens = [
            (46, 70, 92),
            (46, 77, 92),
            (46, 84, 92),
            (38, 91, 92),
            (30, 98, 92),
            (54, 105, 92),
            (66, 117, 102),
        ];
        for (i, &(x, y, ink)) in pens.iter().enumerate() {
            let text = line(&t, i);
            td_render::small_text::draw(fb, &self.prog.assets.small, &text, (x, y), ink);
        }
        s
    }

    /// Shows the menu again (episode-program-flow §4.2): the loader plays the menu song.
    fn enter_menu(&mut self, ctx: &mut FlowContext<'_>, cursor: u8, how: Reentry) {
        if matches!(how, Reentry::Loader { .. }) {
            self.prog.play_cue(ctx, td_audio::music_cues::MENU);
        }
        self.menu.enter(cursor, self.in_game(), how);
        self.stage = Stage::Menu;
    }

    fn in_game(&self) -> bool {
        self.mission != 0
    }

    fn notice(&self, text: &str) -> StoryScreen {
        self.prog.page(&[text.to_owned()], Hold::Key)
    }

    /// The action of `item` once the menu has faded out (episode-program-flow §4.2).
    fn select(&mut self, ctx: &mut FlowContext<'_>, item: u8) {
        let loader = Reentry::Loader { first: false };
        // Keys typed during the fade-out are still in the BIOS buffer (screens §2.2).
        let keys = self.menu.take_keys();
        let sub = match item {
            0 => {
                self.start_block(1, 0, keys);
                return;
            }
            NEXT_MISSION => {
                self.start_block(self.mission, self.kills_to_date, keys);
                return;
            }
            QUIT => {
                self.stage = Stage::Quit;
                return;
            }
            4 => story::help(&self.prog.data).map(|s| (s, Reentry::Refade)),
            5 => {
                // ASSUMPTION(U-302): the menu comes back as from the loader, without the song.
                let t = self.prog.texts("e1.background_story");
                story::story_so_far(&self.prog.data, &self.prog.assets, &t)
                    .map(|s| (s, Reentry::Story))
            }
            SAVE | 2 => Ok((
                self.notice("save and restore are not available yet"),
                loader,
            )),
            _ => Ok((self.notice("this program is not available yet"), loader)),
        };
        match sub {
            Ok((mut s, how)) => {
                s.type_ahead(keys);
                self.stage = Stage::Sub(s, how);
            }
            Err(e) => {
                tracing::warn!("menu item {item} unavailable: {e}");
                self.enter_menu(ctx, item, loader);
            }
        }
    }

    fn start_block(&mut self, mission: u8, kills: i16, keys: Vec<Key>) {
        self.mission = mission;
        self.kills_to_date = kills;
        let c = Campaign::new(mission, kills).with_type_ahead(keys);
        self.stage = Stage::Campaign(Box::new(c));
    }

    /// The menu bookkeeping after a block (§4.2); returns the new cursor.
    fn block_done(&mut self, r: BlockResult) -> u8 {
        if r.game_over {
            self.mission = 0;
            return 0;
        }
        if !r.advance {
            return self.menu.cursor();
        }
        self.kills_to_date = self.kills_to_date.saturating_add(r.mission_kills);
        self.mission += 1;
        if self.mission == 21 {
            // Episode finished (§4.2 item 3; §5: the cursor jump to item 7 is kept).
            self.mission = 0;
            return 7;
        }
        NEXT_MISSION
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
                    Stage::Notice(self.missing_dialogue())
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
                    pad: input.pad,
                });
                if s.finished() {
                    self.prog.choice = if s.last_key() == Key::digit(2) {
                        ScriptChoice::Edited
                    } else {
                        ScriptChoice::Original
                    };
                    // §4.1 step 9: the key buffer is flushed, then the menu starts.
                    self.enter_menu(ctx, 0, Reentry::Loader { first: true });
                }
            }
            Stage::Notice(s) => {
                s.step(input);
                if s.finished() {
                    self.enter_menu(ctx, 0, Reentry::Loader { first: true });
                }
            }
            Stage::Sub(s, how) => {
                s.step(input);
                if s.finished() {
                    let how = *how;
                    self.enter_menu(ctx, self.menu.cursor(), how);
                }
            }
            Stage::Campaign(c) => {
                if let Some(r) = c.step(&mut self.prog, ctx, input) {
                    let cursor = self.block_done(r);
                    self.enter_menu(ctx, cursor, Reentry::Loader { first: false });
                }
            }
            Stage::Menu => {
                if let Some(item) = self.menu.step(input) {
                    self.select(ctx, item);
                }
            }
            Stage::Quit => return Transition::Quit,
        }
        Transition::None
    }

    fn draw(&self, fb: &mut Framebuffer) {
        match &self.stage {
            Stage::ScriptChoice(s) | Stage::Notice(s) | Stage::Sub(s, _) => s.draw(fb),
            Stage::Campaign(c) => c.draw(fb),
            Stage::Menu | Stage::Start | Stage::Quit => self.menu.draw(fb),
        }
    }
}
