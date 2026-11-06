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
    /// The mission-logic part of E3 flight start (e3-mission-setup §4.1 steps 1 and 3, §3.2):
    /// comm pending, the trigger (`g_CutsceneRequest`, unused in E3) and the ticker are
    /// cleared, then `Msn_Setup` sets phase 1 (none in flight 7, OB-043; 100 in flight 17), the
    /// home marker 3Dh (not in the dead block 20) and draws the objective (ink 37 for flights
    /// 1–7, 10, 11, 21; 42 for 8, 9, 12–20).
    pub fn setup_e3(&mut self, flight: u8) {
        self.scripted = true;
        self.comm_pending = false;
        self.cutscene = false;
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

#[cfg(test)]
mod tests;
