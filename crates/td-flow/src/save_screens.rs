//! The save and restore screens of the main menu (items 1 and 2: `Sav_SaveScreen`
//! `051A:7EEA`, `Sav_RestoreScreen` `051A:76EA`) and the saved-game files behind them.
//!
//! Spec: `docs/spec/flow/save-screens.md` (rev 1); file layout `docs/re/FORMATS.md` §13.2–13.3
//! (`td_formats::saves`). Fidelity: T0 for the screen and palette, T1 for the bytes written and
//! the campaign state after a restore, T2 for the fades and the polling.
//!
//! The port keeps its own `td.sav` and `td.stt` in the user data directory, in the original
//! byte format, and starts with an empty set: the player's original saves in the game folder
//! are never read (importing them is decision D3). A missing or short file means empty slots,
//! and the first save creates the files (OB-055, fix-all).
//!
//! The original's busy loop handles one key per pass and redraws the highlight on every pass;
//! here every key waiting in the buffer is handled in the frame it arrives, and the highlight
//! is redrawn after each change (the pixels are the same).
//!
//! OB-056 (fix-enhanced): the original ignores Esc during the name entry, so a slot chosen by
//! mistake is lost. With [`SaveScreen::esc_cancels_entry`] (the Enhanced profile's original-bug
//! fixes), Esc puts the slot back as it was and returns to the slot choice.

use std::path::{Path, PathBuf};

use td_assets::DataDir;
use td_formats::font::Font;
use td_formats::palette::Palette;
use td_formats::saves::{SLOTS, SaveNames, SaveSlots, Slot};
use td_input::{Key, Pad, TickInput};
use td_render::{Framebuffer, WIDTH, fade, small_text};

use crate::dialogue::KEY_BUFFER;
use crate::story::is_keystroke;

/// The Episode 1 file names (E2 and E3 use `td2.*` and `td3.*`, save-screens §4.5).
pub const E1_FILES: SaveFiles = SaveFiles {
    sav: "td.sav",
    stt: "td.stt",
};

/// Ink of the column headings (save-screens §3.2).
const HEADING_INK: u8 = 80;
/// Ink of the slot text.
const NORMAL_INK: u8 = 92;
/// Ink of the highlighted slot and of the name entry.
const HIGHLIGHT_INK: u8 = 31;
/// The field colour of the `td.sss` boxes; also the opaque background of the name entry
/// (`td.lt2` byte 650 in the shipped font).
const FIELD: u8 = 178;
/// Rows of one `td.sss` bar.
const BAR_ROWS: usize = 19;
/// First screen row of the bars.
const BAR_TOP: usize = 10;
/// Pen x of the name, mission and kills columns, and of the three headings.
const NAME_X: i32 = 43;
const MISSION_X: i32 = 209;
const KILLS_X: i32 = 250;
const HEADINGS: [(i32, i32); 3] = [(45, 2), (200, 2), (247, 2)];
/// The fields cleared when a slot is chosen for saving: x ranges, 7 rows from the pen y.
const CLEARED: [std::ops::RangeInclusive<i32>; 2] = [209..=225, 250..=266];
/// Spaces drawn over the old name when the entry starts (`saves.save[5]` is 31 spaces).
const BLANK_NAME: usize = 31;
/// Characters a name can have, and the padded width of a stored name.
const MAX_TYPED: usize = 20;
const NAME_WIDTH: usize = 24;
/// The joystick's `Crt.Delay(200)` after a move, in frames.
const JOY_DELAY_FRAMES: u32 = 14;

/// The two files of an episode's saves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SaveFiles {
    pub sav: &'static str,
    pub stt: &'static str,
}

/// The saved games as stored: `td.sav` slots and `td.stt` lines.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SaveSet {
    pub slots: SaveSlots,
    pub names: SaveNames,
}

impl SaveSet {
    /// Reads the files from `dir`; missing files give empty slots (OB-055, fix-all).
    pub fn load(dir: Option<&Path>, files: SaveFiles) -> Self {
        let read = |name: &str| dir.and_then(|d| std::fs::read(d.join(name)).ok());
        Self {
            slots: read(files.sav)
                .map(|b| SaveSlots::parse(&b))
                .unwrap_or_default(),
            names: read(files.stt)
                .map(|b| SaveNames::parse(&b))
                .unwrap_or_default(),
        }
    }

    /// Writes both files to `dir` (created if needed).
    pub fn store(&self, dir: &Path, files: SaveFiles) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        std::fs::write(dir.join(files.sav), self.slots.to_bytes())?;
        std::fs::write(dir.join(files.stt), self.names.to_bytes())
    }
}

/// Which screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveKind {
    /// Menu item 1 with the game in progress (`g_MissionNumber`, `g_KillsToDate`).
    Save { mission: u8, kills: i16 },
    /// Menu item 2.
    Restore,
}

/// How the screen ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveOutcome {
    /// Esc (restore screen): nothing changed.
    Left,
    /// A used slot was restored: the new mission number and kills to date.
    Restored { mission: u8, kills: i16 },
    /// A game was saved: the files to write.
    Saved(Box<SaveSet>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    FadeIn(u8),
    Choose,
    Name,
    FadeOut(u8),
    Done,
}

/// The chosen slot and the screen as they were before the name entry (OB-056 fix).
#[derive(Debug, Clone)]
struct EntryBackup {
    slot: Slot,
    name: String,
    pixels: Box<[u8]>,
}

/// The save or restore screen.
#[derive(Debug, Clone)]
pub struct SaveScreen {
    kind: SaveKind,
    screen: Framebuffer,
    pal: Palette,
    font: Font,
    /// `saves.*[0..3]`: the empty label and the three headings.
    labels: [String; 4],
    set: SaveSet,
    /// Cursor 1..=10.
    cursor: usize,
    typed: String,
    phase: Phase,
    wait: u32,
    keys: Vec<(Key, bool)>,
    pad: Option<Pad>,
    /// The controller's South button (joystick button 2) was down in the previous frame.
    south_held: bool,
    outcome: Option<SaveOutcome>,
    /// OB-056 fixed: Esc cancels the name entry (Enhanced).
    esc_cancels: bool,
    /// The chosen slot as it was before the entry, and the screen then (for the cancel).
    before_entry: Option<Box<EntryBackup>>,
}

/// Pen y of slot `s` (1..=10): 19 s − 3.
fn slot_y(s: usize) -> i32 {
    19 * s as i32 - 3
}

impl SaveScreen {
    /// Builds the screen as the original draws it before its fade-in (save-screens §4.1):
    /// `pal` is the menu's palette buffer, `labels` the `saves.save` or `saves.restore` list,
    /// `sss` the `td.sss` bar (`None` if missing), `set` the files as read.
    pub fn new(
        kind: SaveKind,
        pal: &Palette,
        font: &Font,
        labels: &[String],
        sss: Option<&[u8]>,
        set: SaveSet,
    ) -> Self {
        let label = |i: usize| labels.get(i).cloned().unwrap_or_default();
        let mut s = Self {
            kind,
            screen: Framebuffer::new(),
            pal: pal.clone(),
            font: font.clone(),
            labels: [label(0), label(1), label(2), label(3)],
            set,
            cursor: 1,
            typed: String::new(),
            phase: Phase::FadeIn(0),
            wait: fade::first_level_frames(pal, 1, 0),
            keys: Vec::new(),
            pad: None,
            south_held: false,
            outcome: None,
            esc_cancels: false,
            before_entry: None,
        };
        // Empty slots (mission 0 or > 20) become mission 0 and show the empty label.
        for i in 0..SLOTS {
            if !s.set.slots.slots[i].used() {
                s.set.slots.slots[i].mission = 0;
                s.set.names.names[i].clone_from(&s.labels[0]);
            }
        }
        // The DAC is black (the menu faded out); rows 0..9 colour 0, the headings, the bars.
        s.screen.pixels[..BAR_TOP * WIDTH].fill(0);
        for (i, &pen) in HEADINGS.iter().enumerate() {
            small_text::draw(&mut s.screen, &s.font, &s.labels[i + 1], pen, HEADING_INK);
        }
        if let Some(bar) = sss.and_then(|b| b.get(..BAR_ROWS * WIDTH)) {
            for k in 0..SLOTS {
                let top = (BAR_TOP + BAR_ROWS * k) * WIDTH;
                s.screen.pixels[top..top + bar.len()].copy_from_slice(bar);
            }
        }
        for slot in 1..=SLOTS {
            s.draw_slot(slot, NORMAL_INK);
        }
        s
    }

    /// Loads the font, labels, `td.sss` and the files, then builds the screen.
    pub fn open(
        kind: SaveKind,
        data: &DataDir,
        font: &Font,
        labels: &[String],
        pal: &Palette,
        set: SaveSet,
    ) -> Self {
        let sss = data.read("td.sss").ok();
        if sss.is_none() {
            tracing::warn!("td.sss unavailable; the save slots are drawn without bars");
        }
        Self::new(kind, pal, font, labels, sss.as_deref(), set)
    }

    /// With `on`, Esc during the name entry cancels it (OB-056 fixed; the Enhanced profile's
    /// original-bug fixes, `Settings::enhanced_fixes`). Off, Esc is ignored there, as in the
    /// original (Classic).
    #[must_use]
    pub fn esc_cancels_entry(mut self, on: bool) -> Self {
        self.esc_cancels = on;
        self
    }

    /// The video memory and DAC as shown.
    pub fn screen(&self) -> &Framebuffer {
        &self.screen
    }

    /// The slot cursor (1..=10).
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// True while the name of a saved game is typed.
    pub fn entering_name(&self) -> bool {
        self.phase == Phase::Name
    }

    /// True while the screen waits for a choice (faded in, no name entry).
    pub fn choosing(&self) -> bool {
        self.phase == Phase::Choose && self.wait == 0
    }

    /// How the screen ended, once it has faded out.
    pub fn outcome(&self) -> Option<&SaveOutcome> {
        if self.phase == Phase::Done {
            self.outcome.as_ref()
        } else {
            None
        }
    }

    /// Advances one frame.
    pub fn step(&mut self, input: &TickInput) {
        let shift = input.held(Key::LSHIFT) || input.held(Key::RSHIFT);
        // Port addition (port-front-end §5, M12-07): a press of South (joystick button 2) ends
        // the name entry as Enter does, so a controller alone can save. The keys its binding
        // stands for in that frame (Space by default) are not typed.
        let south = input.pad.is_some_and(|p| p.button2);
        let south_pressed = south && !self.south_held;
        self.south_held = south;
        if south_pressed && self.phase == Phase::Name {
            self.keys.push((Key::ENTER, false));
        } else {
            for &k in &input.pressed {
                if is_keystroke(k) && self.keys.len() < KEY_BUFFER {
                    self.keys.push((k, shift));
                }
            }
        }
        self.pad = input.pad;
        self.wait = self.wait.saturating_sub(1);
        while self.wait == 0 {
            match self.phase {
                Phase::FadeIn(s) => {
                    fade::write_level(&mut self.screen.palette, &self.pal, s);
                    if s < 16 {
                        self.phase = Phase::FadeIn(s + 1);
                        self.wait = fade::level_frames(&self.pal, 1, s + 1);
                    } else {
                        // The first pass of the loop draws the highlight.
                        self.draw_slot(self.cursor, HIGHLIGHT_INK);
                        self.phase = Phase::Choose;
                    }
                }
                Phase::Choose => {
                    if !self.choose() {
                        return;
                    }
                }
                Phase::Name => {
                    if self.keys.is_empty() {
                        return;
                    }
                    let (k, shift) = self.keys.remove(0);
                    self.name_key(k, shift);
                }
                Phase::FadeOut(s) => {
                    fade::write_level(&mut self.screen.palette, &self.pal, s);
                    if s > 0 {
                        self.phase = Phase::FadeOut(s - 1);
                        self.wait = fade::level_frames(&self.pal, 1, s - 1);
                    } else {
                        self.phase = Phase::Done;
                    }
                }
                Phase::Done => return,
            }
        }
    }

    /// One pass of the selection loop (§4.2); false when there was nothing to do. Keys are
    /// handled as fast as they come; the joystick at most once per frame (a move then waits
    /// for its `Crt.Delay(200)`).
    fn choose(&mut self) -> bool {
        let (up, down, select, esc) = if let Some(&(k, _)) = self.keys.first() {
            self.keys.remove(0);
            (
                k == Key::UP || k == KP_8 || Some(k) == Key::digit(8),
                k == Key::DOWN || k == KP_2 || Some(k) == Key::digit(2),
                k == Key::SPACE || k == Key::ENTER || k == Key::KP_ENTER,
                k == Key::ESCAPE,
            )
        } else if let Some(p) = self
            .pad
            .take()
            .filter(|p| p.y != 0 || p.button1 || p.button2)
        {
            self.wait = if p.y != 0 { JOY_DELAY_FRAMES } else { 1 };
            (p.y > 0, p.y < 0, p.button1 || p.button2, false)
        } else {
            return false;
        };
        if up || down {
            self.draw_slot(self.cursor, NORMAL_INK);
            self.cursor = match (up, self.cursor) {
                (true, 1) => SLOTS,
                (true, c) => c - 1,
                (false, SLOTS) => 1,
                (false, c) => c + 1,
            };
        }
        let mut leave = esc;
        if select {
            match self.kind {
                SaveKind::Restore => {
                    let slot = self.set.slots.slots[self.cursor - 1];
                    if slot.mission != 0 {
                        self.outcome = Some(SaveOutcome::Restored {
                            mission: slot.mission,
                            kills: slot.kills as i16,
                        });
                        leave = true;
                    }
                }
                SaveKind::Save { .. } => {
                    self.start_name();
                    return true;
                }
            }
        }
        self.draw_slot(self.cursor, HIGHLIGHT_INK);
        if leave {
            self.outcome.get_or_insert(SaveOutcome::Left);
            self.start_fade_out();
        }
        true
    }

    /// The closing fade-out, from code that ran in this frame: level 16 is shown
    /// [`fade::first_level_frames`] later (screens rev 3 §2.1, M12-14).
    fn start_fade_out(&mut self) {
        self.phase = Phase::FadeOut(16);
        self.wait = fade::first_level_frames(&self.pal, 1, 16);
    }

    /// The chosen slot's fields are cleared and it gets the game in progress (§4.4).
    fn start_name(&mut self) {
        let SaveKind::Save { mission, kills } = self.kind else {
            return;
        };
        let i = self.cursor - 1;
        self.before_entry = Some(Box::new(EntryBackup {
            slot: self.set.slots.slots[i],
            name: self.set.names.names[i].clone(),
            pixels: self.screen.pixels.clone(),
        }));
        let y = slot_y(self.cursor);
        for xs in CLEARED {
            for x in xs {
                for row in y..y + 7 {
                    self.screen.put(x, row, FIELD);
                }
            }
        }
        self.set.names.names[i] = " ".repeat(BLANK_NAME);
        self.set.slots.slots[i] = Slot {
            mission,
            kills: kills as u16,
        };
        self.typed.clear();
        self.draw_entry();
        self.phase = Phase::Name;
    }

    /// One key of the name entry: Backspace, a–z (lower case), 0–9, space, `-`, `.`; Enter
    /// saves. Everything else is ignored, Esc included (OB-056) unless the fix is on, where Esc
    /// cancels the entry.
    fn name_key(&mut self, k: Key, shift: bool) {
        if k == Key::ESCAPE && self.esc_cancels {
            self.cancel_entry();
            return;
        }
        if k == Key::BACKSPACE {
            self.typed.pop();
        } else if self.typed.len() < MAX_TYPED
            && let Some(c) = name_char(k, shift)
        {
            self.typed.push(c);
        }
        let i = self.cursor - 1;
        let mut name = format!(" {}", self.typed);
        while name.len() < NAME_WIDTH {
            name.push(' ');
        }
        self.set.names.names[i] = name;
        if k == Key::ENTER || k == Key::KP_ENTER {
            self.draw_slot(self.cursor, HIGHLIGHT_INK);
            self.outcome = Some(SaveOutcome::Saved(Box::new(self.set.clone())));
            self.start_fade_out();
        } else {
            self.draw_entry();
        }
    }

    /// OB-056 fixed: the slot and the screen as they were before the entry, the highlight on
    /// it, and back to the slot choice.
    fn cancel_entry(&mut self) {
        if let Some(b) = self.before_entry.take() {
            let i = self.cursor - 1;
            self.set.slots.slots[i] = b.slot;
            self.set.names.names[i] = b.name;
            self.screen.pixels = b.pixels;
        }
        self.typed.clear();
        self.draw_slot(self.cursor, HIGHLIGHT_INK);
        self.phase = Phase::Choose;
    }

    /// The entry's opaque redraw: mission, kills and name in ink 31 on the field colour.
    fn draw_entry(&mut self) {
        let i = self.cursor - 1;
        let y = slot_y(self.cursor);
        let slot = self.set.slots.slots[i];
        let font = &self.font;
        let fb = &mut self.screen;
        let ink = HIGHLIGHT_INK;
        small_text::draw_opaque(
            fb,
            font,
            &slot.mission.to_string(),
            (MISSION_X, y),
            ink,
            FIELD,
        );
        small_text::draw_opaque(fb, font, &slot.kills.to_string(), (KILLS_X, y), ink, FIELD);
        small_text::draw_opaque(fb, font, &self.set.names.names[i], (NAME_X, y), ink, FIELD);
    }

    /// Slot `s` drawn transparent in `ink`: mission and kills (used slots only), then the name.
    fn draw_slot(&mut self, s: usize, ink: u8) {
        let slot = self.set.slots.slots[s - 1];
        let y = slot_y(s);
        if slot.mission != 0 {
            let fb = &mut self.screen;
            small_text::draw(
                fb,
                &self.font,
                &slot.mission.to_string(),
                (MISSION_X, y),
                ink,
            );
            small_text::draw(fb, &self.font, &slot.kills.to_string(), (KILLS_X, y), ink);
        }
        let name = &self.set.names.names[s - 1];
        small_text::draw(&mut self.screen, &self.font, name, (NAME_X, y), ink);
    }

    /// Draws the screen as shown.
    pub fn draw(&self, fb: &mut Framebuffer) {
        fb.pixels.copy_from_slice(&self.screen.pixels);
        fb.palette.clone_from(&self.screen.palette);
    }
}

/// The character a key adds to a name, if any (`ReadKey`, then the §4.4 filter; Shift turns
/// digits, `-` and `.` into other characters, which are ignored).
fn name_char(k: Key, shift: bool) -> Option<char> {
    if (Key::A.0..=Key::Z.0).contains(&k.0) {
        return char::from_u32(u32::from(b'a') + u32::from(k.0 - Key::A.0));
    }
    if k == Key::SPACE {
        return Some(' ');
    }
    if k == KP_MINUS_KEY {
        return Some('-');
    }
    if shift {
        return None;
    }
    match k {
        Key::DIGIT_0 => Some('0'),
        Key(d) if (Key::DIGIT_1.0..=Key(Key::DIGIT_1.0 + 8).0).contains(&d) => {
            char::from_digit(u32::from(d - Key::DIGIT_1.0) + 1, 10)
        }
        MINUS => Some('-'),
        PERIOD => Some('.'),
        _ => None,
    }
}

/// Keypad 8 and 2 (extended 48h/50h without NumLock, `8`/`2` with it).
const KP_8: Key = Key(96);
const KP_2: Key = Key(90);
/// `-` and `.` on the main block, and keypad `-`.
const MINUS: Key = Key(45);
const PERIOD: Key = Key(55);
const KP_MINUS_KEY: Key = Key::KP_MINUS;

/// The directory the port keeps its saves in: `saves` under the user data directory.
#[must_use]
pub fn save_dir(user_dir: &Path) -> PathBuf {
    user_dir.join("saves")
}

#[cfg(test)]
mod tests {
    use super::*;
    use td_formats::font::FontKind;

    fn font() -> Font {
        Font::parse(&vec![0xF7; FontKind::Small.file_len()], FontKind::Small).unwrap()
    }

    fn labels() -> Vec<String> {
        ["-", "n", "m", "k"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect()
    }

    fn press(k: Key) -> TickInput {
        TickInput {
            pressed: vec![k],
            ..TickInput::default()
        }
    }

    fn settle(s: &mut SaveScreen) {
        for _ in 0..40 {
            s.step(&TickInput::default());
        }
    }

    fn set_with(slot: usize, mission: u8, kills: u16) -> SaveSet {
        let mut set = SaveSet::default();
        set.slots.slots[slot] = Slot { mission, kills };
        set.names.names[slot] = " old".to_owned();
        set
    }

    #[test]
    fn restore_ignores_empty_slots_and_loads_used_ones() {
        let pal = Palette::default();
        let mut s = SaveScreen::new(
            SaveKind::Restore,
            &pal,
            &font(),
            &labels(),
            None,
            set_with(2, 9, 0xFFFF),
        );
        settle(&mut s);
        assert!(s.choosing());
        s.step(&press(Key::ENTER));
        assert!(s.choosing(), "slot 1 is empty");
        s.step(&TickInput {
            pressed: vec![Key::DOWN, Key::digit(2).unwrap(), Key::ENTER],
            ..TickInput::default()
        });
        assert_eq!(s.cursor(), 3);
        settle(&mut s);
        assert_eq!(
            s.outcome(),
            Some(&SaveOutcome::Restored {
                mission: 9,
                kills: -1
            })
        );
    }

    #[test]
    fn cursor_wraps_and_esc_leaves() {
        let mut s = SaveScreen::new(
            SaveKind::Restore,
            &Palette::default(),
            &font(),
            &labels(),
            None,
            SaveSet::default(),
        );
        settle(&mut s);
        s.step(&press(Key::UP));
        assert_eq!(s.cursor(), 10);
        s.step(&press(Key::digit(2).unwrap()));
        assert_eq!(s.cursor(), 1);
        s.step(&press(Key::ESCAPE));
        settle(&mut s);
        assert_eq!(s.outcome(), Some(&SaveOutcome::Left));
    }

    #[test]
    fn save_name_rules() {
        let mut s = SaveScreen::new(
            SaveKind::Save {
                mission: 7,
                kills: 1234,
            },
            &Palette::default(),
            &font(),
            &labels(),
            None,
            set_with(9, 21, 5),
        );
        settle(&mut s);
        s.step(&press(Key::DOWN));
        s.step(&press(Key::ENTER));
        assert!(s.entering_name());
        let l = |c| Key::letter(c).unwrap();
        // r, Shift+e, '.', '-', 7, space, ',' (ignored), Esc (ignored), a, b, Backspace.
        for (k, shift) in [
            (l('r'), false),
            (l('e'), true),
            (PERIOD, false),
            (MINUS, false),
            (Key::digit(7).unwrap(), false),
            (Key::SPACE, false),
            (Key(54), false),
            (Key::ESCAPE, false),
            (Key::digit(1).unwrap(), true),
            (l('a'), false),
            (l('b'), false),
            (Key::BACKSPACE, false),
        ] {
            let held = if shift {
                [Key::LSHIFT].into_iter().collect()
            } else {
                Default::default()
            };
            s.step(&TickInput {
                pressed: vec![k],
                held,
                pad: None,
            });
        }
        assert!(s.entering_name(), "Esc does not cancel");
        s.step(&press(Key::ENTER));
        settle(&mut s);
        let Some(SaveOutcome::Saved(set)) = s.outcome() else {
            panic!("{:?}", s.outcome());
        };
        assert_eq!(set.names.names[1], format!("{:<24}", " re.-7 a"));
        assert_eq!(
            set.slots.slots[1],
            Slot {
                mission: 7,
                kills: 1234
            }
        );
        assert_eq!(set.names.names[0], "-", "empty slots get the label");
        assert_eq!(set.slots.slots[9].mission, 0, "21 is normalised");
        assert_eq!(set.slots.slots[9].kills, 5, "kills word kept");
    }

    /// M12-07 (port-front-end §5): a controller alone can save. South chooses the slot (as its
    /// Space binding), and its next press ends the name entry like Enter without typing a space.
    #[test]
    fn south_ends_the_name_entry() {
        let mut s = SaveScreen::new(
            SaveKind::Save {
                mission: 7,
                kills: 99,
            },
            &Palette::default(),
            &font(),
            &labels(),
            None,
            set_with(1, 4, 12),
        );
        settle(&mut s);
        let south = |down: bool, keys: Vec<Key>| TickInput {
            pressed: keys,
            held: Default::default(),
            pad: Some(Pad {
                button2: down,
                ..Pad::default()
            }),
        };
        // The press that chooses the slot comes with its binding's Space.
        s.step(&south(true, vec![Key::SPACE]));
        assert!(s.entering_name());
        s.step(&south(true, Vec::new()));
        s.step(&south(false, Vec::new()));
        assert!(s.entering_name(), "held South does not save");
        s.step(&press(Key::letter('a').unwrap()));
        s.step(&south(true, vec![Key::SPACE]));
        settle(&mut s);
        let Some(SaveOutcome::Saved(set)) = s.outcome() else {
            panic!("{:?}", s.outcome());
        };
        assert_eq!(
            set.names.names[0],
            format!("{:<24}", " a"),
            "no space typed"
        );
        assert_eq!(set.slots.slots[0].mission, 7);
    }

    /// OB-056: Classic ignores Esc in the name entry (the slot is lost once chosen); with the
    /// Enhanced fix Esc puts the slot and the screen back and returns to the slot choice.
    #[test]
    fn esc_cancels_the_name_entry_only_with_the_fix() {
        let open = |fix: bool| {
            let mut s = SaveScreen::new(
                SaveKind::Save {
                    mission: 7,
                    kills: 99,
                },
                &Palette::default(),
                &font(),
                &labels(),
                None,
                set_with(1, 4, 12),
            )
            .esc_cancels_entry(fix);
            settle(&mut s);
            s.step(&press(Key::DOWN));
            s
        };
        // Classic: Esc is ignored and Enter writes over slot 2.
        let mut s = open(false);
        let before = s.screen().pixels.clone();
        s.step(&press(Key::ENTER));
        assert!(s.entering_name());
        s.step(&press(Key::letter('x').unwrap()));
        s.step(&press(Key::ESCAPE));
        assert!(s.entering_name(), "Classic: Esc does not cancel");
        s.step(&press(Key::ENTER));
        settle(&mut s);
        let Some(SaveOutcome::Saved(set)) = s.outcome() else {
            panic!("{:?}", s.outcome());
        };
        assert_eq!(set.slots.slots[1].mission, 7, "the old game is lost");
        // Enhanced: Esc restores the slot and the picture; the choice goes on.
        let mut s = open(true);
        s.step(&press(Key::ENTER));
        s.step(&press(Key::letter('x').unwrap()));
        s.step(&press(Key::ESCAPE));
        assert!(!s.entering_name());
        assert!(s.choosing());
        assert_eq!(s.screen().pixels, before, "the slot drawn as before");
        assert_eq!(
            s.set.slots.slots[1],
            Slot {
                mission: 4,
                kills: 12
            }
        );
        assert_eq!(s.set.names.names[1], " old");
        // A second Esc leaves without saving.
        s.step(&press(Key::ESCAPE));
        settle(&mut s);
        assert_eq!(s.outcome(), Some(&SaveOutcome::Left));
        // The restore screen ignores the setting (nothing to cancel).
        let mut r = SaveScreen::new(
            SaveKind::Restore,
            &Palette::default(),
            &font(),
            &labels(),
            None,
            SaveSet::default(),
        )
        .esc_cancels_entry(true);
        settle(&mut r);
        r.step(&press(Key::ESCAPE));
        settle(&mut r);
        assert_eq!(r.outcome(), Some(&SaveOutcome::Left));
    }

    #[test]
    fn names_stop_at_20_characters_and_backspace_still_works() {
        let mut s = SaveScreen::new(
            SaveKind::Save {
                mission: 2,
                kills: 0,
            },
            &Palette::default(),
            &font(),
            &labels(),
            None,
            SaveSet::default(),
        );
        settle(&mut s);
        s.step(&press(Key::SPACE));
        for c in 'a'..='y' {
            s.step(&press(Key::letter(c).unwrap()));
        }
        s.step(&press(Key::BACKSPACE));
        s.step(&press(Key::letter('z').unwrap()));
        s.step(&press(Key::ENTER));
        settle(&mut s);
        let Some(SaveOutcome::Saved(set)) = s.outcome() else {
            panic!();
        };
        assert_eq!(set.names.names[0], " abcdefghijklmnopqrsz   ");
    }

    #[test]
    fn files_round_trip_in_a_directory() {
        let dir = std::env::temp_dir().join(format!("td2192-saves-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(SaveSet::load(Some(&dir), E1_FILES), SaveSet::default());
        let set = set_with(4, 3, 77);
        set.store(&dir, E1_FILES).unwrap();
        assert_eq!(SaveSet::load(Some(&dir), E1_FILES), set);
        assert_eq!(std::fs::read(dir.join("td.sav")).unwrap().len(), 70);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
