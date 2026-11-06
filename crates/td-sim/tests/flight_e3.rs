//! Episode 3 setup on the shipped mission files (data-gated): the static vectors 3–9 of
//! `docs/spec/sim/e3-mission-setup.md` rev 1 §6 (vectors 1 and 2 are checked against the
//! captures in `oracle_e3.rs`).

use std::path::Path;

use td_assets::DataDir;
use td_formats::map::TileMap;
use td_formats::mission::Mission;
use td_sim::Flight;
use td_sim::flight::episode::{self, Episode};
use td_sim::flight::ships::NO_SHIP;
use td_sim::flight::tables::ClassTables;
use td_sim::flight::{Session, World};
use td_test_support::require_data;

const FF: u8 = NO_SHIP;

fn world(dir: &DataDir, n: u8) -> World {
    let map =
        TileMap::parse_episode(&dir.read(episode::map_file(Episode::E3, n)).unwrap()).unwrap();
    episode::world(Episode::E3, n, map, vec![0; 342])
}

/// E3 flight `n` after setup, from `session`.
fn start(data: &Path, n: u8, session: Session) -> (Flight, World) {
    let dir = DataDir::open(data).unwrap();
    let w = world(&dir, n);
    let file = episode::mission_file(Episode::E3, n).unwrap();
    let m = Mission::parse(&dir.read(&file).unwrap()).unwrap();
    (Flight::start(n, Some(&m), &w, session), w)
}

/// Expands (value, count) runs into the 20 slot values.
fn runs<T: Copy>(r: &[(T, usize)]) -> Vec<T> {
    let v: Vec<T> = r
        .iter()
        .flat_map(|&(x, n)| std::iter::repeat_n(x, n))
        .collect();
    assert_eq!(v.len(), 20);
    v
}

fn classes(f: &Flight) -> Vec<u8> {
    f.ships.0.iter().map(|s| s.class).collect()
}
fn armour(f: &Flight) -> Vec<i16> {
    f.ships.0.iter().map(|s| s.armour).collect()
}
fn shields(f: &Flight) -> Vec<i16> {
    f.ships.0.iter().map(|s| s.shields).collect()
}
fn aggression(f: &Flight) -> Vec<u8> {
    f.ships.0.iter().map(|s| s.record.aggression).collect()
}
fn markers(f: &Flight) -> Vec<u8> {
    f.ships.0.iter().map(|s| s.marker).collect()
}
/// (class, weapon, missiles, armour, shields) of the player.
fn player(f: &Flight) -> (u8, u8, u8, i8, i8) {
    (
        f.player.class,
        f.weapon,
        f.missiles,
        f.player.armour,
        f.player.shields,
    )
}
fn view(f: &Flight) -> (i16, i16, i16, i16) {
    let v = f.player.view;
    (v.tile_x, v.tile_y, v.sub_x, v.sub_y)
}

fn e3() -> Session {
    Session::for_episode(Episode::E3)
}

/// Vector 3: flight 9 (`td4.map`).
#[test]
fn e3_flight_9_setup() {
    let data = require_data!();
    let (f, _) = start(&data, 9, e3());
    assert_eq!(
        classes(&f),
        runs(&[(0, 3), (8, 2), (7, 2), (8, 2), (FF, 11)])
    );
    let tiles: Vec<(u8, u8)> = (1..=9).map(|s| f.slot_tile(s)).collect();
    assert_eq!(
        tiles,
        [
            (5, 94),
            (3, 94),
            (1, 94),
            (7, 96),
            (5, 96),
            (11, 95),
            (13, 95),
            (15, 95),
            (17, 95)
        ]
    );
    assert_eq!(
        armour(&f),
        runs(&[(100, 3), (40, 2), (50, 2), (40, 2), (68, 11)])
    );
    assert_eq!(shields(&f), runs(&[(0, 3), (25, 6), (34, 11)]));
    assert_eq!(aggression(&f), runs(&[(0, 3), (255, 6), (0, 11)]));
    assert_eq!(markers(&f), runs(&[(0x32, 3), (0, 17)]));
    assert_eq!(f.mission.home_marker, 0x3D);
    assert_eq!(player(&f), (6, 3, 6, 31, 31));
    assert_eq!(view(&f), (17, 164, 32, 16));
    assert_eq!(f.mission.phase, 1);
}

/// Vector 4: flight 12, the 27 map cells and 10 missiles.
#[test]
fn e3_flight_12_setup() {
    let data = require_data!();
    let (f, w) = start(&data, 12, e3());
    assert_eq!(classes(&f), runs(&[(0, 6), (FF, 1), (8, 8), (FF, 5)]));
    assert_eq!(armour(&f), runs(&[(62, 6), (68, 1), (40, 8), (68, 5)]));
    assert_eq!(shields(&f), runs(&[(0, 6), (34, 1), (25, 8), (34, 5)]));
    assert_eq!(aggression(&f), runs(&[(0, 7), (200, 8), (0, 5)]));
    assert_eq!(markers(&f), runs(&[(0x32, 6), (0, 14)]));
    assert_eq!(player(&f), (2, 1, 10, 15, 18));
    assert_eq!(view(&f), (17, 164, 32, 16));
    assert_eq!(w.map.get(40, 122), Some(0x45));
    assert_eq!(f.map_tile(&w, 40, 122), Some(0x26));
    assert_eq!(w.map.get(54, 137), Some(0x42));
    assert_eq!(f.map_tile(&w, 54, 137), Some(0x26));
    // The patch writes back exactly the `td4.map` bytes (e3-mission-logic §4.6.3).
    for (x, y, v) in td_sim::flight::setup::e3::FLIGHT12_CELLS {
        assert_eq!(w.map.get(x.into(), y.into()), Some(v), "({x}, {y})");
    }
    // The other ten hazard cells are untouched.
    for (x, y) in [(46, 109), (47, 109), (48, 109), (74, 78), (91, 88)] {
        let t = f.map_tile(&w, x, y).unwrap();
        assert!((0x41..=0x46).contains(&t), "({x}, {y}) = {t:02X}");
    }
}

/// Vector 5: flight 5 retypes slot 3, not slot 2.
#[test]
fn e3_flight_5_setup() {
    let data = require_data!();
    let (f, _) = start(&data, 5, e3());
    assert_eq!(
        classes(&f),
        runs(&[
            (FF, 1),
            (0, 1),
            (2, 1),
            (FF, 1),
            (7, 3),
            (FF, 1),
            (7, 1),
            (6, 2),
            (FF, 9)
        ])
    );
    assert_eq!(
        armour(&f),
        runs(&[
            (68, 1),
            (900, 1),
            (50, 1),
            (68, 1),
            (50, 3),
            (68, 1),
            (50, 1),
            (62, 2),
            (68, 9)
        ])
    );
    assert_eq!(
        shields(&f),
        runs(&[
            (34, 1),
            (0, 1),
            (25, 1),
            (34, 1),
            (25, 3),
            (34, 1),
            (25, 1),
            (31, 2),
            (34, 9)
        ])
    );
    assert_eq!(aggression(&f), runs(&[(0, 2), (250, 2), (150, 8), (0, 8)]));
    assert_eq!(
        markers(&f),
        runs(&[(0, 1), (0x39, 1), (0, 2), (0x32, 3), (0, 13)])
    );
    assert_eq!(
        [f.slot_tile(1), f.slot_tile(2), f.slot_tile(3)],
        [(0, 0), (172, 70), (166, 66)]
    );
    assert_eq!(f.player.class, 2);
    assert_eq!(view(&f), (167, 66, 32, 16));
}

/// Vector 6: flight 7 keeps the phase it inherits (OB-043) and starts with armour 5.
#[test]
fn e3_flight_7_setup() {
    let data = require_data!();
    let mut s = e3();
    s.mission.phase = 100;
    let (f, _) = start(&data, 7, s);
    assert_eq!(player(&f), (2, 1, 6, 5, 18));
    assert_eq!(f.mission.phase, 100);
    assert_eq!(view(&f), (185, 32, 32, 16));
    assert_eq!(
        classes(&f),
        [
            FF, 4, 4, FF, 4, 2, 2, 7, 0, 0, FF, 8, 8, FF, 6, 6, FF, 6, FF, FF
        ]
    );
    assert_eq!(aggression(&f), runs(&[(0, 11), (75, 7), (0, 2)]));
    assert_eq!(markers(&f), runs(&[(0, 1), (0x39, 9), (0, 10)]));
    let mut s = e3();
    s.mission.phase = 3;
    assert_eq!(start(&data, 7, s).0.mission.phase, 3);
}

/// Vector 7: flight 17 restores the class tables a flight-16 phase 3 patched (OB-028).
#[test]
fn e3_flight_17_restores_the_class_tables() {
    let data = require_data!();
    let mut s = e3();
    for c in 0..=8 {
        s.classes.max_speed[c] = 0;
        s.classes.accel[c] = 0;
    }
    s.classes.max_speed[2] = 20;
    s.classes.accel[2] = 4;
    let (f, _) = start(&data, 17, s);
    assert_eq!(f.classes, ClassTables::default());
    assert_eq!(f.mission.phase, 100);
    assert_eq!(player(&f), (3, 1, 20, 20, 20));
    assert_eq!(view(&f), (88, 76, 32, 16));
    assert_eq!(classes(&f), runs(&[(FF, 2), (7, 5), (FF, 13)]));
    assert_eq!(aggression(&f), runs(&[(0, 2), (255, 5), (0, 13)]));
    assert_eq!(markers(&f), runs(&[(0, 20)]));
    assert_eq!(f.mission.home_marker, 0x3D);
}

/// OB-028 fix-all (user decision 2026-11-05): a retry of flight 16 and a flight after a
/// restore start with the image tables (the original kept the freeze until a flight-17 setup).
#[test]
fn e3_ob028_freeze_never_leaks() {
    let data = require_data!();
    for n in [16, 8, 14] {
        let mut s = e3();
        for c in 0..=8 {
            s.classes.max_speed[c] = 0;
            s.classes.accel[c] = 0;
        }
        s.classes.max_speed[2] = 20;
        s.classes.accel[2] = 4;
        let (f, _) = start(&data, n, s);
        assert_eq!(f.classes, ClassTables::default(), "flight {n}");
    }
}

/// Vector 8: flight 18 fills every marker with 32h (OB-052).
#[test]
fn e3_flight_18_setup() {
    let data = require_data!();
    let (f, _) = start(&data, 18, e3());
    assert_eq!(markers(&f), runs(&[(0x32, 20)]));
    assert_eq!(f.mission.home_marker, 0x3D);
    assert_eq!(classes(&f), runs(&[(FF, 2), (7, 3), (8, 4), (FF, 11)]));
    assert_eq!(aggression(&f), runs(&[(0, 2), (255, 7), (0, 11)]));
    assert_eq!(f.player.class, 3);
    assert_eq!(f.mission.phase, 1);
}

/// Vector 9: Kreel's 1750 shields (OB-048).
#[test]
fn e3_flight_15_setup() {
    let data = require_data!();
    let (f, _) = start(&data, 15, e3());
    let k = &f.ships.0[14];
    assert_eq!(
        (k.class, k.armour, k.shields, k.marker, k.record.aggression),
        (6, 62, 1750, 0x32, 255)
    );
    assert_eq!(f.player.class, 2);
}
