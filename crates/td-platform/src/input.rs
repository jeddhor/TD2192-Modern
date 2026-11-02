//! Keyboard state tracking with edge detection.
//!
//! Keys are identified by USB HID usage IDs (the values of SDL scancodes), so bindings are
//! independent of the keyboard layout.

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

#[derive(Debug, Default)]
pub(crate) struct InputState {
    held: BTreeSet<Key>,
    pressed: Vec<Key>,
    repeated: Vec<Key>,
    pub(crate) text: String,
    pub(crate) quit: bool,
}

impl InputState {
    pub(crate) fn key_down(&mut self, key: Key, repeat: bool) {
        if repeat {
            self.repeated.push(key);
        } else if self.held.insert(key) {
            self.pressed.push(key);
        }
    }

    pub(crate) fn key_up(&mut self, key: Key) {
        self.held.remove(&key);
    }

    pub(crate) fn snapshot(&mut self) -> InputSnapshot {
        InputSnapshot {
            held: self.held.clone(),
            pressed: std::mem::take(&mut self.pressed),
            repeated: std::mem::take(&mut self.repeated),
            text: std::mem::take(&mut self.text),
            quit: self.quit,
        }
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

    #[test]
    fn named_keys() {
        assert_eq!(Key::letter('z'), Some(Key::Z));
        assert_eq!(Key::digit(1), Some(Key::DIGIT_1));
        assert_eq!(Key::digit(0), Some(Key::DIGIT_0));
        assert_eq!(Key::letter('A'), None);
    }
}
