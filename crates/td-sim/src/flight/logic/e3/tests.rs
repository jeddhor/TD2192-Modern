//! Unit tests from `e3-mission-logic.md` rev 1 §7 and `e3-mission-ships.md` rev 1 §5 (static
//! vectors and the pseudocode).

use crate::flight::episode::{self, Episode};
use crate::flight::player::{Event, Player, View};
use crate::flight::ships::NO_SHIP;
use crate::flight::tables::ClassTables;
use crate::flight::{Flight, World};
use crate::input::TickControls;
use td_formats::map::TileMap;

/// An E3 world: every cell tile 10h (open), the listed cells set, E3 attributes.
fn world(cells: &[(usize, usize, u8)]) -> World {
    let mut m = vec![0x10u8; 201 * 201];
    for &(x, y, t) in cells {
        m[x * 201 + y] = t;
    }
    World {
        map: TileMap::parse_episode(&m).unwrap(),
        tile_attr: episode::tile_attributes(Episode::E3, 8),
        muzzles: crate::flight::shots::Muzzles(vec![0; 342]),
    }
}

/// A scripted E3 flight with every slot empty and the player at its E3 start view.
fn scripted(n: u8) -> Flight {
    let view = episode::start_view(Episode::E3, n, false);
    let mut f = Flight::with_player(n, Player::new(episode::player_class(Episode::E3, n), view));
    f.episode = Episode::E3;
    f.mission.setup_e3(n);
    f.cells_off = n == 12;
    f
}

fn put(f: &mut Flight, s: usize, class: u8, tile: (u8, u8)) {
    let sh = &mut f.ships.0[s - 1];
    sh.class = class;
    (sh.tile_x, sh.tile_y) = tile;
    sh.armour = 100;
}

fn set_view(f: &mut Flight, x: i16, y: i16, sub_x: i16, sub_y: i16) {
    f.player.view = View {
        tile_x: x,
        tile_y: y,
        sub_x,
        sub_y,
    };
}

fn run(f: &mut Flight, w: &World, ticks: usize) -> Vec<Event> {
    let mut ev = vec![];
    for _ in 0..ticks {
        f.tick(w, &TickControls::default(), &mut ev);
    }
    ev
}

/// e3-mission-ships §7 vectors 7–9 (C-0041): view (35, 130), offsets (31, 29), the cell
/// (40, 132) = 45h; 17 per tick, through the end sequence, with the i8 wrap (OB-054).
#[test]
fn hazard_damage_wraps_through_the_end_sequence() {
    let w = world(&[(40, 132, 0x45)]);
    let mut f = scripted(8);
    set_view(&mut f, 35, 130, 31, 29);
    let ev = run(&mut f, &w, 1);
    assert_eq!((f.player.armour, f.player.shields), (14, 0));
    assert!(ev.contains(&Event::Sound {
        kind: 9,
        source: None
    }));
    run(&mut f, &w, 1);
    assert_eq!(f.player.armour, -3);
    assert!(!f.mission_over);
    run(&mut f, &w, 1);
    assert!(f.mission_over);
    assert_eq!(f.player.armour, -20);
    let mut seen = vec![];
    for _ in 0..8 {
        run(&mut f, &w, 1);
        seen.push(f.player.armour);
    }
    assert_eq!(seen, [-37, -54, -71, -88, -105, -122, 117, 100]);
}

/// The four clauses (§5.1): (4, 2) only with offset X ≤ 32; (4, 3) always.
#[test]
fn hazard_clauses() {
    let w = world(&[(39, 132, 0x41)]);
    for (sub_x, hit) in [(32, true), (33, false)] {
        let mut f = scripted(8);
        set_view(&mut f, 35, 130, sub_x, 29);
        run(&mut f, &w, 1);
        assert_eq!(f.player.armour == 14, hit, "offset X {sub_x}");
    }
    let w = world(&[(39, 133, 0x46)]);
    let mut f = scripted(8);
    set_view(&mut f, 35, 130, 40, 40);
    run(&mut f, &w, 1);
    assert_eq!(f.player.armour, 14);
    // Tile 40h is solid and not a hazard; 47h is not a hazard.
    let w = world(&[(40, 132, 0x47)]);
    let mut f = scripted(8);
    set_view(&mut f, 35, 130, 31, 29);
    run(&mut f, &w, 1);
    assert_eq!(f.player.armour, 31);
}

/// No hazard test in flights 14, 17 and 19.
#[test]
fn hazard_exclusions() {
    let w = world(&[(40, 132, 0x45)]);
    for n in [14, 17, 19] {
        let mut f = scripted(n);
        let a = f.player.armour;
        set_view(&mut f, 35, 130, 31, 29);
        run(&mut f, &w, 3);
        assert_eq!(f.player.armour, a, "flight {n}");
    }
}

/// e3-mission-logic §7 vector 12: flight 12's cells are tile 26h until the event routine of
/// tick 6480, whose hazard test (step 28 of the same tick) already sees them.
#[test]
fn flight_12_cells_return_at_tick_6480() {
    let w = world(&[(40, 122, 0x45)]);
    let mut f = scripted(12);
    assert_eq!(f.map_tile(&w, 40, 122), Some(0x26));
    set_view(&mut f, 35, 120, 31, 29);
    f.ticks = 6478;
    run(&mut f, &w, 1);
    assert_eq!(f.player.armour, 15);
    run(&mut f, &w, 1);
    assert_eq!(f.ticks, 6480);
    assert!(!f.cells_off);
    assert_eq!(f.map_tile(&w, 40, 122), Some(0x45));
    assert_eq!(f.player.armour, -2);
}

/// §4.6.2, §4.6.5: flight 14 spawns at the end of tick 25, phase 2 at 27, silent removals.
#[test]
fn flight_14_timeline() {
    let w = world(&[]);
    let mut f = scripted(14);
    for s in 6..=11 {
        put(&mut f, s, NO_SHIP, (90, 80));
    }
    f.ticks = 24;
    run(&mut f, &w, 1);
    let classes: Vec<u8> = (6..=11).map(|s| f.ships.0[s - 1].class).collect();
    assert_eq!(classes, [8, 8, 8, 7, 7, 7]);
    assert_eq!(f.mission.phase, 1);
    run(&mut f, &w, 2);
    assert_eq!(f.mission.phase, 2);
    f.ticks = 74;
    run(&mut f, &w, 1);
    assert_eq!(f.ships.0[10].armour, 0);
    run(&mut f, &w, 1);
    assert_eq!(f.ships.0[10].class, NO_SHIP);
}

/// §4.6.6, §7 vector 9: flight 5 is lost in the tick slot 2 goes, below phase 100.
#[test]
fn flight_5_lost_rule() {
    let w = world(&[]);
    let mut f = scripted(5);
    put(&mut f, 2, 0, (172, 70));
    run(&mut f, &w, 1);
    assert!(!f.lost);
    f.ships.0[1].armour = 0;
    run(&mut f, &w, 1);
    assert_eq!(f.ships.0[1].class, NO_SHIP);
    assert!(f.lost);
}

/// §4.6.4, §7 vectors 16 and 19: flight 16 phase 3 → fade, freeze, phase 5.
#[test]
fn flight_16_freeze() {
    let mut f = scripted(16);
    f.mission.phase = 3;
    f.e3_timed_events();
    assert_eq!(f.mission.phase, 5);
    assert!(f.mission.tint);
    assert_eq!(f.classes.max_speed, [0, 0, 20, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(f.classes.accel, [0, 0, 4, 0, 0, 0, 0, 0, 0, 0]);
    // Disarmed: phase 5 does not repeat it.
    f.classes = ClassTables::default();
    f.e3_timed_events();
    assert_eq!(f.classes, ClassTables::default());
}

/// §7 vector 29: no store in the other flights.
#[test]
fn timed_events_are_inert_elsewhere() {
    for n in [1, 2, 3, 4, 6, 7, 8, 9, 11, 13, 15, 17, 19] {
        let mut f = scripted(n);
        let before = f.clone();
        for t in [10, 25, 27, 75, 175, 300, 2160, 6480] {
            f.ticks = t;
            f.e3_timed_events();
        }
        f.ticks = before.ticks;
        assert_eq!(f, before, "flight {n}");
    }
}

/// §7 vectors 6 and 25: the flight-10 and flight-18 timers, flight 21's tick-10 phase.
#[test]
fn timers() {
    let mut f = scripted(10);
    put(&mut f, 4, NO_SHIP, (22, 166));
    f.ticks = 10;
    f.e3_timed_events();
    assert_eq!((f.mission.phase, f.ships.0[3].class), (2, 2));
    f.ticks = 300;
    f.e3_timed_events();
    assert_eq!(f.mission.phase, 3);
    let mut f = scripted(18);
    f.ticks = 2159;
    f.e3_timed_events();
    assert!(!f.e3_point_offered());
    f.ticks = 2160;
    f.e3_timed_events();
    assert_eq!(f.mission.phase, 2);
    assert!(f.e3_point_offered());
    let mut f = scripted(21);
    f.ticks = 10;
    f.e3_timed_events();
    assert_eq!(f.mission.phase, 2);
}
