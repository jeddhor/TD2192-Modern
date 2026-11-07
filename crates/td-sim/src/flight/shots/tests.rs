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
    update_drawn(shots, ships).latched
}

/// [`update_once`], returning what the update leaves for the drawing.
fn update_drawn(shots: &mut Shots, ships: &mut Ships) -> Drawn {
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

#[test]
fn a_shot_is_drawn_in_the_tick_that_frees_it_for_age() {
    // flight-screens rev 7 §4.8 (BUG-21, C-0321 tick 118): step 6 draws the shot, step 7 then
    // frees it because its life is above 25; a shot freed earlier (map test) is not drawn.
    let mut ships = Ships::default();
    let mut shots = Shots::default();
    let shot = Shot {
        tile_x: 100,
        tile_y: 100,
        off_x: 16,
        off_y: 16,
        heading: 8,
        kind: 1,
        life: MAX_LIFE,
        owner: 3,
    };
    shots.0[40] = shot;
    shots.0[39] = Shot { life: 4, ..shot };
    let drawn = update_drawn(&mut shots, &mut ships);
    assert!(!shots.0[40].alive(), "freed for age in step 7");
    assert_eq!(
        drawn.shown.0[40],
        Shot {
            life: MAX_LIFE + 1,
            ..shot
        },
        "drawn at its tile before the move"
    );
    // The younger shot: drawn at (100, 100), then moved to (100, 99).
    assert_eq!(
        (drawn.shown.0[39].tile_x, drawn.shown.0[39].tile_y),
        (100, 100)
    );
    assert_eq!((shots.0[39].tile_x, shots.0[39].tile_y), (100, 99));
    // Free slots and a shot off the map draw nothing.
    assert!(!drawn.shown.0[0].alive());
    let mut shots = Shots::default();
    shots.0[5] = Shot {
        tile_x: 1,
        life: 2,
        ..shot
    };
    assert!(!update_drawn(&mut shots, &mut ships).shown.0[5].alive());
}

// BUG-30: the 16-bit position rules after an edge wrap (mission-ships rev 8 §4.2.5 rows 4–8).

/// One shot update in E1 flight 1 (an open map, the player at the start view, a fresh
/// generator): the events it requested.
fn update_e1(ships: &mut Ships, shots: &mut Shots, explosions: &mut Explosions) -> Vec<Event> {
    let mut attr = [tables::TILE_SOLID; 256];
    attr[0] = tables::TILE_OPEN;
    let world = World {
        map: TileMap::parse_episode(&vec![0u8; 201 * 201]).unwrap(),
        tile_attr: attr,
        muzzles: Muzzles(vec![0; 342]),
    };
    let mut player = Player::new(4, START_VIEW);
    let mut rng = BpRandom::new();
    let mut events = Vec::new();
    let (mut kills, mut white, mut flash) = (0, 0, 0);
    shots.update(&mut Combat {
        world: &world,
        ships,
        player: &mut player,
        explosions,
        rng: &mut rng,
        events: &mut events,
        kills: &mut kills,
        white_flash: &mut white,
        hit_flash: &mut flash,
        late_draw: false,
        flight: 1,
        difficulty: 1,
        episode: Episode::E1,
    });
    events
}

/// A class-7 ship in slot 3 parked on row 199 of column 100 with offset Y `sub_y` (as C-0340's
/// slot 3 before and after its wrap), armour 200.
fn parked(sub_y: i16) -> Ships {
    let mut ships = Ships::default();
    ships.0[2] = Ship {
        class: 7,
        tile_x: 100,
        tile_y: 199,
        sub_x: 16,
        sub_y,
        heading: 2,
        armour: 200,
        ..Ship::EMPTY
    };
    ships
}

/// A player shot of `kind` at world pixel (100 × 32 + 16, `y`) in projectile slot 45, on
/// heading 0 (no move and no map test).
fn shot_at_y(kind: u8, y: i16) -> Shots {
    let mut shots = Shots::default();
    shots.0[45] = Shot {
        tile_x: 100,
        tile_y: (y / 32) as u8,
        off_x: 16,
        off_y: y % 32,
        heading: 0,
        kind,
        life: 1,
        owner: 0,
    };
    shots
}

#[test]
fn abs16_leaves_minus_32768_negative() {
    assert_eq!(abs16(-32_768), -32_768);
    assert_eq!(abs16(-32_767), 32_767);
    assert_eq!(abs16(5), 5);
    // A true difference of 32,768 reads as −32,768 in 16 bits and passes `< 20`; 32,767 and
    // −32,769 (32,767 in 16 bits) do not.
    assert!(near16((-32_768, 0), (0, 0), 20));
    assert!(near16((0, 0), (0, -32_768), 20));
    assert!(!near16((32_767, 0), (0, 0), 20));
    assert!(!near16((0, -32_767), (0, 0), 20));
    assert_eq!(pos16(199, 30_000), -29_168, "199 × 32 + 30,000 wraps");
    assert_eq!(pos16(1, -32_760), -32_728);
}

#[test]
fn shot_hits_a_wrapped_ship_when_the_true_difference_is_32768() {
    // mission-ships rev 8 §4.2.5 rows 4–6 and the note under them: a ship on row 199 with
    // offset Y 30,000 has the 16-bit world Y 6,368 + 30,000 − 65,536; a shot at world Y
    // 30,000 − 26,400 = 3,600 differs by exactly −32,768 and hits (step 4, 20 px). In 32-bit
    // arithmetic the difference is 32,768: a miss.
    let mut ships = parked(30_000);
    let mut shots = shot_at_y(0, 3_600);
    let events = update_e1(&mut ships, &mut shots, &mut Explosions::default());
    assert!(!shots.0[45].alive(), "hit: the shot is freed");
    assert_eq!(ships.0[2].armour, 195, "kind 0 deals 5");
    assert_eq!(
        events,
        [Event::Sound {
            kind: 9,
            source: Some((100, 199))
        }]
    );
    // One pixel further down the 16-bit difference is 32,767: a miss.
    let mut ships = parked(30_000);
    let mut shots = shot_at_y(0, 3_601);
    update_e1(&mut ships, &mut shots, &mut Explosions::default());
    assert!(shots.0[45].alive(), "miss");
    assert_eq!(ships.0[2].armour, 200);
}

#[test]
fn homing_missile_destroys_a_wrapped_ship_at_the_32768_difference() {
    // Row 4: step 3 (40 px) with the same 16-bit rule. The missile's target is slot 3.
    let mut ships = parked(30_000);
    let mut shots = shot_at_y(HOMING, 3_600);
    shots.0[45].life = 3;
    update_e1(&mut ships, &mut shots, &mut Explosions::default());
    assert!(!shots.0[45].alive());
    assert_eq!((ships.0[2].armour, shots.0[3].kind), (0, 1));
    // abs16 passes only −32,768 itself: 30 px further it is 32,738 in 16 bits.
    let mut ships = parked(30_000);
    let mut shots = shot_at_y(HOMING, 3_630);
    shots.0[45].life = 3;
    update_e1(&mut ships, &mut shots, &mut Explosions::default());
    assert!(shots.0[45].alive());
    assert_eq!(ships.0[2].armour, 200);
}

#[test]
fn hit_explosion_of_a_wrapped_ship_keeps_the_low_byte() {
    // Row 7: `offset + Random(16)` must not overflow; only the low byte is stored. Offset Y
    // 32,767: the shot at the −32,768 difference, 6,368 + 32,767 − 65,536 + 32,768 = 6,367.
    let mut ships = parked(32_767);
    let mut shots = shot_at_y(0, 6_367);
    let mut explosions = Explosions::default();
    update_e1(&mut ships, &mut shots, &mut explosions);
    assert!(!shots.0[45].alive(), "hit");
    let mut rng = BpRandom::new();
    let (rx, ry) = (rng.random(16) as i16, rng.random(16) as i16);
    let e = explosions.0[0];
    assert_eq!((e.frame, e.tile_x, e.tile_y), (1, 100, 199));
    assert_eq!(e.sub_x, (16 + rx) as u8);
    assert_eq!(e.sub_y, 32_767i16.wrapping_add(ry) as u8);
}

#[test]
fn enemy_shot_with_a_wrapped_offset_hits_the_player_by_the_16_bit_rule() {
    // Row 6: the player's reference at the start view is (107 × 32 + 32, 107 × 32 + 16) =
    // (3,456, 3,440). A shot of slot 3 at tile (108, 107) with offset (0, −32,752) has the
    // 16-bit world Y 3,424 − 32,752 = −29,328: the difference 3,440 + 29,328 = 32,768 reads as
    // −32,768 and passes.
    assert_eq!(player_ref(&Player::new(4, START_VIEW)), (3_456, 3_440));
    let mut ships = Ships::default();
    let mut shots = Shots::default();
    shots.0[10] = Shot {
        tile_x: 108,
        tile_y: 107,
        off_x: 0,
        off_y: -32_752,
        heading: 0,
        kind: 1,
        life: 1,
        owner: 3,
    };
    let events = update_e1(&mut ships, &mut shots, &mut Explosions::default());
    assert!(!shots.0[10].alive(), "the player is hit");
    assert_eq!(
        events,
        [Event::Sound {
            kind: 9,
            source: None
        }]
    );
}

#[test]
fn a_wrapped_ship_fires_without_overflow() {
    // Row 8: `dx + offset − 16` wraps (it panicked before BUG-30's fix). Class 7 fires
    // muzzles 12 and 13 (kind 1) into slots 40 and 39; every muzzle of heading 2 is
    // (dx, dy) = (5, 127) here and of heading 8 (5, −128).
    let mut table = vec![0u8; 342];
    for m in 1..=17 {
        table[34 * 2 + 2 * m] = 5;
        table[34 * 2 + 2 * m + 1] = 127;
        table[34 * 8 + 2 * m] = 5;
        table[34 * 8 + 2 * m + 1] = 0x80;
    }
    let muzzles = Muzzles(table);
    // 127 + 32,760 − 16 = 32,871 → −32,665; −128 − 32,760 − 16 = −32,904 → 32,632.
    for (heading, sub_y, want) in [(2, 32_760, -32_665), (8, -32_760, 32_632)] {
        let mut ships = parked(sub_y);
        ships.0[2].heading = heading;
        ships.0[2].fire[1] = true;
        let mut shots = Shots::default();
        let mut events = Vec::new();
        shots.ship_volleys(&ships, &muzzles, Episode::E1, &mut events);
        for slot in [40, 39] {
            let s = shots.0[slot];
            assert_eq!((s.tile_x, s.tile_y, s.owner, s.kind), (100, 199, 3, 1));
            assert_eq!(
                (s.off_x, s.off_y),
                (5, want),
                "heading {heading}, slot {slot}"
            );
        }
        assert_eq!(events.len(), 2);
    }
}
