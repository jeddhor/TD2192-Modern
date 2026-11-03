//! The dialogue scene player (`Dlg_PlayScene`): two portraits, names, typed lines that scroll,
//! page waits and fades, for one scene of a dialogue script.
//!
//! Spec: `docs/spec/narrative/dialogue-player.md` (rev 1); text per `docs/spec/render/text.md`
//! (rev 1). Fidelity: T0 for layout, palette and text; T1 for the order of events; T2 for the
//! typing and fade pace, which is CPU-bound in the original (U-062).
//!
//! The screen ticks once per vertical retrace ([`RETRACE_HZ`]). Every wait of the original
//! becomes a whole number of ticks: one per typed character
//! (`td_render::small_text::FRAMES_PER_TYPED_CHAR`) and two per fade level
//! ([`FADE_FRAMES_PER_LEVEL`]). Everything between two waits happens at once inside one tick,
//! in the original's order. A page break waits for a key; keys pressed while text is typing or
//! fading are kept, like the BIOS type-ahead buffer, and taken by the next page break.
//!
//! The screen keeps its own video memory and two palettes, as the original does: the palette
//! buffer (`g_Palette`, always full brightness) and the DAC (what is shown: faded, dimmed or
//! highlighted). DAC writes made while the screen is dark are left to the next fade-in.
//!
//! Not modelled: the joystick button that also ends a page wait (§4.7; there is no joystick
//! input yet), and palette entries left over from the previous screen (224..239, 255 and the
//! unused text entries start black here; no pixel uses them).

use std::collections::VecDeque;
use std::fmt;

use td_assets::{AssetError, DataDir, Episode, ExeResources};
use td_formats::FormatError;
use td_formats::dialogue::{Command, read_scene};
use td_formats::font::{Font, FontKind};
use td_formats::palette::Palette;
use td_formats::portraits::Portraits;
use td_input::{Key, TickInput};
use td_render::dialogue::{self, Side};
use td_render::small_text::Typewriter;
use td_render::{Framebuffer, fade};
use thiserror::Error;

use crate::screen::{FlowContext, RETRACE_HZ, Screen, Transition};

/// Face code meaning "load no portrait". Spec: dialogue-player.md §3.3, §4.4 step 5.
pub const NO_PORTRAIT: u8 = 200;
/// First text palette entry of a scene. Spec: §3.4.
pub const FIRST_TEXT_ENTRY: u8 = 250;
/// Where the text entry allocator wraps to. Spec: §3.4.
pub const TEXT_ENTRY_WRAP: u8 = 240;
/// Ticks per fade level: 17 levels, one per two frames at 70 Hz, about 34 frames per fade as
/// at preset P1 (C-0002 frames 2186..2219). Spec: §5 item 6 (T2).
pub const FADE_FRAMES_PER_LEVEL: u32 = 2;
/// Keystrokes the BIOS type-ahead buffer holds (16 words, one always free); further keys are
/// dropped. Spec: §4.7 (keys typed during text stay in the BIOS buffer).
pub const KEY_BUFFER: usize = 15;

/// Which script the player chose at the start (DS:0D28): 1 = original, 2 = edited.
/// Spec: dialogue-player.md §4.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptChoice {
    /// `dialogue.dat`, `dialogu2.dat`, `dialogu3.dat`.
    Original,
    /// `safedial.dat`, `safedia2.dat`, `safedia3.dat`.
    Edited,
}

/// Errors while loading a scene.
#[derive(Debug, Error)]
pub enum DialogueError {
    #[error(transparent)]
    Asset(#[from] AssetError),
    #[error(transparent)]
    Format(#[from] FormatError),
    #[error("bad dialogue scene {0:?}: expected ID, e1:ID, e2:ID, e3:ID or e1-safe:ID (ID 0..255)")]
    BadSpec(String),
}

/// The script file of an episode.
pub fn script_file(episode: Episode, choice: ScriptChoice) -> &'static str {
    match (episode, choice) {
        (Episode::E1, ScriptChoice::Original) => "dialogue.dat",
        (Episode::E1, ScriptChoice::Edited) => "safedial.dat",
        (Episode::E2, ScriptChoice::Original) => "dialogu2.dat",
        (Episode::E2, ScriptChoice::Edited) => "safedia2.dat",
        (Episode::E3, ScriptChoice::Original) => "dialogu3.dat",
        (Episode::E3, ScriptChoice::Edited) => "safedia3.dat",
    }
}

/// The portrait file of an episode (FORMATS §10).
pub fn portrait_file(episode: Episode) -> &'static str {
    match episode {
        Episode::E1 => "newfaces.td",
        Episode::E2 => "newfaces.td2",
        Episode::E3 => "newfaces.td3",
    }
}

/// The per-episode face-code remap applied before a portrait is loaded (§3.3).
pub fn remap_face(episode: Episode, face: u8) -> u8 {
    match episode {
        Episode::E1 => match face {
            78 => 13,
            65 => 10,
            66 => 14,
            32 => 9,
            f => f,
        },
        Episode::E2 => match face {
            78 => 13,
            33 => 10,
            32 => 9,
            f => f,
        },
        // Applied in sequence, each rule to the result of the previous ones.
        Episode::E3 => [
            (27, 13),
            (28, 27),
            (33, 27),
            (89, 16),
            (90, 17),
            (87, 18),
            (126, 10),
            (32, 9),
        ]
        .iter()
        .fold(face, |f, &(from, to)| if f == from { to } else { f }),
    }
}

/// Last text entry before the allocator wraps: 253 in E1, 254 in E2 and E3 (§3.4, §4.9).
fn last_text_entry(episode: Episode) -> u8 {
    if episode == Episode::E1 { 253 } else { 254 }
}

/// Parses a developer scene spec: `ID` (episode 1, original script), `eN:ID` or
/// `eN-safe:ID` (edited script).
pub fn parse_scene_spec(spec: &str) -> Result<(Episode, ScriptChoice, u8), DialogueError> {
    let bad = || DialogueError::BadSpec(spec.to_owned());
    let (prefix, id) = spec.rsplit_once(':').unwrap_or(("e1", spec));
    let (ep, choice) = match prefix.strip_suffix("-safe") {
        Some(ep) => (ep, ScriptChoice::Edited),
        None => (prefix, ScriptChoice::Original),
    };
    let episode = match ep {
        "e1" => Episode::E1,
        "e2" => Episode::E2,
        "e3" => Episode::E3,
        _ => return Err(bad()),
    };
    Ok((episode, choice, id.parse().map_err(|_| bad())?))
}

/// The files a scene needs, already decoded.
#[derive(Clone)]
pub struct DialogueAssets {
    /// The whole script file.
    pub script: Vec<u8>,
    /// `td.lt2`.
    pub font: Font,
    /// The episode's portraits.
    pub faces: Portraits,
    /// `td.pal`, the source of the speakers' text colours (§4.2 step 2).
    pub td_pal: Palette,
    /// The scene-1 prompt (episode 1 only), from `file0001.exe`.
    pub prompt: Option<String>,
}

impl fmt::Debug for DialogueAssets {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DialogueAssets")
            .field("script_bytes", &self.script.len())
            .field("faces", &self.faces.faces.len())
            .field("prompt", &self.prompt.is_some())
            .finish_non_exhaustive()
    }
}

impl DialogueAssets {
    /// Loads the files for `episode` and `choice` from the original installation.
    pub fn load(
        data: &DataDir,
        resources: &ExeResources,
        episode: Episode,
        choice: ScriptChoice,
    ) -> Result<Self, DialogueError> {
        let prompt = (episode == Episode::E1)
            .then(|| resources.text("dialogue.prompt").map(str::to_owned))
            .flatten();
        if episode == Episode::E1 && prompt.is_none() {
            tracing::warn!("dialogue prompt unavailable (file0001.exe resource map)");
        }
        Ok(Self {
            script: data.read(script_file(episode, choice))?,
            font: Font::parse(&data.read("td.lt2")?, FontKind::Small)?,
            faces: Portraits::parse(&data.read(portrait_file(episode))?)?,
            td_pal: Palette::parse(&data.read("td.pal")?)?,
            prompt,
        })
    }
}

/// A fade in progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FadeRun {
    /// Next level to write (0..=16).
    level: u8,
    fading_in: bool,
    /// End the scene when the fade is done (Esc at a page wait).
    then_end: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum State {
    /// Reading the next script line.
    Run,
    /// Typing a line; when it is done the text area scrolls.
    Typing(Typewriter),
    Fading(FadeRun),
    /// Page break: waiting for a key.
    WaitKey,
    /// The scene is over.
    Done,
}

/// One dialogue scene as a [`Screen`]. It pops itself when the scene ends.
pub struct DialogueScene {
    episode: Episode,
    id: u8,
    assets: DialogueAssets,
    commands: Vec<Command>,
    next: usize,
    /// Video memory and the DAC (its palette).
    screen: Framebuffer,
    /// The palette buffer (`g_Palette`).
    pal: Palette,
    /// `g_ScreenLit`: set by a fade-in, cleared by a fade-out.
    lit: bool,
    /// Speaker colour byte of the left and right slots.
    speaker_color: [u8; 2],
    /// Text entry allocated for each colour byte.
    entry_of: [Option<u8>; 256],
    next_entry: u8,
    /// Frames still to wait: the scene resumes `wait` ticks after the one that set it.
    wait: u32,
    keys: VecDeque<Key>,
    skip: bool,
    state: State,
}

impl fmt::Debug for DialogueScene {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DialogueScene")
            .field("episode", &self.episode)
            .field("id", &self.id)
            .field("next", &self.next)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

/// True for keys that put a keystroke into the BIOS buffer: not the modifiers, and not the keys
/// the port keeps for the host (F10 scale, F11 fullscreen, F12 screenshot).
fn is_keystroke(k: Key) -> bool {
    !matches!(
        k,
        Key::LSHIFT | Key::RSHIFT | Key::LCTRL | Key::RCTRL | Key::LALT | Key::RALT
    ) && !matches!(k, Key::F10 | Key::F11 | Key::F12)
}

impl DialogueScene {
    /// Loads and prepares scene `id` of `episode`'s script.
    pub fn load(
        data: &DataDir,
        resources: &ExeResources,
        episode: Episode,
        choice: ScriptChoice,
        id: u8,
    ) -> Result<Self, DialogueError> {
        let assets = DialogueAssets::load(data, resources, episode, choice)?;
        Ok(Self::new(assets, episode, id))
    }

    /// Prepares scene `id` from loaded files (§4.2). A scene ID that is not in the script
    /// plays as an empty scene (port policy for the original's endless search, §5 item 5).
    pub fn new(assets: DialogueAssets, episode: Episode, id: u8) -> Self {
        let commands = read_scene(&assets.script, id).unwrap_or_else(|| {
            tracing::warn!("dialogue scene {id} not found; playing nothing");
            Vec::new()
        });
        Self {
            episode,
            id,
            assets,
            commands,
            next: 0,
            // Gfx_InitUnchained: video memory cleared, every DAC entry black.
            screen: Framebuffer::new(),
            pal: Palette::default(),
            lit: false,
            speaker_color: [0; 2],
            entry_of: [None; 256],
            next_entry: FIRST_TEXT_ENTRY,
            wait: 0,
            keys: VecDeque::new(),
            skip: false,
            state: State::Run,
        }
    }

    /// The video memory and DAC as shown.
    pub fn screen(&self) -> &Framebuffer {
        &self.screen
    }

    /// The palette buffer (full-brightness colours).
    pub fn palette_buffer(&self) -> &Palette {
        &self.pal
    }

    /// True while a page break waits for a key.
    pub fn waiting_for_key(&self) -> bool {
        self.state == State::WaitKey
    }

    /// True once the scene has ended.
    pub fn finished(&self) -> bool {
        self.state == State::Done
    }

    /// True if Esc ended the scene early.
    pub fn skipped(&self) -> bool {
        self.skip
    }

    /// Advances one frame: takes new keys, then runs the scene until its next wait.
    pub fn step(&mut self, input: &TickInput) {
        for &k in &input.pressed {
            if is_keystroke(k) && self.keys.len() < KEY_BUFFER {
                self.keys.push_back(k);
            }
        }
        self.wait = self.wait.saturating_sub(1);
        while self.wait == 0 {
            match &mut self.state {
                State::Done => return,
                State::Run => self.run_line(),
                State::Typing(t) => {
                    if let Some(frames) = t.step(&mut self.screen, &self.assets.font) {
                        self.wait = frames;
                        continue;
                    }
                    // §4.5 step 5.
                    dialogue::scroll_text_area(&mut self.screen);
                    self.state = State::Run;
                }
                State::Fading(f) => {
                    let run = *f;
                    fade::write_level(&mut self.screen.palette, &self.pal, run.level);
                    let last = if run.fading_in { 16 } else { 0 };
                    if run.level != last {
                        f.level = if run.fading_in {
                            run.level + 1
                        } else {
                            run.level - 1
                        };
                        self.wait = FADE_FRAMES_PER_LEVEL;
                        continue;
                    }
                    if run.fading_in {
                        self.lit = true;
                    }
                    self.state = if run.then_end {
                        State::Done
                    } else {
                        State::Run
                    };
                }
                State::WaitKey => match self.keys.pop_front() {
                    None => return,
                    Some(Key::ESCAPE) => {
                        self.skip = true;
                        self.start_fade(false, true);
                    }
                    Some(_) => self.state = State::Run,
                },
            }
        }
    }

    /// Starts a fade (§4.6): each level, the first included, is written after a wait.
    fn start_fade(&mut self, fading_in: bool, then_end: bool) {
        if !fading_in {
            self.lit = false;
        }
        self.state = State::Fading(FadeRun {
            level: if fading_in { 0 } else { 16 },
            fading_in,
            then_end,
        });
        self.wait = FADE_FRAMES_PER_LEVEL;
    }

    /// Reads and performs the next script line (§4.3).
    fn run_line(&mut self) {
        let Some(cmd) = self.commands.get(self.next).cloned() else {
            // The line starting with `d` (or the end of the file): close the file and stop.
            self.state = State::Done;
            return;
        };
        self.next += 1;
        match cmd {
            Command::Speaker {
                slot,
                face,
                color,
                name,
            } => self.speaker(slot, face, color, &name),
            Command::Line { color, text } => self.line(color, &text),
            Command::SegmentStart => {
                if self.episode == Episode::E1
                    && self.id == 1
                    && let Some(prompt) = &self.assets.prompt
                {
                    self.pal.colors[usize::from(dialogue::PROMPT_ENTRY)] = dialogue::PROMPT_RGB;
                    dialogue::draw_prompt(&mut self.screen, &self.assets.font, prompt);
                }
                self.start_fade(true, false);
            }
            Command::SegmentEnd => self.start_fade(false, false),
            Command::PageBreak => self.state = State::WaitKey,
        }
    }

    /// A speaker definition (§4.4).
    fn speaker(&mut self, slot: u8, face: u8, color: u8, name: &str) {
        let side = Side::of_slot(slot);
        self.speaker_color[side.index()] = color;
        let face = remap_face(self.episode, face);
        let entry = self.next_entry;
        self.next_entry = if entry >= last_text_entry(self.episode) {
            TEXT_ENTRY_WRAP
        } else {
            entry + 1
        };
        let e = usize::from(entry);
        self.pal.colors[e] = self.assets.td_pal.colors[usize::from(color)];
        self.entry_of[usize::from(color)] = Some(entry);
        if self.lit {
            self.screen.palette.colors[e] = self.pal.colors[e];
        }
        if face == NO_PORTRAIT {
            return;
        }
        let Some(portrait) = self.assets.faces.by_face_byte(face) else {
            tracing::warn!("dialogue scene {}: no portrait {face}", self.id);
            return;
        };
        dialogue::load_face_palette(&mut self.pal, side, &portrait.palette);
        if self.lit {
            dialogue::dim_both_faces(&mut self.screen.palette, &self.pal);
        }
        dialogue::draw_portrait(&mut self.screen, side, &portrait.pixels);
        dialogue::draw_name(&mut self.screen, &self.assets.font, side, name, entry);
    }

    /// A line (§4.5): highlight the speaker, then start typing.
    fn line(&mut self, color: u8, text: &str) {
        let (speaker, ink_color) = if color == self.speaker_color[0] {
            (Side::Left, color)
        } else if self.episode == Episode::E1 {
            // E1 draws any other colour in slot 2's colour (§5 item 1).
            (Side::Right, self.speaker_color[1])
        } else {
            (Side::Right, color)
        };
        // A colour never given to a speaker: not in the shipped scripts; use slot 2's entry.
        let ink = self.entry_of[usize::from(ink_color)]
            .or(self.entry_of[usize::from(self.speaker_color[1])])
            .unwrap_or(0);
        dialogue::highlight(&mut self.screen.palette, &self.pal, speaker);
        self.state = State::Typing(Typewriter::new(text, dialogue::TEXT_PEN, ink, 0));
    }
}

impl Screen for DialogueScene {
    fn tick_hz(&self) -> f64 {
        RETRACE_HZ
    }

    fn tick(&mut self, _ctx: &mut FlowContext<'_>, input: &TickInput) -> Transition {
        self.step(input);
        if self.finished() {
            Transition::Pop
        } else {
            Transition::None
        }
    }

    fn draw(&self, fb: &mut Framebuffer) {
        fb.pixels.copy_from_slice(&self.screen.pixels);
        fb.palette.clone_from(&self.screen.palette);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use td_formats::palette::Rgb6;
    use td_formats::portraits::{FACE_HEIGHT, FACE_WIDTH, Portrait};

    fn script(lines: &[&[u8]]) -> Vec<u8> {
        let mut v = Vec::new();
        for l in lines {
            v.extend_from_slice(l);
            v.extend_from_slice(b"\r\n");
        }
        v
    }

    /// Synthetic files: every small-font glyph solid; face n has pixel value n everywhere and
    /// colour n = (n, n, n) at entry n; td.pal entry c = (c / 4, 0, 0).
    fn assets(script_bytes: Vec<u8>) -> DialogueAssets {
        let faces = (1..=3u8)
            .map(|n| {
                let mut palette = Palette::default();
                palette.colors[usize::from(n)] = Rgb6 { r: n, g: n, b: n };
                palette.colors[0] = Rgb6 { r: 1, g: 2, b: 3 };
                Portrait {
                    palette,
                    pixels: vec![n; FACE_WIDTH * FACE_HEIGHT].into_boxed_slice(),
                }
            })
            .collect();
        let mut td_pal = Palette::default();
        for (i, c) in td_pal.colors.iter_mut().enumerate() {
            c.r = (i / 4) as u8;
        }
        DialogueAssets {
            script: script_bytes,
            font: Font::parse(&vec![0xF7; FontKind::Small.file_len()], FontKind::Small).unwrap(),
            faces: Portraits { faces },
            td_pal,
            prompt: Some("hit space".into()),
        }
    }

    fn press(k: Key) -> TickInput {
        TickInput {
            pressed: vec![k],
            ..TickInput::default()
        }
    }

    /// Ticks with no input until `done` holds; returns the ticks taken.
    fn run_until(s: &mut DialogueScene, done: fn(&DialogueScene) -> bool) -> u32 {
        let mut n = 0;
        while !done(s) {
            s.step(&TickInput::default());
            n += 1;
            assert!(n < 10_000, "scene stuck");
        }
        n
    }

    fn basic_scene() -> DialogueScene {
        let data = script(&[
            b"\x06\x01",
            b"\x01\x01\x01\x40left",
            b"\x01\x02\x02\x80right",
            b"\x03",
            b"\x02\x01\x80ab",
            b"\x05",
            b"\x02\x01\x40c",
            b"\x05",
            b"\x04",
            b"d",
        ]);
        DialogueScene::new(assets(data), Episode::E1, 1)
    }

    #[test]
    fn plays_a_scene_with_its_timing() {
        let mut s = basic_scene();
        // Tick 1: both speakers, the prompt, the fade-in starts (screen still dark).
        s.step(&TickInput::default());
        assert_eq!(
            s.screen().get(0, 0),
            Some(1),
            "left portrait drawn while dark"
        );
        assert_eq!(s.screen().get(160, 0), Some(2 + 112));
        assert_eq!(s.screen().palette.colors[1], Rgb6::default());
        assert_eq!(s.palette_buffer().colors[250], Rgb6 { r: 16, g: 0, b: 0 });
        assert_eq!(s.palette_buffer().colors[251], Rgb6 { r: 32, g: 0, b: 0 });
        assert_eq!(s.palette_buffer().colors[254], dialogue::PROMPT_RGB);
        assert!(
            s.screen().pixels[192 * 320..197 * 320].contains(&254),
            "prompt"
        );
        // Fade-in: 17 levels, each after a 2-tick wait; the last level and the first character
        // come together, 34 ticks after the one that started the fade.
        let fade = run_until(&mut s, |s| s.lit);
        assert_eq!(fade, 2 * 17);
        // The first line ("ab", slot 2) has started: one character typed in that tick, with
        // the cursor; left face dim, right face full.
        assert_eq!(s.screen().palette.colors[1], Rgb6::default(), "1 >> 1 = 0");
        assert_eq!(s.screen().palette.colors[114], Rgb6 { r: 2, g: 2, b: 2 });
        assert_eq!(s.screen().palette.colors[112], Rgb6 { r: 1, g: 2, b: 3 });
        assert_eq!(s.screen().get(4, 176), Some(251), "'a' in slot 2's entry");
        assert_eq!(s.screen().get(8, 176), Some(251), "cursor");
        // Second character, then the closing space; the scroll comes with the next tick.
        s.step(&TickInput::default());
        assert_eq!(s.screen().get(8, 176), Some(251), "'b'");
        assert_eq!(s.screen().get(12, 176), Some(0), "cursor erased");
        assert!(!s.waiting_for_key());
        s.step(&TickInput::default());
        assert!(s.waiting_for_key());
        assert_eq!(s.screen().get(4, 169), Some(251), "scrolled up 7 rows");
        assert_eq!(s.screen().get(4, 176), Some(0));
        // Waiting: nothing changes without a key; modifiers are not keystrokes.
        let before = s.screen().clone();
        s.step(&press(Key::LSHIFT));
        assert!(s.waiting_for_key());
        assert_eq!(*s.screen(), before);
        // Any key: the next line (slot 1) is typed at once, in the left entry (250).
        s.step(&press(Key::SPACE));
        assert_eq!(s.screen().get(4, 176), Some(250));
        assert_eq!(s.screen().palette.colors[1], Rgb6 { r: 1, g: 1, b: 1 });
        assert_eq!(s.screen().palette.colors[114], Rgb6 { r: 1, g: 1, b: 1 });
        run_until(&mut s, DialogueScene::waiting_for_key);
        // The last page: the fade-out (17 levels) and the end of the scene.
        s.step(&press(Key::ENTER));
        let out = run_until(&mut s, DialogueScene::finished);
        assert_eq!(out, 2 * 17);
        assert!(!s.skipped() && !s.lit);
        assert!(
            s.screen().palette.colors[1..]
                .iter()
                .all(|c| *c == Rgb6::default())
        );
    }

    #[test]
    fn keys_typed_ahead_are_kept_and_esc_skips_the_rest() {
        let mut s = basic_scene();
        // Esc pressed during the fade-in waits in the buffer for the first page break.
        s.step(&press(Key::ESCAPE));
        run_until(&mut s, |s| s.skipped());
        assert!(!s.finished());
        let out = run_until(&mut s, DialogueScene::finished);
        assert_eq!(out, 2 * 17, "fade-out, then the scene ends");
        assert_eq!(
            s.screen().get(4, 176),
            Some(0),
            "the second line was never typed"
        );
    }

    #[test]
    fn text_entries_rotate_and_unmatched_lines_use_slot_2() {
        let mut lines: Vec<&[u8]> = vec![b"\x06\x05"];
        let speakers: Vec<Vec<u8>> = (0..6u8)
            .map(|i| vec![1, 1 + (i % 2), 200, 10 + i])
            .collect();
        lines.extend(speakers.iter().map(Vec::as_slice));
        lines.extend([&b"\x03"[..], &b"\x02\x01\x63x"[..], &b"\x04"[..], &b"d"[..]]);
        let mut s = DialogueScene::new(assets(script(&lines)), Episode::E1, 5);
        run_until(&mut s, |s| s.lit);
        // 250, 251, 252, 253, then 240, 241 (E1 wraps after 253).
        for (i, e) in [250, 251, 252, 253, 240, 241].into_iter().enumerate() {
            let c = 10 + i;
            assert_eq!(s.entry_of[c], Some(e));
            assert_eq!(s.pal.colors[usize::from(e)], s.assets.td_pal.colors[c]);
        }
        // Colour 63h matches no speaker: slot 2's colour (the last right speaker, 15) -> 241.
        assert_eq!(s.screen().get(5, 176), Some(241));
        // Face 200: nothing drawn.
        assert!(s.screen().pixels[..100 * 320].iter().all(|&p| p == 0));
    }

    #[test]
    fn episodes_2_and_3_use_254_and_the_line_s_own_colour() {
        let mut lines: Vec<&[u8]> = vec![b"\x06\x05"];
        let speakers: Vec<Vec<u8>> = (0..6u8)
            .map(|i| vec![1, 1 + (i % 2), 200, 10 + i])
            .collect();
        lines.extend(speakers.iter().map(Vec::as_slice));
        // Colour 12 belonged to an earlier left speaker; slot 1 is now colour 14.
        lines.extend([&b"\x03"[..], &b"\x02\x01\x0cx"[..], &b"\x04"[..], &b"d"[..]]);
        for episode in [Episode::E2, Episode::E3] {
            let mut s = DialogueScene::new(assets(script(&lines)), episode, 5);
            run_until(&mut s, |s| s.lit);
            for (i, e) in [250, 251, 252, 253, 254, 240].into_iter().enumerate() {
                assert_eq!(s.entry_of[10 + i], Some(e), "{episode}");
            }
            assert_eq!(
                s.screen().get(5, 176),
                Some(252),
                "{episode}: entry of colour 12"
            );
        }
    }

    #[test]
    fn face_codes_remap_per_episode() {
        assert_eq!(remap_face(Episode::E1, 32), 9);
        assert_eq!(remap_face(Episode::E1, 65), 10);
        assert_eq!(remap_face(Episode::E1, 66), 14);
        assert_eq!(remap_face(Episode::E1, 78), 13);
        assert_eq!(remap_face(Episode::E1, 33), 33);
        assert_eq!(remap_face(Episode::E2, 33), 10);
        assert_eq!(remap_face(Episode::E2, 66), 66);
        assert_eq!(remap_face(Episode::E3, 27), 13);
        assert_eq!(remap_face(Episode::E3, 28), 27, "not remapped again to 13");
        assert_eq!(remap_face(Episode::E3, 33), 27);
        assert_eq!(remap_face(Episode::E3, 89), 16);
        assert_eq!(remap_face(Episode::E3, 90), 17);
        assert_eq!(remap_face(Episode::E3, 87), 18);
        assert_eq!(remap_face(Episode::E3, 126), 10);
        assert_eq!(remap_face(Episode::E3, 32), 9);
        assert_eq!(remap_face(Episode::E3, 200), 200);
    }

    #[test]
    fn missing_scene_plays_nothing() {
        let mut s = DialogueScene::new(assets(script(&[b"\x06\x01", b"d"])), Episode::E2, 9);
        s.step(&TickInput::default());
        assert!(s.finished());
    }

    #[test]
    fn scene_specs() {
        assert_eq!(
            parse_scene_spec("1").unwrap(),
            (Episode::E1, ScriptChoice::Original, 1)
        );
        assert_eq!(
            parse_scene_spec("e2:57").unwrap(),
            (Episode::E2, ScriptChoice::Original, 57)
        );
        assert_eq!(
            parse_scene_spec("e3-safe:202").unwrap(),
            (Episode::E3, ScriptChoice::Edited, 202)
        );
        for bad in ["", "e4:1", "e1:300", "x", "e1-safe"] {
            assert!(parse_scene_spec(bad).is_err(), "{bad}");
        }
    }
}
