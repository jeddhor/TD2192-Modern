//! L4 success replays of Episode 1 (MDG §10.2, ADG §10.5; VE-2, M9-01..M9-17, M9-21).
//!
//! Each `tests/replays/e1fNN.tdr` was planned by `tdtool plan NN` and flown in the original by
//! the oracle (`docs/parity/M9-playthrough.md`). The test starts the flight as the first flight
//! of a program run (`Session::new()`), feeds the replay's input words through
//! [`td_sim::replay::step`] (a modal screen is closed at once with no input), checks every
//! state-hash checkpoint, and checks that the flight ends with success on the replay's last
//! tick, not earlier. Needs `TD2192_DATA` (map, mission file, the muzzle table of
//! `file0001.exe` and `tdmen.pal`, from which the direct start's two-digit-heading bytes are
//! built, weapons rev 4 §3.3). A replay that diverges after an intended behaviour change is re-recorded
//! only after a Verifier confirms the new behaviour against the original (ADG §10.5).

use std::path::{Path, PathBuf};

use td_assets::{DataDir, ExeResources, ResourceValue};
use td_formats::map::TileMap;
use td_formats::mission::Mission;
use td_sim::flight::front_end::{FrontEnd, muzzle_world};
use td_sim::flight::{Episode, Session, TickResult, episode};
use td_sim::replay::Replay;
use td_sim::{Flight, World, state_hash};
use td_test_support::require_data;

fn replay_path(flight: u8) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/replays")
        .join(format!("e1f{flight:02}.tdr"))
}

fn world(data: &Path, flight: u8) -> (World, Mission) {
    let dir = DataDir::open(data).unwrap();
    let res = ExeResources::load(&dir).unwrap();
    let Some(ResourceValue::Bytes(muzzles)) = res.get("flight.muzzle_offsets").cloned() else {
        panic!("muzzle table");
    };
    let map =
        TileMap::parse_episode(&dir.read(episode::map_file(Episode::E1, flight)).unwrap()).unwrap();
    let mission = Mission::parse(&dir.read(&format!("m{flight}.td")).unwrap()).unwrap();
    // A direct start's two-digit-heading bytes (weapons rev 4 §3.3, OB-017 preserved).
    let menu = dir.read("tdmen.pal").unwrap();
    let muzzles = muzzle_world(&muzzles, &menu, &FrontEnd::direct_start());
    (episode::world(Episode::E1, flight, map, muzzles), mission)
}

/// Runs the replay of `flight`: checkpoints, then success on the last tick.
fn check(flight: u8) {
    let data = require_data!();
    let path = replay_path(flight);
    let Ok(bytes) = std::fs::read(&path) else {
        eprintln!("SKIPPED: no replay {}", path.display());
        return;
    };
    let r = Replay::from_bytes(&bytes).unwrap();
    assert_eq!((r.episode, r.mission), (1, flight), "replay header");
    assert_eq!(r.seed, Session::new().rng.seed(), "direct start");
    assert_eq!(
        r.checkpoints.len(),
        r.inputs.len() / r.interval as usize,
        "one checkpoint per interval"
    );
    let (world, mission) = world(&data, flight);
    let mut f = Flight::start(flight, Some(&mission), &world, Session::new());
    let n = r.inputs.len();
    let mut last = TickResult::Done;
    let mut tick = 0;
    let checked = td_sim::replay::verify(&r, |w| {
        tick += 1;
        assert_ne!(
            last,
            TickResult::Finished,
            "flight {flight}: ended before tick {tick} of {n}"
        );
        last = td_sim::replay::step(&mut f, &world, w);
        state_hash(&f)
    });
    if let Err((t, want, got)) = checked {
        panic!(
            "flight {flight}: checkpoint at tick {t}: state hash {got:016X}, replay {want:016X}"
        );
    }
    assert_eq!(
        last,
        TickResult::Finished,
        "flight {flight}: still flying after tick {n}"
    );
    assert!(
        f.succeeded && !f.lost && !f.died(),
        "flight {flight}: succeeded {} lost {} died {}",
        f.succeeded,
        f.lost,
        f.died()
    );
    eprintln!(
        "flight {flight}: {n} ticks, {} checkpoints, kills {}, outcome {}",
        r.checkpoints.len(),
        f.kills,
        f.outcome()
    );
}

#[test]
fn flight_01_success_replay() {
    check(1);
}

#[test]
fn flight_02_success_replay() {
    check(2);
}

#[test]
fn flight_03_success_replay() {
    check(3);
}

#[test]
fn flight_04_success_replay() {
    check(4);
}

#[test]
fn flight_05_success_replay() {
    check(5);
}

#[test]
fn flight_06_success_replay() {
    check(6);
}

#[test]
fn flight_07_success_replay() {
    check(7);
}

#[test]
fn flight_08_success_replay() {
    check(8);
}

#[test]
fn flight_09_success_replay() {
    check(9);
}

#[test]
fn flight_10_success_replay() {
    check(10);
}

#[test]
fn flight_11_success_replay() {
    check(11);
}

#[test]
fn flight_12_success_replay() {
    check(12);
}

#[test]
fn flight_13_success_replay() {
    check(13);
}

#[test]
fn flight_14_success_replay() {
    check(14);
}

#[test]
fn flight_15_success_replay() {
    check(15);
}

#[test]
fn flight_16_success_replay() {
    check(16);
}

#[test]
fn flight_17_success_replay() {
    check(17);
}

#[test]
fn flight_18_success_replay() {
    check(18);
}

#[test]
fn flight_19_success_replay() {
    check(19);
}

#[test]
fn flight_20_success_replay() {
    check(20);
}
