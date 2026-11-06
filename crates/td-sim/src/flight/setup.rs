//! Per-flight setup `Msn_Setup` (`file0001:0E8B:60D8`) and the start view.
//!
//! Spec: `docs/spec/sim/mission-setup.md` rev 1 §3 (table 3.2, in the order the code writes;
//! a later write wins). Objective text, ticker and comm tables are presentation and mission
//! logic (RE-6) and are not here. Episode 2's blocks are in [`e2`], Episode 3's in [`e3`].

pub mod e2;
pub mod e3;

use super::ships::{NO_SHIP, Ship, Ships};
use super::tables::ClassTables;

/// What setup decides besides the ships and class tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetupFlags {
    /// `g_PlayerDrawLate` (flights 12, 15, 21).
    pub late_draw: bool,
    /// `g_PaletteMode` (1 = infra-red in flights 4 and 8).
    pub palette_mode: u8,
}

/// Applies `f` to the 1-based `slots`.
fn each(ships: &mut Ships, slots: &[usize], mut f: impl FnMut(&mut Ship)) {
    for &s in slots {
        f(&mut ships.0[s - 1]);
    }
}

fn range(a: usize, b: usize) -> Vec<usize> {
    (a..=b).collect()
}

/// `Msn_Setup` for flight `n`: slot classes, markers, armour, aggression, class-table patches.
pub fn msn_setup(n: u8, ships: &mut Ships, classes: &mut ClassTables) -> SetupFlags {
    let class = |ships: &mut Ships, slots: &[usize], c: u8| each(ships, slots, |s| s.class = c);
    let marker = |ships: &mut Ships, slots: &[usize], m: u8| each(ships, slots, |s| s.marker = m);
    let armour = |ships: &mut Ships, slots: &[usize], a: i16| each(ships, slots, |s| s.armour = a);
    let aggr = |ships: &mut Ships, slots: &[usize], a: u8| {
        each(ships, slots, |s| s.record.aggression = a);
    };
    let helicopter = |c: &mut ClassTables| {
        c.max_speed[3] = 0x20;
        c.accel[3] = 4;
    };
    const M: u8 = 0x32;
    const M9: u8 = 0x39;
    const FF: u8 = NO_SHIP;
    marker(ships, &range(1, 20), 0);
    let mut flags = SetupFlags {
        late_draw: false,
        palette_mode: 0,
    };
    match n {
        1 => {
            class(ships, &range(3, 8), FF);
            class(ships, &range(14, 20), 7);
            marker(ships, &range(1, 11), M);
            armour(ships, &range(1, 17), 3);
            armour(ships, &[12, 13], 1000);
            aggr(ships, &[8], 100);
            aggr(ships, &[11], 50);
            aggr(ships, &range(14, 20), 50);
        }
        2 => {
            class(ships, &range(8, 20), FF);
            class(ships, &range(3, 7), FF);
            class(ships, &range(18, 20), 7);
            marker(ships, &range(1, 7), M);
            marker(ships, &[2], M9);
            armour(ships, &range(18, 20), 4);
            armour(ships, &range(3, 7), 5);
            armour(ships, &[1], 5);
            aggr(ships, &range(18, 20), 25);
            aggr(ships, &[1], 100);
            aggr(ships, &[5], 50);
            aggr(ships, &[6, 7], 150);
        }
        3 => {
            class(ships, &[1], 4);
            class(ships, &range(5, 8), FF);
            class(ships, &[12], FF);
            marker(ships, &[4], M9);
            armour(ships, &[2, 3, 6, 7, 12], 6);
            aggr(ships, &[1], 150);
            aggr(ships, &[2, 3], 125);
            aggr(ships, &[6, 7, 12], 225);
            aggr(ships, &[19, 20], 50);
        }
        4 => {
            class(ships, &[1], 4);
            class(ships, &[4, 5, 7], FF);
            class(ships, &[8, 9], 1);
            marker(ships, &[2, 3, 4, 5, 7], M);
            armour(ships, &range(2, 5), 6);
            armour(ships, &[7], 7);
            aggr(ships, &[1, 2], 210);
            aggr(ships, &[3], 100);
            aggr(ships, &[4], 110);
            aggr(ships, &[5], 210);
            aggr(ships, &[7], 150);
            aggr(ships, &[19, 20], 50);
            flags.palette_mode = 1;
        }
        5 => {
            class(ships, &[2], 7);
            marker(ships, &[3, 4, 5, 7], M);
            marker(ships, &[1], M9);
            armour(ships, &[1], 250);
            aggr(ships, &[3, 4], 100);
            aggr(ships, &[5], 175);
            aggr(ships, &[7], 75);
            aggr(ships, &[19, 20], 50);
        }
        6 => {
            marker(ships, &[7, 8, 9, 3, 5], M);
            armour(ships, &[3], 50);
            aggr(ships, &[3], 255);
            aggr(ships, &[5], 150);
            aggr(ships, &range(7, 9), 75);
            aggr(ships, &[19, 20], 50);
        }
        7 => {
            class(ships, &range(2, 8), FF);
            marker(ships, &range(13, 20), 0);
            marker(ships, &range(1, 12), M);
            armour(ships, &[1], 50);
            aggr(ships, &[13, 14], 175);
        }
        8 => {
            class(ships, &[8, 9], FF);
            class(ships, &range(3, 6), 7);
            marker(ships, &[15, 16, 17, 18, 3, 4, 5, 6, 7, 8, 9, 12, 13], M);
            aggr(ships, &[1], 175);
            aggr(ships, &range(3, 6), 75);
            aggr(ships, &[8, 9], 200);
            aggr(ships, &[12, 13], 50);
            ships.0[11].sub_y = 31;
            flags.palette_mode = 1;
        }
        9 => {
            marker(ships, &range(1, 10), M);
            aggr(ships, &range(5, 12), 75);
            aggr(ships, &range(13, 18), 150);
            ships.0[17].record.path[499] = (199, 121);
        }
        10 => {
            marker(ships, &range(7, 12), M);
            armour(ships, &[12], 250);
            aggr(ships, &range(7, 11), 95);
            aggr(ships, &[12], 180);
            aggr(ships, &[1], 150);
        }
        11 => {
            class(ships, &range(7, 9), FF);
            class(ships, &range(1, 3), 2);
            marker(ships, &[1, 2, 3, 7], M);
            armour(ships, &range(1, 3), 100);
            armour(ships, &[7], 150);
            aggr(ships, &range(1, 3), 195);
            aggr(ships, &[4, 5], 25);
            aggr(ships, &range(7, 9), 255);
            aggr(ships, &[17, 18], 50);
        }
        12 | 21 => {
            if n == 12 {
                marker(ships, &range(1, 5), M9);
            }
            aggr(ships, &range(7, 13), 115);
            aggr(ships, &[15, 16], 55);
            helicopter(classes);
            flags.late_draw = true;
        }
        13 | 14 => {
            if n == 13 {
                class(ships, &range(15, 17), 3);
                marker(ships, &[8, 9, 11, 12, 13, 15, 16, 17], M);
            } else {
                marker(ships, &[8, 9, 10, 12, 13], M);
            }
            aggr(ships, &[15, 16, 17, 18, 11, 12, 13], 195);
            aggr(ships, &[8, 9], 150);
            aggr(ships, &range(1, if n == 13 { 2 } else { 6 }), 75);
        }
        15 => {
            class(ships, &range(8, 12), FF);
            marker(ships, &[1, 2, 3, 4, 5, 8, 9, 10, 11, 12], M);
            aggr(ships, &range(3, 12), 125);
            aggr(ships, &[15], 200);
            aggr(ships, &[16], 125);
            helicopter(classes);
            flags.late_draw = true;
        }
        16 => {
            class(ships, &range(8, 13), FF);
            marker(ships, &[1, 2, 3, 8, 9, 10], M);
            aggr(ships, &[4, 5, 6, 11, 12, 13], 125);
            aggr(ships, &[17], 200);
        }
        17 => {
            class(ships, &range(5, 14), FF);
            marker(ships, &[1, 5, 6, 8, 9, 10, 12, 13, 14], M);
            armour(ships, &range(5, 14), 4);
            aggr(ships, &[3], 150);
            aggr(ships, &range(5, 14), 25);
        }
        18 => {
            marker(ships, &[7, 10, 11, 15, 17, 18], M);
            armour(ships, &[3], 250);
            aggr(ships, &[1, 5, 6, 9, 13, 14], 255);
        }
        19 => aggr(ships, &range(1, 10), 250),
        20 => {
            class(ships, &[3], 4);
            class(ships, &[4], 5);
            class(ships, &[5], 6);
            marker(ships, &[5], M);
            armour(ships, &[7, 8], 250);
            armour(ships, &[5], 500);
            aggr(ships, &[7, 8], 250);
            classes.shields[5] = 64;
            for c in [0, 1, 3, 4, 5] {
                classes.max_speed[c] = 0x11;
            }
        }
        _ => {}
    }
    flags
}
