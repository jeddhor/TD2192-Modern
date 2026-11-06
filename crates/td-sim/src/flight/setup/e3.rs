//! Episode 3 per-flight setup `Msn_Setup` (`td3:0C5D:6149`).
//!
//! Spec: `docs/spec/sim/e3-mission-setup.md` rev 1 §3.2 (one block per flight, dispatch order
//! 1..11, 21, 12..20; lists in the order the code writes them, a later write wins), §4.2 (store
//! order, flight-12 map cells) and §4.3 (the flight-17 class-table restore, applied to every
//! flight by the OB-028 fix-all policy, `docs/spec/original-bugs.md`). Fidelity: T1.
//! Confidence: flights 1 and 8 CONFIRMED (C-0036, C-0041), the rest static.
//!
//! Every block except 18 and 20 clears the markers of slots 1..20 before its own marker writes;
//! block 18 fills them with 32h instead (OB-052) and block 20 (dead: E3 never flies flight 20)
//! writes none. No block writes late draw, the palette mode or comm pending. The mission-logic
//! part (phase, home marker, objective) is [`crate::flight::logic::MissionState::setup_e3`];
//! the F5 target information (`Msn_LoadTargetInfo`, §3.4) is presentation and is not here.

use super::{SetupFlags, each, range};
use crate::flight::ships::{NO_SHIP, Ships};
use crate::flight::tables::ClassTables;

/// The 27 `td4.map` cells that block 12 sets to tile 26h (open, not a hazard), in code order,
/// with the value `Msn_E3TimedEvents` writes back at tick 6480 (the `td4.map` byte;
/// e3-mission-setup §4.2, e3-mission-logic §4.6.3).
pub const FLIGHT12_CELLS: [(u8, u8, u8); 27] = [
    (40, 121, 0x44),
    (40, 122, 0x45),
    (40, 123, 0x46),
    (40, 131, 0x44),
    (40, 132, 0x45),
    (40, 133, 0x46),
    (66, 128, 0x44),
    (66, 129, 0x45),
    (66, 130, 0x46),
    (66, 121, 0x44),
    (66, 122, 0x45),
    (66, 123, 0x46),
    (53, 137, 0x41),
    (54, 137, 0x42),
    (55, 137, 0x43),
    (62, 114, 0x41),
    (63, 114, 0x42),
    (64, 114, 0x43),
    (53, 114, 0x41),
    (54, 114, 0x42),
    (55, 114, 0x43),
    (53, 118, 0x41),
    (54, 118, 0x42),
    (55, 118, 0x43),
    (46, 118, 0x41),
    (47, 118, 0x42),
    (48, 118, 0x43),
];

/// The tile block 12 writes over [`FLIGHT12_CELLS`].
pub const FLIGHT12_TILE: u8 = 0x26;

/// The max-speed and accel stores of block 17 (§4.3, `td3:0C5D:6BF3..6C43`), in code order:
/// the image values of classes 0..8; class 9 is not written.
pub fn restore_class_tables(classes: &mut ClassTables) {
    classes.max_speed[0] = 24;
    for c in 1..=8 {
        classes.max_speed[c] = 28;
    }
    classes.max_speed[2] = 32;
    classes.max_speed[6] = 24;
    classes.max_speed[8] = 32;
    for c in 0..=8 {
        classes.accel[c] = 4;
    }
    classes.accel[0] = 2;
    classes.accel[6] = 2;
}

/// `Msn_Setup` for E3 flight `n`: the class-table restore (every flight, OB-028 fix-all),
/// slot classes, markers, armour, shields and aggression. Late draw and palette mode are
/// always 0 in E3 (§3.3).
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
    // OB-028 fix-all (user decision 2026-11-05): the original restores the tables only in
    // block 17 (first thing in the block, §4.3), so flight 16's phase-3 patch leaked into a
    // retry or resume of flight 16 and into any flight after a restore or New Game. Every
    // flight's setup now writes block 17's values; for flight 17 this is the original store.
    restore_class_tables(classes);
    match n {
        // OB-052: 32h where every other block clears.
        18 => marker(ships, &range(1, 20), M),
        // The dead flight-20 block has no marker writes.
        20 => {}
        _ => marker(ships, &range(1, 20), 0),
    }
    match n {
        1 => {
            marker(ships, &[2, 3, 4, 8, 9, 11, 12, 13, 14], M);
            class(ships, &range(2, 4), FF);
            class(ships, &range(11, 14), FF);
            aggr(ships, &range(2, 14), 200);
        }
        2 => {
            marker(ships, &range(2, 7), M);
            aggr(ships, &range(9, 18), 255);
            class(ships, &range(15, 17), 7);
        }
        3 => {
            marker(ships, &[2, 3, 4, 9, 10, 11, 12], M);
            aggr(ships, &range(6, 14), 255);
        }
        4 => {
            marker(ships, &range(5, 7), M);
            aggr(ships, &range(2, 15), 255);
        }
        5 => {
            marker(ships, &[2], M9);
            marker(ships, &range(5, 7), M);
            // Slot 3 (DS:39A0), not slot 2 (§5).
            class(ships, &[3], 2);
            armour(ships, &[2], 900);
            aggr(ships, &range(3, 12), 250);
            aggr(ships, &range(5, 12), 150);
        }
        6 => {
            marker(ships, &range(2, 10), M9);
            aggr(ships, &range(2, 17), 200);
        }
        7 => {
            // No phase store (OB-043, `MissionState::setup_e3`).
            marker(ships, &range(2, 10), M9);
            aggr(ships, &range(12, 18), 75);
        }
        8 => {
            marker(ships, &range(1, 3), M9);
            armour(ships, &range(1, 3), 100);
            aggr(ships, &range(4, 10), 255);
            class(ships, &range(11, 20), FF);
        }
        9 => {
            marker(ships, &range(1, 3), M);
            armour(ships, &range(1, 3), 100);
            aggr(ships, &range(4, 9), 255);
            class(ships, &range(10, 20), FF);
        }
        10 => {
            marker(ships, &[5], M9);
            marker(ships, &range(7, 10), M);
            aggr(ships, &range(3, 13), 255);
            armour(ships, &[5], 100);
            class(ships, &[5], FF);
            class(ships, &[4], FF);
        }
        11 | 21 => aggr(ships, &range(3, 15), 255),
        12 => {
            // The 27 map cells (`FLIGHT12_CELLS`) are the caller's: the map is not here.
            marker(ships, &range(1, 6), M);
            aggr(ships, &range(8, 15), 200);
        }
        13 => aggr(ships, &range(3, 8), 200),
        14 => {
            aggr(ships, &range(3, 17), 255);
            class(ships, &range(6, 11), FF);
        }
        15 => {
            marker(ships, &[15], M);
            aggr(ships, &range(4, 15), 255);
            // OB-048: the first hit sets shields > 60 to 0.
            each(ships, &[15], |s| s.shields = 1750);
        }
        16 => {
            marker(ships, &range(2, 4), M);
            aggr(ships, &range(6, 14), 255);
        }
        17 => aggr(ships, &range(3, 7), 255),
        18 => {
            marker(ships, &range(3, 6), M);
            aggr(ships, &range(3, 9), 255);
        }
        19 => {
            marker(ships, &[2], M);
            aggr(ships, &[2], 255);
            armour(ships, &[2], 500);
        }
        20 => {
            aggr(ships, &range(2, 11), 10);
            aggr(ships, &range(12, 15), 220);
        }
        _ => {}
    }
    SetupFlags {
        late_draw: false,
        palette_mode: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The flight-16 phase-3 patch (e3-mission-logic §4.6.4).
    fn freeze(c: &mut ClassTables) {
        for k in 0..=8 {
            c.max_speed[k] = 0;
            c.accel[k] = 0;
        }
        c.max_speed[2] = 20;
        c.accel[2] = 4;
    }

    /// OB-028 fix-all (user decision 2026-11-05): no flight inherits the flight-16 freeze, not
    /// a retry of flight 16 itself and not a flight after a restore or New Game.
    #[test]
    fn every_flight_setup_restores_the_class_tables() {
        for n in (1..=21).filter(|&n| n != 20) {
            let mut classes = ClassTables::default();
            freeze(&mut classes);
            msn_setup(n, &mut Ships::default(), &mut classes);
            assert_eq!(classes, ClassTables::default(), "flight {n}");
        }
    }

    /// Block 17's stores give exactly the image values (§4.3).
    #[test]
    fn the_block_17_values_are_the_image() {
        let mut classes = ClassTables::default();
        freeze(&mut classes);
        restore_class_tables(&mut classes);
        assert_eq!(classes, ClassTables::default());
    }

    /// Block 18 fills the markers with 32h (OB-052); block 20 writes none.
    #[test]
    fn marker_fills() {
        let mut ships = Ships::default();
        for s in &mut ships.0 {
            s.marker = 7;
        }
        let mut c = ClassTables::default();
        msn_setup(20, &mut ships, &mut c);
        assert!(ships.0.iter().all(|s| s.marker == 7));
        msn_setup(18, &mut ships, &mut c);
        assert!(ships.0.iter().all(|s| s.marker == 0x32));
        msn_setup(11, &mut ships, &mut c);
        assert!(ships.0.iter().all(|s| s.marker == 0));
    }
}
