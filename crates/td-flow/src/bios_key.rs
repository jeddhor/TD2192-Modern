//! The bytes a keystroke puts into the BIOS keyboard buffer, as BP7 `ReadKey` returns them: the
//! character code, or for an extended key a 0 and then its scan code.
//!
//! The E2 and E3 main menus read one key per poll and discard every other waiting key, keeping
//! the last byte `ReadKey` returned in `g_MenuDrainKey` (e2-program-flow rev 3 §4.2 item 5,
//! e3-program-flow rev 3 §4.2 item 5), which becomes a two-digit muzzle byte (weapons rev 5
//! §3.3.2 item 2). [`last_byte`] gives that byte for a port key.
//!
//! Layout: US (preset P1, `keyboardlayout=us`); the keys come from INT 16h function 00h, which
//! turns the grey keys' E0h into 00h and drops F11 and F12. ASSUMPTION(U-491): NumLock is off,
//! so the keypad's digit keys are extended keys (keypad 8 = 0, 48h, as save-screens §4.1 names
//! it); keypad 5 then gives no keystroke. Shift is applied to the character keys and F1..F10;
//! Ctrl and Alt combinations are not modelled (the menu ignores them).

use td_input::Key;

/// Character codes of the keys from usage 45 (`-`) to 56 (`/`), unshifted and shifted; 0 for
/// usage 50 (non-US `#`, not on a US keyboard).
const PUNCTUATION: [(u8, u8); 12] = [
    (b'-', b'_'),
    (b'=', b'+'),
    (b'[', b'{'),
    (b']', b'}'),
    (b'\\', b'|'),
    (0, 0),
    (b';', b':'),
    (b'\'', b'"'),
    (b'`', b'~'),
    (b',', b'<'),
    (b'.', b'>'),
    (b'/', b'?'),
];

/// Shifted digits 1..9, 0.
const SHIFTED_DIGITS: &[u8; 10] = b"!@#$%^&*()";

/// Scan codes of the keypad keys 1..9, 0 and `.` with NumLock off (End, Down, PgDn, Left, none,
/// Right, Home, Up, PgUp, Ins, Del).
const KEYPAD: [u8; 11] = [
    0x4F, 0x50, 0x51, 0x4B, 0, 0x4D, 0x47, 0x48, 0x49, 0x52, 0x53,
];

/// The last byte `ReadKey` returns for keystroke `k` (`shift`: a Shift key held): the character
/// code, or the scan code of an extended key; `None` for keys that put nothing into the buffer
/// (modifiers, locks, Print Screen, Pause, F11, F12, keypad 5).
#[must_use]
pub fn last_byte(k: Key, shift: bool) -> Option<u8> {
    let u = k.0;
    let pick = |(lo, hi): (u8, u8)| if shift { hi } else { lo };
    let b = match u {
        // Letters a..z.
        4..=29 => pick((b'a', b'A')) + (u - 4) as u8,
        // Digits 1..9, 0.
        30..=39 => {
            let i = usize::from(u - 30);
            pick((b"1234567890"[i], SHIFTED_DIGITS[i]))
        }
        40 => 0x0D,
        41 => 0x1B,
        42 => 0x08,
        // Tab; Shift+Tab is extended (0, 0Fh).
        43 => pick((0x09, 0x0F)),
        44 => b' ',
        45..=56 => pick(PUNCTUATION[usize::from(u - 45)]),
        // F1..F10: scan 3Bh..44h, shifted 54h..5Dh.
        58..=67 => pick((0x3B, 0x54)) + (u - 58) as u8,
        // Insert, Home, PgUp, Delete, End, PgDn, Right, Left, Down, Up.
        73 => 0x52,
        74 => 0x47,
        75 => 0x49,
        76 => 0x53,
        77 => 0x4F,
        78 => 0x51,
        79 => 0x4D,
        80 => 0x4B,
        81 => 0x50,
        82 => 0x48,
        // Keypad /, *, -, +, Enter.
        84 => b'/',
        85 => b'*',
        86 => b'-',
        87 => b'+',
        88 => 0x0D,
        // Keypad 1..9, 0, `.`.
        89..=99 => KEYPAD[usize::from(u - 89)],
        _ => 0,
    };
    (b != 0).then_some(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn characters_and_extended_keys() {
        assert_eq!(last_byte(Key::letter('a').unwrap(), false), Some(b'a'));
        assert_eq!(last_byte(Key::letter('y').unwrap(), true), Some(b'Y'));
        assert_eq!(last_byte(Key::digit(2).unwrap(), false), Some(b'2'));
        assert_eq!(last_byte(Key::digit(0).unwrap(), true), Some(b')'));
        assert_eq!(last_byte(Key::ENTER, false), Some(0x0D));
        assert_eq!(last_byte(Key::ESCAPE, false), Some(0x1B));
        assert_eq!(last_byte(Key::SPACE, true), Some(0x20));
        assert_eq!(last_byte(Key(56), true), Some(b'?'));
        // Extended keys: the scan code comes last.
        assert_eq!(last_byte(Key::UP, false), Some(0x48));
        assert_eq!(last_byte(Key::DOWN, true), Some(0x50));
        assert_eq!(last_byte(Key::F1, false), Some(0x3B));
        assert_eq!(last_byte(Key::F10, true), Some(0x5D));
        assert_eq!(last_byte(Key::TAB, true), Some(0x0F));
        // Keypad 8 and 2 with NumLock off (U-491); keypad 5 gives nothing.
        assert_eq!(last_byte(Key(96), false), Some(0x48));
        assert_eq!(last_byte(Key(90), false), Some(0x50));
        assert_eq!(last_byte(Key(93), false), None);
        assert_eq!(last_byte(Key::KP_ENTER, false), Some(0x0D));
        // Nothing reaches the buffer.
        for k in [
            Key::LSHIFT,
            Key::RALT,
            Key::F11,
            Key::F12,
            Key::PAUSE,
            Key(57),
        ] {
            assert_eq!(last_byte(k, false), None, "{k:?}");
        }
    }
}
