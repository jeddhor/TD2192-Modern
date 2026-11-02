//! Mission files (`m<N>.td`, `m<N>.td2`, `m<N>.td3`).
//!
//! Spec: `docs/re/FORMATS.md` §7 (structure LIKELY; field meanings U-008, U-014, U-015).
//! Fidelity: T0 for parsing.
//!
//! | Offset | Size | Content |
//! |---:|---:|---|
//! | 0 | 40 | reserved (zero in 59 of 60 files) |
//! | 40 | 20 | class code per ship slot (0..=8; `0xFF` = unused) |
//! | 60 | 20 | reserved (zero) |
//! | 80 | 20 x 500 | X path per slot, in map tiles |
//! | 10080 | 20 x 500 | Y path per slot, in map tiles |

use crate::{FormatError, Result};

/// Ship slots per mission.
pub const SLOTS: usize = 20;
/// Steps in each path.
pub const PATH_LEN: usize = 500;
/// File size.
pub const MISSION_FILE_LEN: usize = 80 + 2 * SLOTS * PATH_LEN;
/// Class code marking an unused slot.
pub const EMPTY_SLOT: u8 = 0xFF;

const CLASS_OFFSET: usize = 40;
const X_OFFSET: usize = 80;
const Y_OFFSET: usize = X_OFFSET + SLOTS * PATH_LEN;

/// A parsed mission file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mission {
    /// Bytes 0..40, kept verbatim.
    pub reserved_head: [u8; 40],
    /// Class code per slot (`0xFF` = unused).
    pub slot_class: [u8; SLOTS],
    /// Bytes 60..80, kept verbatim.
    pub reserved_tail: [u8; 20],
    /// X path per slot (500 tile coordinates each).
    pub path_x: Vec<[u8; PATH_LEN]>,
    /// Y path per slot (500 tile coordinates each).
    pub path_y: Vec<[u8; PATH_LEN]>,
}

impl Mission {
    /// Parses a 20,080-byte mission file.
    pub fn parse(data: &[u8]) -> Result<Self> {
        FormatError::check_size("mission", data, MISSION_FILE_LEN)?;
        let paths = |base: usize| -> Vec<[u8; PATH_LEN]> {
            (0..SLOTS)
                .map(|s| {
                    let start = base + s * PATH_LEN;
                    let mut p = [0u8; PATH_LEN];
                    p.copy_from_slice(&data[start..start + PATH_LEN]);
                    p
                })
                .collect()
        };
        let mut reserved_head = [0u8; 40];
        reserved_head.copy_from_slice(&data[..CLASS_OFFSET]);
        let mut slot_class = [0u8; SLOTS];
        slot_class.copy_from_slice(&data[CLASS_OFFSET..CLASS_OFFSET + SLOTS]);
        let mut reserved_tail = [0u8; 20];
        reserved_tail.copy_from_slice(&data[CLASS_OFFSET + SLOTS..X_OFFSET]);
        Ok(Self {
            reserved_head,
            slot_class,
            reserved_tail,
            path_x: paths(X_OFFSET),
            path_y: paths(Y_OFFSET),
        })
    }

    /// Indices of slots in use (class code other than `0xFF`).
    pub fn used_slots(&self) -> impl Iterator<Item = usize> + '_ {
        (0..SLOTS).filter(|&s| self.slot_class[s] != EMPTY_SLOT)
    }

    /// The (x, y) tile path of `slot`, all 500 steps (unused tail steps are zero).
    pub fn path(&self, slot: usize) -> Option<impl Iterator<Item = (u8, u8)> + '_> {
        let xs = self.path_x.get(slot)?;
        let ys = self.path_y.get(slot)?;
        Some(xs.iter().copied().zip(ys.iter().copied()))
    }

    /// The file bytes, rebuilt (for round-trip tests).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(MISSION_FILE_LEN);
        v.extend_from_slice(&self.reserved_head);
        v.extend_from_slice(&self.slot_class);
        v.extend_from_slice(&self.reserved_tail);
        for p in self.path_x.iter().chain(&self.path_y) {
            v.extend_from_slice(p);
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_round_trips() {
        let mut data = vec![0u8; MISSION_FILE_LEN];
        data[CLASS_OFFSET..CLASS_OFFSET + SLOTS].fill(EMPTY_SLOT);
        data[CLASS_OFFSET + 3] = 7;
        data[X_OFFSET + 3 * PATH_LEN] = 100;
        data[Y_OFFSET + 3 * PATH_LEN] = 50;
        let m = Mission::parse(&data).unwrap();
        assert_eq!(m.used_slots().collect::<Vec<_>>(), [3]);
        assert_eq!(m.path(3).unwrap().next(), Some((100, 50)));
        assert!(m.path(20).is_none());
        assert_eq!(m.to_bytes(), data);
        assert!(Mission::parse(&data[1..]).is_err());
    }
}
