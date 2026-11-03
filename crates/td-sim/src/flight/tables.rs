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

/// The class tables as the running program holds them. They start as the constants above, and
/// per-flight setup patches them for the rest of the program run (mission-setup §3.3, OB-018),
/// so they are session state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassTables {
    pub max_speed: [u8; CLASSES],
    pub armour: [u8; CLASSES],
    pub shields: [u8; CLASSES],
    pub accel: [u8; CLASSES],
}

impl Default for ClassTables {
    fn default() -> Self {
        Self {
            max_speed: CLASS_MAX_SPEED,
            armour: CLASS_ARMOUR,
            shields: CLASS_SHIELDS,
            accel: CLASS_ACCEL,
        }
    }
}

/// Entry `class` of `table`. Classes past the table (FFh = empty slot) read other globals in
/// the original (mission-ships §5, "Class FFh at setup"); they give 0 here (U-143).
fn entry(table: &[u8; CLASSES], class: u8) -> u8 {
    table.get(usize::from(class)).copied().unwrap_or(0)
}

impl ClassTables {
    pub fn max_speed(&self, class: u8) -> u8 {
        entry(&self.max_speed, class)
    }
    pub fn armour(&self, class: u8) -> u8 {
        entry(&self.armour, class)
    }
    pub fn shields(&self, class: u8) -> u8 {
        entry(&self.shields, class)
    }
    pub fn accel(&self, class: u8) -> u8 {
        entry(&self.accel, class)
    }
}

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

/// Muzzles (1..=17) with a shot-table entry, and that entry (shot kind + 1), per class
/// (`weapons.md` §3.1; the table at DS:02E5).
const SHOT_TABLE: [(u8, &[usize], u8); 7] = [
    (2, &[1, 2, 4, 5], 1),
    (3, &[8, 9], 1),
    (4, &[6, 7], 2),
    (5, &[3], 2),
    (6, &[10, 11], 3),
    (7, &[12, 13], 2),
    (8, &[14, 15, 16, 17], 1),
];

/// The shot-table entry (kind + 1, or 0 = no shot) for `class` and muzzle `m` (1..=17).
pub fn shot_entry(class: u8, m: usize) -> u8 {
    SHOT_TABLE
        .iter()
        .find(|(c, muzzles, _)| *c == class && muzzles.contains(&m))
        .map_or(0, |e| e.2)
}

/// The player's weapons for a class (`weapons.md` §3.2): primary weapon (1..=3), missile
/// weapon, and missiles at flight start. `None` for classes 0 and 1 (unarmed; the weapon is
/// left as it was).
pub fn player_weapons(class: u8) -> Option<(u8, u8, u8)> {
    match class {
        2 => Some((1, 4, 6)),
        3 => Some((1, 5, 20)),
        4 => Some((2, 4, 4)),
        5 => Some((2, 6, 1)),
        6 => Some((3, 5, 6)),
        7 => Some((2, 2, 0)),
        8 => Some((1, 1, 0)),
        _ => None,
    }
}

/// Damage of a shot of `kind` to a mission ship (kind 6 destroys outright, handled apart).
pub fn ship_damage(kind: u8) -> i16 {
    let k = i16::from(kind);
    (k + 1) * (k + 1) + 4 + 2 * k
}

/// Damage of a shot of `kind` to the player.
pub fn player_damage(kind: u8, difficulty: u8) -> i16 {
    2 * (i16::from(kind) + 1) * i16::from(difficulty)
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
    fn weapons_tables() {
        let volley = |c| (1..=17).filter(|&m| shot_entry(c, m) != 0).count();
        assert_eq!(
            (0..=8).map(volley).collect::<Vec<_>>(),
            [0, 0, 4, 2, 2, 1, 2, 2, 4]
        );
        assert_eq!(
            (0..=5).map(ship_damage).collect::<Vec<_>>(),
            [5, 10, 17, 26, 37, 50]
        );
        assert_eq!(player_damage(1, 1), 4);
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
