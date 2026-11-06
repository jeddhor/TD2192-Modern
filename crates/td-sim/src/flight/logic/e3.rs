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
