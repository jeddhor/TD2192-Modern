//! M12-06 soak (MDG §11 "zero panics in 10 h of replay soak", ADG §10.6): seeded random input
//! through whole flights, and the committed replays on many threads at once.
//!
//! **Random flights.** A seeded generator holds and releases the flight keys the way a player
//! might (steering and fire often, afterburner, infra-red, Tab, M, S, Esc, P, F5 and F6 now and
//! then, the joystick bits and the cheat-chord letters rarely; a held key sends typematic make
//! codes). Each flight starts as its replays start it: the first flight of a program run, or,
//! for E1 flight 21 and E3 flights 7 and 21, after the predecessor's committed success replay
//! (VE-5). It runs through [`td_sim::replay::step`] (modal screens closed at once) until it
//! finishes or reaches its tick limit. Checked on every tick: no panic (td-sim has overflow
//! checks in every profile, MDG §8.6), the tick count advances by one, nothing runs after
//! `Finished`; at intervals, a clone of the flight steps to the same state hash as the flight
//! itself. Each job runs twice, on different threads and in a different order, and both runs
//! must end with the same tick count, outcome and state hash.
//!
//! **Replays on many threads.** Every committed `tests/replays/*.tdr` is flown on several
//! threads at once; every checkpoint must match and every thread must end with the same state.
//!
//! The default tests are cheap (a synthetic world without data; with `TD2192_DATA`, three
//! flights per episode). `long_soak` is ignored: every flight of every episode with
//! `TD2192_SOAK_SEEDS` seeds (default 8) of up to `TD2192_SOAK_TICKS` ticks (default 30,000,
//! 26 minutes of game time), base seed `TD2192_SOAK_SEED` (default 1), and the replays
//! `TD2192_SOAK_ROUNDS` times (default 4):
//!
//! ```text
//! TD2192_SOAK_SEEDS=64 cargo test -p td-sim --release --test soak -- --ignored --nocapture
//! ```

use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;

use td_assets::{DataDir, ExeResources, ResourceValue};
use td_formats::map::TileMap;
use td_formats::mission::Mission;
use td_sim::flight::front_end::direct_start_world;
use td_sim::flight::tables::episode1_tile_attributes;
use td_sim::flight::{Episode, Session, TickResult, episode};
use td_sim::replay::Replay;
use td_sim::{ChordKey, Flight, FlightKey, Joystick, TickControls, World, state_hash};
use td_test_support::data_dir;

/// splitmix64, seeded: every run reproduces.
#[derive(Debug, Clone)]
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// True with probability 1/n.
    fn one_in(&mut self, n: u64) -> bool {
        self.next().is_multiple_of(n)
    }
}

/// How often a key changes state (1 in n ticks) by kind of key.
fn toggle_odds(k: FlightKey) -> u64 {
    use FlightKey::*;
    match k {
        Kp8 | Up | Kp2 | Down | Kp4 | Left | Kp6 | Right => 6,
        Space => 4,
        KpPlus | Kp0 => 40,
        Alt | RightAlt => 80,
        Tab | M | S => 120,
        Esc | P | F5 | F6 => 200,
    }
}

/// A player-like random input stream.
#[derive(Debug, Clone)]
struct Inputs {
    rng: Rng,
    down: Vec<bool>,
    chord: Vec<bool>,
    joy: Joystick,
}

impl Inputs {
    fn new(seed: u64) -> Self {
        Self {
            rng: Rng(seed),
            down: vec![false; FlightKey::ALL.len()],
            chord: vec![false; ChordKey::ALL.len()],
            joy: Joystick::default(),
        }
    }

    fn next_word(&mut self) -> u128 {
        let mut c = TickControls::default();
        for (i, &k) in FlightKey::ALL.iter().enumerate() {
            let was = self.down[i];
            if self.rng.one_in(toggle_odds(k)) {
                self.down[i] = !was;
            }
            if self.down[i] {
                c.set_down(k);
                // A new press, or now and then a typematic repeat.
                if !was || self.rng.one_in(3) {
                    c.set_made(k);
                }
            } else if self.rng.one_in(500) {
                // Pressed and released within the tick.
                c.set_made(k);
            }
        }
        for (i, &k) in ChordKey::ALL.iter().enumerate() {
            let was = self.chord[i];
            if self.rng.one_in(300) {
                self.chord[i] = !was;
            }
            if self.chord[i] {
                c.set_chord_down(k);
                if !was {
                    c.set_chord_made(k);
                }
            }
        }
        if self.rng.one_in(30) {
            let axis = |r: u64| [0i8, 1, -1][(r % 3) as usize];
            self.joy = Joystick {
                x: axis(self.rng.next()),
                y: axis(self.rng.next()),
                button1: self.rng.one_in(4),
                button2: self.rng.one_in(3),
            };
        }
        c.set_joystick(self.joy);
        c.to_word()
    }
}

/// A flight ready to start.
#[derive(Debug, Clone)]
struct Setup {
    episode: Episode,
    number: u8,
    world: World,
    mission: Option<Mission>,
    session: Session,
}

impl Setup {
    fn start(&self) -> Flight {
        Flight::start(
            self.number,
            self.mission.as_ref(),
            &self.world,
            self.session.clone(),
        )
    }
}

fn ep_number(ep: Episode) -> u8 {
    match ep {
        Episode::E1 => 1,
        Episode::E2 => 2,
        Episode::E3 => 3,
    }
}

/// Every flight of `ep` (E3 has no flight 20).
fn flights(ep: Episode) -> Vec<u8> {
    match ep {
        Episode::E1 => (1..=21).collect(),
        Episode::E2 => (1..=20).collect(),
        Episode::E3 => (1..=21).filter(|&n| n != 20).collect(),
    }
}

/// The flight before `n` in its block, for the flights no block starts with (VE-5).
fn predecessor(ep: Episode, n: u8) -> Option<u8> {
    match (ep, n) {
        (Episode::E1, 21) => Some(19),
        (Episode::E3, 7) => Some(6),
        (Episode::E3, 21) => Some(11),
        _ => None,
    }
}

fn replays_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/replays")
}

/// Flight `n` of `ep` as its replays start it (see the module doc).
fn load(data: &Path, ep: Episode, n: u8) -> Setup {
    let dir = DataDir::open(data).unwrap();
    let res = ExeResources::load(&dir).unwrap();
    let Some(ResourceValue::Bytes(table)) = res.get("flight.muzzle_offsets").cloned() else {
        panic!("muzzle table");
    };
    let map = TileMap::parse_episode(&dir.read(episode::map_file(ep, n)).unwrap()).unwrap();
    let mission =
        episode::mission_file(ep, n).map(|file| Mission::parse(&dir.read(&file).unwrap()).unwrap());
    let muzzles = direct_start_world(ep, n, &table, |f| dir.read(f)).unwrap();
    let world = episode::world(ep, n, map, muzzles);
    let session = match predecessor(ep, n) {
        None => Session::for_episode(ep),
        Some(p) => {
            let prev = load(data, ep, p);
            let path = replays_dir().join(format!("e{}f{p:02}.tdr", ep_number(ep)));
            let r = Replay::from_bytes(&std::fs::read(&path).unwrap()).unwrap();
            let mut f = prev.start();
            for &w in &r.inputs {
                td_sim::replay::step(&mut f, &prev.world, w);
            }
            f.session()
        }
    };
    Setup {
        episode: ep,
        number: n,
        world,
        mission,
        session,
    }
}

/// How a random flight ended.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Ending {
    ticks: u32,
    finished: bool,
    succeeded: bool,
    lost: bool,
    died: bool,
    kills: i16,
    hash: u64,
    /// The tick that panicked, and the message.
    panic: Option<(u32, String)>,
}

/// Ticks between clone checks.
const CLONE_EVERY: u32 = 500;

/// Flies `setup` with the random inputs of `seed` for at most `max_ticks` ticks.
fn fly(setup: &Setup, seed: u64, max_ticks: u32) -> Ending {
    let mut f = setup.start();
    let mut inputs = Inputs::new(seed);
    let mut finished = false;
    let mut panic = None;
    let mut t = 0;
    while t < max_ticks {
        let w = inputs.next_word();
        let before = f.ticks;
        let check = (t + 1).is_multiple_of(CLONE_EVERY).then(|| f.clone());
        let step = catch_unwind(AssertUnwindSafe(|| {
            td_sim::replay::step(&mut f, &setup.world, w)
        }));
        t += 1;
        let r = match step {
            Ok(r) => r,
            Err(e) => {
                let msg = e
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| e.downcast_ref::<&str>().map(|s| (*s).to_owned()))
                    .unwrap_or_default();
                panic = Some((t, msg));
                break;
            }
        };
        assert_eq!(
            f.ticks,
            before.wrapping_add(1),
            "{} flight {} seed {seed:#x}: tick count after tick {t}",
            setup.episode,
            setup.number
        );
        if let Some(mut c) = check {
            td_sim::replay::step(&mut c, &setup.world, w);
            assert_eq!(
                state_hash(&c),
                state_hash(&f),
                "{} flight {} seed {seed:#x}: a clone diverged at tick {t}",
                setup.episode,
                setup.number
            );
        }
        if r == TickResult::Finished {
            finished = true;
            break;
        }
    }
    Ending {
        ticks: t,
        finished,
        succeeded: f.succeeded,
        lost: f.lost,
        died: f.died(),
        kills: f.kills,
        hash: state_hash(&f),
        panic,
    }
}

/// The jobs that panicked, one line each.
fn panics(endings: &BTreeMap<Job, Ending>) -> Vec<String> {
    endings
        .iter()
        .filter_map(|(j, e)| {
            e.panic.as_ref().map(|(t, msg)| {
                format!(
                    "e{}f{:02} seed {:#018x}: tick {t}: {msg}",
                    j.episode, j.flight, j.seed
                )
            })
        })
        .collect()
}

/// Fails with every panicked job.
fn assert_no_panics(endings: &BTreeMap<Job, Ending>) {
    let p = panics(endings);
    assert!(
        p.is_empty(),
        "{} flights panicked:
{}",
        p.len(),
        p.join(
            "
"
        )
    );
}

/// One random flight to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Job {
    episode: u8,
    flight: u8,
    seed: u64,
}

/// Runs every job on `threads` threads, twice, in two different orders; asserts that both runs
/// of a job end the same and returns the endings.
fn run_twice(
    setups: &BTreeMap<(u8, u8), Setup>,
    jobs: &[Job],
    max_ticks: u32,
    threads: usize,
) -> BTreeMap<Job, Ending> {
    let mut runs = Vec::new();
    for pass in 0..2 {
        let mut order: Vec<Job> = jobs.to_vec();
        if pass == 1 {
            order.reverse();
        }
        let queue = Mutex::new(order);
        let out = Mutex::new(BTreeMap::new());
        std::thread::scope(|s| {
            for _ in 0..threads {
                s.spawn(|| {
                    loop {
                        let Some(job) = queue.lock().unwrap().pop() else {
                            break;
                        };
                        let setup = &setups[&(job.episode, job.flight)];
                        let e = fly(setup, job.seed, max_ticks);
                        out.lock().unwrap().insert(job, e);
                    }
                });
            }
        });
        runs.push(out.into_inner().unwrap());
    }
    let (a, b) = (&runs[0], &runs[1]);
    for (job, ea) in a {
        assert_eq!(Some(ea), b.get(job), "{job:?}: the two runs differ");
    }
    runs.swap_remove(0)
}

fn threads() -> usize {
    std::thread::available_parallelism().map_or(4, |n| n.get().clamp(2, 16))
}

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn report(endings: &BTreeMap<Job, Ending>, started: Instant) {
    let ticks: u64 = endings.values().map(|e| u64::from(e.ticks)).sum();
    let count = |f: fn(&Ending) -> bool| endings.values().filter(|e| f(e)).count();
    eprintln!(
        "{} random flights, {ticks} ticks ({:.1} h of game time at 19.2015 Hz) twice each in \
         {:.1} s: {} finished ({} succeeded, {} lost, {} died), {} at the tick limit",
        endings.len(),
        ticks as f64 / 19.2015 / 3600.0,
        started.elapsed().as_secs_f64(),
        count(|e| e.finished),
        count(|e| e.succeeded),
        count(|e| e.lost),
        count(|e| e.died),
        count(|e| !e.finished && e.panic.is_none()),
    );
    let p = panics(endings);
    if !p.is_empty() {
        eprintln!("{} panicked (the same way in both runs)", p.len());
    }
}

/// No data: random input on a synthetic world (random tiles, Episode 1 attributes, no mission)
/// for flight 1 of each episode's session.
#[test]
fn random_input_on_a_synthetic_world() {
    let started = Instant::now();
    let mut rng = Rng(0x5EED);
    let cells: Vec<u8> = (0..201 * 201).map(|_| (rng.next() % 256) as u8).collect();
    let world = World {
        map: TileMap::parse_episode(&cells).unwrap(),
        tile_attr: episode1_tile_attributes(),
        muzzles: td_sim::flight::shots::Muzzles(vec![0; 342]),
    };
    let mut setups = BTreeMap::new();
    for ep in [Episode::E1, Episode::E2, Episode::E3] {
        setups.insert(
            (ep_number(ep), 1),
            Setup {
                episode: ep,
                number: 1,
                world: world.clone(),
                mission: None,
                session: Session::for_episode(ep),
            },
        );
    }
    let jobs: Vec<Job> = setups
        .keys()
        .flat_map(|&(episode, flight)| {
            (0..2).map(move |s| Job {
                episode,
                flight,
                seed: s,
            })
        })
        .collect();
    let endings = run_twice(&setups, &jobs, 1500, 3);
    report(&endings, started);
    assert_no_panics(&endings);
}

/// With data: three flights of each episode (chained ones included: E1 21, E3 7), two seeds
/// each, up to 6,000 ticks (5.2 minutes of game time).
#[test]
fn random_input_through_sample_flights() {
    let Some(data) = data_dir() else {
        eprintln!("SKIPPED: TD2192_DATA not set (data-gated test soak)");
        return;
    };
    let started = Instant::now();
    let sample: [(Episode, [u8; 3]); 3] = [
        (Episode::E1, [1, 8, 21]),
        (Episode::E2, [3, 10, 17]),
        (Episode::E3, [5, 7, 16]),
    ];
    let mut setups = BTreeMap::new();
    let mut jobs = Vec::new();
    for (ep, ns) in sample {
        for n in ns {
            setups.insert((ep_number(ep), n), load(&data, ep, n));
            for s in 0..2 {
                jobs.push(Job {
                    episode: ep_number(ep),
                    flight: n,
                    seed: u64::from(n) * 7919 + u64::from(ep_number(ep)) + (s << 20),
                });
            }
        }
    }
    let endings = run_twice(&setups, &jobs, 6000, threads());
    report(&endings, started);
    assert_no_panics(&endings);
}

/// Replays whose checkpoints were computed in a capture's muzzle world, not in a direct
/// start's (RE-17's U-430 runs, `oracle_e3_full::hangar`): their checkpoints are not compared
/// with a direct start here, only between threads.
const CAPTURE_WORLD: [&str; 2] = ["e3f10-hangar-b.tdr", "e3f10-hangar-c.tdr"];

/// The state hash after every `interval` ticks and at the end.
fn timeline(setup: &Setup, r: &Replay) -> Vec<u64> {
    let mut f = setup.start();
    let mut out = Vec::new();
    for (i, &w) in r.inputs.iter().enumerate() {
        td_sim::replay::step(&mut f, &setup.world, w);
        if r.interval > 0 && (i as u32 + 1).is_multiple_of(r.interval) {
            out.push(state_hash(&f));
        }
    }
    out.push(state_hash(&f));
    out
}

/// Every committed replay on `threads` threads at once, `rounds` times: every run's state
/// hashes (at the checkpoints and at the end) equal the single-threaded run's, which equal the
/// file's checkpoints. Returns the number of flights flown.
fn replays_on_threads(data: &Path, rounds: usize, threads: usize) -> usize {
    let mut files: Vec<PathBuf> = std::fs::read_dir(replays_dir())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "tdr"))
        .collect();
    files.sort();
    let mut prepared = Vec::new();
    for path in &files {
        let r = Replay::from_bytes(&std::fs::read(path).unwrap()).unwrap();
        let ep = match r.episode {
            1 => Episode::E1,
            2 => Episode::E2,
            _ => Episode::E3,
        };
        let setup = load(data, ep, r.mission);
        let reference = timeline(&setup, &r);
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if !CAPTURE_WORLD.contains(&name.as_str()) {
            assert_eq!(
                reference[..reference.len() - 1],
                r.checkpoints[..],
                "{name}: checkpoints"
            );
        }
        prepared.push((name, setup, r, reference));
    }
    let flown = Mutex::new(0usize);
    for _ in 0..rounds {
        std::thread::scope(|s| {
            for _ in 0..threads {
                s.spawn(|| {
                    for (name, setup, r, reference) in &prepared {
                        assert_eq!(
                            &timeline(setup, r),
                            reference,
                            "{name}: a thread's run differs from the reference run"
                        );
                        *flown.lock().unwrap() += 1;
                    }
                });
            }
        });
    }
    flown.into_inner().unwrap()
}

/// With data: every committed replay on four threads at once.
#[test]
fn replays_agree_across_threads() {
    let Some(data) = data_dir() else {
        eprintln!("SKIPPED: TD2192_DATA not set (data-gated test soak)");
        return;
    };
    let started = Instant::now();
    let n = replays_on_threads(&data, 1, 4);
    eprintln!(
        "{n} replay runs on 4 threads agree ({:.1} s)",
        started.elapsed().as_secs_f64()
    );
}

#[test]
#[ignore = "long soak; run with --release -- --ignored --nocapture"]
fn long_soak() {
    let Some(data) = data_dir() else {
        eprintln!("SKIPPED: TD2192_DATA not set (data-gated test soak)");
        return;
    };
    let seeds = env_u64("TD2192_SOAK_SEEDS", 8);
    let max_ticks = u32::try_from(env_u64("TD2192_SOAK_TICKS", 30_000)).unwrap_or(u32::MAX);
    let base = env_u64("TD2192_SOAK_SEED", 1);
    let rounds = usize::try_from(env_u64("TD2192_SOAK_ROUNDS", 4)).unwrap_or(4);
    let threads = threads();
    let started = Instant::now();
    let mut setups = BTreeMap::new();
    let mut jobs = Vec::new();
    for ep in [Episode::E1, Episode::E2, Episode::E3] {
        for n in flights(ep) {
            setups.insert((ep_number(ep), n), load(&data, ep, n));
            for s in 0..seeds {
                let mut mix = Rng(base ^ (u64::from(ep_number(ep)) << 40) ^ (u64::from(n) << 32));
                jobs.push(Job {
                    episode: ep_number(ep),
                    flight: n,
                    seed: mix.next() ^ s,
                });
            }
        }
    }
    eprintln!(
        "long soak: {} flights x {seeds} seeds, up to {max_ticks} ticks, base seed {base}, \
         {threads} threads",
        setups.len()
    );
    let endings = run_twice(&setups, &jobs, max_ticks, threads);
    for (ep, n) in setups.keys() {
        let mine: Vec<&Ending> = endings
            .iter()
            .filter(|(j, _)| (j.episode, j.flight) == (*ep, *n))
            .map(|(_, e)| e)
            .collect();
        let ticks: u64 = mine.iter().map(|e| u64::from(e.ticks)).sum();
        eprintln!(
            "  e{ep}f{n:02}: {ticks:>8} ticks; finished {}/{} (succeeded {}, lost {}, died {}), \
             kills up to {}, panics {}",
            mine.iter().filter(|e| e.finished).count(),
            mine.len(),
            mine.iter().filter(|e| e.succeeded).count(),
            mine.iter().filter(|e| e.lost).count(),
            mine.iter().filter(|e| e.died).count(),
            mine.iter().map(|e| e.kills).max().unwrap_or(0),
            mine.iter().filter(|e| e.panic.is_some()).count(),
        );
    }
    report(&endings, started);
    let t = Instant::now();
    let n = replays_on_threads(&data, rounds, threads);
    eprintln!(
        "{n} replay runs on {threads} threads in {rounds} rounds agree ({:.1} s)",
        t.elapsed().as_secs_f64()
    );
    for line in panics(&endings) {
        eprintln!("PANIC {line}");
    }
    assert_no_panics(&endings);
}

/// BUG-18: in E3 flight 19, with no input, slot 2 (class 0) parks at tile (102, 199) with down
/// speed 23..24 and a goal of (102, 200); its offset Y grows by about 8.5 a tick (mission-ships
/// §4.2.3: "Y never wraps at row 199 (offsetY grows)") and overflows i16 in tick 4235, which
/// panics in every build (td-sim keeps overflow checks in release, MDG §8.6): the game crashes
/// 3 minutes 40 seconds into the flight. Ignored until the overflow follows the original's
/// 16-bit arithmetic (U-570); then this runs to tick 6000.
#[test]
#[ignore = "BUG-18: i16 overflow in ship drift at row 199 (E3 flight 19, tick 4235)"]
fn e3_flight_19_idle_outlives_tick_4235() {
    let Some(data) = data_dir() else {
        eprintln!("SKIPPED: TD2192_DATA not set (data-gated test soak)");
        return;
    };
    let setup = load(&data, Episode::E3, 19);
    let mut f = setup.start();
    for _ in 0..6000 {
        if td_sim::replay::step(&mut f, &setup.world, 0) == TickResult::Finished {
            break;
        }
    }
}

/// BUG-18, the top edge: with these random inputs, a class-7 ship of E1 flight 6 (slot 19) and
/// of E1 flight 7 (slot 13) keeps flying up at tile row 1 toward a goal on row 0; the edge rule
/// removes it, the mission logic brings the class back, and its offset Y falls by about 6 a
/// tick until `sub_y += 16 - up` overflows i16 (ticks 5528 and 8831). Ignored with the bottom
/// edge case above (U-570).
#[test]
#[ignore = "BUG-18: i16 overflow in ship drift at row 1 (E1 flights 6 and 7, random inputs)"]
fn e1_flights_6_and_7_outlive_the_top_edge_overflow() {
    let Some(data) = data_dir() else {
        eprintln!("SKIPPED: TD2192_DATA not set (data-gated test soak)");
        return;
    };
    for (n, seed, tick) in [
        (6, 0xc4c2_e355_fd51_c5c0, 5528),
        (7, 0x0e8c_bfa7_1515_28ff, 8831),
    ] {
        let setup = load(&data, Episode::E1, n);
        let e = fly(&setup, seed, tick + 100);
        assert_eq!(e.panic, None, "E1 flight {n}");
    }
}
