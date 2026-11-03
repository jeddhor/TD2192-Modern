//! L5 parity: mission logic of E1 flight 1 (`mission-logic.md` rev 1 §5.1, §7 vectors 1 and
//! 5–10) against capture C-0010 (ship parked at the start; T+I+M chord held 12 frames every
//! 70 frames; DGROUP DS:1700..3CAF dumped every 35 frames). Needs TD2192_DATA and
//! TD2192_ORACLE.
//!
//! The dump holds the mission phase, ticker, transmission flags, markers and the slot arrays
//! (class, tile, offsets, headings, speeds, goal), not the ship records. Sampling: a dump is
//! compared with the state after the tick its counter (DS:1718) shows; a dump taken while that
//! tick was still drawing (before step 19) is compared with the steps that had run (see
//! `matches`).
//!
//! The chord is not ported (decision D5/D9). Its effect at step 34 (armour := 64, missiles :=
//! 30) is copied into the sim on the ticks that see the keys; M also replays an idle ticker at
//! step 33 (weapons.md §4.1), so the test holds M on those ticks. The scenario gives the key
//! frames; frames map to ticks by interpolating the dumps' tick counters, which is exact to
//! about a tick, so each hold is fitted to the dumps (at most ±3 ticks).

use std::fs;
use std::path::Path;

use td_assets::DataDir;
use td_formats::map::TileMap;
use td_formats::mission::Mission;
use td_sim::flight::ships::SHIP_SLOTS;
use td_sim::flight::shots::Muzzles;
use td_sim::flight::tables::episode1_tile_attributes;
use td_sim::flight::{TickResult, World};
use td_sim::{Flight, FlightKey, TickControls};
use td_test_support::{require_data, require_oracle};

/// First and last frame of the chord holds (scenario `msn-e1f1.txt`).
const CHORD_FIRST: u32 = 3600;
const CHORD_LAST: u32 = 30000;
const CHORD_PERIOD: u32 = 70;
const CHORD_LEN: u32 = 12;
/// DS offset of the dump's first byte.
const BASE: usize = 0x1700;

struct Dump {
    frame: u32,
    bytes: Vec<u8>,
}

impl Dump {
    fn u8(&self, ds: usize) -> u8 {
        self.bytes[ds - BASE]
    }
    fn u16(&self, ds: usize) -> u16 {
        u16::from_le_bytes([self.u8(ds), self.u8(ds + 1)])
    }
    fn ticks(&self) -> u32 {
        u32::from_le_bytes(self.bytes[0x18..0x1C].try_into().unwrap())
    }
}

/// The compared state, grouped by the tick step that writes it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Sample {
    /// Steps 8–10 and 18: phase, comm pending, succeeded, mission over, markers (+ home),
    /// slot arrays.
    logic: Vec<i32>,
    /// Step 19: ticker position and indicator phase.
    hud: Vec<i32>,
}

fn sample_orig(d: &Dump) -> Sample {
    let mut logic = vec![
        i32::from(d.u8(0x32BA)),
        i32::from(d.u8(0x329C)),
        i32::from(d.u8(0x329E)),
        i32::from(d.u8(0x32B7)),
        i32::from(d.u8(0x396C)),
    ];
    for i in 0..SHIP_SLOTS {
        logic.extend([
            i32::from(d.u8(0x3958 + i)),
            i32::from(d.u8(0x3A7A + i)),
            i32::from(d.u8(0x3B2E + i)),
            i32::from(d.u8(0x3B42 + i)),
            i32::from(d.u16(0x3ACA + 2 * i) as i16),
            i32::from(d.u16(0x3AF2 + 2 * i) as i16),
            i32::from(d.u8(0x3AA2 + i)),
            i32::from(d.u8(0x3AB6 + i)),
            i32::from(d.u8(0x3B1A + i)),
        ]);
        logic.extend((0..4).map(|k| i32::from(d.u8(0x3B6A + 4 * i + k))));
        logic.extend([i32::from(d.u8(0x3BBA + i)), i32::from(d.u8(0x3BCE + i))]);
    }
    Sample {
        logic,
        hud: vec![i32::from(d.u16(0x32BC)), i32::from(d.u8(0x1705))],
    }
}

fn sample_ours(f: &Flight) -> Sample {
    let m = &f.mission;
    let mut logic = vec![
        i32::from(m.phase),
        i32::from(m.comm_pending),
        i32::from(f.succeeded),
        i32::from(f.mission_over),
        i32::from(m.home_marker),
    ];
    for s in &f.ships.0 {
        logic.extend([
            i32::from(s.marker),
            i32::from(s.class),
            i32::from(s.tile_x),
            i32::from(s.tile_y),
            i32::from(s.sub_x),
            i32::from(s.sub_y),
            i32::from(s.heading),
            i32::from(s.target_heading),
            i32::from(s.turn),
        ]);
        logic.extend(s.speed.iter().map(|&v| i32::from(v)));
        logic.extend([i32::from(s.goal.0), i32::from(s.goal.1)]);
    }
    Sample {
        logic,
        hud: vec![i32::from(m.ticker.pos), i32::from(m.comm_phase)],
    }
}

/// A dump of counter t shows the state after tick t, or a tick t still drawing: after its
/// step 10 (logic of t, HUD of t − 1) or before its step 8 (all of t − 1).
fn matches(orig: &Sample, now: &Sample, prev: &Sample) -> bool {
    orig == now || (orig.logic == now.logic && orig.hud == prev.hud) || orig == prev
}

fn describe(orig: &Sample, ours: &Sample) -> String {
    let names = ["phase", "comm", "succeeded", "over", "home marker"];
    let mut out = Vec::new();
    for (k, (a, b)) in orig.logic.iter().zip(&ours.logic).enumerate() {
        if a != b {
            let what = if k < 5 {
                names[k].to_string()
            } else {
                let fields = [
                    "marker", "class", "tile x", "tile y", "sub x", "sub y", "heading", "target",
                    "turn", "up", "down", "left", "right", "goal x", "goal y",
                ];
                format!("slot {} {}", (k - 5) / 15 + 1, fields[(k - 5) % 15])
            };
            out.push(format!("{what}: orig {a} ours {b}"));
        }
    }
    if orig.hud != ours.hud {
        out.push(format!(
            "ticker/indicator: orig {:?} ours {:?}",
            orig.hud, ours.hud
        ));
    }
    out.join(", ")
}

fn load(dir: &Path) -> Vec<Dump> {
    let mut names: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.to_string_lossy().ends_with("_ds.bin"))
        .collect();
    names.sort();
    names
        .into_iter()
        .map(|p| {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            Dump {
                frame: name[1..9].parse().unwrap(),
                bytes: fs::read(&p).unwrap(),
            }
        })
        .collect()
}

/// Tick counter at `frame`, interpolated between the dumps.
fn tick_at(dumps: &[Dump], frame: u32) -> u32 {
    let k = dumps.partition_point(|d| d.frame <= frame).max(1) - 1;
    let (a, b) = (&dumps[k], &dumps[(k + 1).min(dumps.len() - 1)]);
    if b.frame == a.frame {
        return a.ticks();
    }
    a.ticks() + (frame - a.frame) * (b.ticks() - a.ticks()) / (b.frame - a.frame)
}

/// Ticks that see the chord: the counter at the key-down frame up to the one before the
/// key-up frame (steps 33–34 run at the end of a tick).
fn holds(dumps: &[Dump]) -> Vec<(u32, u32)> {
    (CHORD_FIRST..=CHORD_LAST)
        .step_by(CHORD_PERIOD as usize)
        .map(|f| {
            let a = tick_at(dumps, f);
            let b = tick_at(dumps, f + CHORD_LEN).saturating_sub(1).max(a);
            (a, b)
        })
        .collect()
}

/// Runs `f` up to tick `t` with the chord held on the ticks of `holds`; returns our sample
/// after tick t − 1.
fn run_to(f: &mut Flight, world: &World, t: u32, holds: &[(u32, u32)]) -> Sample {
    let mut before = sample_ours(f);
    for n in f.ticks + 1..=t {
        before = sample_ours(f);
        let chord = holds.iter().any(|&(a, b)| (a..=b).contains(&n));
        tick(f, world, chord);
    }
    before
}

fn tick(f: &mut Flight, world: &World, chord: bool) {
    let mut c = TickControls::default();
    if chord {
        c.set_down(FlightKey::M);
        c.set_made(FlightKey::M);
    }
    assert_eq!(f.tick(world, &c, &mut vec![]), TickResult::Done);
    if chord {
        // Step 34, T+I+M (globals §5): armour 64, missiles 30.
        f.player.armour = 64;
        f.missiles = 30;
    }
}

#[test]
fn flight_1_mission_logic_matches_c0010() {
    let data = require_data!();
    let oracle = require_oracle!();
    let dir = DataDir::open(&data).unwrap();
    // Ships fire on two-digit headings too, and the original then reads its DGROUP past the
    // muzzle table (DS:04F6..10A6, U-160). C-0010 does not hold those bytes; C-0009 (the same
    // key path into flight 1) does, and they stay constant through its flight.
    let c0009 = oracle.join("C-0009").join("mem");
    let first = fs::read_dir(&c0009)
        .ok()
        .and_then(|r| {
            r.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.to_string_lossy().ends_with("_ds.bin"))
                .min()
        })
        .expect("C-0009 for the DGROUP past the muzzle table");
    let dgroup = fs::read(first).unwrap();
    let muzzles = dgroup[0x035C..0x035C + 34 * 99 + 36].to_vec();
    let world = World {
        map: TileMap::parse_episode(&dir.read("td.map").unwrap()).unwrap(),
        tile_attr: episode1_tile_attributes(),
        muzzles: Muzzles(muzzles),
    };
    let mission = Mission::parse(&dir.read("m1.td").unwrap()).unwrap();
    let dumps = load(&oracle.join("C-0010").join("mem"));
    let mut holds = holds(&dumps);

    // The last dump of each tick counter.
    let mut samples: Vec<&Dump> = Vec::new();
    for d in &dumps {
        if samples.last().is_some_and(|l| l.ticks() == d.ticks()) {
            samples.pop();
        }
        samples.push(d);
    }
    let mut f = Flight::with_mission(1, &mission, &world);
    let mut phases = vec![(0, f.mission.phase)];
    let mut h = 0;
    for (k, d) in samples.iter().enumerate() {
        let t = d.ticks();
        let orig = sample_orig(d);
        // A hold that starts before this dump is fitted: the estimate, then up to three ticks
        // either way. A shift must match this dump and, in the first pass, the next one too
        // (a dump taken mid-tick can match a wrong shift on its own).
        let fit = holds.get(h).is_some_and(|&(a, _)| a <= t + 2);
        let shifts: &[i32] = if fit { &[0, 1, -1, 2, -2, 3, -3] } else { &[0] };
        let mut chosen = None;
        'search: for pass in 0..2 {
            for &shift in shifts {
                let mut hs = holds.clone();
                if fit {
                    let (a, b) = hs[h];
                    let a2 = (a as i32 + shift).max(0) as u32;
                    hs[h] = (a2, (b as i32 + shift).max(a2 as i32) as u32);
                }
                let mut g = f.clone();
                let before = run_to(&mut g, &world, t, &hs);
                if !matches(&orig, &sample_ours(&g), &before) {
                    continue;
                }
                if pass == 0
                    && let Some(next) = samples.get(k + 1)
                {
                    let mut g2 = g.clone();
                    let b2 = run_to(&mut g2, &world, next.ticks(), &hs);
                    if !matches(&sample_orig(next), &sample_ours(&g2), &b2) {
                        continue;
                    }
                }
                chosen = Some((g, hs));
                break 'search;
            }
        }
        let Some((g, hs)) = chosen else {
            let mut g = f.clone();
            run_to(&mut g, &world, t, &holds);
            panic!(
                "frame {} tick {t}: {}",
                d.frame,
                describe(&orig, &sample_ours(&g))
            );
        };
        f = g;
        holds = hs;
        while holds.get(h).is_some_and(|&(_, b)| b <= t) {
            h += 1;
        }
        if phases.last().unwrap().1 != f.mission.phase {
            phases.push((t, f.mission.phase));
        }
    }
    let compared = samples.len();
    eprintln!("C-0010: {compared} dumps compared; phase changes (tick, phase): {phases:?}");
    // §7 vectors 5–9.
    let p: Vec<u8> = phases.iter().map(|&(_, p)| p).collect();
    assert_eq!(p, [1, 2, 3, 4, 5]);
    let at = |phase| phases.iter().find(|&&(_, p)| p == phase).unwrap().0;
    assert!((740..=788).contains(&at(2)));
    assert!((1536..=1603).contains(&at(3)));
    assert!((2419..=2716).contains(&at(4)));
    assert!((4165..=4174).contains(&at(5)));
    assert!(!f.succeeded);
}
