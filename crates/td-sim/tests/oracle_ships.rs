//! L5 parity: mission ships (setup, AI, movement, removal, enemy fire through the full flight
//! tick) against capture C-0009 (E1 flight 1, no input after launch; whole DGROUP dumped every
//! video frame). Needs TD2192_DATA and TD2192_ORACLE.
//!
//! Spec: mission-ships.md rev 2 §6, mission-setup.md rev 1. Sampling (§6): the state after tick
//! t is the **last** frame whose tick counter DS:1718 is t; the first frame of tick 1 shows the
//! state after setup. The flight is built with `Flight::with_mission` as the first flight of a
//! program run (the seed after setup is the seed after 40 `Random` steps from 0, as captured).
//!
//! Our own mission logic (`mission-logic.md` rev 1 §5.1) drives the flight: it removes slots
//! 9–11 at tick 1 and orders the attacks; the harness only ticks and compares. The mission
//! phase (DS:32BA), the ticker (DS:32BC) and the markers (DS:3958, in the slot arrays) are
//! compared as well.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use td_assets::DataDir;
use td_formats::map::TileMap;
use td_formats::mission::Mission;
use td_sim::flight::ships::{NO_SHIP, RECORD_LEN, SHIP_SLOTS, Ship};
use td_sim::flight::shots::Muzzles;
use td_sim::flight::tables::episode1_tile_attributes;
use td_sim::flight::{TickResult, World};
use td_sim::{Flight, TickControls};
use td_test_support::{require_data, require_oracle};

struct Dump(Vec<u8>);

impl Dump {
    fn u8(&self, ds: usize) -> u8 {
        self.0[ds]
    }
    fn i16(&self, ds: usize) -> i16 {
        i16::from_le_bytes([self.0[ds], self.0[ds + 1]])
    }
    fn ticks(&self) -> u32 {
        u32::from_le_bytes(self.0[0x1718..0x171C].try_into().unwrap())
    }
    fn class(&self, i: usize) -> u8 {
        self.u8(0x3A7A + i)
    }
    /// Slot `i` (0-based) as (slot arrays, record bytes).
    fn slot(&self, i: usize) -> (Vec<i16>, Vec<u8>) {
        let arrays = vec![
            i16::from(self.u8(0x3A7A + i)),
            i16::from(self.u8(0x3B2E + i)),
            i16::from(self.u8(0x3B42 + i)),
            self.i16(0x3ACA + 2 * i),
            self.i16(0x3AF2 + 2 * i),
            i16::from(self.u8(0x3AA2 + i)),
            i16::from(self.u8(0x3AB6 + i)),
            i16::from(self.u8(0x3B1A + i)),
            i16::from(self.u8(0x3B6A + 4 * i)),
            i16::from(self.u8(0x3B6B + 4 * i)),
            i16::from(self.u8(0x3B6C + 4 * i)),
            i16::from(self.u8(0x3B6D + 4 * i)),
            i16::from(self.u8(0x3BBA + i)),
            i16::from(self.u8(0x3BCE + i)),
            i16::from(self.u8(0x3958 + i)),
        ];
        let base = 0x4432 + i * RECORD_LEN;
        (arrays, self.0[base..base + RECORD_LEN].to_vec())
    }
}

fn ours(s: &Ship) -> (Vec<i16>, Vec<u8>) {
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

/// Describes how slot `i` differs, or `None` when it matches.
fn diff(f: &Flight, d: &Dump, i: usize) -> Option<String> {
    let (oa, or) = ours(&f.ships.0[i]);
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

/// First frame of tick 1 and the last frame of every tick.
fn load(dir: &Path) -> (Dump, BTreeMap<u32, Dump>) {
    let mut names: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.to_string_lossy().ends_with("_ds.bin"))
        .collect();
    names.sort();
    let mut first = None;
    let mut last = BTreeMap::new();
    for p in names {
        let d = Dump(fs::read(&p).unwrap());
        let t = d.ticks();
        if t == 1 && first.is_none() {
            first = Some(Dump(d.0.clone()));
        }
        last.insert(t, d);
    }
    (first.expect("a frame of tick 1"), last)
}

#[test]
fn mission_ships_match_c0009_tick_by_tick() {
    let data = require_data!();
    let oracle = require_oracle!();
    let dir = DataDir::open(&data).unwrap();
    let (setup, ticks) = load(&oracle.join("C-0009").join("mem"));
    let world = World {
        map: TileMap::parse_episode(&dir.read("td.map").unwrap()).unwrap(),
        tile_attr: episode1_tile_attributes(),
        // The muzzle table (DS:035C) as the original holds it.
        muzzles: Muzzles(setup.0[0x035C..0x035C + 342].to_vec()),
    };
    let mission = Mission::parse(&dir.read("m1.td").unwrap()).unwrap();
    let mut f = Flight::with_mission(1, &mission, &world);

    // Setup (mission-ships §4.1, §6 vector 6): every slot, after AI_ChooseWaypoint.
    assert_eq!(
        f.rng.seed(),
        u32::from_le_bytes(setup.0[0x4EE..0x4F2].try_into().unwrap())
    );
    let bad: Vec<String> = (0..SHIP_SLOTS)
        .filter_map(|i| diff(&f, &setup, i))
        .collect();
    assert!(bad.is_empty(), "after setup:\n{}", bad.join("\n"));

    let last = *ticks.keys().last().unwrap();
    let mut compared = 0usize;
    let mut flashed = 0usize;
    let mut mismatches = Vec::new();
    for t in 1..=last {
        let d = &ticks[&t];
        assert_eq!(
            f.tick(&world, &TickControls::default(), &mut vec![]),
            TickResult::Done
        );
        let ours = (f.mission.phase, f.mission.ticker.pos);
        let orig = (d.u8(0x32BA), d.i16(0x32BC) as u16);
        if ours != orig {
            mismatches.push(format!(
                "tick {t}: (phase, ticker) ours {ours:?} orig {orig:?}"
            ));
        }
        // g_HitFlash (DS:12C4) after step 14 (hit-flash.md rev 1): slot 1 dies at tick 254,
        // five tiles from the player, so 15 decays to 13, 11, ..., 1, 0.
        let flash = d.i16(0x12C4) as u16;
        flashed += usize::from(flash > 0);
        if f.hit_flash != flash {
            mismatches.push(format!(
                "tick {t}: hit flash ours {} orig {flash}",
                f.hit_flash
            ));
        }
        for i in 0..SHIP_SLOTS {
            if d.class(i) == NO_SHIP && !f.ships.0[i].alive() {
                continue;
            }
            compared += 1;
            if let Some(m) = diff(&f, d, i) {
                mismatches.push(format!("tick {t}: {m}"));
            }
        }
        // Stop at the first divergent tick: later ones follow from it.
        if !mismatches.is_empty() {
            break;
        }
    }
    eprintln!("C-0009: {compared} slot-ticks compared over ticks 1..={last}");
    assert!(
        mismatches.is_empty(),
        "{} mismatches:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
    assert_eq!(flashed, 7, "ticks with a hit flash (13, 11, ..., 1)");
}
