//! Hits on mission ships and the effects they request (`weapons.md` rev 5 §4.3 steps 3–4,
//! `sfx-policy.md` rev 4 §3.2: `0E8B:106B` hit 9, `11C4` destroyed 100; the kind-6 request at
//! `122D` is unreachable, U-421, so no test relies on it), and the per-slot order of weapons
//! rev 6 §4.3 (the latched heading, step 1's skip; U-502, BUG-11).

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

/// A live class-7 ship in slot 5 at tile (100, 100) and a player homing missile (kind 6) in
/// projectile slot 45, 24 px from it (inside step 3's 40 px, outside step 4's 20 px), run through
/// one shot update in `episode`: the effects, projectile 5's kind, the kills and the ship's armour.
fn homing(episode: Episode) -> (Vec<Event>, u8, i16, i16) {
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
        armour: 200,
        ..Ship::EMPTY
    };
    let mut shots = Shots::default();
    shots.0[45] = Shot {
        tile_x: 100,
        tile_y: 100,
        off_x: 40,
        off_y: 16,
        heading: 8,
        kind: HOMING,
        life: 5,
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
    shots.update(&mut c);
    assert!(!shots.0[45].alive(), "step 3 frees the missile");
    (events, shots.0[5].kind, kills, ships.0[4].armour)
}

#[test]
fn homing_kill_is_step_3_only() {
    // weapons rev 5 §4.3 step 3 (U-421): the missile destroys the first live ship within 40 px
    // and is freed, so step 4 (and its kind-6 branch) never sees it. E1 and E3: no sound, one
    // kill, projectile kind[s] := 1; E2 (P4): sound 100 at the ship, no kind write.
    for ep in [Episode::E1, Episode::E3] {
        let (events, kind5, kills, armour) = homing(ep);
        assert!(events.is_empty(), "{ep}: {events:?}");
        assert_eq!((kind5, kills, armour), (1, 1, 0), "{ep}");
    }
    let (events, kind5, kills, armour) = homing(Episode::E2);
    assert_eq!(events, [at_ship(100)]);
    assert_eq!((kind5, kills, armour), (0, 1, 0));
}

#[test]
fn ordinary_hits_request_9_and_100_on_a_kill() {
    // Kind 0 deals 5.
    for ep in [Episode::E1, Episode::E2, Episode::E3] {
        assert_eq!(hit(0, 20, ep), [at_ship(9)], "{ep}");
        assert_eq!(hit(0, 3, ep), [at_ship(9), at_ship(100)], "{ep}");
    }
}

/// An open map and every shot of `shots` run through one update (step 6 of a tick) in E2 with
/// `ships`; the headings the update latched.
fn update_once(shots: &mut Shots, ships: &mut Ships) -> [u8; SHOT_SLOTS] {
    let mut attr = [tables::TILE_SOLID; 256];
    attr[0] = tables::TILE_OPEN;
    let world = World {
        map: TileMap::parse_episode(&vec![0u8; 201 * 201]).unwrap(),
        tile_attr: attr,
        muzzles: Muzzles(vec![0; 342]),
    };
    let mut player = Player::new(5, START_VIEW);
    let mut explosions = Explosions::default();
    let mut rng = BpRandom::new();
    let mut events = Vec::new();
    let (mut kills, mut white, mut flash) = (0, 0, 0);
    shots.update(&mut Combat {
        world: &world,
        ships,
        player: &mut player,
        explosions: &mut explosions,
        rng: &mut rng,
        events: &mut events,
        kills: &mut kills,
        white_flash: &mut white,
        hit_flash: &mut flash,
        late_draw: false,
        flight: 20,
        difficulty: 1,
        episode: Episode::E2,
    })
}

#[test]
fn homing_missile_moves_along_the_heading_it_had_before_steering() {
    // weapons rev 6 §4.3 vector 11 (C-0207, E2 flight 20, BUG-11 / U-502): the missile
    // launched in tick 7 at tile (91, 33) on heading 7 toward slot 10 at (93, 34). Tick 8 moves
    // it along 7 to (90, 32) and stores heading 3, steered from (91, 33); steering first and
    // moving along 3 would give (92, 34).
    let mut ships = Ships::default();
    ships.0[9] = Ship {
        class: 7,
        tile_x: 93,
        tile_y: 34,
        sub_x: 16,
        sub_y: 16,
        heading: 6,
        armour: 50,
        ..Ship::EMPTY
    };
    let mut shots = Shots::default();
    shots.0[40] = Shot {
        tile_x: 91,
        tile_y: 33,
        off_x: 30,
        off_y: 10,
        heading: 7,
        kind: HOMING,
        life: 10,
        owner: 0,
    };
    let latched = update_once(&mut shots, &mut ships);
    let s = shots.0[40];
    assert_eq!((s.tile_x, s.tile_y, s.heading), (90, 32, 3), "after tick 8");
    assert_eq!(
        latched[40], 7,
        "drawn and moved with the heading of the tick's start"
    );
    // The next tick moves along 3 and steers from (90, 32): still down-right.
    let latched = update_once(&mut shots, &mut ships);
    let s = shots.0[40];
    assert_eq!((s.tile_x, s.tile_y, s.heading), (91, 33, 3));
    assert_eq!(latched[40], 3);
}

#[test]
fn a_shot_off_the_map_skips_steps_2_to_6() {
    // weapons rev 6 §4.3 step 1: freed, then straight to step 7. A cannon shot is not aged; a
    // homing missile does not steer (its heading array keeps the old value).
    let mut ships = Ships::default();
    ships.0[0] = Ship {
        class: 7,
        tile_x: 100,
        tile_y: 100,
        armour: 50,
        ..Ship::EMPTY
    };
    let mut shots = Shots::default();
    shots.0[10] = Shot {
        tile_x: 1,
        tile_y: 100,
        off_x: 16,
        off_y: 16,
        heading: 4,
        kind: 0,
        life: 3,
        owner: 2,
    };
    shots.0[41] = Shot {
        tile_x: 100,
        tile_y: 200,
        off_x: 16,
        off_y: 16,
        heading: 2,
        kind: HOMING,
        life: 1,
        owner: 0,
    };
    let latched = update_once(&mut shots, &mut ships);
    assert!(!shots.0[10].alive() && !shots.0[41].alive());
    assert_eq!(shots.0[10].life, 3, "no ageing");
    assert_eq!(shots.0[41].heading, 2, "no steering");
    assert_eq!((latched[10], latched[41]), (4, 2));
}
