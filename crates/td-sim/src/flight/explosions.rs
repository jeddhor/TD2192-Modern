//! Explosion animations (30 slots).
//!
//! Spec: `docs/re/structures.md` §5, tick-order step 13. An explosion shows frames 0..=6
//! (sprite 13h + frame of the sprite bank), one per tick, then goes idle (frame 8; frame 7 is
//! skipped).

use crate::hash::{HashState, StateHasher};

/// Number of explosion slots.
pub const EXPLOSION_SLOTS: usize = 30;
/// Frame value of an idle slot.
pub const IDLE: u8 = 8;
/// Sprite-bank index of frame 0.
pub const FIRST_SPRITE: u8 = 0x13;

/// One explosion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Explosion {
    pub frame: u8,
    pub tile_x: u8,
    pub tile_y: u8,
    pub sub_x: u8,
    pub sub_y: u8,
}

/// All explosion slots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Explosions(pub [Explosion; EXPLOSION_SLOTS]);

impl Default for Explosions {
    fn default() -> Self {
        Self(
            [Explosion {
                frame: IDLE,
                tile_x: 0,
                tile_y: 0,
                sub_x: 0,
                sub_y: 0,
            }; EXPLOSION_SLOTS],
        )
    }
}

impl Explosions {
    /// Starts an explosion at a tile plus pixel offset (the offset is stored as a byte).
    ///
    /// ASSUMPTION: the first idle slot is used, and nothing happens when all are busy (the
    /// slot choice is not specified yet).
    pub fn start(&mut self, tile: (u8, u8), sub: (i16, i16)) {
        if let Some(e) = self.0.iter_mut().find(|e| e.frame >= IDLE) {
            *e = Explosion {
                frame: 0,
                tile_x: tile.0,
                tile_y: tile.1,
                sub_x: sub.0 as u8,
                sub_y: sub.1 as u8,
            };
        }
    }

    /// Advances every active explosion by one frame (step 13, after drawing).
    pub fn step(&mut self) {
        for e in self.0.iter_mut().filter(|e| e.frame < IDLE) {
            e.frame = if e.frame == 6 { IDLE } else { e.frame + 1 };
        }
    }
}

impl HashState for Explosions {
    fn hash_state(&self, h: &mut StateHasher) {
        for e in &self.0 {
            h.bytes(&[e.frame, e.tile_x, e.tile_y, e.sub_x, e.sub_y]);
        }
    }
}
