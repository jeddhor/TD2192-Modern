//! The effects the simulation requests (`docs/spec/audio/sfx-policy.md` rev 2 §3.2, U-083):
//! the end sequence's blasts, the volleys (once per shot spawned; id 2 while the player is
//! drawn late) and the incoming-transmission chime (200).

use super::*;
use crate::input::FlightKey;

/// A 201 × 201 world where every tile is open.
fn open_world() -> World {
    let mut attr = [tables::TILE_SOLID; 256];
    attr[0] = tables::TILE_OPEN;
    World {
        map: TileMap::parse_episode(&vec![0u8; 201 * 201]).unwrap(),
        tile_attr: attr,
        muzzles: shots::Muzzles(vec![0; 342]),
    }
}

fn pressed(key: FlightKey) -> TickControls {
    let mut c = TickControls::default();
    c.set_down(key);
    c.set_made(key);
    c
}

#[test]
fn end_sequence_requests_blasts_and_the_big_explosion() {
    // sfx-policy rev 2 §3.2 (`0E8B:5719`, `578D`, `5908`, `596A`, `59CF`): 9 at end ticks 3,
    // 6, 9, 100 at 12 (after the distance flash), 9 at 17, 19, 25; all with no position.
    let w = open_world();
    for (episode, number) in [(Episode::E1, 1), (Episode::E2, 1), (Episode::E3, 1)] {
        let mut f = Flight::new(number);
        f.episode = episode;
        f.player.armour = 0;
        let mut ev = vec![];
        let mut seen = vec![];
        loop {
            let at = f.end_ticks;
            ev.clear();
            let r = f.tick(&w, &TickControls::default(), &mut ev);
            for &Event::Sound { kind, source } in &ev {
                assert_eq!(source, None, "end tick {at}");
                seen.push((at, kind));
            }
            if r == TickResult::Finished {
                break;
            }
        }
        assert_eq!(
            seen,
            [(3, 9), (6, 9), (9, 9), (12, 100), (17, 9), (19, 9), (25, 9)],
            "{episode}"
        );
    }
    assert_eq!(end_sequence_sound(0), None);
    assert_eq!(end_sequence_sound(35), None);
}

/// The sounds of one volley tick and the number of shots it spawned.
fn volley(f: &mut Flight) -> (Vec<Event>, usize) {
    let w = open_world();
    let before = f.shots.0.iter().filter(|s| s.alive()).count();
    let mut ev = vec![];
    f.tick(&w, &pressed(FlightKey::Space), &mut ev);
    let after = f.shots.0.iter().filter(|s| s.alive()).count();
    (ev, after - before)
}

#[test]
fn player_volley_requests_its_class_once_per_shot() {
    // sfx-policy §3.2 `0E8B:B6D1`: the player's class, once per shot spawned, no position.
    for number in [1, 6] {
        let mut f = Flight::new(number);
        let class = f.player.class;
        let (ev, shots) = volley(&mut f);
        assert!(shots >= 2, "flight {number}: {shots} shots");
        let want = vec![
            Event::Sound {
                kind: class,
                source: None
            };
            shots
        ];
        assert_eq!(ev, want, "flight {number}");
    }
}

#[test]
fn helicopter_volley_requests_id_2() {
    // sfx-policy §3.2 `0E8B:B6EA`: while the player is drawn late (helicopter flights 12, 15,
    // 21; class 3) the volley requests `stiletto.voc` (id 2) instead of the class sound.
    for number in [12, 15, 21] {
        let mut f = Flight::new(number);
        f.late_draw = true;
        assert_eq!(f.player.class, 3, "flight {number}");
        let (ev, shots) = volley(&mut f);
        assert!(shots >= 1, "flight {number}");
        let want = vec![
            Event::Sound {
                kind: 2,
                source: None
            };
            shots
        ];
        assert_eq!(ev, want, "flight {number}");
    }
}

#[test]
fn mission_ship_volley_requests_its_class_at_its_tile_once_per_shot() {
    // sfx-policy §3.2 `0E8B:F413`: the shooter's class at its tile, once per shot spawned.
    let w = open_world();
    let mut f = Flight::new(1);
    f.ships.0[3] = ships::Ship {
        class: 7,
        tile_x: 120,
        tile_y: 90,
        sub_x: 16,
        sub_y: 16,
        heading: 2,
        armour: 50,
        ..ships::Ship::EMPTY
    };
    // Volleys spawn when the back page is 1 at tick-order step 31, after the toggle of step
    // 24: the first tick of a flight. The request (direction 2) survives the AI pass of step
    // 10 and is consumed at step 31.
    let mut ev = vec![];
    let before = f.shots.0.iter().filter(|s| s.alive()).count();
    f.ships.0[3].fire[1] = true;
    f.tick(&w, &TickControls::default(), &mut ev);
    assert_eq!(f.draw_page, 1);
    let after = f.shots.0.iter().filter(|s| s.alive()).count();
    let n = after - before;
    assert!(n >= 1, "class 7 fires");
    let tile = (f.ships.0[3].tile_x, f.ships.0[3].tile_y);
    let volley: Vec<&Event> = ev
        .iter()
        .filter(|e| matches!(e, Event::Sound { kind: 7, .. }))
        .collect();
    assert_eq!(volley.len(), n);
    for e in volley {
        assert_eq!(
            *e,
            Event::Sound {
                kind: 7,
                source: Some(tile)
            }
        );
    }
}

#[test]
fn incoming_transmission_chime_once_when_the_lamp_starts() {
    // sfx-policy §3.2 `0E8B:8073`, hud §4.8: 200 (`income.voc`, no position) when a
    // transmission is pending and the indicator phase is 0; the lamp then cycles 1..7 without
    // requesting it again.
    let w = open_world();
    let mut f = Flight::new(1);
    f.mission.comm_pending = true;
    f.mission.comm_phase = 0;
    let mut chimes = 0;
    for _ in 0..20 {
        let mut ev = vec![];
        f.tick(&w, &TickControls::default(), &mut ev);
        chimes += ev
            .iter()
            .filter(|e| {
                **e == Event::Sound {
                    kind: logic::SOUND_INCOMING,
                    source: None,
                }
            })
            .count();
        assert!(f.mission.comm_phase >= 1, "the lamp runs");
    }
    assert_eq!(logic::SOUND_INCOMING, 200);
    assert_eq!(chimes, 1);
}
