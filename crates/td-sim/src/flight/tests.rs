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
        muzzles: shots::Muzzles(vec![0; 342]),
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
        muzzles: shots::Muzzles(vec![0; 342]),
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

// ---- weapons.md §6 ----

use super::ships::Ship;
use super::shots::Shot;

/// A shot of `kind` from `owner`, placed at world pixel `at` (tile + offset 16).
fn shot_at(kind: u8, owner: u8, at: (i32, i32), heading: u8) -> Shot {
    Shot {
        tile_x: (at.0 / 32) as u8,
        tile_y: (at.1 / 32) as u8,
        off_x: (at.0 % 32) as i16,
        off_y: (at.1 % 32) as i16,
        heading,
        kind,
        life: 1,
        owner,
    }
}

fn ship(class: u8, tile: (u8, u8), shields: i16, armour: i16) -> Ship {
    Ship {
        class,
        tile_x: tile.0,
        tile_y: tile.1,
        sub_x: 16,
        sub_y: 16,
        heading: 2,
        shields,
        armour,
        ..Ship::EMPTY
    }
}

#[test]
fn weapons_vector_1_volleys_every_second_tick() {
    let w = open_world();
    let mut f = Flight::new(6); // class 2: four muzzles, kind 0
    let mut ev = vec![];
    let mut space = held(&[FlightKey::Space]);
    for t in 0..4 {
        f.tick(&w, &space, &mut ev);
        if t == 0 {
            let used: Vec<usize> = (0..51).filter(|&i| f.shots.0[i].alive()).collect();
            assert_eq!(used, [45, 46, 47, 48]);
            assert!(
                used.iter()
                    .all(|&i| f.shots.0[i].kind == 0 && f.shots.0[i].owner == 0)
            );
        }
        space = TickControls::default();
        space.set_down(FlightKey::Space); // held: the volley does not need new make codes
    }
    assert_eq!(f.shots.0.iter().filter(|s| s.alive()).count(), 8);
    assert!(f.shots.0[41].alive() && !f.shots.0[40].alive());
}

#[test]
fn weapons_vector_2_shields_absorb() {
    let w = open_world();
    let mut f = Flight::new(1);
    f.ships.0[0] = ship(7, (110, 100), 25, 50);
    f.shots.0[30] = shot_at(1, 0, f.ships.0[0].world(), 8);
    let seed = f.rng.seed();
    f.tick(&w, &TickControls::default(), &mut vec![]);
    // 25 − 10 = 15, then the mission-ship step regenerates one point in the same tick.
    assert_eq!((f.ships.0[0].shields, f.ships.0[0].armour), (16, 50));
    assert!(!f.shots.0[30].alive());
    assert_ne!(
        f.rng.seed(),
        seed,
        "a non-lethal hit draws Random(16) twice"
    );
    assert_eq!(f.explosions.0.iter().filter(|e| e.frame < 8).count(), 1);
}

#[test]
fn weapons_vector_3_player_hit() {
    let w = open_world();
    let mut f = Flight::new(1);
    f.player.shields = 1;
    f.player.armour = 25;
    // Stop shield regeneration from hiding the result: class 4 regenerates +1 per tick, so
    // read the values after the hit and before step 27 by making the hit the only change.
    let at = shots::player_ref(&f.player);
    f.shots.0[0] = shot_at(0, 3, at, 8);
    f.tick(&w, &TickControls::default(), &mut vec![]);
    // Hit: shields 0, armour 24; then step 27 regenerates one shield point.
    assert_eq!((f.player.shields, f.player.armour), (1, 24));
}

#[test]
fn destroying_a_ship_counts_a_kill_and_frees_the_slot() {
    let w = open_world();
    let mut f = Flight::new(1);
    f.ships.0[2] = ship(1, (110, 100), 0, 4);
    f.shots.0[3].kind = 2;
    f.shots.0[30] = shot_at(0, 0, f.ships.0[2].world(), 8);
    f.tick(&w, &TickControls::default(), &mut vec![]);
    assert_eq!(f.ships.0[2].class, ships::NO_SHIP);
    assert_eq!(f.kills, 1);
    assert_eq!(
        f.shots.0[3].kind, 1,
        "original bug: projectile kind[slot] := 1"
    );
    assert_eq!(f.explosions.0.iter().filter(|e| e.frame < 8).count(), 5);
}

#[test]
fn shots_move_a_tile_per_tick_and_expire() {
    let w = open_world();
    let mut f = Flight::new(1);
    f.shots.0[10] = shot_at(0, 5, (100 * 32 + 16, 100 * 32 + 16), 6);
    for t in 1..=24 {
        f.tick(&w, &TickControls::default(), &mut vec![]);
        assert_eq!(f.shots.0[10].tile_x, 100 + t, "tick {t}");
    }
    f.tick(&w, &TickControls::default(), &mut vec![]);
    assert!(!f.shots.0[10].alive(), "life > 25");
}

#[test]
fn missiles_need_a_new_press_and_a_cooldown() {
    let w = open_world();
    let mut f = Flight::new(1); // class 4: weapon 2, missile weapon 4, 4 missiles
    assert_eq!((f.weapon, f.missiles), (2, 4));
    let mut ev = vec![];
    f.tick(&w, &held(&[FlightKey::Tab]), &mut ev);
    assert_eq!(f.weapon, 4);
    f.tick(&w, &held(&[FlightKey::Space]), &mut ev);
    assert_eq!((f.missiles, f.missile_cooldown), (3, 3));
    // Held without a new make code: no second missile.
    let mut still = TickControls::default();
    still.set_down(FlightKey::Space);
    f.tick(&w, &still, &mut ev);
    assert_eq!(f.missiles, 3);
    // A new press during the cooldown does nothing; after it, one more missile.
    f.tick(&w, &held(&[FlightKey::Space]), &mut ev);
    assert_eq!(f.missiles, 3);
    for _ in 0..2 {
        f.tick(&w, &TickControls::default(), &mut ev);
    }
    f.tick(&w, &held(&[FlightKey::Space]), &mut ev);
    assert_eq!(f.missiles, 2);
}
