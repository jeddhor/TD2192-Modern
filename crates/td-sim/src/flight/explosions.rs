//! Explosion animations (30 slots).
//!
//! Spec: `docs/re/structures.md` §5, tick-order step 13, `docs/spec/flow/flight-screens.md` §4.7.
//! An explosion starts at frame 1 and shows frames 1..=6 (sprite 13h + frame of the sprite
//! bank), one per tick, then goes idle (frame 8; frame 7 is never shown).

use crate::hash::{HashState, StateHasher};

/// Number of explosion slots.
pub const EXPLOSION_SLOTS: usize = 30;
/// Frame value of an idle slot.
pub const IDLE: u8 = 8;
/// Sprite-bank index of frame 0 (frames 1..=6 are `td.exp` 0..=5).
pub const FIRST_SPRITE: u8 = 0x13;
/// Slots the projectile routine uses (1..=20 in the original).
pub const HIT_SLOTS: usize = 20;

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
    /// The lowest idle slot among 1..=20 (index 0..=19), where the projectile routine starts
    /// its explosions (`flight-screens.md` §4.7; slots 21..=30 belong to other starters).
    pub fn idle_hit_slot(&self) -> Option<usize> {
        (0..HIT_SLOTS).find(|&i| self.0[i].frame >= IDLE)
    }

    /// Starts an explosion in slot `i` at a tile plus pixel offset (stored as bytes), at frame 1.
    pub fn start_in(&mut self, i: usize, tile: (u8, u8), sub: (i16, i16)) {
        self.0[i] = Explosion {
            frame: 1,
            tile_x: tile.0,
            tile_y: tile.1,
            sub_x: sub.0 as u8,
            sub_y: sub.1 as u8,
        };
    }

    /// Starts an explosion in the lowest idle hit slot, if any. Returns false when all twenty
    /// are busy (the explosion is then not started).
    pub fn start(&mut self, tile: (u8, u8), sub: (i16, i16)) -> bool {
        match self.idle_hit_slot() {
            Some(i) => {
                self.start_in(i, tile, sub);
                true
            }
            None => false,
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
