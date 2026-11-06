//! L5 parity: Episode 1 flights 2..=20, flight setup and the first few hundred ticks, against
//! the VE-1 captures (M8-11 / M9-21). Needs TD2192_DATA and TD2192_ORACLE.
//!
//! **Captures.** `FILE0001.EXE` started directly in a private rig copy with a prepared
//! `tdstatus.dat` (status 1, kills 0, mission N, menu cursor 3, script 1) and the shipped
//! `td.cfg` (2, 2, 1): Enter at the main menu, Esc at the mission card, then flight N with no
//! input. The whole DGROUP (segment 4467h when started directly) is dumped every frame. A
//! capture is found in the oracle root by the first line of its `scenario.txt`
//! (`# ve1-e1f<N>-firstticks`, C-0017..C-0035). The `comm` captures of flights 2, 6, 9 and 10
//! (C-0037..C-0040) add F6 after the transmission is announced, Space for the prompts and F6 to
//! close the screen; the test feeds the original's key bytes to every tick.
//!
//! **Session.** A direct start is the first flight of the program run: seed 0 at program start,
//! 20 `Random(2)` at program start and 20 at flight start (mission-ships §4.1). The test counts
//! the `Random` steps from seed 0 to the captured seed and builds the session as
//! `Session::new()` plus any steps the original made before the flight that we do not
//! (reported in the coverage line; none expected). Class tables, ship records, the AI's player
//! tile and the mission globals are the program-start ones, as in the original.
//!
//! **Sampling.** In these captures the first frame of tick 1 can already show tick 1's mission
//! ships (the fade-in ends in the frame before), so setup is compared with the last frame
//! before tick 1:`Msn_Setup` has run but the setup `AI_ChooseWaypoint` pass (mission-ships
//! §4.1 step 6) has not, so the record bytes that pass writes are left out there. The state
//! after tick t is the last frame whose tick counter (DS:1718) is t (mission-ships §6) or, when
//! that frame was taken before tick t's mission logic and ships had run (a tick shorter than a
//! frame, seen around tick 1), the first frame of tick t + 1; the coverage line counts those
//! (`late_frames`). Fields written after the tick limiter (tick-order steps 26–33: shield
//! regeneration, the player-destroyed test) are checked the same way, independently. The
//! capture's last tick has no next frame and is not compared.
//!
//! **Compared.** Setup: seed, flight, player class, palette mode, phase, ticker, home marker,
//! markers, every slot's arrays and ship record. Each tick: phase, comm pending, succeeded,
//! lost, kills, home marker, the 20 markers, ticker position and comm indicator; arrays and
//! record of every slot that is live in either; player armour, shields, view, heading,
//! mission over and end ticks. The ticker message itself is drawn into VGA memory, not
//! DGROUP; it is checked through the phase step that posts it and the ticker restart.
//!
//! **Known divergence left out of the per-flight setup check:** the shields and armour that
//! setup gives a slot whose mission-file class is FFh (U-143); see
//! `class_ffh_slots_setup_shields_and_armour`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use td_assets::DataDir;
use td_formats::map::TileMap;
use td_formats::mission::Mission;
use td_sim::flight::ships::{NO_SHIP, RECORD_LEN, SHIP_SLOTS, Ship};
use td_sim::flight::shots::Muzzles;
use td_sim::flight::tables::episode1_tile_attributes;
use td_sim::flight::{Modal, Session, TickResult, World};
use td_sim::{BpRandom, Flight, FlightKey, TickControls};
use td_test_support::{require_data, require_oracle};

/// DS of the tick counter `g_MissionTicks` (u32).
const TICKS: usize = 0x1718;
/// DS of the key table `g_Keys` (one byte per scan code; extended codes at +100h).
const KEYS: usize = 0x1A5C;
/// DS of `RandSeed`.
const SEED: usize = 0x04EE;
/// The muzzle table and the DGROUP bytes ships read past it on two-digit headings (U-160).
const MUZZLES: std::ops::Range<usize> = 0x035C..0x035C + 34 * 99 + 36;
/// Record bytes the setup `AI_ChooseWaypoint` pass writes in path mode (pathCursor, prevX/Y,
/// returnX/Y; mission-ships §4.3.5), plus moveDir, nearFlag and targetX/Y, which the pass may
/// write.
const STEP6_BYTES: [usize; 10] = [
    0x003, 0x004, 0x00B, 0x00C, 0x3F5, 0x414, 0x420, 0x421, 0x426, 0x427,
];
/// Record bytes of shields (+3F8h) and armour (+3FAh), i16 each.
const SHIELDS_ARMOUR: std::ops::Range<usize> = 0x3F8..0x3FC;

struct Dump {
    frame: u32,
    ds: Vec<u8>,
}

impl Dump {
    fn u8(&self, a: usize) -> u8 {
        self.ds[a]
    }
    fn i16(&self, a: usize) -> i16 {
        i16::from_le_bytes([self.ds[a], self.ds[a + 1]])
    }
    fn ticks(&self) -> u32 {
        u32::from_le_bytes(self.ds[TICKS..TICKS + 4].try_into().unwrap())
    }
    fn seed(&self) -> u32 {
        u32::from_le_bytes(self.ds[SEED..SEED + 4].try_into().unwrap())
    }
}

/// The capture made by the VE-1 scenario `ve1-e1f<flight>-<tag>`.
fn find_capture(oracle: &Path, flight: u8, tag: &str) -> Option<PathBuf> {
    let head = format!("# ve1-e1f{flight}-{tag} ");
    let mut dirs: Vec<PathBuf> = fs::read_dir(oracle)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    dirs.sort();
    dirs.into_iter().find(|dir| {
        fs::read_to_string(dir.join("scenario.txt"))
            .is_ok_and(|s| s.lines().next().is_some_and(|l| l.starts_with(&head)))
    })
}

/// The frames the test needs: the last frame before tick 1 (setup still running) and, for each
/// tick counter t ≥ 1, the first and the last frame.
struct Capture {
    setup: Dump,
    first: BTreeMap<u32, Dump>,
    last: BTreeMap<u32, Dump>,
}

/// Reads every DGROUP dump in `mem`, in parallel (opening thousands of files is the slow part).
fn load(mem: &Path) -> Capture {
    let mut names: Vec<(u32, PathBuf)> = fs::read_dir(mem)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.to_string_lossy().ends_with("_ds.bin"))
        .map(|p| {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            (name[1..9].parse().unwrap(), p)
        })
        .collect();
    names.sort();
    let chunk = names.len().div_ceil(16).max(1);
    let dumps: Vec<Dump> = std::thread::scope(|s| {
        let jobs: Vec<_> = names
            .chunks(chunk)
            .map(|part| {
                s.spawn(move || {
                    part.iter()
                        .map(|(frame, p)| Dump {
                            frame: *frame,
                            ds: fs::read(p).unwrap(),
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        jobs.into_iter().flat_map(|j| j.join().unwrap()).collect()
    });
    let mut setup = None;
    let mut first = BTreeMap::new();
    let mut last = BTreeMap::new();
    for d in dumps {
        let t = d.ticks();
        if t == 0 {
            if first.is_empty() {
                setup = Some(d);
            }
            continue;
        }
        first.entry(t).or_insert_with(|| Dump {
            frame: d.frame,
            ds: d.ds.clone(),
        });
        last.insert(t, d);
    }
    Capture {
        setup: setup.expect("a frame before tick 1"),
        first,
        last,
    }
}

/// The input of tick t: the original's key bytes as the first frame of tick t + 1 holds them
/// (the frame after tick t's input steps; `oracle_flight.rs`), as down and made.
fn controls(next: &Dump) -> TickControls {
    let mut c = TickControls::default();
    for k in FlightKey::ALL {
        if next.u8(KEYS + usize::from(k.scan_code())) != 0 {
            c.set_down(k);
            c.set_made(k);
        }
    }
    c
}

/// Number of `Random` steps from seed 0 to `seed` (at most `limit`).
fn steps_to(seed: u32, limit: u32) -> Option<u32> {
    let mut r = BpRandom::new();
    for n in 0..=limit {
        if r.seed() == seed {
            return Some(n);
        }
        r.step();
    }
    None
}

/// Slot `i` (0-based) as (slot arrays, record bytes), original side.
fn slot_orig(d: &Dump, i: usize) -> (Vec<i16>, Vec<u8>) {
    let arrays = vec![
        i16::from(d.u8(0x3A7A + i)),
        i16::from(d.u8(0x3B2E + i)),
        i16::from(d.u8(0x3B42 + i)),
        d.i16(0x3ACA + 2 * i),
        d.i16(0x3AF2 + 2 * i),
        i16::from(d.u8(0x3AA2 + i)),
        i16::from(d.u8(0x3AB6 + i)),
        i16::from(d.u8(0x3B1A + i)),
        i16::from(d.u8(0x3B6A + 4 * i)),
        i16::from(d.u8(0x3B6B + 4 * i)),
        i16::from(d.u8(0x3B6C + 4 * i)),
        i16::from(d.u8(0x3B6D + 4 * i)),
        i16::from(d.u8(0x3BBA + i)),
        i16::from(d.u8(0x3BCE + i)),
    ];
    let base = 0x4432 + i * RECORD_LEN;
    (arrays, d.ds[base..base + RECORD_LEN].to_vec())
}

fn slot_ours(s: &Ship) -> (Vec<i16>, Vec<u8>) {
    let arrays = vec![
        i16::from(s.class),
        i16::from(s.tile_x),
        i16::from(s.tile_y),
        s.sub_x,
        s.sub_y,
        i16::from(s.heading),
        i16::from(s.target_heading),
        i16::from(s.turn),
        i16::from(s.speed[0]),
        i16::from(s.speed[1]),
        i16::from(s.speed[2]),
        i16::from(s.speed[3]),
        i16::from(s.goal.0),
        i16::from(s.goal.1),
    ];
    (arrays, s.record_bytes())
}

const SLOT_FIELDS: [&str; 14] = [
    "class", "tile x", "tile y", "sub x", "sub y", "heading", "target", "turn", "up", "down",
    "left", "right", "goal x", "goal y",
];

/// How slot `i` differs, ignoring the record bytes in `skip`, or `None`.
fn slot_diff(f: &Flight, d: &Dump, i: usize, skip: &[usize]) -> Option<String> {
    let (oa, or) = slot_ours(&f.ships.0[i]);
    let (ta, tr) = slot_orig(d, i);
    let rec: Vec<usize> = (0..RECORD_LEN)
        .filter(|o| !skip.contains(o) && or[*o] != tr[*o])
        .collect();
    if oa == ta && rec.is_empty() {
        return None;
    }
    let mut out: Vec<String> = (0..oa.len())
        .filter(|&k| oa[k] != ta[k])
        .map(|k| format!("{} ours {} orig {}", SLOT_FIELDS[k], oa[k], ta[k]))
        .collect();
    out.extend(
        rec.iter()
            .take(8)
            .map(|&o| format!("record +{o:03X}h ours {:02X} orig {:02X}", or[o], tr[o])),
    );
    Some(format!("slot {}: {}", i + 1, out.join(", ")))
}

/// Named scalar fields, (name, ours, original).
type Fields = Vec<(String, i32, i32)>;

fn field(name: &str, ours: impl Into<i32>, orig: impl Into<i32>) -> (String, i32, i32) {
    (name.to_string(), ours.into(), orig.into())
}

/// Mission logic and the HUD, tick-order steps 6–19 (before the tick limiter).
fn logic_fields(f: &Flight, d: &Dump) -> Fields {
    let m = &f.mission;
    let mut v = vec![
        field("phase", m.phase, d.u8(0x32BA)),
        field("comm pending", u8::from(m.comm_pending), d.u8(0x329C)),
        field("succeeded", u8::from(f.succeeded), d.u8(0x329E)),
        field("lost", u8::from(f.lost), d.u8(0x12CC)),
        field("kills", f.kills, d.i16(0x12CA)),
        field("home marker", m.home_marker, d.u8(0x396C)),
        field("ticker", m.ticker.pos, d.i16(0x32BC) as u16),
        field("comm indicator", m.comm_phase, d.u8(0x1705)),
    ];
    for (i, s) in f.ships.0.iter().enumerate() {
        v.push(field(
            &format!("marker {}", i + 1),
            s.marker,
            d.u8(0x3958 + i),
        ));
    }
    v
}

/// Player state and the end sequence (partly written after the tick limiter).
fn player_fields(f: &Flight, d: &Dump) -> Fields {
    let p = &f.player;
    vec![
        field("armour", p.armour, d.u8(0x398E) as i8),
        field("shields", p.shields, d.u8(0x398F) as i8),
        field("view tile x", p.view.tile_x, d.i16(0x3A36)),
        field("view tile y", p.view.tile_y, d.i16(0x3A38)),
        field("view sub x", p.view.sub_x, d.i16(0x3A3A)),
        field("view sub y", p.view.sub_y, d.i16(0x3A3C)),
        field("heading", p.heading.code(), d.u8(0x1A56)),
        field("over", u8::from(f.mission_over), d.u8(0x32B7)),
        field("end ticks", f.end_ticks, d.u8(0x32B8)),
    ]
}

fn bad(fields: &Fields) -> Vec<String> {
    fields
        .iter()
        .filter(|(_, a, b)| a != b)
        .map(|(n, a, b)| format!("{n} ours {a} orig {b}"))
        .collect()
}

/// What changed in the original from `prev` to `d` (the state after tick `t`).
fn events(t: u32, prev: &Dump, d: &Dump) -> Vec<String> {
    let mut out = Vec::new();
    let named = [
        ("phase", 0x32BA),
        ("comm", 0x329C),
        ("succeeded", 0x329E),
        ("lost", 0x12CC),
        ("over", 0x32B7),
        ("kills", 0x12CA),
        ("player armour", 0x398E),
    ];
    for (n, a) in named {
        if prev.u8(a) != d.u8(a) {
            out.push(format!("t{t} {n} {}", d.u8(a)));
        }
    }
    let (p, q) = (prev.i16(0x32BC) as u16, d.i16(0x32BC) as u16);
    if q != 0 && (p == 0 || q < p) {
        out.push(format!("t{t} post"));
    }
    for i in 0..SHIP_SLOTS {
        let (a, b) = (prev.u8(0x3A7A + i), d.u8(0x3A7A + i));
        if a != b {
            out.push(format!("t{t} slot {} class {b:02X}", i + 1));
        }
    }
    out
}

/// What one flight's comparison covered.
#[derive(Debug, Default)]
struct Coverage {
    /// Ticks compared (1..=ticks).
    ticks: u32,
    /// Slot-ticks compared (slots live in ours or the original).
    slot_ticks: usize,
    /// Ticks whose state was found in the first frame of the next tick (short ticks).
    late_frames: Vec<u32>,
    /// What changed in the original during the compared ticks: phase, comm pending, ticker
    /// restarts (posts), slot classes (spawn, hide, removal), kills and the end flags.
    events: Vec<String>,
    /// `Random` steps from seed 0 to the seed after setup, and how many the test added.
    seed_steps: u32,
    extra_steps: u32,
}

struct Run {
    name: String,
    cap: Capture,
    world: World,
    mission: Mission,
}

fn open(flight: u8, tag: &str) -> Option<Run> {
    let data = td_test_support::data_dir()?;
    let oracle = td_test_support::oracle_dir()?;
    let Some(dir) = find_capture(&oracle, flight, tag) else {
        eprintln!("SKIPPED: no ve1-e1f{flight}-{tag} capture under the oracle root");
        return None;
    };
    let cap = load(&dir.join("mem"));
    let data = DataDir::open(&data).unwrap();
    let world = World {
        map: TileMap::parse_episode(&data.read("td.map").unwrap()).unwrap(),
        tile_attr: episode1_tile_attributes(),
        muzzles: Muzzles(cap.setup.ds[MUZZLES].to_vec()),
    };
    let mission = Mission::parse(&data.read(&format!("m{flight}.td")).unwrap()).unwrap();
    Some(Run {
        name: dir.file_name().unwrap().to_string_lossy().into_owned(),
        cap,
        world,
        mission,
    })
}

/// Flight `flight` as the first flight of a program run, matched to the capture's seed:
/// (flight, steps from seed 0, steps added).
fn start(flight: u8, run: &Run) -> (Flight, u32, u32) {
    let seed = run.cap.setup.seed();
    let n = steps_to(seed, 100_000).expect("seed after setup reachable from seed 0");
    let plain = Flight::start(flight, Some(&run.mission), &run.world, Session::new());
    let ours = steps_to(plain.rng.seed(), 100_000).unwrap();
    let extra = n.saturating_sub(ours);
    let mut session = Session::new();
    for _ in 0..extra {
        session.rng.step();
    }
    let f = Flight::start(flight, Some(&run.mission), &run.world, session);
    (f, n, extra)
}

/// Compares flight `flight` with its capture: setup, then tick by tick up to the first
/// divergence. `Err` holds the first divergence.
fn compare(flight: u8, run: &Run) -> Result<Coverage, String> {
    let (mut f, seed_steps, extra_steps) = start(flight, run);
    let setup = &run.cap.setup;
    let name = &run.name;
    let mut cov = Coverage {
        seed_steps,
        extra_steps,
        ..Coverage::default()
    };

    // Setup.
    let mut errs = Vec::new();
    if f.rng.seed() != setup.seed() {
        errs.push(format!(
            "seed ours {:08X} orig {:08X} ({seed_steps} steps from 0)",
            f.rng.seed(),
            setup.seed()
        ));
    }
    let mut fields = logic_fields(&f, setup);
    fields.retain(|(n, _, _)| n != "kills");
    fields.extend([
        field("flight", f.number, setup.u8(0x32B9)),
        field("player class", f.player.class, setup.u8(0x1A57)),
        field("palette mode", f.palette_mode, setup.u8(0x1704)),
    ]);
    errs.extend(bad(&fields));
    for i in 0..SHIP_SLOTS {
        let mut skip = STEP6_BYTES.to_vec();
        if run.mission.slot_class[i] == NO_SHIP {
            skip.extend(SHIELDS_ARMOUR);
        }
        errs.extend(slot_diff(&f, setup, i, &skip));
    }
    if !errs.is_empty() {
        return Err(format!(
            "{name} flight {flight} after setup (frame {}):\n{}",
            setup.frame,
            errs.join("\n")
        ));
    }

    // The state after tick t is in the last frame of tick t or, when that frame was taken before
    // tick t's mission logic and ships had run (a short tick), in the first frame of tick t + 1.
    // The capture's last tick has no next frame and is not compared.
    let last_tick = *run.cap.last.keys().last().unwrap();
    let mut prev = setup;
    for t in 1..last_tick {
        let (Some(d), Some(next)) = (run.cap.last.get(&t), run.cap.first.get(&(t + 1))) else {
            return Err(format!("{name}: no frame of tick {t} or of tick {}", t + 1));
        };
        let mut r = f.tick(&run.world, &controls(next), &mut vec![]);
        while let TickResult::Modal(m) = r {
            if m != Modal::CommViewer {
                return Err(format!("{name}: tick {t} opened {m:?}"));
            }
            cov.events
                .push(format!("t{t} transmission {}", f.mission.transmission));
            r = f.resume(&run.world, &TickControls::default(), &mut vec![]);
        }
        let live: Vec<usize> = (0..SHIP_SLOTS)
            .filter(|&i| f.ships.0[i].alive() || d.u8(0x3A7A + i) != NO_SHIP)
            .collect();
        let logic = |d: &Dump| -> Vec<String> {
            let mut errs = bad(&logic_fields(&f, d));
            errs.extend(live.iter().filter_map(|&i| slot_diff(&f, d, i, &[])));
            errs
        };
        let mut errs = logic(d);
        let mut seen = d;
        if !errs.is_empty() && logic(next).is_empty() {
            errs.clear();
            seen = next;
            cov.late_frames.push(t);
        }
        let player = bad(&player_fields(&f, d));
        if !player.is_empty() && !bad(&player_fields(&f, next)).is_empty() {
            errs.extend(player);
        }
        if !errs.is_empty() {
            return Err(format!(
                "{name} flight {flight}: first divergence at tick {t} (frames {} and {}), {cov:?}:\n{}",
                d.frame,
                next.frame,
                errs.join("\n")
            ));
        }
        cov.ticks = t;
        cov.slot_ticks += live.len();
        cov.events.extend(events(t, prev, seen));
        prev = seen;
        if r != TickResult::Done {
            break;
        }
    }
    eprintln!("{name} flight {flight}: {cov:?}");
    Ok(cov)
}

fn check(flight: u8) {
    check_capture(flight, "firstticks");
}

fn check_capture(flight: u8, tag: &str) {
    let _data = require_data!();
    let _oracle = require_oracle!();
    let Some(run) = open(flight, tag) else { return };
    let cov = compare(flight, &run).unwrap_or_else(|e| panic!("{e}"));
    // mission-ships §4.1: 20 Random(2) at program start and 20 at flight start, nothing else
    // before the first tick of a directly started flight.
    assert_eq!(
        (cov.seed_steps, cov.extra_steps),
        (40, 0),
        "Random steps before tick 1 (captured, added by the test)"
    );
}

/// U-143 (mission-ships §4.1 step 3, §5): a slot whose mission-file class is FFh gets the
/// shields and armour that the class tables hold past their end (DS:03D1, 2 × DS:03C7; both
/// bytes are 22h). Flight 2 has eight such slots (8, 10–13, 15–17).
#[test]
fn class_ffh_slots_setup_shields_and_armour() {
    let _data = require_data!();
    let _oracle = require_oracle!();
    let Some(run) = open(2, "firstticks") else {
        return;
    };
    let (f, _, _) = start(2, &run);
    let d = &run.cap.setup;
    let mut errs = Vec::new();
    for i in (0..SHIP_SLOTS).filter(|&i| run.mission.slot_class[i] == NO_SHIP) {
        let s = &f.ships.0[i];
        let base = 0x4432 + i * RECORD_LEN;
        let orig = (d.i16(base + 0x3F8), d.i16(base + 0x3FA));
        if (s.shields, s.armour) != orig {
            errs.push(format!(
                "slot {}: (shields, armour) ours {:?} orig {orig:?}",
                i + 1,
                (s.shields, s.armour)
            ));
        }
    }
    assert!(errs.is_empty(), "{}", errs.join("\n"));
}

/// Flights 2, 6, 9 and 10 with F6 pressed after the transmission is announced (prompts with
/// Space, closed with F6): the comm viewer's phase advance and the ticks after it.
#[test]
fn flight_2_comm_matches_oracle() {
    check_capture(2, "comm");
}

#[test]
fn flight_6_comm_matches_oracle() {
    check_capture(6, "comm");
}

#[test]
fn flight_9_comm_matches_oracle() {
    check_capture(9, "comm");
}

#[test]
fn flight_10_comm_matches_oracle() {
    check_capture(10, "comm");
}

#[test]
fn flight_2_matches_oracle() {
    check(2);
}

#[test]
fn flight_3_matches_oracle() {
    check(3);
}

#[test]
fn flight_4_matches_oracle() {
    check(4);
}

#[test]
fn flight_5_matches_oracle() {
    check(5);
}

#[test]
fn flight_6_matches_oracle() {
    check(6);
}

#[test]
fn flight_7_matches_oracle() {
    check(7);
}

#[test]
fn flight_8_matches_oracle() {
    check(8);
}

#[test]
fn flight_9_matches_oracle() {
    check(9);
}

#[test]
fn flight_10_matches_oracle() {
    check(10);
}

#[test]
fn flight_11_matches_oracle() {
    check(11);
}

#[test]
fn flight_12_matches_oracle() {
    check(12);
}

#[test]
fn flight_13_matches_oracle() {
    check(13);
}

#[test]
fn flight_14_matches_oracle() {
    check(14);
}

#[test]
fn flight_15_matches_oracle() {
    check(15);
}

#[test]
fn flight_16_matches_oracle() {
    check(16);
}

#[test]
fn flight_17_matches_oracle() {
    check(17);
}

#[test]
fn flight_18_matches_oracle() {
    check(18);
}

#[test]
fn flight_19_matches_oracle() {
    check(19);
}

#[test]
fn flight_20_matches_oracle() {
    check(20);
}
