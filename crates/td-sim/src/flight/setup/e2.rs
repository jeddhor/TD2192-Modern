//! Episode 2 per-flight setup `Msn_Setup` (`td2:0E96:60F9`).
//!
//! Spec: `docs/spec/sim/e2-mission-setup.md` rev 1 §3.2 (one block per flight 1..=20; lists in
//! the order the code writes them, a later write wins), §4.2. Every block clears the markers of
//! slots 1..=20 before its own marker writes, so clearing them first gives the same end state.
//! The F5 target information (`Msn_LoadTargetInfo`, §3.4) is presentation and is not here; the
//! mission-logic part (phase, home marker, objective, comm pending) is
//! [`crate::flight::logic::MissionState::setup_e2`].

use super::{SetupFlags, each, range};
use crate::flight::ships::{NO_SHIP, Ships};
use crate::flight::tables::ClassTables;

/// `Msn_Setup` for E2 flight `n`: slot classes, markers, armour, aggression, the flight-8
/// class-table patch (OB-018), late draw and the start palette mode.
pub fn msn_setup(n: u8, ships: &mut Ships, classes: &mut ClassTables) -> SetupFlags {
    let class = |ships: &mut Ships, slots: &[usize], c: u8| each(ships, slots, |s| s.class = c);
    let marker = |ships: &mut Ships, slots: &[usize], m: u8| each(ships, slots, |s| s.marker = m);
    let armour = |ships: &mut Ships, slots: &[usize], a: i16| each(ships, slots, |s| s.armour = a);
    let aggr = |ships: &mut Ships, slots: &[usize], a: u8| {
        each(ships, slots, |s| s.record.aggression = a);
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
            marker(ships, &range(6, 13), M);
            marker(ships, &[1], M);
            armour(ships, &[1], 100);
            aggr(ships, &range(3, 13), 150);
            aggr(ships, &[1], 255);
        }
        2 => {
            // Slot 8 is empty in `m2.td2` and becomes a class-3 ship at (0, 0) (OB-035).
            class(ships, &range(4, 8), 3);
            marker(ships, &range(4, 8), M);
            marker(ships, &range(12, 18), M);
            aggr(ships, &range(1, 18), 155);
        }
        3 => {
            marker(ships, &range(3, 8), M);
            marker(ships, &[1], M9);
            armour(ships, &[1], 400);
            aggr(ships, &[3], 50);
            aggr(ships, &[4], 75);
            aggr(ships, &[3], 25);
            aggr(ships, &[5, 6], 125);
            aggr(ships, &[7], 55);
            aggr(ships, &[8], 25);
            flags.palette_mode = 1;
        }
        4 => {
            class(ships, &range(1, 3), 0);
            class(ships, &range(8, 11), 7);
            class(ships, &range(14, 16), 3);
            marker(ships, &range(1, 3), M);
            aggr(ships, &[8, 9], 210);
            aggr(ships, &[10], 100);
            aggr(ships, &[11], 110);
            aggr(ships, &range(14, 16), 210);
        }
        5 => {
            marker(ships, &[1, 2, 3, 5, 6], M);
            aggr(ships, &range(5, 12), 150);
        }
        6 => {
            class(ships, &[13], 7);
            aggr(ships, &range(2, 11), 200);
        }
        7 => {
            // The player armour := 2 of this block is a dead store (§5).
            armour(ships, &[2], 75);
            aggr(ships, &range(3, 13), 195);
            aggr(ships, &[2], 250);
        }
        8 => {
            class(ships, &[4], 4);
            class(ships, &range(5, 11), FF);
            marker(ships, &range(1, 11), M9);
            marker(ships, &range(15, 20), M);
            armour(ships, &[2, 3], 200);
            armour(ships, &[5, 6], 125);
            armour(ships, &range(8, 10), 90);
            aggr(ships, &[4, 7, 19, 11, 15, 16, 17], 115);
            aggr(ships, &[18], 55);
            flags.late_draw = true;
            // The class tables of the player class (3), never undone (§4.3, OB-018).
            classes.max_speed[3] = 0x20;
            classes.accel[3] = 4;
        }
        9 => {
            marker(ships, &[5], M);
            armour(ships, &[5], 500);
            aggr(ships, &[2, 3], 200);
            aggr(ships, &range(8, 15), 200);
        }
        10 => {
            class(ships, &range(14, 16), FF);
            marker(ships, &range(14, 16), M);
            marker(ships, &[2], M9);
            armour(ships, &[2], 500);
            armour(ships, &[8], 250);
            aggr(ships, &[4, 5, 6, 7, 15], 195);
            aggr(ships, &[16], 180);
            flags.palette_mode = 1;
        }
        11 => {
            class(ships, &range(6, 14), FF);
            aggr(ships, &range(2, 14), 200);
            marker(ships, &[2, 6, 11], M);
        }
        12 => {
            marker(ships, &range(2, 4), M9);
            aggr(ships, &range(8, 13), 200);
        }
        13 => {
            marker(ships, &[4, 5, 6, 8, 9, 10, 11, 13, 14, 15], M);
            aggr(ships, &range(4, 15), 225);
        }
        14 => {
            marker(ships, &[4, 5, 6, 7, 8, 10, 11, 12], M);
            aggr(ships, &range(4, 16), 195);
            aggr(ships, &[2], 255);
        }
        15 => {
            marker(ships, &[2], M);
            aggr(ships, &range(4, 7), 125);
            aggr(ships, &[2], 200);
        }
        16 => {
            marker(ships, &[2], M);
            armour(ships, &[2], 500);
            armour(ships, &[10], 200);
            aggr(ships, &range(5, 12), 225);
        }
        17 => {
            class(ships, &[12], 6);
            marker(ships, &[2, 3], M9);
            // Five times the armour set from the file class (16-bit).
            for s in [2, 3] {
                let sh = &mut ships.0[s - 1];
                sh.armour = sh.armour.wrapping_mul(5);
            }
            aggr(ships, &[5, 6], 15);
            aggr(ships, &range(8, 14), 125);
        }
        18 => {
            // No aggression write: every ship keeps 0 (§5).
            class(ships, &range(9, 11), FF);
            marker(ships, &range(6, 11), M);
        }
        19 => {
            marker(ships, &[2], M);
            armour(ships, &[2], 1000);
            aggr(ships, &range(2, 7), 255);
        }
        20 => {
            marker(ships, &range(12, 15), M);
            aggr(ships, &range(2, 11), 10);
            aggr(ships, &range(12, 15), 220);
        }
        _ => {}
    }
    flags
}
