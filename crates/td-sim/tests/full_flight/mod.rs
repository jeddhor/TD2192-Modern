//! Shared harness of the whole-flight L5 parity tests (`oracle_e1_full.rs`,
//! `oracle_e2_full.rs`, `oracle_e3_full.rs`): a committed success replay against the original
//! flown with the same inputs, every tick from setup to the end of the flight (VE-2, M9-21,
//! `docs/parity/M9-playthrough.md`; E2 and E3: VE-4, M10-45, `docs/parity/M10-playthrough.md`).
//!
//! **Inputs.** `tests/replays/e<E>f<NN>.tdr` (planned by `tdtool plan`). The capture flew its
//! tick-keyed schedule (`tdtool replay --oracle-schedule`, oracle build 2, ORACLE.md §5a): the
//! transitions of port tick t are injected within 1 ms after the game's tick counter
//! `g_MissionTicks` becomes t, during tick t's steps 1–4, so every input step of tick t (steps
//! 23–33; tick-order rev 1) sees them and none of tick t − 1 does. While a comm viewer is open
//! the schedule presses Space and F6 only while the counter still shows the tick that opened
//! it, so no key reaches a flight tick; the port closes the screen at once with no input
//! ([`td_sim::replay::step`]).
//!
//! **Capture.** The episode's program started directly in a private rig copy (E1: VE-1's
//! `tdstatus.dat` method; E2/E3: New game or slot 1 of `TD2.SAV`/`TD3.SAV`, e2-program-flow §8,
//! e3-program-flow §8). The oracle dumps DGROUP and the projectile heap arrays once per tick,
//! within 1 ms after the counter changes: the record of value t + 1 holds the state after tick
//! t, plus tick t + 1's steps 1–3 (the counter and the turn step). A stall record 150 ms after
//! the last change holds the state after the last tick, and a DGROUP dump 240 frames after it
//! holds the outcome (teardown runs after the fade-out). The capture is found by the first
//! line of its `scenario.txt` ([`Layout::tag`]).
//!
//! **Compared, every tick.** The `Random` seed; draw page; phase, comm pending, succeeded,
//! lost, kills, home marker, the 20 markers, ticker, comm indicator, hit flash; the player's
//! view, speeds, heading (or its next turn step), target heading, turn, armour, shields,
//! weapon, missiles, cooldown, mission over and end ticks; the key-table bytes of the flight
//! keys (this checks the tick rule itself); every slot's arrays and its 1,065-byte record; the
//! 51 projectile slots. **At the end:** the original's last tick is the replay's last tick, and
//! the outcome flag is the port's.

// Each test crate uses one layout.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use td_assets::DataDir;
use td_formats::map::TileMap;
use td_formats::mission::Mission;
use td_sim::flight::player::Turn;
use td_sim::flight::ships::{NO_SHIP, RECORD_LEN, SHIP_SLOTS, Ship};
use td_sim::flight::shots::{SHOT_SLOTS, Shot};
use td_sim::flight::{Episode, Session, TickResult, episode};
use td_sim::replay::Replay;
use td_sim::{Flight, FlightKey};

/// The muzzle table and the DGROUP bytes ships read past it on two-digit headings (U-160).
const MUZZLE_LEN: usize = 34 * 99 + 36;
const STALL: u32 = 0x8000_0000;

/// Where an episode keeps the compared state (DGROUP offsets; E1 `docs/re/symbols/file0001.csv`,
/// E2 `td2.csv`, E3 `td3.csv`; episode-deltas §3.2).
#[derive(Debug, Clone, Copy)]
pub struct Layout {
    pub episode: Episode,
    /// First words of the capture's `scenario.txt` (`# <tag><N>-plan `).
    pub tag: &'static str,
    /// DGROUP paragraph and dump length (the packed dump file name).
    pub dgroup: &'static str,
    pub ds_len: usize,
    pub seed: usize,
    pub draw_page: usize,
    pub phase: usize,
    pub comm: usize,
    pub succeeded: usize,
    pub lost: usize,
    pub kills: usize,
    pub outcome: usize,
    pub ticker: usize,
    pub comm_indicator: usize,
    pub hit_flash: usize,
    pub armour: usize,
    pub view: usize,
    pub speed: usize,
    pub target_heading: usize,
    pub turn: usize,
    pub heading: usize,
    pub weapon: usize,
    pub over: usize,
    pub keys: usize,
    /// `g_SlotMarker` (20 slots, then the home marker).
    pub markers: usize,
    /// `g_SlotClass`; the other slot arrays follow at E1's distances.
    pub class: usize,
    pub records: usize,
    /// The 8 projectile far pointers (owner, flag, subX, subY, heading, kind, tile X, tile Y).
    pub proj: usize,
    pub muzzles: usize,
}

/// Episode 1 (`file0001.exe`, DGROUP 4467h when started directly).
pub const E1: Layout = Layout {
    episode: Episode::E1,
    tag: "ve2-e1f",
    dgroup: "4467",
    ds_len: 0x9DF0,
    seed: 0x04EE,
    draw_page: 0x3A30,
    phase: 0x32BA,
    comm: 0x329C,
    succeeded: 0x329E,
    lost: 0x12CC,
    kills: 0x12CA,
    outcome: 0x12CF,
    ticker: 0x32BC,
    comm_indicator: 0x1705,
    hit_flash: 0x12C4,
    armour: 0x398E,
    view: 0x3A36,
    speed: 0x3995,
    target_heading: 0x1CBB,
    turn: 0x1A5A,
    heading: 0x1A56,
    weapon: 0x1CB8,
    over: 0x32B7,
    keys: 0x1A5C,
    markers: 0x3958,
    class: 0x3A7A,
    records: 0x4432,
    proj: 0x396E,
    muzzles: 0x035C,
};

/// Episode 2 (`td2.exe /sb`, DGROUP 3EDAh): E1's offsets −D8h for the flight unit's variables
/// and the slot arrays, other regions by `td2.csv`.
pub const E2: Layout = Layout {
    episode: Episode::E2,
    tag: "ve4-e2f",
    dgroup: "3EDA",
    ds_len: 0x9800,
    seed: 0x064C,
    draw_page: 0x3958,
    phase: 0x31E2,
    comm: 0x31C4,
    succeeded: 0x31C6,
    lost: 0x11F0,
    kills: 0x11EE,
    outcome: 0x11F3,
    ticker: 0x31E4,
    comm_indicator: 0x162A,
    hit_flash: 0x11E4,
    armour: 0x38B6,
    view: 0x395E,
    speed: 0x38BD,
    target_heading: 0x1BE3,
    turn: 0x1982,
    heading: 0x197E,
    weapon: 0x1BE0,
    over: 0x31DF,
    keys: 0x1984,
    markers: 0x3880,
    class: 0x39A2,
    records: 0x435A,
    proj: 0x3896,
    muzzles: 0x04BA,
};

/// Episode 3 (`td3.exe /sb`, DGROUP 3AE9h): E2's offsets −4 for the unit variables
/// (`td3.csv`).
pub const E3: Layout = Layout {
    episode: Episode::E3,
    tag: "ve4-e3f",
    dgroup: "3AE9",
    ds_len: 0x9800,
    seed: 0x064C,
    draw_page: 0x3954,
    phase: 0x31DE,
    comm: 0x31C0,
    succeeded: 0x31C2,
    lost: 0x11EC,
    kills: 0x11EA,
    outcome: 0x11EF,
    ticker: 0x31E0,
    comm_indicator: 0x1626,
    hit_flash: 0x11E4,
    armour: 0x38B2,
    view: 0x395A,
    speed: 0x38B9,
    target_heading: 0x1BDF,
    turn: 0x197E,
    heading: 0x197A,
    weapon: 0x1BDC,
    over: 0x31DB,
    keys: 0x1980,
    markers: 0x387C,
    class: 0x399E,
    records: 0x4356,
    proj: 0x3892,
    muzzles: 0x04BA,
};

impl Layout {
    fn number(&self) -> u8 {
        match self.episode {
            Episode::E1 => 1,
            Episode::E2 => 2,
            Episode::E3 => 3,
        }
    }
    // The slot arrays, at E1's distances from `g_SlotClass` (3A7A).
    fn tile_x(&self) -> usize {
        self.class + 0xB4
    }
    fn tile_y(&self) -> usize {
        self.class + 0xC8
    }
    fn sub_x(&self) -> usize {
        self.class + 0x50
    }
    fn sub_y(&self) -> usize {
        self.class + 0x78
    }
    fn s_heading(&self) -> usize {
        self.class + 0x28
    }
    fn s_target(&self) -> usize {
        self.class + 0x3C
    }
    fn s_turn(&self) -> usize {
        self.class + 0xA0
    }
    fn s_speed(&self) -> usize {
        self.class + 0xF0
    }
    fn goal_x(&self) -> usize {
        self.class + 0x140
    }
    fn goal_y(&self) -> usize {
        self.class + 0x154
    }
}

/// One packed per-tick record (ORACLE.md §5a).
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
    l: &'a Layout,
    ds: &'a [u8],
    proj: &'a [u8],
    /// Linear address of the projectile dump.
    proj_base: usize,
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
    fn proj_u8(&self, k: usize, i: usize) -> u8 {
        let ptr = self.l.proj + 4 * k;
        let lin = usize::from(self.u16(ptr + 2)) * 16 + usize::from(self.u16(ptr));
        self.proj[lin - self.proj_base + i]
    }
    fn proj_i16(&self, k: usize, i: usize) -> i16 {
        i16::from_le_bytes([self.proj_u8(k, 2 * i), self.proj_u8(k, 2 * i + 1)])
    }
    fn shot(&self, i: usize) -> Shot {
        Shot {
            owner: self.proj_u8(0, i),
            life: self.proj_u8(1, i),
            off_x: self.proj_i16(2, i),
            off_y: self.proj_i16(3, i),
            heading: self.proj_u8(4, i),
            kind: self.proj_u8(5, i),
            tile_x: self.proj_u8(6, i),
            tile_y: self.proj_u8(7, i),
        }
    }
}

fn find_capture(oracle: &Path, l: &Layout, flight: u8) -> Option<PathBuf> {
    let head = format!("# {}{flight}-plan ", l.tag);
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

fn replay_path(l: &Layout, flight: u8) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/replays")
        .join(format!("e{}f{flight:02}.tdr", l.number()))
}

type Fields = Vec<(String, i64, i64)>;

fn field(name: &str, ours: impl Into<i64>, orig: impl Into<i64>) -> (String, i64, i64) {
    (name.to_string(), ours.into(), orig.into())
}

/// Scalar state: (name, ours, original).
fn fields(f: &Flight, d: &Dump) -> Fields {
    let l = d.l;
    let m = &f.mission;
    let p = &f.player;
    let mut v = vec![
        field("seed", f.rng.seed(), d.u32(l.seed)),
        field("draw page", f.draw_page, d.u8(l.draw_page)),
        field("phase", m.phase, d.u8(l.phase)),
        field("comm pending", u8::from(m.comm_pending), d.u8(l.comm)),
        field("succeeded", u8::from(f.succeeded), d.u8(l.succeeded)),
        field("lost", u8::from(f.lost), d.u8(l.lost)),
        field("kills", f.kills, d.i16(l.kills)),
        field("home marker", m.home_marker, d.u8(l.markers + 20)),
        field("ticker", m.ticker.pos, d.u16(l.ticker)),
        field("comm indicator", m.comm_phase, d.u8(l.comm_indicator)),
        field("hit flash", f.hit_flash, d.u16(l.hit_flash)),
        field("armour", p.armour, d.u8(l.armour) as i8),
        field("shields", p.shields, d.u8(l.armour + 1) as i8),
        field("view tile x", p.view.tile_x, d.i16(l.view)),
        field("view tile y", p.view.tile_y, d.i16(l.view + 2)),
        field("view sub x", p.view.sub_x, d.i16(l.view + 4)),
        field("view sub y", p.view.sub_y, d.i16(l.view + 6)),
        field("speed up", p.speed.up, d.u8(l.speed)),
        field("speed down", p.speed.down, d.u8(l.speed + 1)),
        field("speed left", p.speed.left, d.u8(l.speed + 2)),
        field("speed right", p.speed.right, d.u8(l.speed + 3)),
        field("target heading", p.target.code(), d.u8(l.target_heading)),
        field(
            "turn",
            match p.turn {
                Turn::None => 0u8,
                Turn::CounterClockwise => 1,
                Turn::Clockwise => 2,
            },
            d.u8(l.turn),
        ),
        field("weapon", f.weapon, d.u8(l.weapon)),
        field("missiles", f.missiles, d.u8(l.weapon + 1)),
        field("cooldown", f.missile_cooldown, d.u8(l.weapon + 2)),
        field("over", u8::from(f.mission_over), d.u8(l.over)),
        field("end ticks", f.end_ticks, d.u8(l.over + 1)),
    ];
    for (i, s) in f.ships.0.iter().enumerate() {
        v.push(field(
            &format!("marker {}", i + 1),
            s.marker,
            d.u8(l.markers + i),
        ));
    }
    for k in FlightKey::ALL {
        v.push(field(
            &format!("key {k:?}"),
            u8::from(f.keys.get(k)),
            u8::from(d.u8(l.keys + usize::from(k.scan_code())) != 0),
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
    let o = d.u8(d.l.heading);
    (o != p.heading.code() && o != next.code()).then(|| {
        format!(
            "heading ours {} (next {}) orig {o}",
            p.heading.code(),
            next.code()
        )
    })
}

fn slot_orig(d: &Dump, i: usize) -> (Vec<i16>, Vec<u8>) {
    let l = d.l;
    let arrays = vec![
        i16::from(d.u8(l.class + i)),
        i16::from(d.u8(l.tile_x() + i)),
        i16::from(d.u8(l.tile_y() + i)),
        d.i16(l.sub_x() + 2 * i),
        d.i16(l.sub_y() + 2 * i),
        i16::from(d.u8(l.s_heading() + i)),
        i16::from(d.u8(l.s_target() + i)),
        i16::from(d.u8(l.s_turn() + i)),
        i16::from(d.u8(l.s_speed() + 4 * i)),
        i16::from(d.u8(l.s_speed() + 4 * i + 1)),
        i16::from(d.u8(l.s_speed() + 4 * i + 2)),
        i16::from(d.u8(l.s_speed() + 4 * i + 3)),
        i16::from(d.u8(l.goal_x() + i)),
        i16::from(d.u8(l.goal_y() + i)),
    ];
    let base = l.records + i * RECORD_LEN;
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
        if f.ships.0[i].alive() || d.u8(d.l.class + i) != NO_SHIP {
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
    let l = a.l;
    let mut out = Vec::new();
    for (n, x) in [
        ("phase", l.phase),
        ("comm", l.comm),
        ("succeeded", l.succeeded),
        ("lost", l.lost),
        ("over", l.over),
        ("kills", l.kills),
        ("player armour", l.armour),
    ] {
        if a.u8(x) != b.u8(x) {
            out.push(format!("t{t} {n} {}", b.u8(x)));
        }
    }
    for i in 0..SHIP_SLOTS {
        if a.u8(l.class + i) != b.u8(l.class + i) {
            out.push(format!(
                "t{t} slot {} class {:02X}",
                i + 1,
                b.u8(l.class + i)
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

/// The packed projectile dump of a capture and its linear address (`tick_SSSS-0000_240_*.bin`).
fn proj_file(cap: &Path) -> (PathBuf, usize) {
    let p = fs::read_dir(cap.join("mem"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| {
            let n = p.file_name().unwrap().to_string_lossy();
            n.starts_with("tick_") && n.ends_with("_240_proj.bin")
        })
        .expect("a projectile dump");
    let name = p.file_name().unwrap().to_string_lossy().into_owned();
    let seg = usize::from_str_radix(&name[5..9], 16).unwrap();
    (p, seg * 16)
}

/// Compares flight `flight` of `l`'s episode with its capture (skips without a replay or a
/// capture). `data` and `oracle` are the installation and the oracle capture root.
pub fn check(l: &Layout, data: &Path, oracle: &Path, flight: u8) {
    let Ok(bytes) = fs::read(replay_path(l, flight)) else {
        eprintln!("SKIPPED: no replay for flight {flight}");
        return;
    };
    let Some(cap) = find_capture(oracle, l, flight) else {
        eprintln!(
            "SKIPPED: no {}{flight}-plan capture under the oracle root",
            l.tag
        );
        return;
    };
    let name = cap.file_name().unwrap().to_string_lossy().into_owned();
    let r = Replay::from_bytes(&bytes).unwrap();
    assert_eq!(
        (r.episode, r.mission),
        (l.number(), flight),
        "{name}: replay"
    );
    let ds = load_packed(
        &cap.join(format!("mem/tick_{}-0000_{:X}_ds.bin", l.dgroup, l.ds_len)),
        l.ds_len,
    );
    let (pf, proj_base) = proj_file(&cap);
    let proj = load_packed(&pf, 0x240);
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
        l,
        ds: &ds[i].bytes,
        proj: &proj[i].bytes,
        proj_base,
    };

    let ep = l.episode;
    let dir = DataDir::open(data).unwrap();
    let first = dump(*by_tick.get(&1).expect("a record of tick 1"));
    let map = TileMap::parse_episode(&dir.read(episode::map_file(ep, flight)).unwrap()).unwrap();
    let muzzles = first.ds[l.muzzles..l.muzzles + MUZZLE_LEN].to_vec();
    let world = episode::world(ep, flight, map, muzzles);
    let mission = episode::mission_file(ep, flight)
        .map(|file| Mission::parse(&dir.read(&file).unwrap()).unwrap());
    let mut f = Flight::start(flight, mission.as_ref(), &world, Session::for_episode(ep));
    let mut known = vec![];
    // Known divergence BUG-2 (weapons §3.2: classes 0 and 1 keep the weapon of the previous
    // flight; at program start the original's g_Weapon is 0, td-sim's 1): copied from the
    // original so that the rest of the flight can be compared.
    if f.player.class <= 1 && f.weapon != first.u8(l.weapon) {
        known.push(format!(
            "setup: weapon ours {} orig {} (BUG-2, copied)",
            f.weapon,
            first.u8(l.weapon)
        ));
        f.weapon = first.u8(l.weapon);
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
            // The transmission screen does not fade, so g_HitFlash survives it (C-0058 tick
            // 318: 10 after the screen), and td2192 leaves it alone too.
            result = f.resume(&world, &td_sim::TickControls::default(), &mut ev);
        }
        // The host's part for the real-time interlude of step 18 (mission-logic §5.17): the
        // E1 flight-17 cut-scene fades out and back, which zeroes g_HitFlash (hit-flash §4.2).
        if ep == Episode::E1 && f.mission.scene == 17 {
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
        (end[l.outcome] != 0, end[l.succeeded] != 0, end[l.lost] != 0),
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
