//! What the port's own screens share: the game's fonts and menu palette, a few colours set in
//! that palette, the fades and the keyboard and controller navigation (M11-04).
//!
//! The screens are drawn like the original's own: indexed 320 × 200, the small font
//! (`td.lt2`) and the title font (`td-let.cin`), `tdmen.pal` with the colours they need set in
//! the entries the main menu also recolours (224..), and 16-level palette fades.

use td_assets::DataDir;
use td_formats::font::{Font, FontKind};
use td_formats::palette::{Palette, Rgb6};
use td_input::{Key, Pad, PadButton, TickInput};
use td_render::{Framebuffer, fade, small_text, title_text};

use crate::dialogue::FADE_FRAMES_PER_LEVEL;

/// Palette entry of normal text.
pub const INK_TEXT: u8 = 224;
/// The selected item (the main menu's cursor red).
pub const INK_CURSOR: u8 = 225;
/// Items that cannot be chosen.
pub const INK_DISABLED: u8 = 226;
/// Headings (the main menu's yellow).
pub const INK_HEADING: u8 = 227;
/// Values.
pub const INK_VALUE: u8 = 228;
/// Notes and hints.
pub const INK_NOTE: u8 = 229;

const INKS: [(u8, Rgb6); 6] = [
    (
        INK_TEXT,
        Rgb6 {
            r: 42,
            g: 42,
            b: 42,
        },
    ),
    (INK_CURSOR, Rgb6 { r: 63, g: 0, b: 0 }),
    // M12-18 (VE-8's F9, WCAG 1.4.3): (28, 28, 28) on black is 4.30:1 (was 18, 18, 18:
    // 2.33:1), and still 2.1:1 darker than the text grey.
    (
        INK_DISABLED,
        Rgb6 {
            r: 28,
            g: 28,
            b: 28,
        },
    ),
    (INK_HEADING, Rgb6 { r: 63, g: 63, b: 0 }),
    (
        INK_VALUE,
        Rgb6 {
            r: 63,
            g: 63,
            b: 63,
        },
    ),
    // M12-18 (F9): (56, 32, 32) on black is 7.78:1 (was 40, 10, 10: 2.87:1), still reddish
    // and 1.48:1 from the cursor red.
    (
        INK_NOTE,
        Rgb6 {
            r: 56,
            g: 32,
            b: 32,
        },
    ),
];

/// The fonts and palette of the port's screens.
#[derive(Debug, Clone)]
pub struct UiAssets {
    pub small: Font,
    pub title: Font,
    pub palette: Palette,
}

impl UiAssets {
    /// The game's fonts and `tdmen.pal`. A missing file gives a blank font or a black
    /// palette, so the screens still work (and their colours are always set).
    pub fn load(data: &DataDir) -> Self {
        let font = |name: &str, kind: FontKind| {
            data.read(name)
                .ok()
                .and_then(|b| Font::parse(&b, kind).ok())
                .unwrap_or_else(|| blank_font(kind, 0))
        };
        let palette = data
            .read("tdmen.pal")
            .ok()
            .and_then(|b| Palette::parse(&b).ok())
            .unwrap_or_default();
        Self::new(
            font("td.lt2", FontKind::Small),
            font("td-let.cin", FontKind::Title),
            palette,
        )
    }

    /// Solid-block fonts and a black base palette (tests, snapshot hashes).
    pub fn blank() -> Self {
        Self::new(
            blank_font(FontKind::Small, 0xF7),
            blank_font(FontKind::Title, 0xF7),
            Palette::default(),
        )
    }

    fn new(small: Font, title: Font, mut palette: Palette) -> Self {
        palette.colors[0] = Rgb6::default();
        for (entry, c) in INKS {
            palette.colors[usize::from(entry)] = c;
        }
        Self {
            small,
            title,
            palette,
        }
    }

    /// Small-font text at pen (`x`, `y`). Returns what `small_text::draw` returns: the offset
    /// after the text **relative to `x`** (1 + the advances), not an absolute pen; the pen
    /// after the text is `x + offset - 1`.
    pub fn text(&self, fb: &mut Framebuffer, text: &str, (x, y): (i32, i32), ink: u8) -> i32 {
        small_text::draw(fb, &self.small, text, (x, y), ink)
    }

    /// Small-font text centred on the screen.
    pub fn text_centred(&self, fb: &mut Framebuffer, text: &str, y: i32, ink: u8) {
        let w: i32 = text.chars().map(small_text::advance).sum();
        self.text(fb, text, (160 - w / 2, y), ink);
    }

    /// A file path in the small font: [`Self::text_marked`] of its display form.
    pub fn path(
        &self,
        fb: &mut Framebuffer,
        path: &std::path::Path,
        pen: (i32, i32),
        ink: u8,
        max_w: i32,
    ) -> i32 {
        self.text_marked(fb, &path.display().to_string(), pen, ink, max_w)
    }

    /// Small-font text that may hold any character, at pen (`x`, `y`), at most `max_w` pixels
    /// wide; returns the absolute pen after it. Letters are shown in lower case (the font has
    /// none other), and the characters paths need that the font lacks (`\ / : _ ( ) + ' &`)
    /// are drawn as the port's own marks; anything else becomes `?`. Text too wide keeps its
    /// end, after `...` (the end of a path names the file).
    pub fn text_marked(
        &self,
        fb: &mut Framebuffer,
        text: &str,
        (x, y): (i32, i32),
        ink: u8,
        max_w: i32,
    ) -> i32 {
        let text = path_chars(text);
        let width = |s: &[char]| s.iter().map(|&c| path_advance(c)).sum::<i32>();
        let mut shown: &[char] = &text;
        let mut prefix = "";
        if width(shown) > max_w {
            prefix = "...";
            let room = max_w - width(&['.'; 3]);
            while !shown.is_empty() && width(shown) > room {
                shown = &shown[1..];
            }
        }
        // Absolute pens: a glyph drawn at pen p starts at p + 1 (`small_text`).
        let mut pen = x + self.text(fb, prefix, (x, y), ink) - 1;
        for &c in shown {
            if let Some(marks) = path_marks(c) {
                for &(col, row) in marks {
                    fb.put(pen + 1 + col, y + 1 + row, ink);
                }
                pen += path_advance(c);
            } else {
                pen += self.text(fb, c.encode_utf8(&mut [0; 4]), (pen, y), ink) - 1;
            }
        }
        pen
    }

    /// A title-font heading centred on the screen.
    pub fn heading(&self, fb: &mut Framebuffer, text: &str, y: i32) {
        title_text::draw(
            fb,
            &self.title,
            text,
            (title_text::centred_x(text), y),
            INK_HEADING,
        );
    }
}

/// The characters of a path as [`UiAssets::path`] draws them: lower case; anything the font
/// and the port's marks cannot show becomes `?`.
fn path_chars(s: &str) -> Vec<char> {
    s.chars()
        .map(|c| c.to_ascii_lowercase())
        .map(|c| {
            if path_marks(c).is_some() || small_text::advance(c) > 0 {
                c
            } else {
                '?'
            }
        })
        .collect()
}

/// The port's marks for path characters the small font lacks: pixels (column, row) of a 5 × 5
/// cell, drawn like a glyph (pen + 1).
fn path_marks(c: char) -> Option<&'static [(i32, i32)]> {
    Some(match c {
        '\\' => &[(0, 0), (0, 1), (1, 2), (2, 3), (2, 4)],
        '/' => &[(2, 0), (2, 1), (1, 2), (0, 3), (0, 4)],
        ':' => &[(0, 1), (0, 3)],
        '_' => &[(0, 4), (1, 4), (2, 4)],
        '(' => &[(1, 0), (0, 1), (0, 2), (0, 3), (1, 4)],
        ')' => &[(0, 0), (1, 1), (1, 2), (1, 3), (0, 4)],
        '+' => &[(1, 1), (0, 2), (1, 2), (2, 2), (1, 3)],
        '\'' => &[(0, 0), (0, 1)],
        '&' => &[(1, 0), (0, 1), (1, 2), (0, 3), (2, 3), (1, 4), (2, 4)],
        _ => return None,
    })
}

/// How far the pen moves after a path character.
fn path_advance(c: char) -> i32 {
    match c {
        '\'' => 2,
        ':' | '(' | ')' => 3,
        '\\' | '/' | '_' | '+' | '&' => 4,
        _ => small_text::advance(c),
    }
}

fn blank_font(kind: FontKind, byte: u8) -> Font {
    Font::parse(&vec![byte; kind.file_len()], kind).expect("a font of the right length")
}

/// A 16-level palette fade (screens §2.1 pace: [`FADE_FRAMES_PER_LEVEL`] frames a level).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fader {
    level: u8,
    dir: i8,
    wait: u32,
}

impl Fader {
    /// Black, about to fade in.
    pub fn fading_in() -> Self {
        Self {
            level: 0,
            dir: 1,
            wait: FADE_FRAMES_PER_LEVEL,
        }
    }

    /// Starts fading out from the current level.
    pub fn fade_out(&mut self) {
        self.dir = -1;
        self.wait = FADE_FRAMES_PER_LEVEL;
    }

    /// Advances one frame; true once the fade has finished (fully in or fully out).
    pub fn step(&mut self) -> bool {
        if self.done() {
            return true;
        }
        self.wait = self.wait.saturating_sub(1);
        if self.wait == 0 {
            self.level = self.level.saturating_add_signed(self.dir).min(16);
            self.wait = FADE_FRAMES_PER_LEVEL;
        }
        self.done()
    }

    /// True when the fade has reached its end.
    pub fn done(&self) -> bool {
        (self.dir > 0 && self.level == 16) || (self.dir < 0 && self.level == 0)
    }

    /// True while fading out (or faded out).
    pub fn out(&self) -> bool {
        self.dir < 0
    }

    /// `src` at the current level.
    pub fn apply(&self, dac: &mut Palette, src: &Palette) {
        fade::write_level(dac, src, self.level);
    }
}

/// A navigation action on the port's screens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nav {
    Up,
    Down,
    Left,
    Right,
    Confirm,
    Back,
}

/// Frames a held direction waits before it repeats, then between repeats (70 Hz frames).
const REPEAT_DELAY: u32 = 24;
const REPEAT_PERIOD: u32 = 7;

/// What a controller did in one tick.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PadStep {
    /// Buttons that went down this tick.
    pub pressed: Vec<PadButton>,
    /// A direction from the stick or the D-pad: when it is first pushed, then repeating.
    pub dir: Option<Nav>,
}

/// Edge and repeat tracking for a controller (M11-04).
#[derive(Debug, Clone, Default)]
pub struct PadNav {
    prev_buttons: u32,
    prev_dir: Option<Nav>,
    held: u32,
}

impl PadNav {
    /// A tracker that treats the buttons held now as already seen (a screen opened by a
    /// button press does not take that press as its own).
    pub fn starting_with(pad: Option<Pad>) -> Self {
        Self {
            prev_buttons: pad.map_or(0, |p| p.buttons),
            ..Self::default()
        }
    }

    /// Takes this tick's controller state.
    pub fn update(&mut self, pad: Option<Pad>) -> PadStep {
        let pad = pad.unwrap_or_default();
        let pressed = PadButton::ALL
            .into_iter()
            .filter(|b| pad.held(*b) && self.prev_buttons & b.bit() == 0)
            .collect();
        self.prev_buttons = pad.buttons;
        // The original's signs: x 1 = left, y 1 = up.
        let now = match (pad.x, pad.y) {
            (_, 1) => Some(Nav::Up),
            (_, -1) => Some(Nav::Down),
            (1, _) => Some(Nav::Left),
            (-1, _) => Some(Nav::Right),
            _ => None,
        };
        let dir = if now != self.prev_dir {
            self.held = 0;
            now
        } else if now.is_some() {
            self.held += 1;
            let repeat = self.held >= REPEAT_DELAY
                && (self.held - REPEAT_DELAY).is_multiple_of(REPEAT_PERIOD);
            now.filter(|_| repeat)
        } else {
            None
        };
        self.prev_dir = now;
        PadStep { pressed, dir }
    }
}

/// The navigation key `k` stands for.
pub fn key_nav(k: Key) -> Option<Nav> {
    match k {
        Key::UP | KP_8 => Some(Nav::Up),
        Key::DOWN | KP_2 => Some(Nav::Down),
        Key::LEFT | KP_4 => Some(Nav::Left),
        Key::RIGHT | KP_6 => Some(Nav::Right),
        Key::ENTER | Key::KP_ENTER | Key::SPACE => Some(Nav::Confirm),
        Key::ESCAPE | Key::BACKSPACE => Some(Nav::Back),
        _ => None,
    }
}

/// This tick's one navigation action. South confirms and East goes back. A tick with a
/// controller button press ignores the keys of that tick, which may be the keys its bound
/// buttons stand for ([`crate::controls::ControlsSettings::pad_keys`]), except a Start press,
/// which is Esc (back) through its default binding.
pub fn nav(input: &TickInput, pad: &PadStep) -> Option<Nav> {
    if pad.pressed.contains(&PadButton::South) {
        return Some(Nav::Confirm);
    }
    if pad.pressed.contains(&PadButton::East) {
        return Some(Nav::Back);
    }
    input.pressed.iter().find_map(|&k| key_nav(k)).or(pad.dir)
}

/// A controller's answer on the script-choice screen, which reads only the keys 1 and 2: South
/// is 1 (the original script), East is 2 (the edited one). A port addition so that a
/// controller alone can start an episode (M11-04).
pub fn pad_script_choice(pad: Option<Pad>) -> Option<Key> {
    let p = pad?;
    if p.held(PadButton::South) {
        Key::digit(1)
    } else if p.held(PadButton::East) {
        Key::digit(2)
    } else {
        None
    }
}

/// Keypad 8, 2, 4, 6 (HID usage IDs).
const KP_8: Key = Key(96);
const KP_2: Key = Key(90);
const KP_4: Key = Key(92);
const KP_6: Key = Key(94);

#[cfg(test)]
mod tests {
    use super::*;

    fn pad(x: i8, y: i8, buttons: u32) -> Option<Pad> {
        Some(Pad {
            x,
            y,
            buttons,
            ..Pad::default()
        })
    }

    #[test]
    fn pad_directions_move_once_then_repeat() {
        let mut p = PadNav::default();
        assert_eq!(p.update(pad(0, 1, 0)).dir, Some(Nav::Up));
        let moves = (0..REPEAT_DELAY + 2 * REPEAT_PERIOD)
            .filter(|_| p.update(pad(0, 1, 0)).dir.is_some())
            .count();
        assert_eq!(moves, 3, "the delay, then one move per period");
        assert_eq!(p.update(pad(0, 0, 0)).dir, None);
        assert_eq!(p.update(pad(-1, 0, 0)).dir, Some(Nav::Right));
        assert_eq!(p.update(pad(1, 0, 0)).dir, Some(Nav::Left));
        assert_eq!(p.update(pad(0, -1, 0)).dir, Some(Nav::Down));
    }

    #[test]
    fn buttons_are_edges() {
        let mut p = PadNav::starting_with(pad(0, 0, PadButton::South.bit()));
        assert!(
            p.update(pad(0, 0, PadButton::South.bit()))
                .pressed
                .is_empty()
        );
        assert!(p.update(pad(0, 0, 0)).pressed.is_empty());
        assert_eq!(
            p.update(pad(0, 0, PadButton::South.bit())).pressed,
            [PadButton::South]
        );
    }

    #[test]
    fn south_confirms_east_goes_back_and_keys_come_next() {
        let keys = |k: Vec<Key>| TickInput {
            pressed: k,
            ..TickInput::default()
        };
        let step = |b: Vec<PadButton>, dir| PadStep { pressed: b, dir };
        // South with the Space its binding stands for: one confirm.
        assert_eq!(
            nav(&keys(vec![Key::SPACE]), &step(vec![PadButton::South], None)),
            Some(Nav::Confirm)
        );
        assert_eq!(
            nav(&keys(vec![]), &step(vec![PadButton::East], None)),
            Some(Nav::Back)
        );
        // Start stands for Esc.
        assert_eq!(
            nav(
                &keys(vec![Key::ESCAPE]),
                &step(vec![PadButton::Start], None)
            ),
            Some(Nav::Back)
        );
        assert_eq!(
            nav(&keys(vec![Key::DOWN]), &step(vec![], Some(Nav::Up))),
            Some(Nav::Down)
        );
        assert_eq!(
            nav(&keys(vec![]), &step(vec![], Some(Nav::Up))),
            Some(Nav::Up)
        );
        assert_eq!(nav(&keys(vec![Key::A]), &step(vec![], None)), None);
    }

    #[test]
    fn fader_runs_both_ways() {
        let mut f = Fader::fading_in();
        let frames = (1..100).find(|_| f.step()).unwrap();
        assert_eq!(frames, 16 * FADE_FRAMES_PER_LEVEL);
        f.fade_out();
        assert!(!f.done());
        let frames = (1..100).find(|_| f.step()).unwrap();
        assert_eq!(frames, 16 * FADE_FRAMES_PER_LEVEL);
        assert!(f.out() && f.done());
    }

    #[test]
    fn paths_show_in_lower_case_with_the_ports_marks_and_keep_their_end() {
        let ui = UiAssets::blank();
        let mut fb = Framebuffer::new();
        let path = std::path::Path::new("C:\\Games\\TD2192_v1 (orig)\\TD2192.ZIP");
        assert_eq!(
            path_chars(&path.display().to_string())
                .iter()
                .collect::<String>(),
            "c:\\games\\td2192_v1 (orig)\\td2192.zip"
        );
        assert_eq!(path_chars("é~").iter().collect::<String>(), "??");
        let end = ui.path(&mut fb, path, (0, 0), INK_VALUE, 1000);
        let full: i32 = path_chars(&path.display().to_string())
            .iter()
            .map(|&c| path_advance(c))
            .sum();
        assert_eq!(end, full);
        // The colon's mark: (pen + 1 + 0, 1 + 1) and (.., 1 + 3) after "c" (advance 4).
        assert_eq!(fb.get(5, 2), Some(INK_VALUE));
        assert_eq!(fb.get(5, 4), Some(INK_VALUE));
        let mut fb = Framebuffer::new();
        let end = ui.path(&mut fb, path, (10, 20), INK_VALUE, 60);
        assert!(end - 10 <= 60, "{end}");
        assert!(end - 10 > 50, "fills the room: {end}");
    }

    /// WCAG 2.x contrast ratio of a 6-bit colour against black (6-bit values as the DAC's
    /// share of full scale, sRGB).
    fn contrast_on_black(c: Rgb6) -> f64 {
        let lin = |v: u8| {
            let s = f64::from(v) / 63.0;
            if s <= 0.04045 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        };
        let l = 0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b);
        (l + 0.05) / 0.05
    }

    /// M12-18 (F9): the inks of text read at least 4.5:1 on black (WCAG 1.4.3), and the
    /// unavailable grey at least 4.3:1 while staying well below the text grey.
    #[test]
    fn inks_have_enough_contrast() {
        let ink = |e: u8| INKS.iter().find(|(i, _)| *i == e).map(|&(_, c)| c).unwrap();
        for e in [INK_TEXT, INK_HEADING, INK_VALUE, INK_NOTE] {
            assert!(contrast_on_black(ink(e)) >= 4.5, "ink {e}");
        }
        let disabled = contrast_on_black(ink(INK_DISABLED));
        assert!(disabled >= 4.3, "{disabled}");
        assert!(disabled < contrast_on_black(ink(INK_TEXT)) / 1.5);
    }

    #[test]
    fn colours_are_set_on_any_palette() {
        let ui = UiAssets::blank();
        assert_eq!(
            ui.palette.colors[usize::from(INK_CURSOR)],
            Rgb6 { r: 63, g: 0, b: 0 }
        );
        assert_eq!(ui.palette.colors[0], Rgb6::default());
    }
}
