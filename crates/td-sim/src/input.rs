//! Flight input as the original sees it: a key-state table set by make codes, cleared by break
//! codes **and by the game itself**, plus a digital joystick.
//!
//! Spec: `docs/re/globals.md` §3 (key table DS:1A5C indexed by scan code, `+100h` after an E0
//! prefix; joystick bytes DS:1F67..1F6A). Fidelity: T1.
//!
//! The original's keyboard ISR writes 1 on a make code (including typematic repeats) and 0 on a
//! break code. Game code also clears bytes (the afterburner clears its keys after use, modal
//! screens clear the whole table), so "held" is not enough: a cleared byte stays 0 until the
//! next make code even while the key is still down. Each tick therefore receives, per key,
//! whether it is down now and whether a make code arrived since the previous tick; the table
//! is updated as `byte = down && (byte || made)`. A key pressed and released within one tick
//! leaves the byte 0, as when the break arrives before the game reads the byte.

use crate::hash::{HashState, StateHasher};

/// The keys the flight code reads, with their original scan codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum FlightKey {
    /// Keypad 8 (48h).
    Kp8,
    /// Grey Up (E0 48h).
    Up,
    /// Keypad 2 (50h).
    Kp2,
    /// Grey Down (E0 50h).
    Down,
    /// Keypad 4 (4Bh).
    Kp4,
    /// Grey Left (E0 4Bh).
    Left,
    /// Keypad 6 (4Dh).
    Kp6,
    /// Grey Right (E0 4Dh).
    Right,
    /// Keypad + (4Eh).
    KpPlus,
    /// Keypad 0 / Ins (52h). The grey Insert key (E0 52h) is a different byte.
    Kp0,
    /// Left Alt (38h).
    Alt,
    /// Right Alt (E0 38h).
    RightAlt,
    /// Space (39h).
    Space,
    /// Esc (01h).
    Esc,
    /// P (19h).
    P,
    /// F5 (3Fh).
    F5,
    /// F6 (40h).
    F6,
    /// S (1Fh).
    S,
    /// Tab (0Fh).
    Tab,
    /// M (32h).
    M,
}

impl FlightKey {
    /// Every key, in bit order.
    pub const ALL: [FlightKey; 20] = [
        Self::Kp8,
        Self::Up,
        Self::Kp2,
        Self::Down,
        Self::Kp4,
        Self::Left,
        Self::Kp6,
        Self::Right,
        Self::KpPlus,
        Self::Kp0,
        Self::Alt,
        Self::RightAlt,
        Self::Space,
        Self::Esc,
        Self::P,
        Self::F5,
        Self::F6,
        Self::S,
        Self::Tab,
        Self::M,
    ];

    /// The original's key-table index (set-1 scan code, `+100h` after E0).
    pub const fn scan_code(self) -> u16 {
        match self {
            Self::Kp8 => 0x48,
            Self::Up => 0x148,
            Self::Kp2 => 0x50,
            Self::Down => 0x150,
            Self::Kp4 => 0x4B,
            Self::Left => 0x14B,
            Self::Kp6 => 0x4D,
            Self::Right => 0x14D,
            Self::KpPlus => 0x4E,
            Self::Kp0 => 0x52,
            Self::Alt => 0x38,
            Self::RightAlt => 0x138,
            Self::Space => 0x39,
            Self::Esc => 0x01,
            Self::P => 0x19,
            Self::F5 => 0x3F,
            Self::F6 => 0x40,
            Self::S => 0x1F,
            Self::Tab => 0x0F,
            Self::M => 0x32,
        }
    }

    const fn bit(self) -> u32 {
        1 << self as u32
    }
}

/// Digital joystick state as the original stores it (`g_JoyX`, `g_JoyY`: 1 = left/up,
/// -1 = right/down; two buttons).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Joystick {
    pub x: i8,
    pub y: i8,
    pub button1: bool,
    pub button2: bool,
}

// Joystick bits in the `down` mask, above the keys.
const JOY_LEFT: u32 = 1 << 24;
const JOY_RIGHT: u32 = 1 << 25;
const JOY_UP: u32 = 1 << 26;
const JOY_DOWN: u32 = 1 << 27;
const JOY_B1: u32 = 1 << 28;
const JOY_B2: u32 = 1 << 29;

/// Input for one tick. Packs into one replay word ([`Self::to_word`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TickControls {
    /// Keys down at the tick (bit = `FlightKey as u8`), plus the joystick bits.
    down: u32,
    /// Keys with a make code (press or typematic repeat) since the previous tick.
    made: u32,
}

impl TickControls {
    /// Marks `key` as down now.
    pub fn set_down(&mut self, key: FlightKey) {
        self.down |= key.bit();
    }

    /// Records a make code for `key` since the previous tick.
    pub fn set_made(&mut self, key: FlightKey) {
        self.made |= key.bit();
    }

    /// Sets the joystick (centred = default).
    pub fn set_joystick(&mut self, joy: Joystick) {
        self.down &= !(JOY_LEFT | JOY_RIGHT | JOY_UP | JOY_DOWN | JOY_B1 | JOY_B2);
        for (on, bit) in [
            (joy.x > 0, JOY_LEFT),
            (joy.x < 0, JOY_RIGHT),
            (joy.y > 0, JOY_UP),
            (joy.y < 0, JOY_DOWN),
            (joy.button1, JOY_B1),
            (joy.button2, JOY_B2),
        ] {
            if on {
                self.down |= bit;
            }
        }
    }

    /// True if `key` is down now.
    pub fn is_down(&self, key: FlightKey) -> bool {
        self.down & key.bit() != 0
    }

    /// The joystick state.
    pub fn joystick(&self) -> Joystick {
        let axis = |neg: u32, pos: u32| -> i8 {
            match (self.down & neg != 0, self.down & pos != 0) {
                (true, false) => 1,
                (false, true) => -1,
                _ => 0,
            }
        };
        Joystick {
            x: axis(JOY_LEFT, JOY_RIGHT),
            y: axis(JOY_UP, JOY_DOWN),
            button1: self.down & JOY_B1 != 0,
            button2: self.down & JOY_B2 != 0,
        }
    }

    /// The replay word: `down` in the low half, `made` in the high half.
    pub fn to_word(self) -> u64 {
        u64::from(self.down) | u64::from(self.made) << 32
    }

    /// Inverse of [`Self::to_word`].
    pub fn from_word(w: u64) -> Self {
        Self {
            down: w as u32,
            made: (w >> 32) as u32,
        }
    }
}

/// The original's key-state bytes for the keys in [`FlightKey`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KeyTable(u32);

impl KeyTable {
    /// Applies one tick's input: `byte = down && (byte || made)`.
    pub fn update(&mut self, c: &TickControls) {
        self.0 = c.down & (self.0 | c.made) & ((1 << FlightKey::ALL.len()) - 1);
    }

    /// The byte for `key`.
    pub fn get(&self, key: FlightKey) -> bool {
        self.0 & key.bit() != 0
    }

    /// Clears one byte (game code writing 0).
    pub fn clear(&mut self, key: FlightKey) {
        self.0 &= !key.bit();
    }

    /// Clears every byte (flight start, modal screens).
    pub fn clear_all(&mut self) {
        self.0 = 0;
    }
}

impl HashState for KeyTable {
    fn hash_state(&self, h: &mut StateHasher) {
        self.0.hash_state(h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn controls(down: &[FlightKey], made: &[FlightKey]) -> TickControls {
        let mut c = TickControls::default();
        down.iter().for_each(|&k| c.set_down(k));
        made.iter().for_each(|&k| c.set_made(k));
        c
    }

    #[test]
    fn key_bytes_follow_make_break_and_game_clears() {
        use FlightKey::*;
        let mut t = KeyTable::default();
        t.update(&controls(&[Kp0], &[Kp0]));
        assert!(t.get(Kp0));
        t.clear(Kp0);
        t.update(&controls(&[Kp0], &[]));
        assert!(!t.get(Kp0), "held without a new make code stays cleared");
        t.update(&controls(&[Kp0], &[Kp0]));
        assert!(t.get(Kp0), "typematic repeat sets it again");
        t.update(&controls(&[], &[]));
        assert!(!t.get(Kp0), "break");
        t.update(&controls(&[], &[Kp8]));
        assert!(!t.get(Kp8), "pressed and released within one tick");
    }

    #[test]
    fn packs_into_one_word() {
        let mut c = controls(&[FlightKey::S, FlightKey::Right], &[FlightKey::S]);
        c.set_joystick(Joystick {
            x: -1,
            y: 1,
            button1: true,
            button2: false,
        });
        let back = TickControls::from_word(c.to_word());
        assert_eq!(back, c);
        assert_eq!(
            back.joystick(),
            Joystick {
                x: -1,
                y: 1,
                button1: true,
                button2: false
            }
        );
        assert!(back.is_down(FlightKey::Right) && !back.is_down(FlightKey::Left));
        assert_eq!(FlightKey::ALL.len(), FlightKey::M as usize + 1);
    }
}
