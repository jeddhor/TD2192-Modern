//! Saved games: `td.sav` (slots) and `td.stt` (slot names); E2 and E3 use the same layout in
//! `td2.*` and `td3.*`.
//!
//! Layout per `docs/re/FORMATS.md` §13.2–§13.3 and `docs/spec/flow/save-screens.md` (rev 1)
//! §3.1, §4.4 (U-011 resolved): `td.sav` slot s (1..10) is the 3 bytes at (s − 1) × 3: the
//! mission number (0 or > 20 = empty) and the kills to date (u16, little-endian). Bytes 30..69
//! are never read or written by the game and are kept as they are. `td.stt` is a text file of
//! ten lines ended by CR LF; the game reads each with `ReadLn` into a `String[255]` and writes
//! them back with `WriteLn`, so lines can have any length up to 255.
//! Confidence: CONFIRMED (C-0042, C-0043).

/// Slots per file.
pub const SLOTS: usize = 10;
/// Bytes of `td.sav` the game uses.
pub const SLOT_BYTES: usize = 3 * SLOTS;
/// Size of the shipped `td.sav`.
pub const SAV_LEN: usize = 70;
/// Longest `td.stt` line (`String[255]`).
pub const MAX_LINE: usize = 255;
/// The filler of bytes 30..69 in the shipped files (FORMATS §13.2), used for a new file.
const FILLER: u8 = 0xFF;

/// One slot of `td.sav`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Slot {
    /// The next mission to fly (1..=20); 0 or > 20 = empty.
    pub mission: u8,
    /// Kills to date, as stored (the screens show it unsigned; the game loads it as an i16).
    pub kills: u16,
}

impl Slot {
    /// True when the slot holds a game (mission 1..=20).
    pub fn used(self) -> bool {
        (1..=20).contains(&self.mission)
    }
}

/// `td.sav`: ten slots and the bytes after them, kept as they are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveSlots {
    pub slots: [Slot; SLOTS],
    rest: Vec<u8>,
}

impl Default for SaveSlots {
    /// An empty set: every slot (0, 0), then the shipped filler.
    fn default() -> Self {
        Self {
            slots: [Slot::default(); SLOTS],
            rest: vec![FILLER; SAV_LEN - SLOT_BYTES],
        }
    }
}

impl SaveSlots {
    /// Reads `td.sav`. The original stops with an I/O error when the file has fewer than 30
    /// bytes (OB-055); here missing slots are empty and a new tail is the shipped filler.
    pub fn parse(data: &[u8]) -> Self {
        let mut s = Self::default();
        for (slot, b) in s.slots.iter_mut().zip(data.chunks_exact(3)) {
            *slot = Slot {
                mission: b[0],
                kills: u16::from_le_bytes([b[1], b[2]]),
            };
        }
        if data.len() > SLOT_BYTES {
            s.rest = data[SLOT_BYTES..].to_vec();
        }
        s
    }

    /// The bytes of the file: the ten slots, then the tail as read.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(SLOT_BYTES + self.rest.len());
        for s in &self.slots {
            out.push(s.mission);
            out.extend_from_slice(&s.kills.to_le_bytes());
        }
        out.extend_from_slice(&self.rest);
        out
    }
}

/// `td.stt`: ten lines, as `ReadLn` gives them (Latin-1 characters).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SaveNames {
    pub names: [String; SLOTS],
}

impl SaveNames {
    /// Reads ten lines (CR LF or LF); missing lines are empty, longer ones are cut at 255
    /// characters as `ReadLn` into a `String[255]` does.
    pub fn parse(data: &[u8]) -> Self {
        let mut s = Self::default();
        let mut lines = data.split(|&b| b == b'\n');
        for name in &mut s.names {
            let Some(line) = lines.next() else { break };
            let line = line.strip_suffix(b"\r").unwrap_or(line);
            *name = line.iter().take(MAX_LINE).map(|&b| char::from(b)).collect();
        }
        s
    }

    /// The file as ten `WriteLn` lines (CR LF). Characters outside Latin-1 become `?`.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for name in &self.names {
            out.extend(
                name.chars()
                    .take(MAX_LINE)
                    .map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?')),
            );
            out.extend_from_slice(b"\r\n");
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_round_trip_and_keep_the_tail() {
        let mut data = vec![0u8; SAV_LEN];
        data[0] = 2;
        data[1] = 13;
        data[27] = 21;
        data[28] = 0xFE;
        data[29] = 0xFF;
        data[40] = 7;
        let s = SaveSlots::parse(&data);
        assert_eq!(
            s.slots[0],
            Slot {
                mission: 2,
                kills: 13
            }
        );
        assert!(s.slots[0].used());
        assert_eq!(s.slots[9].kills, 0xFFFE);
        assert!(!s.slots[9].used(), "mission 21 is empty");
        assert!(!s.slots[1].used(), "mission 0 is empty");
        assert_eq!(s.to_bytes(), data);
    }

    #[test]
    fn short_or_missing_files_are_empty_slots() {
        let s = SaveSlots::parse(&[7, 1, 0, 9]);
        assert_eq!(
            s.slots[0],
            Slot {
                mission: 7,
                kills: 1
            }
        );
        assert_eq!(s.slots[1], Slot::default());
        let b = SaveSlots::default().to_bytes();
        assert_eq!(b.len(), SAV_LEN);
        assert!(b[..30].iter().all(|&x| x == 0));
        assert!(b[30..].iter().all(|&x| x == 0xFF));
    }

    #[test]
    fn names_round_trip() {
        let mut n = SaveNames::default();
        n.names[3] = " re.-7 a".to_owned();
        n.names[4] = " --- empty game ---".to_owned();
        let b = n.to_bytes();
        assert_eq!(SaveNames::parse(&b), n);
        assert_eq!(b.len(), 10 * 2 + 8 + 19);
        let lf = SaveNames::parse(b"abc\r\nde\n");
        assert_eq!(lf.names[0], "abc");
        assert_eq!(lf.names[1], "de");
        assert_eq!(lf.names[2], "");
        let long = SaveNames::parse(&[b'x'; 300]);
        assert_eq!(long.names[0].len(), MAX_LINE);
    }

    #[test]
    fn arbitrary_bytes_never_panic() {
        for len in 0..300 {
            let data: Vec<u8> = (0..len).map(|i| (i * 37 % 256) as u8).collect();
            let _ = SaveSlots::parse(&data).to_bytes();
            let _ = SaveNames::parse(&data).to_bytes();
        }
    }
}
