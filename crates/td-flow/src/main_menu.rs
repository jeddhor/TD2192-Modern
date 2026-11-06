//! The main menu screen of all three episodes: `td.men` with the item colours, the menu fades,
//! the cursor pulses, the keys and the two confirmation boxes.
//!
//! Spec: `docs/spec/flow/screens.md` (rev 3) §3.1 (loader `UI_LoadMainMenu` `0000:2E88`, loop
//! `UI_MainMenuLoop` `0000:3381`), §2.1 (fades); `docs/spec/flow/episode-program-flow.md`
//! (rev 2) §4.2 (which actions fade and reload the menu); `docs/spec/flow/e2-program-flow.md`
//! rev 2 §4.2 and `e3-program-flow.md` rev 2 §4.2 for Episodes 2 and 3 ([`MenuStyle::Later`]:
//! the OB-041 colours, the same fades and ramps: 34-frame fades, the start cursor's ramp once
//! at loop entry, a ramp on every cursor change and none after an action; C-0120, C-0125).
//! Graphics: `docs/re/FORMATS.md` §12.1. Fidelity: T0 for the pixels and colours, T2 for the
//! pace.
//!
//! The menu keeps two palettes, as the original does: the palette buffer (`g_MenuPalette`
//! DS:07E0) and the DAC. Fades and the half-brightness boxes are computed from the buffer;
//! the cursor pulses write the DAC only.
//!
//! Pace (T2): fade levels take `td_render::fade::frames_per_level` frames of the palette buffer,
//! two for `tdmen.pal` as everywhere else (C-0002 frames 835..868; E2 C-0120 f272..305, E3
//! C-0125 f270..303). The pulse waits for the retrace bit, which stays set long enough for
//! several cheap steps: C-0002 shows the 48 steps of the first pulse as 11, 11, 11, 11 and 4
//! DAC writes in frames 868..872, so the port runs [`PULSE_STEPS_PER_FRAME`] steps per frame.
//! The box fade writes 256 entries per level and gets one level per frame.
//!
//! Provisional (ASSUMPTION(U-302), see `docs/re/UNKNOWNS.md`): the colour the boxes give the
//! cursor entry when the cursor is on item 3 (the loader's yellow is used); the pulse after a
//! cancelled quit box (the cursor's item fades down and up again); after the story so far
//! the menu is reloaded as by the loader, without the music and without a pulse.
//! Not ported: the `MemAvail` digits (screens §3.1, T3: 5 pixels) and the joystick's own
//! calibration.

use std::collections::VecDeque;

use td_assets::DataDir;
use td_formats::palette::{Palette, Rgb6};
use td_input::{Key, Pad, PadButton, TickInput};
use td_render::{Framebuffer, WIDTH, fade};

use crate::dialogue::KEY_BUFFER;
use crate::story::is_keystroke;

/// Menu items (episode-program-flow §4.2).
pub const ITEMS: u8 = 9;
/// Item 1: save (only while a game is in progress).
pub const SAVE: u8 = 1;
/// Item 3: next mission (only while a game is in progress; invisible unless selected).
pub const NEXT_MISSION: u8 = 3;
/// Item 8: quit.
pub const QUIT: u8 = 8;
/// First palette entry of the item colours (FORMATS §12.1).
pub const ITEM_ENTRY: usize = 224;
/// Cursor pulse steps per frame at preset P1 (C-0002 frames 868..872; T2).
pub const PULSE_STEPS_PER_FRAME: u32 = 11;
/// The joystick's `Crt.Delay(200)` after a move up, in frames (screens §3.1).
const JOY_DELAY_FRAMES: u32 = 14;
/// Menu rows the confirmation boxes cover (`UI_ShowMenuBox` `0000:330E`).
const BOX_ROWS: std::ops::Range<usize> = 84..116;
/// Entries the half-brightness box fade puts back to full (screens §3.1).
const BOX_FULL: std::ops::RangeInclusive<usize> = 192..=209;
/// The quit box and the "current game lost" box (FORMATS §12.1).
const BOX_QUIT: u8 = 1;
const BOX_GAME_LOST: u8 = 2;

const DIM: u8 = 16;
const BRIGHT: u8 = 63;

/// The menu image, the confirmation boxes (4 × 320×32) and `tdmen.pal`.
#[derive(Debug, Clone)]
pub struct MenuGraphics {
    image: Box<[u8]>,
    boxes: Box<[u8]>,
    palette: Palette,
}

impl MenuGraphics {
    /// Reads `td.men` (bytes 16,000..120,959) and `tdmen.pal`; `None` if either is missing.
    pub fn load(data: &DataDir) -> Option<Self> {
        let men = data.read("td.men").ok()?;
        let palette = Palette::parse(&data.read("tdmen.pal").ok()?).ok()?;
        Some(Self {
            image: men.get(16_000..80_000)?.into(),
            boxes: men.get(80_000..120_960)?.into(),
            palette,
        })
    }
}

/// Which program's menu: the item colours differ (e2-program-flow rev 2 §4.2 item 2, OB-041).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuStyle {
    /// Episode 1 (screens §3.1): items dim red, item 3 black, the cursor bright (yellow for
    /// item 3), DAC 255 white after the fade-in.
    E1,
    /// Episodes 2 and 3: all ten entries dim red, item 3 included; the cursor gets red 63 only
    /// (item 3 too: red, not yellow); no DAC 255 change. The ramps are E1's (item 3 yellow).
    Later,
}

/// How the menu comes back after an action (episode-program-flow §4.2; e2-program-flow §4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reentry {
    /// `UI_LoadMainMenu`: the palette reloaded and the item colours reset, fade in, DAC 255
    /// white. At program start the cursor pulses once after it.
    Loader { first: bool },
    /// After the story so far (`tdmen.pal` reloaded): as the loader, without the song.
    Story,
    /// Help: the menu fades in again from the palette buffer as it is.
    Refade,
}

/// One cursor pulse: DAC entry `entry` from `k` to `to`, in red or yellow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Pulse {
    entry: usize,
    yellow: bool,
    k: u8,
    to: u8,
}

impl Pulse {
    /// The highlight of `item` (16..63 red; item 3 0..63 yellow).
    fn up(item: u8) -> Self {
        let yellow = item == NEXT_MISSION;
        Self {
            entry: ITEM_ENTRY + usize::from(item),
            yellow,
            k: if yellow { 0 } else { DIM },
            to: BRIGHT,
        }
    }

    /// The un-highlight of `item` (63..16 red; item 3 63..0 yellow).
    fn down(item: u8) -> Self {
        let yellow = item == NEXT_MISSION;
        Self {
            entry: ITEM_ENTRY + usize::from(item),
            yellow,
            k: BRIGHT,
            to: if yellow { 0 } else { DIM },
        }
    }

    /// Writes the current level; returns false when the pulse is over.
    fn step(&mut self, dac: &mut Palette) -> bool {
        let k = self.k;
        dac.colors[self.entry] = Rgb6 {
            r: k,
            g: if self.yellow { k } else { 0 },
            b: 0,
        };
        if k == self.to {
            return false;
        }
        if self.to > k {
            self.k += 1;
        } else {
            self.k -= 1;
        }
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Fade-in level s (0..=16); `loader` runs the loader's tail afterwards.
    FadeIn { s: u8, loader: bool, pulse: bool },
    /// The queued pulses run, then the loop waits.
    Pulses,
    /// Waiting for a key, a button or the joystick.
    Wait,
    /// Box fade to half brightness, level k = 32..16.
    BoxDim { n: u8, k: u8, item: u8 },
    /// Box `n` shown; waiting for one key.
    BoxKey { n: u8, item: u8 },
    /// Fade back from half brightness, level k = 16..32.
    BoxUndim { n: u8, k: u8, item: u8, yes: bool },
    /// Fade-out level s (16..=0) before `item`'s action.
    FadeOut { s: u8, item: u8 },
    /// Faded out; `item`'s action runs.
    Chosen(u8),
}

/// The main menu: what is on screen, the palette buffer, the cursor and the key buffer.
#[derive(Debug, Clone)]
pub struct MainMenu {
    style: MenuStyle,
    gfx: Option<MenuGraphics>,
    screen: Framebuffer,
    buffer: Palette,
    cursor: u8,
    in_game: bool,
    phase: Phase,
    wait: u32,
    pulses: VecDeque<Pulse>,
    /// The BIOS keyboard buffer as the loop sees it.
    keys: Vec<Key>,
    pad: Option<Pad>,
    /// A palette-buffer entry the program changes after the loader's fade-in (the E2/E3
    /// restore bookkeeping, e2-program-flow §4.2 item 2).
    after_loader: Option<(usize, Rgb6)>,
}

/// The highlighted colour of `item` (the loader's cursor colour).
fn bright(item: u8) -> Rgb6 {
    Rgb6 {
        r: BRIGHT,
        g: if item == NEXT_MISSION { BRIGHT } else { 0 },
        b: 0,
    }
}

/// The resting colour of `item`: dim red, item 3 black.
fn resting(item: u8) -> Rgb6 {
    if item == NEXT_MISSION {
        Rgb6::default()
    } else {
        Rgb6 { r: DIM, g: 0, b: 0 }
    }
}

/// The next cursor position one step in `dir` (+1 down, -1 up), wrapping 0..8 and skipping
/// items 1 and 3 while no game is in progress (screens §3.1).
#[must_use]
pub fn move_cursor(cursor: u8, dir: i8, in_game: bool) -> u8 {
    let mut c = cursor;
    loop {
        c = if dir < 0 {
            c.checked_sub(1).unwrap_or(ITEMS - 1)
        } else {
            (c + 1) % ITEMS
        };
        if in_game || (c != SAVE && c != NEXT_MISSION) {
            return c;
        }
    }
}

impl MainMenu {
    /// The Episode 1 menu showing `gfx` (black if `None`), not yet entered.
    pub fn new(gfx: Option<MenuGraphics>) -> Self {
        Self::with_style(gfx, MenuStyle::E1)
    }

    /// A menu of `style` showing `gfx` (black if `None`), not yet entered.
    pub fn with_style(gfx: Option<MenuGraphics>, style: MenuStyle) -> Self {
        Self {
            style,
            gfx,
            screen: Framebuffer::new(),
            buffer: Palette::default(),
            cursor: 0,
            in_game: false,
            phase: Phase::Wait,
            wait: 0,
            pulses: VecDeque::new(),
            keys: Vec::new(),
            pad: None,
            after_loader: None,
        }
    }

    /// The menu cursor (`g_MenuCursor` DS:0754).
    pub fn cursor(&self) -> u8 {
        self.cursor
    }

    /// The video memory and DAC as shown.
    pub fn screen(&self) -> &Framebuffer {
        &self.screen
    }

    /// The palette buffer (`g_MenuPalette`).
    pub fn palette_buffer(&self) -> &Palette {
        &self.buffer
    }

    /// The keys typed after the selection (during the fade-out): the action's first screen
    /// finds them in the BIOS buffer (screens §2.2).
    pub fn take_keys(&mut self) -> Vec<Key> {
        std::mem::take(&mut self.keys)
    }

    /// True while the loop waits for input (no fade, pulse or box in progress).
    pub fn waiting(&self) -> bool {
        self.phase == Phase::Wait && self.wait == 0
    }

    /// Keys already in the BIOS buffer when the menu is entered (E2: typed after the script
    /// choice, e2-program-flow §4.1): the loop reads the first after its fade-in.
    pub fn type_ahead(&mut self, keys: Vec<Key>) {
        for k in keys {
            if self.keys.len() < KEY_BUFFER {
                self.keys.push(k);
            }
        }
    }

    /// Sets palette-buffer entry `entry` to `rgb` once the loader's fade-in is done (after
    /// the loader puts the cursor's colour back): the E2/E3 restore bookkeeping.
    pub fn set_after_loader(&mut self, entry: usize, rgb: Rgb6) {
        self.after_loader = Some((entry, rgb));
    }

    /// Frames per fade level of the palette buffer (screens rev 3 §2.1).
    fn fade_frames(&self) -> u32 {
        fade::frames_per_level(&self.buffer, 1)
    }

    /// Shows the menu with `cursor` after an action (or at program start), with an empty key
    /// buffer.
    pub fn enter(&mut self, cursor: u8, in_game: bool, how: Reentry) {
        self.cursor = cursor;
        self.in_game = in_game;
        self.keys.clear();
        self.pulses.clear();
        self.after_loader = None;
        if let Some(g) = &self.gfx {
            self.screen.pixels.copy_from_slice(&g.image);
        } else {
            self.screen.clear(0);
        }
        // Mode X initialisation (or the previous screen's fade-out) left the DAC black.
        self.screen.palette = Palette::default();
        let first = how == Reentry::Loader { first: true };
        let (loader, pulse) = match how {
            Reentry::Loader { .. } | Reentry::Story => {
                self.buffer = self
                    .gfx
                    .as_ref()
                    .map(|g| g.palette.clone())
                    .unwrap_or_default();
                let c = ITEM_ENTRY + usize::from(cursor);
                match self.style {
                    MenuStyle::E1 => {
                        // 0000:2E88: entries 224..233 dim red, 227 black; the cursor bright.
                        for item in 0..10 {
                            self.buffer.colors[ITEM_ENTRY + usize::from(item)] = resting(item);
                        }
                        self.buffer.colors[c] = bright(cursor);
                    }
                    MenuStyle::Later => {
                        // E2 0000:2EC0, E3 0000:29E5: all ten dim red; the cursor's red 63.
                        for e in ITEM_ENTRY..ITEM_ENTRY + 10 {
                            self.buffer.colors[e] = Rgb6 { r: DIM, g: 0, b: 0 };
                        }
                        self.buffer.colors[c].r = BRIGHT;
                    }
                }
                (true, first)
            }
            Reentry::Refade => (false, false),
        };
        self.phase = Phase::FadeIn {
            s: 0,
            loader,
            pulse,
        };
        self.wait = self.fade_frames();
    }

    /// Advances one frame; returns the chosen item once the menu has faded out for it.
    /// Items 0 and 2 with a game in progress, and Esc, ask first (boxes 2 and 1); a confirmed
    /// quit returns [`QUIT`].
    pub fn step(&mut self, input: &TickInput) -> Option<u8> {
        for &k in &input.pressed {
            if is_keystroke(k) && self.keys.len() < KEY_BUFFER {
                self.keys.push(k);
            }
        }
        let buttons = input.pad.map_or(0, |p| p.buttons);
        let pad_pressed = buttons & !self.pad.map_or(0, |p| p.buttons);
        self.pad = input.pad;
        self.wait = self.wait.saturating_sub(1);
        let mut pulse_steps = PULSE_STEPS_PER_FRAME;
        while self.wait == 0 {
            match self.phase {
                Phase::FadeIn { s, loader, pulse } => {
                    fade::write_level(&mut self.screen.palette, &self.buffer, s);
                    if s < 16 {
                        self.phase = Phase::FadeIn {
                            s: s + 1,
                            loader,
                            pulse,
                        };
                        self.wait = self.fade_frames();
                        continue;
                    }
                    if loader {
                        // The cursor entry goes back in the buffer only.
                        let c = self.cursor;
                        let e = ITEM_ENTRY + usize::from(c);
                        match self.style {
                            MenuStyle::E1 => {
                                // DAC 255 := white (screens §3.1).
                                self.buffer.colors[e] = resting(c);
                                self.screen.palette.colors[255] = Rgb6 {
                                    r: 63,
                                    g: 63,
                                    b: 63,
                                };
                            }
                            MenuStyle::Later => self.buffer.colors[e].r = DIM,
                        }
                        if let Some((e, rgb)) = self.after_loader.take() {
                            self.buffer.colors[e] = rgb;
                        }
                    }
                    if pulse {
                        // C-0002 f868: the last fade level and the first 11 pulse steps.
                        self.pulses.push_back(Pulse::up(self.cursor));
                    }
                    self.phase = Phase::Pulses;
                }
                Phase::Pulses => {
                    let Some(p) = self.pulses.front_mut() else {
                        self.phase = Phase::Wait;
                        continue;
                    };
                    if pulse_steps == 0 {
                        self.wait = 1;
                        continue;
                    }
                    pulse_steps -= 1;
                    if !p.step(&mut self.screen.palette) {
                        self.pulses.pop_front();
                    }
                }
                Phase::Wait => {
                    if !self.poll() {
                        return None;
                    }
                }
                Phase::BoxDim { n, k, item } => {
                    self.dim(k);
                    if k > 16 {
                        self.phase = Phase::BoxDim { n, k: k - 1, item };
                    } else {
                        for e in BOX_FULL {
                            self.screen.palette.colors[e] = self.buffer.colors[e];
                        }
                        self.show_box(n);
                        self.phase = Phase::BoxKey { n, item };
                        continue;
                    }
                    self.wait = 1;
                }
                Phase::BoxKey { n, item } => {
                    // Port addition (M11-04): a controller answers the box, South yes, East
                    // no, so a controller alone never gets stuck here. The keys a press's
                    // binding stands for are dropped with it.
                    let pad_answer = if pad_pressed & PadButton::South.bit() != 0 {
                        Some(true)
                    } else if pad_pressed & PadButton::East.bit() != 0 {
                        Some(false)
                    } else {
                        None
                    };
                    let yes = match pad_answer {
                        Some(yes) => {
                            self.keys.clear();
                            yes
                        }
                        None if self.keys.is_empty() => return None,
                        None => Some(self.keys.remove(0)) == Key::letter('y'),
                    };
                    self.show_box(0);
                    self.phase = Phase::BoxUndim {
                        n,
                        k: 16,
                        item,
                        yes,
                    };
                }
                Phase::BoxUndim { n, k, item, yes } => {
                    self.dim(k);
                    if k < 32 {
                        self.phase = Phase::BoxUndim {
                            n,
                            k: k + 1,
                            item,
                            yes,
                        };
                        self.wait = 1;
                        continue;
                    }
                    self.box_answered(n, item, yes);
                }
                Phase::FadeOut { s, item } => {
                    fade::write_level(&mut self.screen.palette, &self.buffer, s);
                    self.phase = match s {
                        0 => Phase::Chosen(item),
                        _ => Phase::FadeOut { s: s - 1, item },
                    };
                    self.wait = self.fade_frames();
                }
                Phase::Chosen(item) => {
                    self.phase = Phase::Wait;
                    return Some(item);
                }
            }
        }
        None
    }

    /// The loop's wait: one key (the rest of the buffer is discarded), else the joystick.
    /// Returns true if the loop moved on (its next phase starts in the same frame).
    fn poll(&mut self) -> bool {
        if let Some(&k) = self.keys.first() {
            self.keys.clear();
            match k {
                Key::UP | KP_8 => self.move_by(-1),
                Key::DOWN | KP_2 => self.move_by(1),
                Key::ENTER | Key::KP_ENTER | Key::SPACE => self.select(),
                Key::ESCAPE => self.ask(BOX_QUIT, QUIT),
                k if Some(k) == Key::digit(8) => self.move_by(-1),
                k if Some(k) == Key::digit(2) => self.move_by(1),
                _ => return false,
            }
        } else if let Some(pad) = self.pad {
            if pad.button1 || pad.button2 {
                self.select();
            } else if pad.y > 0 {
                self.move_by(-1);
                self.wait = JOY_DELAY_FRAMES;
            } else if pad.y < 0 {
                self.move_by(1);
            } else {
                return false;
            }
        } else {
            return false;
        }
        true
    }

    fn move_by(&mut self, dir: i8) {
        let old = self.cursor;
        self.cursor = move_cursor(old, dir, self.in_game);
        self.pulses.push_back(Pulse::down(old));
        self.pulses.push_back(Pulse::up(self.cursor));
        self.phase = Phase::Pulses;
    }

    fn select(&mut self) {
        let item = self.cursor;
        if self.in_game && (item == 0 || item == 2) {
            self.ask(BOX_GAME_LOST, item);
        } else {
            self.phase = Phase::FadeOut { s: 16, item };
        }
    }

    /// Starts confirmation box `n` for `item`: the cursor entry bright in the buffer, then
    /// the fade to half brightness.
    fn ask(&mut self, n: u8, item: u8) {
        let c = self.cursor;
        let e = ITEM_ENTRY + usize::from(c);
        match self.style {
            // ASSUMPTION(U-302): "set to 63" uses the loader's cursor colour (yellow for
            // item 3).
            MenuStyle::E1 => self.buffer.colors[e] = bright(c),
            // E2/E3: the red component := 63 (e2-program-flow §4.2).
            MenuStyle::Later => self.buffer.colors[e].r = BRIGHT,
        }
        self.phase = Phase::BoxDim { n, k: 32, item };
    }

    fn box_answered(&mut self, n: u8, item: u8, yes: bool) {
        match (n, yes) {
            (BOX_QUIT, true) => {
                self.cursor = QUIT;
                self.phase = Phase::FadeOut { s: 16, item: QUIT };
            }
            (BOX_QUIT, false) => {
                // A cancelled quit counts as a cursor change: the pulse runs again.
                // ASSUMPTION(U-302): the previous cursor is the current one.
                self.pulses.push_back(Pulse::down(self.cursor));
                self.pulses.push_back(Pulse::up(self.cursor));
                self.phase = Phase::Pulses;
            }
            (_, true) => self.phase = Phase::FadeOut { s: 16, item },
            (_, false) => self.phase = Phase::Wait,
        }
    }

    /// DAC := buffer × k / 32 (truncating) for all 256 entries.
    fn dim(&mut self, k: u8) {
        let scale = |v: u8| (u16::from(v) * u16::from(k) / 32) as u8;
        for (d, s) in self
            .screen
            .palette
            .colors
            .iter_mut()
            .zip(&self.buffer.colors)
        {
            *d = Rgb6 {
                r: scale(s.r),
                g: scale(s.g),
                b: scale(s.b),
            };
        }
    }

    /// `UI_ShowMenuBox(n)`: box n (1..4) over rows 84..115; box 0 puts the menu rows back.
    fn show_box(&mut self, n: u8) {
        let Some(g) = &self.gfx else { return };
        let dst = BOX_ROWS.start * WIDTH..BOX_ROWS.end * WIDTH;
        let rows = match n {
            0 => g.image.get(dst.clone()),
            _ => {
                let src = usize::from(n - 1) * BOX_ROWS.len() * WIDTH;
                g.boxes.get(src..src + BOX_ROWS.len() * WIDTH)
            }
        };
        if let Some(rows) = rows {
            self.screen.pixels[dst].copy_from_slice(rows);
        }
    }

    /// Draws the menu as shown.
    pub fn draw(&self, fb: &mut Framebuffer) {
        fb.pixels.copy_from_slice(&self.screen.pixels);
        fb.palette.clone_from(&self.screen.palette);
    }
}

/// Keypad 8 and 2: the BIOS gives extended 48h/50h without NumLock and `8`/`2` with it, so
/// both move the cursor.
const KP_8: Key = Key(96);
const KP_2: Key = Key(90);

#[cfg(test)]
mod tests {
    use super::*;

    fn gfx() -> MenuGraphics {
        let mut palette = Palette::default();
        for (i, c) in palette.colors.iter_mut().enumerate() {
            let v = (i % 64) as u8;
            *c = Rgb6 { r: v, g: v, b: v };
        }
        MenuGraphics {
            image: vec![5; 64_000].into(),
            boxes: vec![9; 40_960].into(),
            palette,
        }
    }

    fn press(k: Key) -> TickInput {
        TickInput {
            pressed: vec![k],
            ..TickInput::default()
        }
    }

    fn idle(m: &mut MainMenu, frames: u32) -> Option<u8> {
        let mut out = None;
        for _ in 0..frames {
            out = out.or(m.step(&TickInput::default()));
        }
        out
    }

    fn entry(m: &MainMenu, item: u8) -> Rgb6 {
        m.screen().palette.colors[ITEM_ENTRY + usize::from(item)]
    }

    fn red(r: u8) -> Rgb6 {
        Rgb6 { r, g: 0, b: 0 }
    }

    #[test]
    fn start_up_fades_in_then_pulses_the_cursor() {
        let mut m = MainMenu::new(Some(gfx()));
        m.enter(0, false, Reentry::Loader { first: true });
        // 17 levels, two frames each (C-0002 f835..868).
        idle(&mut m, 33);
        assert_eq!(entry(&m, 0), red(59), "level 15: Round(63 × 15 / 16)");
        idle(&mut m, 1);
        assert_eq!(
            m.screen().palette.colors[255],
            Rgb6 {
                r: 63,
                g: 63,
                b: 63
            }
        );
        // The buffer has the cursor back at dim; the pulse 16..63 starts in the frame of the
        // last level and runs 11 steps per frame (C-0002 f868..872: 11, 11, 11, 11, 4 reads
        // of the retrace bit and as many DAC writes).
        assert_eq!(m.palette_buffer().colors[ITEM_ENTRY], red(16));
        assert_eq!(entry(&m, 0), red(16 + 10));
        idle(&mut m, 3);
        assert_eq!(entry(&m, 0), red(16 + 43));
        assert!(!m.waiting());
        idle(&mut m, 1);
        assert_eq!(entry(&m, 0), red(63));
        assert!(m.waiting());
        assert_eq!(entry(&m, 3), Rgb6::default(), "item 3 invisible");
        assert_eq!(entry(&m, 1), red(16), "save visible but skipped");
    }

    #[test]
    fn keys_move_and_the_rest_of_the_buffer_is_discarded() {
        let mut m = MainMenu::new(Some(gfx()));
        m.enter(0, false, Reentry::Loader { first: false });
        idle(&mut m, 40);
        assert!(m.waiting());
        // Down then 2 in the same frame: only the first counts.
        m.step(&TickInput {
            pressed: vec![Key::DOWN, Key::digit(2).unwrap()],
            ..TickInput::default()
        });
        assert_eq!(m.cursor(), 2, "1 is skipped without a game");
        idle(&mut m, 20);
        assert_eq!(m.cursor(), 2);
        assert_eq!(entry(&m, 0), red(16));
        assert_eq!(entry(&m, 2), red(63));
        m.step(&press(Key::digit(8).unwrap()));
        assert_eq!(m.cursor(), 0);
        m.step(&press(Key::UP));
        idle(&mut m, 20);
        assert_eq!(m.cursor(), 8, "keys typed during a pulse wait for the loop");
        assert_eq!(move_cursor(8, 1, false), 0);
        assert_eq!(move_cursor(4, -1, false), 2);
        assert_eq!(move_cursor(2, 1, true), 3);
    }

    #[test]
    fn item_3_pulses_yellow() {
        let mut m = MainMenu::new(Some(gfx()));
        m.enter(2, true, Reentry::Loader { first: false });
        idle(&mut m, 40);
        m.step(&press(Key::DOWN));
        idle(&mut m, 20);
        assert_eq!(entry(&m, 3), Rgb6 { r: 63, g: 63, b: 0 });
        m.step(&press(Key::DOWN));
        idle(&mut m, 20);
        assert_eq!(entry(&m, 3), Rgb6::default(), "back to invisible");
        assert_eq!(entry(&m, 4), red(63));
    }

    #[test]
    fn selecting_fades_out_then_reports_the_item() {
        let mut m = MainMenu::new(Some(gfx()));
        m.enter(4, false, Reentry::Loader { first: false });
        idle(&mut m, 40);
        assert_eq!(m.step(&press(Key::ENTER)), None);
        // 17 levels from the buffer (the cursor entry dim there), then the action.
        assert_eq!(entry(&m, 4), red(16));
        assert_eq!(idle(&mut m, 33), None);
        assert_eq!(entry(&m, 4), Rgb6::default());
        assert_eq!(idle(&mut m, 1), Some(4));
    }

    #[test]
    fn the_quit_box_dims_to_half_and_y_quits() {
        let mut m = MainMenu::new(Some(gfx()));
        m.enter(0, false, Reentry::Loader { first: false });
        idle(&mut m, 40);
        m.step(&press(Key::ESCAPE));
        // k = 32..16, one level per frame; entries 192..209 back to full; box 1 shown.
        idle(&mut m, 16);
        let dac = &m.screen().palette;
        assert_eq!(
            dac.colors[100],
            Rgb6 {
                r: 18,
                g: 18,
                b: 18
            },
            "36 × 16 / 32"
        );
        assert_eq!(dac.colors[200], m.palette_buffer().colors[200]);
        assert_eq!(
            dac.colors[ITEM_ENTRY],
            red(31),
            "cursor 63 in the buffer, halved"
        );
        assert_eq!(m.screen().get(0, 84), Some(9));
        assert_eq!(m.screen().get(0, 83), Some(5));
        // Any other key cancels: rows restored, fade back, the cursor pulses again.
        m.step(&press(Key::letter('n').unwrap()));
        assert_eq!(m.screen().get(0, 84), Some(5));
        assert_eq!(idle(&mut m, 40), None);
        assert_eq!(m.cursor(), 0);
        assert_eq!(entry(&m, 0), red(63));
        assert!(m.waiting());
        // Y confirms: cursor 8, fade out, quit.
        m.step(&press(Key::ESCAPE));
        idle(&mut m, 17);
        m.step(&press(Key::letter('y').unwrap()));
        assert_eq!(idle(&mut m, 60), Some(QUIT));
        assert_eq!(m.cursor(), QUIT);
    }

    #[test]
    fn new_game_in_a_game_asks_with_box_2() {
        let mut m = MainMenu::new(Some(gfx()));
        m.enter(0, true, Reentry::Loader { first: false });
        idle(&mut m, 40);
        m.step(&press(Key::SPACE));
        idle(&mut m, 17);
        assert_eq!(m.screen().get(0, 84), Some(9));
        m.step(&press(Key::ESCAPE));
        assert_eq!(idle(&mut m, 60), None, "cancelled");
        assert!(m.waiting());
        m.step(&press(Key::SPACE));
        idle(&mut m, 17);
        m.step(&press(Key::letter('y').unwrap()));
        assert_eq!(idle(&mut m, 60), Some(0));
    }

    #[test]
    fn a_controller_answers_the_boxes() {
        // M11-04: South yes, East no; the Space that South's default binding stands for is
        // dropped with it.
        let pad = |b: PadButton, keys: Vec<Key>| TickInput {
            pressed: keys,
            pad: Some(Pad {
                buttons: b.bit(),
                ..Pad::default()
            }),
            ..TickInput::default()
        };
        let mut m = MainMenu::new(Some(gfx()));
        m.enter(0, true, Reentry::Loader { first: false });
        idle(&mut m, 40);
        m.step(&press(Key::SPACE));
        idle(&mut m, 17);
        m.step(&pad(PadButton::East, vec![]));
        assert_eq!(idle(&mut m, 60), None, "cancelled");
        assert!(m.waiting());
        m.step(&press(Key::SPACE));
        idle(&mut m, 17);
        m.step(&pad(PadButton::South, vec![Key::SPACE]));
        assert_eq!(idle(&mut m, 60), Some(0));
    }

    #[test]
    fn refade_keeps_the_buffer() {
        let mut m = MainMenu::new(Some(gfx()));
        m.enter(4, false, Reentry::Loader { first: false });
        idle(&mut m, 40);
        m.step(&press(Key::ENTER));
        assert_eq!(idle(&mut m, 40), Some(4));
        m.enter(4, false, Reentry::Refade);
        idle(&mut m, 34);
        assert_eq!(
            m.screen().palette.colors[1..],
            m.palette_buffer().colors[1..]
        );
        assert!(m.waiting(), "no pulse after an action");
    }
}
