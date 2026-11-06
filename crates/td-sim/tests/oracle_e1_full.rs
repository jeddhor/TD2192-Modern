//! L5 parity: whole Episode 1 flights, every tick from setup to the end of the flight, against
//! the original flown with the same inputs (VE-2, M9-21; `docs/parity/M9-playthrough.md`).
//! Needs TD2192_DATA and TD2192_ORACLE.
//!
//! **Inputs.** The committed success replay `tests/replays/e1fNN.tdr` (planned by
//! `tdtool plan`). The capture flew its tick-keyed schedule (`tdtool replay --oracle-schedule`,
//! oracle build 2, ORACLE.md §5a): the transitions of port tick t are injected within 1 ms after
//! the game's tick counter `g_MissionTicks` (DS:1718) becomes t, during tick t's steps 1–4, so
//! every input step of tick t (steps 23–33; tick-order rev 1) sees them and none of tick t − 1
//! does. While a comm viewer is open the schedule presses Space and F6 only while the counter
//! still shows the tick that opened it, so no key reaches a flight tick; the port closes the
//! screen at once with no input ([`td_sim::replay::step`]).
//!
//! **Capture.** `FILE0001.EXE` started directly in a private rig copy (VE-1's method:
//! `tdstatus.dat` = status 1, kills 0, mission N, cursor 3, script 1; DGROUP 4467h). The oracle
//! dumps DGROUP and the projectile heap arrays (5C4E:0000) once per tick, within 1 ms after the
//! counter changes: the record of value t + 1 holds the state after tick t, plus tick t + 1's
//! steps 1–3 (the counter and the turn step; drawing the tile window, step 4, takes longer
//! than the poll interval). A stall record 150 ms after the last change holds the state after
//! the last tick, and a DGROUP dump 240 frames after it holds the outcome (teardown runs after
//! the fade-out). The capture is found by the first line of its `scenario.txt`
//! (`# ve2-e1f<N>-plan`).
//!
//! **Compared, every tick.** The `Random` seed; tick counter; draw page; phase, comm pending,
//! succeeded, lost, kills, home marker, the 20 markers, ticker, comm indicator, hit flash; the
//! player's view, speeds, heading (or its next turn step), target heading, turn, armour,
//! shields, weapon, missiles, cooldown, mission over and end ticks; the key-table bytes of the
//! flight keys (this checks the tick rule itself); every slot's arrays and its 1,065-byte
//! record; the 51 projectile slots. **At the end:** the original's last tick is the replay's
//! last tick, and the outcome flag (DS:12CF) is the port's.

use std::fs;
use std::path::{Path, PathBuf};

use td_assets::DataDir;
use td_formats::map::TileMap;
use td_formats::mission::Mission;
use td_sim::flight::player::Turn;
use td_sim::flight::ships::{NO_SHIP, RECORD_LEN, SHIP_SLOTS, Ship};
use td_sim::flight::shots::{Muzzles, SHOT_SLOTS, Shot};
use td_sim::flight::{Episode, Session, TickResult, episode};
use td_sim::replay::Replay;
use td_sim::{Flight, FlightKey, World};
use td_test_support::{require_data, require_oracle};

const DS_LEN: usize = 0x9DF0;
const PROJ_LEN: usize = 0x240;
/// Linear address of the projectile heap dump (5C4E:0000 when FILE0001.EXE starts directly).
const PROJ_BASE: usize = 0x5C4E0;
/// The muzzle table and the DGROUP bytes ships read past it on two-digit headings (U-160).
const MUZZLES: std::ops::Range<usize> = 0x035C..0x035C + 34 * 99 + 36;
const STALL: u32 = 0x8000_0000;

/// One packed per-tick record (ORACLE.md §6.6a).
struct Rec {
    value: u32,
    frame: u32,
    bytes: Vec<u8>,
}

fn load_packed(path: &Path, len: usize) -> Vec<Rec> {
    let b = fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert_eq!(b.len() % (16 + len), 0, "{}: record size", path.display());
    b.chunks(16 + len)
        .map(|c| Rec {
            value: u32::from_le_bytes(c[0..4].try_into().unwrap()),
            frame: u32::from_le_bytes(c[4..8].try_into().unwrap()),
            bytes: c[16..].to_vec(),
        })
        .collect()
}

/// A tick's DGROUP and projectile arrays.
struct Dump<'a> {
    ds: &'a [u8],
    proj: &'a [u8],
}

impl Dump<'_> {
    fn u8(&self, a: usize) -> u8 {
        self.ds[a]
    }
    fn u16(&self, a: usize) -> u16 {
        u16::from_le_bytes([self.ds[a], self.ds[a + 1]])
    }
    fn i16(&self, a: usize) -> i16 {
        self.u16(a) as i16
    }
    fn u32(&self, a: usize) -> u32 {
        u32::from_le_bytes(self.ds[a..a + 4].try_into().unwrap())
    }
    fn proj_u8(&self, ptr: usize, i: usize) -> u8 {
        let lin = usize::from(self.u16(ptr + 2)) * 16 + usize::from(self.u16(ptr));
        self.proj[lin - PROJ_BASE + i]
    }
    fn proj_i16(&self, ptr: usize, i: usize) -> i16 {
        i16::from_le_bytes([self.proj_u8(ptr, 2 * i), self.proj_u8(ptr, 2 * i + 1)])
    }
    fn shot(&self, i: usize) -> Shot {
        Shot {
            tile_x: self.proj_u8(0x3986, i),
            tile_y: self.proj_u8(0x398A, i),
            off_x: self.proj_i16(0x3976, i),
            off_y: self.proj_i16(0x397A, i),
            heading: self.proj_u8(0x397E, i),
            kind: self.proj_u8(0x3982, i),
            life: self.proj_u8(0x3972, i),
            owner: self.proj_u8(0x396E, i),
        }
    }
}

fn find_capture(oracle: &Path, flight: u8) -> Option<PathBuf> {
    let head = format!("# ve2-e1f{flight}-plan ");
    let mut dirs: Vec<PathBuf> = fs::read_dir(oracle)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    dirs.sort();
    dirs.into_iter().rev().find(|dir| {
        fs::read_to_string(dir.join("scenario.txt"))
            .is_ok_and(|s| s.lines().next().is_some_and(|l| l.starts_with(&head)))
    })
}

fn replay_path(flight: u8) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/replays")
        .join(format!("e1f{flight:02}.tdr"))
}

type Fields = Vec<(String, i64, i64)>;

fn field(name: &str, ours: impl Into<i64>, orig: impl Into<i64>) -> (String, i64, i64) {
    (name.to_string(), ours.into(), orig.into())
}

/// Scalar state: (name, ours, original).
fn fields(f: &Flight, d: &Dump) -> Fields {
    let m = &f.mission;
    let p = &f.player;
    let mut v = vec![
        field("seed", f.rng.seed(), d.u32(0x04EE)),
        field("draw page", f.draw_page, d.u8(0x3A30)),
        field("phase", m.phase, d.u8(0x32BA)),
        field("comm pending", u8::from(m.comm_pending), d.u8(0x329C)),
        field("succeeded", u8::from(f.succeeded), d.u8(0x329E)),
        field("lost", u8::from(f.lost), d.u8(0x12CC)),
        field("kills", f.kills, d.i16(0x12CA)),
        field("home marker", m.home_marker, d.u8(0x396C)),
        field("ticker", m.ticker.pos, d.u16(0x32BC)),
        field("comm indicator", m.comm_phase, d.u8(0x1705)),
        field("hit flash", f.hit_flash, d.u16(0x12C4)),
        field("armour", p.armour, d.u8(0x398E) as i8),
        field("shields", p.shields, d.u8(0x398F) as i8),
        field("view tile x", p.view.tile_x, d.i16(0x3A36)),
        field("view tile y", p.view.tile_y, d.i16(0x3A38)),
        field("view sub x", p.view.sub_x, d.i16(0x3A3A)),
        field("view sub y", p.view.sub_y, d.i16(0x3A3C)),
        field("speed up", p.speed.up, d.u8(0x3995)),
        field("speed down", p.speed.down, d.u8(0x3996)),
        field("speed left", p.speed.left, d.u8(0x3997)),
        field("speed right", p.speed.right, d.u8(0x3998)),
        field("target heading", p.target.code(), d.u8(0x1CBB)),
        field(
            "turn",
            match p.turn {
                Turn::None => 0u8,
                Turn::CounterClockwise => 1,
                Turn::Clockwise => 2,
            },
            d.u8(0x1A5A),
        ),
        field("weapon", f.weapon, d.u8(0x1CB8)),
        field("missiles", f.missiles, d.u8(0x1CB9)),
        field("cooldown", f.missile_cooldown, d.u8(0x1CBA)),
        field("over", u8::from(f.mission_over), d.u8(0x32B7)),
        field("end ticks", f.end_ticks, d.u8(0x32B8)),
    ];
    for (i, s) in f.ships.0.iter().enumerate() {
        v.push(field(
            &format!("marker {}", i + 1),
            s.marker,
            d.u8(0x3958 + i),
        ));
    }
    for k in FlightKey::ALL {
        v.push(field(
            &format!("key {k:?}"),
            u8::from(f.keys.get(k)),
            u8::from(d.u8(0x1A5C + usize::from(k.scan_code())) != 0),
        ));
    }
    v
}

/// The heading may already include the next tick's turn step (tick-order step 3).
fn heading_ok(f: &Flight, d: &Dump) -> Option<String> {
    let p = &f.player;
    let next = match (p.target != p.heading, p.turn) {
        (true, Turn::Clockwise) => p.heading.cw(),
        (true, Turn::CounterClockwise) => p.heading.ccw(),
        _ => p.heading,
    };
    let o = d.u8(0x1A56);
    (o != p.heading.code() && o != next.code()).then(|| {
        format!(
            "heading ours {} (next {}) orig {o}",
            p.heading.code(),
            next.code()
        )
    })
}

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

fn slot_diff(f: &Flight, d: &Dump, i: usize) -> Option<String> {
    let (oa, or) = slot_ours(&f.ships.0[i]);
    let (ta, tr) = slot_orig(d, i);
    let rec: Vec<usize> = (0..RECORD_LEN).filter(|&o| or[o] != tr[o]).collect();
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

fn differences(f: &Flight, d: &Dump) -> Vec<String> {
    let mut errs: Vec<String> = fields(f, d)
        .iter()
        .filter(|(_, a, b)| a != b)
        .map(|(n, a, b)| format!("{n} ours {a} orig {b}"))
        .collect();
    errs.extend(heading_ok(f, d));
    for i in 0..SHIP_SLOTS {
        if f.ships.0[i].alive() || d.u8(0x3A7A + i) != NO_SHIP {
            errs.extend(slot_diff(f, d, i));
        }
    }
    for i in 0..SHOT_SLOTS {
        let (o, t) = (f.shots.0[i], d.shot(i));
        // A free slot keeps stale fields in the original; only tile X = FFh matters there.
        let same = if !o.alive() || !t.alive() {
            o.alive() == t.alive()
        } else {
            o == t
        };
        if !same {
            errs.push(format!("shot {i}: ours {o:?} orig {t:?}"));
        }
    }
    errs
}

/// What changed in the original from `a` to `b` (shown with a divergence).
fn events(t: u32, a: &Dump, b: &Dump) -> Vec<String> {
    let mut out = Vec::new();
    for (n, x) in [
        ("phase", 0x32BA),
        ("comm", 0x329C),
        ("succeeded", 0x329E),
        ("lost", 0x12CC),
        ("over", 0x32B7),
        ("kills", 0x12CA),
        ("player armour", 0x398E),
    ] {
        if a.u8(x) != b.u8(x) {
            out.push(format!("t{t} {n} {}", b.u8(x)));
        }
    }
    for i in 0..SHIP_SLOTS {
        if a.u8(0x3A7A + i) != b.u8(0x3A7A + i) {
            out.push(format!(
                "t{t} slot {} class {:02X}",
                i + 1,
                b.u8(0x3A7A + i)
            ));
        }
    }
    out
}

#[derive(Debug, Default)]
struct Coverage {
    ticks: u32,
    slot_ticks: usize,
    comm_viewers: Vec<u32>,
    events: Vec<String>,
}

fn check(flight: u8) {
    let data = require_data!();
    let oracle = require_oracle!();
    let Ok(bytes) = fs::read(replay_path(flight)) else {
        eprintln!("SKIPPED: no replay for flight {flight}");
        return;
    };
    let Some(cap) = find_capture(&oracle, flight) else {
        eprintln!("SKIPPED: no ve2-e1f{flight}-plan capture under the oracle root");
        return;
    };
    let name = cap.file_name().unwrap().to_string_lossy().into_owned();
    let r = Replay::from_bytes(&bytes).unwrap();
    let ds = load_packed(&cap.join("mem/tick_4467-0000_9DF0_ds.bin"), DS_LEN);
    let proj = load_packed(&cap.join("mem/tick_5C4E-0000_240_proj.bin"), PROJ_LEN);
    assert_eq!(ds.len(), proj.len(), "{name}: record counts");
    // Records by counter value (stall records apart).
    let mut by_tick = std::collections::BTreeMap::new();
    let mut stalls = std::collections::BTreeMap::new();
    for (i, rec) in ds.iter().enumerate() {
        assert_eq!(rec.value, proj[i].value, "{name}: record {i}");
        if rec.value & STALL == 0 {
            by_tick.insert(rec.value, i);
        } else {
            stalls.insert(rec.value & !STALL, i);
        }
    }
    let dump = |i: usize| Dump {
        ds: &ds[i].bytes,
        proj: &proj[i].bytes,
    };

    let dir = DataDir::open(&data).unwrap();
    let first = dump(*by_tick.get(&1).expect("a record of tick 1"));
    let world = World {
        map: TileMap::parse_episode(&dir.read(episode::map_file(Episode::E1, flight)).unwrap())
            .unwrap(),
        tile_attr: episode::tile_attributes(Episode::E1, flight),
        muzzles: Muzzles(first.ds[MUZZLES].to_vec()),
    };
    let mission = Mission::parse(&dir.read(&format!("m{flight}.td")).unwrap()).unwrap();
    let mut f = Flight::start(flight, Some(&mission), &world, Session::new());
    let mut known = vec![];
    // Known divergence BUG-2 (weapons §3.2: classes 0 and 1 keep the weapon of the previous
    // flight; at program start the original's g_Weapon is 0, td-sim's 1): copied from the
    // original so that the rest of flight 19 can be compared.
    if f.player.class <= 1 && f.weapon != first.u8(0x1CB8) {
        known.push(format!(
            "setup: weapon ours {} orig {} (BUG-2, copied)",
            f.weapon,
            first.u8(0x1CB8)
        ));
        f.weapon = first.u8(0x1CB8);
    }

    // Setup: the record of tick 1 holds the state after setup (plus tick 1's turn step).
    let mut errs = differences(&f, &first);
    errs.retain(|e| !e.starts_with("draw page"));
    assert!(
        errs.is_empty(),
        "{name} flight {flight} after setup (frame {}):\n{}",
        ds[by_tick[&1]].frame,
        errs.join("\n")
    );

    let n = r.inputs.len() as u32;
    let mut cov = Coverage::default();
    let mut prev = first;
    for t in 1..=n {
        let c = td_sim::TickControls::from_word(r.inputs[t as usize - 1]);
        let mut ev = vec![];
        let mut result = f.tick(&world, &c, &mut ev);
        while let TickResult::Modal(_) = result {
            cov.comm_viewers.push(t);
            // The host's part (hit-flash rev 2 §4.2): the screen's fades zero g_HitFlash,
            // as td2192 does when it resumes the tick.
            f.hit_flash = 0;
            result = f.resume(&world, &td_sim::TickControls::default(), &mut ev);
        }
        // The host's part for the real-time interlude of step 18 (mission-logic §5.17): the
        // flight-17 cut-scene fades out and back, which zeroes g_HitFlash (hit-flash §4.2).
        if f.mission.scene == 17 {
            f.hit_flash = 0;
        }
        // After the last tick: the stall record of that tick when the original stopped there
        // too, else the record of the tick it went on to fly.
        let idx = if t < n {
            by_tick.get(&(t + 1)).copied()
        } else {
            by_tick.get(&(t + 1)).or_else(|| stalls.get(&t)).copied()
        };
        let Some(i) = idx else {
            panic!(
                "{name} flight {flight}: the original has no state after tick {t} \
                 (records up to tick {:?}), {cov:?}",
                by_tick.keys().last()
            );
        };
        let d = dump(i);
        let errs = differences(&f, &d);
        if !errs.is_empty() {
            panic!(
                "{name} flight {flight}: first divergence after tick {t} (record frame {}), \
                 events so far {:?}, comm {:?}:\n{}",
                ds[i].frame,
                cov.events.iter().rev().take(12).collect::<Vec<_>>(),
                cov.comm_viewers,
                errs.join("\n")
            );
        }
        cov.events.extend(events(t, &prev, &d));
        cov.slot_ticks += (0..SHIP_SLOTS).filter(|&s| f.ships.0[s].alive()).count();
        cov.ticks = t;
        prev = d;
        assert_eq!(
            result == TickResult::Finished,
            t == n,
            "{name} flight {flight}: port flight finished at tick {t} of {n}"
        );
    }
    // The original flew no further tick, and its outcome is ours.
    let last = by_tick.keys().last().copied().unwrap();
    assert_eq!(last, n, "{name} flight {flight}: original's last tick");
    let end = fs::read_dir(cap.join("mem"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| {
            let n = p.file_name().unwrap().to_string_lossy();
            n.starts_with('f') && n.ends_with("_end240.bin")
        })
        .expect("the end240 dump");
    let end = fs::read(end).unwrap();
    assert_eq!(
        (end[0x12CF] != 0, end[0x329E] != 0, end[0x12CC] != 0),
        (f.outcome(), f.succeeded, f.lost),
        "{name} flight {flight}: (outcome, succeeded, lost) after teardown"
    );
    eprintln!(
        "{name} flight {flight}: PASS {} ticks, {} slot-ticks, kills {}, outcome {}, comm {:?}, \
         known divergences {known:?}, events {:?}",
        cov.ticks,
        cov.slot_ticks,
        f.kills,
        f.outcome(),
        cov.comm_viewers,
        cov.events
    );
}

#[test]
fn flight_01_full_matches_oracle() {
    check(1);
}

#[test]
fn flight_02_full_matches_oracle() {
    check(2);
}

#[test]
fn flight_03_full_matches_oracle() {
    check(3);
}

#[test]
fn flight_04_full_matches_oracle() {
    check(4);
}

#[test]
fn flight_05_full_matches_oracle() {
    check(5);
}

#[test]
fn flight_06_full_matches_oracle() {
    check(6);
}

#[test]
fn flight_07_full_matches_oracle() {
    check(7);
}

#[test]
fn flight_08_full_matches_oracle() {
    check(8);
}

#[test]
fn flight_09_full_matches_oracle() {
    check(9);
}

#[test]
fn flight_10_full_matches_oracle() {
    check(10);
}

#[test]
fn flight_11_full_matches_oracle() {
    check(11);
}

#[test]
fn flight_12_full_matches_oracle() {
    check(12);
}

#[test]
fn flight_13_full_matches_oracle() {
    check(13);
}

#[test]
fn flight_14_full_matches_oracle() {
    check(14);
}

#[test]
fn flight_15_full_matches_oracle() {
    check(15);
}

#[test]
fn flight_16_full_matches_oracle() {
    check(16);
}

#[test]
fn flight_17_full_matches_oracle() {
    check(17);
}

#[test]
fn flight_18_full_matches_oracle() {
    check(18);
}

#[test]
fn flight_19_full_matches_oracle() {
    check(19);
}

#[test]
fn flight_20_full_matches_oracle() {
    check(20);
}
