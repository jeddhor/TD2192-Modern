//! The 20 mission-ship slots: setup, AI, movement, fire requests and removal.
//!
//! Spec: `docs/spec/sim/mission-ships.md` rev 2 (CONFIRMED against C-0009) and
//! `docs/spec/sim/mission-setup.md` rev 1. Fidelity: T1. Slot `i` here is the original's slot
//! `i + 1`; target selectors and `owner` fields keep the original's 1-based numbers.
//!
//! The ship record (1,065 bytes in the original) is kept as typed fields; [`Ship::record_byte`]
//! gives the original byte view, which the path cursor reads through (path step 0 is the cursor
//! itself, step 501 is (moveDir, aggression); mission-ships §4.3.5).

use td_formats::mission::{EMPTY_SLOT, Mission, PATH_LEN};

use super::World;
use super::tables::ClassTables;
use crate::hash::{HashState, StateHasher};
use crate::real48::{ONE, Real48};
use crate::rng::BpRandom;

/// Number of mission-ship slots.
pub const SHIP_SLOTS: usize = 20;
/// Class code of an empty or destroyed slot.
pub const NO_SHIP: u8 = EMPTY_SLOT;
/// Size of the original's ship record.
pub const RECORD_LEN: usize = 0x429;
/// Target selector: the player.
pub const TARGET_PLAYER: u8 = 0;
/// Target selector: the ship's return point.
pub const TARGET_RETURN: u8 = 0xFF;
/// Target selector: the fixed point (never used in `file0001`).
pub const TARGET_FIXED: u8 = 0xC8;

/// Speed component cap (`3292:0011`).
const SPEED_CAP: u8 = 42;
/// `voice_owner`-style "unowned" marker for `targetSel` etc. is not needed; FREE values below.
/// Detour length: `detourSteps` counts 0..=4 (§3.2).
const DETOUR_LAST: u8 = 4;
/// Heading ring with the wrap entries (`k_HeadingRing`, DS:02AD..02BC, plus ring[0] = 01h and
/// ring[17] = 0Eh).
const RING: [u8; 18] = [
    0x01, 0x0E, 0x04, 0x4A, 0x07, 0x4E, 0x08, 0x62, 0x09, 0x60, 0x06, 0x24, 0x03, 0x20, 0x02, 0x0C,
    0x01, 0x0E,
];
/// Flight 7: slots 2..=12 are pinned to these tiles every tick (§4.2.4).
const FLIGHT7_PINS: [(u8, u8); 11] = [
    (50, 182),
    (50, 167),
    (50, 123),
    (79, 122),
    (111, 122),
    (151, 122),
    (184, 122),
    (13, 122),
    (18, 124),
    (50, 101),
    (50, 36),
];

/// The ship record fields the original uses (mission-ships §3.1). Fields never written in
/// `file0001` (hold list, near limit, hold target, fixed point) are always 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Record {
    /// +000h.
    pub fixed_done: u8,
    /// +001h: the tile the ship is flying to.
    pub waypoint: (u8, u8),
    /// +003h.
    pub prev: (u8, u8),
    /// +005h.
    pub blocked_y: u8,
    /// +006h.
    pub blocked_x: u8,
    /// +007h.
    pub step_x: i16,
    /// +009h.
    pub step_y: i16,
    /// +00Bh: index of the last path step reached.
    pub path_cursor: u16,
    /// +00Dh: path steps 1..=500.
    pub path: [(u8, u8); PATH_LEN],
    /// +3F5h: 2, 4, 6 or 8.
    pub move_dir: u8,
    /// +3F6h.
    pub aggression: u8,
    /// +3FCh.
    pub hold_flag: u8,
    /// +411h.
    pub hold_index: u8,
    /// +412h.
    pub hold_count: u8,
    /// +414h (written, never read).
    pub near_flag: u8,
    /// +415h.
    pub attack_mode: u8,
    /// +416h.
    pub attack_counter: u16,
    /// +418h (written, never read).
    pub follower_flag: u8,
    /// +419h: 1 = right-hand wall follower in path/attack mode.
    pub hand_bit: u8,
    /// +41Ah.
    pub detour_steps: u8,
    /// +41Bh.
    pub direct_mode: u8,
    /// +41Ch.
    pub in_line: u8,
    /// +41Dh.
    pub out_of_range: u8,
    /// +41Fh: [`TARGET_PLAYER`], 1..=20 (that slot's waypoint), [`TARGET_RETURN`].
    pub target_sel: u8,
    /// +420h.
    pub target: (u8, u8),
    /// +426h.
    pub return_pt: (u8, u8),
    /// +428h.
    pub return_mode: u8,
}

impl Record {
    const ZERO: Record = Record {
        fixed_done: 0,
        waypoint: (0, 0),
        prev: (0, 0),
        blocked_y: 0,
        blocked_x: 0,
        step_x: 0,
        step_y: 0,
        path_cursor: 0,
        path: [(0, 0); PATH_LEN],
        move_dir: 0,
        aggression: 0,
        hold_flag: 0,
        hold_index: 0,
        hold_count: 0,
        near_flag: 0,
        attack_mode: 0,
        attack_counter: 0,
        follower_flag: 0,
        hand_bit: 0,
        detour_steps: 0,
        direct_mode: 0,
        in_line: 0,
        out_of_range: 0,
        target_sel: 0,
        target: (0, 0),
        return_pt: (0, 0),
        return_mode: 0,
    };
}

/// One mission-ship slot: the slot arrays plus the record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ship {
    /// Class code, `NO_SHIP` when empty or destroyed.
    pub class: u8,
    pub tile_x: u8,
    pub tile_y: u8,
    /// Pixel offsets added to the tile × 32.
    pub sub_x: i16,
    pub sub_y: i16,
    /// Heading as a keypad code.
    pub heading: u8,
    /// Heading the ship turns toward.
    pub target_heading: u8,
    /// 1 = counter-clockwise, 2 = clockwise.
    pub turn: u8,
    /// Speed components up, down, left, right, biased by 16.
    pub speed: [u8; 4],
    /// Copy of the waypoint used by the thrust step.
    pub goal: (u8, u8),
    /// Target bracket marker: 0, 32h or 39h (§4.7).
    pub marker: u8,
    /// Fire requests, entries 1..=9 at index 0..=8 (the AI sets 2, 4, 6, 8).
    pub fire: [bool; 9],
    /// Ship record +3F8h.
    pub shields: i16,
    /// Ship record +3FAh.
    pub armour: i16,
    pub record: Record,
}

impl Ship {
    /// An empty slot.
    pub const EMPTY: Ship = Ship {
        class: NO_SHIP,
        tile_x: 0,
        tile_y: 0,
        sub_x: 0,
        sub_y: 0,
        heading: 8,
        target_heading: 8,
        turn: 0,
        speed: [0; 4],
        goal: (0, 0),
        marker: 0,
        fire: [false; 9],
        shields: 0,
        armour: 0,
        record: Record::ZERO,
    };

    /// True while the slot holds a ship.
    pub fn alive(&self) -> bool {
        self.class != NO_SHIP
    }

    /// World pixel (`tile × 32 + offset` per axis).
    pub fn world(&self) -> (i32, i32) {
        (
            i32::from(self.tile_x) * 32 + i32::from(self.sub_x),
            i32::from(self.tile_y) * 32 + i32::from(self.sub_y),
        )
    }

    /// Byte `off` of the original's ship record (0 for bytes the original never writes and for
    /// offsets past the record).
    pub fn record_byte(&self, off: usize) -> u8 {
        let r = &self.record;
        let word = |v: u16, hi: bool| if hi { (v >> 8) as u8 } else { v as u8 };
        match off {
            0x000 => r.fixed_done,
            0x001 => r.waypoint.0,
            0x002 => r.waypoint.1,
            0x003 => r.prev.0,
            0x004 => r.prev.1,
            0x005 => r.blocked_y,
            0x006 => r.blocked_x,
            0x007 | 0x008 => word(r.step_x as u16, off == 0x008),
            0x009 | 0x00A => word(r.step_y as u16, off == 0x00A),
            0x00B | 0x00C => word(r.path_cursor, off == 0x00C),
            0x00D..=0x3F4 => {
                let (k, odd) = ((off - 0x00D) / 2, (off - 0x00D) % 2 == 1);
                if odd { r.path[k].1 } else { r.path[k].0 }
            }
            0x3F5 => r.move_dir,
            0x3F6 => r.aggression,
            0x3F8 | 0x3F9 => word(self.shields as u16, off == 0x3F9),
            0x3FA | 0x3FB => word(self.armour as u16, off == 0x3FB),
            0x3FC => r.hold_flag,
            0x411 => r.hold_index,
            0x412 => r.hold_count,
            0x414 => r.near_flag,
            0x415 => r.attack_mode,
            0x416 | 0x417 => word(r.attack_counter, off == 0x417),
            0x418 => r.follower_flag,
            0x419 => r.hand_bit,
            0x41A => r.detour_steps,
            0x41B => r.direct_mode,
            0x41C => r.in_line,
            0x41D => r.out_of_range,
            0x41F => r.target_sel,
            0x420 => r.target.0,
            0x421 => r.target.1,
            0x426 => r.return_pt.0,
            0x427 => r.return_pt.1,
            0x428 => r.return_mode,
            _ => 0,
        }
    }

    /// The whole record as the original stores it.
    pub fn record_bytes(&self) -> Vec<u8> {
        (0..RECORD_LEN).map(|o| self.record_byte(o)).collect()
    }

    /// Path step `k` read through the record (step 0 = the cursor word, 501 = (moveDir,
    /// aggression)).
    fn path_step(&self, k: u16) -> (u8, u8) {
        let o = 0x00B + 2 * usize::from(k);
        (self.record_byte(o), self.record_byte(o + 1))
    }
}

/// −1, 0 or +1 from `a` toward `b`.
fn sign(a: u8, b: u8) -> i16 {
    i16::from(b > a) - i16::from(b < a)
}

/// `a + d` with byte wrap.
fn add(a: u8, d: i16) -> u8 {
    (i16::from(a) + d) as u8
}

/// The 16-bit wrapped squared tile distance (§2 `D`), as its absolute value.
fn dist_sq(a: (u8, u8), b: (u8, u8)) -> u16 {
    let dx = i16::from(b.0) - i16::from(a.0);
    let dy = i16::from(b.1) - i16::from(a.1);
    dx.wrapping_mul(dx)
        .wrapping_add(dy.wrapping_mul(dy))
        .unsigned_abs()
}

/// What the AI reads besides the ships.
#[derive(Debug)]
pub struct AiContext<'a> {
    pub world: &'a World,
    pub rng: &'a mut BpRandom,
    /// Player tile (DS:9766/9767).
    pub player: (u8, u8),
}

impl AiContext<'_> {
    /// `Blocked(x, y)`: any of the 2 × 2 cells from (x, y) is not open (§2).
    fn blocked(&self, x: u8, y: u8) -> bool {
        [
            (x, y),
            (add(x, 1), y),
            (x, add(y, 1)),
            (add(x, 1), add(y, 1)),
        ]
        .iter()
        .any(|&(cx, cy)| self.world.solid(i16::from(cx), i16::from(cy)))
    }
}

/// Facing helpers for the wall followers (§4.3.7).
fn ahead(d: u8) -> (i16, i16) {
    match d {
        6 => (1, 0),
        2 => (0, 1),
        4 => (-1, 0),
        _ => (0, -1),
    }
}
fn right_side(d: u8) -> (i16, i16) {
    match d {
        6 => (0, 1),
        2 => (-1, 0),
        4 => (0, -1),
        _ => (1, 0),
    }
}
fn turn_right(d: u8) -> u8 {
    match d {
        6 => 2,
        2 => 4,
        4 => 8,
        _ => 6,
    }
}
fn turn_left(d: u8) -> u8 {
    match d {
        6 => 8,
        2 => 6,
        4 => 2,
        _ => 4,
    }
}

/// All mission-ship slots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ships(pub [Ship; SHIP_SLOTS]);

impl Default for Ships {
    fn default() -> Self {
        Self([Ship::EMPTY; SHIP_SLOTS])
    }
}

impl Ships {
    /// `AI_ResetShipRecords` (§4.1 step 1; also run once at program start). Draws one
    /// `Random(2)` per slot for the wall-follower hand. Fields +411h, +418h, +41Ah, +41Bh keep
    /// their values.
    pub fn reset_records(&mut self, rng: &mut BpRandom) {
        for s in &mut self.0 {
            let keep = s.record;
            let r = &mut s.record;
            *r = Record {
                hold_index: keep.hold_index,
                follower_flag: keep.follower_flag,
                detour_steps: keep.detour_steps,
                direct_mode: keep.direct_mode,
                ..Record::ZERO
            };
            // Only path steps 1..=255 are cleared.
            r.path[255..].copy_from_slice(&keep.path[255..]);
            s.shields = 0;
            r.hand_bit = u8::from(rng.random(2) == 1);
        }
    }

    /// Setup steps 2–4 (§4.1): classes and paths from the mission file, shields and armour
    /// from the class tables, start tile = path step 1.
    pub fn load_mission(&mut self, m: &Mission, classes: &ClassTables) {
        for (i, s) in self.0.iter_mut().enumerate() {
            s.class = m.slot_class[i];
            s.shields = i16::from(classes.shields(s.class));
            s.armour = 2 * i16::from(classes.armour(s.class));
            if let Some(path) = m.path(i) {
                for (k, step) in path.enumerate().take(PATH_LEN) {
                    s.record.path[k] = step;
                }
            }
            s.record.waypoint = s.record.path[0];
            s.record.return_pt = s.record.waypoint;
            (s.tile_x, s.tile_y) = s.record.path[0];
            s.heading = 6;
            s.target_heading = 6;
            s.turn = 0;
            s.sub_x = 16;
            s.sub_y = 16;
            s.speed = [0; 4];
        }
    }

    /// Setup step 6: `AI_ChooseWaypoint` for every slot, live or not.
    pub fn choose_all(&mut self, ctx: &mut AiContext<'_>) {
        for s in 0..SHIP_SLOTS {
            self.choose_waypoint(s, ctx);
        }
    }

    /// `AI_OrderAttack(slot, target)` (§4.3.10), called by mission logic: `slot` is 0-based,
    /// `target` is a target selector ([`TARGET_PLAYER`], 1..=20, [`TARGET_RETURN`]). The
    /// attack starts at the ship's next waypoint arrival.
    pub fn order_attack(&mut self, slot: usize, target: u8) {
        let r = &mut self.0[slot].record;
        r.target_sel = target;
        r.attack_mode = 1;
        r.direct_mode = 1;
    }

    /// `AI_UpdateShips` (tick-order step 10, §4.2): shield regeneration and removal, then for
    /// every live ship the AI, turn, thrust, drift, carry and edge removal.
    pub fn update(&mut self, flight: u8, classes: &ClassTables, ctx: &mut AiContext<'_>) {
        for s in self.0.iter_mut().filter(|s| s.alive()) {
            if s.shields < i16::from(classes.shields(s.class)) {
                s.shields += 1;
            }
            let w = s.record.waypoint;
            if (w.0 == 50 && w.1 < 2) || s.armour < 1 {
                s.class = NO_SHIP;
            }
        }
        for i in 0..SHIP_SLOTS {
            if !self.0[i].alive() {
                continue;
            }
            let sinking = flight == 20 && i < 5;
            // (a) is a no-op (OB-013). (b) New waypoint on arrival.
            let s = &self.0[i];
            if s.record.waypoint == (s.tile_x, s.tile_y) && !sinking {
                self.choose_waypoint(i, ctx);
            }
            let s = &mut self.0[i];
            s.goal = s.record.waypoint;
            s.turn_step();
            if sinking {
                s.heading = 8;
                s.sub_y += 2;
            } else {
                let flags = s.thrust(classes);
                s.choose_target_heading(flags);
            }
            for v in &mut s.speed {
                *v = (*v).min(SPEED_CAP);
            }
            s.drift();
            if flight == 7 {
                for (p, &(x, y)) in self.0[1..12].iter_mut().zip(&FLIGHT7_PINS) {
                    (p.tile_x, p.tile_y) = (x, y);
                    (p.sub_x, p.sub_y) = (16, 16);
                    p.heading = 8;
                    p.target_heading = 8;
                }
            }
        }
    }

    /// Clears every fire request (tick-order step 32).
    pub fn clear_fire_requests(&mut self) {
        for s in &mut self.0 {
            s.fire = [false; 9];
        }
    }

    /// Target position (§4.3.2).
    fn target_position(&mut self, i: usize, ctx: &AiContext<'_>) {
        let r = &self.0[i].record;
        let t = match r.target_sel {
            TARGET_PLAYER => ctx.player,
            TARGET_RETURN => r.return_pt,
            TARGET_FIXED => (0, 0),
            n => self
                .0
                .get(usize::from(n) - 1)
                .map_or((0, 0), |o| o.record.waypoint),
        };
        self.0[i].record.target = t;
    }

    /// `AI_ChooseWaypoint` (§4.3).
    pub fn choose_waypoint(&mut self, i: usize, ctx: &mut AiContext<'_>) {
        self.target_position(i, ctx);
        let r = self.0[i].record;
        if r.return_mode == 0 {
            self.0[i].record.near_flag = 0;
        }
        if self.0[i].record.hold_flag == 1 {
            self.hold_schedule(i);
        }
        let r = self.0[i].record;
        if r.attack_mode == 0 && r.return_mode == 0 && r.hold_flag == 0 {
            self.path_step(i);
        }
        let r = self.0[i].record;
        if r.attack_mode == 1 && r.direct_mode == 1 {
            self.attack_step(i, ctx);
        } else if r.attack_mode == 1 {
            self.detour_move(i, ctx);
        }
        let r = self.0[i].record;
        if r.return_mode == 1 && r.direct_mode == 1 {
            self.return_step(i, ctx);
        } else if r.return_mode == 1 {
            self.detour_move(i, ctx);
        }
        let r = &mut self.0[i].record;
        if r.waypoint != r.prev {
            if r.waypoint.0 < r.prev.0 {
                r.move_dir = 4;
            }
            if r.prev.0 < r.waypoint.0 {
                r.move_dir = 6;
            }
            if r.waypoint.1 < r.prev.1 {
                r.move_dir = 8;
            }
            if r.prev.1 < r.waypoint.1 {
                r.move_dir = 2;
            }
        }
    }

    /// Hold schedule (§4.3.6), with the empty hold list of `file0001`.
    fn hold_schedule(&mut self, i: usize) {
        let s = &self.0[i];
        let idx = usize::from(s.record.hold_index);
        let below = s.record_byte(0x3FC + idx) < s.record.hold_count;
        let listed = s.record_byte(0x3FD + idx);
        let r = &mut self.0[i].record;
        if below {
            if listed == 0 {
                r.hold_index = 1;
                r.return_mode = 1;
                r.direct_mode = 1;
                r.target_sel = TARGET_RETURN;
                r.hold_flag = 0;
                r.hold_count = 0;
                r.attack_mode = 0;
            } else {
                r.hold_index = r.hold_index.wrapping_add(1);
                r.attack_mode = 1;
                r.target_sel = 0; // hold target, never written
                r.direct_mode = 1;
                r.return_mode = 0;
            }
        } else {
            r.hold_count = r.hold_count.wrapping_add(1);
            r.return_mode = 1;
            r.direct_mode = 1;
            r.target_sel = TARGET_RETURN;
        }
    }

    /// Path step (§4.3.5).
    fn path_step(&mut self, i: usize) {
        let s = &self.0[i];
        let c = s.record.path_cursor;
        let next = s.path_step(c.wrapping_add(1));
        let after = s.path_step(c.wrapping_add(2));
        let r = &mut self.0[i].record;
        r.return_mode = 0;
        let w = r.waypoint;
        let (dx, dy) = (sign(w.0, next.0), sign(w.1, next.1));
        r.prev = w;
        r.return_pt = w;
        r.waypoint = (add(w.0, dx), add(w.1, dy));
        if r.waypoint == next {
            r.path_cursor = c.wrapping_add(1);
            if after.0 == 0 {
                r.path_cursor = 1;
            }
        }
    }

    /// Attack step (§4.3.3).
    fn attack_step(&mut self, i: usize, ctx: &mut AiContext<'_>) {
        let r = &mut self.0[i].record;
        let d = dist_sq(r.waypoint, r.target);
        let in_line = d < 9 && (r.waypoint.0 == r.target.0 || r.waypoint.1 == r.target.1);
        r.in_line = u8::from(in_line);
        r.hold_flag = r.in_line;
        if 15 * u16::from(r.aggression) < r.attack_counter {
            r.attack_counter = 0;
            r.detour_steps = 0;
            r.direct_mode = 1;
            r.return_mode = 1;
            r.target_sel = TARGET_RETURN;
        } else {
            r.attack_counter = r.attack_counter.wrapping_add(1);
        }
        self.fire_decision(i, d, ctx);
        self.target_position(i, ctx);
        if !in_line {
            self.0[i].record.detour_steps = 0;
            self.direct_step(i, true, ctx);
        }
    }

    /// Fire decision (§4.4.1). `d` is the squared distance to the target position from the
    /// start of the call.
    fn fire_decision(&mut self, i: usize, d: u16, ctx: &mut AiContext<'_>) {
        let r = &mut self.0[i].record;
        r.out_of_range = u8::from(d > 121);
        let hundred = Real48::from_i32(100);
        let roll = ctx
            .rng
            .random_real()
            .checked_mul(hundred)
            .and_then(|x| x.checked_add(ONE))
            .expect("in range");
        let budget = Real48::from_i32(5 * i32::from(r.aggression));
        if budget.compare(roll) != std::cmp::Ordering::Greater || r.out_of_range != 0 {
            return;
        }
        let (w, t) = (r.waypoint, r.target);
        let scan = |along_y: bool| {
            let n = if along_y {
                w.1.abs_diff(t.1)
            } else {
                w.0.abs_diff(t.0)
            };
            (0..=i16::from(n)).any(|k| {
                if along_y {
                    ctx.blocked(w.0, add(w.1, k))
                } else {
                    ctx.blocked(add(w.0, k), w.1)
                }
            })
        };
        let v_blocked = w.0 == t.0 && scan(true);
        let h_blocked = w.1 == t.1 && scan(false);
        let near = |a: u8, b: u8| a == b || a == add(b, -1) || a == add(b, 1);
        let s = &mut self.0[i];
        let mut request = |e: u8| {
            s.fire[usize::from(e) - 1] = true;
            s.record.attack_counter = s
                .record
                .attack_counter
                .wrapping_add(u16::from(s.record.aggression));
        };
        if !v_blocked && near(t.0, w.0) {
            request(if w.1 < t.1 { 2 } else { 8 });
        }
        if !h_blocked && near(t.1, w.1) {
            request(if w.0 < t.0 { 6 } else { 4 });
        }
    }

    /// Direct step toward the target (§4.3.3).
    fn direct_step(&mut self, i: usize, attack: bool, ctx: &mut AiContext<'_>) {
        let r = &mut self.0[i].record;
        let w = r.waypoint;
        if r.blocked_x == 0 {
            r.step_x = sign(w.0, r.target.0);
        }
        if r.blocked_y == 0 {
            r.step_y = sign(w.1, r.target.1);
        }
        let (mut dx, mut dy) = (0, 0);
        if !ctx.blocked(add(w.0, r.step_x), w.1) {
            dx = r.step_x;
        } else {
            r.step_x = 0;
            r.blocked_x = 1;
        }
        if !ctx.blocked(w.0, add(w.1, r.step_y)) {
            dy = r.step_y;
        } else {
            r.step_y = 0;
            r.blocked_y = 1;
        }
        if (r.blocked_y == 1 && r.step_y == 0)
            || (r.blocked_x == 1 && r.step_x == 0)
            || (attack && r.direct_mode == 0)
        {
            (dx, dy) = self.detour(i, ctx);
        }
        let r = &mut self.0[i].record;
        r.prev = r.waypoint;
        r.waypoint = (add(r.waypoint.0, dx), add(r.waypoint.1, dy));
    }

    /// Return step (§4.3.8).
    fn return_step(&mut self, i: usize, ctx: &mut AiContext<'_>) {
        self.0[i].record.attack_mode = 0;
        self.target_position(i, ctx);
        let r = &mut self.0[i].record;
        if r.waypoint == r.target {
            r.return_mode = 0;
            r.target_sel = TARGET_PLAYER;
            r.near_flag = 0;
        } else {
            self.direct_step(i, false, ctx);
        }
    }

    /// `DetourMove` (§4.3).
    fn detour_move(&mut self, i: usize, ctx: &mut AiContext<'_>) {
        let (dx, dy) = self.detour(i, ctx);
        let r = &mut self.0[i].record;
        r.prev = r.waypoint;
        r.waypoint = (add(r.waypoint.0, dx), add(r.waypoint.1, dy));
    }

    /// Detour (§4.3.7): one wall-follower step.
    fn detour(&mut self, i: usize, ctx: &mut AiContext<'_>) -> (i16, i16) {
        let r = self.0[i].record;
        let right = (r.hand_bit == 1) == (r.return_mode == 0);
        let step = self.follow(i, right, ctx);
        let r = &mut self.0[i].record;
        r.direct_mode = 0;
        if r.detour_steps < DETOUR_LAST {
            r.detour_steps += 1;
        } else {
            r.detour_steps = 0;
            r.direct_mode = 1;
        }
        step
    }

    /// The right- or left-hand wall follower (§4.3.7).
    fn follow(&mut self, i: usize, right: bool, ctx: &AiContext<'_>) -> (i16, i16) {
        let r = &mut self.0[i].record;
        let (x, y) = r.waypoint;
        let b = |dx: i16, dy: i16| ctx.blocked(add(x, dx), add(y, dy));
        // 1. Re-orient: every test on the original facing, later ones overwrite.
        let d0 = r.move_dir;
        let rules: [(u8, (i16, i16), u8); 8] = if right {
            [
                (6, (0, -1), 4),
                (4, (0, 1), 6),
                (2, (1, 0), 8),
                (8, (-1, 0), 2),
                (6, (1, 0), 8),
                (4, (-1, 0), 2),
                (2, (0, 1), 6),
                (8, (0, -1), 4),
            ]
        } else {
            [
                (6, (0, 1), 4),
                (4, (0, -1), 6),
                (2, (-1, 0), 8),
                (8, (1, 0), 2),
                (6, (1, 0), 2),
                (4, (-1, 0), 8),
                (2, (0, 1), 4),
                (8, (0, -1), 6),
            ]
        };
        let mut d = d0;
        for (when, (cx, cy), to) in rules {
            if d0 == when && b(cx, cy) {
                d = to;
            }
        }
        r.move_dir = d;
        if !matches!(d, 2 | 4 | 6 | 8) {
            // Endless loop in the original; not reachable in Episode 1 (U-162): no step.
            return (0, 0);
        }
        // 2. One action.
        let side = |d: u8| {
            let s = right_side(d);
            if right { s } else { (-s.0, -s.1) }
        };
        let toward = |d: u8| if right { turn_right(d) } else { turn_left(d) };
        let away = |d: u8| if right { turn_left(d) } else { turn_right(d) };
        let (sx, sy) = side(d);
        let (ax, ay) = ahead(d);
        // OB-015: the right-hand follower facing south tests the cell ahead.
        let (px, py) = if right && d == 2 { (ax, ay) } else { (sx, sy) };
        let mut step = (0, 0);
        if b(sx, sy) && !b(ax, ay) {
            step = (ax, ay);
        } else if !b(px, py) {
            d = toward(d);
            step = ahead(d);
        } else if b(ax, ay) {
            d = away(d);
            step = ahead(d);
        }
        r.move_dir = d;
        // 3. Release the blocked axes.
        let (tx, ty) = r.target;
        let vertical = matches!(d, 2 | 8);
        let horizontal = matches!(d, 4 | 6);
        if x <= tx && vertical && !b(1, 0) {
            r.blocked_y = 0;
        }
        if tx <= x && vertical && !b(-1, 0) {
            r.blocked_y = 0;
        }
        if y <= ty && horizontal && !b(0, -1) {
            r.blocked_x = 0;
        }
        if ty <= y && horizontal && !b(0, 1) {
            r.blocked_x = 0;
        }
        r.follower_flag = u8::from(r.blocked_x != 0 || r.blocked_y != 0);
        step
    }
}

/// Thrust flags of one ship's thrust step (DS:3BE3..3BE9).
#[derive(Debug, Clone, Copy, Default)]
struct Flags {
    up: bool,
    down: bool,
    left: bool,
    right: bool,
}

impl Ship {
    /// Turn one ring step toward the target heading (§4.2 (d)).
    fn turn_step(&mut self) {
        if self.target_heading == self.heading {
            return;
        }
        if let Some(i) = RING[1..17].iter().position(|&c| c == self.heading) {
            let i = i + 1;
            match self.turn {
                1 => self.heading = RING[i - 1],
                2 => self.heading = RING[i + 1],
                _ => {}
            }
        }
    }

    /// Thrust toward the goal (§4.2.1). 8-bit arithmetic; OB-014 preserved.
    fn thrust(&mut self, classes: &ClassTables) -> Flags {
        let (max, acc) = (classes.max_speed(self.class), classes.accel(self.class));
        let (gx, gy) = self.goal;
        let (tx, ty) = (self.tile_x, self.tile_y);
        let [u, d, l, r] = &mut self.speed;
        let mut f = Flags::default();
        if gy < ty && *u < max {
            *u = u.wrapping_add(acc);
            if *d > 16 && gy <= ty {
                *u = u.wrapping_sub(acc);
            }
            if *l > 16 && tx <= gx {
                *l = l.wrapping_sub(acc);
            }
            if *r > 16 && gx <= tx {
                *r = r.wrapping_sub(acc);
            }
            f.up = true;
        }
        if ty < gy && *d < max {
            *d = d.wrapping_add(acc);
            if *u > 16 && ty <= gy {
                *u = u.wrapping_sub(acc);
            }
            if *l > 16 && tx <= gx {
                *l = l.wrapping_sub(acc);
            }
            if *r > 16 && gx <= tx {
                *r = r.wrapping_sub(acc);
            }
            f.down = true;
        }
        if gx < tx && *l < max {
            if *u > 16 && ty <= gy {
                *u = u.wrapping_sub(acc);
            }
            if *d > 16 && gy <= ty {
                *d = d.wrapping_sub(acc);
            }
            if *r > 16 && gx <= tx {
                *r = r.wrapping_sub(acc);
            }
            *l = l.wrapping_add(acc);
            f.left = true;
        }
        if tx < gx && *r < max {
            if *u > 16 && ty <= gy {
                *u = u.wrapping_sub(acc);
            }
            if *d > 16 && gy <= ty {
                *d = d.wrapping_sub(acc);
            }
            if *l > 16 && tx <= gx {
                *l = l.wrapping_sub(acc);
            }
            *r = r.wrapping_add(acc);
            f.right = true;
        }
        f
    }

    /// Target heading and turn direction from the thrust flags (§4.2.2).
    fn choose_target_heading(&mut self, f: Flags) {
        let h = self.heading;
        let (target, cw, ccw, unless_same): (u8, &[u8], &[u8], bool) =
            match (f.up, f.down, f.left, f.right) {
                (true, _, _, true) => (9, &[7, 8, 4, 1], &[6, 2, 3], false),
                (true, _, true, _) => (7, &[2, 4, 1], &[9, 6, 8], false),
                (true, _, _, _) => (8, &[7, 4, 1], &[9, 6, 3, 2], true),
                (_, true, _, true) => (3, &[9, 8, 6, 7], &[1, 2, 4], false),
                (_, true, true, _) => (1, &[2, 3, 6, 9], &[8, 7, 4], false),
                (_, true, _, _) => (2, &[9, 6, 3], &[7, 4, 1, 8], true),
                (_, _, true, _) => (4, &[3, 2, 1], &[9, 8, 7, 6], true),
                (_, _, _, true) => (6, &[9, 8, 7, 4], &[3, 2, 1], true),
                _ => return,
            };
        if unless_same && h == target {
            return;
        }
        self.target_heading = target;
        if cw.contains(&h) {
            self.turn = 2;
        }
        if ccw.contains(&h) {
            self.turn = 1;
        }
    }

    /// Drift, drag, carry and edge removal (§4.2.3).
    fn drift(&mut self) {
        while self.sub_x > 80 || self.sub_x < -16 {
            if self.sub_x > 80 {
                self.sub_x -= 32;
            }
            if self.sub_x < -16 {
                self.sub_x += 32;
            }
        }
        let [u, d, l, r] = &mut self.speed;
        if *u > 16 {
            self.sub_y += 16 - i16::from(*u);
            *u -= 1;
        }
        if self.sub_y < 16 && self.tile_y > 1 {
            self.tile_y -= 1;
            self.sub_y += 32;
        }
        if self.sub_y < 16 && self.tile_y == 1 {
            self.class = NO_SHIP;
        }
        if *d > 16 {
            self.sub_y += i16::from(*d) - 16;
            *d -= 1;
        }
        if self.sub_y > 48 && self.tile_y < 199 {
            self.tile_y += 1;
            self.sub_y -= 32;
        }
        if self.tile_y == 200 || self.tile_x == 200 || self.tile_x == 0 || self.tile_y == 0 {
            self.class = NO_SHIP;
        }
        if *l > 16 {
            self.sub_x += 16 - i16::from(*l);
            *l -= 1;
        }
        if self.sub_x < 16 && self.tile_x > 1 {
            self.tile_x -= 1;
            self.sub_x += 32;
        }
        if self.sub_x < 16 && self.tile_x == 1 {
            self.class = NO_SHIP;
        }
        if *r > 16 {
            self.sub_x += i16::from(*r) - 16;
            *r -= 1;
        }
        if self.sub_x > 48 && self.tile_x < 199 {
            self.tile_x += 1;
            self.sub_x -= 32;
        }
        if self.sub_x > 48 && self.tile_x == 199 {
            self.class = NO_SHIP;
        }
        if self.sub_x > 48 {
            self.sub_x -= 32;
        }
    }
}

impl HashState for Ships {
    fn hash_state(&self, h: &mut StateHasher) {
        for s in &self.0 {
            h.bytes(&[
                s.class,
                s.tile_x,
                s.tile_y,
                s.heading,
                s.target_heading,
                s.turn,
                s.goal.0,
                s.goal.1,
                s.marker,
            ]);
            h.bytes(&s.speed);
            h.bytes(&s.fire.map(u8::from));
            for v in [s.sub_x, s.sub_y] {
                v.hash_state(h);
            }
            h.bytes(&s.record_bytes());
        }
    }
}

#[cfg(test)]
mod tests;
