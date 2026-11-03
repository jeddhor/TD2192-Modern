//! The 20 mission-ship slots, as far as combat needs them.
//!
//! Spec: `docs/re/structures.md` §3–§4 (slot arrays and ship record), `weapons.md` §4.3.
//! Movement, AI and per-mission overrides are M8 (`mission-ships.md`, in progress); until
//! then ships stay where the mission file starts them. Slot `i` here is the original's
//! slot `i + 1`.

use td_formats::mission::{EMPTY_SLOT, Mission};

use super::tables::{CLASS_ARMOUR, CLASS_SHIELDS, CLASSES};
use crate::hash::{HashState, StateHasher};

/// Number of mission-ship slots.
pub const SHIP_SLOTS: usize = 20;
/// Class code of an empty or destroyed slot.
pub const NO_SHIP: u8 = EMPTY_SLOT;

/// One mission-ship slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ship {
    /// Class code, `NO_SHIP` when empty or destroyed.
    pub class: u8,
    pub tile_x: u8,
    pub tile_y: u8,
    /// Pixel offsets added to the tile × 32.
    pub sub_x: i16,
    pub sub_y: i16,
    /// Heading as a keypad code.
    pub heading: u8,
    /// Ship record +3F8h.
    pub shields: i16,
    /// Ship record +3FAh.
    pub armour: i16,
}

impl Ship {
    /// An empty slot.
    pub const EMPTY: Ship = Ship {
        class: NO_SHIP,
        tile_x: 0,
        tile_y: 0,
        sub_x: 0,
        sub_y: 0,
        heading: 8,
        shields: 0,
        armour: 0,
    };

    /// True while the slot holds a ship.
    pub fn alive(&self) -> bool {
        self.class != NO_SHIP
    }

    /// World pixel (`tile × 32 + offset` per axis).
    pub fn world(&self) -> (i32, i32) {
        (
            i32::from(self.tile_x) * 32 + i32::from(self.sub_x),
            i32::from(self.tile_y) * 32 + i32::from(self.sub_y),
        )
    }
}

/// All mission-ship slots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ships(pub [Ship; SHIP_SLOTS]);

impl Default for Ships {
    fn default() -> Self {
        Self([Ship::EMPTY; SHIP_SLOTS])
    }
}

impl Ships {
    /// Slots as set up from a mission file: class, start tile = path step 1, shields from the
    /// class table, armour twice the class armour (structures §3).
    ///
    /// ASSUMPTION (until `mission-ships.md` rev 2): pixel offsets start at 16 and ships face
    /// up; per-mission overrides are not applied.
    pub fn from_mission(m: &Mission) -> Self {
        let mut ships = Self::default();
        for (i, ship) in ships.0.iter_mut().enumerate() {
            let class = m.slot_class[i];
            if usize::from(class) >= CLASSES {
                continue;
            }
            let (x, y) = m.path(i).and_then(|mut p| p.next()).unwrap_or((0, 0));
            *ship = Ship {
                class,
                tile_x: x,
                tile_y: y,
                sub_x: 16,
                sub_y: 16,
                heading: 8,
                shields: i16::from(CLASS_SHIELDS[usize::from(class)]),
                armour: 2 * i16::from(CLASS_ARMOUR[usize::from(class)]),
            };
        }
        ships
    }
}

impl HashState for Ships {
    fn hash_state(&self, h: &mut StateHasher) {
        for s in &self.0 {
            h.bytes(&[s.class, s.tile_x, s.tile_y, s.heading]);
            for v in [s.sub_x, s.sub_y, s.shields, s.armour] {
                v.hash_state(h);
            }
        }
    }
}
