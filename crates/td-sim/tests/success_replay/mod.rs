//! Shared harness of the L4 success-replay tests (`replays_e1.rs`, `replays_e2.rs`,
//! `replays_e3.rs`; MDG §10.2, ADG §10.5; VE-2 M9-21, VE-4 M10-45).
//!
//! Each `tests/replays/e<E>f<NN>.tdr` was planned by `tdtool plan NN --episode E` and flown in
//! the original by the oracle (`docs/parity/M9-playthrough.md`, `M10-playthrough.md`). The
//! test starts the flight as the first flight of a program run (`Session::for_episode`),
//! feeds the replay's input words through [`td_sim::replay::step`] (a modal screen is closed at
//! once with no input), checks every state-hash checkpoint, and checks that the flight ends
//! with success on the replay's last tick, not earlier. Needs `TD2192_DATA` (map, mission file
//! and the muzzle table of `file0001.exe`, byte-identical in every episode, episode-deltas
//! §11; E1 adds a direct start's two-digit-heading bytes built from `tdmen.pal`, weapons rev 4
//! §3.3, OB-017; E2 and E3 keep (0, 0) past the table, U-441). A replay that diverges after an intended behaviour change is re-recorded only after a
//! Verifier confirms the new behaviour against the original (ADG §10.5).

use std::path::{Path, PathBuf};

use td_assets::{DataDir, ExeResources, ResourceValue};
use td_formats::map::TileMap;
use td_formats::mission::Mission;
use td_sim::flight::front_end::{FrontEnd, muzzle_world};
use td_sim::flight::{Episode, Session, TickResult, episode};
use td_sim::replay::Replay;
use td_sim::{Flight, World, state_hash};

fn number(ep: Episode) -> u8 {
    match ep {
        Episode::E1 => 1,
        Episode::E2 => 2,
        Episode::E3 => 3,
    }
}

fn replay_path(ep: Episode, flight: u8, suffix: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/replays")
        .join(format!("e{}f{flight:02}{suffix}.tdr", number(ep)))
}

fn world(data: &Path, ep: Episode, flight: u8) -> (World, Option<Mission>) {
    let dir = DataDir::open(data).unwrap();
    let res = ExeResources::load(&dir).unwrap();
    let Some(ResourceValue::Bytes(muzzles)) = res.get("flight.muzzle_offsets").cloned() else {
        panic!("muzzle table");
    };
    let map = TileMap::parse_episode(&dir.read(episode::map_file(ep, flight)).unwrap()).unwrap();
    let mission = episode::mission_file(ep, flight)
        .map(|file| Mission::parse(&dir.read(&file).unwrap()).unwrap());
    let muzzles = if ep == Episode::E1 {
        let menu = dir.read("tdmen.pal").unwrap();
        muzzle_world(&muzzles, &menu, &FrontEnd::direct_start())
    } else {
        muzzles
    };
    (episode::world(ep, flight, map, muzzles), mission)
}

/// Runs the replay of `flight` of `ep` (skips without one): checkpoints, then success on the
/// last tick.
pub fn check(data: &Path, ep: Episode, flight: u8) {
    check_ending(data, ep, flight, "", false);
}

/// Runs the replay `e<E>f<NN><suffix>.tdr`: checkpoints, then the flight ends on the last
/// tick, by success, or by the player's death (`death`: the end sequence ran its 35 ticks).
pub fn check_ending(data: &Path, ep: Episode, flight: u8, suffix: &str, death: bool) {
    let path = replay_path(ep, flight, suffix);
    let Ok(bytes) = std::fs::read(&path) else {
        eprintln!("SKIPPED: no replay {}", path.display());
        return;
    };
    let r = Replay::from_bytes(&bytes).unwrap();
    assert_eq!(
        (r.episode, r.mission),
        (number(ep), flight),
        "replay header"
    );
    let session = Session::for_episode(ep);
    assert_eq!(r.seed, session.rng.seed(), "direct start");
    assert_eq!(
        r.checkpoints.len(),
        r.inputs.len() / r.interval as usize,
        "one checkpoint per interval"
    );
    let (world, mission) = world(data, ep, flight);
    let mut f = Flight::start(flight, mission.as_ref(), &world, session);
    let n = r.inputs.len();
    let mut last = TickResult::Done;
    let mut tick = 0;
    let checked = td_sim::replay::verify(&r, |w| {
        tick += 1;
        assert_ne!(
            last,
            TickResult::Finished,
            "{ep} flight {flight}: ended before tick {tick} of {n}"
        );
        last = td_sim::replay::step(&mut f, &world, w);
        state_hash(&f)
    });
    if let Err((t, want, got)) = checked {
        panic!(
            "{ep} flight {flight}: checkpoint at tick {t}: state hash {got:016X}, replay {want:016X}"
        );
    }
    assert_eq!(
        last,
        TickResult::Finished,
        "{ep} flight {flight}: still flying after tick {n}"
    );
    assert!(
        (f.succeeded, f.lost, f.died()) == (!death, false, death),
        "{ep} flight {flight}: succeeded {} lost {} died {}",
        f.succeeded,
        f.lost,
        f.died()
    );
    eprintln!(
        "{ep} flight {flight}: {n} ticks, {} checkpoints, kills {}, outcome {}",
        r.checkpoints.len(),
        f.kills,
        f.outcome()
    );
}
