//! L5 parity: a shot is drawn in the tick that frees it for age (flight-screens rev 7 §4.8,
//! BUG-21). C-0321 (RE-19) flies E2 flight 20 with `e2f20-weapons.tdr` (C-0207's run) and dumps
//! every frame of ticks 115..122: in tick 117 the homing launch writes life 200 into projectile
//! slot 40, an enemy shot three tiles right of the player (OB-143); tick 118 draws it in step 6
//! and frees it in step 7, and its page (frames 1870..1872) shows the enemy shots of slots 39
//! and 40 side by side, at rows 86 and 108. Needs TD2192_DATA and TD2192_ORACLE; skips
//! otherwise. No captured pixels are stored here.

use std::fs;
use std::path::Path;

use td_assets::{DataDir, ExeResources, ResourceValue};
use td_formats::map::TileMap;
use td_formats::mission::Mission;
use td_render::flight::{self, PLAYFIELD, ShotLook, ViewPos};
use td_sim::flight::front_end::direct_start_world;
use td_sim::flight::{Episode, Session, TickResult, episode};
use td_sim::replay::{self, Replay};
use td_sim::{Flight, World};
use td_test_support::{require_data, require_oracle};

/// E2 flight 20's world as a direct start leaves it (the success-replay harness's).
fn world(data: &Path) -> (World, Option<Mission>) {
    let (ep, n) = (Episode::E2, 20);
    let dir = DataDir::open(data).unwrap();
    let res = ExeResources::load(&dir).unwrap();
    let Some(ResourceValue::Bytes(table)) = res.get("flight.muzzle_offsets").cloned() else {
        panic!("muzzle table");
    };
    let map = TileMap::parse_episode(&dir.read(episode::map_file(ep, n)).unwrap()).unwrap();
    let mission =
        episode::mission_file(ep, n).map(|f| Mission::parse(&dir.read(&f).unwrap()).unwrap());
    let muzzles = direct_start_world(ep, n, &table, |f| dir.read(f)).unwrap();
    (episode::world(ep, n, map, muzzles), mission)
}

#[test]
fn a_shot_freed_for_age_is_drawn_in_its_last_tick() {
    let data = require_data!();
    let oracle = require_oracle!();
    let replay = Replay::from_bytes(
        &fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/replays/e2f20-weapons.tdr"),
        )
        .unwrap(),
    )
    .unwrap();
    let (world, mission) = world(&data);
    let mut f = Flight::start(
        20,
        mission.as_ref(),
        &world,
        Session::for_episode(Episode::E2),
    );
    for &word in &replay.inputs[..118] {
        assert_eq!(replay::step(&mut f, &world, word), TickResult::Done);
    }
    assert_eq!(f.ticks, 118);

    // Slot 40 (port index 40) is freed in step 7 (life above 25): the arrays after step 6 no
    // longer hold it (a later volley of the tick reuses the slot), but step 6 drew it at its
    // tile before the move, row 37 + 31, next to slot 39's shot at row 37 + 53.
    let shown = &f.drawn.shown_shots.0;
    assert!(!f.drawn.shots.0[40].alive(), "freed in step 7");
    assert!(shown[39].alive() && shown[40].alive());
    assert!(shown[40].life > 25 && shown[40].owner != 0);
    assert_eq!(
        [
            (shown[39].tile_y, shown[39].off_y),
            (shown[40].tile_y, shown[40].off_y)
        ],
        [(37, 53), (37, 31)]
    );
    let v = f.frame.view;
    let view = ViewPos {
        tile_x: i32::from(v.tile_x),
        tile_y: i32::from(v.tile_y),
        sub_x: i32::from(v.sub_x),
        sub_y: i32::from(v.sub_y),
    };
    let mut rows = Vec::new();
    for slot in [39, 40] {
        let s = shown[slot];
        let Some(ShotLook::Pixels(pixels)) = flight::shot_look(
            view,
            s.kind,
            s.heading,
            (s.tile_x, s.tile_y),
            (s.off_x, s.off_y),
        ) else {
            panic!("slot {slot} {s:?} is not drawn");
        };
        let on_screen: Vec<_> = pixels
            .into_iter()
            .filter(|&(x, y, _)| (0..PLAYFIELD.w).contains(&x) && (0..PLAYFIELD.h).contains(&y))
            .collect();
        assert!(!on_screen.is_empty(), "slot {slot} on screen");
        rows.push(on_screen.iter().map(|p| p.1).min().unwrap());
        for frame in 1870..=1872 {
            let name = format!("f{frame:08}_320x192_life.vga");
            let raw = fs::read(oracle.join("C-0321").join("fb").join(&name)).unwrap();
            let px = &raw[768..];
            for &(x, y, c) in &on_screen {
                assert_eq!(
                    px[(y * 320 + x) as usize],
                    c,
                    "C-0321 f{frame}: slot {slot} pixel ({x}, {y})"
                );
            }
        }
    }
    eprintln!("slots 39 and 40 drawn from rows {rows:?}");
}
