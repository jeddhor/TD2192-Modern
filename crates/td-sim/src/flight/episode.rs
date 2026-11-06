//! Episode configuration: what one engine does differently as `file0001.exe` (E1), `td2.exe`
//! (E2) or `td3.exe` (E3).
//!
//! Spec: `docs/re/episode-deltas.md` rev 3 (§4.1 tick order, §4.3 setup, §4.4 tile attributes,
//! §5 weapons and mission ships), `docs/spec/sim/e2-mission-setup.md` rev 1 (§3.1 map sets,
//! §3.3 player state, §3.5 tile attributes, §4.1 setup order) and
//! `docs/spec/sim/e3-mission-setup.md` rev 1 (§3.1 maps, §3.3 player state, §3.5 tile
//! attributes, §3.7 home points, §4.1 step 9 start view). Fidelity: T1.
//!
//! The episodes are one engine: [`super::Flight`] holds its [`Episode`] and the tick and the
//! setup branch on it where the deltas say so.

pub use td_assets::Episode;

use super::player::View;
use super::tables::{TILE_OPEN, TILE_SOLID, player_class as e1_player_class};
use super::{START_VIEW, World};

/// `g_MapSet` (E2 DS:1302): 1 for `td.map`/`td.blk`, 2 for `td2.map`/`td2.blk` (E2 flights
/// 9–20). E1 has one map. E3 stores 1 always (e3-mission-setup §3.1) but loads `td4.map` for
/// flights ≥ 7; this function counts those as set 2 to select the file.
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

/// The player's class in a flight (E1: structures §6; E2: e2-mission-setup §3.3; E3:
/// e3-mission-setup §3.3, the dead flight 20 included).
pub fn player_class(ep: Episode, flight: u8) -> u8 {
    match ep {
        Episode::E1 => e1_player_class(flight),
        Episode::E3 => match flight {
            4 => 4,
            8..=11 | 21 => 6,
            13 | 19 => 7,
            14 | 17 | 18 => 3,
            20 => 5,
            _ => 2,
        },
        Episode::E2 => match flight {
            4 | 9 => 4,
            5 | 6 => 7,
            7 | 8 => 3,
            11..=13 | 20 => 5,
            _ => 2,
        },
    }
}

/// Missiles at flight start for a class: class 5 starts with 4 in E2 and E3, 1 in E1; E3
/// flight 12 with class 2 starts with 10 (episode-deltas §4.3, e3-mission-setup §3.3; weapons
/// §3.2 otherwise).
pub fn start_missiles(ep: Episode, flight: u8, class: u8, table: u8) -> u8 {
    if ep == Episode::E3 && flight == 12 && class == 2 {
        10
    } else if class == 5 && ep != Episode::E1 {
        4
    } else {
        table
    }
}

/// The player's armour at flight start: the class value, 5 in E2 and E3 flight 7
/// (episode-deltas §4.3).
pub fn start_armour(ep: Episode, flight: u8, class_armour: u8) -> i8 {
    if ep != Episode::E1 && flight == 7 {
        5
    } else {
        class_armour as i8
    }
}

/// The E3 guidance home point (`UI_DrawGuidanceLine`, e3-mission-setup §3.7; a later rule
/// wins). No E3 home point depends on the view.
pub fn e3_home_point(flight: u8) -> (u8, u8) {
    let mut home = (172, 68);
    if flight == 6 {
        home = (1, 110);
    }
    if flight > 6 {
        home = (22, 166);
    }
    if flight == 11 {
        home = (166, 78);
    }
    if flight == 13 {
        home = (41, 46);
    }
    if flight >= 14 {
        home = (22, 166);
    }
    if flight == 18 {
        home = (28, 33);
    }
    home
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
        // Final before `Msn_Setup` (e3-mission-setup §4.1 step 9); late draw is never set.
        Episode::E3 => {
            tile(167, 66);
            if flight > 6 {
                tile(17, 164);
            }
            if flight == 21 {
                tile(160, 75);
            }
            if matches!(flight, 14 | 17 | 19) {
                tile(88, 76);
            }
            if flight == 7 {
                tile(185, 32);
            }
        }
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

    /// e3-mission-setup §6 vector 10.
    #[test]
    fn e3_tile_attributes() {
        let a = tile_attributes(Episode::E3, 1);
        assert_eq!(a, tile_attributes(Episode::E3, 12));
        let want: Vec<usize> = (0x10..=0x16)
            .chain([0x22])
            .chain(0x25..=0x2C)
            .chain(0x41..=0x46)
            .collect();
        assert_eq!(open(&a, 0x46), want);
    }

    /// e3-mission-setup §3.1, §3.3 and §6 vectors 11 (start view) and 12 (home point).
    #[test]
    fn e3_files_classes_views_and_home_points() {
        let e = Episode::E3;
        assert_eq!(map_file(e, 6), "td3.map");
        assert_eq!(map_file(e, 7), "td4.map");
        assert_eq!(map_file(e, 21), "td4.map");
        assert_eq!(tileset_file(e, 19), "td3.blk");
        assert_eq!(mission_file(e, 21).as_deref(), Some("m21.td3"));
        let tile = |f| {
            let v = start_view(e, f, false);
            (v.tile_x, v.tile_y, v.sub_x, v.sub_y)
        };
        for f in 1..=6 {
            assert_eq!(tile(f), (167, 66, 32, 16));
        }
        assert_eq!(tile(7), (185, 32, 32, 16));
        for f in [8, 9, 10, 11, 12, 13, 15, 16, 18] {
            assert_eq!(tile(f), (17, 164, 32, 16));
        }
        for f in [14, 17, 19] {
            assert_eq!(tile(f), (88, 76, 32, 16));
        }
        assert_eq!(tile(21), (160, 75, 32, 16));
        let classes: Vec<u8> = (1..=21).map(|f| player_class(e, f)).collect();
        assert_eq!(
            classes,
            [
                2, 2, 2, 4, 2, 2, 2, 6, 6, 6, 6, 2, 7, 3, 2, 2, 3, 3, 7, 5, 6
            ]
        );
        let home: Vec<(u8, u8)> = [3, 6, 11, 13, 14, 18, 21]
            .iter()
            .map(|&f| e3_home_point(f))
            .collect();
        assert_eq!(
            home,
            [
                (172, 68),
                (1, 110),
                (166, 78),
                (41, 46),
                (22, 166),
                (28, 33),
                (22, 166)
            ]
        );
        assert_eq!(start_missiles(e, 12, 2, 6), 10);
        assert_eq!(start_missiles(e, 1, 2, 6), 6);
        assert_eq!(start_missiles(e, 20, 5, 1), 4);
        assert_eq!(start_armour(e, 7, 15), 5);
        assert_eq!(start_armour(Episode::E1, 7, 15), 15);
    }
}
