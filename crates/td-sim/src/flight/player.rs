//! The player's ship: view position, the four biased speed components, heading and turning,
//! afterburner burst, shields and armour.
//!
//! Spec: `docs/spec/sim/flight-model.md` rev 1 (§ numbers below). Fidelity: T1. Integer only.
//! The player has no position of its own: it sits at the centre of the playfield and the
//! **view** moves.

use super::World;
use super::heading::Heading;
use super::tables::{CLASS_SHIELDS, ClassTables};
use crate::hash::{HashState, StateHasher};
use crate::input::{FlightKey, Joystick, KeyTable};

/// View limits in tiles (§4.4).
const VIEW_X: (i16, i16) = (6, 183);
const VIEW_Y: (i16, i16) = (7, 190);
/// Speed values at or below this do not move the ship (the bias).
const SPEED_BIAS: u8 = 16;

/// The camera: a map tile plus a pixel offset per axis (offsets kept in 16..=48 between ticks).
/// The playfield's top-left world pixel is `tile * 32 + offset - 16`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct View {
    pub tile_x: i16,
    pub tile_y: i16,
    pub sub_x: i16,
    pub sub_y: i16,
}

impl View {
    /// World pixel of the playfield's top-left corner.
    pub fn world_origin(&self) -> (i32, i32) {
        (
            i32::from(self.tile_x) * 32 + i32::from(self.sub_x) - 16,
            i32::from(self.tile_y) * 32 + i32::from(self.sub_y) - 16,
        )
    }
}

/// Speed components, each biased by 16: a value `s > 16` moves the view `s - 16` px per tick.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Speeds {
    pub up: u8,
    pub down: u8,
    pub left: u8,
    pub right: u8,
}

/// Turn direction toward the target heading.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum Turn {
    #[default]
    None = 0,
    CounterClockwise = 1,
    Clockwise = 2,
}

/// The direction predicates of §3, from the key table and the joystick.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Dirs {
    u: bool,
    d: bool,
    l: bool,
    r: bool,
    ufull: bool,
    dfull: bool,
    lfull: bool,
    rfull: bool,
}

impl Dirs {
    pub(crate) fn read(k: &KeyTable, joy: Joystick) -> Self {
        use FlightKey::*;
        Self {
            u: k.get(Kp8) || joy.y == 1 || k.get(Up),
            d: k.get(Kp2) || joy.y == -1 || k.get(Down),
            l: k.get(Kp4) || joy.x == 1 || k.get(Left),
            r: k.get(Kp6) || joy.x == -1 || k.get(Right),
            // The "full" predicates pair each key with the opposite joystick direction
            // (original bug, preserved).
            ufull: k.get(Kp8) && k.get(Up) && joy.y < 0,
            dfull: k.get(Kp2) && k.get(Down) && joy.y >= 1,
            lfull: k.get(Kp4) && k.get(Left) && joy.x < 0,
            rfull: k.get(Kp6) && k.get(Right) && joy.x >= 1,
        }
    }
}

/// A sound the simulation asks the presentation layer to start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// `kind` is the shooter's class code, or 0 for the player; `source` is the source tile, or
    /// `None` for the player's own ship (the original passes FFFFh).
    Sound { kind: u8, source: Option<(u8, u8)> },
}

/// The player's flight state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Player {
    pub class: u8,
    pub view: View,
    pub speed: Speeds,
    pub heading: Heading,
    pub target: Heading,
    pub turn: Turn,
    pub armour: i8,
    pub shields: i8,
    /// Afterburner burst frame (0 = idle, 1..=5).
    pub burst_frame: u8,
    pub burst_heading: Heading,
}

impl Player {
    /// The state at flight start for `class` with the view at `view` (§3).
    pub fn new(class: u8, view: View) -> Self {
        let c = usize::from(class);
        Self {
            class,
            view,
            speed: Speeds::default(),
            heading: Heading::N,
            target: Heading::N,
            turn: Turn::None,
            armour: super::tables::CLASS_ARMOUR[c] as i8,
            shields: CLASS_SHIELDS[c] as i8,
            burst_frame: 0,
            burst_heading: Heading::N,
        }
    }

    /// Turn step (§4.6, tick-order step 3): one ring step toward the target.
    pub(crate) fn turn_step(&mut self) {
        if self.target != self.heading {
            match self.turn {
                Turn::CounterClockwise => self.heading = self.heading.ccw(),
                Turn::Clockwise => self.heading = self.heading.cw(),
                Turn::None => {}
            }
        }
    }

    /// Afterburner burst of classes 2 and 5 (§4.7, tick-order step 5). Returns the flame
    /// sprite drawn this tick, if any.
    pub(crate) fn afterburner(
        &mut self,
        keys: &mut KeyTable,
        button1: bool,
        over: bool,
        events: &mut Vec<Event>,
    ) -> Option<u8> {
        let triggered = keys.get(FlightKey::KpPlus) || keys.get(FlightKey::Kp0) || button1;
        if triggered && matches!(self.class, 2 | 5) && !over {
            if self.burst_frame == 0 && self.heading.is_principal() {
                self.burst_frame = 1;
                self.burst_heading = self.heading;
                events.push(Event::Sound {
                    kind: 0,
                    source: None,
                });
            }
            // One burst per make code.
            keys.clear(FlightKey::KpPlus);
            keys.clear(FlightKey::Kp0);
        }
        if self.burst_frame == 0 {
            return None;
        }
        self.heading = self.burst_heading;
        match self.burst_frame {
            2 => self.set_along(self.burst_heading, 34),
            3 => self.set_along(self.burst_heading, 44),
            _ => {}
        }
        let flame = 9 + self.burst_frame;
        self.burst_frame += 1;
        if self.burst_frame > 5 {
            self.burst_frame = 0;
        }
        Some(flame)
    }

    /// Sets every speed component along a principal heading (keypad codes: 7..=9 point up,
    /// 1..=3 down, 1/4/7 left, 3/6/9 right).
    fn set_along(&mut self, h: Heading, v: u8) {
        let c = h.code();
        if matches!(c, 4 | 1 | 7) {
            self.speed.left = v;
        }
        if matches!(c, 6 | 9 | 3) {
            self.speed.right = v;
        }
        if matches!(c, 7..=9) {
            self.speed.up = v;
        }
        if matches!(c, 1..=3) {
            self.speed.down = v;
        }
    }

    /// Shields and destruction (§4.1, tick-order step 27). Returns true when the player was
    /// destroyed this tick.
    pub(crate) fn shields_step(&mut self, classes: &ClassTables, over: bool) -> bool {
        if self.shields < 1 {
            self.shields = 0;
        }
        let destroyed = self.armour < 1 && !over;
        if self.shields < classes.shields(self.class) as i8 {
            self.shields += 1;
        }
        destroyed
    }

    /// Drift and drag, map collision, tile carry and map limits (§4.2–4.4, step 28). Returns
    /// true when the view was pushed back at a map limit (the edge alert).
    pub(crate) fn move_step(&mut self, world: &World, collide: bool) -> bool {
        self.drift();
        if collide {
            self.collide(world);
        }
        self.carry()
    }

    fn drift(&mut self) {
        let (s, v) = (&mut self.speed, &mut self.view);
        let step = |s: u8| i16::from(s - SPEED_BIAS);
        if s.up > SPEED_BIAS {
            v.sub_y = v.sub_y.wrapping_sub(step(s.up));
            s.up -= 1;
        }
        if s.down > SPEED_BIAS {
            v.sub_y = v.sub_y.wrapping_add(step(s.down));
            s.down -= 1;
        }
        if s.left > SPEED_BIAS {
            v.sub_x = v.sub_x.wrapping_sub(step(s.left));
            s.left -= 1;
        }
        if s.right > SPEED_BIAS {
            v.sub_x = v.sub_x.wrapping_add(step(s.right));
            s.right -= 1;
        }
    }

    fn collide(&mut self, world: &World) {
        let (tx, ty) = (self.view.tile_x, self.view.tile_y);
        let solid = |dx: i16, dy: i16| world.solid(tx + dx, ty + dy);
        let (s, v) = (&mut self.speed, &mut self.view);
        if v.sub_x < 32 && (solid(4, 3) || (solid(4, 2) && v.sub_y < 40)) {
            v.sub_x = 32;
            std::mem::swap(&mut s.left, &mut s.right);
        }
        if v.sub_x > 32 && (solid(6, 3) || (solid(6, 2) && v.sub_y < 40)) {
            v.sub_x = 32;
            std::mem::swap(&mut s.left, &mut s.right);
        }
        if v.sub_y < 48
            && (solid(5, 2) || (solid(6, 2) && v.sub_x > 32) || (solid(4, 2) && v.sub_x < 32))
        {
            v.sub_y = 48;
            std::mem::swap(&mut s.up, &mut s.down);
        }
        if v.sub_y > 48
            && (solid(5, 4) || (solid(4, 4) && v.sub_x < 32) || (solid(6, 4) && v.sub_x > 32))
        {
            v.sub_y = 48;
            std::mem::swap(&mut s.up, &mut s.down);
        }
    }

    fn carry(&mut self) -> bool {
        let v = &mut self.view;
        let mut edge = false;
        for (tile, sub, (lo, hi)) in [
            (&mut v.tile_y, &mut v.sub_y, VIEW_Y),
            (&mut v.tile_x, &mut v.sub_x, VIEW_X),
        ] {
            if *sub < 16 {
                if *tile > lo {
                    *tile -= 1;
                } else {
                    edge = true;
                }
                *sub += 32;
            }
            if *sub > 48 {
                if *tile < hi {
                    *tile += 1;
                } else {
                    edge = true;
                }
                *sub -= 32;
            }
        }
        edge
    }

    /// Thrust (§4.5) then steering (§4.6), tick-order step 30.
    pub(crate) fn thrust_and_steer(&mut self, classes: &ClassTables, k: &Dirs, over: bool) {
        if !over {
            self.thrust(classes, k);
        }
        self.steer(k, over);
    }

    fn thrust(&mut self, classes: &ClassTables, k: &Dirs) {
        let (max, accel) = (classes.max_speed(self.class), classes.accel(self.class));
        // Real48 Round(accel / 2.0): halves round up for these small positive values.
        let half = accel.div_ceil(2);
        let s = &mut self.speed;
        let brake = |v: &mut u8, full: bool| {
            if *v > SPEED_BIAS && !full {
                *v -= half;
            }
        };
        if k.u && s.up < max {
            s.up += accel;
            brake(&mut s.down, k.dfull);
            brake(&mut s.left, k.lfull);
            brake(&mut s.right, k.rfull);
        }
        if k.d && s.down < max {
            s.down += accel;
            brake(&mut s.up, k.ufull);
            brake(&mut s.left, k.lfull);
            brake(&mut s.right, k.rfull);
        }
        if k.l && s.left < max {
            brake(&mut s.up, k.ufull);
            brake(&mut s.down, k.dfull);
            brake(&mut s.right, k.rfull);
            s.left += accel;
        }
        if k.r && s.right < max {
            brake(&mut s.up, k.ufull);
            brake(&mut s.down, k.dfull);
            brake(&mut s.left, k.lfull);
            s.right += accel;
        }
    }

    /// Sets the turn direction when the current heading code `h` is in one of the sets.
    fn turn_if(&mut self, h: u8, cw: [u8; 3], ccw: [u8; 3]) {
        if cw.contains(&h) {
            self.turn = Turn::Clockwise;
        } else if ccw.contains(&h) {
            self.turn = Turn::CounterClockwise;
        }
    }

    fn aim(&mut self, h: u8, keep_if: u8, target: Heading) {
        if h != keep_if {
            self.target = target;
        }
    }

    fn steer(&mut self, k: &Dirs, over: bool) {
        let h = self.heading.code();
        let mut d_block = true;
        if k.u && !over {
            d_block = false;
            if k.r && !k.lfull {
                self.turn_if(h, [7, 8, 4], [6, 2, 3]);
                self.aim(h, 9, Heading::NE);
            } else if !k.l || k.rfull {
                if h != 8 {
                    self.turn_if(h, [7, 4, 1], [9, 6, 3]);
                    self.aim(h, 2, Heading::N);
                }
                d_block = true;
            } else {
                self.turn_if(h, [2, 4, 1], [9, 6, 8]);
                self.aim(h, 7, Heading::NW);
            }
        }
        let mut side_block = false;
        if d_block {
            if k.d && !over {
                if k.r && !k.lfull {
                    self.turn_if(h, [9, 8, 6], [1, 2, 4]);
                    self.aim(h, 3, Heading::SE);
                } else if k.l && !k.rfull {
                    self.turn_if(h, [2, 3, 6], [8, 7, 4]);
                    self.aim(h, 1, Heading::SW);
                } else if k.lfull || k.rfull || h == 2 {
                    side_block = true;
                } else {
                    self.turn_if(h, [9, 6, 3], [7, 4, 1]);
                    self.aim(h, 8, Heading::S);
                }
            } else {
                side_block = true;
            }
        }
        if side_block {
            if k.l && !over && h != 4 {
                self.turn_if(h, [3, 2, 1], [9, 8, 7]);
                self.aim(h, 6, Heading::W);
            } else if k.r && !over && h != 6 {
                self.turn_if(h, [9, 8, 7], [3, 2, 1]);
                self.aim(h, 4, Heading::E);
            }
        }
    }
}

impl HashState for View {
    fn hash_state(&self, h: &mut StateHasher) {
        for v in [self.tile_x, self.tile_y, self.sub_x, self.sub_y] {
            v.hash_state(h);
        }
    }
}

impl HashState for Player {
    fn hash_state(&self, h: &mut StateHasher) {
        self.class.hash_state(h);
        self.view.hash_state(h);
        let s = self.speed;
        h.bytes(&[s.up, s.down, s.left, s.right]);
        self.heading.hash_state(h);
        self.target.hash_state(h);
        (self.turn as u8).hash_state(h);
        self.armour.hash_state(h);
        self.shields.hash_state(h);
        self.burst_frame.hash_state(h);
        self.burst_heading.hash_state(h);
    }
}
