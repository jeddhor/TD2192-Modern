//! One flight (`Sim_FlyMission`): the per-tick order and the player's ship.
//!
//! Spec: `docs/spec/sim/tick-order.md` rev 1 (step numbers below are its §4 list),
//! `docs/spec/sim/flight-model.md` rev 2 and `docs/spec/sim/weapons.md` rev 1. Fidelity: T1.
//!
//! Implemented: tick counter, turn step, afterburner burst, projectiles and hits, end-sequence
//! counting, explosions, the white flash, the per-tick page toggle, modal interruptions (comm
//! viewer, pause, flight menu, ship switch), shields and destruction, player movement with map
//! collision, thrust and steering, player weapons (volleys, missiles, Tab, infra-red toggle),
//! the missile cooldown, the end-of-flight test, mission ships (step 10) and their fire
//! requests and volleys (`mission-ships.md` rev 2), and per-flight setup (`mission-setup.md`
//! rev 1). Not yet specified, so not implemented: mission logic (steps 8–9, M8-05). Cheat
//! chords (34) are not ported (decision D5/D9).
//!
//! Session state: the original keeps the RNG, the class tables (patched by some flights,
//! OB-018) and the ship slots and records across the flights of one program run. [`Session`]
//! carries them from one [`Flight`] to the next.
//!
//! Modal screens run *inside* a tick in the original: the tick stops at that point and
//! continues when the screen closes. [`Flight::tick`] returns [`TickResult::Modal`] there;
//! the caller shows the screen and then calls [`Flight::resume`], which clears the key table
//! (the original's modal screens do) and runs the rest of the tick.

pub mod explosions;
pub mod heading;
pub mod player;
pub mod setup;
pub mod ships;
pub mod shots;
pub mod tables;

use td_formats::map::TileMap;
use td_formats::mission::Mission;

use crate::hash::{HashState, StateHasher};
use crate::input::{FlightKey, Joystick, KeyTable, TickControls};
use crate::rng::BpRandom;
use explosions::Explosions;
use heading::Heading;
use player::{Dirs, Event, Player, View};
use ships::{AiContext, Ships};
use shots::{Combat, Muzzles, Shots};
use tables::ClassTables;

/// The static world of a flight: the map, its tile attributes and the muzzle table.
#[derive(Debug, Clone)]
pub struct World {
    pub map: TileMap,
    pub tile_attr: [u8; 256],
    pub muzzles: Muzzles,
}

impl World {
    /// True when the tile at (x, y) is not open (`tile_attr != 2`). Outside the map is solid.
    pub fn solid(&self, x: i16, y: i16) -> bool {
        let (Ok(x), Ok(y)) = (usize::try_from(x), usize::try_from(y)) else {
            return true;
        };
        self.map
            .get(x, y)
            .is_none_or(|t| self.tile_attr[usize::from(t)] != tables::TILE_OPEN)
    }
}

/// A modal screen that interrupts the tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modal {
    /// F6 comm viewer (step 15).
    CommViewer,
    /// P: pause text, then wait for any key (step 23).
    Pause,
    /// Esc: the flight menu, which may abort the flight (step 25).
    FlightMenu,
    /// F5: the ship-switch screen (step 29).
    ShipSwitch,
}

/// How a call to [`Flight::tick`] or [`Flight::resume`] ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TickResult {
    /// The tick completed; the flight goes on.
    Done,
    /// The tick stopped for a modal screen; call [`Flight::resume`] when it closes.
    Modal(Modal),
    /// The tick completed and the flight loop ends (step 35).
    Finished,
}

/// What the tick drew (the original draws the world before it moves the player, so the frame
/// shows the player where the previous tick left it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrawnFrame {
    /// View used for the tile window (step 4).
    pub view: View,
    /// Player heading when the ship was drawn (step 7 or 16).
    pub heading: Heading,
    /// Whether the player ship was drawn.
    pub player_visible: bool,
    /// Afterburner flame sprite (9 + burst frame), if drawn.
    pub flame: Option<u8>,
    /// White flash frame shown (4 white .. 1 restore), 0 = none.
    pub white_flash: u8,
}

/// The moving objects as a tick drew them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DrawnObjects {
    pub shots: Shots,
    pub ships: Ships,
    pub explosions: Explosions,
}

/// Default view at flight start (flight-model §3).
pub const START_VIEW: View = View {
    tile_x: 102,
    tile_y: 104,
    sub_x: 32,
    sub_y: 16,
};

/// What the original keeps across the flights of one program run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    /// The program's `Random` generator.
    pub rng: BpRandom,
    /// Class tables, with the patches of the flights flown so far.
    pub classes: ClassTables,
    /// Ship slots and records (flight 21 loads no mission file and flies these).
    pub ships: Ships,
    /// The player tile the AI last saw (DS:9766/9767; 0 at program start). The AI reads it
    /// at setup, before the first tick writes it.
    pub ai_player: (u8, u8),
}

impl Session {
    /// Program start: the AI unit's initialisation resets the ship records and draws 20
    /// `Random(2)` before anything else uses the generator (mission-ships §4.1 step 1).
    pub fn new() -> Self {
        let mut rng = BpRandom::new();
        let mut ships = Ships::default();
        ships.reset_records(&mut rng);
        Self {
            rng,
            classes: ClassTables::default(),
            ships,
            ai_player: (0, 0),
        }
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

/// The state of one flight.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flight {
    /// Flight number 1..=21 (`Sim_FlyMission` argument).
    pub number: u8,
    /// Ticks since flight start.
    pub ticks: u32,
    /// Back page 0/1, toggled once per tick.
    pub draw_page: u8,
    pub mission_over: bool,
    /// Ticks since `mission_over` (player hidden from 15; the loop ends at 35).
    pub end_ticks: u8,
    pub aborted: bool,
    pub lost: bool,
    pub succeeded: bool,
    /// Draw the player after the mission ships; also disables player map collision.
    pub late_draw: bool,
    /// F6 was pressed; the comm viewer opens on the next tick with back page 0.
    pub comm_request: bool,
    /// The player touched the map limit (drives the jurisdiction warning).
    pub edge_hit: bool,
    /// Suppresses repeated edge alerts (set by the jurisdiction warning, not yet specified).
    pub edge_latch: bool,
    /// Read the joystick each tick (controller = joystick).
    pub joystick_enabled: bool,
    pub joystick: Joystick,
    pub keys: KeyTable,
    pub player: Player,
    pub ships: Ships,
    pub shots: Shots,
    pub explosions: Explosions,
    /// The program's `Random` generator (its state carries across flights).
    pub rng: BpRandom,
    /// Class tables (session state, see [`Session`]).
    pub classes: ClassTables,
    /// The player tile the AI last saw (session state, see [`Session`]).
    pub ai_player: (u8, u8),
    /// `g_MissionKills`.
    pub kills: i16,
    /// Full-palette white flash countdown (`g_FlashFrames`).
    pub white_flash: u8,
    /// Damage multiplier for hits on the player (DS:13DE).
    pub difficulty: u8,
    /// Selected weapon: 1..=3 fire the class volley, 4..=6 a missile (`g_Weapon`).
    pub weapon: u8,
    /// Missiles left (`g_Missiles`).
    pub missiles: u8,
    /// Ticks until the next missile launch (`g_MissileCooldown`).
    pub missile_cooldown: u8,
    /// 0 normal, 1 infra-red, 2 red-only (`g_PaletteMode`).
    pub palette_mode: u8,
    /// What the last tick drew.
    pub frame: DrawnFrame,
    /// Shots, ships and explosions as the last tick drew them (steps 6, 10 and 13); not part
    /// of the state hash.
    pub drawn: DrawnObjects,
    /// Step to continue at after a modal screen.
    resume_at: u8,
}

impl Flight {
    /// The state after setup for flight `number` (1..=21), with the default start view.
    pub fn new(number: u8) -> Self {
        let view = match number {
            13 | 14 => View {
                tile_x: 177,
                tile_y: 153,
                ..START_VIEW
            },
            _ => START_VIEW,
        };
        Self::with_player(number, Player::new(tables::player_class(number), view))
    }

    /// The state after setup for flight `number` with the mission file's ships, as the first
    /// flight of a program run.
    pub fn with_mission(number: u8, mission: &Mission, world: &World) -> Self {
        Self::start(number, Some(mission), world, Session::new())
    }

    /// Flight setup (`Sim_FlyMission`, mission-ships §4.1, mission-setup §3): reset the ship
    /// records (20 `Random(2)`), load the mission file (`None` for flight 21), apply
    /// `Msn_Setup`, set the start view, and choose every slot's first waypoint.
    pub fn start(number: u8, mission: Option<&Mission>, world: &World, session: Session) -> Self {
        let Session {
            mut rng,
            mut classes,
            mut ships,
            ai_player,
        } = session;
        ships.reset_records(&mut rng);
        if let Some(m) = mission {
            ships.load_mission(m, &classes);
        }
        let flags = setup::msn_setup(number, &mut ships, &mut classes);
        let mut view = START_VIEW;
        if flags.late_draw {
            view.tile_y = 100;
        }
        if matches!(number, 13 | 14) {
            (view.tile_x, view.tile_y) = (177, 153);
        }
        let mut player = Player::new(tables::player_class(number), view);
        player.armour = classes.armour(player.class) as i8;
        player.shields = classes.shields(player.class) as i8;
        let mut f = Self::with_player(number, player);
        f.late_draw = flags.late_draw;
        f.palette_mode = flags.palette_mode;
        // Default start: draw page 1 (mission-setup §3.1).
        f.draw_page = 1;
        let mut ctx = AiContext {
            world,
            rng: &mut rng,
            player: ai_player,
        };
        ships.choose_all(&mut ctx);
        f.rng = rng;
        f.classes = classes;
        f.ships = ships;
        f.ai_player = ai_player;
        f
    }

    /// The state to carry into the next flight of the same program run.
    pub fn session(&self) -> Session {
        Session {
            rng: self.rng,
            classes: self.classes.clone(),
            ships: self.ships.clone(),
            ai_player: self.ai_player,
        }
    }

    /// The player's tile as the AI sees it (DS:9766/9767: view tile + (5, 3), low bytes).
    pub fn player_tile(&self) -> (u8, u8) {
        let v = self.player.view;
        ((v.tile_x + 5) as u8, (v.tile_y + 3) as u8)
    }

    /// A flight with an explicit player (dev arena, tests). Weapons follow the player's class
    /// (`weapons.md` §3.2; classes 0 and 1 keep weapon 1).
    pub fn with_player(number: u8, player: Player) -> Self {
        let (weapon, _, missiles) = tables::player_weapons(player.class).unwrap_or((1, 1, 0));
        Self {
            number,
            ticks: 0,
            draw_page: 0,
            mission_over: false,
            end_ticks: 0,
            aborted: false,
            lost: false,
            succeeded: false,
            late_draw: false,
            comm_request: false,
            edge_hit: false,
            edge_latch: false,
            joystick_enabled: false,
            joystick: Joystick::default(),
            keys: KeyTable::default(),
            frame: DrawnFrame {
                view: player.view,
                heading: player.heading,
                player_visible: true,
                flame: None,
                white_flash: 0,
            },
            drawn: DrawnObjects::default(),
            ships: Ships::default(),
            shots: Shots::default(),
            explosions: Explosions::default(),
            rng: BpRandom::new(),
            classes: ClassTables::default(),
            ai_player: (0, 0),
            kills: 0,
            white_flash: 0,
            difficulty: 1,
            weapon,
            missiles,
            missile_cooldown: 0,
            palette_mode: 0,
            player,
            resume_at: 1,
        }
    }

    /// Runs one tick with this tick's input.
    pub fn tick(&mut self, world: &World, c: &TickControls, events: &mut Vec<Event>) -> TickResult {
        self.keys.update(c);
        self.run(world, c, events, 1)
    }

    /// Continues the tick interrupted by a modal screen. `c` is the input gathered while the
    /// screen was open; the key table is cleared first, as the original's screens do.
    pub fn resume(
        &mut self,
        world: &World,
        c: &TickControls,
        events: &mut Vec<Event>,
    ) -> TickResult {
        self.keys.clear_all();
        self.keys.update(c);
        let from = std::mem::replace(&mut self.resume_at, 1);
        self.run(world, c, events, from)
    }

    fn end_mission(&mut self) {
        self.mission_over = true;
        self.end_ticks = 1;
    }

    fn interrupt(&mut self, resume_at: u8, modal: Modal) -> TickResult {
        self.resume_at = resume_at;
        TickResult::Modal(modal)
    }

    fn run(
        &mut self,
        world: &World,
        c: &TickControls,
        events: &mut Vec<Event>,
        from: u8,
    ) -> TickResult {
        if from <= 1 {
            // 1. Tick counter; flight 21 ends itself after 25 ticks.
            self.ticks = self.ticks.wrapping_add(1);
            if self.number == 21 && self.ticks == 25 {
                self.end_mission();
            }
            // 3. Turn step.
            self.player.turn_step();
            // 4. Tile window drawn from the current view.
            self.frame.view = self.player.view;
            // 5. Afterburner burst.
            self.frame.flame = self.player.afterburner(
                &mut self.keys,
                self.joystick.button1,
                self.mission_over,
                events,
            );
            // 6. Projectiles and hits.
            self.shots.update(&mut Combat {
                world,
                ships: &mut self.ships,
                player: &mut self.player,
                explosions: &mut self.explosions,
                rng: &mut self.rng,
                events,
                kills: &mut self.kills,
                white_flash: &mut self.white_flash,
                late_draw: self.late_draw,
                flight: self.number,
                difficulty: self.difficulty,
            });
            self.drawn.shots.clone_from(&self.shots);
            // 7. Player ship (unless drawn late).
            if !self.late_draw {
                self.frame.heading = self.player.heading;
                self.frame.player_visible = self.end_ticks < 15;
            }
            // 8–9. Mission logic (`Msn_UpdateA`/`Msn_UpdateB`, M8-05, RE-6 mission-logic.md):
            // HOOK. It orders attacks (`Ships::order_attack`) and writes slot classes here,
            // before the ships move; flight 1 removes slots 9–11 at tick 1.
            // 10. Mission ships: AI, movement, removal (mission-ships §4.2).
            self.ai_player = self.player_tile();
            let player = self.ai_player;
            self.ships.update(
                self.number,
                &self.classes,
                &mut AiContext {
                    world,
                    rng: &mut self.rng,
                    player,
                },
            );
            // Flight 19 with mission phase > 1 caps the player's shields and moves slot 19
            // (mission-ships §4.2.4): needs the mission phase (M8-05).
            self.drawn.ships.clone_from(&self.ships);
            // 11. End sequence (provisional: counts the end ticks; its other effects are not
            // specified yet).
            if self.mission_over {
                self.end_ticks = self.end_ticks.saturating_add(1);
            }
            // 12 is presentation. 13. Explosions advance after being drawn.
            self.drawn.explosions.clone_from(&self.explosions);
            self.explosions.step();
            // 14. Palette effects: the white flash counts down.
            self.frame.white_flash = self.white_flash;
            self.white_flash = self.white_flash.saturating_sub(1);
            // 15. Comm viewer on an even back page.
            if self.comm_request && self.draw_page == 0 {
                self.comm_request = false;
                return self.interrupt(16, Modal::CommViewer);
            }
        }
        if from <= 16 {
            // 16. Player ship drawn late.
            if self.late_draw {
                self.frame.heading = self.player.heading;
                self.frame.player_visible = self.end_ticks < 15;
            }
            // 17. Joystick.
            if self.joystick_enabled {
                self.joystick = c.joystick();
            }
            // 18–22: HUD, tick wait, flip (presentation and host timing). 23. Pause.
            if self.keys.get(FlightKey::P) {
                return self.interrupt(24, Modal::Pause);
            }
        }
        if from <= 24 {
            // 24. Page toggle.
            self.draw_page ^= 1;
            // 25. Flight menu.
            if self.keys.get(FlightKey::Esc) && !self.mission_over {
                return self.interrupt(26, Modal::FlightMenu);
            }
        }
        if from <= 26 {
            // 26 is audio. 27. Shields and destruction.
            if self.player.shields_step(&self.classes, self.mission_over) {
                self.end_mission();
            }
            // 28. Movement, map collision (skipped while drawing late), carry and limits.
            if self.player.move_step(world, !self.late_draw) && !self.edge_latch {
                self.edge_hit = true;
            }
            // 29. Comm request and ship switch.
            if self.keys.get(FlightKey::F6) {
                self.comm_request = true;
            }
            if self.keys.get(FlightKey::F5) && self.number != 14 {
                return self.interrupt(30, Modal::ShipSwitch);
            }
        }
        // 30. Thrust and steering.
        let dirs = Dirs::read(&self.keys, self.joystick);
        self.player
            .thrust_and_steer(&self.classes, &dirs, self.mission_over);
        // 31. Mission-ship volleys on back page 1; 32. fire requests cleared every tick
        // (mission-ships §4.4.2).
        if self.draw_page == 1 {
            self.shots.ship_volleys(&self.ships, &world.muzzles, events);
        }
        self.ships.clear_fire_requests();
        // 33. Player weapons and infra-red.
        self.player_specials(world, events);
        // 34. Missile cooldown (the cheat chords checked here are not ported).
        self.missile_cooldown = self.missile_cooldown.saturating_sub(1);
        // 35. End of flight.
        if self.aborted || self.end_ticks == 35 || self.lost || self.succeeded {
            TickResult::Finished
        } else {
            TickResult::Done
        }
    }
}

impl Flight {
    /// Tab, infra-red toggle and fire (`weapons.md` §4.1, tick-order step 33).
    fn player_specials(&mut self, world: &World, events: &mut Vec<Event>) {
        use FlightKey::{Alt, RightAlt, Space, Tab};
        let class = self.player.class;
        if self.keys.get(Tab) || (self.weapon > 3 && self.missiles == 0) {
            self.keys.clear(Tab);
            if let Some((primary, missile, _)) = tables::player_weapons(class) {
                if self.weapon > 3 {
                    self.weapon = primary;
                } else if self.missiles > 0 {
                    self.weapon = missile;
                }
            }
        }
        // M replays the HUD ticker message (presentation, M6-07).
        if (self.keys.get(Alt) || self.keys.get(RightAlt))
            && !self.mission_over
            && matches!(self.palette_mode, 1 | 2)
        {
            self.palette_mode = 3 - self.palette_mode;
            self.keys.clear(Alt);
            self.keys.clear(RightAlt);
        }
        if self.keys.get(Space) || self.joystick.button2 {
            if self.weapon > 3
                && self.missiles > 0
                && self.missile_cooldown == 0
                && !self.mission_over
            {
                if self
                    .shots
                    .player_missile(&self.player, self.weapon, &self.ships, events)
                {
                    self.missiles -= 1;
                    self.missile_cooldown = 4;
                }
                self.keys.clear(Space);
            }
            if self.weapon <= 3 && !self.mission_over && self.draw_page == 1 {
                self.shots
                    .player_volley(&self.player, &world.muzzles, self.late_draw, events);
            }
        }
    }
}

impl HashState for Flight {
    fn hash_state(&self, h: &mut StateHasher) {
        self.number.hash_state(h);
        self.ticks.hash_state(h);
        self.draw_page.hash_state(h);
        for b in [
            self.mission_over,
            self.aborted,
            self.lost,
            self.succeeded,
            self.late_draw,
            self.comm_request,
            self.edge_hit,
            self.edge_latch,
            self.joystick_enabled,
        ] {
            b.hash_state(h);
        }
        self.end_ticks.hash_state(h);
        let j = self.joystick;
        h.bytes(&[
            j.x as u8,
            j.y as u8,
            u8::from(j.button1),
            u8::from(j.button2),
        ]);
        self.keys.hash_state(h);
        self.player.hash_state(h);
        self.ships.hash_state(h);
        self.shots.hash_state(h);
        self.explosions.hash_state(h);
        self.rng.hash_state(h);
        for t in [
            &self.classes.max_speed,
            &self.classes.armour,
            &self.classes.shields,
            &self.classes.accel,
        ] {
            h.bytes(t);
        }
        h.bytes(&[self.ai_player.0, self.ai_player.1]);
        self.kills.hash_state(h);
        h.bytes(&[
            self.white_flash,
            self.difficulty,
            self.weapon,
            self.missiles,
            self.missile_cooldown,
            self.palette_mode,
        ]);
        self.resume_at.hash_state(h);
    }
}

#[cfg(test)]
mod tests;
