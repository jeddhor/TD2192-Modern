//! Mission files (`m<N>.td`, `m<N>.td2`, `m<N>.td3`).
//!
//! Spec: `docs/re/FORMATS.md` §7 and `docs/re/structures.md` §2 (structure LIKELY; header bytes
//! A, B, D are U-038; class codes U-008). Fidelity: T0 for parsing.
//!
//! | Offset | Size | Content |
//! |---:|---:|---|
//! | 0 | 20 | per-slot byte A (zero in 59 of 60 files) |
//! | 20 | 20 | per-slot byte B (zero) |
//! | 40 | 20 | class code per ship slot (0..=8; `0xFF` = unused) |
//! | 60 | 20 | per-slot byte D (zero) |
//! | 80 | 10,000 | X paths |
//! | 10080 | 10,000 | Y paths |
//!
//! Each path block is the image of an in-memory `array[1..20, 0..500] of byte` (stride 501)
//! from element (1, 1) on: slot `s` (1-based), step `k` is at `base + (s - 1) * 501 + k - 1`.
//! So step 0 of slot `s >= 2` is the byte before the slot's run (editor data the game never
//! uses), step 0 of slot 1 is not in the file, and slot 20 is truncated after step 481. This
//! module stores the arrays in that memory layout (bytes not in the file are zero, as on a fresh
//! heap) and indexes slots from 0 (index `i` is the original's slot `i + 1`).

use crate::{FormatError, Result};

/// Ship slots per mission.
pub const SLOTS: usize = 20;
/// Path steps the game uses per slot (steps 1..=500).
pub const PATH_LEN: usize = 500;
/// In-memory stride of one slot's path (steps 0..=500).
pub const PATH_STRIDE: usize = PATH_LEN + 1;
/// Bytes of each path array stored in the file.
pub const PATH_BLOCK_LEN: usize = SLOTS * PATH_LEN;
/// File size.
pub const MISSION_FILE_LEN: usize = HEADER_LEN + 2 * PATH_BLOCK_LEN;
/// Class code marking an unused slot.
pub const EMPTY_SLOT: u8 = 0xFF;
/// The slot (0-based) whose `0xFF` path steps the loader replaces by 0 (structures §2).
pub const FIXUP_SLOT: usize = 1;

const HEADER_LEN: usize = 4 * SLOTS;
const X_OFFSET: usize = HEADER_LEN;
const Y_OFFSET: usize = X_OFFSET + PATH_BLOCK_LEN;

/// One path array in the original's memory layout: `SLOTS` runs of `PATH_STRIDE` bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathArray(Box<[u8]>);

impl PathArray {
    fn from_file(block: &[u8]) -> Self {
        let mut mem = vec![0u8; SLOTS * PATH_STRIDE];
        // The file image starts at element (1, 1), memory index 1.
        mem[1..=PATH_BLOCK_LEN].copy_from_slice(block);
        Self(mem.into_boxed_slice())
    }

    fn file_image(&self) -> &[u8] {
        &self.0[1..=PATH_BLOCK_LEN]
    }

    /// Step `k` (0..=500) of slot `slot` (0-based), as stored in memory after the file is read.
    pub fn get(&self, slot: usize, k: usize) -> Option<u8> {
        (slot < SLOTS && k < PATH_STRIDE).then(|| self.0[slot * PATH_STRIDE + k])
    }

    /// Steps 1..=500 of `slot`, as the game copies them into a ship record.
    pub fn steps(&self, slot: usize) -> Option<&[u8]> {
        (slot < SLOTS).then(|| &self.0[slot * PATH_STRIDE + 1..(slot + 1) * PATH_STRIDE])
    }
}

/// A parsed mission file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mission {
    /// Per-slot byte A (file bytes 0..20; U-038).
    pub hdr_a: [u8; SLOTS],
    /// Per-slot byte B (file bytes 20..40; U-038).
    pub hdr_b: [u8; SLOTS],
    /// Class code per slot (`0xFF` = unused).
    pub slot_class: [u8; SLOTS],
    /// Per-slot byte D (file bytes 60..80; U-038).
    pub hdr_d: [u8; SLOTS],
    /// X paths (map column).
    pub path_x: PathArray,
    /// Y paths (map row).
    pub path_y: PathArray,
}

impl Mission {
    /// Parses a 20,080-byte mission file.
    pub fn parse(data: &[u8]) -> Result<Self> {
        FormatError::check_size("mission", data, MISSION_FILE_LEN)?;
        let header = |n: usize| -> [u8; SLOTS] {
            let mut h = [0u8; SLOTS];
            h.copy_from_slice(&data[n * SLOTS..(n + 1) * SLOTS]);
            h
        };
        Ok(Self {
            hdr_a: header(0),
            hdr_b: header(1),
            slot_class: header(2),
            hdr_d: header(3),
            path_x: PathArray::from_file(&data[X_OFFSET..Y_OFFSET]),
            path_y: PathArray::from_file(&data[Y_OFFSET..]),
        })
    }

    /// Indices of slots in use (class code other than `0xFF`).
    pub fn used_slots(&self) -> impl Iterator<Item = usize> + '_ {
        (0..SLOTS).filter(|&s| self.slot_class[s] != EMPTY_SLOT)
    }

    /// The (x, y) tile path of `slot` as the game uses it: steps 1..=500, after the loader's
    /// fixup (every `0xFF` in slot 2 of both arrays becomes 0; structures.md §2).
    pub fn path(&self, slot: usize) -> Option<impl Iterator<Item = (u8, u8)> + '_> {
        let fix = move |v: u8| {
            if slot == FIXUP_SLOT && v == 0xFF {
                0
            } else {
                v
            }
        };
        let xs = self.path_x.steps(slot)?;
        let ys = self.path_y.steps(slot)?;
        Some(xs.iter().zip(ys).map(move |(&x, &y)| (fix(x), fix(y))))
    }

    /// The file bytes, rebuilt (for round-trip tests).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(MISSION_FILE_LEN);
        for h in [&self.hdr_a, &self.hdr_b, &self.slot_class, &self.hdr_d] {
            v.extend_from_slice(h);
        }
        v.extend_from_slice(self.path_x.file_image());
        v.extend_from_slice(self.path_y.file_image());
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// File offset of (0-based slot, step k) in the block at `base`.
    fn at(base: usize, slot: usize, k: usize) -> usize {
        base + slot * PATH_STRIDE + k - 1
    }

    #[test]
    fn parses_with_stride_501_and_round_trips() {
        let mut data = vec![0u8; MISSION_FILE_LEN];
        data[2 * SLOTS..3 * SLOTS].fill(EMPTY_SLOT);
        data[2 * SLOTS + 3] = 7;
        data[at(X_OFFSET, 3, 1)] = 100;
        data[at(Y_OFFSET, 3, 1)] = 50;
        data[at(X_OFFSET, 3, 0)] = 0x80; // step 0: present in memory, never used
        data[at(Y_OFFSET, 3, 500)] = 9;
        let m = Mission::parse(&data).unwrap();
        assert_eq!(m.used_slots().collect::<Vec<_>>(), [3]);
        let path: Vec<_> = m.path(3).unwrap().collect();
        assert_eq!(path.len(), PATH_LEN);
        assert_eq!(path[0], (100, 50));
        assert_eq!(path[499], (0, 9));
        assert_eq!(m.path_x.get(3, 0), Some(0x80));
        assert!(m.path(SLOTS).is_none());
        assert_eq!(m.to_bytes(), data);
        assert!(Mission::parse(&data[1..]).is_err());
    }

    #[test]
    fn edges_of_the_file_image() {
        let mut data: Vec<u8> = (0..MISSION_FILE_LEN).map(|i| (i % 251) as u8).collect();
        data[..HEADER_LEN].fill(0);
        let m = Mission::parse(&data).unwrap();
        // Slot 1 step 0 is not in the file; slot 1 step 1 is the block's first byte.
        assert_eq!(m.path_x.get(0, 0), Some(0));
        assert_eq!(m.path_x.get(0, 1), Some(data[X_OFFSET]));
        // Slot 20 ends at step 481; the rest stays zero.
        assert_eq!(m.path_y.get(19, 481), Some(data[MISSION_FILE_LEN - 1]));
        assert_eq!(m.path_y.get(19, 482), Some(0));
        assert_eq!(m.path_y.get(19, 500), Some(0));
        assert_eq!(m.to_bytes(), data);
    }

    #[test]
    fn loader_clears_ff_in_slot_2_only() {
        let mut data = vec![0u8; MISSION_FILE_LEN];
        for slot in [0, FIXUP_SLOT] {
            data[at(X_OFFSET, slot, 5)] = 0xFF;
            data[at(Y_OFFSET, slot, 5)] = 0xFF;
        }
        let m = Mission::parse(&data).unwrap();
        assert_eq!(m.path(0).unwrap().nth(4), Some((0xFF, 0xFF)));
        assert_eq!(m.path(FIXUP_SLOT).unwrap().nth(4), Some((0, 0)));
        assert_eq!(
            m.to_bytes(),
            data,
            "the fixup does not change the parsed file"
        );
    }
}
