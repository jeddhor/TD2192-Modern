//! The 16 ship headings, encoded as the original stores them: numeric-keypad codes.
//!
//! Spec: `docs/re/structures.md` §7 (CONFIRMED). The principal directions are keypad digits
//! (8 up, 9 up-right, 6 right, 3 down-right, 2 down, 1 down-left, 4 left, 7 up-left); each
//! intermediate direction is its two neighbouring digits written as a decimal number. A turn
//! step moves one place round the ring.

use crate::hash::{HashState, StateHasher};

/// A heading as its keypad code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Heading(u8);

/// The ring as stored, clockwise, starting at west-south-west.
const RING: [u8; 16] = [14, 4, 74, 7, 78, 8, 98, 9, 96, 6, 36, 3, 32, 2, 12, 1];
/// Ring position of north.
const NORTH_POS: usize = 5;

impl Heading {
    pub const N: Heading = Heading(8);
    pub const NE: Heading = Heading(9);
    pub const E: Heading = Heading(6);
    pub const SE: Heading = Heading(3);
    pub const S: Heading = Heading(2);
    pub const SW: Heading = Heading(1);
    pub const W: Heading = Heading(4);
    pub const NW: Heading = Heading(7);

    /// The heading with keypad code `code`, if it is one of the 16.
    pub fn from_code(code: u8) -> Option<Heading> {
        RING.contains(&code).then_some(Heading(code))
    }

    /// The keypad code.
    pub fn code(self) -> u8 {
        self.0
    }

    /// True for the eight single-digit (principal) headings.
    pub fn is_principal(self) -> bool {
        self.0 < 10
    }

    fn ring_pos(self) -> usize {
        RING.iter()
            .position(|&c| c == self.0)
            .expect("Heading holds a ring code")
    }

    /// One step counter-clockwise.
    pub fn ccw(self) -> Heading {
        Heading(RING[(self.ring_pos() + 15) % 16])
    }

    /// One step clockwise.
    pub fn cw(self) -> Heading {
        Heading(RING[(self.ring_pos() + 1) % 16])
    }

    /// Index into `td_render::headings::HeadingSprites` (0 = nose up, counter-clockwise).
    pub fn sprite_index(self) -> usize {
        (NORTH_POS + 16 - self.ring_pos()) % 16
    }
}

impl HashState for Heading {
    fn hash_state(&self, h: &mut StateHasher) {
        self.0.hash_state(h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_order_and_sprite_index() {
        let mut h = Heading::N;
        let mut seen = vec![];
        for _ in 0..16 {
            seen.push(h.code());
            h = h.cw();
        }
        assert_eq!(
            seen,
            [8, 98, 9, 96, 6, 36, 3, 32, 2, 12, 1, 14, 4, 74, 7, 78]
        );
        assert_eq!(h, Heading::N);
        assert_eq!(Heading::N.ccw().code(), 78);
        assert_eq!(Heading::N.sprite_index(), 0);
        assert_eq!(Heading::W.sprite_index(), 4);
        assert_eq!(Heading::S.sprite_index(), 8);
        assert_eq!(Heading::E.sprite_index(), 12);
        assert_eq!(Heading::N.cw().sprite_index(), 15);
        assert!(Heading::from_code(5).is_none());
        assert!(Heading::SW.is_principal() && !Heading::N.cw().is_principal());
    }
}
