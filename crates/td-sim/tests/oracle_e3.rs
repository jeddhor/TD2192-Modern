//! L5 parity: Episode 3 (`td3.exe`) setup and ticks against captures C-0036 (E3 flight 1,
//! `td3.map`, `TD3.EXE /sb` started directly, no flight input, ticks 0–58) and C-0041 (E3
//! flight 8, `td4.map`, reached by restoring a prepared `TD3.SAV`: ticks 0–36 without input,
//! then a keypad route into the hazard cell (40, 132) and the whole end sequence to tick 296).
//! Both dump the whole E3 DGROUP (`3AE9:0000`, 0x9800 bytes) every video frame over the ranges
//! of their `scenario.txt`; C-0041 also dumps the player bytes (DS:38B0..3961) and the tick
//! counter (DS:1630..163F) every second frame in between. Needs TD2192_DATA and TD2192_ORACLE.
//!
//! Spec: e3-mission-setup.md rev 1 §6 vectors 1 and 2, e3-mission-ships.md rev 1 §4.3 and §7
//! (hazard test, OB-054), e3-mission-logic.md rev 1 §7 vectors C1 and C2. E3 DGROUP offsets:
//! `docs/re/symbols/td3.csv` (E2's −4 for the unit variables, episode-deltas §3.2).
//!
//! Sampling: ships, the seed and the mission globals after tick t are read from the **last**
//! frame whose tick counter (DS:1638) is t; the player's physics (view, armour, shields; tick
//! steps 26–30, after the tick wait) from the **first** frame whose counter is t + 1. The keys
//! tick t's thrust saw are the key bytes (DS:1980 + scan code) of that same frame (as in
//! `oracle_flight.rs`); where the capture has no key bytes (C-0041 frames 1261–1939) they come
//! from the capture's key events (`events.tsv`) at that frame.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use td_assets::DataDir;
use td_formats::map::TileMap;
use td_formats::mission::Mission;
use td_sim::flight::episode::{self, Episode};
use td_sim::flight::ships::{NO_SHIP, RECORD_LEN, SHIP_SLOTS, Ship};
use td_sim::flight::{Session, TickResult, World};
use td_sim::{Flight, FlightKey, TickControls};
use td_test_support::{require_data, require_oracle};

/// E3 DGROUP offsets (td3.csv).
mod ds {
    pub const RAND_SEED: usize = 0x064C;
    pub const MUZZLES: usize = 0x04BA;
    pub const MAX_SPEED: usize = 0x041C;
    pub const ACCEL: usize = 0x043A;
    pub const LOST: usize = 0x11EC;
    pub const MAP_SET: usize = 0x12FE;
    pub const LATE_DRAW: usize = 0x161D;
    pub const PALETTE_MODE: usize = 0x161E;
    pub const TICKS: usize = 0x1638;
    pub const PLAYER_CLASS: usize = 0x197B;
    pub const KEYS: usize = 0x1980;
    pub const WEAPON: usize = 0x1BDC;
    pub const MISSILES: usize = 0x1BDD;
    pub const COOLDOWN: usize = 0x1BDE;
    pub const COMM: usize = 0x31C0;
    pub const SUCCEEDED: usize = 0x31C2;
    pub const OVER: usize = 0x31DB;
    pub const END_TICKS: usize = 0x31DC;
    pub const PHASE: usize = 0x31DE;
    pub const TICKER: usize = 0x31E0;
    pub const MARKER: usize = 0x387C;
    pub const HOME_MARKER: usize = 0x3890;
    pub const ARMOUR: usize = 0x38B2;
    pub const SHIELDS: usize = 0x38B3;
    pub const SPEED_PLAYER: usize = 0x38B9;
    pub const VIEW: usize = 0x395A;
    pub const CLASS: usize = 0x399E;
    pub const HEADING: usize = 0x39C6;
    pub const TARGET_HEADING: usize = 0x39DA;
    pub const SUB_X: usize = 0x39EE;
    pub const SUB_Y: usize = 0x3A16;
    pub const TURN: usize = 0x3A3E;
    pub const TILE_X: usize = 0x3A52;
    pub const TILE_Y: usize = 0x3A66;
    pub const SPEED: usize = 0x3A8E;
    pub const GOAL_X: usize = 0x3ADE;
    pub const GOAL_Y: usize = 0x3AF2;
    pub const RECORDS: usize = 0x4356;
}

/// The DGROUP ranges dumped in one frame: (base, bytes).
#[derive(Clone)]
struct Dump {
    frame: u32,
    segs: Vec<(usize, Vec<u8>)>,
}

impl Dump {
    fn seg(&self, a: usize, n: usize) -> Option<&(usize, Vec<u8>)> {
        self.segs
            .iter()
            .find(|(b, v)| a >= *b && a + n <= b + v.len())
    }
    fn has(&self, a: usize, n: usize) -> bool {
        self.seg(a, n).is_some()
    }
    fn full(&self) -> bool {
        self.has(0, 0x9800)
    }
    fn u8(&self, a: usize) -> u8 {
        let (b, v) = self.seg(a, 1).expect("dumped");
        v[a - b]
    }
    fn i16(&self, a: usize) -> i16 {
        i16::from_le_bytes([self.u8(a), self.u8(a + 1)])
    }
    fn u32(&self, a: usize) -> u32 {
        u32::from_le_bytes([self.u8(a), self.u8(a + 1), self.u8(a + 2), self.u8(a + 3)])
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
        (
            arrays,
            (base..base + RECORD_LEN).map(|a| self.u8(a)).collect(),
        )
    }
    /// The mission globals written before the tick wait.
    fn mission(&self) -> Vec<i32> {
        let b = |a: usize| i32::from(self.u8(a));
        vec![
            b(ds::PHASE),
            i32::from(self.i16(ds::TICKER)),
            b(ds::COMM),
            b(ds::SUCCEEDED),
            b(ds::HOME_MARKER),
            b(ds::PLAYER_CLASS),
            b(ds::WEAPON),
            b(ds::MISSILES),
            b(ds::COOLDOWN),
            b(ds::PALETTE_MODE),
            b(ds::LATE_DRAW),
            b(ds::LOST),
        ]
    }
    /// The player's physics (tick steps 26–30): armour, shields, speeds, view tile and
    /// offsets.
    fn physics(&self) -> Vec<i32> {
        let mut v = vec![
            i32::from(self.u8(ds::ARMOUR) as i8),
            i32::from(self.u8(ds::SHIELDS) as i8),
        ];
        v.extend((0..4).map(|k| i32::from(self.u8(ds::SPEED_PLAYER + k))));
        v.extend((0..4).map(|k| i32::from(self.i16(ds::VIEW + 2 * k))));
        v
    }
    fn end(&self) -> Vec<i32> {
        vec![
            i32::from(self.u8(ds::OVER)),
            i32::from(self.u8(ds::END_TICKS)),
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

fn ours_mission(f: &Flight) -> Vec<i32> {
    let m = &f.mission;
    vec![
        i32::from(m.phase),
        i32::from(m.ticker.pos),
        i32::from(m.comm_pending),
        i32::from(f.succeeded),
        i32::from(m.home_marker),
        i32::from(f.player.class),
        i32::from(f.weapon),
        i32::from(f.missiles),
        i32::from(f.missile_cooldown),
        i32::from(f.palette_mode),
        i32::from(f.late_draw),
        i32::from(f.lost),
    ]
}

fn ours_physics(f: &Flight) -> Vec<i32> {
    let (v, s) = (f.player.view, f.player.speed);
    vec![
        i32::from(f.player.armour),
        i32::from(f.player.shields),
        i32::from(s.up),
        i32::from(s.down),
        i32::from(s.left),
        i32::from(s.right),
        i32::from(v.tile_x),
        i32::from(v.tile_y),
        i32::from(v.sub_x),
        i32::from(v.sub_y),
    ]
}

fn ours_end(f: &Flight) -> Vec<i32> {
    vec![i32::from(f.mission_over), i32::from(f.end_ticks)]
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

/// Ships, seed, class tables and the mission globals against a full dump (`all_slots`: every
/// slot, as after setup; else the slots live in either).
fn compare_full(f: &Flight, d: &Dump, all_slots: bool) -> Vec<String> {
    let mut out = Vec::new();
    let (om, tm) = (ours_mission(f), d.mission());
    if om != tm {
        out.push(format!("mission ours {om:?} orig {tm:?}"));
    }
    if f.rng.seed() != d.u32(ds::RAND_SEED) {
        out.push(format!(
            "seed ours {:08X} orig {:08X}",
            f.rng.seed(),
            d.u32(ds::RAND_SEED)
        ));
    }
    let tables: Vec<u8> = (0..10)
        .map(|c| d.u8(ds::MAX_SPEED + c))
        .chain((0..10).map(|c| d.u8(ds::ACCEL + c)))
        .collect();
    let ours: Vec<u8> = f
        .classes
        .max_speed
        .iter()
        .chain(&f.classes.accel)
        .copied()
        .collect();
    if tables != ours {
        out.push(format!("class tables ours {ours:?} orig {tables:?}"));
    }
    for i in 0..SHIP_SLOTS {
        if !all_slots && d.class(i) == NO_SHIP && !f.ships.0[i].alive() {
            continue;
        }
        out.extend(diff(f, d, i));
    }
    out
}

/// A capture: its dumps by tick counter, and its key events.
struct Capture {
    /// The last full dump with counter 0 before tick 1.
    setup: Dump,
    /// The first dump (of any range that has the counter) of every tick ≥ 1.
    first: BTreeMap<u32, Dump>,
    /// The last full dump of every tick ≥ 1.
    last_full: BTreeMap<u32, Dump>,
    /// Keypad events: (frame, key, down).
    events: Vec<(u32, FlightKey, bool)>,
    /// Every frame with a counter dump.
    frames: Vec<u32>,
}

fn key_name(s: &str) -> Option<FlightKey> {
    Some(match s {
        "kp2" => FlightKey::Kp2,
        "kp4" => FlightKey::Kp4,
        "kp6" => FlightKey::Kp6,
        "kp8" => FlightKey::Kp8,
        _ => return None,
    })
}

/// Scan codes of the keypad keys (DS:1980 + scan).
fn scan(k: FlightKey) -> usize {
    match k {
        FlightKey::Kp8 => 0x48,
        FlightKey::Kp4 => 0x4B,
        FlightKey::Kp6 => 0x4D,
        FlightKey::Kp2 => 0x50,
        _ => unreachable!(),
    }
}

const PAD: [FlightKey; 4] = [
    FlightKey::Kp2,
    FlightKey::Kp4,
    FlightKey::Kp6,
    FlightKey::Kp8,
];

fn load(dir: &Path) -> Capture {
    let mem = dir.join("mem");
    let mut names: Vec<_> = fs::read_dir(&mem)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    names.sort();
    // Every range dumped in a frame, as one dump (`fNNNNNNNN_3AE9-BASE_LEN_kind.bin`).
    let mut frames: BTreeMap<u32, Dump> = BTreeMap::new();
    for p in names {
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        let parts: Vec<&str> = name.trim_end_matches(".bin").split('_').collect();
        let frame: u32 = parts[0][1..].parse().unwrap();
        let base = usize::from_str_radix(parts[1].split('-').nth(1).unwrap(), 16).unwrap();
        frames
            .entry(frame)
            .or_insert_with(|| Dump {
                frame,
                segs: vec![],
            })
            .segs
            .push((base, fs::read(&p).unwrap()));
    }
    let mut setup = None;
    let mut first = BTreeMap::new();
    let mut last_full = BTreeMap::new();
    let counted: Vec<u32> = frames
        .values()
        .filter(|d| d.has(ds::TICKS, 4))
        .map(|d| d.frame)
        .collect();
    for d in frames.into_values().filter(|d| d.has(ds::TICKS, 4)) {
        let t = d.u32(ds::TICKS);
        if t == 0 && first.is_empty() && d.full() {
            setup = Some(d.clone());
        }
        if t > 0 {
            if d.full() {
                last_full.insert(t, d.clone());
            }
            first.entry(t).or_insert(d);
        }
    }
    let mut events = Vec::new();
    for line in fs::read_to_string(dir.join("events.tsv")).unwrap().lines() {
        let c: Vec<&str> = line.split('\t').collect();
        if c.len() >= 5
            && c[2] == "KEY"
            && let Some(k) = key_name(c[3])
        {
            events.push((c[0].parse().unwrap(), k, c[4] == "down"));
        }
    }
    // The last full dump of a tick is complete only if the capture goes on into the next
    // tick (the final frame of a dump range can stop inside a tick).
    let complete: Vec<u32> = last_full
        .keys()
        .copied()
        .filter(|t| last_full.contains_key(&(t + 1)))
        .collect();
    last_full.retain(|t, _| complete.contains(t));
    Capture {
        setup: setup.expect("a setup frame"),
        first,
        last_full,
        events,
        frames: counted,
    }
}

impl Capture {
    /// The keys tick `t`'s thrust may have seen: the key bytes of the first frame of tick
    /// t + 1; where that frame has none, the key events up to it. Between C-0041's full dumps
    /// the counter is dumped every second frame, so the first frame of tick t + 1 is the one
    /// found or the frame before it; when a key event falls on the one found, both states are
    /// returned and the caller keeps the one whose speeds match the capture.
    fn keys(&self, t: u32) -> Vec<Vec<FlightKey>> {
        let Some(d) = self.first.get(&(t + 1)) else {
            return vec![vec![]];
        };
        if d.has(ds::KEYS, 0x100) {
            return vec![
                PAD.into_iter()
                    .filter(|&k| d.u8(ds::KEYS + scan(k)) != 0)
                    .collect(),
            ];
        }
        let at = |frame: u32| -> Vec<FlightKey> {
            PAD.into_iter()
                .filter(|&k| {
                    self.events
                        .iter()
                        .rev()
                        .find(|e| e.1 == k && e.0 <= frame)
                        .is_some_and(|e| e.2)
                })
                .collect()
        };
        let (now, before) = (at(d.frame), at(d.frame - 1));
        if now == before || self.frames.contains(&(d.frame - 1)) {
            vec![now]
        } else {
            vec![now, before]
        }
    }
}

/// Runs E3 flight `number` from a fresh program start with the capture's keys, comparing the
/// setup and every tick the capture covers. Returns the problems and the flight after the
/// last tick.
fn check(data: &Path, oracle: &Path, capture: &str, number: u8) -> (Vec<String>, Flight, World) {
    let dir = DataDir::open(data).unwrap();
    let cap = load(&oracle.join(capture));
    let ep = Episode::E3;
    let map = TileMap::parse_episode(&dir.read(episode::map_file(ep, number)).unwrap()).unwrap();
    let setup = &cap.setup;
    let muzzles = (ds::MUZZLES..0x9800).map(|a| setup.u8(a)).collect();
    let world = episode::world(ep, number, map, muzzles);
    let file = episode::mission_file(ep, number).unwrap();
    let mission = Mission::parse(&dir.read(&file).unwrap()).unwrap();
    let mut f = Flight::start(number, Some(&mission), &world, Session::for_episode(ep));

    // Map set 1 always; the radar table is the exe's (no copy, e3-mission-setup §3.1).
    assert_eq!(setup.u8(ds::MAP_SET), 1, "{capture}: map set");
    assert!(!f.radar_set2, "{capture}: radar table");
    let mut problems: Vec<String> = compare_full(&f, setup, true)
        .into_iter()
        .chain((ours_physics(&f) != setup.physics()).then(|| {
            format!(
                "physics ours {:?} orig {:?}",
                ours_physics(&f),
                setup.physics()
            )
        }))
        .map(|m| format!("{capture} setup: {m}"))
        .collect();
    if !problems.is_empty() {
        return (problems, f, world);
    }
    let last = *cap.first.keys().last().unwrap();
    let mut held: Vec<FlightKey> = vec![];
    let mut resolved = vec![];
    let controls = |keys: &[FlightKey], held: &[FlightKey]| {
        let mut c = TickControls::default();
        for &k in keys {
            c.set_down(k);
            if !held.contains(&k) {
                c.set_made(k);
            }
        }
        c
    };
    for t in 1..=last {
        let mut options = cap.keys(t);
        if options.len() > 1 {
            resolved.push(t);
            // Keep the key state whose speeds match the capture after this tick.
            let want = cap.first.get(&(t + 1)).map(Dump::physics);
            options.retain(|keys| {
                let mut g = f.clone();
                g.tick(&world, &controls(keys, &held), &mut vec![]);
                want.as_ref().is_none_or(|w| *w == ours_physics(&g))
            });
            if options.is_empty() {
                problems.push(format!("{capture} tick {t}: no key state matches"));
                break;
            }
        }
        let keys = options.swap_remove(0);
        let c = controls(&keys, &held);
        held = keys;
        let r = f.tick(&world, &c, &mut vec![]);
        let want = if t == last && f.end_ticks == 35 {
            TickResult::Finished
        } else {
            TickResult::Done
        };
        assert_eq!(r, want, "{capture} tick {t}");
        if let Some(d) = cap.last_full.get(&t) {
            problems.extend(
                compare_full(&f, d, false)
                    .into_iter()
                    .map(|m| format!("{capture} tick {t}: {m}")),
            );
        }
        let next = cap.first.get(&(t + 1));
        if let Some(d) = next.filter(|d| d.has(ds::VIEW, 8))
            && ours_physics(&f) != d.physics()
        {
            problems.push(format!(
                "{capture} tick {t}: physics ours {:?} orig {:?} (keys {:?})",
                ours_physics(&f),
                d.physics(),
                held
            ));
        }
        // Mission over is set at step 26 (after the tick wait), so it is read with the
        // physics; the end ticks count at step 11 of the next tick, after that frame.
        if let Some(d) = next.filter(|d| d.has(ds::OVER, 2))
            && ours_end(&f) != d.end()
        {
            problems.push(format!(
                "{capture} tick {t}: over/end ticks ours {:?} orig {:?}",
                ours_end(&f),
                d.end()
            ));
        }
        // Stop at the first divergent tick: later ones follow from it.
        if !problems.is_empty() {
            break;
        }
    }
    eprintln!(
        "{capture}: setup and ticks 1..={last} compared; key state chosen by speeds in ticks {resolved:?}"
    );
    (problems, f, world)
}

/// e3-mission-setup §6 vector 1, e3-mission-ships §7 vectors 1–4, e3-mission-logic §7 C1.
#[test]
fn e3_flight_1_matches_c0036() {
    let (data, oracle) = (require_data!(), require_oracle!());
    let (p, f, _) = check(&data, &oracle, "C-0036", 1);
    assert!(p.is_empty(), "{}", p.join("\n"));
    assert_eq!(f.rng.seed(), 0x0532_2D58);
    assert_eq!(f.mission.phase, 1);
}

/// e3-mission-setup §6 vector 2, e3-mission-ships §7 vectors 5–9 (the hazard cell (40, 132)
/// from tick 261, armour −17 per tick through the end sequence with the i8 wrap, OB-054).
#[test]
fn e3_flight_8_matches_c0041() {
    let (data, oracle) = (require_data!(), require_oracle!());
    let (p, f, w) = check(&data, &oracle, "C-0041", 8);
    assert!(p.is_empty(), "{}", p.join("\n"));
    assert_eq!(f.ticks, 296);
    assert_eq!(f.end_ticks, 35);
    assert!(f.died());
    assert!(!f.outcome());
    assert_eq!(f.map_tile(&w, 40, 132), Some(0x45));
}
