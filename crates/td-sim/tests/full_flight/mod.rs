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
//!
//! **World.** The port's own muzzle world for a direct start (`td_sim::flight::front_end`,
//! weapons rev 5 §3.3, §3.3.2, §3.3.3, OB-017 preserved): the executable's table and the 272
//! two-digit-heading bytes built from the front-end state the capture's start leaves (E1: none;
//! E2/E3: the block's first scene card, or E3's restore screen and title card), first checked
//! against the capture's DGROUP at tick 1 (weapons §3.3 test vector (a), §3.3.2 and §3.3.3 test
//! vectors (a)). Nothing is copied from the capture: `g_Weapon` comes from the session (BUG-2
//! fixed), and the hit flash ended by E1 flight 17's cut-scene is zeroed by td-sim
//! (flight-screens §4.10, BUG-3).
//!
//! **Runs of several flights** (VE-5, oracle build 3, ORACLE.md §5b; [`check_run`]): each
//! `tickwatch` line of the scenario starts a watcher generation, and the second flight's dumps
//! carry the suffix `_g2`. The first flight starts at program start, each later one with the
//! session the flight before it left (`Flight::session`: the turn directions too, U-501), after
//! the front end's heap uses between the two flights ([`Leg::front_end`]: E1 flight 21's path
//! arrays, mission-ships rev 6 §4.1.1, U-500), its muzzle bytes from its own tick-1 record. The
//! afterburner's burst frame (and its heading during a burst) is compared too.
//! A [`Leg`] with `ends: false` compares a replay that covers only part of a flight (RE
//! captures keyed by hand, U-430).

// Each test crate uses one layout.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use td_assets::{DataDir, ExeResources, ResourceValue};
use td_formats::map::TileMap;
use td_formats::mission::Mission;
use td_sim::flight::front_end::{
    MUZZLE_TABLE_LEN, TWO_DIGIT_HEADINGS, direct_start_world_music, muzzle_base,
};
use td_sim::flight::heap::HeapUse;
use td_sim::flight::player::Turn;
use td_sim::flight::ships::{NO_SHIP, RECORD_LEN, SHIP_SLOTS, Ship};
use td_sim::flight::shots::{SHOT_SLOTS, Shot};
use td_sim::flight::{Episode, Session, TickResult, episode};
use td_sim::replay::Replay;
use td_sim::{Flight, FlightKey};

/// The muzzle table and the DGROUP bytes ships read past it on two-digit headings (U-160).
const MUZZLE_LEN: usize = 34 * 99 + 36;
const STALL: u32 = 0x8000_0000;

/// The direct-start muzzle world of flight `flight` of `ep` (weapons rev 7 §3.3, §3.3.2,
/// §3.3.3) for a run with `music` on or off (E1 `g_MemAvailText`, U-560), checked against
/// `captured` (the capture's DGROUP from the muzzle table on, at tick 1): the table and the
/// eight two-digit ranges.
fn direct_start_muzzles(
    dir: &DataDir,
    (ep, flight, music): (Episode, u8, bool),
    captured: &[u8],
    name: &str,
) -> Vec<u8> {
    let res = ExeResources::load(dir).unwrap();
    let Some(ResourceValue::Bytes(table)) = res.get("flight.muzzle_offsets").cloned() else {
        panic!("muzzle table");
    };
    let world = direct_start_world_music(ep, flight, &table, music, |f| dir.read(f)).unwrap();
    assert_eq!(
        world[..MUZZLE_TABLE_LEN],
        captured[..MUZZLE_TABLE_LEN],
        "{name}: table"
    );
    let base = usize::from(muzzle_base(ep));
    for h in TWO_DIGIT_HEADINGS {
        let at = 34 * usize::from(h) + 2;
        assert_eq!(
            world[at..at + 34],
            captured[at..at + 34],
            "{name}: heading {h:02X}h bytes (DS:{:04X})",
            base + at
        );
    }
    world
}

/// Checks the direct-start muzzle world of flight `flight` of `l`'s episode against the
/// DGROUP of its capture (the success replay's oracle run) at tick 1, and that the 272 bytes
/// stay constant in every record of the flight; skips without a capture. Returns whether it
/// compared.
pub fn check_world(l: &Layout, data: &Path, oracle: &Path, flight: u8) -> bool {
    let tag = format!("{}{flight}-plan", l.tag);
    let Some(cap) = find_capture(oracle, &tag) else {
        eprintln!("SKIPPED: no {tag} capture under the oracle root");
        return false;
    };
    let name = cap.file_name().unwrap().to_string_lossy().into_owned();
    let ds = load_packed(
        &cap.join(format!("mem/tick_{}-0000_{:X}_ds.bin", l.dgroup, l.ds_len)),
        l.ds_len,
    );
    let first = ds
        .iter()
        .find(|r| r.value == 1)
        .expect("a record of tick 1");
    let dir = DataDir::open(data).unwrap();
    let world = direct_start_muzzles(
        &dir,
        (l.episode, flight, true),
        &first.bytes[l.muzzles..l.muzzles + MUZZLE_LEN],
        &name,
    );
    for r in &ds {
        for h in TWO_DIGIT_HEADINGS {
            let at = 34 * usize::from(h) + 2;
            assert_eq!(
                world[at..at + 34],
                r.bytes[l.muzzles + at..l.muzzles + at + 34],
                "{name}: heading {h:02X}h bytes in the record of {:X}",
                r.value
            );
        }
    }
    eprintln!(
        "{name} flight {flight}: muzzle world matches in {} records",
        ds.len()
    );
    true
}

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
    /// `g_BurstFrame`, then `g_BurstHeading` (afterburner, flight-model §4.7).
    pub burst: usize,
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
    burst: 0x1836,
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
    burst: 0x175A,
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
    burst: 0x1756,
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

fn find_capture(oracle: &Path, tag: &str) -> Option<PathBuf> {
    let head = format!("# {tag} ");
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

fn replay_path(l: &Layout, flight: u8, suffix: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/replays")
        .join(format!("e{}f{flight:02}{suffix}.tdr", l.number()))
}

/// The replay `e<E>f<NN><suffix>.tdr` of `l`'s episode, if it exists.
pub fn replay(l: &Layout, flight: u8, suffix: &str) -> Option<Replay> {
    let bytes = fs::read(replay_path(l, flight, suffix)).ok()?;
    Some(Replay::from_bytes(&bytes).unwrap())
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
        field("burst frame", f.player.burst_frame, d.u8(l.burst)),
        field("over", u8::from(f.mission_over), d.u8(l.over)),
        field("end ticks", f.end_ticks, d.u8(l.over + 1)),
    ];
    // The burst heading is stale between bursts on both sides (VE-5).
    if f.player.burst_frame != 0 || d.u8(l.burst) != 0 {
        v.push(field(
            "burst heading",
            f.player.burst_heading.code(),
            d.u8(l.burst + 1),
        ));
    }
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

fn differences(f: &Flight, d: &Dump, shots: bool) -> Vec<String> {
    // E1 flight 21 ends itself in step 1 of tick 25 (tick-order step 1, `0E8B:CC9A`), so the
    // record of value 25, which holds tick 25's steps 1–3, already shows the mission over and
    // the end-sequence counter at 1.
    let ends_next = f.episode == Episode::E1 && f.number == 21 && f.ticks == 24;
    let next_tick = |n: &str, b: i64| ends_next && matches!(n, "over" | "end ticks") && b == 1;
    let mut errs: Vec<String> = fields(f, d)
        .iter()
        .filter(|(n, a, b)| a != b && !next_tick(n, *b))
        .map(|(n, a, b)| format!("{n} ours {a} orig {b}"))
        .collect();
    errs.extend(heading_ok(f, d));
    for i in 0..SHIP_SLOTS {
        if f.ships.0[i].alive() || d.u8(d.l.class + i) != NO_SHIP {
            errs.extend(slot_diff(f, d, i));
        }
    }
    for i in (0..SHOT_SLOTS).filter(|_| shots) {
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

/// The file name suffix of a watcher generation's dumps (oracle 3, ORACLE.md §5b: `_g<n>` from
/// the second flight of a run on).
fn gen_suffix(generation: usize) -> String {
    if generation > 1 {
        format!("_g{generation}")
    } else {
        String::new()
    }
}

/// The first file in `cap/mem` whose name starts with `head` and ends with `tail`.
fn mem_file(cap: &Path, head: &str, tail: &str) -> Option<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(cap.join("mem"))
        .ok()?
        .map(|e| e.unwrap().path())
        .filter(|p| {
            let n = p.file_name().unwrap().to_string_lossy();
            n.starts_with(head) && n.ends_with(tail)
        })
        .collect();
    v.sort();
    v.into_iter().next()
}

/// The packed projectile dump of a generation and its linear address and length
/// (`tick_SSSS-0000_<LEN>_proj[_g<n>].bin`; VE-5 dumps a window that holds every heap place of
/// E2/E3), if the capture has one.
fn proj_file(cap: &Path, generation: usize) -> Option<(PathBuf, usize, usize)> {
    let p = mem_file(
        cap,
        "tick_",
        &format!("_proj{}.bin", gen_suffix(generation)),
    )?;
    let name = p.file_name().unwrap().to_string_lossy().into_owned();
    let seg = usize::from_str_radix(&name[5..9], 16).unwrap();
    let len = name[15..].split('_').next().unwrap();
    let len = usize::from_str_radix(len, 16).unwrap();
    Some((p, seg * 16, len))
}

/// Where a flight's muzzle bytes past the table come from (U-160, weapons rev 4/5 §3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Muzzles {
    /// E1 direct start: built by td-sim (`FrontEnd::direct_start`) and checked against the
    /// capture's tick-1 record.
    DirectStart,
    /// The same with music off (`td.cfg` 01 02 02): `g_MemAvailText` `209520` (weapons rev 7
    /// §3.3 item 2, U-560, BUG-22).
    DirectStartMusicOff,
    /// The capture's tick-1 record (E2/E3, whose ranges td-sim does not build yet; a flight
    /// after a played story or after another flight, whose front-end state only td-flow keeps:
    /// `td-flow/tests/front_end_state.rs` checks those bytes against the same records).
    Capture,
}

/// One flight of a capture's program run: the replay `e<E>f<NN><suffix>.tdr`, flown with the
/// session the flights before it left, compared with the records of its watcher generation.
#[derive(Debug, Clone, Copy)]
pub struct Leg<'a> {
    pub flight: u8,
    pub suffix: &'a str,
    pub muzzles: Muzzles,
    /// The flight ends on the replay's last tick (success or the end of a death), the
    /// original's records end there too, and the capture has the outcome dump `end240`.
    /// Otherwise the replay covers part of the flight and each of its ticks is compared.
    pub ends: bool,
    /// What the capture's front end did on the flight heap before this flight, after the
    /// previous one (`PathMemory::apply`; only E1 flight 21 reads it, mission-ships rev 6
    /// §4.1.1).
    pub front_end: &'a [HeapUse],
}

/// The frame of the next generation's first record, if the capture flew a next flight.
fn next_flight_frame(l: &Layout, cap: &Path, generation: usize) -> Option<u32> {
    let p = mem_file(
        cap,
        "tick_",
        &format!("-0000_{:X}_ds{}.bin", l.ds_len, gen_suffix(generation + 1)),
    )?;
    let mut head = [0u8; 8];
    std::io::Read::read_exact(&mut fs::File::open(p).ok()?, &mut head).ok()?;
    Some(u32::from_le_bytes([head[4], head[5], head[6], head[7]]))
}

/// The dump with the outcome after teardown: 240 frames after the last tick, or 120 when the
/// next flight of the block had started by then (E3 flight 21 follows flight 11 with no
/// debrief).
fn end_dump(l: &Layout, cap: &Path, generation: usize) -> PathBuf {
    let gs = gen_suffix(generation);
    let frame = |p: &Path| -> u32 {
        let n = p.file_name().unwrap().to_string_lossy();
        n[1..9].parse().unwrap()
    };
    let next = next_flight_frame(l, cap, generation);
    for tag in ["end240", "end120"] {
        if let Some(p) = mem_file(cap, "f", &format!("_{tag}{gs}.bin"))
            && next.is_none_or(|n| frame(&p) < n)
        {
            return p;
        }
    }
    panic!(
        "{}: no outcome dump of generation {generation}",
        cap.display()
    );
}

/// Compares flight `flight` of `l`'s episode (success replay `e<E>f<NN>.tdr`) with its
/// capture (skips without a replay or a capture). `data` and `oracle` are the installation and
/// the oracle capture root.
pub fn check(l: &Layout, data: &Path, oracle: &Path, flight: u8) {
    let tag = format!("{}{flight}-plan", l.tag);
    check_replay(l, data, oracle, flight, "", &tag);
}

/// As [`check`] for the replay `e<E>f<NN><suffix>.tdr` and the capture whose scenario starts
/// with `# <tag> `.
pub fn check_replay(l: &Layout, data: &Path, oracle: &Path, flight: u8, suffix: &str, tag: &str) {
    let muzzles = if l.episode == Episode::E1 {
        Muzzles::DirectStart
    } else {
        Muzzles::Capture
    };
    check_run(
        l,
        data,
        oracle,
        tag,
        &[Leg {
            flight,
            suffix,
            muzzles,
            ends: true,
            front_end: &[],
        }],
    );
}

/// Compares the flights of one program run with the capture whose scenario starts with
/// `# <tag> ` (VE-5, oracle 3: the second flight's records are generation 2): the first flight
/// starts at program start (`Session::for_episode`), each later one with the session the one
/// before it left (`Flight::session`; the front end between them draws no `Random`), after the
/// leg's heap uses ([`Leg::front_end`]).
pub fn check_run(l: &Layout, data: &Path, oracle: &Path, tag: &str, legs: &[Leg]) {
    let mut replays = vec![];
    for leg in legs {
        let Some(r) = replay(l, leg.flight, leg.suffix) else {
            eprintln!("SKIPPED: no replay for flight {}{}", leg.flight, leg.suffix);
            return;
        };
        replays.push(r);
    }
    check_run_with(l, (data, oracle), tag, legs, &replays, true);
}

/// [`check_run`] with the replays given (`replays[i]` for `legs[i]`). Without `outcome_dump`
/// a leg that ends has no outcome dump in the capture: its last tick is compared with the
/// stall record the original left after it (the state after that tick, before the teardown),
/// and the original must have flown no further tick.
pub fn check_run_with(
    l: &Layout,
    (data, oracle): (&Path, &Path),
    tag: &str,
    legs: &[Leg],
    replays: &[Replay],
    outcome_dump: bool,
) {
    let Some(cap) = find_capture(oracle, tag) else {
        eprintln!("SKIPPED: no {tag} capture under the oracle root");
        return;
    };
    let dir = DataDir::open(data).unwrap();
    let mut session = Session::for_episode(l.episode);
    for (i, (leg, r)) in legs.iter().zip(replays).enumerate() {
        Arc::make_mut(&mut session.paths)
            .apply(leg.front_end, |f| dir.read(f))
            .unwrap();
        let f = check_leg(l, data, &cap, (i + 1, outcome_dump), leg, r, session, None);
        session = f.session();
    }
}

/// [`check_run`] for one flight that the player quits from the flight menu (VE-9): the menu
/// the replay opens in tick `abort` (its Esc) is answered with Quit (flight-screens §4.1:
/// `g_MissionAborted` := 1), so that tick is the flight's last (tick-order step 35) and the
/// outcome after teardown is compared.
pub fn check_abort(l: &Layout, (data, oracle): (&Path, &Path), tag: &str, leg: &Leg, abort: u32) {
    let Some(r) = replay(l, leg.flight, leg.suffix) else {
        eprintln!("SKIPPED: no replay for flight {}{}", leg.flight, leg.suffix);
        return;
    };
    let Some(cap) = find_capture(oracle, tag) else {
        eprintln!("SKIPPED: no {tag} capture under the oracle root");
        return;
    };
    let session = Session::for_episode(l.episode);
    let f = check_leg(l, data, &cap, (1, true), leg, &r, session, Some(abort));
    assert!(f.aborted, "flight {}: not aborted", leg.flight);
    // E2 `g_MissionAborted` DS:11F2 (e2-program-flow §3), after `g_PlayerDied`.
    if l.episode == Episode::E2 {
        let end = fs::read(end_dump(l, &cap, 1)).unwrap();
        assert_eq!(
            end[l.lost + 2],
            1,
            "flight {}: g_MissionAborted",
            leg.flight
        );
    }
}

/// Compares one flight (generation `generation` of the capture) and returns its final state.
/// `abort`: the tick whose flight menu the player quits ([`check_abort`]).
#[allow(clippy::too_many_arguments)]
fn check_leg(
    l: &Layout,
    data: &Path,
    cap: &Path,
    (generation, outcome_dump): (usize, bool),
    leg: &Leg,
    r: &Replay,
    session: Session,
    abort: Option<u32>,
) -> Flight {
    let flight = leg.flight;
    let name = format!(
        "{}{}",
        cap.file_name().unwrap().to_string_lossy(),
        if generation > 1 {
            format!(" (flight {generation} of the run)")
        } else {
            String::new()
        }
    );
    assert_eq!(
        (r.episode, r.mission),
        (l.number(), flight),
        "{name}: replay"
    );
    assert_eq!(r.seed, session.rng.seed(), "{name}: the replay's session");
    let gs = gen_suffix(generation);
    let ds_path = mem_file(cap, "tick_", &format!("-0000_{:X}_ds{gs}.bin", l.ds_len))
        .unwrap_or_else(|| panic!("{name}: no DGROUP dump of generation {generation}"));
    let ds = load_packed(&ds_path, l.ds_len);
    let proj = proj_file(cap, generation).map(|(p, base, len)| (load_packed(&p, len), base));
    let empty: Vec<u8> = vec![];
    if let Some((p, _)) = &proj {
        assert_eq!(ds.len(), p.len(), "{name}: record counts");
    }
    // Records by counter value (stall records apart).
    let mut by_tick = std::collections::BTreeMap::new();
    let mut stalls = std::collections::BTreeMap::new();
    for (i, rec) in ds.iter().enumerate() {
        if let Some((p, _)) = &proj {
            assert_eq!(rec.value, p[i].value, "{name}: record {i}");
        }
        if rec.value & STALL == 0 {
            by_tick.insert(rec.value, i);
        } else {
            stalls.insert(rec.value & !STALL, i);
        }
    }
    let dump = |i: usize| Dump {
        l,
        ds: &ds[i].bytes,
        proj: proj.as_ref().map_or(&empty, |(p, _)| &p[i].bytes),
        proj_base: proj.as_ref().map_or(0, |(_, b)| *b),
    };

    let ep = l.episode;
    let dir = DataDir::open(data).unwrap();
    let first = dump(*by_tick.get(&1).expect("a record of tick 1"));
    let map = TileMap::parse_episode(&dir.read(episode::map_file(ep, flight)).unwrap()).unwrap();
    let captured = &first.ds[l.muzzles..l.muzzles + MUZZLE_LEN];
    let muzzles = match leg.muzzles {
        Muzzles::DirectStart => direct_start_muzzles(&dir, (ep, flight, true), captured, &name),
        Muzzles::DirectStartMusicOff => {
            direct_start_muzzles(&dir, (ep, flight, false), captured, &name)
        }
        Muzzles::Capture => captured.to_vec(),
    };
    let world = episode::world(ep, flight, map, muzzles);
    let mission = episode::mission_file(ep, flight)
        .map(|file| Mission::parse(&dir.read(&file).unwrap()).unwrap());
    // Nothing is copied from the capture: the session carries the turn directions (U-501,
    // flight-model rev 3 §2) and, for E1 flight 21, the path arrays (U-500).
    let mut f = Flight::start(flight, mission.as_ref(), &world, session);
    let has_proj = proj.is_some();

    // Setup: the record of tick 1 holds the state after setup (plus tick 1's turn step).
    let mut errs = differences(&f, &first, has_proj);
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
        while let TickResult::Modal(m) = result {
            cov.comm_viewers.push(t);
            // The transmission screen does not fade, so g_HitFlash survives it (C-0058 tick
            // 318: 10 after the screen), and td2192 leaves it alone too. The flight menu fades
            // out and back, which zeroes it (hit-flash §4.2; the host's part).
            if m == td_sim::Modal::FlightMenu {
                f.hit_flash = 0;
                // Quit (the host's part of the menu): the tick then leaves the loop.
                f.aborted = abort == Some(t);
            }
            result = f.resume(&world, &td_sim::TickControls::default(), &mut ev);
        }
        if f.mission.scene != 0 {
            cov.events.push(format!("t{t} scene {}", f.mission.scene));
        }
        // After the last tick the host fades the flight out (flight-screens §4.5.3), and every
        // level of that fade zeroes g_HitFlash (hit-flash §4.2): the stall record, 150 ms
        // later, shows 0 (E3 flight 8, C-0135: 8 in td-sim, 0 in the original).
        if result == TickResult::Finished {
            f.hit_flash = 0;
        }
        // After the last tick: the stall record of that tick when the original stopped there
        // too, else the record of the tick it went on to fly.
        let idx = if t < n || !leg.ends {
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
        let mut errs = differences(&f, &d, has_proj);
        // A quit tick is compared with the stall record taken while the flight menu was
        // opening (150 ms after the tick began): the Esc that opened it may still be down
        // there, and the menu clears the key bytes later (flight-screens §4.1).
        if abort == Some(t) {
            errs.retain(|e| !e.starts_with("key "));
        }
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
        if leg.ends {
            assert_eq!(
                result == TickResult::Finished,
                t == n,
                "{name} flight {flight}: port flight finished at tick {t} of {n}"
            );
        } else {
            assert_ne!(
                result,
                TickResult::Finished,
                "{name} flight {flight}: port flight finished at tick {t}"
            );
        }
    }
    if !leg.ends {
        eprintln!(
            "{name} flight {flight}: PASS ticks 1..{} (part of the flight), {} slot-ticks, \
             projectiles {}, comm {:?}, events {:?}",
            cov.ticks,
            cov.slot_ticks,
            if has_proj { "compared" } else { "not dumped" },
            cov.comm_viewers,
            cov.events
        );
        return f;
    }
    // The original flew no further tick, and its outcome is ours.
    let last = by_tick.keys().last().copied().unwrap();
    assert_eq!(last, n, "{name} flight {flight}: original's last tick");
    if !outcome_dump {
        assert!(
            stalls.contains_key(&n),
            "{name} flight {flight}: a stall record after the last tick"
        );
        eprintln!(
            "{name} flight {flight}: PASS {} ticks (the last against the stall record; no \
             outcome dump), {} slot-ticks, kills {}, succeeded {}, lost {}, died {}, comm {:?}, \
             events {:?}",
            cov.ticks,
            cov.slot_ticks,
            f.kills,
            f.succeeded,
            f.lost,
            f.died(),
            cov.comm_viewers,
            cov.events
        );
        return f;
    }
    let end = fs::read(end_dump(l, cap, generation)).unwrap();
    // g_PlayerDied follows g_MissionLost in every episode.
    assert_eq!(
        (
            end[l.outcome] != 0,
            end[l.succeeded] != 0,
            end[l.lost] != 0,
            end[l.lost + 1] != 0
        ),
        (f.outcome(), f.succeeded, f.lost, f.died()),
        "{name} flight {flight}: (outcome, succeeded, lost, died) after teardown"
    );
    eprintln!(
        "{name} flight {flight}: PASS {} ticks, {} slot-ticks, kills {}, outcome {}, died {}, \
         comm {:?}, events {:?}",
        cov.ticks,
        cov.slot_ticks,
        f.kills,
        f.outcome(),
        f.died(),
        cov.comm_viewers,
        cov.events
    );
    f
}
