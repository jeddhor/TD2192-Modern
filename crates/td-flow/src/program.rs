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
//! 6 and 7 show `aboutus.scr` / `order.scr` in-process instead of halting for the launcher,
//! [`crate::extras`]).
//!
//! The menu screen itself (fades, pulses, keys, boxes) is [`crate::main_menu`]; the save and
//! restore screens (items 1 and 2) are [`crate::save_screens`], with the port's own files in
//! the directory given to [`Episode1::with_save_dir`] (none: saving is not possible).

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use td_assets::{DataDir, Episode, ExeResources};
use td_audio::{AudioCommand, CueRestart, CueTable, MusicCues};
use td_formats::cmf::Cmf;
use td_formats::palette::{Palette, Rgb6};
use td_input::{Key, TickInput};
use td_render::dialogue::Side;
use td_render::{Framebuffer, WIDTH, title_text};

use crate::campaign::{Bg, BlockResult, Campaign, FlightLauncher, Rating, Step, rating};
use crate::dialogue::{DialogueAssets, DialogueScene, ScriptChoice};
use crate::main_menu::{MainMenu, MenuGraphics, NEXT_MISSION, QUIT, Reentry, SAVE};
use crate::save_screens::{E1_FILES, SaveKind, SaveOutcome, SaveScreen, SaveSet};
use crate::screen::{FlowContext, RETRACE_HZ, Screen, Transition};
use crate::story::{self, Hold, ScenePalette, StoryAssets, StoryError, StoryScreen, TextItem};
use td_sim::flight::front_end::FrontEnd;

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

/// Services shared by the menu and the campaign. The screens shared by Episodes 1, 2 and 3
/// (dialogue, retry prompt, debrief, pilot dossier, music) follow [`Program::episode`]
/// (episode-deltas rev 3 §7, §9).
pub struct Program {
    /// The program: `file0001.exe` (E1), `td2.exe` (E2) or `td3.exe` (E3).
    pub episode: Episode,
    pub data: DataDir,
    pub res: ExeResources,
    pub assets: StoryAssets,
    pub choice: ScriptChoice,
    dialogue: Option<DialogueAssets>,
    pub cues: MusicCues,
    songs: BTreeMap<&'static str, Arc<Cmf>>,
    pub launcher: Box<dyn FlightLauncher>,
    /// What the front-end screens of this program run left in DGROUP for the flights: the
    /// scene palette, the lit flag and the dialogue reader's last line (weapons rev 4 §3.3,
    /// OB-017), and block 19's uses of the flight heap before flight 21 (mission-ships rev 6
    /// §4.1.1). The port's program is always started through the launcher flow.
    pub front_end: FrontEnd,
    /// `g_CfgMusic` = 2 (music on): `Snd_PlayMusic` loads its song onto the heap
    /// (episode-program-flow §3.1, §3.2). The port always plays the songs, as the reference
    /// start `/sb` does (the music volume is the mixer's business), so this is true; false
    /// models the original's music-off configuration, whose heap layout is specified
    /// statically only (U-540).
    pub music_on: bool,
}

impl fmt::Debug for Program {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Program")
            .field("choice", &self.choice)
            .finish_non_exhaustive()
    }
}

/// When a music cue restarts the song already playing (music-cues §4.1, OB-012 fix-enhanced):
/// every time under Classic, as the original; with the Enhanced profile's original-bug fixes a
/// looping song requested again keeps playing.
#[must_use]
pub fn cue_restart(settings: &crate::Settings) -> CueRestart {
    if settings.enhanced_fixes() {
        CueRestart::IfDifferent
    } else {
        CueRestart::Always
    }
}

impl Program {
    /// The Episode 1 program's services.
    pub fn new(
        data: &DataDir,
        res: &ExeResources,
        launcher: Box<dyn FlightLauncher>,
    ) -> Result<Self, StoryError> {
        Self::for_episode(data, res, launcher, Episode::E1)
    }

    /// The services of `episode`'s program.
    pub fn for_episode(
        data: &DataDir,
        res: &ExeResources,
        launcher: Box<dyn FlightLauncher>,
        episode: Episode,
    ) -> Result<Self, StoryError> {
        let table = match episode {
            Episode::E1 => CueTable::E1,
            Episode::E2 => CueTable::E2,
            Episode::E3 => CueTable::E3,
        };
        Ok(Self {
            episode,
            data: data.clone(),
            res: res.clone(),
            assets: StoryAssets::load(data)?,
            choice: ScriptChoice::Original,
            dialogue: None,
            cues: MusicCues::with_table(CueRestart::Always, table),
            songs: BTreeMap::new(),
            launcher,
            front_end: FrontEnd::launcher(),
            music_on: true,
        })
    }

    /// Resource id `e1` in Episode 1; `e2.<later>` in Episode 2, `e3.<later>` in Episode 3.
    fn id(&self, e1: &str, later: &str) -> String {
        match self.episode {
            Episode::E1 => e1.to_owned(),
            Episode::E2 => format!("e2.{later}"),
            Episode::E3 => format!("e3.{later}"),
        }
    }

    /// The `0000:OOOO` string of `file0001.exe` (empty if unavailable).
    pub fn text(&self, addr: u16) -> &str {
        self.res.text_at(EXE, usize::from(addr)).unwrap_or_else(|| {
            tracing::warn!("file0001.exe string 0000:{addr:04X} unavailable");
            ""
        })
    }

    pub(crate) fn texts(&self, id: &str) -> Vec<String> {
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

    /// `Snd_PlayMusic(n)`. The Enhanced fixes keep a looping song that is requested again
    /// ([`cue_restart`], OB-012).
    pub fn play_cue(&mut self, ctx: &mut FlowContext<'_>, n: u8) {
        self.cues.set_restart(cue_restart(ctx.settings));
        let load = Self::song_loader(&self.data, &mut self.songs);
        self.cues.play_cue(n, load, ctx.audio);
    }

    /// `g_Palette` (DS:0D2E) as the front-end screens of this program run left it
    /// ([`Program::front_end`]): all black at program start, then what each screen wrote. The
    /// mission title card loads no palette and fades this buffer with only entry 85 changed,
    /// so its fade speed depends on it (screens rev 3 §2.1, text rev 3 §4.5.2, BUG-6); a
    /// dialogue scene starts from it too.
    #[must_use]
    pub fn scene_palette(&self) -> Palette {
        Palette::parse(&self.front_end.scene_palette).unwrap_or_default()
    }

    /// The flight song of flight `n`.
    pub fn play_flight_song(&mut self, ctx: &mut FlowContext<'_>, n: u8) {
        self.cues.set_restart(cue_restart(ctx.settings));
        let load = Self::song_loader(&self.data, &mut self.songs);
        self.cues.play_flight(n, load, ctx.audio);
    }

    /// A dialogue scene of the chosen script (an empty scene if the files are missing).
    pub fn dialogue(&mut self, id: u8) -> DialogueScene {
        if self.dialogue.is_none() {
            match DialogueAssets::load(&self.data, &self.res, self.episode, self.choice) {
                Ok(a) => self.dialogue = Some(a),
                Err(e) => tracing::warn!("dialogue unavailable: {e}"),
            }
        }
        let scene = match &self.dialogue {
            Some(a) => DialogueScene::new(a.clone(), self.episode, id),
            None => DialogueScene::new(
                DialogueAssets {
                    script: Vec::new(),
                    font: self.assets.small.clone(),
                    faces: td_formats::portraits::Portraits { faces: Vec::new() },
                    td_pal: self.assets.td_pal.clone(),
                    prompt: None,
                },
                self.episode,
                id,
            ),
        };
        // The scene overwrites only its face, text and prompt entries of g_Palette.
        scene.with_palette_buffer(&self.scene_palette())
    }

    /// The screen of a story step (text card, scene card, stills, hint, orders, dossier).
    pub fn story(&self, step: Step) -> Result<StoryScreen, StoryError> {
        let a = &self.assets;
        let lines = |s: &[u16; 3]| [self.text(s[0]), self.text(s[1]), self.text(s[2])];
        Ok(match step {
            Step::T(addr) => story::text_card(a, self.text(addr), &self.scene_palette()),
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
    pub(crate) fn page(&self, lines: &[String], hold: Hold) -> StoryScreen {
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
    /// colours 112..223 (screens §3.5; E2 reads `newfaces.td2`, episode-deltas §9.3).
    fn portrait(&self, s: &mut StoryScreen, face: u8) {
        let faces = self
            .data
            .read(crate::dialogue::portrait_file(self.episode))
            .ok()
            .and_then(|b| td_formats::portraits::Portraits::parse(&b).ok());
        let face = crate::dialogue::remap_face(self.episode, face);
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
    /// drawn after it. Any key but Esc flies again. E2 (`04EC:6E62`) and E3 (`04A0:4F47`,
    /// episode-deltas §9.1): taunt 5 has its own text and pen x 50.
    pub fn retry_prompt(&mut self) -> StoryScreen {
        /// Pen x of taunt r (screens §3.7).
        const TAUNT_X: [i32; 10] = [78, 46, 54, 82, 58, 46, 130, 30, 70, 42];
        let r = usize::from(self.launcher.random(10).min(9));
        let t = self.texts(&self.id("retry.taunts", "retry.taunts"));
        let x = if self.episode != Episode::E1 && r == 5 {
            50
        } else {
            TAUNT_X[r]
        };
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
        story::type_now(&mut s, &self.assets.small, &line(&t, r), (x, 90), 40);
        s
    }

    /// The debrief's last line (episode-program-flow §4.5, MDG §9.6.9, U-010): the failure note
    /// when the outcome is 0, else the rating of T = kills to date + kills, where T < 0 or
    /// T > 29,999 gives the original's "cheater" line (D9: text only; impossible in normal
    /// play, the cheat chords do not touch the kills).
    #[must_use]
    pub fn debrief_verdict(&self, kills: i16, kills_to_date: i16, outcome: bool) -> String {
        let (_, cheater, tiers, failure, _) = self.debrief_texts();
        if !outcome {
            return failure;
        }
        match rating(i32::from(kills_to_date) + i32::from(kills)) {
            Rating::Cheater => cheater,
            Rating::Tier(t) => line(&tiers, usize::from(t) - 1),
        }
    }

    /// The debrief texts: labels (7), the cheater line, tiers (11), the failure note and the
    /// note label. E2 and E3 keep them as one run of 21 strings in this order (`e2.debrief`,
    /// `e3.debrief`; E3 has its own failure note, episode-deltas §9.1).
    fn debrief_texts(&self) -> (Vec<String>, String, Vec<String>, String, String) {
        if self.episode != Episode::E1 {
            let all = self.texts(&self.id("", "debrief"));
            let part = |r: std::ops::Range<usize>| all.get(r).map(<[_]>::to_vec);
            return (
                part(0..7).unwrap_or_default(),
                line(&all, 7),
                part(8..19).unwrap_or_default(),
                line(&all, 19),
                line(&all, 20),
            );
        }
        (
            self.texts("debrief.labels"),
            self.text_of("debrief.cheater"),
            self.texts("debrief.tiers"),
            self.text_of("debrief.failure_note"),
            self.text_of("debrief.note_label"),
        )
    }

    /// The debrief (`UI_Debrief` `051A:6FF2`, screens §3.6, episode-program-flow §4.5): face 1;
    /// the rating of T = kills to date + kills, or the failure note when the outcome is 0;
    /// 10 s. Identical in E2; E3 waits 15 s and flushes the keyboard first (episode-deltas
    /// §9.1, e3-program-flow §3.3, §3.5).
    pub fn debrief(
        &self,
        mission: u8,
        kills: i16,
        kills_to_date: i16,
        outcome: bool,
    ) -> StoryScreen {
        let (l, _, _, _, note_label) = self.debrief_texts();
        let get = |i: usize| line(&l, i);
        let total = i32::from(kills_to_date) + i32::from(kills);
        let verdict = self.debrief_verdict(kills, kills_to_date, outcome);
        let items = vec![
            TextItem::typed((20, 45), format!("{}{}{}", get(0), mission, get(1)), 40),
            TextItem::typed((20, 65), get(2), 40),
            TextItem::typed((20, 75), if outcome { get(3) } else { get(4) }, 39).after(500),
            TextItem::typed((20, 95), format!("{}{}", get(5), kills), 40),
            TextItem::typed((20, 105), format!("{}{}", get(6), total), 40).after(500),
            TextItem::typed((20, 130), note_label, 40).after(500),
            TextItem::typed((20, 140), verdict, 39).after(500),
        ];
        let mut s = story::typed_page(
            &self.assets,
            &self.assets.td_pal,
            &[(40, DEBRIEF_GREEN), (39, DEBRIEF_DIM)],
            items,
            Hold::Seconds(if self.episode == Episode::E3 { 15 } else { 10 }),
        )
        .flushing_waits(self.episode == Episode::E3);
        self.portrait(&mut s, 1);
        s
    }

    /// The pilot dossier of ending A (`UI_PlayerDossier` `051A:74B2`, screens §3.5): face 1;
    /// 25 s. E2 (`04EC:712D`), E3 (`04A0:5212`, flushing first): same layout,
    /// `e2.dossier.player` / `e3.dossier.player`.
    pub fn player_dossier(&self, total: i16) -> StoryScreen {
        let t = self.texts(&self.id("e1.dossier.player", "dossier.player"));
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
        )
        .flushing_waits(self.episode == Episode::E3);
        self.portrait(&mut s, 1);
        s
    }
}

/// Line `i` of a resource list ("" if missing).
pub(crate) fn line(list: &[String], i: usize) -> String {
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
    /// The save or restore screen.
    Saves(Box<SaveScreen>),
    Quit,
}

/// The Episode 1 program as a screen.
pub struct Episode1 {
    prog: Program,
    stage: Stage,
    menu: MainMenu,
    mission: u8,
    kills_to_date: i16,
    /// Where the port keeps `td.sav` and `td.stt` (never the game folder).
    save_dir: Option<PathBuf>,
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
            save_dir: None,
        })
    }

    /// Keeps the saved games in `dir` (the user data directory's `saves`), in the original
    /// `td.sav`/`td.stt` format. Without it the restore screen shows empty slots and saving
    /// writes nothing.
    #[must_use]
    pub fn with_save_dir(mut self, dir: PathBuf) -> Self {
        self.save_dir = Some(dir);
        self
    }

    /// The main menu (cursor, palette buffer and screen).
    pub fn menu(&self) -> &MainMenu {
        &self.menu
    }

    /// The campaign state the menu keeps: the next mission (0 = no game) and the kills to date.
    pub fn game(&self) -> (u8, i16) {
        (self.mission, self.kills_to_date)
    }

    /// The save (item 1) or restore (item 2) screen with the files as they are now.
    fn save_screen(&self, kind: SaveKind) -> SaveScreen {
        let id = match kind {
            SaveKind::Save { .. } => "saves.save",
            SaveKind::Restore => "saves.restore",
        };
        let set = SaveSet::load(self.save_dir.as_deref(), E1_FILES);
        SaveScreen::open(
            kind,
            &self.prog.data,
            &self.prog.assets.small,
            &self.prog.texts(id),
            self.menu.palette_buffer(),
            set,
        )
    }

    /// The menu after the save or restore screen (save-screens §4.3–§4.4): a restore sets the
    /// game; a save writes the files; the cursor goes to 3 whenever a game is in progress
    /// after the restore screen, Esc included.
    fn saves_done(&mut self, ctx: &mut FlowContext<'_>, outcome: SaveOutcome) {
        let restore = !matches!(outcome, SaveOutcome::Saved(_));
        match outcome {
            SaveOutcome::Left => {}
            SaveOutcome::Restored { mission, kills } => {
                self.mission = mission;
                self.kills_to_date = kills;
            }
            SaveOutcome::Saved(set) => match &self.save_dir {
                Some(dir) => {
                    if let Err(e) = set.store(dir, E1_FILES) {
                        tracing::warn!("saving to {} failed: {e}", dir.display());
                    }
                }
                None => tracing::warn!("no save directory; the game was not saved"),
            },
        }
        let cursor = if restore && self.in_game() {
            NEXT_MISSION
        } else {
            self.menu.cursor()
        };
        self.enter_menu(ctx, cursor, Reentry::Loader { first: false });
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
            SAVE | 2 => {
                // Both screens load the menu's palette buffer into g_Palette (save-screens
                // §3, weapons rev 4 §3.3 item 5) and end with a fade-out.
                let menu = self.menu.palette_buffer().to_bytes();
                self.prog.front_end.scene_palette.copy_from_slice(&menu);
                self.prog.front_end.lit = false;
                // Both screens flush the BIOS buffer at entry (save-screens §4.1).
                let kind = if item == SAVE {
                    SaveKind::Save {
                        mission: self.mission,
                        kills: self.kills_to_date,
                    }
                } else {
                    SaveKind::Restore
                };
                // OB-056: Esc cancels the name entry with the Enhanced fixes.
                let s = self
                    .save_screen(kind)
                    .esc_cancels_entry(ctx.settings.enhanced_fixes());
                self.stage = Stage::Saves(Box::new(s));
                return;
            }
            // In-process pictures with intro.cmf, as E2/E3 (launcher §4.10). The original
            // halts for the launcher, which runs the extra and then a new program; the port's
            // run goes on, its front-end state included.
            crate::extras::ABOUT | crate::extras::ORDER => {
                crate::extras::about_or_order(&mut self.prog, ctx, item)
                    .map(|s| (s.with_scene_palette(ScenePalette::Untouched), loader))
            }
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
    /// The campaign's flight runs at the flight rate; everything else at the retrace rate
    /// (BUG-5).
    fn tick_hz(&self) -> f64 {
        match &self.stage {
            Stage::Campaign(c) => c.tick_hz(),
            _ => RETRACE_HZ,
        }
    }

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
                let mut keys: Vec<Key> = input
                    .pressed
                    .iter()
                    .copied()
                    .filter(|&k| Some(k) == Key::digit(1) || Some(k) == Key::digit(2))
                    .collect();
                if keys.is_empty() {
                    // A controller answers too (M11-04, port addition).
                    keys.extend(crate::front::ui::pad_script_choice(input.pad));
                }
                s.step(&TickInput {
                    pressed: keys,
                    held: input.held.clone(),
                    pad: input.pad,
                });
                if s.finished() {
                    s.apply_front_end(&mut self.prog.front_end);
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
                    s.apply_front_end(&mut self.prog.front_end);
                    self.enter_menu(ctx, 0, Reentry::Loader { first: true });
                }
            }
            Stage::Sub(s, how) => {
                s.step(input);
                if s.finished() {
                    s.apply_front_end(&mut self.prog.front_end);
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
            Stage::Saves(s) => {
                s.step(input);
                if let Some(o) = s.outcome() {
                    let o = o.clone();
                    self.saves_done(ctx, o);
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
            Stage::Saves(s) => s.draw(fb),
            Stage::Menu | Stage::Start | Stage::Quit => self.menu.draw(fb),
        }
    }
}
