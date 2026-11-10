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
//!
//! The cheat chords (`docs/re/globals.md` §5, tick-order step 34; decision D9) read the bytes
//! of eleven more letters, [`ChordKey`]. They are not flight keys (nothing binds them) and
//! ride in the upper half of the replay word, so a word without them is the 64-bit word of
//! replay version 2.

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

/// The letter keys the cheat chords read besides [`FlightKey::M`] (`docs/re/globals.md` §5:
/// T+I+M, H+A+R+D, J+O+H+N, J+D+W, M+K+A+R), with their set-1 scan codes. The host passes the
/// letter keys of the keyboard as they are; they are not remappable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum ChordKey {
    /// T (14h).
    T,
    /// I (17h).
    I,
    /// H (23h).
    H,
    /// A (1Eh).
    A,
    /// R (13h).
    R,
    /// D (20h).
    D,
    /// J (24h).
    J,
    /// O (18h).
    O,
    /// N (31h).
    N,
    /// W (11h).
    W,
    /// K (25h).
    K,
}

impl ChordKey {
    /// Every chord letter, in bit order.
    pub const ALL: [ChordKey; 11] = [
        Self::T,
        Self::I,
        Self::H,
        Self::A,
        Self::R,
        Self::D,
        Self::J,
        Self::O,
        Self::N,
        Self::W,
        Self::K,
    ];

    /// The original's key-table index (set-1 scan code).
    pub const fn scan_code(self) -> u16 {
        match self {
            Self::T => 0x14,
            Self::I => 0x17,
            Self::H => 0x23,
            Self::A => 0x1E,
            Self::R => 0x13,
            Self::D => 0x20,
            Self::J => 0x24,
            Self::O => 0x18,
            Self::N => 0x31,
            Self::W => 0x11,
            Self::K => 0x25,
        }
    }

    /// The letter, lower case.
    pub const fn letter(self) -> char {
        match self {
            Self::T => 't',
            Self::I => 'i',
            Self::H => 'h',
            Self::A => 'a',
            Self::R => 'r',
            Self::D => 'd',
            Self::J => 'j',
            Self::O => 'o',
            Self::N => 'n',
            Self::W => 'w',
            Self::K => 'k',
        }
    }

    const fn bit(self) -> u16 {
        1 << self as u16
    }
}

/// Where the chord letters sit in [`KeyTable`]'s bits, above the flight keys.
const CHORD_SHIFT: u32 = FlightKey::ALL.len() as u32;

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
    /// Chord letters down at the tick (bit = `ChordKey as u8`).
    chord_down: u16,
    /// Chord letters with a make code since the previous tick.
    chord_made: u16,
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

    /// Marks chord letter `key` as down now.
    pub fn set_chord_down(&mut self, key: ChordKey) {
        self.chord_down |= key.bit();
    }

    /// Records a make code for chord letter `key` since the previous tick.
    pub fn set_chord_made(&mut self, key: ChordKey) {
        self.chord_made |= key.bit();
    }

    /// True if chord letter `key` is down now.
    pub fn is_chord_down(&self, key: ChordKey) -> bool {
        self.chord_down & key.bit() != 0
    }

    /// True if any key or chord letter got a make code since the previous tick.
    pub fn any_made(&self) -> bool {
        self.made & ((1 << FlightKey::ALL.len()) - 1) != 0 || self.chord_made != 0
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

    /// T3 analog steering: stable facing independent of this tick's thrust pulse.
    /// Spec frontend/analog-steering rev 1. Requires replay version 4.
    pub fn set_steering(&mut self, joy: Joystick) {
        const MASK: u32 = 0x4f00_0000;
        self.made &= !MASK;
        self.made |= 1 << 30;
        for (on, bit) in [
            (joy.x > 0, 24),
            (joy.x < 0, 25),
            (joy.y > 0, 26),
            (joy.y < 0, 27),
        ] {
            if on {
                self.made |= 1 << bit;
            }
        }
    }

    /// Stable T3 facing override, absent for every Classic input.
    pub fn steering(&self) -> Option<Joystick> {
        if self.made & (1 << 30) == 0 {
            return None;
        }
        let axis = |a: u32, b: u32| {
            i8::from(self.made & (1u32 << a) != 0u32) - i8::from(self.made & (1u32 << b) != 0u32)
        };
        Some(Joystick {
            x: axis(24, 25),
            y: axis(26, 27),
            ..Joystick::default()
        })
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

    /// The replay word: `down` in bits 0..32, `made` in 32..64 (replay version 2's word), the
    /// chord letters down in 64..80 and made in 96..112 (version 3). Optional stable-facing
    /// metadata occupies made bits 24..27 and 30 (version 4; frontend/analog-steering rev 1).
    pub fn to_word(self) -> u128 {
        u128::from(self.down)
            | u128::from(self.made) << 32
            | u128::from(self.chord_down) << 64
            | u128::from(self.chord_made) << 96
    }

    /// Inverse of [`Self::to_word`]. A version-2 word is the same word zero-extended.
    pub fn from_word(w: u128) -> Self {
        Self {
            down: w as u32,
            made: (w >> 32) as u32,
            chord_down: (w >> 64) as u16,
            chord_made: (w >> 96) as u16,
        }
    }
}

/// The original's key-state bytes for the keys in [`FlightKey`] (bits 0..20) and the chord
/// letters in [`ChordKey`] (bits 20..31).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KeyTable(u32);

impl KeyTable {
    /// Applies one tick's input: `byte = down && (byte || made)`.
    pub fn update(&mut self, c: &TickControls) {
        let keys = (1 << FlightKey::ALL.len()) - 1;
        let down = (c.down & keys) | u32::from(c.chord_down) << CHORD_SHIFT;
        let made = (c.made & keys) | u32::from(c.chord_made) << CHORD_SHIFT;
        let all = (1 << (CHORD_SHIFT + ChordKey::ALL.len() as u32)) - 1;
        self.0 = down & (self.0 | made) & all;
    }

    /// The byte for chord letter `key`.
    pub fn chord(&self, key: ChordKey) -> bool {
        self.0 & u32::from(key.bit()) << CHORD_SHIFT != 0
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
    fn chord_letters_follow_make_break_and_pack_above_the_v2_word() {
        let mut c = controls(&[FlightKey::M], &[FlightKey::M]);
        c.set_chord_down(ChordKey::T);
        c.set_chord_made(ChordKey::T);
        c.set_chord_down(ChordKey::K);
        let mut t = KeyTable::default();
        t.update(&c);
        assert!(t.get(FlightKey::M) && t.chord(ChordKey::T));
        assert!(!t.chord(ChordKey::K), "down without a make code");
        assert!(!t.chord(ChordKey::I));
        let w = c.to_word();
        assert_eq!(TickControls::from_word(w), c);
        // The low 64 bits are the version-2 word of the same flight keys.
        let v2 = controls(&[FlightKey::M], &[FlightKey::M]).to_word();
        assert_eq!(w as u64 as u128, v2);
        assert_eq!(
            TickControls::from_word(v2),
            controls(&[FlightKey::M], &[FlightKey::M])
        );
        assert!(c.any_made() && !TickControls::default().any_made());
        // The scan codes are the set-1 letters; none is a flight key's.
        for k in ChordKey::ALL {
            assert!(
                FlightKey::ALL
                    .iter()
                    .all(|f| f.scan_code() != k.scan_code())
            );
        }
        t.clear_all();
        assert!(!t.chord(ChordKey::T));
        // A table without chord letters hashes as before (bits 20.. are clear).
        assert_eq!(KeyTable::default().0 >> CHORD_SHIFT, 0);
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
