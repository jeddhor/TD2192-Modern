//! Episode 1 story screens: scene cards, text (title) cards, stills, pre-flight hint screens,
//! help pages and the text-only screens.
//!
//! Spec: `docs/spec/flow/episode-program-flow.md` (rev 1) §3.3, `docs/re/asset-load-sites.md`
//! §3–§4 (files, positions, waits), `docs/spec/render/text.md` (rev 1) §4.5 (caption positions
//! and colours), `docs/re/FORMATS.md` §2, §3, §11; OB-003 (`.scr` scenes and `secret.pic`
//! plotted one pixel to the right). Fidelity: T0 for images and spec'd captions; T2 for fades
//! and waits.
//!
//! Every screen works the same way (asset-load-sites §4): black video memory, the image and
//! captions drawn while dark, a fade-in, a wait (seconds or a key; Esc also sets the skip
//! flag), a fade-out. Fades use the 17 levels of `td_render::fade::write_level`, one level per
//! [`FADE_FRAMES_PER_LEVEL`] frames as in the dialogue player.
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
use td_render::{Framebuffer, WIDTH, fade, title_text};
use thiserror::Error;

use crate::dialogue::FADE_FRAMES_PER_LEVEL;
use crate::screen::{FlowContext, RETRACE_HZ, Screen, Transition};

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

/// A typed text line: pen and text.
type TypedLine = ((i32, i32), String);

/// One story screen.
pub struct StoryScreen {
    screen: Framebuffer,
    pal: Palette,
    hold: Hold,
    /// Lines typed after the fade-in (hint screens), with their ink.
    typed: Vec<TypedLine>,
    ink: u8,
    font: Option<Font>,
    /// Second page (help): shown after the first wait, then held again.
    next_page: Option<(Box<[u8]>, Palette)>,
    phase: Phase,
    wait: u32,
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

/// Seconds to frames at the retrace rate.
fn seconds(s: u32) -> u32 {
    (f64::from(s) * RETRACE_HZ).round() as u32
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
            typed: Vec::new(),
            ink: 0,
            font: None,
            next_page: None,
            phase: Phase::FadeIn(0),
            wait: FADE_FRAMES_PER_LEVEL,
            keys: Vec::new(),
            skip: false,
            last_key: None,
        }
    }

    /// The video memory and DAC as shown.
    pub fn screen(&self) -> &Framebuffer {
        &self.screen
    }

    /// The palette buffer (what the fade-in reaches).
    pub fn palette_buffer(&self) -> &Palette {
        &self.pal
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

    /// Lines to type (one character per frame) after the fade-in, before the wait.
    pub fn with_typed(mut self, font: Font, ink: u8, lines: Vec<TypedLine>) -> Self {
        self.font = Some(font);
        self.ink = ink;
        self.typed = lines;
        self
    }

    /// Advances one frame.
    pub fn step(&mut self, input: &TickInput) {
        self.keys
            .extend(input.pressed.iter().copied().filter(|&k| is_keystroke(k)));
        self.wait = self.wait.saturating_sub(1);
        while self.wait == 0 {
            match &mut self.phase {
                Phase::Done => return,
                Phase::FadeIn(level) => {
                    let k = *level;
                    fade::write_level(&mut self.screen.palette, &self.pal, k);
                    if k < 16 {
                        *level = k + 1;
                        self.wait = FADE_FRAMES_PER_LEVEL;
                    } else {
                        self.phase = Phase::Typing(0, self.typewriter(0));
                    }
                }
                Phase::Typing(i, t) => {
                    let Some(font) = &self.font else {
                        self.phase = self.hold_phase();
                        continue;
                    };
                    if let Some(frames) = t.step(&mut self.screen, font) {
                        self.wait = frames;
                    } else if *i + 1 < self.typed.len() {
                        let n = *i + 1;
                        self.phase = Phase::Typing(n, self.typewriter(n));
                    } else {
                        self.phase = self.hold_phase();
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
                    self.wait = FADE_FRAMES_PER_LEVEL;
                }
                Phase::FadeOut(level) => {
                    let k = *level;
                    fade::write_level(&mut self.screen.palette, &self.pal, k);
                    if k > 0 {
                        *level = k - 1;
                        self.wait = FADE_FRAMES_PER_LEVEL;
                    } else if let Some((pixels, pal)) = self.next_page.take() {
                        self.screen.pixels.copy_from_slice(&pixels);
                        self.pal = pal;
                        self.phase = Phase::FadeIn(0);
                        self.wait = FADE_FRAMES_PER_LEVEL;
                    } else {
                        self.phase = Phase::Done;
                    }
                }
            }
        }
    }

    fn typewriter(&self, i: usize) -> Typewriter {
        match self.typed.get(i) {
            Some((pen, text)) => Typewriter::new(text, *pen, self.ink, 0),
            None => Typewriter::new("", (0, 0), 0, 0),
        }
    }

    fn hold_phase(&self) -> Phase {
        Phase::Hold(match self.hold {
            Hold::Seconds(s) => seconds(s),
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

/// Caption entry of the `.scr` scene cards and its colour. Spec: text.md §4.5.
pub const SCENE_CAPTION_ENTRY: u8 = 210;
const SCENE_CAPTION_RGB: Rgb6 = Rgb6 {
    r: 14,
    g: 63,
    b: 63,
};
/// Title card ink entry. Spec: text.md §4.5; colour (3Fh, 0, 0) from episode-program-flow §2.
pub const TITLE_ENTRY: u8 = 85;
const TITLE_RGB: Rgb6 = Rgb6 { r: 63, g: 0, b: 0 };

/// A scene card (`051A:1E5A` and siblings): a `.scr` background plotted at x = 1..320 (OB-003)
/// and three caption lines at pens (10, 170/180/190), each first drawn black at (9, y + 1).
/// Waits 10 s (`eggbase.scr` 15 s) or a key. Spec: asset-load-sites §4; text.md §4.5.
pub fn scene_card(
    data: &DataDir,
    assets: &StoryAssets,
    background: &str,
    lines: [&str; 3],
) -> Result<StoryScreen, StoryError> {
    let scr = ScrImage::parse(&data.read(background)?)?;
    let secs = if background == "eggbase.scr" { 15 } else { 10 };
    let mut s = StoryScreen::blank(Hold::Seconds(secs));
    s.pal = scr.palette.clone();
    plot(&mut s.screen, &scr.pixels, WIDTH, (1, 0));
    s.pal.colors[usize::from(SCENE_CAPTION_ENTRY)] = SCENE_CAPTION_RGB;
    for (line, y) in lines.iter().zip([170, 180, 190]) {
        small_text::draw(&mut s.screen, &assets.small, line, (9, y + 1), 0);
        small_text::draw(
            &mut s.screen,
            &assets.small,
            line,
            (10, y),
            SCENE_CAPTION_ENTRY,
        );
    }
    Ok(s)
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

/// A text card (`Flow_TextCard`, `051A:5852`): one title-font line centred at y = 90 in entry
/// 85 = (63, 0, 0) on black; 5 s. Spec: text.md §4.5; asset-load-sites §4.
pub fn text_card(assets: &StoryAssets, text: &str) -> StoryScreen {
    let mut s = StoryScreen::blank(Hold::Seconds(5));
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

/// `td1-15.pic` (`051A:4979`) at (32, 30); 10 s. ASSUMPTION(U-122): the original's
/// split-screen/palette effect is not specified; the still is shown plainly.
pub fn still_15(data: &DataDir) -> Result<StoryScreen, StoryError> {
    still_screen(data, "td1-15.pic", (32, 30), Hold::Seconds(10))
}

/// Hint text entry and colour. Spec: text.md §4.5.
pub const HINT_ENTRY: u8 = 240;

/// Lines of `e1.hints` per hint screen, as (first line, count, pen y of the first line).
///
/// ASSUMPTION(U-121): only hint 1's first two paragraphs are pinned by C-0002 frame 3332 (pen
/// y 40 and 100, 7-pixel pitch, x 10). The grouping of the other lines and the paragraph
/// positions are provisional.
type Paragraph = (usize, usize, i32);
const HINT_PARAGRAPHS: [(u8, &[Paragraph]); 3] = [
    (1, &[(8, 7, 40), (15, 4, 100), (19, 1, 135), (7, 1, 160)]),
    (2, &[(20, 3, 40), (7, 1, 160)]),
    (6, &[(0, 3, 40), (3, 2, 70), (5, 2, 93), (7, 1, 160)]),
];

/// A pre-flight hint screen (`051A:2B26`): the `secret.pic` banner at x = 1..320, rows 0..31
/// (OB-003), then the hint typed in entry 240 = (63, 10, 10); waits for a key.
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
    let mut lines = Vec::new();
    let paragraphs = HINT_PARAGRAPHS
        .iter()
        .find(|(h, _)| *h == n)
        .map_or(&[][..], |(_, p)| *p);
    for &(first, count, y) in paragraphs {
        for (k, text) in hints.iter().skip(first).take(count).enumerate() {
            lines.push(((10, y + 7 * k as i32), text.clone()));
        }
    }
    Ok(s.with_typed(assets.small.clone(), HINT_ENTRY, lines))
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
    let mut s = StoryScreen::blank(Hold::Key);
    s.screen.pixels.copy_from_slice(&page(0)?);
    s.pal = pal.clone();
    s.next_page = Some((page(1)?, pal));
    Ok(s)
}

/// A text-only screen on black with `td.pal` (orders, dossiers, debrief, retry prompt,
/// notices): `lines` are (pen, text) in `ink`.
///
/// ASSUMPTION(U-121): the original layouts of these screens are not specified; the port puts
/// the lines at the pens the caller gives.
pub fn text_screen(
    assets: &StoryAssets,
    lines: &[((i32, i32), String)],
    ink: (u8, Rgb6),
    hold: Hold,
) -> StoryScreen {
    let mut s = StoryScreen::blank(hold);
    s.pal = assets.td_pal.clone();
    s.pal.colors[usize::from(ink.0)] = ink.1;
    for (pen, text) in lines {
        small_text::draw(&mut s.screen, &assets.small, text, *pen, ink.0);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(k: Key) -> TickInput {
        TickInput {
            pressed: vec![k],
            ..TickInput::default()
        }
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
        let mut s = StoryScreen::blank(Hold::Seconds(5));
        s.pal.colors[3] = Rgb6 { r: 63, g: 0, b: 0 };
        // 17 levels in (34), 5 s (350 frames), 17 levels out (34).
        let n = run(&mut s, &TickInput::default());
        assert_eq!(n, 34 + 350 + 34);
        assert!(!s.skipped());
        assert_eq!(s.screen().palette.colors[3], Rgb6::default());
    }

    #[test]
    fn a_key_ends_the_hold_and_esc_skips() {
        let mut s = StoryScreen::blank(Hold::Key);
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
        let mut s = StoryScreen::blank(Hold::Key).with_typed(
            font,
            240,
            vec![((10, 40), "ab".into()), ((10, 47), "c".into())],
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
    }

    #[test]
    fn caption_centring_rounds_halves_up() {
        assert_eq!(160 - round_half(5 * 3), 152, "Round(7.5) = 8");
        assert_eq!(160 - round_half(5 * 2), 155);
    }
}
