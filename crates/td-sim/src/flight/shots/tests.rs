//! Hits on mission ships and the effects they request (`weapons.md` rev 3 §4.3 step 4,
//! `sfx-policy.md` rev 2 §3.2: `0E8B:106B` hit 9, `11C4` destroyed 100, `122D` kind 6 100;
//! episode-deltas P5: E2 has no `122D` request).

use super::*;
use crate::flight::ships::Ship;
use crate::flight::{START_VIEW, tables};
use td_formats::map::TileMap;

/// A ship of class 7 with `armour` (no shields) at tile (100, 100) hit by a player shot of
/// `kind` in `episode`; the effects requested.
fn hit(kind: u8, armour: i16, episode: Episode) -> Vec<Event> {
    let mut attr = [tables::TILE_SOLID; 256];
    attr[0] = tables::TILE_OPEN;
    let world = World {
        map: TileMap::parse_episode(&vec![0u8; 201 * 201]).unwrap(),
        tile_attr: attr,
        muzzles: Muzzles(vec![0; 342]),
    };
    let mut ships = Ships::default();
    ships.0[4] = Ship {
        class: 7,
        tile_x: 100,
        tile_y: 100,
        sub_x: 16,
        sub_y: 16,
        heading: 2,
        armour,
        ..Ship::EMPTY
    };
    let mut shots = Shots::default();
    shots.0[45] = Shot {
        tile_x: 100,
        tile_y: 100,
        off_x: 16,
        off_y: 16,
        heading: 8,
        kind,
        life: 1,
        owner: 0,
    };
    let mut player = Player::new(4, START_VIEW);
    let mut explosions = Explosions::default();
    let mut rng = BpRandom::new();
    let mut events = Vec::new();
    let (mut kills, mut white, mut flash) = (0, 0, 0);
    let mut c = Combat {
        world: &world,
        ships: &mut ships,
        player: &mut player,
        explosions: &mut explosions,
        rng: &mut rng,
        events: &mut events,
        kills: &mut kills,
        white_flash: &mut white,
        hit_flash: &mut flash,
        late_draw: false,
        flight: 1,
        difficulty: 1,
        episode,
    };
    shots.hit_ship(45, 4, &mut c);
    events
}

fn at_ship(kind: u8) -> Event {
    Event::Sound {
        kind,
        source: Some((100, 100)),
    }
}

#[test]
fn homing_kill_requests_100_twice() {
    // Kind 6 deals 65: a ship with 20 armour is destroyed by the damage (100 at `11C4`), then
    // the kind-6 branch requests 100 again (`122D`). E3 keeps it; E2 drops it (P5).
    assert_eq!(tables::ship_damage(6), 65);
    for ep in [Episode::E1, Episode::E3] {
        assert_eq!(
            hit(6, 20, ep),
            [at_ship(9), at_ship(100), at_ship(100)],
            "{ep}"
        );
        // Not destroyed by the damage: destroyed outright, one 100.
        assert_eq!(hit(6, 200, ep), [at_ship(9), at_ship(100)], "{ep}");
    }
    assert_eq!(hit(6, 20, Episode::E2), [at_ship(9), at_ship(100)]);
    assert_eq!(hit(6, 200, Episode::E2), [at_ship(9)]);
}

#[test]
fn ordinary_hits_request_9_and_100_on_a_kill() {
    // Kind 0 deals 5.
    for ep in [Episode::E1, Episode::E2, Episode::E3] {
        assert_eq!(hit(0, 20, ep), [at_ship(9)], "{ep}");
        assert_eq!(hit(0, 3, ep), [at_ship(9), at_ship(100)], "{ep}");
    }
}
