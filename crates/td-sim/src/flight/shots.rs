//! Projectiles: one pool of 51 slots shared by the player and the mission ships.
//!
//! Spec: `docs/spec/sim/weapons.md` rev 1 (§4.2 spawning, §4.3 per-tick update; static,
//! LIKELY). Fidelity: T1. Shots move one whole tile per tick along a principal heading; hit
//! tests use the positions before the move. "Freeing" a slot writes FFh to its tile X, as the
//! original does; later tests in the same pass then fail on distance by themselves.

use super::World;
use super::explosions::Explosions;
use super::player::{Event, Player};
use super::ships::{SHIP_SLOTS, Ships};
use super::tables::{player_damage, ship_damage, shot_entry};
use crate::hash::{HashState, StateHasher};
use crate::rng::BpRandom;

/// Number of projectile slots.
pub const SHOT_SLOTS: usize = 51;
/// Tile X of a free slot.
pub const FREE: u8 = 0xFF;
/// Shots older than this die.
pub const MAX_LIFE: u8 = 25;
/// Kind of the homing missile.
pub const HOMING: u8 = 6;

/// One projectile slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shot {
    /// FFh when the slot is free.
    pub tile_x: u8,
    pub tile_y: u8,
    pub off_x: i16,
    pub off_y: i16,
    /// Keypad code; above 9 the shot is deleted on its first move.
    pub heading: u8,
    /// 0..=3 cannon classes, 4 and 5 missiles, 6 homing missile.
    pub kind: u8,
    /// Age for kinds 0..=5; the target slot (1-based) for kind 6.
    pub life: u8,
    /// 0 = player, 1..=20 = mission slot.
    pub owner: u8,
}

impl Shot {
    const UNUSED: Shot = Shot {
        tile_x: FREE,
        tile_y: 0,
        off_x: 0,
        off_y: 0,
        heading: 0,
        kind: 0,
        life: 0,
        owner: 0,
    };

    pub fn alive(&self) -> bool {
        self.tile_x != FREE
    }

    /// World pixel (`tile × 32 + offset`).
    pub fn world(&self) -> (i32, i32) {
        (
            i32::from(self.tile_x) * 32 + i32::from(self.off_x),
            i32::from(self.tile_y) * 32 + i32::from(self.off_y),
        )
    }
}

/// The muzzle-offset table (DS:035C, read from the executable): signed byte pairs, muzzle `m`
/// of heading digit `h` at `34·h + 2·m`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Muzzles(pub Vec<u8>);

impl Muzzles {
    /// Offset (dx, dy) of muzzle `m` (1..=17) for `heading`. Two-digit headings index far
    /// outside the table in the original (U-040); here they give (0, 0) (port policy; such a
    /// shot is deleted on its first move anyway).
    pub fn get(&self, heading: u8, m: usize) -> (i16, i16) {
        let i = 34 * usize::from(heading) + 2 * m;
        match (heading <= 9, self.0.get(i), self.0.get(i + 1)) {
            (true, Some(&dx), Some(&dy)) => (i16::from(dx as i8), i16::from(dy as i8)),
            _ => (0, 0),
        }
    }
}

/// Everything a shot update touches besides the pool itself.
#[derive(Debug)]
pub struct Combat<'a> {
    pub world: &'a World,
    pub ships: &'a mut Ships,
    pub player: &'a mut Player,
    pub explosions: &'a mut Explosions,
    pub rng: &'a mut BpRandom,
    pub events: &'a mut Vec<Event>,
    /// `g_MissionKills`.
    pub kills: &'a mut i16,
    /// Full-palette white flash countdown (`g_FlashFrames`).
    pub white_flash: &'a mut u8,
    /// `g_PlayerDrawLate` (also disables the player's shots' map test and enemy hits on the
    /// player).
    pub late_draw: bool,
    pub flight: u8,
    /// Damage multiplier for hits on the player (DS:13DE, 1 by default).
    pub difficulty: u8,
}

/// The projectile pool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shots(pub [Shot; SHOT_SLOTS]);

impl Default for Shots {
    fn default() -> Self {
        Self([Shot::UNUSED; SHOT_SLOTS])
    }
}

fn near(a: (i32, i32), b: (i32, i32), r: i32) -> bool {
    (a.0 - b.0).abs() < r && (a.1 - b.1).abs() < r
}

/// Applies `d` damage: shields absorb first, the rest goes to armour.
fn absorb(shields: &mut i16, armour: &mut i16, d: i16) {
    if d < *shields {
        *shields -= d;
    } else {
        *armour = *armour - d + *shields;
        *shields = 0;
    }
}

impl Shots {
    fn highest_free(&self, range: std::ops::RangeInclusive<usize>) -> Option<usize> {
        range.rev().find(|&i| !self.0[i].alive())
    }

    /// Player volley (§4.2): one shot per muzzle of the class's shot table.
    pub fn player_volley(
        &mut self,
        p: &Player,
        muzzles: &Muzzles,
        late_draw: bool,
        events: &mut Vec<Event>,
    ) {
        let v = p.view;
        let heading = p.heading.code();
        for m in 1..=17 {
            let entry = shot_entry(p.class, m);
            if entry == 0 {
                continue;
            }
            let Some(slot) = self.highest_free(8..=48) else {
                continue;
            };
            let (dx, dy) = muzzles.get(heading, m);
            self.0[slot] = Shot {
                tile_x: (v.tile_x + 4) as u8,
                tile_y: (v.tile_y + 3) as u8,
                // weapons.md §4.2 says `4·Round(subX/4) + dx`; the original subtracts 16 more
                // (C-0011: dx 7 → 23 and 22 → 38 at subX 32).
                off_x: 4 * (v.sub_x + 2).div_euclid(4) - 16 + dx,
                off_y: v.sub_y - 28 + dy,
                heading,
                kind: entry - 1,
                life: 1,
                owner: 0,
            };
            events.push(Event::Sound {
                kind: if late_draw { 2 } else { p.class },
                source: None,
            });
        }
    }

    /// Mission-ship volleys (mission-ships §4.4.2, tick-order step 30; the caller runs it when
    /// the back page is 1): one volley per fire request, along the ship's current heading.
    pub fn ship_volleys(&mut self, ships: &Ships, muzzles: &Muzzles, events: &mut Vec<Event>) {
        for (i, s) in ships.0.iter().enumerate() {
            for e in 2..=8 {
                if !s.fire[e - 1] {
                    continue;
                }
                for m in 1..=17 {
                    let Some(slot) = self.highest_free(0..=40) else {
                        continue;
                    };
                    let entry = shot_entry(s.class, m);
                    if entry == 0 {
                        continue;
                    }
                    // Two-digit headings read past the muzzle table (OB-017): (0, 0) here
                    // (U-160 proposal); such a shot is deleted by its first move.
                    let (dx, dy) = muzzles.get(s.heading, m);
                    self.0[slot] = Shot {
                        tile_x: s.tile_x,
                        tile_y: s.tile_y,
                        off_x: dx + s.sub_x - 16,
                        off_y: dy + s.sub_y - 16,
                        heading: s.heading,
                        kind: entry - 1,
                        life: 1,
                        owner: i as u8 + 1,
                    };
                    events.push(Event::Sound {
                        kind: s.class,
                        source: Some((s.tile_x, s.tile_y)),
                    });
                }
            }
        }
    }

    /// Player missile (§4.2). Returns false when no slot was free (the original then writes
    /// outside its arrays; the port skips the launch).
    pub fn player_missile(
        &mut self,
        p: &Player,
        weapon: u8,
        ships: &Ships,
        events: &mut Vec<Event>,
    ) -> bool {
        let Some(slot) = (40..=50).find(|&i| !self.0[i].alive()) else {
            return false;
        };
        let v = p.view;
        let tile = ((v.tile_x + 4) as u8, (v.tile_y + 3) as u8);
        let mut shot = Shot {
            tile_x: tile.0,
            tile_y: tile.1,
            off_x: v.sub_x - 2,
            off_y: v.sub_y - 12,
            heading: p.heading.code(),
            kind: weapon,
            life: 0,
            owner: 0,
        };
        if weapon == HOMING {
            // Original bug, preserved: the launch writes life 200 into slot 40 (ASSUMPTION:
            // before the new shot is stored, so a missile in slot 40 keeps its target).
            self.0[40].life = 200;
            let dist = |s: &super::ships::Ship| {
                let d = |a: u8, b: u8| (i32::from(a) - i32::from(b)).pow(2);
                d(s.tile_x, tile.0) + d(s.tile_y, tile.1)
            };
            let target = ships
                .0
                .iter()
                .enumerate()
                .filter(|(_, s)| s.alive())
                .min_by_key(|&(i, s)| (dist(s), i));
            match target {
                Some((i, _)) => shot.life = (i + 1) as u8,
                None => shot.kind = 0,
            }
        }
        self.0[slot] = shot;
        events.push(Event::Sound {
            kind: 10,
            source: None,
        });
        true
    }

    /// The per-tick update of every shot (§4.3, tick-order step 6).
    pub fn update(&mut self, c: &mut Combat<'_>) {
        for i in 0..SHOT_SLOTS {
            if self.0[i].alive() {
                self.update_one(i, c);
            }
        }
    }

    fn free(&mut self, i: usize) {
        self.0[i].tile_x = FREE;
    }

    fn update_one(&mut self, i: usize, c: &mut Combat<'_>) {
        // 1. Map bounds.
        let s = self.0[i];
        if !(2..=199).contains(&s.tile_x) || !(2..=199).contains(&s.tile_y) {
            self.free(i);
        }
        // 2. Age and map collision.
        if self.0[i].kind != HOMING {
            self.0[i].life = self.0[i].life.wrapping_add(1);
            if !(c.late_draw && self.0[i].owner == 0) {
                self.map_test(i, c.world);
            }
        }
        // 3. Homing missile: destroys the first ship within 40 px.
        if self.0[i].kind == HOMING {
            for s in 0..SHIP_SLOTS {
                let shot = self.0[i];
                let ship = c.ships.0[s];
                if ship.alive()
                    && s + 1 != usize::from(shot.owner)
                    && near(ship.world(), shot.world(), 40)
                {
                    self.free(i);
                    *c.kills += 1;
                    *c.white_flash = 4;
                    let ship = &mut c.ships.0[s];
                    ship.armour = 0;
                    ship.shields = 0;
                    five_explosions(c.explosions, (ship.tile_x, ship.tile_y));
                }
            }
        }
        // 4. Hits on mission ships.
        if !(c.flight == 20 && self.0[i].owner == 1) {
            for s in 0..SHIP_SLOTS {
                let shot = self.0[i];
                if c.ships.0[s].alive()
                    && s + 1 != usize::from(shot.owner)
                    && near(c.ships.0[s].world(), shot.world(), 20)
                {
                    self.hit_ship(i, s, c);
                }
            }
        }
        // 5. Hits on the player.
        let shot = self.0[i];
        if shot.owner != 0 && !c.late_draw && near(player_ref(c.player), shot.world(), 20) {
            self.hit_player(i, c);
        }
        // 6. Homing missiles steer toward their target.
        if self.0[i].kind == HOMING {
            self.steer(i, c.ships);
        }
        // 7. Age limit.
        if self.0[i].life > MAX_LIFE {
            self.free(i);
        }
        // 8. Move one tile.
        if self.0[i].alive() {
            let shot = &mut self.0[i];
            let (dx, dy): (i16, i16) = match shot.heading {
                8 => (0, -1),
                2 => (0, 1),
                4 => (-1, 0),
                6 => (1, 0),
                9 => (1, -1),
                7 => (-1, -1),
                1 => (-1, 1),
                3 => (1, 1),
                _ => {
                    shot.tile_x = FREE;
                    return;
                }
            };
            shot.tile_x = (i16::from(shot.tile_x) + dx) as u8;
            shot.tile_y = (i16::from(shot.tile_y) + dy) as u8;
        }
    }

    fn map_test(&mut self, i: usize, world: &World) {
        let s = self.0[i];
        let (tx, ty) = (i16::from(s.tile_x), i16::from(s.tile_y));
        if matches!(s.heading, 8 | 2 | 9 | 1 | 3 | 7)
            && ((world.solid(tx, ty) && s.off_x < 17) || (world.solid(tx + 1, ty) && s.off_x > 16))
        {
            self.free(i);
        }
        let s = self.0[i];
        let tx = i16::from(s.tile_x);
        if matches!(s.heading, 4 | 6 | 9 | 1 | 3 | 7)
            && ((world.solid(tx + 1, ty - 1) && s.off_y < 17)
                || (world.solid(tx + 1, ty) && s.off_y > 16))
        {
            self.free(i);
        }
    }

    fn hit_ship(&mut self, i: usize, s: usize, c: &mut Combat<'_>) {
        let shot = self.0[i];
        self.free(i);
        let ship = &mut c.ships.0[s];
        let at = Some((ship.tile_x, ship.tile_y));
        c.events.push(Event::Sound {
            kind: 9,
            source: at,
        });
        absorb(&mut ship.shields, &mut ship.armour, ship_damage(shot.kind));
        if ship.armour < 1 {
            c.events.push(Event::Sound {
                kind: 100,
                source: at,
            });
            ship.armour = 0;
        }
        if shot.kind == HOMING {
            *c.white_flash = 4;
            ship.armour = 0;
            ship.shields = 0;
        }
        if ship.shields > 60 || ship.shields < 0 {
            ship.shields = 0;
        }
        if ship.armour == 0 {
            // Original bug, preserved: writes the *projectile* kind array at the ship's index.
            self.0[s + 1].kind = 1;
            if shot.owner == 0 {
                *c.kills += 1;
            }
            ship.class = super::ships::NO_SHIP;
            five_explosions(c.explosions, (ship.tile_x, ship.tile_y));
        } else if let Some(e) = c.explosions.idle_hit_slot() {
            // The Random calls sit inside the slot search (weapons rev 3): none without a slot.
            let rx = c.rng.random(16) as i16;
            let ry = c.rng.random(16) as i16;
            c.explosions.start_in(
                e,
                (ship.tile_x, ship.tile_y),
                (ship.sub_x + rx, ship.sub_y + ry),
            );
        }
    }

    fn hit_player(&mut self, i: usize, c: &mut Combat<'_>) {
        let shot = self.0[i];
        self.free(i);
        c.events.push(Event::Sound {
            kind: 9,
            source: None,
        });
        let p = &mut *c.player;
        if c.difficulty != 0 {
            let (mut shields, mut armour) = (i16::from(p.shields), i16::from(p.armour));
            absorb(
                &mut shields,
                &mut armour,
                player_damage(shot.kind, c.difficulty),
            );
            p.shields = shields as i8;
            p.armour = armour as i8;
        }
        if p.armour < 0 {
            p.armour = 0;
        }
        if p.shields > 60 || p.shields < 0 {
            p.shields = 0;
        }
        if let Some(e) = c.explosions.idle_hit_slot() {
            let rx = c.rng.random(8) as i16 - 8;
            let ry = c.rng.random(8) as i16 - 8;
            let v = p.view;
            c.explosions.start_in(
                e,
                ((v.tile_x + 4) as u8, (v.tile_y + 3) as u8),
                (v.sub_x + rx, v.sub_y + ry),
            );
        }
    }

    fn steer(&mut self, i: usize, ships: &Ships) {
        let shot = &mut self.0[i];
        let Some(target) = usize::from(shot.life)
            .checked_sub(1)
            .and_then(|t| ships.0.get(t))
        else {
            return;
        };
        let (dx, dy) = (
            i32::from(target.tile_x) - i32::from(shot.tile_x),
            i32::from(target.tile_y) - i32::from(shot.tile_y),
        );
        shot.heading = match (dx.signum(), dy.signum()) {
            (1, 1) => 3,
            (1, -1) => 9,
            (-1, 1) => 1,
            (-1, -1) => 7,
            (0, 1) => 2,
            (0, _) => 8,
            (1, 0) => 6,
            _ => 4,
        };
    }
}

/// The five explosions of a destroyed ship: tiles (X+1, Y+1), (X, Y), (X+1, Y−1), (X−1, Y+1),
/// (X−1, Y−1) in this order, each at offset (16, 16) and each in its own slot search
/// (`flight-screens.md` §4.7).
fn five_explosions(explosions: &mut Explosions, (x, y): (u8, u8)) {
    for (dx, dy) in [(1, 1), (0, 0), (1, -1), (-1, 1), (-1, -1)] {
        let tile = ((i16::from(x) + dx) as u8, (i16::from(y) + dy) as u8);
        explosions.start(tile, (16, 16));
    }
}

/// The player's hit reference point (`weapons.md` §2).
pub fn player_ref(p: &Player) -> (i32, i32) {
    let v = p.view;
    (
        (i32::from(v.tile_x) + 5) * 32 + i32::from(v.sub_x),
        (i32::from(v.tile_y) + 3) * 32 + i32::from(v.sub_y),
    )
}

impl HashState for Shots {
    fn hash_state(&self, h: &mut StateHasher) {
        for s in &self.0 {
            h.bytes(&[s.tile_x, s.tile_y, s.heading, s.kind, s.life, s.owner]);
            s.off_x.hash_state(h);
            s.off_y.hash_state(h);
        }
    }
}
