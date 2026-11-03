//! L5 parity: player weapons against the original (capture made with
//! `re-work\dosbox\scenarios\wpn-e1m1.txt`: E1 mission 1, class 4, volleys, Tab, missiles).
//! Needs TD2192_DATA and TD2192_ORACLE; the capture is found in the oracle root by the first
//! line of its `scenario.txt` (the rig keeps a copy of the scenario in every capture).
//!
//! Inputs come from the scenario's own key events, not from the dumped key bytes, because the
//! game clears the Space byte when it launches a missile. A key event scheduled at frame F shows
//! up from dump F + 1 on (C-0011: Space down at f3800 is 0 in dump 3800 and 1 in 3801), and tick
//! t's physics runs between the dumps first(t + 1) − 1 and first(t + 1), so the event is seen
//! by tick t when F < first(t + 1).
//!
//! Where a frame lies inside a tick is read from the draw-page byte (DS:3A30), which flips at
//! tick-order step 24: the first frame of a tick is always before its flip, and a frame of tick t
//! whose page differs from that first frame comes after step 24, so it holds the end-of-tick
//! state (shots spawned in step 33 included). After every tick t that has such frames, the
//! player state, weapon, missiles, cooldown and the whole projectile pool must equal one of them
//! (one is enough: a dump can land in the middle of a spawn).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use td_assets::{DataDir, ExeResources, ResourceValue};
use td_formats::map::TileMap;
use td_sim::flight::heading::Heading;
use td_sim::flight::player::Player;
use td_sim::flight::shots::{Muzzles, SHOT_SLOTS, Shot};
use td_sim::flight::tables::episode1_tile_attributes;
use td_sim::flight::{TickResult, World};
use td_sim::{Flight, FlightKey, TickControls};
use td_test_support::{require_data, require_oracle};

const SCENARIO: &str = "# wpn-e1m1 ";
const PROJ_BASE: usize = 0x67D20;

struct Frame {
    ds: Vec<u8>,
    proj: Vec<u8>,
}

impl Frame {
    fn u8(&self, a: usize) -> u8 {
        self.ds[a]
    }
    fn u16(&self, a: usize) -> u16 {
        u16::from_le_bytes([self.ds[a], self.ds[a + 1]])
    }
    fn i16(&self, a: usize) -> i16 {
        self.u16(a) as i16
    }
    fn ticks(&self) -> u32 {
        u32::from_le_bytes(self.ds[0x1718..0x171C].try_into().unwrap())
    }
    /// Byte `i` of the projectile array whose far pointer is at DS:`ptr`.
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

/// The oracle capture made with the weapons scenario, if present.
fn find_capture(oracle: &Path) -> Option<PathBuf> {
    fs::read_dir(oracle).ok()?.find_map(|e| {
        let dir = e.ok()?.path();
        let scenario = fs::read_to_string(dir.join("scenario.txt")).ok()?;
        scenario.starts_with(SCENARIO).then_some(dir)
    })
}

/// Key events of the scenario: (frame, key, down).
fn key_events(capture: &Path) -> Vec<(u32, FlightKey, bool)> {
    let text = fs::read_to_string(capture.join("scenario.txt")).unwrap();
    let mut out = vec![];
    for line in text.lines() {
        let mut w = line.split_whitespace();
        let (Some(at), Some(action), Some(key)) = (w.next(), w.next(), w.next()) else {
            continue;
        };
        let Some(frame) = at.strip_prefix('f').and_then(|f| f.parse::<u32>().ok()) else {
            continue;
        };
        let key = match key {
            "space" => FlightKey::Space,
            "tab" => FlightKey::Tab,
            "kp6" => FlightKey::Kp6,
            _ => continue,
        };
        match action {
            "down" => out.push((frame, key, true)),
            "up" => out.push((frame, key, false)),
            _ => {}
        }
    }
    out
}

fn load_frames(capture: &Path) -> BTreeMap<u32, Frame> {
    let mem = capture.join("mem");
    let mut frames = BTreeMap::new();
    for e in fs::read_dir(&mem).unwrap() {
        let p = e.unwrap().path();
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        if !name.ends_with("_ds.bin") {
            continue;
        }
        let n: u32 = name[1..9].parse().unwrap();
        let proj = fs::read_dir(&mem)
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|q| {
                let s = q.file_name().unwrap().to_string_lossy();
                s.starts_with(&name[..9]) && s.ends_with("_proj.bin")
            })
            .unwrap();
        frames.insert(
            n,
            Frame {
                ds: fs::read(&p).unwrap(),
                proj: fs::read(proj).unwrap(),
            },
        );
    }
    frames
}

#[test]
fn player_weapons_match_the_original() {
    let data = require_data!();
    let oracle = require_oracle!();
    let Some(capture) = find_capture(&oracle) else {
        eprintln!("SKIPPED: no capture made with {SCENARIO}");
        return;
    };
    let dir = DataDir::open(&data).unwrap();
    let res = ExeResources::load(&dir).unwrap();
    let Some(ResourceValue::Bytes(m)) = res.get("flight.muzzle_offsets") else {
        panic!("muzzle table");
    };
    let world = World {
        map: TileMap::parse_episode(&dir.read("td.map").unwrap()).unwrap(),
        tile_attr: episode1_tile_attributes(),
        muzzles: Muzzles(m.clone()),
    };
    let frames = load_frames(&capture);
    let events = key_events(&capture);
    // First and last frame of every tick.
    let mut span: BTreeMap<u32, (u32, u32)> = BTreeMap::new();
    for (&n, f) in &frames {
        span.entry(f.ticks())
            .and_modify(|s| s.1 = n)
            .or_insert((n, n));
    }
    let (&t0, &(f0, _)) = span.iter().next().unwrap();
    let first = &frames[&f0];
    let mut p = Player::new(first.u8(0x1A57), START);
    p.view.sub_x = first.i16(0x3A3A);
    p.view.sub_y = first.i16(0x3A3C);
    p.heading = Heading::from_code(first.u8(0x1A56)).unwrap();
    p.target = Heading::from_code(first.u8(0x1CBB)).unwrap();
    let mut f = Flight::with_player(1, p);
    f.ticks = t0;
    // Page before tick t0's flip = page after it (first frame of t0 + 1), flipped back.
    f.draw_page = frames[&span[&(t0 + 1)].0].u8(0x3A30) ^ 1;
    assert_eq!((f.weapon, f.missiles), (first.u8(0x1CB8), first.u8(0x1CB9)));

    let last = *span.keys().last().unwrap();
    let mut down: BTreeMap<FlightKey, bool> = BTreeMap::new();
    let mut prev_end = f0;
    let (mut volleys, mut post_checked, mut pre_checked) = (0, 0, 0);
    for t in t0..last {
        let next_first = span[&(t + 1)].0;
        let mut c = TickControls::default();
        for &(frame, key, is_down) in &events {
            if frame >= prev_end && frame < next_first {
                down.insert(key, is_down);
                if is_down {
                    c.set_made(key);
                }
            }
        }
        for (&k, &d) in &down {
            if d {
                c.set_down(k);
            }
        }
        prev_end = next_first;
        let before = Snapshot::of(&f, &f.shots);
        assert_eq!(
            f.tick(&world, &c, &mut vec![]),
            TickResult::Done,
            "tick {t}"
        );
        let mid = Snapshot::of_mid(&before, &f.drawn.shots);
        let after = Snapshot::of(&f, &f.shots);
        volleys += f
            .shots
            .0
            .iter()
            .filter(|s| s.alive() && s.life == 1)
            .count();

        let (start, end) = span[&t];
        let pre_page = frames[&start].u8(0x3A30);
        for (&n, fr) in frames.range(start..=end) {
            let theirs = Snapshot::from_frame(fr);
            if fr.u8(0x3A30) == pre_page {
                // Before step 24: before or after this tick's step 6.
                assert!(
                    theirs.matches(&before, t) || theirs.matches(&mid, t),
                    "frame {n} (tick {t}, before the flip):
 original {theirs:?}
 ours before {before:?}
 ours mid {mid:?}"
                );
                pre_checked += 1;
            } else {
                assert!(
                    theirs.matches(&after, t),
                    "frame {n} (tick {t}, after the flip):
 original {theirs:?}
 ours {after:?}"
                );
                post_checked += 1;
            }
        }
    }
    assert!(volleys > 0, "the capture contains player shots");
    assert!(
        pre_checked > 200 && post_checked > 10,
        "compared {pre_checked} + {post_checked} frames"
    );
}

/// The compared part of the state: view offsets, weapon, missiles, cooldown and the player's
/// shots (free and foreign slots normalised to free).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Snapshot {
    sub: (i16, i16),
    weapon: (u8, u8, u8),
    shots: Vec<Shot>,
}

fn player_shots(shots: &[Shot]) -> Vec<Shot> {
    shots
        .iter()
        .map(|s| {
            if s.alive() && s.owner == 0 {
                *s
            } else {
                FREE_SHOT
            }
        })
        .collect()
}

impl Snapshot {
    fn of(f: &Flight, shots: &td_sim::flight::shots::Shots) -> Self {
        Self {
            sub: (f.player.view.sub_x, f.player.view.sub_y),
            weapon: (f.weapon, f.missiles, f.missile_cooldown),
            shots: player_shots(&shots.0),
        }
    }

    /// The state after step 6 of a tick: shots updated, everything else still as before it.
    fn of_mid(before: &Snapshot, drawn: &td_sim::flight::shots::Shots) -> Self {
        Self {
            shots: player_shots(&drawn.0),
            ..before.clone()
        }
    }

    fn from_frame(fr: &Frame) -> Self {
        let shots: Vec<Shot> = (0..SHOT_SLOTS).map(|i| fr.shot(i)).collect();
        Self {
            sub: (fr.i16(0x3A3A), fr.i16(0x3A3C)),
            weapon: (fr.u8(0x1CB8), fr.u8(0x1CB9), fr.u8(0x1CBA)),
            shots: player_shots(&shots),
        }
    }

    /// Equality, except that a just-spawned slot whose other fields match but whose offsets
    /// are still the previous shot's is accepted: a dump taken in the middle of a spawn (seen in
    /// C-0011). The next comparisons check those offsets after the shot has moved.
    fn matches(&self, ours: &Snapshot, t: u32) -> bool {
        if (self.sub, self.weapon) != (ours.sub, ours.weapon) {
            return false;
        }
        self.shots
            .iter()
            .zip(&ours.shots)
            .enumerate()
            .all(|(i, (s, o))| {
                let torn = s.alive()
                    && s.life == 1
                    && Shot {
                        off_x: o.off_x,
                        off_y: o.off_y,
                        ..*s
                    } == *o;
                if s != o && torn {
                    eprintln!("tick {t}: slot {i} torn in the dump, offsets not compared");
                }
                s == o || torn
            })
    }
}

const START: td_sim::flight::player::View = td_sim::flight::START_VIEW;

const FREE_SHOT: Shot = Shot {
    tile_x: 0xFF,
    tile_y: 0,
    off_x: 0,
    off_y: 0,
    heading: 0,
    kind: 0,
    life: 0,
    owner: 0,
};
