//! Ship-class and tile-attribute tables.
//!
//! Spec: `docs/re/structures.md` §6 (class tables, CONFIRMED values) and §8 (tile attributes,
//! CONFIRMED for Episode 1), `docs/spec/sim/flight-model.md` §3.

/// Number of class codes (0..=9; 9 is unused, `0xFF` = no ship).
pub const CLASSES: usize = 10;

/// Maximum raw (biased) speed component per class.
pub const CLASS_MAX_SPEED: [u8; CLASSES] = [24, 28, 32, 28, 28, 28, 24, 28, 32, 0];
/// Armour per class (the player uses it as is; mission ships get twice this).
pub const CLASS_ARMOUR: [u8; CLASSES] = [31, 16, 15, 20, 25, 25, 31, 25, 20, 0];
/// Shields per class, also the regeneration cap.
pub const CLASS_SHIELDS: [u8; CLASSES] = [0, 0, 18, 20, 25, 20, 31, 25, 25, 0];
/// Thrust step per tick per class.
pub const CLASS_ACCEL: [u8; CLASSES] = [2, 4, 4, 4, 4, 4, 2, 4, 4, 0];

/// The player's class in each flight 1..=21 (`structures.md` §6, per-mission setup).
pub fn player_class(flight: u8) -> u8 {
    match flight {
        6..=8 | 11 | 17 | 18 | 20 => 2,
        12 | 15 | 21 => 3,
        13 | 14 => 8,
        19 => 0,
        _ => 4,
    }
}

/// Tile attribute: the ship can fly over the tile.
pub const TILE_OPEN: u8 = 2;
/// Tile attribute: solid.
pub const TILE_SOLID: u8 = 1;

/// The Episode 1 tile attribute table (filled at flight start). Tiles above 70 are not
/// initialised by the original (they never occur in `td.map`); they are solid here.
pub fn episode1_tile_attributes() -> [u8; 256] {
    let mut attr = [TILE_SOLID; 256];
    for t in [15..=16, 49..=51, 56..=61, 69..=69] {
        for i in t {
            attr[i] = TILE_OPEN;
        }
    }
    attr
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_attributes_match_the_table() {
        let a = episode1_tile_attributes();
        let open: Vec<usize> = (0..=70).filter(|&t| a[t] == TILE_OPEN).collect();
        assert_eq!(open, [15, 16, 49, 50, 51, 56, 57, 58, 59, 60, 61, 69]);
    }

    #[test]
    fn flight_classes() {
        let classes: Vec<u8> = (1..=21).map(player_class).collect();
        assert_eq!(
            classes,
            [
                4, 4, 4, 4, 4, 2, 2, 2, 4, 4, 2, 3, 8, 8, 3, 4, 2, 2, 0, 2, 3
            ]
        );
    }
}
