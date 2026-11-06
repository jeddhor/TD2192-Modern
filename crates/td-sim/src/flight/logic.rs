//! Mission logic: the per-flight rules (`Msn_UpdateA` flights 1–9, `Msn_UpdateB` flights
//! 10–20, tick steps 8–9), `Msn_UpdateEvents` (step 18), the comm viewer's phase advances
//! (step 15), the flight-7 rule of the guidance routine and the HUD ticker and transmission
//! indicator steps (step 19), and the teardown outcome tests.
//!
//! Spec: `docs/spec/sim/mission-logic.md` rev 1 (section numbers below are its own),
//! `docs/spec/render/hud.md` rev 2 §4.3, §4.7, §4.8 (the ticker step decides the "ticker idle"
//! gate of the rules, so it is simulation state) and `docs/spec/sim/flight-model.md` rev 2 §4.4
//! (edge alert). Fidelity: T1. Confidence: flight 1 CONFIRMED (C-0010), flights 2–21 LIKELY.
//!
//! Slots are 1-based here, as in the spec (`Ships::0[s - 1]`); slot 0 in a range rule is the
//! player, whose position is the **view tile** (§2). Messages are carried as resource ids
//! ([`Message`]); resolving text is the renderer's job.
//!
//! Episode 2 (`docs/spec/sim/e2-mission-logic.md` rev 1) is in [`e2`]; the primitives here
//! serve both, and the episode-specific parts below branch on [`Flight::episode`].

use super::player::Event;
use super::ships::{NO_SHIP, SHIP_SLOTS};
use super::shots::Shot;
use super::{Episode, Flight};
use crate::hash::{HashState, StateHasher};
use crate::real48::Real48;

/// Default ticker ink.
pub const INK: u8 = 37;
/// Ticker ink of alerts and of the objectives of flights 12–21.
pub const INK_ALERT: u8 = 42;
/// Marker: target (guidance colour class 32h).
pub const MARK_TARGET: u8 = 0x32;
/// Marker: friend.
pub const MARK_FRIEND: u8 = 0x39;
/// Marker: home point.
pub const MARK_HOME: u8 = 0x3D;
/// Last ticker position; the step after it returns to idle (hud §4.7).
pub const TICKER_LAST: u16 = 78;
/// Phase value of "objectives complete".
pub const PHASE_DONE: u8 = 100;
/// Sound effect of the transmission indicator (hud §4.8, "income").
pub const SOUND_INCOMING: u8 = 200;

/// A text drawn into the ticker strip, as a resource of `file0001` ([`Message::resource`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {
    /// `e1.objectives[k]`.
    Objective(u8),
    /// `e1.flight_messages.01[k]` (`M1[k]`).
    Flight1(u8),
    /// `e1.flight_messages.02[k]` (`M2[k]`).
    Flight2(u8),
    /// `jurisdiction.warnings[k]`.
    Jurisdiction(u8),
    /// `e1.flight_events.01[k]`.
    Events1(u8),
    /// `e2.objectives[k]` (`td2.exe`).
    E2Objective(u8),
    /// `e2.flight_messages.01[k]` (E2 `M1[k]`).
    E2Flight1(u8),
    /// `e2.flight_messages.02[k]` (E2 `M2[k]`).
    E2Flight2(u8),
    /// `e2.jurisdiction.warnings` (one text).
    E2Jurisdiction,
    /// `e2.flight_events` (one text: the flight-18 base destruction).
    E2Events,
}

impl Message {
    /// The `ExeResources` id of the text list and the element index.
    pub fn resource(self) -> (&'static str, usize) {
        let (id, k) = match self {
            Self::Objective(k) => ("e1.objectives", k),
            Self::Flight1(k) => ("e1.flight_messages.01", k),
            Self::Flight2(k) => ("e1.flight_messages.02", k),
            Self::Jurisdiction(k) => ("jurisdiction.warnings", k),
            Self::Events1(k) => ("e1.flight_events.01", k),
            Self::E2Objective(k) => ("e2.objectives", k),
            Self::E2Flight1(k) => ("e2.flight_messages.01", k),
            Self::E2Flight2(k) => ("e2.flight_messages.02", k),
            // Single texts (`ExeResources::text`), index 0.
            Self::E2Jurisdiction => ("e2.jurisdiction.warnings", 0),
            Self::E2Events => ("e2.flight_events", 0),
        };
        (id, usize::from(k))
    }

    /// True when the text is drawn with the flight font (`Txt_FlightDrawStringOpaque`): the
    /// objectives, the jurisdiction warning and the E2 flight-18 event text (e2-mission-logic
    /// §4.7.1); the mission messages use the mission glyph routine.
    pub fn flight_font(self) -> bool {
        matches!(
            self,
            Self::Objective(_)
                | Self::Jurisdiction(_)
                | Self::E2Objective(_)
                | Self::E2Jurisdiction
                | Self::E2Events
        )
    }

    fn code(self) -> [u8; 2] {
        match self {
            Self::Objective(k) => [1, k],
            Self::Flight1(k) => [2, k],
            Self::Flight2(k) => [3, k],
            Self::Jurisdiction(k) => [4, k],
            Self::Events1(k) => [5, k],
            Self::E2Objective(k) => [6, k],
            Self::E2Flight1(k) => [7, k],
            Self::E2Flight2(k) => [8, k],
            Self::E2Jurisdiction => [9, 0],
            Self::E2Events => [10, 0],
        }
    }
}

/// The HUD message ticker (hud §4.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Ticker {
    /// `g_TickerPos` (DS:32BC): 0 = idle, else the next window position (1..=78).
    pub pos: u16,
    /// The text last drawn into the strip (`None` before the first one).
    pub message: Option<Message>,
    /// Its ink: [`INK`] or [`INK_ALERT`].
    pub ink: u8,
    /// Counts strip draws (wrapping): changes whenever a text is drawn into the strip, also
    /// when it is the same text again.
    pub draws: u16,
    /// Window position of the last blit (strip x = 4 × window), 0 = never blitted.
    pub window: u16,
}

impl Ticker {
    /// True when no message scrolls (`g_TickerPos = 0`).
    pub fn idle(&self) -> bool {
        self.pos == 0
    }

    /// Draws `message` into the strip and restarts the scroll.
    pub fn post(&mut self, message: Message, ink: u8) {
        self.message = Some(message);
        self.ink = ink;
        self.draws = self.draws.wrapping_add(1);
        self.pos = 1;
    }

    /// The ticker step of `UI_HudTickerAndIndicator` (hud §4.7), page-0 ticks only.
    fn step(&mut self, back_page: u8) {
        if self.pos >= 1 && back_page == 0 {
            self.window = self.pos;
            self.pos += 1;
            if self.pos > TICKER_LAST {
                self.pos = 0;
            }
        }
    }
}

/// Mission-logic state. The whole struct carries from flight to flight in
/// [`super::Session`]: per-flight setup re-initialises only what §3.3 says it does (phase,
/// home marker, objective and ticker); the other globals keep their values (U-144).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissionState {
    /// The rules run (flights built by `Flight::start`; the dev and test constructors build
    /// flights without them).
    pub scripted: bool,
    /// `g_MissionPhase` (DS:32BA): 1 at start, [`PHASE_DONE`] when the objectives are done.
    pub phase: u8,
    pub ticker: Ticker,
    /// `g_CommIncoming` (DS:329C): a transmission waits for F6.
    pub comm_pending: bool,
    /// `g_CommIndicatorPhase` (DS:1705), 0..=7 (hud §4.8).
    pub comm_phase: u8,
    /// Marker of the home point (element 21 of `g_SlotMarker`, DS:396C).
    pub home_marker: u8,
    /// Transmission shown by the last comm viewer (`UI_CommScreen(n)`, text `e1.comms.0n`);
    /// 0 = none yet. Valid while [`super::Modal::CommViewer`] is open.
    pub transmission: u8,
    /// Real-time interlude requested in this tick's step 18 (presentation; ticks stop while it
    /// shows): 17 = flight-17 cut-scene (`td1-17.pic`, `e1.flight_events.02`), 19 = the
    /// flight-19 V.C.I. screen (`e1.flight_events.01[5]` for 2,000 ms); 0 = none.
    pub scene: u8,
    /// Flight 17 (DS:329D): ships that reached the base, then the destruction counter; flight
    /// 20: hits on the lander. Shared by both flights, as in the original.
    pub count: u8,
    /// Flight 17 (DS:32A0..32B3): slot `s` has hit the base (index `s - 1`).
    pub hit: [bool; SHIP_SLOTS],
    /// Flight 17: the cut-scene request (DS:32B6).
    pub cutscene: bool,
    /// Flight 17: the base tiles (105..108, 104..107) are rubble (tile 49).
    pub base_rubble: bool,
    /// Flight 18: ticks since the ion truck was found (DS:32B4).
    pub ion_timer: u16,
    /// Flight 18: the bomb went off (DS:32B1).
    pub bomb: bool,
    /// Flights 18 and 20: the DAC and the flight palette are all white (presentation).
    pub white_out: bool,
    /// Flight 19: slot 19's tile and offsets while the player flies the Vulture II
    /// (DS:16EE/16F0/16F2/16F4).
    pub truck: [i16; 4],
    /// E2 flight 18: the debris counter of `Msn_E2FlightEvents` (E2 DS:11E8, cleared at flight
    /// start; e2-mission-logic §4.7.1).
    pub debris: u16,
}

impl Default for MissionState {
    fn default() -> Self {
        Self {
            scripted: false,
            phase: 0,
            ticker: Ticker::default(),
            comm_pending: false,
            comm_phase: 0,
            home_marker: 0,
            transmission: 0,
            scene: 0,
            count: 0,
            hit: [false; SHIP_SLOTS],
            cutscene: false,
            base_rubble: false,
            ion_timer: 0,
            bomb: false,
            white_out: false,
            truck: [0; 4],
            debris: 0,
        }
    }
}

impl MissionState {
    /// The mission-logic part of `Msn_Setup` (§3.3): phase 1, home marker, the objective in
    /// the ticker (all flights except 20).
    pub fn setup(&mut self, flight: u8) {
        self.scripted = true;
        self.phase = 1;
        self.home_marker = if flight == 19 { MARK_TARGET } else { MARK_HOME };
        self.transmission = 0;
        self.scene = 0;
        let objective = match flight {
            1..=7 => Some(flight - 1),
            8 => Some(3),
            9..=19 => Some(flight - 2),
            21 => Some(18),
            _ => None,
        };
        if let Some(k) = objective {
            let ink = if flight >= 12 { INK_ALERT } else { INK };
            self.ticker.post(Message::Objective(k), ink);
        }
    }

    /// The mission-logic part of E2 flight start (e2-mission-setup §4.1 steps 1 and 3, §3.2):
    /// the debris counter, comm pending and the trigger (`g_CutsceneRequest`) are cleared,
    /// then `Msn_Setup` sets phase 1, the home marker 3Dh and the objective (ink 37 for flights
    /// 1–7 and 9–11, 42 for 8 and 12–20); flight 11 starts with a transmission waiting.
    pub fn setup_e2(&mut self, flight: u8) {
        self.scripted = true;
        self.debris = 0;
        self.comm_pending = false;
        self.cutscene = false;
        self.transmission = 0;
        self.scene = 0;
        self.ticker.pos = 0;
        if !(1..=20).contains(&flight) {
            return;
        }
        self.phase = 1;
        self.home_marker = MARK_HOME;
        let ink = if flight == 8 || flight >= 12 {
            INK_ALERT
        } else {
            INK
        };
        self.ticker.post(Message::E2Objective(flight - 1), ink);
        if flight == 11 {
            self.comm_pending = true;
        }
    }

    /// Frame of the transmission indicator (`td.fac`), 0 = blank lamp (hud §4.8).
    pub fn comm_frame(&self) -> u8 {
        match self.comm_phase {
            p @ 0..=4 => p,
            p => 8 - p,
        }
    }
}

impl HashState for MissionState {
    fn hash_state(&self, h: &mut StateHasher) {
        let t = &self.ticker;
        let msg = t.message.map_or([0, 0], Message::code);
        h.bytes(&[
            u8::from(self.scripted),
            self.phase,
            msg[0],
            msg[1],
            t.ink,
            u8::from(self.comm_pending),
            self.comm_phase,
            self.home_marker,
            self.transmission,
            self.scene,
            self.count,
            u8::from(self.cutscene),
            u8::from(self.base_rubble),
            u8::from(self.bomb),
            u8::from(self.white_out),
        ]);
        for w in [t.pos, t.draws, t.window, self.ion_timer, self.debris] {
            w.hash_state(h);
        }
        for w in self.truck {
            w.hash_state(h);
        }
        for b in self.hit {
            b.hash_state(h);
        }
    }
}

/// What the HUD shows at tick step 19 (`UI_DrawInstruments`); later steps of the same tick
/// change shields (27), the weapon, missiles, palette mode (33) and the ticker (M key, 33).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HudSnapshot {
    pub armour: i8,
    pub shields: i8,
    pub missiles: u8,
    pub missile_cooldown: u8,
    pub weapon: u8,
    pub palette_mode: u8,
    /// Mission phase.
    pub phase: u8,
    /// The ticker after its step (`window` is the position blitted to the HUD).
    pub ticker: Ticker,
    pub comm_pending: bool,
    /// Indicator phase after its step; [`MissionState::comm_frame`] gives the frame.
    pub comm_phase: u8,
    /// Marker of the home point (element 21).
    pub home_marker: u8,
}

/// One comm-viewer entry (§4.6): flight, phase it applies in, transmission, phase after.
const COMMS: [(u8, u8, u8, u8); 9] = [
    (2, 1, 1, 2),
    (5, 2, 2, 3),
    (6, 3, 3, 4),
    (8, 4, 4, 5),
    (9, 2, 5, 3),
    (10, 2, 6, 3),
    (14, 2, 7, PHASE_DONE),
    (17, 3, 8, 4),
    (18, 2, 9, 3),
];

/// The transmission the comm viewer shows in `flight` at `phase` and the phase after it
/// (§4.6), or `None` when F6 only consumes the request.
pub fn transmission(flight: u8, phase: u8) -> Option<(u8, u8)> {
    COMMS
        .iter()
        .find(|c| c.0 == flight && c.1 == phase)
        .map(|c| (c.2, c.3))
}

/// Respawn table of `Msn_UpdateEvents` (§4.7): slots in order, class, armour.
fn respawns(flight: u8) -> (&'static [usize], u8, i16) {
    match flight {
        3 | 4 => (&[19], 7, 1),
        16 => (&[4, 5], 8, 15),
        7 => (&[13, 14], 7, 10),
        9 => (&[11, 12], 7, 10),
        11 => (&[8, 9, 17, 18], 7, 10),
        1 | 2 | 5 | 6 => (&[19, 20], 7, 1),
        _ => (&[], 0, 0),
    }
}

use Message::{Flight1 as M1, Flight2 as M2};

/// Primitives (§3.1, §3.2). Slots are 1-based; 0 is the player.
impl Flight {
    fn class_of(&self, s: usize) -> u8 {
        self.ships.0[s - 1].class
    }

    fn live(&self, s: usize) -> bool {
        self.class_of(s) != NO_SHIP
    }

    /// `dead(s, …)`: every listed slot has class FFh.
    fn dead(&self, slots: &[usize]) -> bool {
        slots.iter().all(|&s| !self.live(s))
    }

    fn dead_range(&self, a: usize, b: usize) -> bool {
        (a..=b).all(|s| !self.live(s))
    }

    /// Tile of slot `s` (kept after the ship dies or leaves).
    pub fn slot_tile(&self, s: usize) -> (u8, u8) {
        let sh = &self.ships.0[s - 1];
        (sh.tile_x, sh.tile_y)
    }

    /// Position used by the range rules: for 0 the view tile (low bytes) in E1, the player tile
    /// (view tile + (5, 3), low bytes) in E2 (e2-mission-logic §3.1); else the slot tile.
    fn range_pos(&self, k: usize) -> (u8, u8) {
        if k == 0 {
            if self.episode == Episode::E1 {
                let v = self.player.view;
                (v.tile_x as u8, v.tile_y as u8)
            } else {
                self.player_tile()
            }
        } else {
            self.slot_tile(k)
        }
    }

    /// `Msn_Near(A, B, Range)` (`2404:07DB`).
    fn msn_near(&self, a: usize, b: usize, range: u8) -> bool {
        let (pa, pb) = (self.range_pos(a), self.range_pos(b));
        let r = i16::from(range);
        (i16::from(pa.0) - i16::from(pb.0)).abs() < r
            && (i16::from(pa.1) - i16::from(pb.1)).abs() < r
    }

    /// `Msn_NearLive(A, B, Range)` (`2404:06EE`).
    fn near_live(&self, a: usize, b: usize, range: u8) -> bool {
        self.msn_near(a, b, range) && (a == 0 || self.live(a)) && (b == 0 || self.live(b))
    }

    /// `engage a → b, r` for each target in order.
    fn engage(&mut self, a: usize, targets: &[usize], range: u8) {
        for &b in targets {
            if self.near_live(a, b, range) {
                // `AI_SetTarget` (`34E6:3232`).
                self.ships.order_attack(a - 1, b as u8);
            }
        }
    }

    /// `Msn_ExitAt` (`2404:05DC`) for each (slot, x, y).
    fn exits(&mut self, list: &[(usize, u8, u8)]) {
        for &(s, x, y) in list {
            if self.slot_tile(s) == (x, y) {
                self.ships.0[s - 1].class = NO_SHIP;
            }
        }
    }

    fn spawn(&mut self, slots: &[usize], class: u8) {
        for &s in slots {
            self.ships.0[s - 1].class = class;
        }
    }

    fn hide(&mut self, slots: &[usize]) {
        self.spawn(slots, NO_SHIP);
    }

    fn mark(&mut self, slots: &[usize], marker: u8) {
        for &s in slots {
            self.ships.0[s - 1].marker = marker;
        }
    }

    /// `go s`: the current waypoint := path step 2.
    fn go(&mut self, slots: &[usize]) {
        for &s in slots {
            let r = &mut self.ships.0[s - 1].record;
            r.waypoint = r.path[1];
        }
    }

    fn post(&mut self, m: Message) {
        self.mission.ticker.post(m, INK);
    }

    fn alert(&mut self, m: Message) {
        self.mission.ticker.post(m, INK_ALERT);
    }

    fn idle(&self) -> bool {
        self.mission.ticker.idle()
    }

    fn phase(&self) -> u8 {
        self.mission.phase
    }

    fn set_phase(&mut self, p: u8) {
        self.mission.phase = p;
    }

    /// `Msn_PlayerTouches(s)` (`2404:0616`).
    fn player_touches(&self, s: usize) -> bool {
        let sh = &self.ships.0[s - 1];
        let v = self.player.view;
        let four = Real48::from_i32(4);
        let ox = Real48::from_i32(i32::from(v.sub_x))
            .checked_div(four)
            .and_then(Real48::round)
            .map_or(0, |r| 4 * r);
        let dx =
            (i32::from(sh.tile_x) - i32::from(v.tile_x)) * 32 + i32::from(sh.sub_x) - (ox + 128);
        let dy = (i32::from(sh.tile_y) - i32::from(v.tile_y)) * 32 + i32::from(sh.sub_y)
            - (i32::from(v.sub_y) + 84);
        dx.abs() < 32 && dy.abs() < 32 && sh.alive()
    }

    /// The base test (§4.4).
    fn base_test(&mut self) {
        let v = self.player.view;
        let at_base = match self.number {
            12 | 15 => v.tile_x == 102 && v.tile_y > 98 && v.tile_y < 101,
            13 => (175..=177).contains(&v.tile_x) && (152..=154).contains(&v.tile_y),
            _ => (v.tile_x, v.tile_y) == (102, 104),
        };
        self.succeeded = at_base && self.phase() == PHASE_DONE;
    }

    /// An explosion in the lowest idle slot of 1..=20 with frame 1; `false` when none is idle.
    ///
    /// mission-logic rev 1 (§5.7, §5.17, §5.20) says slots 1..=20 for these starters;
    /// flight-screens §4.7 says `2404:1426` uses only slots 21..=30. The port follows the
    /// mission-logic text until RE settles it (U-145).
    fn mission_explosion(&mut self, tile: (u8, u8), sub: (u8, u8)) -> bool {
        let Some(i) = self.explosions.idle_hit_slot() else {
            return false;
        };
        self.explosions
            .start_in(i, tile, (i16::from(sub.0), i16::from(sub.1)));
        true
    }

    /// An explosion with random offsets (`Random(48)` each); the generator is drawn only
    /// when a slot is idle (ASSUMPTION, U-145).
    fn random_explosion(&mut self, tile: impl FnOnce(&mut Self) -> (u8, u8)) -> Option<(u8, u8)> {
        self.explosions.idle_hit_slot()?;
        let sx = self.rng.random(48) as u8;
        let sy = self.rng.random(48) as u8;
        let t = tile(self);
        self.mission_explosion(t, (sx, sy));
        Some(t)
    }
}

/// Steps 8–9: `Msn_UpdateA` (flights 1–9) and `Msn_UpdateB` (flights 10–20).
impl Flight {
    pub(super) fn msn_update(&mut self) {
        if !self.mission.scripted {
            return;
        }
        match self.episode {
            Episode::E1 => {}
            Episode::E2 => return self.e2_update(),
            Episode::E3 => return,
        }
        match self.number {
            1 => self.flight_1(),
            2 => self.flight_2(),
            3 => self.flight_3(),
            4 => self.flight_4(),
            5 => self.flight_5(),
            6 => self.flight_6(),
            7 => self.flight_7(),
            8 => self.flight_8(),
            9 => self.flight_9(),
            10 => self.flight_10(),
            11 => self.flight_11(),
            12 => self.flight_12(),
            13 => self.flight_13(),
            14 => self.flight_14(),
            15 => self.flight_15(),
            16 => self.flight_16(),
            17 => self.flight_17(),
            18 => self.flight_18(),
            19 => self.flight_19(),
            20 => self.engage_list(&[(7, 0, 35), (8, 0, 35)]),
            _ => {}
        }
    }

    fn engage_list(&mut self, list: &[(usize, usize, u8)]) {
        for &(a, b, r) in list {
            self.engage(a, &[b], r);
        }
    }

    /// §5.1.
    fn flight_1(&mut self) {
        self.base_test();
        self.exits(&[
            (1, 50, 196),
            (2, 51, 197),
            (3, 50, 2),
            (4, 51, 2),
            (5, 198, 122),
            (6, 198, 123),
            (7, 198, 124),
            (8, 198, 123),
            (9, 198, 122),
            (10, 198, 123),
            (11, 50, 1),
        ]);
        self.engage(8, &[0], 20);
        self.engage(11, &[0], 20);
        for s in 14..=20 {
            self.engage(s, &[0], 10);
        }
        if self.dead(&[1, 2]) && self.phase() == 1 {
            self.set_phase(2);
            self.spawn(&[3, 4], 0);
            self.post(M1(0));
        }
        if self.dead(&[3, 4, 1, 2]) && self.phase() < 3 {
            self.post(M1(1));
            self.set_phase(3);
            self.spawn(&[5, 6, 7], 0);
            self.spawn(&[8], 7);
        } else if self.phase() < 3 {
            self.hide(&[5, 6, 7, 8]);
        }
        if self.dead(&[5, 6, 7, 8]) && self.phase() == 3 {
            self.post(M1(2));
            self.set_phase(4);
            self.spawn(&[9, 10], 0);
        } else if self.phase() < 4 {
            self.hide(&[9, 10]);
        }
        if self.dead(&[9, 10]) && self.phase() == 4 {
            if self.idle() {
                self.post(M1(3));
                self.set_phase(5);
                self.spawn(&[11], 7);
            }
        } else if self.phase() < 5 {
            self.hide(&[11]);
        }
        if self.dead(&[11]) && self.phase() == 5 {
            self.post(M1(4));
            self.set_phase(PHASE_DONE);
        }
    }

    /// §5.2.
    fn flight_2(&mut self) {
        self.base_test();
        self.exits(&[
            (1, 50, 198),
            (2, 51, 199),
            (3, 1, 122),
            (4, 1, 121),
            (5, 2, 124),
            (6, 1, 122),
            (7, 1, 124),
        ]);
        self.engage_list(&[
            (1, 0, 30),
            (1, 2, 70),
            (5, 0, 30),
            (6, 0, 60),
            (7, 0, 60),
            (18, 0, 10),
            (19, 0, 10),
            (20, 0, 10),
        ]);
        if self.idle() && self.phase() == 1 {
            self.alert(M1(5));
            self.mission.comm_pending = true;
        }
        if self.dead(&[2]) {
            self.mission.comm_pending = false;
        }
        if self.dead(&[1, 2]) && self.phase() < 3 {
            self.mission.comm_pending = false;
            self.set_phase(3);
            self.post(M1(6));
            self.go(&[3, 4, 5]);
            self.spawn(&[3, 4], 0);
            self.spawn(&[5], 7);
        } else if !matches!(self.phase(), 3 | 4) {
            self.hide(&[3, 4, 5]);
        }
        if self.dead(&[5]) && self.phase() == 3 {
            self.set_phase(4);
            self.post(M1(7));
            self.go(&[6, 7]);
            self.spawn(&[6, 7], 7);
        } else if self.phase() != 4 {
            self.hide(&[6, 7]);
        }
        if self.phase() == 4 && self.dead(&[6, 7]) {
            self.post(M1(4));
            self.set_phase(PHASE_DONE);
        }
    }

    /// §5.3.
    fn flight_3(&mut self) {
        self.base_test();
        for i in [2, 3] {
            self.engage(1, &[i], 20);
            self.engage(i, &[1, 0, 4], 20);
        }
        self.engage_list(&[
            (1, 12, 20),
            (7, 0, 30),
            (6, 0, 30),
            (12, 1, 50),
            (12, 0, 90),
            (19, 0, 10),
        ]);
        self.exits(&[(1, 107, 106), (2, 7, 122), (3, 1, 124)]);
        if self.slot_tile(4) == (107, 106) && self.live(4) {
            self.hide(&[4]);
            self.post(M1(8));
            self.mark(&[2, 3], MARK_TARGET);
        }
        self.exits(&[
            (5, 199, 122),
            (6, 50, 1),
            (7, 51, 1),
            (8, 50, 198),
            (12, 199, 122),
        ]);
        let t12 = self.slot_tile(12);
        if t12.0 == 199 && (120..=125).contains(&t12.1) {
            self.hide(&[12]);
        }
        if self.phase() == 1 && self.idle() {
            self.post(M1(9));
            self.set_phase(2);
            self.spawn(&[5], 8);
            self.mark(&[5], MARK_TARGET);
        }
        if self.phase() == 2 && self.idle() && self.dead(&[4]) {
            self.post(M1(10));
            self.set_phase(3);
            self.spawn(&[6, 7], 7);
            self.mark(&[7, 6], MARK_TARGET);
        }
        if (self.dead(&[2]) || self.dead(&[3])) && self.idle() && self.phase() == 3 {
            self.post(M1(11));
            self.spawn(&[8], 8);
            self.set_phase(4);
            self.mark(&[8], MARK_TARGET);
        }
        if self.dead(&[8]) && self.idle() && self.phase() == 4 {
            self.post(M1(12));
            self.spawn(&[12], 7);
            self.set_phase(5);
            self.mark(&[12], MARK_TARGET);
        }
        if self.dead(&[12]) && self.idle() && self.phase() == 5 {
            self.post(M1(13));
            self.set_phase(PHASE_DONE);
        }
    }

    /// §5.4.
    fn flight_4(&mut self) {
        self.base_test();
        self.exits(&[
            (2, 199, 124),
            (3, 199, 124),
            (4, 50, 1),
            (5, 199, 122),
            (7, 50, 199),
        ]);
        if self.slot_tile(6) == (84, 135) {
            self.ships.0[5].speed = [16; 4];
        }
        self.engage_list(&[
            (1, 7, 50),
            (1, 2, 50),
            (1, 3, 60),
            (1, 5, 20),
            (1, 4, 30),
            (2, 1, 30),
            (3, 1, 30),
            (2, 0, 40),
            (3, 0, 40),
        ]);
        if self.phase() > 3 {
            self.engage_list(&[(4, 0, 30), (5, 0, 30)]);
        }
        self.engage_list(&[
            (7, 1, 30),
            (7, 0, 60),
            (7, 8, 30),
            (7, 9, 30),
            (19, 0, 10),
            (20, 0, 10),
        ]);
        if self.idle() && self.phase() == 1 {
            self.post(M1(14));
            self.set_phase(2);
        }
        if self.phase() == 3 {
            self.set_phase(4);
        }
        if self.idle() && self.phase() == 2 && self.dead(&[2, 3]) {
            self.post(M1(15));
            self.spawn(&[4, 5], 8);
            self.set_phase(3);
        }
        if self.phase() == 4 && self.dead(&[4, 5]) {
            self.post(M1(16));
            self.spawn(&[7], 8);
            self.spawn(&[8, 9], 1);
            self.set_phase(5);
        }
        if self.phase() == 5 && self.dead(&[7]) {
            self.post(M1(17));
            self.set_phase(PHASE_DONE);
        }
    }

    /// §5.5.
    fn flight_5(&mut self) {
        self.base_test();
        if self.slot_tile(1) == (83, 174) && self.live(1) && self.phase() == 1 {
            self.set_phase(2);
        }
        self.engage_list(&[
            (3, 1, 40),
            (3, 0, 20),
            (4, 1, 35),
            (4, 0, 15),
            (5, 1, 40),
            (5, 0, 20),
            (7, 0, 20),
            (7, 1, 10),
        ]);
        self.exits(&[(1, 199, 122)]);
        if self.dead(&[1]) && self.slot_tile(1) == (199, 122) && self.phase() < 4 {
            self.alert(M1(18));
            self.set_phase(4);
        }
        if self.dead(&[1]) && self.phase() < 4 {
            self.alert(M1(19));
            self.set_phase(4);
        }
        let escorts_dead = self.dead(&[3, 4, 5, 7]);
        if self.phase() == 4 && self.idle() && !escorts_dead {
            self.alert(M1(20));
            self.set_phase(5);
        }
        if matches!(self.phase(), 4 | 5) && self.idle() && self.dead(&[3, 4, 5, 7]) {
            self.alert(M1(21));
            self.set_phase(PHASE_DONE);
        }
        if self.dead(&[1]) {
            self.mission.comm_pending = false;
        }
        if self.phase() == 2 && self.idle() {
            self.alert(M1(22));
            self.mission.comm_pending = true;
        }
    }

    /// §5.6.
    fn flight_6(&mut self) {
        self.base_test();
        self.engage_list(&[
            (7, 0, 30),
            (8, 0, 40),
            (9, 0, 35),
            (19, 0, 10),
            (20, 0, 10),
            (5, 0, 40),
            (3, 0, 30),
        ]);
        if self.near_live(3, 0, 40) && self.phase() < 3 && self.idle() {
            self.alert(M1(23));
            self.mission.comm_pending = true;
            self.set_phase(3);
        }
        if self.dead(&[3]) {
            self.mission.comm_pending = false;
            if self.dead(&[5]) && self.idle() && self.phase() < 5 {
                self.set_phase(5);
                self.alert(M1(24));
            }
        }
        if self.dead(&[3, 5, 7, 8, 9]) && self.phase() < PHASE_DONE {
            self.alert(M1(25));
            self.set_phase(PHASE_DONE);
        }
    }

    /// §5.7.
    fn flight_7(&mut self) {
        self.base_test();
        for m in 2..=12 {
            if self.player_touches(m) {
                self.end_mission();
                let (x, y) = self.slot_tile(m);
                for (dx, dy) in [(1, 1), (0, 0), (1, -1), (-1, 1), (-1, -1)] {
                    let t = ((i16::from(x) + dx) as u8, (i16::from(y) + dy) as u8);
                    self.mission_explosion(t, (16, 16));
                }
                let sh = &mut self.ships.0[m - 1];
                sh.armour = 0;
                sh.shields = 0;
            }
        }
        for m in 2..=8 {
            if self.msn_near(1, m, 1) {
                self.ships.0[m - 1].class = 3;
            }
        }
        self.exits(&[(1, 199, 122)]);
        self.engage_list(&[(13, 0, 50), (14, 0, 50)]);
        if self.phase() < 3 {
            let gone = (1..=12).filter(|&s| !self.live(s)).count() as u8;
            let p = 1u8.wrapping_add(5u8.wrapping_mul(gone));
            self.set_phase(if p >= 60 { 2 } else { 1 });
        }
        if self.phase() == 2 && self.idle() {
            self.alert(M1(26));
            self.set_phase(PHASE_DONE);
        }
    }

    /// §5.8.
    fn flight_8(&mut self) {
        self.base_test();
        for s in 3..=6 {
            if self.live(s) {
                self.ships.0[s - 1].class = 7;
            }
        }
        if self.phase() == 1 && self.idle() {
            self.set_phase(2);
            self.alert(M1(27));
        }
        if self.dead(&[12, 13]) && self.phase() == 2 {
            self.alert(M1(28));
            self.set_phase(3);
            self.spawn(&[8, 9], 8);
        }
        if self.dead(&[8, 9]) && self.mission.comm_pending && self.phase() == 4 {
            self.set_phase(5);
        }
        if self.dead(&[8, 9]) {
            self.mission.comm_pending = false;
        }
        if (self.near_live(0, 8, 30) || self.near_live(0, 9, 30))
            && self.idle()
            && self.phase() == 3
        {
            self.set_phase(4);
            self.alert(M1(29));
            self.mission.comm_pending = true;
        }
        if self.dead(&[15, 16, 17, 18, 3, 4, 5, 6, 8, 9]) && self.phase() == 5 {
            self.alert(M1(30));
            self.set_phase(PHASE_DONE);
        }
        for s in 3..=6 {
            self.engage(1, &[s], 20);
        }
        self.engage_list(&[
            (1, 8, 10),
            (1, 9, 10),
            (1, 12, 20),
            (1, 13, 20),
            (1, 15, 30),
            (1, 16, 30),
        ]);
        for s in 3..=6 {
            self.engage_list(&[
                (s, 0, 20),
                (s, 1, 15),
                (s, 12, 25),
                (s, 13, 25),
                (12, s, 5),
                (13, s, 5),
            ]);
        }
        self.exits(&[(3, 50, 1), (4, 51, 1), (5, 49, 1), (6, 51, 1), (8, 50, 1)]);
        self.engage_list(&[(8, 0, 35), (8, 1, 10)]);
        self.exits(&[(9, 51, 1)]);
        self.engage_list(&[(9, 0, 35), (9, 1, 10)]);
        self.exits(&[(12, 199, 121)]);
        self.engage_list(&[(12, 1, 5), (12, 0, 5)]);
        self.exits(&[(13, 199, 124)]);
        self.engage_list(&[(13, 0, 5), (13, 1, 5)]);
    }

    /// §5.9.
    fn flight_9(&mut self) {
        self.base_test();
        if self.phase() == 1 && self.idle() {
            self.alert(M1(31));
            self.mission.comm_pending = true;
            self.set_phase(2);
        }
        if self.dead_range(1, 10) && self.phase() > 1 && self.phase() != PHASE_DONE {
            self.mission.comm_pending = false;
            self.set_phase(PHASE_DONE);
            self.alert(M1(32));
        }
        let guards: Vec<usize> = (13..=18).collect();
        for a in 5..=12 {
            self.engage(a, &guards, 20);
            self.engage(a, &[0], 15);
        }
        for a in 5..=12 {
            for &b in &guards {
                self.engage(b, &[a], 10);
            }
        }
        self.exits(&[(13, 50, 199)]);
        self.engage_list(&[(13, 1, 30), (13, 3, 30), (13, 0, 10)]);
        self.exits(&[(14, 199, 124)]);
        self.engage_list(&[(14, 1, 30), (14, 2, 30), (14, 0, 10)]);
        self.exits(&[(15, 199, 122)]);
        self.engage_list(&[(15, 4, 30), (15, 0, 10)]);
        self.exits(&[(16, 199, 123)]);
        self.engage_list(&[(16, 0, 15), (16, 2, 30), (16, 1, 30)]);
        self.exits(&[(17, 199, 124)]);
        self.engage_list(&[(17, 4, 30), (17, 0, 10), (17, 12, 10)]);
        self.exits(&[(18, 199, 121)]);
        self.engage_list(&[(18, 3, 30), (18, 0, 25)]);
    }

    /// §5.10.
    fn flight_10(&mut self) {
        self.base_test();
        if self.phase() == 1 && self.idle() {
            self.alert(M2(0));
            self.mission.comm_pending = true;
            self.set_phase(2);
        }
        if self.dead(&[12]) {
            self.mission.comm_pending = false;
        }
        if self.dead_range(7, 12) && self.phase() < PHASE_DONE {
            self.set_phase(PHASE_DONE);
            self.alert(M2(1));
        }
        for s in 7..=12 {
            self.engage(1, &[s], 25);
        }
        self.exits(&[(7, 1, 124)]);
        for s in 7..=11 {
            self.engage(s, &[1], 15);
            self.engage(s, &[0], 20);
            self.engage(s, &[3, 4, 5, 14, 15, 16], 30);
        }
        self.exits(&[
            (8, 1, 124),
            (9, 199, 122),
            (10, 199, 124),
            (11, 1, 122),
            (12, 199, 122),
        ]);
        if self.ships.0[11].armour > 50 {
            self.engage(12, &[3, 4, 5], 10);
            self.engage(12, &[14, 15, 16], 25);
            self.engage(12, &[1], 10);
            self.engage(12, &[0], 50);
        }
        self.exits(&[
            (3, 1, 122),
            (4, 1, 124),
            (5, 1, 123),
            (14, 1, 122),
            (15, 1, 124),
            (16, 1, 123),
        ]);
    }

    /// §5.11.
    fn flight_11(&mut self) {
        self.base_test();
        for (s, c) in [(7, 8), (8, 7), (9, 7)] {
            if self.live(s) {
                self.ships.0[s - 1].class = c;
            }
        }
        if self.dead(&[1, 2, 3]) && self.phase() == 1 {
            self.set_phase(2);
            self.alert(M2(2));
            self.spawn(&[7], 8);
            self.spawn(&[8, 9], 7);
        }
        if self.phase() == 2 && self.idle() {
            self.set_phase(PHASE_DONE);
            self.alert(M2(3));
        }
        self.exits(&[(1, 199, 122)]);
        let mercs = [11, 12, 13, 14, 15];
        for a in 1..=3 {
            self.engage(a, &[4, 5], 20);
            self.engage(a, &mercs, 15);
        }
        self.engage(1, &[0], 40);
        self.exits(&[(2, 199, 124)]);
        self.engage(2, &[0], 40);
        self.exits(&[(3, 199, 123)]);
        self.engage(3, &[0], 40);
        for a in [4, 5] {
            self.engage(a, &[1, 2, 3, 7, 8, 9, 17, 18], 10);
        }
        self.exits(&[(7, 199, 122)]);
        self.engage_list(&[(7, 4, 15), (7, 5, 15), (7, 0, 50)]);
        self.exits(&[(8, 199, 124)]);
        self.engage(8, &mercs, 15);
        self.engage_list(&[(8, 4, 20), (8, 5, 20), (8, 0, 35)]);
        self.exits(&[(9, 199, 123)]);
        self.engage(9, &[4, 5], 15);
        self.engage(9, &mercs, 10);
        self.engage(9, &[0], 40);
        for a in [17, 18] {
            self.engage(a, &mercs, 15);
            self.engage(a, &[4, 5], 15);
        }
        self.engage_list(&[(17, 0, 15), (18, 0, 15)]);
    }

    /// §5.12.
    fn flight_12(&mut self) {
        self.base_test();
        if self.dead_range(1, 5) && self.phase() == 1 {
            self.set_phase(PHASE_DONE);
            self.alert(M2(4));
        }
        self.exits(&[(7, 50, 1)]);
        self.engage(7, &[5, 4, 1, 2, 3], 35);
        self.exits(&[(8, 50, 1)]);
        self.engage(8, &[5, 1, 2, 3, 4], 35);
        self.exits(&[(9, 50, 1)]);
        self.engage(9, &[15, 16], 15);
        self.engage(9, &[5, 1, 2, 3, 4], 40);
        self.exits(&[(10, 51, 199)]);
        self.engage(10, &[5, 1, 2, 3, 4], 35);
        self.exits(&[(11, 51, 199)]);
        self.engage(11, &[5, 1, 2, 3, 4], 35);
        self.exits(&[(12, 50, 1)]);
        self.engage(12, &[15, 16], 10);
        self.engage(12, &[1, 2, 3, 4, 5], 25);
        self.exits(&[(13, 51, 1)]);
        self.engage(13, &[15, 16], 10);
        self.engage(13, &[1, 2, 3, 4, 5], 25);
        for s in 1..=5 {
            for x in 49..=51 {
                self.exits(&[(s, x, 199)]);
            }
        }
        self.exits(&[
            (1, 49, 199),
            (2, 50, 199),
            (3, 51, 199),
            (4, 49, 199),
            (5, 51, 199),
        ]);
        let raiders = [7, 8, 9, 10, 11, 12, 13];
        self.exits(&[(15, 50, 199)]);
        self.engage(15, &raiders, 15);
        self.exits(&[(16, 51, 199)]);
        self.engage(16, &raiders, 25);
    }

    /// §5.13.
    fn flight_13(&mut self) {
        self.base_test();
        if self.phase() == 1 && self.dead(&[8, 9, 15, 16, 17]) {
            self.alert(M2(5));
            self.set_phase(2);
        }
        if self.phase() == 2 && self.dead(&[8, 9, 11, 12, 13, 15, 16, 17]) {
            self.alert(M2(6));
            self.set_phase(PHASE_DONE);
        }
        self.engage(1, &[8, 9, 15, 16, 17], 25);
        self.engage(2, &[8, 9, 15, 16, 17], 25);
        self.engage_list(&[
            (8, 1, 15),
            (8, 2, 15),
            (8, 0, 30),
            (9, 2, 15),
            (9, 1, 15),
            (9, 0, 30),
            (11, 1, 25),
            (11, 2, 25),
            (11, 0, 30),
            (12, 2, 15),
            (12, 1, 15),
            (12, 0, 30),
            (13, 0, 35),
            (13, 1, 20),
            (13, 2, 20),
            (15, 0, 25),
            (16, 0, 30),
            (17, 0, 30),
        ]);
    }

    /// §5.14.
    fn flight_14(&mut self) {
        self.base_test();
        let found = self.near_live(0, 12, 10)
            || self.near_live(0, 13, 10)
            || self.dead(&[12])
            || self.dead(&[13]);
        if found && self.phase() == 1 && self.idle() {
            self.alert(M2(7));
            self.mission.comm_pending = true;
            self.set_phase(2);
        }
        for a in 1..=6 {
            self.engage(a, &[8, 9, 10, 12, 13], 5);
            match a {
                1 | 3 | 5 | 6 => self.engage(a, &[0], 50),
                4 => self.engage(a, &[0], 25),
                _ => {}
            }
        }
        for a in [8, 9, 10] {
            self.engage(a, &[1, 2, 3, 4, 5, 6], 5);
        }
        self.engage(12, &[1, 2, 3, 4, 6, 5], 5);
        self.engage(13, &[1, 2, 3, 4, 5, 6], 5);
        self.engage_list(&[
            (8, 0, 35),
            (9, 0, 25),
            (10, 0, 30),
            (12, 0, 35),
            (13, 0, 30),
        ]);
    }

    /// §5.15.
    fn flight_15(&mut self) {
        self.base_test();
        if self.dead_range(1, 5) && self.phase() == 1 {
            self.set_phase(2);
            self.alert(M2(8));
            self.spawn(&[8, 9], 0);
            self.spawn(&[10, 11, 12], 7);
        }
        if self.dead_range(8, 12) && self.phase() == 2 {
            self.set_phase(PHASE_DONE);
            self.alert(M2(9));
        }
        self.exits(&[(1, 50, 199), (2, 50, 199), (8, 50, 1), (9, 50, 1)]);
        for (s, x, y, r) in [
            (3, 1, 122, 20),
            (4, 50, 199, 25),
            (5, 50, 199, 25),
            (10, 50, 1, 20),
            (11, 50, 1, 25),
            (12, 199, 122, 25),
        ] {
            self.exits(&[(s, x, y)]);
            self.engage(s, &[15, 16], r);
        }
        for a in [15, 16] {
            self.engage(a, &[3, 4, 5, 10, 11, 12], 5);
            self.engage(a, &[1, 2, 8, 9], 30);
        }
    }

    /// §5.16.
    fn flight_16(&mut self) {
        self.base_test();
        if self.dead(&[1, 2, 3]) && self.phase() == 1 {
            self.spawn(&[8, 9, 10], 0);
            self.spawn(&[11, 12], 8);
            self.spawn(&[13], 7);
            self.set_phase(2);
            self.alert(M2(10));
        }
        if self.dead(&[8, 9, 10]) && self.phase() == 2 {
            self.set_phase(PHASE_DONE);
            self.alert(M2(11));
        }
        self.exits(&[(1, 124, 71), (2, 125, 71), (3, 126, 71)]);
        self.engage_list(&[(4, 17, 30), (4, 0, 30)]);
        self.exits(&[(6, 199, 122)]);
        self.engage_list(&[(6, 17, 30), (6, 0, 30), (5, 0, 30), (5, 17, 30)]);
        self.exits(&[(8, 125, 71), (9, 125, 71), (10, 125, 71), (11, 1, 122)]);
        self.engage_list(&[(11, 0, 30), (11, 17, 30)]);
        self.exits(&[(12, 1, 122)]);
        self.engage_list(&[(12, 17, 30), (12, 0, 30)]);
        self.exits(&[(13, 50, 199)]);
        self.engage_list(&[(13, 17, 30), (13, 0, 30)]);
        self.engage(17, &[4, 5, 6, 11, 12, 13], 15);
        self.engage(17, &[1, 2, 3, 8, 9, 10], 30);
    }

    /// §5.17.
    fn flight_17(&mut self) {
        self.base_test();
        if self.dead(&[1]) && self.phase() == 1 {
            self.mission.cutscene = true;
            self.set_phase(2);
        }
        if self.phase() == 2 {
            self.set_phase(3);
            self.alert(M2(12));
            self.mission.comm_pending = true;
        }
        if self.phase() == 4 {
            self.mission.comm_pending = false;
            self.spawn(&[5, 6], 7);
            self.set_phase(5);
        }
        if self.phase() == 5 && self.dead(&[5, 6]) {
            self.set_phase(6);
            self.spawn(&[8], 7);
            self.spawn(&[9, 10], 8);
        }
        if self.phase() == 6 && self.dead(&[8, 9, 10]) {
            self.set_phase(7);
            self.spawn(&[12, 13, 14], 8);
        }
        if self.phase() == 7 && self.dead(&[12, 13, 14]) {
            self.set_phase(PHASE_DONE);
            self.alert(M2(13));
        }
        self.exits(&[(1, 50, 1), (3, 51, 1)]);
        self.engage(3, &[1], 15);
        self.exits(&[(5, 199, 122), (6, 199, 124)]);
        self.engage(6, &[0], 5);
        self.exits(&[(8, 50, 1)]);
        self.engage(8, &[0], 5);
        self.exits(&[(9, 50, 1), (10, 199, 124), (12, 199, 122), (13, 199, 122)]);
        self.engage(13, &[0], 5);
        self.exits(&[(14, 199, 122)]);
        self.engage(14, &[0], 5);
    }

    /// §5.18.
    fn flight_18(&mut self) {
        self.base_test();
        if self.dead(&[7, 10, 11, 15, 17]) {
            self.mark(&[3], MARK_TARGET);
        }
        if self.near_live(0, 3, 10) && self.phase() == 1 {
            self.mark(&[3], MARK_TARGET);
            let rest: Vec<usize> = (7..=20).collect();
            self.mark(&rest, 0);
            self.set_phase(2);
            self.alert(M2(14));
            self.mission.comm_pending = true;
            self.mission.ion_timer = 0;
        }
        if self.dead(&[3]) {
            self.mission.comm_pending = false;
        }
        if matches!(self.phase(), 2 | 3) {
            self.mission.ion_timer = self.mission.ion_timer.wrapping_add(1);
        }
        self.mission.bomb = self.mission.ion_timer > 999;
        if self.dead(&[3]) && matches!(self.phase(), 2 | 3) {
            self.alert(M2(15));
            self.mission.ion_timer = 0;
            self.set_phase(PHASE_DONE);
        }
        self.engage(1, &[3, 5, 6, 7, 9, 10, 11, 13, 14, 15], 15);
        self.exits(&[(5, 50, 199)]);
        self.engage_list(&[(5, 1, 10), (5, 0, 10)]);
        self.exits(&[(6, 199, 122)]);
        self.engage_list(&[(6, 1, 10), (6, 0, 15)]);
        self.exits(&[(9, 199, 123)]);
        self.engage_list(&[(9, 1, 10), (9, 0, 15)]);
        self.exits(&[(13, 50, 199)]);
        self.engage_list(&[(13, 1, 15), (13, 0, 15)]);
        self.exits(&[(14, 51, 199)]);
        self.engage_list(&[(14, 1, 15), (14, 0, 15)]);
        self.exits(&[(7, 50, 1), (10, 50, 199), (11, 199, 123), (15, 50, 199)]);
    }

    /// §5.19.
    fn flight_19(&mut self) {
        self.base_test();
        if self.phase() == 2 {
            self.mission.home_marker = MARK_HOME;
            self.alert(M2(16));
            self.set_phase(PHASE_DONE);
        }
        if self.phase() > 1 {
            for s in 1..=10 {
                self.engage(s, &[0], 25);
            }
        }
    }
}

/// Step 10 tail, flight 19 (mission-ships §4.2.4): with the phase above 1 the player's
/// shields are capped at 10 and slot 19 stays where the truck was left.
impl Flight {
    pub(super) fn flight_19_pin(&mut self) {
        if self.episode != Episode::E1
            || self.number != 19
            || !self.mission.scripted
            || self.phase() <= 1
        {
            return;
        }
        self.player.shields = self.player.shields.min(10);
        let [x, y, sx, sy] = self.mission.truck;
        let s = &mut self.ships.0[18];
        (s.tile_x, s.tile_y) = (x as u8, y as u8);
        (s.sub_x, s.sub_y) = (sx, sy);
    }
}

/// Step 18: `Msn_UpdateEvents` (`0E8B:9860`, §4.7).
impl Flight {
    pub(super) fn msn_update_events(&mut self, events: &mut Vec<Event>) {
        if !self.mission.scripted || self.episode != Episode::E1 {
            return;
        }
        self.mission.scene = 0;
        let (slots, class, armour) = respawns(self.number);
        let v = self.player.view;
        for &s in slots {
            let (tx, ty) = self.slot_tile(s);
            if !self.live(s)
                && (v.tile_x + 5 - i16::from(tx)).abs() > 15
                && (v.tile_y + 2 - i16::from(ty)).abs() > 15
            {
                let sh = &mut self.ships.0[s - 1];
                sh.class = class;
                sh.armour = armour;
            }
        }
        match self.number {
            17 => self.flight_17_events(),
            20 => self.flight_20_events(events),
            // The flight-13 time limit.
            13 if self.ticks == 15_000 => {
                self.post(Message::Events1(0));
                self.mission_over = true;
            }
            19 => self.flight_19_events(),
            // The flight-18 bomb.
            18 if self.mission.bomb => {
                self.hit_flash = 0;
                self.mission.white_out = true;
                self.lost = true;
            }
            _ => {}
        }
    }

    /// §5.17 `Msn_UpdateEvents` part (the flight-20, 13, 19 and 18 events between its two
    /// halves never run in flight 17).
    fn flight_17_events(&mut self) {
        if self.phase() == 3 && self.idle() {
            self.mission.ticker.pos = 1;
        }
        for s in 1..=SHIP_SLOTS {
            if !self.mission.hit[s - 1] && self.slot_tile(s) == (107, 106) && self.mission.count < 3
            {
                self.mission.hit[s - 1] = true;
                for _ in 0..4 {
                    self.random_explosion(|_| (106, 106));
                }
                self.mission.count += 1;
            }
        }
        let c = self.mission.count;
        if c > 2 {
            if c < 44 {
                let w = self.rng.random(4);
                for _ in 0..w {
                    self.random_explosion(|f| {
                        let x = 105 + f.rng.random(2) as u8;
                        let y = 105 + f.rng.random(2) as u8;
                        (x, y)
                    });
                }
            }
            if c > 40 {
                self.hit_flash = 0;
            }
            if c == 45 {
                for _ in 0..16 {
                    self.rng.random(1);
                }
                self.mission.base_rubble = true;
            }
            if c == 48 {
                self.lost = true;
            }
            self.mission.count = c.wrapping_add(1);
        }
        if self.mission.cutscene {
            self.mission.scene = 17;
            self.mission.cutscene = false;
        }
    }

    /// §5.19 `Msn_UpdateEvents` part: the switch from the truck to the Vulture II.
    fn flight_19_events(&mut self) {
        let v = self.player.view;
        if self.phase() == 1 && v.tile_y == 70 && v.tile_x > 117 && v.tile_x < 124 {
            self.mission.scene = 19;
            self.player.class = 8;
            // ASSUMPTION (U-146): "armour := 1" is the player's armour.
            self.player.armour = 1;
            let truck = [v.tile_x + 5, v.tile_y + 3, v.sub_x, v.sub_y];
            self.mission.truck = truck;
            let s = &mut self.ships.0[18];
            s.class = 0;
            (s.tile_x, s.tile_y) = (truck[0] as u8, truck[1] as u8);
            (s.sub_x, s.sub_y) = (truck[2], truck[3]);
            self.set_phase(2);
            self.player.view.tile_y += 1;
        }
    }

    /// §5.20 `Msn_UpdateEvents` part: the lander.
    fn flight_20_events(&mut self, events: &mut Vec<Event>) {
        if self.ships.0[4].armour > 50 && self.draw_page == 1 && self.rng.random(3) > 1 {
            let l = self.ships.0[0];
            for k in 0..3 {
                let slot = self.shots.highest_free(0..=40).unwrap_or(40);
                let (dx, ox, oy, heading) = match k {
                    0 => (0, l.sub_x, l.sub_y, 2),
                    1 => (1, l.sub_x + 32, l.sub_y, 2),
                    _ => (1, l.sub_x, l.sub_y - 10, self.rng.random(9) as u8),
                };
                self.shots.0[slot] = Shot {
                    tile_x: l.tile_x.wrapping_add(dx),
                    tile_y: l.tile_y,
                    off_x: ox,
                    off_y: oy,
                    heading,
                    kind: 2,
                    life: 1,
                    owner: 1,
                };
            }
        }
        for s in &mut self.ships.0[..4] {
            s.armour = 200;
        }
        if self.ships.0[4].armour < 50 {
            self.mission.count = self.mission.count.wrapping_add(1);
            self.ships.0[4].armour = 49;
            for s in 1..=5 {
                let t = self.slot_tile(s);
                if self.random_explosion(|_| t).is_some() {
                    events.push(Event::Sound {
                        kind: 9,
                        source: Some(t),
                    });
                }
            }
        }
        if self.mission.count > 149 {
            events.push(Event::Sound {
                kind: 100,
                source: None,
            });
            self.succeeded = true;
            self.hit_flash = 0;
            self.mission.white_out = true;
        }
        let ty = self.ships.0[4].tile_y;
        for (limit, d) in [(160, 2), (170, 4)] {
            if ty > limit {
                for s in &mut self.ships.0[..5] {
                    s.sub_y += d;
                }
            }
        }
        if ty > 194 {
            if self.mission.count < 2 {
                self.lost = true;
            } else {
                self.succeeded = true;
            }
        }
    }
}

/// Step 15 (`UI_CommViewer`, §4.6) and step 19 (`UI_DrawInstruments`: the flight-7 rule,
/// the ticker and the transmission indicator).
impl Flight {
    /// The comm viewer's table check, run at step 15 when F6 was requested on back page 0
    /// (the request is already cleared). Returns the transmission to show, after applying
    /// its phase advance; `None` when the request is only consumed.
    pub fn comm_viewer(&mut self) -> Option<u8> {
        if !self.mission.scripted {
            return None;
        }
        let (n, next) = match self.episode {
            Episode::E1 => transmission(self.number, self.phase())?,
            Episode::E2 => e2::transmission(self.number, self.phase())?,
            Episode::E3 => return None,
        };
        if self.episode == Episode::E1 && self.number == 14 {
            let all: Vec<usize> = (1..=20).collect();
            self.mark(&all, 0);
            self.mark(&[1, 2, 3, 4, 5, 6], MARK_TARGET);
        }
        self.set_phase(next);
        self.keys.clear(crate::input::FlightKey::F6);
        self.mission.comm_pending = false;
        self.mission.transmission = n;
        Some(n)
    }

    /// Marker of the guidance target: the lowest live marked slot, else the home point.
    pub fn guidance_marker(&self) -> u8 {
        self.ships
            .0
            .iter()
            .find(|s| s.marker != 0 && s.alive())
            .map_or(self.mission.home_marker, |s| s.marker)
    }

    pub(super) fn instruments(&mut self, events: &mut Vec<Event>) {
        // Guidance (hud §4.3): the flight-7 completion rule (mission-logic §5.7, U-180).
        if self.mission.scripted
            && self.episode == Episode::E1
            && self.number == 7
            && self.guidance_marker() == MARK_HOME
            && self.dead(&[2, 3, 4])
        {
            self.set_phase(PHASE_DONE);
        }
        // Ticker (hud §4.7).
        if self.edge_hit {
            // E2 has one warning text (episode-deltas §6).
            let m = match self.episode {
                Episode::E1 => Message::Jurisdiction(u8::from(matches!(self.number, 13 | 14))),
                _ => Message::E2Jurisdiction,
            };
            self.post(m);
            self.edge_hit = false;
        }
        self.mission.ticker.step(self.draw_page);
        // Transmission indicator (hud §4.8).
        let m = &mut self.mission;
        if m.comm_pending && m.comm_phase == 0 {
            events.push(Event::Sound {
                kind: SOUND_INCOMING,
                source: None,
            });
        }
        if m.comm_pending {
            m.comm_phase = if m.comm_phase < 7 {
                m.comm_phase + 1
            } else {
                1
            };
        } else if m.comm_phase != 0 {
            m.comm_phase = 0;
        }
    }

    /// The M key (weapons §4.1, step 33): replays the message when the ticker is idle.
    pub(super) fn ticker_replay(&mut self) {
        if self.keys.get(crate::input::FlightKey::M) && self.idle() {
            self.mission.ticker.pos = 1;
        }
    }

    /// `g_MissionOutcome` computed at teardown (§4.8).
    pub fn outcome(&self) -> bool {
        match self.episode {
            Episode::E1 => {}
            Episode::E2 => return self.e2_outcome(),
            // ASSUMPTION(M10-02): E3 outcome tests are not ported yet.
            Episode::E3 => return true,
        }
        let t = |s| self.slot_tile(s);
        match self.number {
            3 => t(4) == (107, 106),
            5 => t(1) == (199, 122),
            12 => {
                t(1) == (49, 199)
                    || t(2) == (50, 199)
                    || t(3) == (51, 199)
                    || t(4) == (49, 199)
                    || t(5) == (51, 199)
            }
            16 => {
                t(1) == (124, 71)
                    || t(2) == (125, 71)
                    || t(3) == (126, 71)
                    || t(8) == (125, 71)
                    || t(9) == (125, 71)
                    || t(10) == (125, 71)
            }
            17 => !self.lost,
            _ => true,
        }
    }

    /// `g_PlayerDied` at teardown: the end sequence ran to its end (35 end ticks).
    pub fn died(&self) -> bool {
        self.end_ticks == 35
    }
}

pub mod e2;

#[cfg(test)]
mod tests;
