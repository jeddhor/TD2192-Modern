//! Episode 2 mission logic: `Msn_UpdateA` (flights 1–9) and `Msn_UpdateB` (flights 10–20) of
//! `td2.exe` (tick steps 8–9), the comm viewer table (step 15), `Msn_E2FlightEvents` (after the
//! flight menu) and the teardown outcome tests.
//!
//! Spec: `docs/spec/sim/e2-mission-logic.md` rev 1 (section numbers below are its own).
//! Fidelity: T1. Confidence: flight 1 tick 1 CONFIRMED (C-0015), the rest LIKELY (static).
//!
//! Against E1 ([`super`]): no respawns and no `Msn_UpdateEvents`; the player position of the
//! range rules is the player tile (view tile + (5, 3)); each flight has its own base test; a
//! ship destroyed by a projectile keeps its class until the AI pass (episode-deltas §5.1 P3).

use super::{INK, Message, PHASE_DONE};
use crate::flight::Flight;
use crate::flight::player::Event;

use Message::{E2Flight1 as M1, E2Flight2 as M2};

/// The player as a slot in the range rules.
const PLAYER: usize = 0;

/// The comm viewer's E2 table (§4.6): flight, phase it applies in, screen `n`, phase after.
/// Code order; each flight appears once.
const COMMS: [(u8, u8, u8, u8); 7] = [
    (1, 2, 1, 3),
    (7, 2, 3, PHASE_DONE),
    (11, 1, 4, 2),
    (19, 2, 5, 3),
    (15, 2, 6, 3),
    (16, 2, 7, 3),
    (5, 2, 2, 3),
];

/// The transmission the E2 comm viewer shows in `flight` at `phase` and the phase after it
/// (§4.6), or `None` when F6 only consumes the request. Screen `n` uses `e2.comms.0n`, except
/// 6 (`e2.comms.06` and `.07`) and 7 (`e2.comms.08`) (episode-deltas §10).
pub fn transmission(flight: u8, phase: u8) -> Option<(u8, u8)> {
    COMMS
        .iter()
        .find(|c| c.0 == flight && c.1 == phase)
        .map(|c| (c.2, c.3))
}

/// Primitives with E2 shapes.
impl Flight {
    /// `succeeded := cond and phase = 100` (§4.4).
    fn e2_base(&mut self, at: bool) {
        self.succeeded = at && self.phase() == PHASE_DONE;
    }

    /// View tile (x, y) and `viewSubY <= sub` (§4.4; i16 compares).
    fn at_view(&self, x: i16, y: i16, sub: i16) -> bool {
        let v = self.player.view;
        v.tile_x == x && v.tile_y == y && v.sub_y <= sub
    }

    /// `engage a → b, r` for each attacker in `attackers` and each target in `targets`, the
    /// attackers in the outer loop.
    fn engage_all(&mut self, attackers: &[usize], targets: &[usize], range: u8) {
        for &a in attackers {
            self.engage(a, targets, range);
        }
    }

    /// `engage a → b, r; engage b → a, r`.
    fn engage_pair(&mut self, a: usize, b: usize, range: u8) {
        self.engage(a, &[b], range);
        self.engage(b, &[a], range);
    }
}

/// Steps 8–9.
impl Flight {
    pub(in crate::flight) fn e2_update(&mut self) {
        match self.number {
            1 => self.e2_flight_1(),
            2 => self.e2_flight_2(),
            3 => self.e2_flight_3(),
            4 => self.e2_flight_4(),
            5 => self.e2_flight_5(),
            6 => self.e2_flight_6(),
            7 => self.e2_flight_7(),
            8 => self.e2_flight_8(),
            9 => self.e2_flight_9(),
            10 => self.e2_flight_10(),
            11 => self.e2_flight_11(),
            12 => self.e2_flight_12(),
            13 => self.e2_flight_13(),
            14 => self.e2_flight_14(),
            15 => self.e2_flight_15(),
            16 => self.e2_flight_16(),
            17 => self.e2_flight_17(),
            18 => self.e2_flight_18(),
            19 => self.e2_flight_19(),
            20 => self.e2_flight_20(),
            _ => {}
        }
    }

    /// §5.1.
    fn e2_flight_1(&mut self) {
        let at = self.at_view(102, 104, 36);
        self.e2_base(at);
        if self.dead(&[1]) && self.phase() < PHASE_DONE {
            self.set_phase(PHASE_DONE);
            self.post(M1(0));
        }
        if self.idle() && self.phase() == 1 && self.near_live(1, PLAYER, 15) {
            self.alert(M1(1));
            self.mission.comm_pending = true;
            self.set_phase(2);
        }
        if self.dead(&[1]) {
            self.mission.comm_pending = false;
        }
        self.engage(1, &[6, 7, 8, 9], 15);
        self.engage(1, &[PLAYER], 35);
        self.engage(3, &[6, 7, 8, 9, 10, 11, 12, 13], 20);
        self.engage(4, &[6, 7, 8, 9, 10, 11, 12, 13], 15);
        for (a, r) in [
            (6, 20),
            (7, 20),
            (8, 15),
            (9, 20),
            (10, 15),
            (11, 20),
            (12, 20),
            (13, 20),
        ] {
            let targets: &[usize] = if a == 8 { &[3, 4, 1] } else { &[1, 3, 4] };
            self.engage(a, targets, 15);
            self.engage(a, &[PLAYER], r);
        }
    }

    /// §5.2.
    fn e2_flight_2(&mut self) {
        let at = self.at_view(102, 104, 36);
        self.e2_base(at);
        for s in 4..=18 {
            self.engage(s, &[PLAYER], 15);
        }
        if self.dead_range(4, 8) && self.dead_range(12, 18) && self.phase() < PHASE_DONE {
            self.alert(M1(2));
            self.set_phase(PHASE_DONE);
        }
        for b in 4..=8 {
            self.engage_pair(1, b, 15);
        }
        for b in 4..=8 {
            self.engage_pair(2, b, 15);
        }
        for b in 12..=18 {
            self.engage_pair(2, b, 15);
        }
        for a in 4..=8 {
            for b in 12..=18 {
                self.engage_pair(a, b, 15);
            }
        }
    }

    /// §5.3.
    fn e2_flight_3(&mut self) {
        let at = self.at_view(102, 104, 36);
        self.e2_base(at);
        for (a, first, r1, second, r2) in [
            (3, 1, 7, PLAYER, 10),
            (4, PLAYER, 15, 1, 10),
            (5, PLAYER, 10, 1, 7),
            (6, PLAYER, 10, 1, 10),
            (7, 1, 10, PLAYER, 20),
            (8, PLAYER, 30, 1, 7),
        ] {
            self.engage(a, &[first], r1);
            self.engage(a, &[second], r2);
        }
        let (x, y) = self.slot_tile(1);
        if (85..=87).contains(&x) && y == 135 && self.live(1) {
            self.hide(&[1]);
            self.post(M1(3));
            let s = &mut self.ships.0[0];
            (s.tile_x, s.tile_y) = (84, 135);
            self.set_phase(PHASE_DONE);
        }
        if self.dead(&[1]) && self.phase() < PHASE_DONE {
            self.post(M1(4));
            self.set_phase(PHASE_DONE);
        }
    }

    /// §5.4.
    fn e2_flight_4(&mut self) {
        let v = self.player.view;
        self.e2_base((v.tile_x, v.tile_y) == (102, 104));
        if self.dead(&[1, 2, 3]) && self.phase() < PHASE_DONE {
            self.post(M1(5));
            self.set_phase(PHASE_DONE);
        }
        self.exits(&[
            (1, 1, 122),
            (2, 1, 122),
            (3, 1, 122),
            (8, 1, 122),
            (9, 1, 122),
            (10, 1, 123),
            (11, 1, 122),
            (14, 1, 122),
            (15, 1, 122),
            (16, 1, 123),
        ]);
        for a in 8..=11 {
            for b in 14..=16 {
                self.engage_pair(a, b, 15);
                self.engage(a, &[PLAYER], 15);
            }
        }
        self.engage_all(&[14, 15, 16], &[1, 2, 3], 20);
    }

    /// §5.5.
    fn e2_flight_5(&mut self) {
        let v = self.player.view;
        self.e2_base((178..=183).contains(&v.tile_x) && v.tile_y == 35);
        if self.dead(&[1, 2, 3]) && self.phase() < PHASE_DONE {
            self.post(M1(6));
            self.set_phase(PHASE_DONE);
        }
        if self.idle() && self.phase() == 1 {
            self.alert(M1(1));
            self.mission.comm_pending = true;
            self.set_phase(2);
        }
        for a in 5..=9 {
            self.engage(a, &[11], 20);
            self.engage(a, &[PLAYER], 15);
            self.engage(11, &[a], 15);
        }
        self.engage(11, &[1, 2, 3], 20);
    }

    /// §5.6 (no phase script; the base test has no phase condition).
    fn e2_flight_6(&mut self) {
        self.exits(&[(13, 124, 72), (15, 124, 72), (16, 124, 72)]);
        let v = self.player.view;
        self.succeeded = v.tile_y == 70 && (118..=123).contains(&v.tile_x);
        for a in 2..=11 {
            self.engage(a, &[13, 14, 15, 16], 15);
            self.engage(a, &[PLAYER], 20);
        }
    }

    /// §5.7 (the base test has no phase condition).
    fn e2_flight_7(&mut self) {
        self.succeeded = self.at_view(102, 104, 35);
        if self.dead(&[2]) {
            self.mission.comm_pending = false;
        }
        if self.idle() && self.phase() == 1 {
            self.alert(M1(1));
            self.mission.comm_pending = true;
            self.set_phase(2);
        }
        for a in 5..=13 {
            self.engage(a, &[PLAYER], 15);
        }
    }

    /// §5.8.
    fn e2_flight_8(&mut self) {
        let x = self.player.view.tile_x;
        self.e2_base(x == 7);
        for a in 15..=19 {
            for b in (2..=11).rev() {
                self.engage(a, &[b], 20);
            }
            self.engage(4, &[a], 20);
            self.engage(7, &[a], 20);
            self.engage(11, &[a], 20);
        }
        if self.dead(&[2, 3]) && self.phase() == 1 {
            self.alert(M1(7));
            self.set_phase(2);
            self.spawn(&[5, 6], 0);
            self.spawn(&[7], 4);
        }
        if self.dead(&[5, 6]) && self.phase() == 2 {
            self.alert(M1(8));
            self.set_phase(3);
            self.spawn(&[8, 9, 10], 0);
            self.spawn(&[11], 4);
        }
        if self.dead(&[11, 10, 9, 8]) && self.phase() == 3 {
            self.set_phase(PHASE_DONE);
            self.alert(M1(9));
        }
        // Slot 11's rule tests (1, 122) although its path ends at (1, 123) (OB-036).
        self.exits(&[
            (2, 1, 122),
            (3, 1, 124),
            (4, 1, 122),
            (5, 1, 124),
            (6, 1, 122),
            (7, 1, 123),
            (8, 1, 122),
            (9, 1, 123),
            (10, 1, 124),
            (11, 1, 122),
        ]);
    }

    /// §5.9.
    fn e2_flight_9(&mut self) {
        let at = self.at_view(87, 28, 31);
        self.e2_base(at);
        if self.dead(&[5]) && self.phase() == 1 {
            self.set_phase(PHASE_DONE);
            self.alert(M1(10));
        }
        self.exits(&[(5, 177, 169)]);
        for a in 8..=15 {
            self.engage(a, &[2], 15);
            self.engage(a, &[PLAYER], 30);
            self.engage(2, &[a], 15);
        }
    }
}

/// `Msn_UpdateB` (flights 10–20, §5.10–§5.20). Every post here is written with its colour.
impl Flight {
    /// `base(87, 28)` of flights 10–19 (§5.10.0.2): view (87, 28), `viewSubY < 32`, phase 100.
    fn e2_base_hq(&mut self) {
        let at = self.at_view(87, 28, 31);
        self.e2_base(at);
    }

    /// §5.10 (phase 18 is a real value).
    fn e2_flight_10(&mut self) {
        self.e2_base_hq();
        if self.phase() == 1 && self.idle() {
            self.alert(M2(0));
            self.set_phase(18);
        }
        if self.dead(&[14]) && self.phase() == 2 {
            self.alert(M2(1));
            self.set_phase(PHASE_DONE);
        }
        if self.dead(&[2]) && self.phase() == 18 {
            self.alert(M2(2));
            self.set_phase(2);
            self.spawn(&[14], 7);
            self.spawn(&[15, 16], 0);
        }
        for s in 4..=8 {
            self.engage_pair(s, 3, 15);
            self.engage(s, &[PLAYER], 15);
            self.engage(s, &[2], 20);
        }
        for s in 14..=16 {
            self.engage(s, &[PLAYER], 15);
        }
        self.exits(&[
            (2, 149, 198),
            (2, 151, 198),
            (2, 150, 198),
            (2, 152, 198),
            (2, 153, 198),
            (14, 177, 175),
            (15, 177, 167),
            (16, 177, 169),
        ]);
    }

    /// §5.11 (D repeats C's spawn; preserved).
    fn e2_flight_11(&mut self) {
        self.e2_base_hq();
        if self.phase() == 2 {
            self.alert(M2(3));
            self.set_phase(3);
        }
        if self.dead(&[2]) && self.phase() <= 3 {
            self.set_phase(4);
            self.alert(M2(4));
            self.spawn(&[6], 0);
            self.spawn(&[7, 8, 9], 7);
        }
        if self.dead(&[6]) && self.phase() == 4 {
            self.set_phase(5);
            self.alert(M2(5));
            self.spawn(&[11], 0);
            self.spawn(&[12], 7);
        }
        if self.dead(&[11]) && self.phase() == 5 {
            self.set_phase(PHASE_DONE);
            self.alert(M2(6));
            self.spawn(&[11], 0);
            self.spawn(&[12], 7);
        }
        self.exits(&[
            (2, 177, 169),
            (3, 151, 199),
            (4, 150, 199),
            (6, 177, 167),
            (7, 151, 199),
            (8, 149, 1),
            (9, 177, 167),
            (11, 177, 169),
            (12, 177, 189),
        ]);
        self.engage_all(&[3, 4, 7, 8, 9, 12], &[PLAYER], 15);
    }

    /// §5.12.
    fn e2_flight_12(&mut self) {
        self.e2_base_hq();
        self.engage_all(&[8, 9, 10, 11, 12, 13], &[PLAYER], 15);
        if self.dead(&[2, 3, 4]) && self.phase() < PHASE_DONE {
            self.set_phase(PHASE_DONE);
            self.alert(M2(7));
        }
        self.exits(&[(2, 92, 29), (3, 92, 29), (4, 92, 29)]);
    }

    /// §5.13: one `Random(9)` every tick; slot 2 fires along its heading whatever its class
    /// (OB-039).
    fn e2_flight_13(&mut self) {
        self.e2_base_hq();
        let h = self.rng.random(9) as u8;
        let s = &mut self.ships.0[1];
        s.target_heading = h;
        if (1..10).contains(&s.heading) {
            s.fire[usize::from(s.heading) - 1] = true;
        }
        if self.idle() && self.phase() == 1 {
            self.set_phase(PHASE_DONE);
            self.alert(M2(8));
        }
        self.engage_all(&(4..=15).collect::<Vec<_>>(), &[PLAYER], 15);
    }

    /// §5.14.
    fn e2_flight_14(&mut self) {
        self.e2_base_hq();
        if self.idle() && self.phase() == 1 && self.dead(&[4, 5, 6, 7, 8, 10, 11, 12]) {
            self.set_phase(PHASE_DONE);
            self.alert(M2(9));
        }
        for s in 4..=13 {
            self.engage_pair(s, 2, 15);
            self.engage(s, &[PLAYER], 20);
        }
    }

    /// §5.15.
    fn e2_flight_15(&mut self) {
        self.e2_base_hq();
        if self.phase() == 1 && self.idle() {
            self.alert(M2(10));
            self.mission.comm_pending = true;
            self.set_phase(2);
        }
        if self.dead(&[2]) {
            self.mission.comm_pending = false;
        }
        if self.dead(&[2]) && self.phase() < PHASE_DONE {
            self.set_phase(PHASE_DONE);
            self.alert(M2(11));
        }
        self.engage(2, &[10, 11, 12], 25);
        self.engage_all(&[2, 3, 4, 5, 6, 7], &[PLAYER], 15);
    }

    /// §5.16 (C tests `ticker = 1`, OB-038).
    fn e2_flight_16(&mut self) {
        self.e2_base_hq();
        self.exits(&[(2, 177, 168)]);
        if self.dead(&[2]) && self.slot_tile(2) == (177, 168) && self.phase() < PHASE_DONE {
            self.set_phase(PHASE_DONE);
            self.alert(M2(12));
        }
        if self.dead(&[2]) && self.phase() < PHASE_DONE {
            self.set_phase(PHASE_DONE);
            self.alert(M2(13));
        }
        if self.near_live(PLAYER, 10, 25) && self.phase() == 1 && self.mission.ticker.pos == 1 {
            self.alert(M2(10));
            self.mission.comm_pending = true;
            self.set_phase(2);
        }
        if self.dead(&[10]) {
            self.mission.comm_pending = false;
        }
        self.engage_all(&[5, 6, 7, 8, 9, 10], &[PLAYER], 15);
    }

    /// §5.17.
    fn e2_flight_17(&mut self) {
        self.e2_base_hq();
        let through = self.slot_tile(2).0 == 1 || self.slot_tile(3).0 == 1;
        if self.phase() == 1 && self.dead(&[2]) && self.dead(&[3]) && through {
            self.set_phase(PHASE_DONE);
            self.alert(M2(14));
        }
        if self.phase() == 1 && self.dead(&[2, 3]) {
            self.set_phase(2);
            self.alert(M2(15));
        }
        if self.phase() == 2 && self.idle() {
            self.mission.cutscene = true;
        }
        for a in 8..=14 {
            self.engage(a, &[PLAYER], 15);
            self.engage(a, &[5, 6], 20);
            for b in [5, 6] {
                self.engage(b, &[a], 5);
            }
            self.engage(a, &[2, 3], 25);
        }
        self.exits(&[(2, 1, 163), (3, 1, 163), (5, 1, 163), (6, 1, 163)]);
    }

    /// §5.18 (`allGone` is the scratch byte DS:31C5, kept in `mission.count`).
    fn e2_flight_18(&mut self) {
        self.e2_base_hq();
        if self.dead(&[6, 7, 8]) && self.phase() == 1 {
            self.spawn(&[9, 10, 11], 6);
            self.post(M2(16));
            self.set_phase(2);
        }
        let all_gone = self.dead_range(6, 11);
        self.mission.count = u8::from(all_gone);
        if all_gone && self.phase() < PHASE_DONE {
            self.alert(M2(17));
            self.set_phase(PHASE_DONE);
        }
        // No class or phase test (U-200, U-247).
        if (6..=11).any(|s| self.slot_tile(s) == (92, 31)) {
            self.mission.cutscene = true;
        }
        self.engage_all(&[2, 3, 4], &[6, 7, 8, 9, 10, 11], 15);
    }

    /// §5.19 (the announcement has no ticker test).
    fn e2_flight_19(&mut self) {
        self.e2_base_hq();
        if self.phase() == 1 && self.near_live(2, PLAYER, 45) {
            self.alert(M2(10));
            self.mission.comm_pending = true;
            self.set_phase(2);
        }
        if self.dead(&[2]) {
            self.mission.comm_pending = false;
        }
        if self.dead(&[2]) && self.phase() < PHASE_DONE {
            self.alert(M2(18));
            self.set_phase(PHASE_DONE);
        }
        self.engage(2, &[PLAYER], 20);
        self.engage_all(&[3, 4, 5, 6, 7], &[PLAYER], 30);
    }

    /// §5.20 (no base test: phase 100 and view Y ≤ 7; `succeeded` is never cleared).
    fn e2_flight_20(&mut self) {
        if self.phase() == PHASE_DONE && self.player.view.tile_y <= 7 {
            self.succeeded = true;
        }
        let exits: Vec<(usize, u8, u8)> = (2..=10).chain(17..=20).map(|s| (s, 148, 1)).collect();
        self.exits(&exits);
        for a in 2..=7 {
            for b in 12..=15 {
                self.engage(a, &[b], 10);
                self.engage(b, &[a], 15);
            }
        }
        for b in 12..=15 {
            self.engage(b, &[10], 15);
        }
        for a in 17..=20 {
            for b in 12..=15 {
                self.engage(b, &[a], 15);
            }
        }
        let all_gone = self.dead_range(12, 15);
        self.mission.count = u8::from(all_gone);
        if all_gone && self.phase() < PHASE_DONE {
            self.alert(M2(19));
            self.set_phase(PHASE_DONE);
        }
    }
}

/// `Msn_E2FlightEvents` (`td2:0E96:99B8`, §4.7.1) and the teardown tests (§4.8).
impl Flight {
    /// Runs every tick after the flight menu, before the shield checks; acts in flights 17
    /// and 18 only.
    pub(in crate::flight) fn e2_flight_events(&mut self, events: &mut Vec<Event>) {
        if !self.mission.scripted {
            return;
        }
        let trigger = self.mission.cutscene;
        if self.number == 18 && trigger && self.mission.debris < 2 {
            self.mission.debris += 1;
            self.debris(events);
        }
        if self.number == 18 && self.mission.debris >= 1 {
            self.mission.debris += 1;
            self.debris(events);
            if self.mission.debris == 5 {
                self.mission.ticker.post(Message::E2Events, INK);
            }
            if self.mission.debris >= 150 {
                self.lost = true;
                events.push(Event::Sound {
                    kind: 100,
                    source: Some((90, 29)),
                });
                // Hit flash := 0, the whole DAC and the palette copy white (presentation).
                self.mission.white_out = true;
            }
        }
        if self.number == 17 && trigger {
            self.lost = true;
        }
    }

    /// One debris explosion in the lowest idle slot of 1..=20, with its sound and four draws
    /// (offset X, offset Y, tile X, tile Y); nothing without an idle slot.
    fn debris(&mut self, events: &mut Vec<Event>) {
        let Some(i) = self.explosions.idle_hit_slot() else {
            return;
        };
        events.push(Event::Sound {
            kind: 9,
            source: Some((90, 29)),
        });
        let sx = self.rng.random(48) as i16;
        let sy = self.rng.random(48) as i16;
        let tx = 91 + self.rng.random(2) as u8;
        let ty = 28 + self.rng.random(2) as u8;
        self.explosions.start_in(i, (tx, ty), (sx, sy));
    }

    /// `g_MissionOutcome` at teardown (§4.8, code order; tiles only, kept after removal).
    pub(super) fn e2_outcome(&self) -> bool {
        let t = |s| self.slot_tile(s);
        match self.number {
            3 => t(1) == (84, 135),
            // Compares slot 2's X with 177 and with 168: never both (OB-025).
            16 => true,
            8 => (2..=11).any(|s| t(s).0 == 1),
            9 => t(5) != (177, 169),
            10 => t(2) != (151, 199),
            11 => ![2, 6, 11].iter().any(|&s| t(s).0 == 177),
            17 => t(2).0 == 1 && t(3).0 == 1,
            _ => true,
        }
    }
}
