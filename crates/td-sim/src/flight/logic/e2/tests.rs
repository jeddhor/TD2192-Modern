//! Unit tests from `e2-mission-logic.md` rev 1 §7 (static vectors) and the pseudocode.

use super::super::{INK, INK_ALERT, MARK_HOME, Message, PHASE_DONE};
use super::transmission;
use crate::flight::episode::{self, Episode};
use crate::flight::player::{Player, View};
use crate::flight::ships::NO_SHIP;
use crate::flight::{Session, World, tables};
use td_formats::map::TileMap;

use crate::flight::Flight;

fn open_world() -> World {
    let mut tile_attr = [tables::TILE_SOLID; 256];
    tile_attr[0] = tables::TILE_OPEN;
    World {
        map: TileMap::parse_episode(&vec![0u8; 201 * 201]).unwrap(),
        tile_attr,
        muzzles: crate::flight::shots::Muzzles(vec![0; 342]),
    }
}

/// A scripted E2 flight with every slot empty and the player at its E2 start view.
fn scripted(n: u8) -> Flight {
    let view = episode::start_view(Episode::E2, n, n == 8);
    let mut f = Flight::with_player(n, Player::new(episode::player_class(Episode::E2, n), view));
    f.episode = Episode::E2;
    f.mission.setup_e2(n);
    f
}

fn put(f: &mut Flight, s: usize, class: u8, tile: (u8, u8)) {
    let sh = &mut f.ships.0[s - 1];
    sh.class = class;
    (sh.tile_x, sh.tile_y) = tile;
    sh.armour = 100;
}

fn view(f: &mut Flight, x: i16, y: i16, sub_y: i16) {
    f.player.view = View {
        tile_x: x,
        tile_y: y,
        sub_x: 32,
        sub_y,
    };
}

#[test]
fn setup_posts_the_e2_objective() {
    let f = scripted(1);
    assert_eq!(
        (f.mission.phase, f.mission.ticker.pos, f.mission.home_marker),
        (1, 1, MARK_HOME)
    );
    assert_eq!(f.mission.ticker.message, Some(Message::E2Objective(0)));
    assert_eq!(f.mission.ticker.ink, INK);
    assert_eq!(scripted(8).mission.ticker.ink, INK_ALERT);
    assert_eq!(scripted(11).mission.ticker.ink, INK);
    assert!(scripted(11).mission.comm_pending);
    assert_eq!(scripted(12).mission.ticker.ink, INK_ALERT);
}

/// Vectors 1–3: the player is the player tile (view + (5, 3)).
#[test]
fn vectors_1_to_3_near_live_uses_the_player_tile() {
    let mut f = scripted(1);
    put(&mut f, 1, 2, (121, 110));
    assert!(f.near_live(1, 0, 15));
    for (x, near) in [(122, false), (92, false), (93, true)] {
        put(&mut f, 1, 2, (x, 107));
        assert_eq!(f.near_live(1, 0, 15), near, "x {x}");
    }
    put(&mut f, 1, NO_SHIP, (121, 110));
    assert!(!f.near_live(1, 0, 15));
}

/// Vectors 4–9 and flights 10–19 vectors 1–2, 14: base tests.
#[test]
fn base_tests() {
    let base = |n: u8, x: i16, y: i16, sub: i16, phase: u8| {
        let mut f = scripted(n);
        f.mission.phase = phase;
        f.mission.ticker.pos = 5;
        put(&mut f, 1, 2, (1, 1));
        put(&mut f, 2, 2, (1, 1));
        put(&mut f, 5, 0, (1, 1));
        put(&mut f, 12, 7, (1, 1));
        view(&mut f, x, y, sub);
        f.e2_update();
        f.succeeded
    };
    assert!(base(1, 102, 104, 36, 100));
    assert!(!base(1, 102, 104, 37, 100));
    assert!(!base(1, 102, 104, 36, 3));
    assert!(base(7, 102, 104, 35, 1));
    assert!(!base(7, 102, 104, 36, 1));
    assert!(base(9, 87, 28, 16, 100));
    assert!(!base(9, 87, 28, 32, 100));
    for (x, y, ok) in [
        (178, 35, true),
        (183, 35, true),
        (177, 35, false),
        (184, 35, false),
    ] {
        assert_eq!(base(5, x, y, 16, 100), ok, "flight 5 at ({x}, {y})");
    }
    assert!(!base(5, 180, 34, 16, 100));
    for (x, y, ok) in [
        (118, 70, true),
        (123, 70, true),
        (117, 70, false),
        (120, 71, false),
    ] {
        assert_eq!(base(6, x, y, 16, 1), ok, "flight 6 at ({x}, {y})");
    }
    assert!(base(8, 7, 150, 40, 100));
    assert!(!base(8, 8, 150, 40, 100));
    assert!(base(12, 87, 28, 31, 100));
    assert!(!base(12, 87, 28, 32, 100));
    assert!(!base(12, 87, 29, 16, 100));
    assert!(base(20, 100, 7, 48, 100));
    assert!(!base(20, 100, 8, 16, 100));
}

/// Vector C1 (C-0015 confirms it end to end): only `engage 3 → 7` and `engage 13 → 1` fire.
#[test]
fn vector_c1_flight_1_tick_1_engagements() {
    let mut f = scripted(1);
    put(&mut f, 1, 2, (20, 89));
    put(&mut f, 3, 4, (21, 159));
    put(&mut f, 7, 8, (40, 152));
    put(&mut f, 13, 7, (30, 98));
    f.e2_update();
    let ordered: Vec<usize> = (1..=20)
        .filter(|&s| f.ships.0[s - 1].record.attack_mode == 1)
        .collect();
    assert_eq!(ordered, [3, 13]);
    assert_eq!(f.ships.0[2].record.target_sel, 7);
    assert_eq!(f.ships.0[12].record.target_sel, 1);
    assert_eq!(f.mission.phase, 1);
}

/// Vector 10: the escorted truck arrives in the strip.
#[test]
fn vector_10_flight_3_arrival() {
    let mut f = scripted(3);
    put(&mut f, 1, 0, (87, 135));
    f.e2_update();
    assert_eq!(f.ships.0[0].class, NO_SHIP);
    assert_eq!(f.slot_tile(1), (84, 135));
    assert_eq!(f.mission.phase, PHASE_DONE);
    assert_eq!(f.mission.ticker.message, Some(Message::E2Flight1(3)));
    assert_eq!(f.mission.ticker.ink, INK);
    assert!(!f.succeeded);
    assert!(f.outcome());
}

/// Vector 13: flight 8's second wave.
#[test]
fn vector_13_flight_8_wave_2() {
    let mut f = scripted(8);
    f.mission.ticker.pos = 0;
    for s in 5..=11 {
        put(&mut f, s, NO_SHIP, (107, 106));
    }
    f.e2_update();
    assert_eq!(f.mission.phase, 2);
    let classes: Vec<u8> = (5..=7).map(|s| f.ships.0[s - 1].class).collect();
    assert_eq!(classes, [0, 0, 4]);
    assert_eq!(f.mission.ticker.message, Some(Message::E2Flight1(7)));
    assert_eq!(f.mission.ticker.ink, INK_ALERT);
}

/// Vector 15 and the comm table (§4.6).
#[test]
fn comm_viewer_table() {
    assert_eq!(transmission(7, 2), Some((3, PHASE_DONE)));
    assert_eq!(transmission(7, 1), None);
    assert_eq!(transmission(11, 1), Some((4, 2)));
    assert_eq!(transmission(2, 1), None);
    let mut f = scripted(7);
    f.mission.phase = 2;
    f.mission.comm_pending = true;
    assert_eq!(f.comm_viewer(), Some(3));
    assert_eq!(
        (f.mission.phase, f.mission.comm_pending),
        (PHASE_DONE, false)
    );
}

/// Flight 13 draws exactly one `Random(9)` per tick and raises a fire request on slot 2's
/// heading even when slot 2 is gone (vector 6, OB-039).
#[test]
fn flight_13_random_and_ghost_fire() {
    let mut f = scripted(13);
    let mut rng = f.rng;
    let h = rng.random(9) as u8;
    f.ships.0[1].heading = 6;
    f.e2_update();
    assert_eq!(f.rng, rng);
    assert_eq!(f.ships.0[1].target_heading, h);
    assert!(f.ships.0[1].fire[5]);
}

/// Flights 10–20 vector 11: the flight-18 base destruction timeline.
#[test]
fn flight_18_base_destruction() {
    let mut f = scripted(18);
    let mut events = Vec::new();
    put(&mut f, 6, 6, (92, 31));
    f.e2_update();
    assert!(f.mission.cutscene);
    f.e2_flight_events(&mut events);
    assert_eq!(f.mission.debris, 2);
    let busy = f.explosions.0.iter().filter(|e| e.frame == 1).count();
    assert_eq!(busy, 2);
    for t in 1..=148 {
        f.explosions.step();
        f.e2_flight_events(&mut events);
        if t == 3 {
            assert_eq!(f.mission.ticker.message, Some(Message::E2Events));
        }
        assert_eq!(f.lost, t == 148, "tick T + {t}");
    }
    assert_eq!(f.mission.debris, 150);
    assert!(f.mission.white_out);
}

/// Flights 10–20 vector 9: flight 17 is lost when the ticker goes idle after M2[15].
#[test]
fn flight_17_lost_after_the_message() {
    let mut f = scripted(17);
    put(&mut f, 2, NO_SHIP, (40, 100));
    put(&mut f, 3, NO_SHIP, (41, 100));
    f.e2_update();
    assert_eq!(f.mission.phase, 2);
    assert_eq!(f.mission.ticker.message, Some(Message::E2Flight2(15)));
    f.e2_flight_events(&mut Vec::new());
    assert!(!f.lost);
    f.mission.ticker.pos = 0;
    f.e2_update();
    f.e2_flight_events(&mut Vec::new());
    assert!(f.lost);
    assert!(!f.outcome());
}

/// Vectors 18, 19 and flights 10–20 vectors 4, 5: outcome tests.
#[test]
fn outcome_tests() {
    let mut f = scripted(8);
    assert!(!f.outcome());
    put(&mut f, 2, NO_SHIP, (1, 122));
    assert!(f.outcome());
    let f = scripted(16);
    assert!(f.outcome());
    let mut f = scripted(10);
    put(&mut f, 2, NO_SHIP, (151, 198));
    assert!(f.outcome());
    let mut f = scripted(11);
    put(&mut f, 6, NO_SHIP, (177, 167));
    assert!(!f.outcome());
    let mut f = scripted(9);
    put(&mut f, 5, NO_SHIP, (177, 169));
    assert!(!f.outcome());
}

/// Flights 10–20 vector 3: flight 10's phase 18 and the second convoy.
#[test]
fn flight_10_phase_18() {
    let mut f = scripted(10);
    f.mission.ticker.pos = 0;
    put(&mut f, 2, 1, (98, 32));
    f.e2_update();
    assert_eq!(f.mission.phase, 18);
    f.ships.0[1].class = NO_SHIP;
    f.e2_update();
    assert_eq!(f.mission.phase, 2);
    let classes: Vec<u8> = (14..=16).map(|s| f.ships.0[s - 1].class).collect();
    assert_eq!(classes, [7, 0, 0]);
}

/// e2-mission-setup §6 vectors 4 and 5 (static): flights 8 and 7 through `Flight::start`.
#[test]
fn setup_vectors_flights_7_and_8() {
    let w = open_world();
    let mut m = td_formats::mission::Mission::parse(&vec![0xFFu8; 20_080]).unwrap();
    m.slot_class = [1; 20];
    let f = Flight::start(8, Some(&m), &w, Session::for_episode(Episode::E2));
    assert!(f.late_draw);
    assert_eq!((f.player.view.tile_x, f.player.view.tile_y), (102, 100));
    assert_eq!((f.classes.max_speed[3], f.classes.accel[3]), (0x20, 4));
    assert_eq!((f.player.class, f.weapon, f.missiles), (3, 1, 20));
    let f = Flight::start(7, Some(&m), &w, Session::for_episode(Episode::E2));
    assert_eq!(
        (
            f.player.class,
            f.player.armour,
            f.player.shields,
            f.missiles
        ),
        (3, 5, 20, 20)
    );
    assert_eq!(f.ships.0[1].armour, 75);
    assert_eq!(f.ships.0[1].record.aggression, 250);
    let f = Flight::start(11, Some(&m), &w, Session::for_episode(Episode::E2));
    assert_eq!((f.player.class, f.missiles), (5, 4));
}
