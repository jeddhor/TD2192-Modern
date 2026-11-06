//! L0 tests from mission-ships §4.2–§4.3 and §6.

use td_formats::map::TileMap;

use super::super::shots::Muzzles;
use super::super::tables::{TILE_OPEN, TILE_SOLID};
use super::*;

/// A 201x201 world, open except for the solid cells listed.
fn world(solid: &[(usize, usize)]) -> World {
    let mut map = vec![0u8; 201 * 201];
    for &(x, y) in solid {
        map[x * 201 + y] = 1;
    }
    let mut attr = [TILE_SOLID; 256];
    attr[0] = TILE_OPEN;
    World {
        map: TileMap::parse_episode(&map).unwrap(),
        tile_attr: attr,
        muzzles: Muzzles(vec![0; 342]),
    }
}

/// Slot 1 (class 0) on a straight path down from (124, 73), set up as §4.1 does.
fn slot1(w: &World, rng: &mut BpRandom) -> Ships {
    let mut ships = Ships::default();
    let s = &mut ships.0[0];
    s.class = 0;
    s.armour = 62;
    for k in 0..20u8 {
        s.record.path[usize::from(k)] = (124, 73 + k);
    }
    s.record.waypoint = (124, 73);
    s.record.return_pt = (124, 73);
    (s.tile_x, s.tile_y) = (124, 73);
    (s.sub_x, s.sub_y) = (16, 16);
    s.heading = 6;
    s.target_heading = 6;
    let mut ctx = AiContext {
        world: w,
        rng,
        player: (107, 107),
    };
    ships.choose_waypoint(0, &mut ctx);
    ships
}

#[test]
fn setup_choose_waypoint_sets_the_cursor() {
    let w = world(&[]);
    let mut rng = BpRandom::new();
    let ships = slot1(&w, &mut rng);
    let r = &ships.0[0].record;
    assert_eq!(
        (r.path_cursor, r.waypoint, r.prev, r.return_pt),
        (1, (124, 73), (124, 73), (124, 73))
    );
}

/// §6 vector 2 and the §4.2.3 speed vector (C-0009 slot 1).
#[test]
fn vector_2_path_following_and_speed() {
    let w = world(&[]);
    let mut rng = BpRandom::new();
    let mut ships = slot1(&w, &mut rng);
    let classes = ClassTables::default();
    let mut log = Vec::new();
    for _ in 1..=17 {
        let mut ctx = AiContext {
            world: &w,
            rng: &mut rng,
            player: (107, 107),
        };
        ships.update(1, &classes, &mut ctx);
        let s = ships.0[0];
        log.push((
            s.speed[1],
            s.tile_y,
            s.sub_y,
            s.record.waypoint.1,
            s.record.path_cursor,
        ));
    }
    let s = ships.0[0];
    assert_eq!(log[0], (2, 73, 16, 74, 2), "tick 1");
    assert_eq!((s.target_heading, s.turn, s.record.move_dir), (2, 2, 2));
    let speeds: Vec<u8> = log.iter().map(|l| l.0).collect();
    assert_eq!(
        speeds,
        [
            2, 4, 6, 8, 10, 12, 14, 16, 17, 18, 19, 20, 21, 22, 23, 24, 23
        ]
    );
    let sub: Vec<i16> = log[8..].iter().map(|l| l.2).collect();
    assert_eq!(sub, [18, 21, 25, 30, 36, 43, 19, 28, 36]);
    assert_eq!(log[14].1, 74, "tile Y 73 → 74 at tick 15");
    assert_eq!(
        (log[15].3, log[15].4),
        (75, 3),
        "waypoint (124, 75), cursor 3 at tick 16"
    );
}

/// §4.2 (d): the heading steps 6 → 24h → 3 → 20h → 2 in ticks 2–5.
#[test]
fn heading_turns_one_ring_step_per_tick() {
    let w = world(&[]);
    let mut rng = BpRandom::new();
    let mut ships = slot1(&w, &mut rng);
    let classes = ClassTables::default();
    let mut headings = Vec::new();
    for _ in 1..=5 {
        let mut ctx = AiContext {
            world: &w,
            rng: &mut rng,
            player: (107, 107),
        };
        ships.update(1, &classes, &mut ctx);
        headings.push(ships.0[0].heading);
    }
    assert_eq!(headings, [6, 0x24, 3, 0x20, 2]);
}

#[test]
fn exit_marker_and_armour_remove_ships() {
    let w = world(&[]);
    let mut rng = BpRandom::new();
    let mut ships = slot1(&w, &mut rng);
    ships.0[0].record.waypoint = (50, 1);
    ships.0[0].armour = 5;
    let mut ctx = AiContext {
        world: &w,
        rng: &mut rng,
        player: (0, 0),
    };
    ships.update(1, &ClassTables::default(), &mut ctx);
    assert!(!ships.0[0].alive());
}

#[test]
fn attack_in_line_fires_and_then_returns() {
    // §6 vector 4 shape: waypoint two tiles west of the player on its row, aggression 50.
    let w = world(&[]);
    let mut rng = BpRandom::new();
    let mut ships = Ships::default();
    let s = &mut ships.0[0];
    s.class = 7;
    s.record.waypoint = (105, 107);
    s.record.prev = (104, 107);
    s.record.return_pt = (92, 111);
    s.record.aggression = 50;
    s.record.attack_counter = 118;
    ships.order_attack(0, TARGET_PLAYER);
    let mut ctx = AiContext {
        world: &w,
        rng: &mut rng,
        player: (107, 107),
    };
    ships.choose_waypoint(0, &mut ctx);
    let s = ships.0[0];
    assert_eq!(s.record.waypoint, (105, 107), "holds position");
    assert!(s.fire[5], "fire request 6");
    assert_eq!(s.record.attack_counter, 169);
    assert_eq!(s.record.hold_flag, 1);
    ships.choose_waypoint(0, &mut ctx);
    let s = ships.0[0];
    assert_eq!(s.record.return_mode, 1);
    assert_eq!(s.record.hold_count, 1);
    assert_eq!(s.record.waypoint, (104, 108), "toward the return point");
    assert_eq!(s.record.attack_counter, 220, "second request");
}

#[test]
fn blocked_direct_step_starts_a_detour() {
    // Solid wall north of the waypoint: the Y step is blocked.
    let w = world(&[(93, 109), (94, 109), (95, 109), (96, 109)]);
    let mut rng = BpRandom::new();
    let mut ships = Ships::default();
    let s = &mut ships.0[0];
    s.class = 7;
    s.record.waypoint = (94, 110);
    s.record.prev = (93, 111);
    s.record.move_dir = 8;
    s.record.hand_bit = 1;
    s.record.aggression = 50;
    ships.order_attack(0, TARGET_PLAYER);
    let mut ctx = AiContext {
        world: &w,
        rng: &mut rng,
        player: (107, 100),
    };
    ships.choose_waypoint(0, &mut ctx);
    let r = ships.0[0].record;
    assert_eq!(r.detour_steps, 1);
    assert_eq!(r.direct_mode, 0);
    assert_ne!(r.waypoint, (94, 110));
}

#[test]
fn record_bytes_overlay_the_path() {
    let mut s = Ship::EMPTY;
    s.record.path_cursor = 0x0102;
    s.record.path[0] = (7, 8);
    s.record.move_dir = 6;
    s.record.aggression = 50;
    s.shields = 0x0304;
    assert_eq!(s.path_step(0), (2, 1));
    assert_eq!(s.path_step(1), (7, 8));
    assert_eq!(s.path_step(501), (6, 50));
    assert_eq!(s.record_bytes().len(), RECORD_LEN);
    assert_eq!(s.record_byte(0x3F8), 4);
}

/// Slot 1 (class 7, aggression 2: a budget of 15 × 2 = 30 attack steps) at waypoint
/// (100, 100) with its return point there, ordered to attack slot 2, whose waypoint is
/// (160, 100): too far to get in line or in range, so only the budget can end the attack.
fn attacker_and_target() -> Ships {
    let mut ships = Ships::default();
    let a = &mut ships.0[0];
    a.class = 7;
    a.armour = 50;
    a.record.waypoint = (100, 100);
    a.record.prev = (99, 100);
    a.record.return_pt = (100, 100);
    a.record.aggression = 2;
    let t = &mut ships.0[1];
    t.class = 8;
    t.armour = 50;
    t.record.waypoint = (160, 100);
    ships.order_attack(0, 2);
    ships
}

#[test]
fn attack_goes_on_after_the_target_dies_until_the_budget_runs_out() {
    // mission-ships rev 4 §4.3.10 (U-181): the target's death (class FFh) does not end the
    // attack. The attacker keeps heading for the target's last waypoint, one tile per call,
    // until its budget sends it home (counter 31 > 30 at the 32nd attack step).
    let w = world(&[]);
    let mut rng = BpRandom::new();
    let mut ships = attacker_and_target();
    ships.0[1].class = NO_SHIP;
    ships.0[1].armour = 0;
    let mut ctx = AiContext {
        world: &w,
        rng: &mut rng,
        player: (20, 20),
    };
    for call in 1..=31u8 {
        ships.choose_waypoint(0, &mut ctx);
        let r = ships.0[0].record;
        assert_eq!(
            (r.attack_mode, r.return_mode, r.target_sel),
            (1, 0, 2),
            "call {call}"
        );
        assert_eq!(
            r.target,
            (160, 100),
            "call {call}: the dead target's last waypoint"
        );
        assert_eq!(r.waypoint, (100 + call, 100), "call {call}");
        assert_eq!(r.attack_counter, u16::from(call));
    }
    ships.choose_waypoint(0, &mut ctx);
    let r = ships.0[0].record;
    assert_eq!(
        (r.attack_mode, r.return_mode, r.target_sel, r.attack_counter),
        (0, 1, TARGET_RETURN, 0),
        "the budget ends the attack"
    );
    // The attack step that spends the budget already steps toward the return point, and the
    // return step runs in the same call: two tiles back.
    assert_eq!(r.waypoint, (129, 100), "heading back to the return point");
}

#[test]
fn repeated_attack_orders_do_not_reset_the_budget() {
    // §4.3.10: `AI_SetTarget` leaves attackCounter alone, so a mission rule that re-orders the
    // attack every tick changes nothing while it runs: the ship goes home at the same call as
    // with a single order. An order after that sets attack mode again while return mode is
    // still 1: the return step then heads for the attack target, not the return point.
    let w = world(&[]);
    let (mut rng_a, mut rng_b) = (BpRandom::new(), BpRandom::new());
    let (mut once, mut every) = (attacker_and_target(), attacker_and_target());
    for call in 1..=32 {
        every.order_attack(0, 2);
        let mut ctx = AiContext {
            world: &w,
            rng: &mut rng_a,
            player: (20, 20),
        };
        once.choose_waypoint(0, &mut ctx);
        let mut ctx = AiContext {
            world: &w,
            rng: &mut rng_b,
            player: (20, 20),
        };
        every.choose_waypoint(0, &mut ctx);
        assert_eq!(once.0[0].record, every.0[0].record, "call {call}");
    }
    assert_eq!(every.0[0].record.return_mode, 1, "home at call 32 in both");
    every.order_attack(0, 2);
    let mut ctx = AiContext {
        world: &w,
        rng: &mut rng_b,
        player: (20, 20),
    };
    every.choose_waypoint(0, &mut ctx);
    let mut ctx = AiContext {
        world: &w,
        rng: &mut rng_a,
        player: (20, 20),
    };
    once.choose_waypoint(0, &mut ctx);
    // From (129, 100): one return step west; the re-ordered ship runs the attack step and the
    // return step, both east.
    assert_eq!(
        once.0[0].record.waypoint,
        (128, 100),
        "single order: back home"
    );
    let r = every.0[0].record;
    assert_eq!(
        r.waypoint,
        (131, 100),
        "re-ordered: toward the attack target again"
    );
    assert_eq!((r.return_mode, r.attack_counter), (1, 1));
}
