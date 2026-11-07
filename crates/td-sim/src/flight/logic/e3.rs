//! Episode 3 mission logic: `Msn_UpdateA` (flights 1–9) and `Msn_UpdateB` (flights 10–19 and
//! 21) of `td3.exe` (tick steps 8–9), the comm viewer table (step 15), `Msn_E3TimedEvents`
//! (after the flight menu) and the teardown outcome tests.
//!
//! Spec: `docs/spec/sim/e3-mission-logic.md` rev 1 (section numbers below are its own) and
//! `docs/spec/sim/e3-mission-setup.md` rev 1 §3.2 (the mission-logic part of `Msn_Setup`).
//! Fidelity: T1. Confidence: flights 1 and 8 start state and first ticks CONFIRMED (C-0036,
//! C-0041), the rest LIKELY (static).
//!
//! Against E2 ([`super::e2`]): the primitives are E2's (the player of the range rules is the
//! player tile), every block starts with its base test, no rule reads the ticker, and the
//! timers of flights 5, 10, 12, 14, 16, 18 and 21 run in `Msn_E3TimedEvents`. No E3 rule and
//! no E3 event draws a random number.

use super::{INK, INK_ALERT, MARK_HOME, Message, MissionState, PHASE_DONE};
use crate::flight::Flight;
use crate::flight::ships::NO_SHIP;

/// `Msn_E3TimedEvents` (`td3:0C5D:959E`, §4.6): once per tick after the flight menu, before
/// the shield checks, in every flight and during the end sequence. Each tick condition is an
/// equality on the 32-bit tick count, so each fires in exactly one tick.
impl Flight {
    pub(in crate::flight) fn e3_timed_events(&mut self) {
        if !self.mission.scripted {
            return;
        }
        let (n, t) = (self.number, self.ticks);
        if n == 18 && t == 2160 {
            // Also the flag of F5 entry 21 (the point (28, 33)) := 1: presentation, §4.9; see
            // [`Flight::e3_point_offered`].
            self.set_phase(2);
        }
        if n == 12 && t == 6480 {
            // §4.6.3: the 27 cells get their `td4.map` bytes back.
            self.cells_off = false;
        }
        if n == 16 && self.phase() == 3 {
            // §4.6.4: the five-step magenta fade (presentation, `MissionState::tint`), then the
            // class freeze; restored at the next E3 setup (OB-028 fix-all).
            self.mission.tint = true;
            for c in 0..=8 {
                self.classes.max_speed[c] = 0;
            }
            for c in 0..=8 {
                self.classes.accel[c] = 0;
            }
            self.classes.max_speed[2] = 20;
            self.classes.accel[2] = 4;
            self.set_phase(5);
        }
        if n == 14 && t == 25 {
            self.spawn(&[6, 7, 8], 8);
            self.spawn(&[9, 10, 11], 7);
        }
        if n == 14 && t == 27 {
            self.set_phase(2);
        }
        if n == 14 && t == 75 {
            self.ships.0[10].armour = 0;
        }
        if n == 14 && t == 175 {
            self.ships.0[7].armour = 0;
        }
        if n == 5 && self.phase() < PHASE_DONE && self.class_of(2) == NO_SHIP {
            self.lost = true;
        }
        if n == 10 && t == 10 {
            self.set_phase(2);
        }
        if n == 10 && t == 300 {
            self.set_phase(3);
        }
        if n == 21 && t == 10 {
            self.set_phase(2);
        }
        if n == 10 && t == 10 {
            self.spawn(&[4], 2);
        }
    }

    /// The flag of F5 target-information entry 21 in E3 flight 18: 0 from setup, 1 from the
    /// end of tick 2160 (§4.9; presentation, the F5 screen offers the point (28, 33)).
    pub fn e3_point_offered(&self) -> bool {
        self.ticks >= 2160
    }
}

impl MissionState {
    /// The mission-logic part of E3 flight start (e3-mission-setup §4.1 steps 1, 3 and 9, §3.2):
    /// the clears of mission-logic rev 5 §3.3 ([`MissionState::clear_flight_start`]: comm
    /// pending, the trigger `g_CutsceneRequest` and the event counter, both unused in E3, the
    /// base-hit bytes, the ion timer, the indicator phase) and the ticker, then `Msn_Setup`
    /// sets phase 1 (none in flight 7, OB-043; 100 in flight 17), the home marker 3Dh (not in
    /// the dead block 20) and draws the objective (ink 37 for flights 1–7, 10, 11, 21; 42 for
    /// 8, 9, 12–20).
    pub fn setup_e3(&mut self, flight: u8) {
        self.scripted = true;
        self.clear_flight_start();
        self.transmission = 0;
        self.scene = 0;
        self.tint = false;
        self.ticker.pos = 0;
        let k = match flight {
            1..=11 => flight - 1,
            21 => 11,
            12..=20 => flight,
            _ => return,
        };
        match flight {
            7 => {}
            17 => self.phase = PHASE_DONE,
            _ => self.phase = 1,
        }
        if flight != 20 {
            self.home_marker = MARK_HOME;
        }
        let ink = if matches!(flight, 1..=7 | 10 | 11 | 21) {
            INK
        } else {
            INK_ALERT
        };
        self.ticker.post(Message::E3Objective(k), ink);
    }
}

use Message::{E3Flight1 as M1, E3Flight2 as M2};

/// The player as a slot in the range rules.
const PLAYER: usize = 0;

/// The comm viewer's E3 table (§4.5), code order: flight, phase it applies in, screen `n`,
/// phase after. Each flight appears once.
const COMMS: [(u8, u8, u8, u8); 4] = [(8, 2, 1, 3), (16, 2, 3, 3), (19, 2, 5, 3), (15, 2, 2, 3)];

/// The transmission the E3 comm viewer shows in `flight` at `phase` and the phase after it
/// (§4.5), or `None` when F6 only consumes the request. Screen 5 shows `e3.comms.04`
/// (episode-deltas §6).
pub fn transmission(flight: u8, phase: u8) -> Option<(u8, u8)> {
    COMMS
        .iter()
        .find(|c| c.0 == flight && c.1 == phase)
        .map(|c| (c.2, c.3))
}

/// Primitives with E3 shapes (§2.1, §4.3).
impl Flight {
    /// `succeeded := cond and phase = 100`.
    fn e3_base(&mut self, at: bool) {
        self.succeeded = at && self.phase() == PHASE_DONE;
    }

    /// View tile (x, y) and `viewSubY <= sub` (i16 compares; viewSubX is not tested).
    fn e3_at(&self, x: i16, y: i16, sub: i16) -> bool {
        let v = self.player.view;
        v.tile_x == x && v.tile_y == y && v.sub_y <= sub
    }

    /// `home17`: view (17, 164) and viewSubY ≤ 35.
    fn home17(&self) -> bool {
        self.e3_at(17, 164, 35)
    }

    /// The view tile is one of `area` (any sub-tile offset; flights 11 and 13).
    fn view_in(&self, area: &[(i16, i16)]) -> bool {
        let v = self.player.view;
        area.contains(&(v.tile_x, v.tile_y))
    }

    /// `engage a ⇄ b, r`: one range test, then both orders (§2.1).
    fn engage_both(&mut self, a: usize, b: usize, range: u8) {
        if self.near_live(a, b, range) {
            self.ships.order_attack(a - 1, b as u8);
            self.ships.order_attack(b - 1, a as u8);
        }
    }

    /// `exit s @ tile` for each slot of `slots`.
    fn exit_all(&mut self, slots: impl IntoIterator<Item = usize>, x: u8, y: u8) {
        let list: Vec<(usize, u8, u8)> = slots.into_iter().map(|s| (s, x, y)).collect();
        self.exits(&list);
    }
}

/// Steps 8–9: `Msn_UpdateA` (flights 1–9) and `Msn_UpdateB` (flights 10–19, 21).
impl Flight {
    pub(in crate::flight) fn e3_update(&mut self) {
        match self.number {
            1 => self.e3_flight_1(),
            2 => self.e3_flight_2(),
            3 => self.e3_flight_3(),
            4 => self.e3_flight_4(),
            5 => self.e3_flight_5(),
            6 => self.e3_flight_6(),
            7 => self.e3_flight_7(),
            8 => self.e3_flight_8(),
            9 => self.e3_flight_9(),
            10 => self.e3_flight_10(),
            11 => self.e3_flight_11(),
            21 => self.e3_flight_21(),
            12 => self.e3_flight_12(),
            13 => self.e3_flight_13(),
            14 => self.e3_flight_14(),
            15 => self.e3_flight_15(),
            16 => self.e3_flight_16(),
            17 => self.e3_flight_17(),
            18 => self.e3_flight_18(),
            19 => self.e3_flight_19(),
            _ => {}
        }
    }

    /// Base test of flights 1–5: view (167, 66), subY ≤ 31, phase 100 (§4.3).
    fn e3_base_east(&mut self) {
        let at = self.e3_at(167, 66, 31);
        self.e3_base(at);
    }

    /// Base test `home17 and phase = 100` (flights 7–10, 12, 15–17, 19).
    fn e3_base_home(&mut self) {
        let at = self.home17();
        self.e3_base(at);
    }

    /// §5.1 (OB-045: completion tests slots 8, 2, 11, 12 only).
    fn e3_flight_1(&mut self) {
        self.e3_base_east();
        if (self.dead(&[8]) || self.dead(&[9])) && self.phase() < 3 {
            self.set_phase(3);
            self.spawn(&[11, 12, 13, 14], 7);
        }
        if (self.dead(&[11]) || self.dead(&[12])) && self.phase() == 3 {
            self.set_phase(5);
            self.spawn(&[2, 3, 4], 7);
        }
        if self.phase() > 2 && self.dead(&[8, 2, 11, 12]) && self.phase() < PHASE_DONE {
            self.set_phase(PHASE_DONE);
            self.post(M1(0));
        }
        for s in 5..=14 {
            self.engage(s, &[PLAYER], 15);
        }
        self.exit_all(2..=6, 85, 1);
        self.exits(&[(8, 80, 199), (9, 78, 199)]);
        self.exit_all(11..=14, 199, 111);
    }

    /// §5.2.
    fn e3_flight_2(&mut self) {
        self.e3_base_east();
        for s in 9..=12 {
            self.engage(s, &[PLAYER], 10);
        }
        for s in 15..=17 {
            self.engage(s, &[PLAYER], 20);
        }
        self.exit_all(15..=17, 79, 2);
        for s in 2..=12 {
            self.exits(&[(s, 83, 199), (s, 84, 199), (s, 85, 199)]);
        }
        if self.dead_range(2, 7) && self.phase() < PHASE_DONE {
            self.set_phase(PHASE_DONE);
            self.post(M1(1));
        }
    }

    /// §5.3.
    fn e3_flight_3(&mut self) {
        self.e3_base_east();
        for s in 9..=14 {
            self.engage(s, &[PLAYER], 15);
        }
        for s in 6..=7 {
            self.engage(s, &[PLAYER], 5);
        }
        self.exit_all(2..=4, 85, 199);
        self.exit_all(6..=7, 83, 199);
        if self.dead_range(2, 4) && self.phase() < 2 {
            self.set_phase(2);
            self.post(M1(2));
        }
        if self.dead_range(9, 12) && self.phase() < PHASE_DONE {
            self.set_phase(PHASE_DONE);
            self.post(M1(3));
        }
    }

    /// §5.4.
    fn e3_flight_4(&mut self) {
        self.e3_base_east();
        for a in 2..=3 {
            for b in 9..=13 {
                self.engage_both(a, b, 15);
            }
            for b in 5..=7 {
                self.engage(a, &[b], 20);
            }
        }
        for s in 9..=11 {
            self.engage(s, &[PLAYER], 20);
        }
        for s in 12..=13 {
            self.engage(s, &[PLAYER], 10);
        }
        if self.dead_range(5, 7) && self.phase() < PHASE_DONE {
            self.set_phase(PHASE_DONE);
            self.post(M1(4));
        }
        self.exit_all(5..=7, 199, 111);
        self.exit_all(11..=13, 199, 111);
        self.exits(&[(9, 199, 110), (10, 199, 112)]);
    }

    /// §5.5 (the lost rule is in `Msn_E3TimedEvents`).
    fn e3_flight_5(&mut self) {
        self.e3_base_east();
        if self.slot_tile(2) == (80, 199) && self.live(2) {
            self.hide(&[2]);
            self.post(M1(5));
            self.set_phase(PHASE_DONE);
        }
        for a in 5..=11 {
            self.engage(a, &[3], 15);
            self.engage(a, &[PLAYER], 15);
            self.engage(a, &[2], 10);
            self.engage(3, &[a], 20);
        }
    }

    /// §5.6: base strip at the west limit, view X 6 and rows 101..114.
    fn e3_flight_6(&mut self) {
        let v = self.player.view;
        self.e3_base(v.tile_x == 6 && (101..=114).contains(&v.tile_y));
        self.exit_all(2..=10, 1, 108);
        if self.dead_range(2, 10) && self.phase() < PHASE_DONE {
            self.set_phase(PHASE_DONE);
            self.post(M1(6));
        }
        for a in 13..=17 {
            for b in 2..=10 {
                self.engage(a, &[b], 5);
            }
            for b in 6..=7 {
                self.engage(b, &[a], 15);
            }
            self.engage(a, &[PLAYER], 7);
        }
    }

    /// §5.7 (the phase is inherited, OB-043).
    fn e3_flight_7(&mut self) {
        self.e3_base_home();
        for s in 2..=10 {
            self.exits(&[(s, 22, 166), (s, 21, 166)]);
        }
        self.exits(&[(18, 1, 168)]);
        for a in 12..=18 {
            for b in 2..=10 {
                self.engage(a, &[b], 15);
            }
            self.engage(a, &[PLAYER], 15);
        }
        let flag = self.dead_range(2, 10) && self.phase() <= 99;
        if flag && self.phase() < PHASE_DONE {
            self.set_phase(PHASE_DONE);
            self.alert(M1(7));
        }
    }

    /// §5.8 (the transmission window is silent, OB-044).
    fn e3_flight_8(&mut self) {
        self.e3_base_home();
        if self.phase() > 1 {
            self.exit_all(1..=3, 22, 166);
        }
        if self.dead(&[1, 2, 3]) && self.phase() < PHASE_DONE {
            self.set_phase(PHASE_DONE);
            self.alert(M1(8));
        }
        if self.phase() == 1 && (4..=10).any(|s| self.near_live(s, PLAYER, 15)) {
            self.set_phase(2);
        }
        if self.phase() < 4 && (4..=10).any(|s| !self.live(s)) {
            self.set_phase(4);
        }
        if self.phase() > 3 {
            for a in 4..=10 {
                self.engage(a, &[1, 2, 3], 15);
                self.engage(a, &[PLAYER], 10);
            }
        }
        if self.phase() < 4 && (1..=3).any(|s| self.ships.0[s - 1].record.waypoint == (108, 115)) {
            self.set_phase(4);
        }
    }

    /// §5.9.
    fn e3_flight_9(&mut self) {
        self.e3_base_home();
        if self.dead(&[1, 2, 3]) && self.phase() == 1 {
            self.set_phase(PHASE_DONE);
            self.alert(M1(9));
        }
        self.exit_all(1..=6, 168, 199);
        self.exits(&[(7, 1, 169), (8, 101, 199), (9, 101, 199)]);
        for s in 4..=9 {
            self.engage(s, &[PLAYER], 20);
        }
    }

    /// §5.10: the medical truck; phases 4..40 count ticks.
    fn e3_flight_10(&mut self) {
        self.e3_base_home();
        if self.phase() > 1 {
            for a in 7..=13 {
                self.engage_both(a, 3, 40);
                self.engage_both(a, 4, 20);
                self.engage(a, &[PLAYER], 40);
                self.engage(a, &[5], 20);
            }
        }
        if self.phase() == 3 {
            self.spawn(&[5], 0);
            self.alert(M2(0));
            self.set_phase(4);
        }
        self.exits(&[(3, 199, 106), (4, 80, 1)]);
        if self.phase() > 3 && self.phase() < 40 {
            self.set_phase(self.phase() + 1);
        }
        if self.phase() > 38 {
            self.exits(&[(5, 22, 166)]);
        }
        if self.dead(&[5]) && self.phase() > 12 && self.phase() < PHASE_DONE {
            self.set_phase(PHASE_DONE);
            let home = self.slot_tile(5) == (22, 166);
            self.alert(M2(if home { 2 } else { 1 }));
        }
    }

    /// §5.11: positional, no phase test.
    fn e3_flight_11(&mut self) {
        self.succeeded = self.view_in(&[(160, 75), (161, 75), (161, 76), (160, 76)]);
        for a in 3..=15 {
            self.engage(a, &[PLAYER], 40);
        }
    }

    /// §5.21: home17 without a phase test; attacks from phase 2 (tick 11).
    fn e3_flight_21(&mut self) {
        self.succeeded = self.home17();
        if self.phase() == 2 {
            for a in 3..=15 {
                self.engage(a, &[PLAYER], 40);
            }
        }
    }

    /// §5.12.
    fn e3_flight_12(&mut self) {
        self.e3_base_home();
        if self.phase() < PHASE_DONE && self.dead_range(1, 6) {
            self.set_phase(PHASE_DONE);
            self.alert(M2(3));
        }
        for a in 8..=15 {
            self.engage(a, &[PLAYER], 40);
        }
        self.exit_all(1..=6, 93, 76);
        self.exits(&[
            (8, 1, 95),
            (9, 1, 95),
            (10, 93, 76),
            (11, 93, 76),
            (12, 1, 169),
            (13, 1, 169),
            (14, 81, 1),
            (15, 81, 1),
        ]);
    }

    /// §5.13: positional, no phase test.
    fn e3_flight_13(&mut self) {
        self.succeeded = self.view_in(&[(36, 44), (37, 44), (36, 45), (37, 45)]);
        for a in 3..=8 {
            self.engage(a, &[PLAYER], 20);
        }
        self.exits(&[(11, 199, 107), (12, 1, 18)]);
    }

    /// §5.14: home17 without a phase test; attacks from phase 2 (tick 28).
    fn e3_flight_14(&mut self) {
        self.succeeded = self.home17();
        if self.phase() > 1 {
            for a in 3..=17 {
                self.engage(a, &[PLAYER], 35);
            }
        }
        self.exits(&[(6, 1, 170), (7, 1, 168), (9, 1, 95), (10, 101, 199)]);
    }

    /// §5.15: Kreel (slot 1's exit is inert, OB-046).
    fn e3_flight_15(&mut self) {
        self.e3_base_home();
        if self.near_live(15, PLAYER, 55) && self.phase() == 1 {
            self.alert(M2(4));
            self.set_phase(2);
            self.mission.comm_pending = true;
        }
        if self.dead(&[15]) && self.phase() < PHASE_DONE {
            self.mission.comm_pending = false;
            self.alert(M2(5));
            self.set_phase(PHASE_DONE);
        }
        for a in 4..=15 {
            self.engage(a, &[PLAYER], 15);
        }
        self.exits(&[(1, 199, 168), (2, 199, 126)]);
    }

    /// §5.16: the convoy; enemies near an escort are sent after the player (OB-047); the
    /// exits are inert (OB-046). The forced transmission is step 15, the freeze the event
    /// routine.
    fn e3_flight_16(&mut self) {
        self.e3_base_home();
        if self.dead(&[2, 3, 4]) && self.phase() == 1 {
            self.alert(M2(4));
            self.set_phase(2);
            self.mission.comm_pending = true;
        }
        if self.phase() == 5 {
            self.alert(M2(6));
            self.set_phase(PHASE_DONE);
        }
        for a in 9..=14 {
            for escort in [6, 7] {
                if self.near_live(a, escort, 10) {
                    self.ships.order_attack(a - 1, PLAYER as u8);
                }
            }
            self.engage(6, &[a], 15);
            self.engage(7, &[a], 15);
            self.engage(a, &[PLAYER], 15);
        }
        self.exit_all(2..=4, 199, 168);
    }

    /// §5.17: phase 100 from setup; fly home.
    fn e3_flight_17(&mut self) {
        self.e3_base_home();
        for a in 3..=7 {
            self.engage(a, &[PLAYER], 15);
        }
    }

    /// §5.18: the timed wait (phase 2 at the end of tick 2160).
    fn e3_flight_18(&mut self) {
        let v = self.player.view;
        self.e3_base((v.tile_x, v.tile_y) == (23, 31));
        for a in 3..=9 {
            self.engage(a, &[PLAYER], 35);
        }
        if self.phase() == 2 {
            self.alert(M2(7));
            let all: Vec<usize> = (1..=20).collect();
            self.mark(&all, 0);
            self.set_phase(PHASE_DONE);
        }
    }

    /// §5.19: Osyluth.
    fn e3_flight_19(&mut self) {
        self.e3_base_home();
        if self.near_live(2, PLAYER, 25) && self.phase() == 1 {
            self.alert(M2(4));
            self.set_phase(2);
            self.mission.comm_pending = true;
        }
        self.engage(2, &[PLAYER], 20);
        if self.dead(&[2]) && self.phase() < PHASE_DONE {
            self.mission.comm_pending = false;
            self.alert(M2(8));
            self.set_phase(PHASE_DONE);
        }
    }
}

/// The teardown tests (§4.8, code order; tiles only, kept after removal).
impl Flight {
    pub(super) fn e3_outcome(&self) -> bool {
        let t = |s| self.slot_tile(s);
        match self.number {
            2 => !(2..=7).any(|s| t(s).1 == 199),
            3 => !(2..=4).any(|s| t(s).1 == 199),
            4 => !(5..=7).any(|s| t(s).0 == 199),
            5 => t(2).1 == 199,
            8 => (1..=3).all(|s| t(s) == (22, 166)),
            9 => !(1..=3).any(|s| t(s).1 == 199),
            10 => t(5) == (22, 166),
            12 => !(1..=6).any(|s| t(s) == (93, 76)),
            _ => true,
        }
    }
}

#[cfg(test)]
mod tests;
