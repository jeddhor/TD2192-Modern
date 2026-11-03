//! L5 parity: Episode 2 (`td2.exe`) setup and first ticks against captures C-0015 (E2 flight 1,
//! `td.map`, the program started directly) and C-0016 (E2 flight 9, `td2.map`, reached by
//! restoring a prepared `TD2.SAV`). Both dump the whole E2 DGROUP (`3EDA:0000`, 0x9800 bytes)
//! every video frame from the mission-file load for about 60 ticks, with no flight input.
//! Needs TD2192_DATA and TD2192_ORACLE.
//!
//! Spec: e2-mission-setup.md rev 1 §6 vectors 1 and 2, e2-mission-ships.md rev 1 §6, and
//! e2-mission-logic.md rev 1 §7 vector C1. E2 DGROUP offsets: episode-deltas §3.2 and
//! `docs/re/symbols/td2.csv` (the slot arrays and records are E1's −D8h).
//!
//! Sampling: the state after tick t is the **last** frame whose tick counter (DS:163C) is t;
//! the state after setup is the last frame with counter 0 before tick 1.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use td_assets::DataDir;
use td_formats::map::TileMap;
use td_formats::mission::Mission;
use td_sim::flight::episode::{self, Episode};
use td_sim::flight::ships::{NO_SHIP, RECORD_LEN, SHIP_SLOTS, Ship};
use td_sim::flight::{Session, TickResult};
use td_sim::{Flight, TickControls};
use td_test_support::{require_data, require_oracle};

/// E2 DGROUP offsets (td2.csv; E1 address − D8h for the slot arrays and records).
mod ds {
    pub const RAND_SEED: usize = 0x064C;
    pub const MUZZLES: usize = 0x04BA;
    pub const TICKS: usize = 0x163C;
    pub const LATE_DRAW: usize = 0x1621;
    pub const PALETTE_MODE: usize = 0x1622;
    pub const MAP_SET: usize = 0x1302;
    pub const PLAYER_CLASS: usize = 0x197F;
    pub const WEAPON: usize = 0x1BE0;
    pub const MISSILES: usize = 0x1BE1;
    pub const COMM: usize = 0x31C4;
    pub const SUCCEEDED: usize = 0x31C6;
    pub const PHASE: usize = 0x31E2;
    pub const TICKER: usize = 0x31E4;
    pub const ARMOUR: usize = 0x38B6;
    pub const SHIELDS: usize = 0x38B7;
    pub const HOME_MARKER: usize = 0x3894;
    pub const VIEW: usize = 0x395E;
    pub const RADAR: usize = 0x036A;
    pub const RADAR2: usize = 0x03BA;
    pub const CLASS: usize = 0x39A2;
    pub const TILE_X: usize = 0x3A56;
    pub const TILE_Y: usize = 0x3A6A;
    pub const SUB_X: usize = 0x39F2;
    pub const SUB_Y: usize = 0x3A1A;
    pub const HEADING: usize = 0x39CA;
    pub const TARGET_HEADING: usize = 0x39DE;
    pub const TURN: usize = 0x3A42;
    pub const SPEED: usize = 0x3A92;
    pub const GOAL_X: usize = 0x3AE2;
    pub const GOAL_Y: usize = 0x3AF6;
    pub const MARKER: usize = 0x3880;
    pub const RECORDS: usize = 0x435A;
}

struct Dump(Vec<u8>);

impl Dump {
    fn u8(&self, a: usize) -> u8 {
        self.0[a]
    }
    fn i16(&self, a: usize) -> i16 {
        i16::from_le_bytes([self.0[a], self.0[a + 1]])
    }
    fn u32(&self, a: usize) -> u32 {
        u32::from_le_bytes(self.0[a..a + 4].try_into().unwrap())
    }
    fn ticks(&self) -> u32 {
        self.u32(ds::TICKS)
    }
    fn class(&self, i: usize) -> u8 {
        self.u8(ds::CLASS + i)
    }
    /// Slot `i` (0-based) as (slot arrays, record bytes).
    fn slot(&self, i: usize) -> (Vec<i16>, Vec<u8>) {
        let b = |a: usize| i16::from(self.u8(a));
        let arrays = vec![
            b(ds::CLASS + i),
            b(ds::TILE_X + i),
            b(ds::TILE_Y + i),
            self.i16(ds::SUB_X + 2 * i),
            self.i16(ds::SUB_Y + 2 * i),
            b(ds::HEADING + i),
            b(ds::TARGET_HEADING + i),
            b(ds::TURN + i),
            b(ds::SPEED + 4 * i),
            b(ds::SPEED + 4 * i + 1),
            b(ds::SPEED + 4 * i + 2),
            b(ds::SPEED + 4 * i + 3),
            b(ds::GOAL_X + i),
            b(ds::GOAL_Y + i),
            b(ds::MARKER + i),
        ];
        let base = ds::RECORDS + i * RECORD_LEN;
        (arrays, self.0[base..base + RECORD_LEN].to_vec())
    }
    /// The mission and player state the harness compares every tick.
    fn globals(&self) -> Vec<i32> {
        let b = |a: usize| i32::from(self.u8(a));
        vec![
            b(ds::PHASE),
            i32::from(self.i16(ds::TICKER)),
            b(ds::COMM),
            b(ds::SUCCEEDED),
            b(ds::HOME_MARKER),
            b(ds::PLAYER_CLASS),
            i32::from(self.u8(ds::ARMOUR) as i8),
            i32::from(self.u8(ds::SHIELDS) as i8),
            b(ds::WEAPON),
            b(ds::MISSILES),
            b(ds::PALETTE_MODE),
            b(ds::LATE_DRAW),
            i32::from(self.i16(ds::VIEW)),
            i32::from(self.i16(ds::VIEW + 2)),
            i32::from(self.i16(ds::VIEW + 4)),
            i32::from(self.i16(ds::VIEW + 6)),
        ]
    }
}

fn ours_slot(s: &Ship) -> (Vec<i16>, Vec<u8>) {
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
        i16::from(s.marker),
    ];
    (arrays, s.record_bytes())
}

fn ours_globals(f: &Flight) -> Vec<i32> {
    let v = f.player.view;
    let m = &f.mission;
    vec![
        i32::from(m.phase),
        i32::from(m.ticker.pos),
        i32::from(m.comm_pending),
        i32::from(f.succeeded),
        i32::from(m.home_marker),
        i32::from(f.player.class),
        i32::from(f.player.armour),
        i32::from(f.player.shields),
        i32::from(f.weapon),
        i32::from(f.missiles),
        i32::from(f.palette_mode),
        i32::from(f.late_draw),
        i32::from(v.tile_x),
        i32::from(v.tile_y),
        i32::from(v.sub_x),
        i32::from(v.sub_y),
    ]
}

/// How slot `i` differs, or `None` when it matches.
fn diff(f: &Flight, d: &Dump, i: usize) -> Option<String> {
    let (oa, or) = ours_slot(&f.ships.0[i]);
    let (ta, tr) = d.slot(i);
    if oa == ta && or == tr {
        return None;
    }
    let bytes: Vec<String> = (0..RECORD_LEN)
        .filter(|&o| or[o] != tr[o])
        .take(6)
        .map(|o| format!("+{o:03X}h ours {:02X} orig {:02X}", or[o], tr[o]))
        .collect();
    Some(format!(
        "slot {}: arrays ours {oa:?} orig {ta:?}; record {}",
        i + 1,
        bytes.join(", ")
    ))
}

/// Every difference between our flight and a dump: globals, the seed, and every slot that is
/// live in either (`all_slots`: every slot, as after setup).
fn compare(f: &Flight, d: &Dump, all_slots: bool) -> Vec<String> {
    let mut out = Vec::new();
    let (og, mut tg) = (ours_globals(f), d.globals());
    if all_slots && tg[12..] == [102, 104, 32, 16] {
        // The start-view override (e2-mission-setup §4.1 step 13) runs after the last setup
        // frame was dumped; the overridden view is checked from tick 1 on.
        tg[12..].copy_from_slice(&og[12..]);
    }
    if og != tg {
        out.push(format!("globals ours {og:?} orig {tg:?}"));
    }
    if f.rng.seed() != d.u32(ds::RAND_SEED) {
        out.push(format!(
            "seed ours {:08X} orig {:08X}",
            f.rng.seed(),
            d.u32(ds::RAND_SEED)
        ));
    }
    for i in 0..SHIP_SLOTS {
        if !all_slots && d.class(i) == NO_SHIP && !f.ships.0[i].alive() {
            continue;
        }
        out.extend(diff(f, d, i));
    }
    out
}

/// The setup state (last counter-0 frame before tick 1) and the last frame of every tick.
fn load(dir: &Path) -> (Dump, BTreeMap<u32, Dump>) {
    let mut names: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.to_string_lossy().ends_with("_dg.bin"))
        .collect();
    names.sort();
    let mut setup = None;
    let mut last = BTreeMap::new();
    for p in names {
        let d = Dump(fs::read(&p).unwrap());
        let t = d.ticks();
        if t == 0 && last.is_empty() {
            setup = Some(Dump(d.0.clone()));
        }
        if t > 0 {
            last.insert(t, d);
        }
    }
    (setup.expect("a setup frame"), last)
}

/// Runs E2 flight `number` from a fresh program start and compares setup and every tick.
fn check(data: &Path, oracle: &Path, capture: &str, number: u8) -> Vec<String> {
    let dir = DataDir::open(data).unwrap();
    let (setup, ticks) = load(&oracle.join(capture).join("mem"));
    let ep = Episode::E2;
    let map = TileMap::parse_episode(&dir.read(episode::map_file(ep, number)).unwrap()).unwrap();
    // The muzzle table as the original holds it (DS:04BA, byte-identical to E1's), with the
    // DGROUP bytes after it for two-digit headings (U-160).
    let world = episode::world(ep, number, map, setup.0[ds::MUZZLES..].to_vec());
    let file = episode::mission_file(ep, number).unwrap();
    let mission = Mission::parse(&dir.read(&file).unwrap()).unwrap();
    let mut f = Flight::start(number, Some(&mission), &world, Session::for_episode(ep));

    // Map set and radar table (presentation, checked once).
    let set2 = setup.u8(ds::MAP_SET) == 2;
    assert_eq!(set2, f.radar_set2, "{capture}: map set");
    if set2 {
        assert_eq!(
            setup.0[ds::RADAR..ds::RADAR + 80],
            setup.0[ds::RADAR2..ds::RADAR2 + 80],
            "{capture}: radar table copy"
        );
    }
    let mut problems: Vec<String> = compare(&f, &setup, true)
        .into_iter()
        .map(|m| format!("{capture} setup: {m}"))
        .collect();
    if !problems.is_empty() {
        return problems;
    }
    let last = *ticks.keys().last().unwrap();
    for t in 1..=last {
        assert_eq!(
            f.tick(&world, &TickControls::default(), &mut vec![]),
            TickResult::Done,
            "{capture} tick {t}"
        );
        let Some(d) = ticks.get(&t) else { continue };
        problems.extend(
            compare(&f, d, false)
                .into_iter()
                .map(|m| format!("{capture} tick {t}: {m}")),
        );
        // Stop at the first divergent tick: later ones follow from it.
        if !problems.is_empty() {
            break;
        }
    }
    eprintln!("{capture}: setup and ticks 1..={last} compared");
    problems
}

#[test]
fn e2_flight_1_matches_c0015() {
    let (data, oracle) = (require_data!(), require_oracle!());
    let p = check(&data, &oracle, "C-0015", 1);
    assert!(p.is_empty(), "{}", p.join("\n"));
}

#[test]
fn e2_flight_9_matches_c0016() {
    let (data, oracle) = (require_data!(), require_oracle!());
    let p = check(&data, &oracle, "C-0016", 9);
    assert!(p.is_empty(), "{}", p.join("\n"));
}
