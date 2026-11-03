//! Flight checks on the original Episode 1 map (data-gated).
//!
//! RE-2 observation (flight-model rev 2, captures C-0007/C-0008): at the E1M1 start the cells
//! beside the ship are solid and the view's Y offset is 16 (< 40), so holding right never moves
//! the view; the left and right speed components swap on every bounce.

use td_assets::DataDir;
use td_formats::map::TileMap;
use td_sim::flight::tables::episode1_tile_attributes;
use td_sim::flight::{START_VIEW, World};
use td_sim::{Flight, FlightKey, TickControls};
use td_test_support::require_data;

fn e1_world(dir: &std::path::Path) -> World {
    let data = DataDir::open(dir).expect("data dir");
    World {
        map: TileMap::parse_episode(&data.read("td.map").expect("td.map")).expect("map"),
        tile_attr: episode1_tile_attributes(),
        muzzles: td_sim::flight::shots::Muzzles(vec![0; 342]),
    }
}

#[test]
fn holding_right_at_the_e1m1_start_bounces_in_place() {
    let dir = require_data!();
    let w = e1_world(&dir);
    assert!(w.solid(START_VIEW.tile_x + 6, START_VIEW.tile_y + 2));
    let mut f = Flight::new(1);
    let mut ev = vec![];
    let mut c = TickControls::default();
    c.set_down(FlightKey::Kp6);
    c.set_made(FlightKey::Kp6);
    let mut swaps = 0;
    for _ in 0..40 {
        let before = f.player.speed;
        f.tick(&w, &c, &mut ev);
        assert_eq!(f.player.view, START_VIEW);
        if before.right > 16 && f.player.speed.left > 16 {
            swaps += 1;
        }
    }
    assert!(swaps > 5, "bounces observed: {swaps}");
}

fn hold(keys: &[FlightKey]) -> TickControls {
    let mut c = TickControls::default();
    for &k in keys {
        c.set_down(k);
        c.set_made(k);
    }
    c
}

/// flight-model.md rev 2 vector 4 (C-0007): class 4 at the start, hold right 14 ticks.
#[test]
fn vector_4_bounce_sequence() {
    let dir = require_data!();
    let w = e1_world(&dir);
    let mut f = Flight::new(1);
    let mut ev = vec![];
    let mut seen = vec![];
    for t in 1..=18 {
        let c = if t <= 14 {
            hold(&[FlightKey::Kp6])
        } else {
            TickControls::default()
        };
        f.tick(&w, &c, &mut ev);
        assert_eq!(f.player.view, START_VIEW, "tick {t}");
        if t >= 6 {
            seen.push((f.player.speed.left, f.player.speed.right));
        }
    }
    assert_eq!(
        seen,
        [
            (17, 4),
            (4, 20),
            (17, 8),
            (8, 20),
            (17, 12),
            (12, 20),
            (17, 16),
            (16, 20),
            (17, 20),
            (19, 16),
            (16, 18),
            (17, 16),
            (16, 16)
        ]
    );
}

/// Vector 5 (C-0007): class 0 facing east, hold left: the target stays east and the speeds
/// settle at (left, right) = (18, 16) with the view in place.
#[test]
fn vector_5_opposite_key_and_steady_bounce() {
    let dir = require_data!();
    let w = e1_world(&dir);
    let mut p = td_sim::flight::player::Player::new(0, START_VIEW);
    p.heading = td_sim::flight::heading::Heading::E;
    p.target = p.heading;
    let mut f = Flight::with_player(1, p);
    let mut ev = vec![];
    for _ in 0..30 {
        f.tick(&w, &hold(&[FlightKey::Kp4]), &mut ev);
        assert_eq!(f.player.target, td_sim::flight::heading::Heading::E);
    }
    for _ in 0..5 {
        f.tick(&w, &hold(&[FlightKey::Kp4]), &mut ev);
        assert_eq!((f.player.speed.left, f.player.speed.right), (18, 16));
        assert_eq!(f.player.view, START_VIEW);
    }
}

/// Vector 7 (C-0008): hold down 6 ticks from the start, then release.
#[test]
fn vector_7_down_without_carry() {
    let dir = require_data!();
    let w = e1_world(&dir);
    let mut f = Flight::new(1);
    let mut ev = vec![];
    let mut speeds = vec![];
    let mut offsets = vec![];
    for t in 1..=13 {
        let c = if t <= 6 {
            hold(&[FlightKey::Kp2])
        } else {
            TickControls::default()
        };
        f.tick(&w, &c, &mut ev);
        if t <= 6 {
            speeds.push(f.player.speed.down);
        }
        if t >= 6 {
            offsets.push(f.player.view.sub_y);
        }
    }
    assert_eq!(speeds, [4, 8, 12, 16, 20, 23]);
    assert_eq!(offsets, [20, 27, 33, 38, 42, 45, 47, 48]);
    assert_eq!(f.player.view.tile_y, START_VIEW.tile_y);
}
