//! Unit tests from `e3-mission-logic.md` rev 1 §7 and `e3-mission-ships.md` rev 1 §5 (static
//! vectors and the pseudocode).

use super::super::{INK, INK_ALERT, MARK_HOME, Message, PHASE_DONE};
use super::transmission;
use crate::flight::episode::{self, Episode};
use crate::flight::player::{Event, Player, View};
use crate::flight::ships::NO_SHIP;
use crate::flight::tables::ClassTables;
use crate::flight::{Flight, World};
use crate::input::TickControls;
use td_formats::map::TileMap;

use Message::{E3Flight1 as M1, E3Flight2 as M2};

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

fn ticker(f: &Flight) -> (Option<Message>, u8, u16) {
    let t = f.mission.ticker;
    (t.message, t.ink, t.pos)
}

fn view(f: &mut Flight, x: i16, y: i16, sub_y: i16) {
    set_view(f, x, y, 32, sub_y);
}

#[test]
fn setup_posts_the_e3_objective() {
    let f = scripted(1);
    assert_eq!((f.mission.phase, f.mission.home_marker), (1, MARK_HOME));
    assert_eq!(ticker(&f), (Some(Message::E3Objective(0)), INK, 1));
    assert_eq!(scripted(8).mission.ticker.ink, INK_ALERT);
    assert_eq!(scripted(10).mission.ticker.ink, INK);
    assert_eq!(
        scripted(21).mission.ticker.message,
        Some(Message::E3Objective(11))
    );
    assert_eq!(
        scripted(12).mission.ticker.message,
        Some(Message::E3Objective(12))
    );
    assert_eq!(scripted(17).mission.phase, PHASE_DONE);
    assert!(!scripted(15).mission.comm_pending);
}

/// §7 vectors 2–4 (flights 1–9) and 1–5 (flights 10–21).
#[test]
fn base_tests() {
    let base = |n: u8, x: i16, y: i16, sub_y: i16, phase: u8| {
        let mut f = scripted(n);
        f.mission.phase = phase;
        view(&mut f, x, y, sub_y);
        f.msn_update();
        f.succeeded
    };
    assert!(base(1, 167, 66, 31, 100));
    assert!(!base(1, 167, 66, 32, 100));
    assert!(!base(1, 167, 66, 31, 5));
    for (y, ok) in [(101, true), (114, true), (100, false), (115, false)] {
        assert_eq!(base(6, 6, y, 48, 100), ok, "flight 6 y {y}");
    }
    assert!(!base(6, 7, 108, 16, 100));
    for n in [7, 9, 10, 12, 15, 16, 17, 19] {
        assert!(base(n, 17, 164, 35, 100), "flight {n}");
        assert!(!base(n, 17, 164, 36, 100), "flight {n}");
        assert!(!base(n, 17, 165, 35, 100), "flight {n}");
    }
    assert!(!base(10, 17, 164, 35, 40));
    assert!(base(14, 17, 164, 16, 1));
    assert!(base(21, 17, 164, 16, 1));
    assert!(base(11, 161, 76, 48, 1));
    assert!(!base(11, 162, 76, 16, 1));
    assert!(!base(11, 160, 77, 16, 1));
    assert!(base(13, 37, 45, 16, 1));
    assert!(!base(13, 38, 45, 16, 1));
    assert!(!base(13, 36, 43, 16, 1));
    assert!(base(18, 23, 31, 48, 100));
    assert!(!base(18, 23, 31, 48, 2));
    assert!(!base(18, 23, 32, 16, 100));
}

/// §7 vectors 5–7: flight 1's waves and its completion set (OB-045).
#[test]
fn flight_1_waves() {
    let mut f = scripted(1);
    put(&mut f, 8, 8, (80, 10));
    for s in 11..=14 {
        put(&mut f, s, NO_SHIP, (2 * s as u8 - 21, 111));
    }
    for s in 2..=4 {
        put(&mut f, s, NO_SHIP, (85, 190));
    }
    f.msn_update();
    assert_eq!(f.mission.phase, 3);
    let classes: Vec<u8> = (11..=14).map(|s| f.ships.0[s - 1].class).collect();
    assert_eq!(classes, [7, 7, 7, 7]);
    assert_eq!(f.ships.0[1].class, NO_SHIP);
    f.ships.0[11].class = NO_SHIP;
    f.msn_update();
    assert_eq!(f.mission.phase, 5);
    assert_eq!(f.ships.0[1].class, 7);
    for s in [8, 2, 11, 12] {
        f.ships.0[s - 1].class = NO_SHIP;
    }
    f.msn_update();
    assert_eq!(f.mission.phase, PHASE_DONE);
    assert_eq!(ticker(&f), (Some(M1(0)), INK, 1));
}

/// §7 vectors 8 and 15 (flight 5 arrival) and the outcome tests (§4.8).
#[test]
fn flight_5_arrival_and_outcome() {
    let mut f = scripted(5);
    put(&mut f, 2, 0, (80, 199));
    f.msn_update();
    assert_eq!(f.ships.0[1].class, NO_SHIP);
    assert_eq!(f.slot_tile(2), (80, 199));
    assert_eq!(f.mission.phase, PHASE_DONE);
    assert_eq!(f.mission.ticker.message, Some(M1(5)));
    f.e3_timed_events();
    assert!(!f.lost);
    assert!(f.outcome());
    f.ships.0[1].tile_y = 150;
    assert!(!f.outcome());
}

/// §7 vectors 10 and 11: flight 7 with the inherited phase (OB-043).
#[test]
fn flight_7_phase() {
    let mut f = scripted(7);
    f.mission.phase = 100;
    view(&mut f, 17, 164, 20);
    f.msn_update();
    assert!(f.succeeded);
    assert_ne!(f.mission.ticker.message, Some(M1(7)));
    let mut f = scripted(7);
    f.mission.phase = 1;
    put(&mut f, 5, 4, (21, 166));
    f.msn_update();
    assert_eq!(f.ships.0[4].class, NO_SHIP);
    assert_eq!(f.mission.phase, PHASE_DONE);
    assert_eq!(ticker(&f), (Some(M1(7)), INK_ALERT, 1));
}

/// §7 vectors 12–15: flight 8's silent window, the attack phase and the outcome.
#[test]
fn flight_8() {
    let mut f = scripted(8);
    for s in 1..=3 {
        put(&mut f, s, 0, (60, 168));
    }
    for s in 4..=10 {
        put(&mut f, s, 8, (120, 60));
    }
    // Player tile (22, 167); slot 6 within 14 on both axes.
    put(&mut f, 6, 8, (36, 153));
    f.msn_update();
    assert_eq!(f.mission.phase, 2);
    assert!(!f.mission.comm_pending);
    assert_eq!(f.comm_viewer(), Some(1));
    assert_eq!(f.mission.phase, 3);
    f.ships.0[4].class = NO_SHIP;
    f.msn_update();
    assert_eq!(f.mission.phase, 4);
    let mut f = scripted(8);
    for s in 1..=10 {
        put(&mut f, s, if s < 4 { 0 } else { 8 }, (120, 60));
    }
    f.ships.0[1].record.waypoint = (108, 115);
    f.msn_update();
    assert_eq!(f.mission.phase, 4);
    for s in 1..=3 {
        put(&mut f, s, NO_SHIP, (22, 166));
    }
    assert!(f.outcome());
    f.ships.0[1].tile_x = 60;
    assert!(!f.outcome());
}

/// §7 vectors 6–9: flight 10's counter phases, the truck and the outcome.
#[test]
fn flight_10() {
    let mut f = scripted(10);
    put(&mut f, 5, NO_SHIP, (22, 166));
    f.mission.phase = 3;
    f.msn_update();
    assert_eq!(f.ships.0[4].class, 0);
    assert_eq!(f.mission.phase, 5);
    assert_eq!(ticker(&f), (Some(M2(0)), INK_ALERT, 1));
    // The truck still on its exit tile: closed until phase 39.
    for _ in 0..33 {
        f.msn_update();
    }
    assert_eq!(f.mission.phase, 38);
    assert_eq!(f.ships.0[4].class, 0);
    // B to 39, C removes the truck at home, D in the same block.
    f.msn_update();
    assert_eq!(f.ships.0[4].class, NO_SHIP);
    assert_eq!(f.mission.phase, PHASE_DONE);
    assert_eq!(f.mission.ticker.message, Some(M2(2)));
    assert!(f.outcome());
    // Destroyed elsewhere at phase 13: M2[1], outcome 0.
    let mut f = scripted(10);
    put(&mut f, 5, NO_SHIP, (30, 100));
    f.mission.phase = 12;
    f.msn_update();
    assert_eq!(f.mission.phase, PHASE_DONE);
    assert_eq!(f.mission.ticker.message, Some(M2(1)));
    assert!(!f.outcome());
}

/// §7 vectors 10 and 11: flight 12.
#[test]
fn flight_12() {
    let mut f = scripted(12);
    f.msn_update();
    assert_eq!(f.mission.phase, PHASE_DONE);
    assert_eq!(f.mission.ticker.message, Some(M2(3)));
    assert!(!f.succeeded);
    put(&mut f, 4, NO_SHIP, (93, 76));
    assert!(!f.outcome());
    f.ships.0[3].tile_y = 77;
    assert!(f.outcome());
}

/// §7 vectors 14 and 15: Kreel's announcement range and its loss.
#[test]
fn flight_15() {
    for (vx, fires) in [(42, false), (43, true)] {
        let mut f = scripted(15);
        put(&mut f, 15, 6, (102, 80));
        view(&mut f, vx, 60, 16);
        f.msn_update();
        assert_eq!(f.mission.phase == 2, fires, "view X {vx}");
        assert_eq!(f.mission.comm_pending, fires);
    }
    let mut f = scripted(15);
    f.mission.phase = 2;
    f.mission.comm_pending = true;
    f.msn_update();
    assert!(!f.mission.comm_pending);
    assert_eq!(f.mission.phase, PHASE_DONE);
    assert_eq!(f.mission.ticker.message, Some(M2(5)));
    assert_eq!(f.comm_viewer(), None);
}

/// §7 vectors 16, 19 and 22: flight 16's forced transmission, freeze and blocked F5.
#[test]
fn flight_16_chain() {
    let w = world(&[]);
    let mut f = scripted(16);
    // Away from the base, on back page 1 in tick 1 (page 0 in tick 2).
    view(&mut f, 40, 150, 16);
    f.draw_page = 1;
    run(&mut f, &w, 1);
    assert_eq!(f.mission.phase, 2);
    assert_eq!(f.mission.ticker.message, Some(M2(4)));
    // Next tick: step 15 with page 0 opens screen 3.
    let r = f.tick(&w, &TickControls::default(), &mut vec![]);
    assert_eq!(
        r,
        crate::flight::TickResult::Modal(crate::flight::Modal::CommViewer)
    );
    assert_eq!(f.mission.transmission, 3);
    assert_eq!(f.mission.phase, 3);
    assert!(!f.mission.comm_pending);
    f.resume(&w, &TickControls::default(), &mut vec![]);
    assert_eq!(f.mission.phase, 5);
    assert!(f.mission.tint);
    assert_eq!(f.classes.max_speed[2], 20);
    run(&mut f, &w, 1);
    assert_eq!(f.mission.phase, PHASE_DONE);
    assert_eq!(f.mission.ticker.message, Some(M2(6)));
    let mut c = TickControls::default();
    c.set_down(crate::input::FlightKey::F5);
    c.set_made(crate::input::FlightKey::F5);
    assert_eq!(f.tick(&w, &c, &mut vec![]), crate::flight::TickResult::Done);
}

/// §7 vector 25: flight 18's rule A one tick after the timer.
#[test]
fn flight_18() {
    let mut f = scripted(18);
    for s in 1..=20 {
        assert_eq!(f.ships.0[s - 1].marker, 0);
        f.ships.0[s - 1].marker = 0x32;
    }
    f.mission.phase = 2;
    f.msn_update();
    assert_eq!(f.mission.phase, PHASE_DONE);
    assert!(f.ships.0.iter().all(|s| s.marker == 0));
    assert_eq!(f.mission.home_marker, MARK_HOME);
    assert_eq!(ticker(&f), (Some(M2(7)), INK_ALERT, 1));
}

/// §7 vectors 26 and 27: Osyluth.
#[test]
fn flight_19() {
    for (x, fires) in [(69, true), (68, false)] {
        let mut f = scripted(19);
        put(&mut f, 2, 8, (x, 96));
        f.msn_update();
        assert_eq!(f.mission.phase == 2, fires, "x {x}");
    }
    let mut f = scripted(19);
    f.mission.phase = 3;
    f.msn_update();
    assert_eq!(f.mission.phase, PHASE_DONE);
    assert_eq!(f.mission.ticker.message, Some(M2(8)));
}

/// §4.5: the comm table; §7 vector 28: no outcome test in 11, 13..19, 21.
#[test]
fn comm_table_and_outcomes() {
    assert_eq!(transmission(8, 2), Some((1, 3)));
    assert_eq!(transmission(16, 2), Some((3, 3)));
    assert_eq!(transmission(19, 2), Some((5, 3)));
    assert_eq!(transmission(15, 2), Some((2, 3)));
    assert_eq!(transmission(8, 3), None);
    assert_eq!(transmission(1, 2), None);
    for n in [1, 6, 7, 11, 13, 14, 15, 16, 17, 18, 19, 21] {
        let mut f = scripted(n);
        for s in 1..=20 {
            put(&mut f, s, NO_SHIP, (93, 199));
        }
        assert!(f.outcome(), "flight {n}");
    }
    let mut f = scripted(2);
    put(&mut f, 4, NO_SHIP, (84, 199));
    assert!(!f.outcome());
    let mut f = scripted(4);
    put(&mut f, 6, NO_SHIP, (199, 111));
    assert!(!f.outcome());
}
