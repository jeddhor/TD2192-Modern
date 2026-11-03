//! Flight tests from the spec's test vectors (`flight-model.md` §6, `tick-order.md` §6).

use super::player::{Player, Turn};
use super::*;
use crate::input::FlightKey;

/// A 201x201 world where every tile is open.
fn open_world() -> World {
    let mut attr = [tables::TILE_SOLID; 256];
    attr[0] = tables::TILE_OPEN;
    World {
        map: TileMap::parse_episode(&vec![0u8; 201 * 201]).unwrap(),
        tile_attr: attr,
    }
}

fn held(keys: &[FlightKey]) -> TickControls {
    let mut c = TickControls::default();
    for &k in keys {
        c.set_down(k);
        c.set_made(k);
    }
    c
}

/// World Y pixel of the view origin.
fn world_y(f: &Flight) -> i32 {
    f.player.view.world_origin().1
}

fn flight_with_class(class: u8) -> Flight {
    Flight::with_player(1, Player::new(class, START_VIEW))
}

#[test]
fn vector_1_class_4_hold_up_then_release() {
    let w = open_world();
    let mut f = flight_with_class(4);
    let mut ev = vec![];
    let start = world_y(&f);
    // (moved px, total px, up speed after the tick) for t1..t14, holding U.
    let expected = [
        (0, 0, 4),
        (0, 0, 8),
        (0, 0, 12),
        (0, 0, 16),
        (0, 0, 20),
        (4, 4, 23),
        (7, 11, 26),
        (10, 21, 29),
        (13, 34, 28),
        (12, 46, 31),
        (15, 61, 30),
        (14, 75, 29),
        (13, 88, 28),
        (12, 100, 31),
    ];
    let up = held(&[FlightKey::Kp8]);
    let mut prev = start;
    for (t, &(moved, total, speed)) in expected.iter().enumerate() {
        assert_eq!(f.tick(&w, &up, &mut ev), TickResult::Done);
        let y = world_y(&f);
        assert_eq!(
            (prev - y, start - y, f.player.speed.up),
            (moved, total, speed),
            "tick {}",
            t + 1
        );
        prev = y;
    }
    // Released: 15 px at t15, then one less per tick down to 1 at t29, then still.
    let none = TickControls::default();
    for (t, moved) in (15..=30).zip((0..=15).rev()) {
        f.tick(&w, &none, &mut ev);
        let y = world_y(&f);
        assert_eq!(prev - y, moved, "tick {t}");
        prev = y;
    }
    assert!(ev.is_empty());
}

#[test]
fn vector_2_class_0_hold_up() {
    let w = open_world();
    let mut f = flight_with_class(0);
    let mut ev = vec![];
    let up = held(&[FlightKey::Up]);
    let mut prev = world_y(&f);
    let mut moved = vec![];
    for _ in 0..19 {
        f.tick(&w, &up, &mut ev);
        moved.push(prev - world_y(&f));
        prev = world_y(&f);
    }
    assert_eq!(
        moved,
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 3, 4, 5, 6, 7, 8, 9, 8, 9]
    );
}

#[test]
fn vector_3_start_state() {
    let f = Flight::new(1);
    let p = &f.player;
    assert_eq!(p.class, 4);
    assert_eq!(p.view, START_VIEW);
    assert_eq!((p.armour, p.shields), (25, 25));
    assert_eq!(
        (p.heading, p.target, p.turn),
        (Heading::N, Heading::N, Turn::None)
    );
    assert_eq!(p.speed, Default::default());
}

#[test]
fn page_toggles_every_tick() {
    // tick-order vector 2: an odd number of ticks flips the draw page.
    let w = open_world();
    let mut f = Flight::new(1);
    let mut ev = vec![];
    for _ in 0..19 {
        f.tick(&w, &TickControls::default(), &mut ev);
    }
    assert_eq!((f.ticks, f.draw_page), (19, 1));
}

#[test]
fn steering_turns_one_step_per_tick() {
    let w = open_world();
    let mut f = Flight::new(1);
    let mut ev = vec![];
    // Right from heading N: clockwise toward E (target set at the end of tick 1, turning from
    // the top of tick 2).
    let right = held(&[FlightKey::Kp6]);
    let mut seen = vec![];
    for _ in 0..6 {
        f.tick(&w, &right, &mut ev);
        seen.push(f.player.heading.code());
    }
    assert_eq!(seen, [8, 98, 9, 96, 6, 6]);
    assert_eq!(f.player.turn, Turn::Clockwise);
    // The opposite key does not change the target: the ship keeps facing east.
    let left = held(&[FlightKey::Kp4]);
    for _ in 0..3 {
        f.tick(&w, &left, &mut ev);
    }
    assert_eq!(
        (f.player.heading, f.player.target),
        (Heading::E, Heading::E)
    );
}

#[test]
fn afterburner_bursts_once_per_make_code() {
    let w = open_world();
    let mut f = Flight::new(6); // class 2 (Stiletto)
    assert_eq!(f.player.class, 2);
    let mut ev = vec![];
    let mut c = held(&[FlightKey::Kp0]);
    let mut up = vec![];
    for t in 0..12 {
        f.tick(&w, &c, &mut ev);
        up.push(f.player.speed.up);
        if t == 0 {
            // Still held, but no new make code.
            c = TickControls::default();
            c.set_down(FlightKey::Kp0);
        }
    }
    assert_eq!(ev.len(), 1, "one sound, one burst");
    // Frame 2 sets 34 and frame 3 sets 44 (each then drags by 1 in the same tick's movement
    // step, which runs after the burst).
    assert_eq!(&up[..6], &[0, 33, 43, 42, 41, 40]);
    assert_eq!(f.player.burst_frame, 0);
}

#[test]
fn pause_resumes_the_same_tick_with_a_cleared_key_table() {
    let w = open_world();
    let mut f = Flight::new(1);
    let mut ev = vec![];
    let r = f.tick(&w, &held(&[FlightKey::P, FlightKey::Kp8]), &mut ev);
    assert_eq!(r, TickResult::Modal(Modal::Pause));
    assert_eq!(f.draw_page, 0, "interrupted before the page toggle");
    // Kp8 is still physically down but the table was cleared and no new make code arrived.
    let mut still = TickControls::default();
    still.set_down(FlightKey::Kp8);
    assert_eq!(f.resume(&w, &still, &mut ev), TickResult::Done);
    assert_eq!((f.ticks, f.draw_page, f.player.speed.up), (1, 1, 0));
}

#[test]
fn destruction_ends_the_flight_after_35_end_ticks() {
    let w = open_world();
    let mut f = Flight::new(1);
    let mut ev = vec![];
    f.player.armour = 0;
    assert_eq!(
        f.tick(&w, &TickControls::default(), &mut ev),
        TickResult::Done
    );
    assert!(f.mission_over);
    let mut n = 1;
    while f.tick(&w, &TickControls::default(), &mut ev) == TickResult::Done {
        n += 1;
        assert!(n < 100);
    }
    assert_eq!(f.end_ticks, 35);
    assert!(!f.frame.player_visible);
}

#[test]
fn bounce_swaps_opposite_components() {
    // A solid column at map x = tile_x + 6: moving right bounces at offset 32.
    let mut cells = vec![0u8; 201 * 201];
    let x = (START_VIEW.tile_x + 6) as usize;
    cells[x * 201..(x + 1) * 201].fill(1);
    let w = World {
        map: TileMap::parse_episode(&cells).unwrap(),
        tile_attr: open_world().tile_attr,
    };
    let mut f = Flight::new(1);
    let mut ev = vec![];
    let mut bounced = false;
    for _ in 0..12 {
        let before = f.player.speed;
        f.tick(&w, &held(&[FlightKey::Kp6]), &mut ev);
        assert!(
            f.player.view.sub_x <= 32,
            "never ends a tick inside the wall"
        );
        assert_eq!(f.player.view.tile_x, START_VIEW.tile_x);
        // First bounce (t6): 20 → drift 4 px into the wall, clamp, swap (left 19), then thrust right 4 and
        // brake left by 2.
        if before.right == 20 && !bounced {
            assert_eq!((f.player.speed.left, f.player.speed.right), (17, 4));
            bounced = true;
        }
    }
    assert!(bounced);
}

#[test]
fn state_hash_changes_with_input() {
    let w = open_world();
    let mut a = Flight::new(1);
    let mut b = Flight::new(1);
    let mut ev = vec![];
    assert_eq!(crate::state_hash(&a), crate::state_hash(&b));
    a.tick(&w, &held(&[FlightKey::Kp8]), &mut ev);
    b.tick(&w, &TickControls::default(), &mut ev);
    assert_ne!(crate::state_hash(&a), crate::state_hash(&b));
}
