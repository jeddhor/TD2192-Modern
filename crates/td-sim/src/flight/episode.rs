//! Episode configuration: what one engine does differently as `file0001.exe` (E1), `td2.exe`
//! (E2) or `td3.exe` (E3).
//!
//! Spec: `docs/re/episode-deltas.md` rev 2 (§4.1 tick order, §4.3 setup, §4.4 tile attributes,
//! §5 weapons and mission ships) and `docs/spec/sim/e2-mission-setup.md` rev 1 (§3.1 map sets,
//! §3.3 player state, §3.5 tile attributes, §4.1 setup order). Fidelity: T1.
//!
//! The episodes are one engine: [`super::Flight`] holds its [`Episode`] and the tick and the
//! setup branch on it where the deltas say so. E3 has its files and tile attributes here, but
//! no setup or mission logic yet (M10-02): an E3 flight runs the shared engine with the mission
//! file's ships and no rules.

pub use td_assets::Episode;

use super::player::View;
use super::tables::{TILE_OPEN, TILE_SOLID, player_class as e1_player_class};
use super::{START_VIEW, World};

/// `g_MapSet` (E2 DS:1302): 1 for `td.map`/`td.blk`, 2 for `td2.map`/`td2.blk` (E2 flights
/// 9–20). E1 has one map; E3 counts `td4.map` (flights ≥ 7) as set 2 here.
pub fn map_set(ep: Episode, flight: u8) -> u8 {
    match ep {
        Episode::E1 => 1,
        Episode::E2 => {
            if flight <= 8 {
                1
            } else {
                2
            }
        }
        Episode::E3 => {
            if flight <= 6 {
                1
            } else {
                2
            }
        }
    }
}

/// The map file of a flight (episode-deltas §4.3).
pub fn map_file(ep: Episode, flight: u8) -> &'static str {
    match (ep, map_set(ep, flight)) {
        (Episode::E1, _) | (Episode::E2, 1) => "td.map",
        (Episode::E2, _) => "td2.map",
        (Episode::E3, 1) => "td3.map",
        (Episode::E3, _) => "td4.map",
    }
}

/// The tile set file of a flight (episode-deltas §4.3).
pub fn tileset_file(ep: Episode, flight: u8) -> &'static str {
    match (ep, map_set(ep, flight)) {
        (Episode::E1, _) | (Episode::E2, 1) => "td.blk",
        (Episode::E2, _) => "td2.blk",
        (Episode::E3, _) => "td3.blk",
    }
}

/// The mission file of a flight: `m<n>.td` (E1, none for flight 21), `m<n>.td2`, `m<n>.td3`.
pub fn mission_file(ep: Episode, flight: u8) -> Option<String> {
    match ep {
        Episode::E1 if flight == 21 => None,
        Episode::E1 => Some(format!("m{flight}.td")),
        Episode::E2 => Some(format!("m{flight}.td2")),
        Episode::E3 => Some(format!("m{flight}.td3")),
    }
}

/// Sets `attr[a..=b]` to `v` for each range in code order (a later write wins).
fn fill(attr: &mut [u8; 256], writes: &[(usize, usize, u8)]) {
    for &(a, b, v) in writes {
        for t in &mut attr[a..=b] {
            *t = v;
        }
    }
}

/// The tile-attribute table of a flight (episode-deltas §4.4, e2-mission-setup §3.5). Entries
/// the original never initialises (above the highest tile of every map of the set) are solid.
pub fn tile_attributes(ep: Episode, flight: u8) -> [u8; 256] {
    const S: u8 = TILE_SOLID;
    const O: u8 = TILE_OPEN;
    let mut attr = [S; 256];
    match (ep, map_set(ep, flight)) {
        (Episode::E1, _) => fill(
            &mut attr,
            &[
                (0x0F, 0x10, O),
                (0x31, 0x33, O),
                (0x38, 0x3D, O),
                (0x45, 0x45, O),
            ],
        ),
        // Set 1: E1's table plus 47h..51h open (td2:0E96:A9D5..AAEE).
        (Episode::E2, 1) => fill(
            &mut attr,
            &[
                (0x00, 0x0E, S),
                (0x0F, 0x10, O),
                (0x11, 0x30, S),
                (0x31, 0x33, O),
                (0x34, 0x37, S),
                (0x38, 0x3D, O),
                (0x3E, 0x44, S),
                (0x45, 0x45, O),
                (0x46, 0x46, S),
                (0x47, 0x51, O),
            ],
        ),
        // Set 2, overlapping writes in code order (td2:0E96:AAF0..ACC0).
        (Episode::E2, _) => fill(
            &mut attr,
            &[
                (0x00, 0x0A, S),
                (0x0B, 0x10, O),
                (0x11, 0x1B, S),
                (0x1C, 0x29, O),
                (0x2A, 0x2D, S),
                (0x2E, 0x32, O),
                (0x30, 0x30, S),
                (0x33, 0x3B, S),
                (0x3A, 0x3A, O),
                (0x3C, 0x4B, O),
            ],
        ),
        (Episode::E3, _) => fill(
            &mut attr,
            &[
                (0x10, 0x16, O),
                (0x22, 0x22, O),
                (0x25, 0x2C, O),
                (0x41, 0x46, O),
            ],
        ),
    }
    attr
}

/// The world of a flight: its map (read from [`map_file`] by the caller), the episode's tile
/// attributes and the muzzle table (byte-identical in all episodes, episode-deltas §11).
pub fn world(ep: Episode, flight: u8, map: td_formats::map::TileMap, muzzles: Vec<u8>) -> World {
    World {
        map,
        tile_attr: tile_attributes(ep, flight),
        muzzles: super::shots::Muzzles(muzzles),
    }
}

/// Whether setup ends with `AI_ChooseWaypoint` for every slot (E1 `0E8B:CC63..CC7D`). E2 and
/// E3 dropped the loop: each ship's first waypoint call comes in tick 1 (e2-mission-ships §4.3,
/// U-205).
pub fn setup_waypoint_loop(ep: Episode) -> bool {
    ep == Episode::E1
}

/// The player's class in a flight (E1: structures §6; E2: e2-mission-setup §3.3).
pub fn player_class(ep: Episode, flight: u8) -> u8 {
    match ep {
        Episode::E1 | Episode::E3 => e1_player_class(flight),
        Episode::E2 => match flight {
            4 | 9 => 4,
            5 | 6 => 7,
            7 | 8 => 3,
            11..=13 | 20 => 5,
            _ => 2,
        },
    }
}

/// Missiles at flight start for a class: class 5 starts with 4 in E2 and E3, 1 in E1
/// (episode-deltas §4.3; weapons §3.2 otherwise).
pub fn start_missiles(ep: Episode, class: u8, table: u8) -> u8 {
    if class == 5 && ep != Episode::E1 {
        4
    } else {
        table
    }
}

/// The start view after setup (offsets 32, 16): E1 mission-setup §3.1, E2 e2-mission-setup
/// §4.1 step 13 (in code order, a later rule wins).
pub fn start_view(ep: Episode, flight: u8, late_draw: bool) -> View {
    let mut v = START_VIEW;
    let mut tile = |x, y| (v.tile_x, v.tile_y) = (x, y);
    match ep {
        Episode::E1 => {
            if late_draw {
                tile(102, 100);
            }
            if matches!(flight, 13 | 14) {
                tile(177, 153);
            }
        }
        Episode::E2 => {
            if matches!(flight, 5 | 6) {
                tile(179, 33);
            }
            if flight > 8 {
                tile(87, 28);
            }
            if flight == 7 {
                tile(120, 70);
            }
            if late_draw {
                tile(102, 100);
            }
        }
        // ASSUMPTION(M10-02): E3 start views are not ported yet; the default view is used.
        Episode::E3 => {}
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open(a: &[u8; 256], hi: usize) -> Vec<usize> {
        (0..=hi).filter(|&t| a[t] == TILE_OPEN).collect()
    }

    #[test]
    fn e1_table_is_unchanged() {
        assert_eq!(
            tile_attributes(Episode::E1, 1),
            super::super::tables::episode1_tile_attributes()
        );
    }

    /// e2-mission-setup §6 vector 7.
    #[test]
    fn e2_tile_attribute_sets() {
        let s1 = tile_attributes(Episode::E2, 8);
        assert_eq!((s1[0x45], s1[0x46]), (TILE_OPEN, TILE_SOLID));
        assert!((0x47..=0x51).all(|t| s1[t] == TILE_OPEN));
        let e1 = tile_attributes(Episode::E1, 8);
        assert_eq!(open(&s1, 0x46), open(&e1, 0x46));
        let s2 = tile_attributes(Episode::E2, 9);
        let want_open: Vec<usize> = (0x0B..=0x10)
            .chain(0x1C..=0x29)
            .chain([0x2E, 0x2F, 0x31, 0x32, 0x3A])
            .chain(0x3C..=0x4B)
            .collect();
        assert_eq!(open(&s2, 0x4B), want_open);
    }

    #[test]
    fn e2_files_and_views() {
        assert_eq!(map_file(Episode::E2, 8), "td.map");
        assert_eq!(map_file(Episode::E2, 9), "td2.map");
        assert_eq!(tileset_file(Episode::E2, 20), "td2.blk");
        assert_eq!(mission_file(Episode::E1, 21), None);
        assert_eq!(mission_file(Episode::E2, 9).as_deref(), Some("m9.td2"));
        let tile = |f, late| {
            let v = start_view(Episode::E2, f, late);
            (v.tile_x, v.tile_y, v.sub_x, v.sub_y)
        };
        assert_eq!(tile(1, false), (102, 104, 32, 16));
        assert_eq!(tile(5, false), (179, 33, 32, 16));
        assert_eq!(tile(7, false), (120, 70, 32, 16));
        assert_eq!(tile(8, true), (102, 100, 32, 16));
        assert_eq!(tile(13, false), (87, 28, 32, 16));
        let classes: Vec<u8> = (1..=20).map(|f| player_class(Episode::E2, f)).collect();
        assert_eq!(
            classes,
            [2, 2, 2, 4, 7, 7, 3, 3, 4, 2, 5, 5, 5, 2, 2, 2, 2, 2, 2, 5]
        );
    }
}
