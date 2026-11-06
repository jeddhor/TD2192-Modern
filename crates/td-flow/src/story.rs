//! Episode 1 story screens: scene cards, text (title) cards, stills, pre-flight hint screens,
//! help pages and the text-only screens.
//!
//! Spec: `docs/spec/flow/episode-program-flow.md` (rev 1) §3.3, `docs/re/asset-load-sites.md`
//! §3–§4 (files, positions, waits), `docs/spec/render/text.md` (rev 2) §4.5 (caption positions
//! and colours), `docs/re/FORMATS.md` §2, §3, §11; OB-003 (`.scr` scenes and `secret.pic`
//! plotted one pixel to the right). Fidelity: T0 for images and spec'd captions; T2 for fades
//! and waits.
//!
//! Every screen works the same way (asset-load-sites §4): black video memory, the image and
//! captions drawn while dark, a fade-in, a wait (seconds or a key; Esc also sets the skip
//! flag), a fade-out. Fades use the 17 levels of `td_render::fade::write_level`; a level takes
//! one or two frames depending on the palette being faded (`td_render::fade::level_frames`,
//! `docs/spec/flow/screens.md` rev 3 §2.1, BUG-6; level 0 one) and is shown from the frame
//! after its last write (`td_render::fade::first_level_frames`, M12-14). Timed waits are DOS time
//! ([`crate::timing::dos_seconds_frames`], screens rev 3 §2.2, BUG-7); `Crt.Delay` pauses are
//! real milliseconds.
//!
//! The palette buffer is the original's shared `g_Palette` (DS:0D2E): a screen that sets only
//! a few entries fades whatever the previous screen left there. Only the mission title card
//! does that in the shipped flows ([`text_card`] starts from the buffer the program keeps in
//! `Program::front_end`, `Program::scene_palette`); every other screen loads a whole palette.
//! Every screen says what it does to `g_Palette` ([`ScenePalette`]); the program run keeps it
//! for the fades and for its flights (weapons rev 4 §3.3 item 5, OB-017).
//!
//! Provisional parts (no spec yet): the line layout of the hint screens and of the text-only
//! screens (orders, dossiers, debrief, retry prompt, script choice), U-121; what `td1-15.pic`'s
//! "split-screen" effect does, U-122; which keys end a timed wait and whether keys typed during
//! the fade-in count (episode-program-flow §8 Q1), U-123.

use td_assets::{AssetError, DataDir};
use td_formats::FormatError;
use td_formats::font::{Font, FontKind};
use td_formats::palette::{Palette, Rgb6};
use td_formats::picture::Picture;
use td_formats::screen::Screen as ScrImage;
use td_input::{Key, TickInput};
use td_render::small_text::{self, Typewriter};
use td_render::{Framebuffer, fade, title_text};
use td_sim::flight::front_end::FrontEnd;
use thiserror::Error;

use crate::dialogue::KEY_BUFFER;
use crate::scene_card::{Captions, Layout, Shadow, lines, rgb};
use crate::screen::{FlowContext, Screen, Transition};
use crate::timing::{delay_ms_frames, dos_seconds_frames};

/// Errors while preparing a story screen.
#[derive(Debug, Error)]
pub enum StoryError {
    #[error(transparent)]
    Asset(#[from] AssetError),
    #[error(transparent)]
    Format(#[from] FormatError),
    #[error("text resource {0} unavailable (unknown file0001.exe version?)")]
    Text(String),
}

/// What a screen does to the scene palette `g_Palette` (DS:0D2E), which the program run keeps
/// (weapons rev 4 §3.3 item 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScenePalette {
    /// The screen loads its palette into `g_Palette` (or clears it) and changes some entries:
    /// its palette buffer is `g_Palette` afterwards. Scene cards, stills, hints, the typed
    /// pages, the story so far, the script choice.
    Loads,
    /// The screen changes this one entry of whatever `g_Palette` holds (the mission title
    /// card, `Flow_TextCard`: entry 85).
    Entry(u8),
    /// The screen does not use `g_Palette`: the help pages (`g_MenuPalette`) and the port's
    /// own screens.
    Untouched,
}

/// How long a screen holds after its fade-in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hold {
    /// Until a key, or this many seconds (`051A:1AA3`).
    Seconds(u32),
    /// Until a key.
    Key,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Phase {
    FadeIn(u8),
    Typing(usize, Typewriter),
    Hold(u32),
    FadeOut(u8),
    Done,
}

/// One line shown after the fade-in (screens §2.3): typed (`Txt_TypeString`, opaque on
/// colour 0, one character per frame) or drawn at once (`Txt_DrawString`, transparent).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextItem {
    pub pen: (i32, i32),
    pub text: String,
    pub ink: u8,
    /// `Crt.Delay` before the line, in milliseconds.
    pub delay_ms: u32,
    pub drawn: bool,
}

impl TextItem {
    /// A typed line.
    #[must_use]
    pub fn typed(pen: (i32, i32), text: impl Into<String>, ink: u8) -> Self {
        Self {
            pen,
            text: text.into(),
            ink,
            delay_ms: 0,
            drawn: false,
        }
    }

    /// A line drawn at once.
    #[must_use]
    pub fn drawn(pen: (i32, i32), text: impl Into<String>, ink: u8) -> Self {
        Self {
            drawn: true,
            ..Self::typed(pen, text, ink)
        }
    }

    /// The same line after a `Crt.Delay(ms)`.
    #[must_use]
    pub fn after(self, ms: u32) -> Self {
        Self {
            delay_ms: ms,
            ..self
        }
    }
}

/// One story screen.
pub struct StoryScreen {
    screen: Framebuffer,
    pal: Palette,
    hold: Hold,
    /// Lines shown after the fade-in.
    items: Vec<TextItem>,
    font: Option<Font>,
    /// Hint screens (screens §3.3): the key buffer is flushed after the fade-in, and a key
    /// waiting after a line skips the remaining lines.
    hint_skip: bool,
    /// E3 (`Flow_WaitSecondsOrKey` `td3:04A0:0269`): every timed wait flushes the keyboard
    /// when it starts, so keys typed during the fade-in or the typing are lost.
    flush_waits: bool,
    /// Second page (help, story so far): shown after the first wait, then held again.
    next_page: Option<(Box<[u8]>, Palette, Hold)>,
    /// What the screen does to `g_Palette`.
    scene_palette: ScenePalette,
    phase: Phase,
    wait: u32,
    /// False until the first frame, which arms the wait before the first fade level (the
    /// palette is built after [`StoryScreen::blank`]).
    primed: bool,
    keys: Vec<Key>,
    skip: bool,
    /// The key that ended the last wait.
    last_key: Option<Key>,
}

impl std::fmt::Debug for StoryScreen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoryScreen")
            .field("hold", &self.hold)
            .field("phase", &self.phase)
            .finish_non_exhaustive()
    }
}

/// True for keys that end a wait (not modifiers or the host keys F10-F12).
pub(crate) fn is_keystroke(k: Key) -> bool {
    !matches!(
        k,
        Key::LSHIFT
            | Key::RSHIFT
            | Key::LCTRL
            | Key::RCTRL
            | Key::LALT
            | Key::RALT
            | Key::F10
            | Key::F11
            | Key::F12
    )
}

/// `Round(x)` of a non-negative `n / 2` (halves away from zero).
fn round_half(n: i32) -> i32 {
    (n + 1).div_euclid(2)
}

impl StoryScreen {
    /// A black screen with an all-black palette buffer.
    pub fn blank(hold: Hold) -> Self {
        Self {
            screen: Framebuffer::new(),
            pal: Palette::default(),
            hold,
            items: Vec::new(),
            font: None,
            hint_skip: false,
            flush_waits: false,
            next_page: None,
            scene_palette: ScenePalette::Loads,
            phase: Phase::FadeIn(0),
            wait: 0,
            primed: false,
            keys: Vec::new(),
            skip: false,
            last_key: None,
        }
    }

    /// The video memory and DAC as shown.
    pub fn screen(&self) -> &Framebuffer {
        &self.screen
    }

    /// The palette buffer (what the fade-in reaches; what the screen leaves in `g_Palette`).
    pub fn palette_buffer(&self) -> &Palette {
        &self.pal
    }

    /// Frames per fade level for the palette buffer as it is (screens rev 3 §2.1): entries
    /// 1..255 are faded.
    pub fn fade_frames_per_level(&self) -> u32 {
        fade::frames_per_level(&self.pal, 1)
    }

    /// Mutable access to video memory and the palette buffer while building a screen.
    pub fn parts_mut(&mut self) -> (&mut Framebuffer, &mut Palette) {
        (&mut self.screen, &mut self.pal)
    }

    /// True if Esc ended a wait (the sequencer's skip flag).
    pub fn skipped(&self) -> bool {
        self.skip
    }

    /// The key that ended the last wait, if any.
    pub fn last_key(&self) -> Option<Key> {
        self.last_key
    }

    /// True once the screen has faded out.
    pub fn finished(&self) -> bool {
        self.phase == Phase::Done
    }

    /// What the screen does to `g_Palette` ([`ScenePalette::Loads`] unless set).
    #[must_use]
    pub fn with_scene_palette(mut self, use_: ScenePalette) -> Self {
        self.scene_palette = use_;
        self
    }

    /// Applies the screen's effect on the front-end state the flights read (weapons rev 4 §3.3
    /// items 5–6): the scene palette as the screen left it, and the lit flag, which its
    /// closing fade-out cleared.
    pub fn apply_front_end(&self, fe: &mut FrontEnd) {
        match self.scene_palette {
            ScenePalette::Loads => fe.scene_palette.copy_from_slice(&self.pal.to_bytes()),
            ScenePalette::Entry(e) => {
                let c = self.pal.colors[usize::from(e)];
                fe.set_entry(e, [c.r, c.g, c.b]);
            }
            ScenePalette::Untouched => return,
        }
        fe.lit = false;
    }

    /// With `on`, every timed wait of this screen flushes the keys typed before it starts (E3,
    /// e3-program-flow rev 1 §3.5).
    #[must_use]
    pub fn flushing_waits(mut self, on: bool) -> Self {
        self.flush_waits = on;
        self
    }

    /// Lines to show after the fade-in, before the wait.
    #[must_use]
    pub fn with_items(mut self, font: Font, items: Vec<TextItem>) -> Self {
        self.font = Some(font);
        self.items = items;
        self
    }

    /// Keys already in the BIOS buffer when the screen starts (typed during the previous
    /// screen, screens §2.2): they end this screen's wait at once.
    pub fn type_ahead(&mut self, keys: Vec<Key>) {
        let mut all = keys;
        all.append(&mut self.keys);
        all.truncate(KEY_BUFFER);
        self.keys = all;
    }

    /// The keys left in the buffer (for the next screen).
    pub fn take_keys(&mut self) -> Vec<Key> {
        std::mem::take(&mut self.keys)
    }

    /// Advances one frame.
    pub fn step(&mut self, input: &TickInput) {
        for &k in &input.pressed {
            if is_keystroke(k) && self.keys.len() < KEY_BUFFER {
                self.keys.push(k);
            }
        }
        if !self.primed {
            self.primed = true;
            self.wait = fade::first_level_frames(&self.pal, 1, 0);
        }
        self.wait = self.wait.saturating_sub(1);
        while self.wait == 0 {
            match &mut self.phase {
                Phase::Done => return,
                Phase::FadeIn(level) => {
                    let k = *level;
                    fade::write_level(&mut self.screen.palette, &self.pal, k);
                    if k < 16 {
                        *level = k + 1;
                        self.wait = fade::level_frames(&self.pal, 1, k + 1);
                    } else {
                        if self.hint_skip {
                            // The hint screen flushes the BIOS buffer after its fade-in.
                            self.keys.clear();
                        }
                        self.phase = self.next_item(0);
                    }
                }
                Phase::Typing(i, t) => {
                    let i = *i;
                    let (Some(font), Some(item)) = (&self.font, self.items.get(i)) else {
                        self.phase = self.hold_phase();
                        continue;
                    };
                    if item.drawn {
                        small_text::draw(&mut self.screen, font, &item.text, item.pen, item.ink);
                        self.phase = self.next_item(i + 1);
                    } else if let Some(frames) = t.step(&mut self.screen, font) {
                        self.wait = frames;
                    } else {
                        self.phase = self.next_item(i + 1);
                    }
                }
                Phase::Hold(left) => {
                    if !self.keys.is_empty() {
                        let k = self.keys.remove(0);
                        self.last_key = Some(k);
                        if k == Key::ESCAPE {
                            self.skip = true;
                        }
                    } else if *left == u32::MAX {
                        return;
                    } else if *left > 0 {
                        *left -= 1;
                        return;
                    }
                    self.phase = Phase::FadeOut(16);
                    self.wait = fade::first_level_frames(&self.pal, 1, 16);
                }
                Phase::FadeOut(level) => {
                    let k = *level;
                    fade::write_level(&mut self.screen.palette, &self.pal, k);
                    if k > 0 {
                        *level = k - 1;
                        self.wait = fade::level_frames(&self.pal, 1, k - 1);
                    } else if let Some((pixels, pal, hold)) = self.next_page.take() {
                        // The next page is drawn after one retrace wait and fades in from the
                        // retrace after that (C-0170 story so far: page 2's levels shown 2,
                        // 4, …, 34 frames after the frame of page 1's last level).
                        self.screen.pixels.copy_from_slice(&pixels);
                        self.pal = pal;
                        self.hold = hold;
                        self.items.clear();
                        self.phase = Phase::FadeIn(0);
                        self.wait = fade::first_level_frames(&self.pal, 1, 0);
                    } else {
                        self.phase = Phase::Done;
                    }
                }
            }
        }
    }

    /// Starts line `n` after its delay, or the wait when the lines are done. On a hint
    /// screen a key waiting after a line skips the rest (screens §3.3).
    fn next_item(&mut self, n: usize) -> Phase {
        if self.hint_skip && n > 0 && !self.keys.is_empty() {
            return self.hold_phase();
        }
        match self.items.get(n) {
            Some(item) => {
                self.wait = delay_ms_frames(item.delay_ms);
                Phase::Typing(n, Typewriter::new(&item.text, item.pen, item.ink, 0))
            }
            None => self.hold_phase(),
        }
    }

    fn hold_phase(&mut self) -> Phase {
        Phase::Hold(match self.hold {
            Hold::Seconds(s) => {
                if self.flush_waits {
                    self.keys.clear();
                }
                dos_seconds_frames(s)
            }
            Hold::Key => u32::MAX,
        })
    }
}

impl Screen for StoryScreen {
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

/// Files shared by the story screens.
#[derive(Clone)]
pub struct StoryAssets {
    pub small: Font,
    pub title: Font,
    pub td_pal: Palette,
}

impl std::fmt::Debug for StoryAssets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StoryAssets")
    }
}

impl StoryAssets {
    pub fn load(data: &DataDir) -> Result<Self, StoryError> {
        Ok(Self {
            small: Font::parse(&data.read("td.lt2")?, FontKind::Small)?,
            title: Font::parse(&data.read("td-let.cin")?, FontKind::Title)?,
            td_pal: Palette::parse(&data.read("td.pal")?)?,
        })
    }
}

/// Plots `pixels` (`w` x `h`) with its top-left corner at (`x`, `y`).
fn plot(fb: &mut Framebuffer, pixels: &[u8], w: usize, (x, y): (i32, i32)) {
    for (row, line) in pixels.chunks_exact(w).enumerate() {
        for (col, &p) in line.iter().enumerate() {
            fb.put(x + col as i32, y + row as i32, p);
        }
    }
}

/// Title card ink entry. Spec: text.md §4.5; colour (3Fh, 0, 0) from episode-program-flow §2.
pub const TITLE_ENTRY: u8 = 85;
const TITLE_RGB: Rgb6 = Rgb6 { r: 63, g: 0, b: 0 };

/// The 14 E1 scene-card routines (`051A:1E5A` .. `596F`), one per `.scr` background: caption
/// pen, black copies, ink entry and colour, wait. Spec: text.md rev 2 §4.5.1.
pub const E1_SCENE_CARDS: [Layout; 14] = {
    use Shadow::*;
    [
        lines("hangar.scr", (10, 170), S1, (210, rgb(14, 63, 63)), 10),
        lines("vlab.scr", (180, 170), S3, (32, rgb(14, 38, 63)), 10),
        lines("intro2.scr", (50, 90), Plain, (140, rgb(0, 0, 0)), 10),
        lines("shooting.scr", (10, 170), Plain, (28, rgb(14, 14, 63)), 10),
        lines("vhang.scr", (20, 170), S3, (165, rgb(14, 38, 63)), 10),
        // 15 rows down; the captions joined into one line at (10, 10), black at (9, 10).
        Layout {
            file: "marauder.scr",
            captions: Captions::Joined((10, 10)),
            shadow: L,
            ink: (255, rgb(63, 5, 5)),
            secs: 10,
            down: 15,
        },
        lines("briefing.scr", (10, 170), S3, (250, rgb(10, 10, 63)), 10),
        lines("tdlab.scr", (10, 170), S3, (109, rgb(10, 10, 63)), 10),
        lines("velsroom.scr", (10, 20), S3, (12, rgb(40, 40, 63)), 10),
        // Opened as `helihangar.scr` (8.3: `helihang.scr`).
        lines("helihang.scr", (180, 20), Plain, (255, rgb(10, 0, 0)), 10),
        lines("eggbase.scr", (10, 170), S3, (255, rgb(15, 63, 15)), 15),
        lines("soffice.scr", (180, 170), O6, (74, rgb(14, 50, 63)), 10),
        lines("sbar.scr", (10, 10), S1, (90, rgb(48, 38, 63)), 10),
        // Each line centred on its own length, y = 10, 20, 30, outlined.
        Layout {
            file: "voffice.scr",
            captions: Captions::Centred([10, 20, 30]),
            shadow: O4,
            ink: (216, rgb(63, 63, 63)),
            secs: 10,
            down: 0,
        },
    ]
};

/// The layout of the E1 scene card on `background` (a `.scr` file name), if there is one.
pub fn e1_scene_layout(background: &str) -> Option<&'static Layout> {
    E1_SCENE_CARDS.iter().find(|l| l.file == background)
}

/// An E1 scene card (`051A:1E5A` and siblings): the `.scr` background plotted at x = 1..320
/// (OB-003), its captions per text.md rev 2 §4.5.1, then 10 s (`eggbase.scr` 15 s) or a key.
pub fn scene_card(
    data: &DataDir,
    assets: &StoryAssets,
    background: &str,
    lines: [&str; 3],
) -> Result<StoryScreen, StoryError> {
    let layout =
        e1_scene_layout(background).ok_or_else(|| AssetError::NotFound(background.to_owned()))?;
    crate::scene_card::scene_card(data, assets, layout, lines)
}

/// The `pic4.td` card (`051A:4F5A`): the still at (11, 16) and captions at (150, 90/100/110)
/// in entry 90 = (63, 5, 5); 10 s. Spec: FORMATS §11; text.md §4.5.
pub fn pic4_card(
    data: &DataDir,
    assets: &StoryAssets,
    lines: [&str; 3],
) -> Result<StoryScreen, StoryError> {
    let mut s = still_screen(data, "pic4.td", (11, 16), Hold::Seconds(10))?;
    s.pal.colors[90] = Rgb6 { r: 63, g: 5, b: 5 };
    for (line, y) in lines.iter().zip([90, 100, 110]) {
        small_text::draw(&mut s.screen, &assets.small, line, (150, y), 90);
    }
    Ok(s)
}

/// A still with its own palette at `at`.
fn still_screen(
    data: &DataDir,
    name: &str,
    at: (i32, i32),
    hold: Hold,
) -> Result<StoryScreen, StoryError> {
    let layout = Picture::layout(name).ok_or_else(|| AssetError::NotFound(name.to_owned()))?;
    let pic = Picture::parse(&data.read(name)?, layout)?;
    let mut s = StoryScreen::blank(hold);
    if let Some(p) = &pic.palette {
        s.pal = p.clone();
    }
    plot(&mut s.screen, &pic.pixels, pic.width, at);
    Ok(s)
}

/// A text card (`Scene_MissionTitle`, `051A:5852`): one title-font line centred at y = 90 in
/// entry 85 = (63, 0, 0) on black; 5 s of DOS time. It loads no palette: `left` is what the
/// previous screens left in `g_Palette`, and only entry 85 is set, so the card fades at one
/// frame per level while that buffer is nearly black (the first card after program start) and
/// at two after any picture palette. Spec: text.md rev 3 §4.5, §4.5.2 (BUG-6); screens rev 3
/// §2.1; asset-load-sites §4.
pub fn text_card(assets: &StoryAssets, text: &str, left: &Palette) -> StoryScreen {
    let mut s =
        StoryScreen::blank(Hold::Seconds(5)).with_scene_palette(ScenePalette::Entry(TITLE_ENTRY));
    s.pal.clone_from(left);
    s.pal.colors[usize::from(TITLE_ENTRY)] = TITLE_RGB;
    let x = title_text::centred_x(text);
    title_text::draw(&mut s.screen, &assets.title, text, (x, 90), TITLE_ENTRY);
    s
}

/// A story-still set (`051A:4113`, sets 1..4) with three captions; 25 s.
/// Spec: asset-load-sites §4 (images, `td.pal` first; `td1-20.pic`'s palette is skipped);
/// text.md §4.5 (caption layouts).
pub fn still_set(
    data: &DataDir,
    assets: &StoryAssets,
    set: u8,
    lines: [&str; 3],
) -> Result<StoryScreen, StoryError> {
    let (name, at) = match set {
        1 => ("td1-16.pic", (86, 0)),
        2 => ("td1-20.pic", (0, 50)),
        3 => ("td1-14.pic", (1, 0)),
        _ => ("td1-13.pic", (44, 39)),
    };
    let mut s = still_screen(data, name, at, Hold::Seconds(25))?;
    if set == 2 {
        s.pal = assets.td_pal.clone();
    }
    let centred = |base: i32, line: &str| base - round_half(5 * line.chars().count() as i32);
    for (i, line) in lines.iter().enumerate() {
        let i = i as i32;
        let (pen, entry, rgb) = match set {
            1 | 2 => ((centred(160, line), 170 + 10 * i), 74, (14, 38, 63)),
            3 => ((160, 30 + 10 * i), 255, (63, 10, 10)),
            _ => ((centred(170, line), 140 + 7 * i), 255, (63, 63, 10)),
        };
        s.pal.colors[usize::from(entry)] = Rgb6 {
            r: rgb.0,
            g: rgb.1,
            b: rgb.2,
        };
        small_text::draw(&mut s.screen, &assets.small, line, pen, entry);
    }
    Ok(s)
}

/// `td1-18.pic` with captions (`051A:471B`): still at (76, 20), centred captions at y = 170,
/// 180, 190 in entry 174 = (63, 4, 63); 15 s. Spec: FORMATS §11; text.md §4.5.
pub fn still_18(
    data: &DataDir,
    assets: &StoryAssets,
    lines: [&str; 3],
) -> Result<StoryScreen, StoryError> {
    let mut s = still_screen(data, "td1-18.pic", (76, 20), Hold::Seconds(15))?;
    s.pal.colors[174] = Rgb6 { r: 63, g: 4, b: 63 };
    for (line, y) in lines.iter().zip([170, 180, 190]) {
        let x = 160 - round_half(5 * line.chars().count() as i32);
        small_text::draw(&mut s.screen, &assets.small, line, (x, y), 174);
    }
    Ok(s)
}

/// `td1-15.pic` (`Scene_StillTd1_15` `051A:4979`, screens §3.9): a plain still. Row r, byte
/// c (c = 0..253) at (33 + c, 30 + r); column 32 shows the byte before the row buffer (0,
/// U-227); byte 254 of each row is never shown. 10 s.
pub fn still_15(data: &DataDir) -> Result<StoryScreen, StoryError> {
    const NAME: &str = "td1-15.pic";
    let layout = Picture::layout(NAME).ok_or_else(|| AssetError::NotFound(NAME.to_owned()))?;
    let pic = Picture::parse(&data.read(NAME)?, layout)?;
    let mut s = StoryScreen::blank(Hold::Seconds(10));
    if let Some(p) = &pic.palette {
        s.pal = p.clone();
    }
    for (r, row) in pic.pixels.chunks_exact(pic.width).enumerate() {
        let y = 30 + r as i32;
        s.screen.put(32, y, 0);
        for (c, &p) in row.iter().take(254).enumerate() {
            s.screen.put(33 + c as i32, y, p);
        }
    }
    Ok(s)
}

/// Hint text entry and colour. Spec: text.md §4.5.
pub const HINT_ENTRY: u8 = 240;

/// Lines of `e1.hints` per hint screen as (index, pen y), all typed at pen x 10 (screens
/// §3.3; line 7 is the shared "press a key" line).
const HINT_LINES: [(u8, &[(usize, i32)]); 3] = [
    (
        1,
        &[
            (8, 40),
            (9, 47),
            (10, 54),
            (11, 61),
            (12, 68),
            (13, 75),
            (14, 82),
            (15, 100),
            (16, 107),
            (17, 114),
            (18, 121),
            (19, 170),
            (7, 185),
        ],
    ),
    (2, &[(20, 40), (21, 47), (22, 54), (7, 170)]),
    (
        6,
        &[
            (0, 40),
            (1, 47),
            (2, 54),
            (3, 68),
            (4, 75),
            (5, 96),
            (6, 103),
            (7, 130),
        ],
    ),
];

/// A pre-flight hint screen (`Scene_HintScreen` `051A:2B26`, screens §3.3): the `secret.pic`
/// banner at x = 1..320, rows 0..31 (OB-003), then the hint lines typed in entry 240 =
/// (63, 10, 10); a key waiting after a line skips the rest; then a key.
pub fn hint(
    data: &DataDir,
    assets: &StoryAssets,
    hints: &[String],
    n: u8,
) -> Result<StoryScreen, StoryError> {
    let mut s = still_screen(data, "secret.pic", (1, 0), Hold::Key)?;
    s.pal.colors[usize::from(HINT_ENTRY)] = Rgb6 {
        r: 63,
        g: 10,
        b: 10,
    };
    let lines = HINT_LINES
        .iter()
        .find(|(h, _)| *h == n)
        .map_or(&[][..], |(_, l)| *l);
    let items = lines
        .iter()
        .map(|&(i, y)| {
            let text = hints.get(i).map_or("", String::as_str);
            TextItem::typed((10, y), text, HINT_ENTRY)
        })
        .collect();
    s.hint_skip = true;
    Ok(s.with_items(assets.small.clone(), items))
}

/// The two help pages of `td.hlp` (`0000:02AF`), each held until a key.
/// Spec: FORMATS §3; asset-load-sites §3.
pub fn help(data: &DataDir) -> Result<StoryScreen, StoryError> {
    let raw = data.read("td.hlp")?;
    let pal = Palette::parse_prefix(&raw)?;
    let page = |i: usize| -> Result<Box<[u8]>, StoryError> {
        let start = 768 + i * 64_000;
        raw.get(start..start + 64_000)
            .map(Into::into)
            .ok_or_else(|| {
                FormatError::Truncated {
                    format: "td.hlp",
                    offset: raw.len(),
                    needed: start + 64_000,
                }
                .into()
            })
    };
    // The help pages use `g_MenuPalette`, which they save and restore (weapons §3.3 item 4).
    let mut s = StoryScreen::blank(Hold::Key).with_scene_palette(ScenePalette::Untouched);
    s.screen.pixels.copy_from_slice(&page(0)?);
    s.pal = pal.clone();
    s.next_page = Some((page(1)?, pal, Hold::Key));
    Ok(s)
}

/// A text-only screen on black with `td.pal` (port notices): `lines` are (pen, text) drawn in
/// `ink`. Not an original screen, so it leaves `g_Palette` alone.
pub fn text_screen(
    assets: &StoryAssets,
    lines: &[((i32, i32), String)],
    ink: (u8, Rgb6),
    hold: Hold,
) -> StoryScreen {
    let mut s = StoryScreen::blank(hold).with_scene_palette(ScenePalette::Untouched);
    s.pal = assets.td_pal.clone();
    s.pal.colors[usize::from(ink.0)] = ink.1;
    for (pen, text) in lines {
        small_text::draw(&mut s.screen, &assets.small, text, *pen, ink.0);
    }
    s
}

/// A typed page on black (screens §3.4–§3.7): `palette` with `entries` changed, `items` shown
/// after the fade-in, then `hold`.
pub fn typed_page(
    assets: &StoryAssets,
    palette: &Palette,
    entries: &[(u8, Rgb6)],
    items: Vec<TextItem>,
    hold: Hold,
) -> StoryScreen {
    let mut s = StoryScreen::blank(hold);
    s.pal = palette.clone();
    for &(e, c) in entries {
        s.pal.colors[usize::from(e)] = c;
    }
    s.with_items(assets.small.clone(), items)
}

/// Types `text` at once into the screen before the fade-in (the retry taunt, screens §3.7).
pub fn type_now(s: &mut StoryScreen, font: &Font, text: &str, pen: (i32, i32), ink: u8) {
    Typewriter::new(text, pen, ink, 0).finish(&mut s.screen, font);
}

/// The two pages of the story so far (`Scene_BackgroundStory` `051A:8EAA`, screens §3.8):
/// `javelin.td` with `lines[0..6]` (30 s), then `city.td` with `lines[6..18]` (45 s); every
/// line drawn with a shadow in 253 at (4, y + 1) and the text in 254 at (5, y). Palette:
/// entries 0..250 := component × 6 div 9, entry 254 G and B := 63.
pub fn story_so_far(
    data: &DataDir,
    assets: &StoryAssets,
    lines: &[String],
) -> Result<StoryScreen, StoryError> {
    let page = |file: &str, ys: &[i32], first: usize| -> Result<Framebuffer, StoryError> {
        let img = ScrImage::parse(&data.read(file)?)?;
        let mut fb = Framebuffer::new();
        fb.pixels.copy_from_slice(&img.pixels);
        fb.palette = img.palette.clone();
        // Each component × 6 div 9 in Integer arithmetic (screens §3.8); a u8 product would
        // overflow above 42.
        let scale = |v: u8| (u16::from(v) * 6 / 9) as u8;
        for c in fb.palette.colors.iter_mut().take(251) {
            *c = Rgb6 {
                r: scale(c.r),
                g: scale(c.g),
                b: scale(c.b),
            };
        }
        fb.palette.colors[254].g = 63;
        fb.palette.colors[254].b = 63;
        for (k, &y) in ys.iter().enumerate() {
            let text = lines.get(first + k).map_or("", String::as_str);
            small_text::draw(&mut fb, &assets.small, text, (4, y + 1), 253);
            small_text::draw(&mut fb, &assets.small, text, (5, y), 254);
        }
        Ok(fb)
    };
    let one = page("javelin.td", &[15, 35, 55, 75, 95, 115], 0)?;
    let two = page(
        "city.td",
        &[10, 20, 30, 40, 50, 60, 80, 90, 100, 110, 130, 140],
        6,
    )?;
    let mut s = StoryScreen::blank(Hold::Seconds(30));
    s.screen.pixels.copy_from_slice(&one.pixels);
    s.pal = one.palette;
    s.next_page = Some((two.pixels, two.palette, Hold::Seconds(45)));
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_e1_background_has_its_own_card_layout() {
        use crate::campaign::Bg;
        let all = [
            Bg::Hangar,
            Bg::Vlab,
            Bg::Intro2,
            Bg::Shooting,
            Bg::Vhang,
            Bg::Marauder,
            Bg::Briefing,
            Bg::Tdlab,
            Bg::Velsroom,
            Bg::Helihang,
            Bg::Eggbase,
            Bg::Soffice,
            Bg::Sbar,
            Bg::Voffice,
        ];
        for bg in all {
            let file = bg.file().unwrap();
            assert!(e1_scene_layout(file).is_some(), "{file}");
        }
        // text.md rev 2 §4.5.1: only hangar has the (10, 170) s1 entry-210 layout.
        let hangar = e1_scene_layout("hangar.scr").unwrap();
        assert_eq!(hangar.captions, Captions::Lines((10, 170)));
        assert_eq!((hangar.shadow, hangar.ink.0), (Shadow::S1, 210));
        let same = E1_SCENE_CARDS
            .iter()
            .filter(|l| {
                (l.captions, l.shadow, l.ink) == (hangar.captions, hangar.shadow, hangar.ink)
            })
            .count();
        assert_eq!(same, 1);
        let secs: Vec<_> = E1_SCENE_CARDS.iter().filter(|l| l.secs == 15).collect();
        assert_eq!(secs.len(), 1);
        assert_eq!(secs[0].file, "eggbase.scr");
        let v = e1_scene_layout("voffice.scr").unwrap();
        assert_eq!(
            (v.captions, v.shadow, v.ink.0),
            (Captions::Centred([10, 20, 30]), Shadow::O4, 216)
        );
        assert_eq!(e1_scene_layout("marauder.scr").unwrap().down, 15);
        assert!(e1_scene_layout("hyde.scr").is_none());
    }

    fn press(k: Key) -> TickInput {
        TickInput {
            pressed: vec![k],
            ..TickInput::default()
        }
    }

    /// A picture-like palette buffer: every component non-zero, so the fades take two frames
    /// per level (screens rev 3 §2.1).
    fn picture(mut s: StoryScreen) -> StoryScreen {
        for c in &mut s.pal.colors {
            *c = Rgb6 { r: 1, g: 2, b: 3 };
        }
        s
    }

    fn run(s: &mut StoryScreen, input: &TickInput) -> u32 {
        let (mut n, idle) = (0, TickInput::default());
        while !s.finished() {
            s.step(if n == 0 { input } else { &idle });
            n += 1;
            assert!(n < 100_000);
        }
        n
    }

    #[test]
    fn timed_hold_fades_in_waits_and_fades_out() {
        let mut s = picture(StoryScreen::blank(Hold::Seconds(5)));
        s.pal.colors[3] = Rgb6 { r: 63, g: 0, b: 0 };
        // 17 levels in (34), 5 s of DOS time (91 BIOS ticks: 332 frames, BUG-7), 17 levels
        // out (34).
        let n = run(&mut s, &TickInput::default());
        assert_eq!(n, 34 + 332 + 34);
        assert!(!s.skipped());
        assert_eq!(s.screen().palette.colors[3], Rgb6::default());
    }

    #[test]
    fn a_nearly_black_buffer_fades_at_one_frame_per_level() {
        // BUG-6 (screens rev 3 §2.1, text rev 3 §4.5.2): the first title card after program
        // start fades only entry 85 over the black (or script-choice) buffer: 17 frames a
        // fade, against 34 once a picture palette is left in g_Palette (C-0002 f1063..1080
        // and f1409..1428 against C-0170 f1003..1036 and f1364..1397).
        let mut s = StoryScreen::blank(Hold::Seconds(5));
        s.pal.colors[usize::from(TITLE_ENTRY)] = TITLE_RGB;
        assert_eq!(s.fade_frames_per_level(), 1);
        assert_eq!(run(&mut s, &TickInput::default()), 17 + 332 + 17);
        let mut left = Palette::default();
        for c in &mut left.colors {
            *c = Rgb6 { r: 9, g: 9, b: 9 };
        }
        let mut s = StoryScreen::blank(Hold::Seconds(5));
        s.pal.clone_from(&left);
        s.pal.colors[usize::from(TITLE_ENTRY)] = TITLE_RGB;
        assert_eq!(s.fade_frames_per_level(), 2);
        assert_eq!(run(&mut s, &TickInput::default()), 34 + 332 + 34);
    }

    #[test]
    fn a_key_ends_the_hold_and_esc_skips() {
        let mut s = picture(StoryScreen::blank(Hold::Key));
        for _ in 0..40 {
            s.step(&TickInput::default());
        }
        assert!(!s.finished());
        s.step(&press(Key::LSHIFT));
        s.step(&TickInput::default());
        assert!(matches!(s.phase, Phase::Hold(_)), "modifiers do not count");
        s.step(&press(Key::ESCAPE));
        assert!(s.skipped());
        assert_eq!(s.last_key(), Some(Key::ESCAPE));
        let mut n = 0;
        while !s.finished() {
            s.step(&TickInput::default());
            n += 1;
        }
        assert_eq!(n, 34);
    }

    #[test]
    fn typed_lines_come_after_the_fade_in() {
        let font = Font::parse(&vec![0xF7; FontKind::Small.file_len()], FontKind::Small).unwrap();
        let mut s = picture(StoryScreen::blank(Hold::Key)).with_items(
            font,
            vec![
                TextItem::typed((10, 40), "ab", 240),
                TextItem::typed((10, 47), "c", 240),
                TextItem::drawn((10, 60), "d", 39).after(100),
            ],
        );
        for _ in 0..34 {
            s.step(&TickInput::default());
        }
        assert_eq!(
            s.screen().get(12, 41),
            Some(240),
            "first character with the last level"
        );
        s.step(&TickInput::default());
        s.step(&TickInput::default());
        assert_eq!(s.screen().get(12, 48), Some(240), "second line");
        assert!(matches!(s.phase, Phase::Typing(1, _)));
        // The drawn line waits for its Crt.Delay(100) (7 frames), then appears at once.
        for _ in 0..7 {
            s.step(&TickInput::default());
        }
        assert_eq!(s.screen().get(12, 61), Some(0));
        s.step(&TickInput::default());
        assert_eq!(s.screen().get(12, 61), Some(39));
    }

    #[test]
    fn a_key_after_a_hint_line_skips_the_rest() {
        let font = Font::parse(&vec![0xF7; FontKind::Small.file_len()], FontKind::Small).unwrap();
        let mut s = picture(StoryScreen::blank(Hold::Key)).with_items(
            font,
            vec![
                TextItem::typed((10, 40), "a", 240),
                TextItem::typed((10, 47), "b", 240),
            ],
        );
        s.hint_skip = true;
        // A key during the fade-in is flushed with the buffer.
        s.step(&press(Key::SPACE));
        for _ in 0..33 {
            s.step(&TickInput::default());
        }
        assert!(matches!(s.phase, Phase::Typing(0, _)));
        // A key while line 0 types: line 1 is skipped, and the wait consumes the key.
        s.step(&press(Key::SPACE));
        for _ in 0..3 {
            s.step(&TickInput::default());
        }
        assert_eq!(s.screen().get(12, 48), Some(0), "line 1 not typed");
        assert!(matches!(s.phase, Phase::FadeOut(_)));
    }

    #[test]
    fn leftover_keys_end_the_next_wait() {
        // Two keys in one frame: the first ends this wait, the second is left for the next
        // screen, whose wait ends right after its fade-in (screens §2.2).
        let mut a = picture(StoryScreen::blank(Hold::Seconds(5)));
        a.step(&TickInput {
            pressed: vec![Key::SPACE, Key::ESCAPE],
            ..TickInput::default()
        });
        let n = 1 + run(&mut a, &TickInput::default());
        assert_eq!(n, 34 + 34, "the first key ended the hold");
        assert!(!a.skipped());
        let left = a.take_keys();
        assert_eq!(left, [Key::ESCAPE]);
        let mut b = picture(StoryScreen::blank(Hold::Seconds(5)));
        b.type_ahead(left);
        assert_eq!(run(&mut b, &TickInput::default()), 34 + 34);
        assert!(b.skipped(), "the carried Esc sets the skip flag");
        // The buffer holds 15 keystrokes.
        let mut c = StoryScreen::blank(Hold::Key);
        c.type_ahead(vec![Key::SPACE; 20]);
        assert_eq!(c.take_keys().len(), KEY_BUFFER);
    }

    #[test]
    fn caption_centring_rounds_halves_up() {
        assert_eq!(160 - round_half(5 * 3), 152, "Round(7.5) = 8");
        assert_eq!(160 - round_half(5 * 2), 155);
    }
}
