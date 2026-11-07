//! Input types shared by `td-platform` (which produces them) and `td-flow` (which consumes
//! them). Keys are identified by USB HID usage IDs (the values of SDL scancodes), so bindings
//! are independent of the keyboard layout.

use std::collections::BTreeSet;

/// A physical key, identified by its USB HID usage ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Key(pub u16);

impl Key {
    pub const A: Key = Key(4);
    pub const Z: Key = Key(29);
    pub const DIGIT_1: Key = Key(30);
    pub const DIGIT_0: Key = Key(39);
    pub const ENTER: Key = Key(40);
    pub const ESCAPE: Key = Key(41);
    pub const BACKSPACE: Key = Key(42);
    pub const TAB: Key = Key(43);
    pub const SPACE: Key = Key(44);
    pub const F1: Key = Key(58);
    pub const F6: Key = Key(63);
    pub const F10: Key = Key(67);
    pub const F11: Key = Key(68);
    pub const F12: Key = Key(69);
    pub const PAUSE: Key = Key(72);
    pub const INSERT: Key = Key(73);
    pub const HOME: Key = Key(74);
    pub const PAGE_UP: Key = Key(75);
    pub const DELETE: Key = Key(76);
    pub const END: Key = Key(77);
    pub const PAGE_DOWN: Key = Key(78);
    pub const RIGHT: Key = Key(79);
    pub const LEFT: Key = Key(80);
    pub const DOWN: Key = Key(81);
    pub const UP: Key = Key(82);
    pub const KP_MINUS: Key = Key(86);
    pub const KP_PLUS: Key = Key(87);
    pub const KP_ENTER: Key = Key(88);
    pub const LCTRL: Key = Key(224);
    pub const LSHIFT: Key = Key(225);
    pub const LALT: Key = Key(226);
    pub const RCTRL: Key = Key(228);
    pub const RSHIFT: Key = Key(229);
    pub const RALT: Key = Key(230);

    /// Letter key `c` (`'a'..='z'`).
    pub fn letter(c: char) -> Option<Key> {
        c.is_ascii_lowercase()
            .then(|| Key(Key::A.0 + (c as u16 - 'a' as u16)))
    }

    /// Digit key `d` (0..=9) on the main row.
    pub fn digit(d: u8) -> Option<Key> {
        match d {
            0 => Some(Key::DIGIT_0),
            1..=9 => Some(Key(Key::DIGIT_1.0 + u16::from(d) - 1)),
            _ => None,
        }
    }

    /// The key's name in settings files and on the options screens: lower case, digits and
    /// `-` only, so the game's small font can draw it. Keys without a name are `key-N` (N =
    /// the usage ID).
    pub fn name(self) -> String {
        if let Some(c) = (self.0 >= Key::A.0 && self.0 <= Key::Z.0)
            .then(|| char::from(b'a' + (self.0 - Key::A.0) as u8))
        {
            return c.to_string();
        }
        if self.0 >= Key::DIGIT_1.0 && self.0 <= Key::DIGIT_0.0 {
            return ((self.0 - Key::DIGIT_1.0 + 1) % 10).to_string();
        }
        if self.0 >= Key::F1.0 && self.0 < Key::F1.0 + 12 {
            return format!("f{}", self.0 - Key::F1.0 + 1);
        }
        if self.0 >= KP_1 && self.0 <= KP_1 + 9 {
            return format!("keypad-{}", (self.0 - KP_1 + 1) % 10);
        }
        match NAMED.iter().find(|&&(k, _)| k == self) {
            Some(&(_, n)) => n.to_string(),
            None => format!("key-{}", self.0),
        }
    }

    /// The key named `name` (see [`Key::name`]).
    pub fn from_name(name: &str) -> Option<Key> {
        let mut chars = name.chars();
        if let (Some(c), None) = (chars.next(), chars.next()) {
            if let Some(k) = Key::letter(c) {
                return Some(k);
            }
            if let Some(d) = c.to_digit(10) {
                return Key::digit(d as u8);
            }
        }
        if let Some(n) = name.strip_prefix('f').and_then(|n| n.parse::<u16>().ok()) {
            return (1..=12).contains(&n).then(|| Key(Key::F1.0 + n - 1));
        }
        if let Some(d) = name
            .strip_prefix("keypad-")
            .and_then(|n| n.parse::<u16>().ok())
        {
            return (d <= 9).then(|| Key(KP_1 + (d + 9) % 10));
        }
        if let Some(n) = name.strip_prefix("key-").and_then(|n| n.parse().ok()) {
            return Some(Key(n));
        }
        NAMED.iter().find(|&&(_, n)| n == name).map(|&(k, _)| k)
    }
}

/// Keypad 1 (keypad 2..9 and 0 follow).
const KP_1: u16 = 89;

/// Names of the keys outside the letter, digit, F-key and keypad-digit runs.
const NAMED: [(Key, &str); 43] = [
    (Key::ENTER, "enter"),
    (Key::ESCAPE, "escape"),
    (Key::BACKSPACE, "backspace"),
    (Key::TAB, "tab"),
    (Key::SPACE, "space"),
    (Key(45), "minus"),
    (Key(46), "equals"),
    (Key(47), "left-bracket"),
    (Key(48), "right-bracket"),
    (Key(49), "backslash"),
    (Key(51), "semicolon"),
    (Key(52), "apostrophe"),
    (Key(53), "grave"),
    (Key(54), "comma"),
    (Key(55), "period"),
    (Key(56), "slash"),
    (Key(57), "caps-lock"),
    (Key(70), "print-screen"),
    (Key(71), "scroll-lock"),
    (Key::PAUSE, "pause"),
    (Key::INSERT, "insert"),
    (Key::HOME, "home"),
    (Key::PAGE_UP, "page-up"),
    (Key::DELETE, "delete"),
    (Key::END, "end"),
    (Key::PAGE_DOWN, "page-down"),
    (Key::RIGHT, "right"),
    (Key::LEFT, "left"),
    (Key::DOWN, "down"),
    (Key::UP, "up"),
    (Key(83), "num-lock"),
    (Key(84), "keypad-slash"),
    (Key(85), "keypad-star"),
    (Key::KP_MINUS, "keypad-minus"),
    (Key::KP_PLUS, "keypad-plus"),
    (Key::KP_ENTER, "keypad-enter"),
    (Key(99), "keypad-period"),
    (Key::LCTRL, "left-ctrl"),
    (Key::LSHIFT, "left-shift"),
    (Key::LALT, "left-alt"),
    (Key::RCTRL, "right-ctrl"),
    (Key::RSHIFT, "right-shift"),
    (Key::RALT, "right-alt"),
];

/// A game-controller button, in SDL3's gamepad layout (South is A on an Xbox pad, cross on a
/// PlayStation pad). The triggers count as buttons when pressed past half way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PadButton {
    South,
    East,
    West,
    North,
    Back,
    Guide,
    Start,
    LeftStick,
    RightStick,
    LeftShoulder,
    RightShoulder,
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
    LeftTrigger,
    RightTrigger,
}

impl PadButton {
    /// Every button, in bit order.
    pub const ALL: [PadButton; 17] = [
        PadButton::South,
        PadButton::East,
        PadButton::West,
        PadButton::North,
        PadButton::Back,
        PadButton::Guide,
        PadButton::Start,
        PadButton::LeftStick,
        PadButton::RightStick,
        PadButton::LeftShoulder,
        PadButton::RightShoulder,
        PadButton::DPadUp,
        PadButton::DPadDown,
        PadButton::DPadLeft,
        PadButton::DPadRight,
        PadButton::LeftTrigger,
        PadButton::RightTrigger,
    ];

    /// The button's bit in [`Pad::buttons`].
    pub fn bit(self) -> u32 {
        1 << self as u32
    }

    /// The button's name in settings files and on the options screens.
    pub fn name(self) -> &'static str {
        match self {
            PadButton::South => "south",
            PadButton::East => "east",
            PadButton::West => "west",
            PadButton::North => "north",
            PadButton::Back => "back",
            PadButton::Guide => "guide",
            PadButton::Start => "start",
            PadButton::LeftStick => "left-stick",
            PadButton::RightStick => "right-stick",
            PadButton::LeftShoulder => "left-shoulder",
            PadButton::RightShoulder => "right-shoulder",
            PadButton::DPadUp => "dpad-up",
            PadButton::DPadDown => "dpad-down",
            PadButton::DPadLeft => "dpad-left",
            PadButton::DPadRight => "dpad-right",
            PadButton::LeftTrigger => "left-trigger",
            PadButton::RightTrigger => "right-trigger",
        }
    }

    /// The button named `name` (see [`PadButton::name`]).
    pub fn from_name(name: &str) -> Option<PadButton> {
        PadButton::ALL.into_iter().find(|b| b.name() == name)
    }
}

/// A game controller reduced to the original's digital joystick (`docs/re/globals.md` §3):
/// each axis is -1, 0 or 1 with the original's signs (`x` 1 = left, `y` 1 = up), plus its two
/// buttons (button 1 = afterburner, button 2 = fire in flight). `buttons` has every button of
/// the controller ([`PadButton::bit`]) for the port's own bindings and screens.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Pad {
    pub x: i8,
    pub y: i8,
    pub button1: bool,
    pub button2: bool,
    pub buttons: u32,
}

impl Pad {
    /// True if `b` is held.
    pub fn held(&self, b: PadButton) -> bool {
        self.buttons & b.bit() != 0
    }
}

/// Input gathered since the previous poll.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InputSnapshot {
    /// Keys currently held.
    pub held: BTreeSet<Key>,
    /// Keys pressed since the previous poll, in order (auto-repeat excluded).
    pub pressed: Vec<Key>,
    /// Keys auto-repeated since the previous poll (for text-like navigation).
    pub repeated: Vec<Key>,
    /// Text typed since the previous poll.
    pub text: String,
    /// The user asked to close the window.
    pub quit: bool,
    /// The first connected game controller, if any.
    pub pad: Option<Pad>,
    /// The window is in the background: it has lost the keyboard focus (M12-18).
    pub background: bool,
}

impl InputSnapshot {
    /// True if `key` is held.
    pub fn is_held(&self, key: Key) -> bool {
        self.held.contains(&key)
    }

    /// True if `key` went down since the previous poll.
    pub fn was_pressed(&self, key: Key) -> bool {
        self.pressed.contains(&key)
    }

    /// True if any key went down since the previous poll.
    pub fn any_pressed(&self) -> bool {
        !self.pressed.is_empty()
    }
}

/// Accumulates key events between snapshots (used by the platform layer).
#[derive(Debug, Default)]
pub struct InputState {
    held: BTreeSet<Key>,
    pressed: Vec<Key>,
    repeated: Vec<Key>,
    pub text: String,
    pub quit: bool,
    /// Set by the platform layer after each poll.
    pub pad: Option<Pad>,
    /// The window has lost the keyboard focus.
    pub background: bool,
}

impl InputState {
    /// Records a key press (`repeat` = OS auto-repeat).
    pub fn key_down(&mut self, key: Key, repeat: bool) {
        if repeat {
            self.repeated.push(key);
        } else if self.held.insert(key) {
            self.pressed.push(key);
        }
    }

    /// Records a key release.
    pub fn key_up(&mut self, key: Key) {
        self.held.remove(&key);
    }

    /// The window lost (`false`) or regained (`true`) the keyboard focus. Keys held when it was
    /// lost count as released: their releases go to the other window (Alt+Tab).
    pub fn focus(&mut self, focused: bool) {
        self.background = !focused;
        if !focused {
            self.held.clear();
        }
    }

    /// Returns everything since the previous snapshot.
    pub fn snapshot(&mut self) -> InputSnapshot {
        InputSnapshot {
            held: self.held.clone(),
            pressed: std::mem::take(&mut self.pressed),
            repeated: std::mem::take(&mut self.repeated),
            text: std::mem::take(&mut self.text),
            quit: self.quit,
            pad: self.pad,
            background: self.background,
        }
    }
}

/// Input delivered to one tick of the game flow: keys held now, and keys pressed since the
/// previous tick (each press is delivered to exactly one tick, even if several display frames
/// pass between ticks).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TickInput {
    pub held: BTreeSet<Key>,
    pub pressed: Vec<Key>,
    /// The game controller at the tick, if one is connected.
    pub pad: Option<Pad>,
    /// Text typed since the previous tick, in the keyboard layout's characters (the platform's
    /// text input, while a screen asks for it; M12-18). Screens that read characters rather
    /// than key positions (the save-name entry) use it.
    pub text: String,
}

impl TickInput {
    /// True if `k` is held.
    pub fn held(&self, k: Key) -> bool {
        self.held.contains(&k)
    }

    /// True if `k` was pressed since the previous tick.
    pub fn pressed(&self, k: Key) -> bool {
        self.pressed.contains(&k)
    }

    /// True if any key was pressed since the previous tick ("hit any key").
    pub fn any_pressed(&self) -> bool {
        !self.pressed.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edges_are_reported_once() {
        let mut s = InputState::default();
        s.key_down(Key::UP, false);
        s.key_down(Key::UP, true);
        let a = s.snapshot();
        assert!(a.was_pressed(Key::UP) && a.is_held(Key::UP));
        assert_eq!(a.repeated, [Key::UP]);
        let b = s.snapshot();
        assert!(!b.was_pressed(Key::UP) && b.is_held(Key::UP));
        s.key_up(Key::UP);
        assert!(!s.snapshot().is_held(Key::UP));
    }

    /// M12-18: losing the focus releases the held keys and marks the snapshot.
    #[test]
    fn focus_loss_releases_the_keys() {
        let mut s = InputState::default();
        s.key_down(Key::LALT, false);
        s.focus(false);
        let a = s.snapshot();
        assert!(a.background && a.held.is_empty());
        assert_eq!(a.pressed, [Key::LALT], "the press itself happened");
        s.focus(true);
        assert!(!s.snapshot().background);
    }

    #[test]
    fn named_keys() {
        assert_eq!(Key::letter('z'), Some(Key::Z));
        assert_eq!(Key::digit(1), Some(Key::DIGIT_1));
        assert_eq!(Key::digit(0), Some(Key::DIGIT_0));
        assert_eq!(Key::letter('A'), None);
    }

    #[test]
    fn key_names_round_trip() {
        for code in 0..=300u16 {
            let k = Key(code);
            let name = k.name();
            assert_eq!(Key::from_name(&name), Some(k), "{code} {name}");
            assert!(
                name.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "{name}"
            );
        }
        assert_eq!(Key(96).name(), "keypad-8");
        assert_eq!(Key(98).name(), "keypad-0");
        assert_eq!(Key::F6.name(), "f6");
        assert_eq!(Key::DIGIT_0.name(), "0");
        assert_eq!(Key::RALT.name(), "right-alt");
        assert_eq!(Key::from_name("f13"), None);
        assert_eq!(Key::from_name("nonsense"), None);
    }

    #[test]
    fn pad_buttons() {
        for b in PadButton::ALL {
            assert_eq!(PadButton::from_name(b.name()), Some(b));
        }
        let pad = Pad {
            buttons: PadButton::Start.bit() | PadButton::South.bit(),
            ..Pad::default()
        };
        assert!(pad.held(PadButton::Start) && pad.held(PadButton::South));
        assert!(!pad.held(PadButton::East));
    }
}
