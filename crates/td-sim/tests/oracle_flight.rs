//! L5 parity: td-sim's player flight driven by the key bytes recorded from the original, tick
//! by tick, compared with the original's own state (captures C-0007 and C-0008, E1 mission 1;
//! DGROUP DS:1700..3A3F dumped every video frame). Needs TD2192_DATA and TD2192_ORACLE.
//!
//! Sampling (flight-model.md rev 2 §6): the state after tick t's physics is read from the
//! first frame whose tick counter (DS:1718) is t + 1; by then the original has usually also
//! run tick t + 1's turn step, which the comparison allows for. The key bytes physics reads
//! are taken from that same frame: physics runs at the very end of tick t, after the tick
//! wait, so the frame just after it holds the bytes it saw (the frame before can be stale at a
//! key release). The captures switch class 4 → 0 with a cheat
//! chord, which the port does not implement; the harness copies that change from the original.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use td_assets::DataDir;
use td_formats::map::TileMap;
use td_sim::flight::heading::Heading;
use td_sim::flight::player::{Player, Turn, View};
use td_sim::flight::shots::Muzzles;
use td_sim::flight::tables::episode1_tile_attributes;
use td_sim::flight::{TickResult, World};
use td_sim::{Flight, FlightKey, TickControls};
use td_test_support::{require_data, require_oracle};

const BASE: usize = 0x1700;

struct Dump(Vec<u8>);

impl Dump {
    fn u8(&self, ds: usize) -> u8 {
        self.0[ds - BASE]
    }
    fn i16(&self, ds: usize) -> i16 {
        i16::from_le_bytes([self.0[ds - BASE], self.0[ds - BASE + 1]])
    }
    fn ticks(&self) -> u32 {
        u32::from_le_bytes(self.0[0x18..0x1C].try_into().unwrap())
    }
    fn key(&self, scan: u16) -> bool {
        self.u8(0x1A5C + usize::from(scan)) != 0
    }
    fn view(&self) -> View {
        View {
            tile_x: self.i16(0x3A36),
            tile_y: self.i16(0x3A38),
            sub_x: self.i16(0x3A3A),
            sub_y: self.i16(0x3A3C),
        }
    }
    fn speeds(&self) -> [u8; 4] {
        [
            self.u8(0x3995),
            self.u8(0x3996),
            self.u8(0x3997),
            self.u8(0x3998),
        ]
    }
    fn heading(&self) -> u8 {
        self.u8(0x1A56)
    }
    fn class(&self) -> u8 {
        self.u8(0x1A57)
    }
    fn turn(&self) -> u8 {
        self.u8(0x1A5A)
    }
    fn target(&self) -> u8 {
        self.u8(0x1CBB)
    }
}

/// First and last dump of every tick.
fn load(dir: &Path) -> BTreeMap<u32, (Dump, Dump)> {
    let mut names: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.to_string_lossy().ends_with("_ds.bin"))
        .collect();
    names.sort();
    let mut ticks: BTreeMap<u32, (Dump, Dump)> = BTreeMap::new();
    for p in names {
        let d = Dump(fs::read(&p).unwrap());
        let t = d.ticks();
        match ticks.get_mut(&t) {
            Some(e) => e.1 = d,
            None => {
                ticks.insert(t, (Dump(d.0.clone()), d));
            }
        }
    }
    ticks
}

fn heading(code: u8) -> Heading {
    Heading::from_code(code).expect("valid heading")
}

fn run(capture: &str, data: &Path, oracle: &Path) -> usize {
    let dir = DataDir::open(data).unwrap();
    let world = World {
        map: TileMap::parse_episode(&dir.read("td.map").unwrap()).unwrap(),
        tile_attr: episode1_tile_attributes(),
        muzzles: Muzzles(vec![0; 342]),
    };
    let ticks = load(&oracle.join(capture).join("mem"));
    let (&t0, (first, _)) = ticks.iter().next().unwrap();
    assert_eq!(
        first.heading(),
        first.target(),
        "start on a settled heading"
    );
    let mut p = Player::new(first.class(), first.view());
    p.heading = heading(first.heading());
    p.target = heading(first.target());
    [p.speed.up, p.speed.down, p.speed.left, p.speed.right] = first.speeds();
    let mut f = Flight::with_player(1, p);
    f.ticks = t0;
    f.draw_page = first.u8(0x3A30);
    let keys = [
        FlightKey::Kp8,
        FlightKey::Up,
        FlightKey::Kp2,
        FlightKey::Down,
        FlightKey::Kp4,
        FlightKey::Left,
        FlightKey::Kp6,
        FlightKey::Right,
    ];
    let mut compared = 0;
    let last = *ticks.keys().last().unwrap();
    for t in t0..last {
        let (Some(_), Some((next, _))) = (ticks.get(&t), ticks.get(&(t + 1))) else {
            panic!("{capture}: tick {t} or {} missing", t + 1);
        };
        let mut c = TickControls::default();
        for k in keys {
            if next.key(k.scan_code()) {
                c.set_down(k);
                c.set_made(k);
            }
        }
        assert_eq!(f.tick(&world, &c, &mut vec![]), TickResult::Done);
        // Class change by the cheat chord (step 34): copy it from the original.
        if next.class() != f.player.class {
            f.player.class = next.class();
            f.player.armour = next.u8(0x398E) as i8;
            f.player.shields = next.u8(0x398F) as i8;
        }
        let s = &f.player;
        let ours = (
            s.view,
            [s.speed.up, s.speed.down, s.speed.left, s.speed.right],
            s.target.code(),
            s.turn as u8,
        );
        let theirs = (next.view(), next.speeds(), next.target(), next.turn());
        assert_eq!(ours, theirs, "{capture}: state after tick {t}");
        // The original's frame may already include tick t+1's turn step.
        let stepped = match (s.target != s.heading, s.turn) {
            (true, Turn::Clockwise) => s.heading.cw(),
            (true, Turn::CounterClockwise) => s.heading.ccw(),
            _ => s.heading,
        };
        assert!(
            next.heading() == s.heading.code() || next.heading() == stepped.code(),
            "{capture}: heading after tick {t}: ours {} (next {}), original {}",
            s.heading.code(),
            stepped.code(),
            next.heading()
        );
        compared += 1;
    }
    compared
}

#[test]
fn flight_matches_the_original_tick_by_tick() {
    let data = require_data!();
    let oracle = require_oracle!();
    let n7 = run("C-0007", &data, &oracle);
    let n8 = run("C-0008", &data, &oracle);
    assert!(n7 + n8 > 200, "compared {n7} + {n8} ticks");
}
