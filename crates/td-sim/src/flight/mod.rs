//! One flight (`Sim_FlyMission`): the per-tick order and the player's ship.
//!
//! Spec: `docs/spec/sim/tick-order.md` rev 1 (step numbers below are its §4 list),
//! `docs/spec/sim/flight-model.md` rev 2 and `docs/spec/sim/weapons.md` rev 1. Fidelity: T1.
//!
//! Implemented: tick counter, turn step, afterburner burst, projectiles and hits, end-sequence
//! counting, explosions, the white flash, the per-tick page toggle, modal interruptions (comm
//! viewer, pause, flight menu, tactical map), shields and destruction, player movement with map
//! collision, thrust and steering, player weapons (volleys, missiles, Tab, infra-red toggle),
//! the missile cooldown, the end-of-flight test, mission ships (step 10) and their fire
//! requests and volleys (`mission-ships.md` rev 2), and per-flight setup (`mission-setup.md`
//! rev 1), and mission logic (steps 8–9, 15, 18 and the game rules of step 19:
//! `mission-logic.md` rev 1, see [`logic`]). Cheat chords (34) are not ported (decision D5/D9).
//!
//! Episodes: one engine flies all three episodes. [`Flight::episode`] selects the setup tables,
//! the mission logic and the tick-order, weapon and AI deltas of `docs/re/episode-deltas.md`
//! rev 3 ([`episode`]); E2 setup and logic follow `docs/spec/sim/e2-mission-setup.md`,
//! `e2-mission-logic.md` and `e2-mission-ships.md` (rev 1 each), E3 setup, logic and the E3
//! player deltas `e3-mission-setup.md`, `e3-mission-logic.md` and `e3-mission-ships.md` (rev 1
//! each).
//!
//! Session state: the original keeps the RNG, the class tables (patched by some flights,
//! OB-018) and the ship slots and records across the flights of one program run. [`Session`]
//! carries them from one [`Flight`] to the next.
//!
//! Modal screens run *inside* a tick in the original: the tick stops at that point and
//! continues when the screen closes. [`Flight::tick`] returns [`TickResult::Modal`] there;
//! the caller shows the screen and then calls [`Flight::resume`], which clears the key table
//! (the original's modal screens do) and runs the rest of the tick.

pub mod episode;
pub mod explosions;
pub mod heading;
pub mod logic;
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
pub use episode::Episode;
use explosions::Explosions;
use heading::Heading;
use logic::{HudSnapshot, MissionState};
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
    /// F5: the tactical map (step 29; no game state changes, `flight-screens.md`).
    TacticalMap,
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
    /// What the instruments showed (step 19).
    pub hud: HudSnapshot,
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
    /// The program (`file0001.exe`, `td2.exe` or `td3.exe`).
    pub episode: Episode,
    /// The program's `Random` generator.
    pub rng: BpRandom,
    /// Class tables, with the patches of the flights flown so far.
    pub classes: ClassTables,
    /// Ship slots and records (flight 21 loads no mission file and flies these).
    pub ships: Ships,
    /// The player tile the AI last saw (DS:9766/9767; 0 at program start). The AI reads it
    /// at setup, before the first tick writes it.
    pub ai_player: (u8, u8),
    /// Mission-logic globals (setup re-initialises only part of them, [`MissionState`]).
    pub mission: MissionState,
    /// E2: `k_TileRadarColour` holds the map-set-2 colours. A set-2 flight copies them over
    /// the table and nothing restores it (OB-024), so it is session state (presentation).
    pub radar_set2: bool,
}

impl Session {
    /// Program start of `file0001.exe` (Episode 1).
    pub fn new() -> Self {
        Self::for_episode(Episode::E1)
    }

    /// Program start: the AI unit's initialisation resets the ship records and draws 20
    /// `Random(2)` before anything else uses the generator (mission-ships §4.1 step 1; the AI
    /// unit is identical in E2 and E3, episode-deltas §3.1).
    pub fn for_episode(episode: Episode) -> Self {
        let mut rng = BpRandom::new();
        let mut ships = Ships::default();
        ships.reset_records(&mut rng);
        Self {
            episode,
            rng,
            classes: ClassTables::default(),
            ships,
            ai_player: (0, 0),
            mission: MissionState::default(),
            radar_set2: false,
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
    /// The program flying this flight; selects the episode deltas ([`episode`]).
    pub episode: Episode,
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
    /// `g_EdgeHit`: the view was pushed back at a map limit while the ticker was idle; the
    /// instruments step posts the jurisdiction warning and clears it.
    pub edge_hit: bool,
    /// Read the joystick each tick. E1 and E2: the configured controller is the joystick; E3:
    /// whenever a stick was detected, whatever the configuration (e3-mission-ships §5.2 row 2,
    /// episode-deltas §4.1 step 17). The caller sets it.
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
    /// Mission phase, ticker, transmission state and the other mission-logic globals.
    pub mission: MissionState,
    /// E2 radar colours of map set 2 in effect (session state, see [`Session`]).
    pub radar_set2: bool,
    /// E3 flight 12: the cells of [`setup::e3::FLIGHT12_CELLS`] hold tile 26h instead of the
    /// map's hazard tiles (from setup until the end of tick 6480). The map buffer is reloaded
    /// at every flight start, so this is per-flight state; [`Flight::map_tile`] applies it.
    pub cells_off: bool,
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

    /// Flight setup (`Sim_FlyMission`, mission-ships §4.1, mission-setup §3; E2:
    /// e2-mission-setup §4.1): reset the ship records (20 `Random(2)`), load the mission file
    /// (`None` for E1 flight 21), apply `Msn_Setup`, set the player and the start view, and (E1
    /// only) choose every slot's first waypoint. The episode is the session's.
    pub fn start(number: u8, mission: Option<&Mission>, world: &World, session: Session) -> Self {
        let Session {
            episode,
            mut rng,
            mut classes,
            mut ships,
            ai_player,
            mission: mut logic,
            mut radar_set2,
        } = session;
        ships.reset_records(&mut rng);
        if let Some(m) = mission {
            ships.load_mission(m, &classes);
            // A class-FFh slot reads the bytes after the class tables: shields 34, armour
            // 2 × 34 = 68 in every episode (E1 DS:03D1 / DS:03C7, confirmed by VE-1 in C-0017,
            // U-143 and BUG-1; E2 DS:052F / DS:0525, e2-mission-setup §2; E3 the same DS:052F /
            // DS:0525, e3-mission-setup §2, confirmed in C-0036 and C-0041).
            for s in ships.0.iter_mut().filter(|s| !s.alive()) {
                s.shields = 34;
                s.armour = 68;
            }
        }
        if episode == Episode::E2 && episode::map_set(episode, number) == 2 {
            radar_set2 = true;
        }
        let flags = match episode {
            Episode::E1 => {
                let flags = setup::msn_setup(number, &mut ships, &mut classes);
                logic.setup(number);
                flags
            }
            Episode::E2 => {
                logic.setup_e2(number);
                setup::e2::msn_setup(number, &mut ships, &mut classes)
            }
            Episode::E3 => {
                logic.setup_e3(number);
                setup::e3::msn_setup(number, &mut ships, &mut classes)
            }
        };
        let view = episode::start_view(episode, number, flags.late_draw);
        let class = episode::player_class(episode, number);
        let mut player = Player::new(class, view);
        player.armour = episode::start_armour(episode, number, classes.armour(class));
        player.shields = classes.shields(class) as i8;
        let mut f = Self::with_player(number, player);
        f.episode = episode;
        f.missiles = episode::start_missiles(episode, number, class, f.missiles);
        // E3 flight 12: 27 hazard cells of `td4.map` are tile 26h until tick 6480
        // (e3-mission-setup §4.2, e3-mission-logic §4.6.3).
        f.cells_off = episode == Episode::E3 && number == 12;
        f.late_draw = flags.late_draw;
        f.palette_mode = flags.palette_mode;
        // Default start: draw page 1 (mission-setup §3.1).
        f.draw_page = 1;
        if episode::setup_waypoint_loop(episode) {
            let mut ctx = AiContext {
                world,
                rng: &mut rng,
                player: ai_player,
            };
            ships.choose_all(&mut ctx);
        }
        f.rng = rng;
        f.classes = classes;
        f.ships = ships;
        f.ai_player = ai_player;
        f.mission = logic;
        f.radar_set2 = radar_set2;
        f
    }

    /// The state to carry into the next flight of the same program run.
    pub fn session(&self) -> Session {
        Session {
            episode: self.episode,
            rng: self.rng,
            classes: self.classes.clone(),
            ships: self.ships.clone(),
            ai_player: self.ai_player,
            mission: self.mission,
            radar_set2: self.radar_set2,
        }
    }

    /// The map tile at (x, y) as the flight sees it: the world's map with the E3 flight-12
    /// cell patch while it is in effect ([`Flight::cells_off`]). `None` outside the map.
    pub fn map_tile(&self, world: &World, x: usize, y: usize) -> Option<u8> {
        let t = world.map.get(x, y)?;
        if self.cells_off
            && setup::e3::FLIGHT12_CELLS
                .iter()
                .any(|&(cx, cy, _)| (usize::from(cx), usize::from(cy)) == (x, y))
        {
            return Some(setup::e3::FLIGHT12_TILE);
        }
        Some(t)
    }

    /// The player's tile as the AI sees it (DS:9766/9767: view tile + (5, 3), low bytes).
    pub fn player_tile(&self) -> (u8, u8) {
        let v = self.player.view;
        ((v.tile_x + 5) as u8, (v.tile_y + 3) as u8)
    }

    /// A flight with an explicit player (dev arena, tests), without mission logic. Weapons
    /// follow the player's class (`weapons.md` §3.2; classes 0 and 1 keep weapon 1).
    pub fn with_player(number: u8, player: Player) -> Self {
        let (weapon, _, missiles) = tables::player_weapons(player.class).unwrap_or((1, 1, 0));
        Self {
            episode: Episode::E1,
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
            joystick_enabled: false,
            joystick: Joystick::default(),
            keys: KeyTable::default(),
            frame: DrawnFrame {
                view: player.view,
                heading: player.heading,
                player_visible: true,
                flame: None,
                white_flash: 0,
                hud: HudSnapshot::default(),
            },
            drawn: DrawnObjects::default(),
            ships: Ships::default(),
            shots: Shots::default(),
            explosions: Explosions::default(),
            rng: BpRandom::new(),
            classes: ClassTables::default(),
            ai_player: (0, 0),
            mission: MissionState::default(),
            radar_set2: false,
            cells_off: false,
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
            // 1. Tick counter; E1 flight 21 ends itself after 25 ticks.
            self.ticks = self.ticks.wrapping_add(1);
            if self.episode == Episode::E1 && self.number == 21 && self.ticks == 25 {
                self.end_mission();
            }
            // 3. Turn step.
            self.player.turn_step();
            // 4. Tile window drawn from the current view.
            self.frame.view = self.player.view;
            // 5. Afterburner burst (E3 flight 16 from phase 2: no new burst, e3-mission-logic
            // §4.7.1; ASSUMPTION (U-330): the test sits inside the burst trigger test).
            let no_new_burst =
                self.episode == Episode::E3 && self.number == 16 && self.mission.phase >= 2;
            self.frame.flame = self.player.afterburner(
                &mut self.keys,
                self.joystick.button1,
                self.mission_over,
                no_new_burst,
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
                // E2/E3 have no difficulty multiplier: always 2 × (kind + 1) (P2).
                difficulty: if self.episode == Episode::E1 {
                    self.difficulty
                } else {
                    1
                },
                episode: self.episode,
            });
            self.drawn.shots.clone_from(&self.shots);
            // 7. Player ship (unless drawn late).
            if !self.late_draw {
                self.frame.heading = self.player.heading;
                self.frame.player_visible = self.end_ticks < 15;
            }
            // 8–9. Mission logic (`Msn_UpdateA`/`Msn_UpdateB`, mission-logic §4.2, §5).
            self.msn_update();
            // 10. Mission ships: AI, movement, removal (mission-ships §4.2). E2/E3 have no
            // per-flight cases (flight 0 selects none) and a prologue that flags the live slots
            // (record +000h, read by nobody; e2-mission-ships §4.2).
            self.ai_player = self.player_tile();
            let player = self.ai_player;
            let ai_flight = if self.episode == Episode::E1 {
                self.number
            } else {
                self.ships.flag_live();
                0
            };
            self.ships.update(
                ai_flight,
                &self.classes,
                &mut AiContext {
                    world,
                    rng: &mut self.rng,
                    player,
                },
            );
            // E1 flight 19 with mission phase > 1 caps the player's shields and pins slot 19
            // (mission-ships §4.2.4; removed in E2/E3).
            self.flight_19_pin();
            self.drawn.ships.clone_from(&self.ships);
            // 11. End sequence (provisional: counts the end ticks; its other effects are not
            // specified yet).
            if self.mission_over {
                self.end_ticks = self.end_ticks.saturating_add(1);
            }
            // 12 is presentation. 13. Explosions advance after being drawn.
            self.drawn.explosions.clone_from(&self.explosions);
            self.explosions.step();
            // 14. Palette effects: the white flash counts down. Not yet here: E3 flight 16 with
            // phase >= 5 sets the hit flash to 0 before its step (e3-mission-logic §4.7.1); it
            // goes in with the hit-flash value (hit-flash.md, M10-39).
            self.frame.white_flash = self.white_flash;
            self.white_flash = self.white_flash.saturating_sub(1);
            // 15. Comm viewer on an even back page: a transmission only when the table has
            // one for this flight and phase (mission-logic §4.6); otherwise the request is
            // consumed. E3 flight 16 in phase 2 forces the request every tick, also during the
            // end sequence (e3-mission-logic §4.5).
            if self.episode == Episode::E3 && self.number == 16 && self.mission.phase == 2 {
                self.comm_request = true;
            }
            if self.comm_request && self.draw_page == 0 {
                self.comm_request = false;
                if self.comm_viewer().is_some() {
                    return self.interrupt(16, Modal::CommViewer);
                }
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
            // 18. Mission events (respawns, scripted events; mission-logic §4.7; E1 only).
            self.msn_update_events(events);
            // 19. Instruments: the flight-7 rule, the ticker and the transmission indicator
            // (hud §4.3, §4.7, §4.8); the rest is presentation.
            self.instruments(events);
            self.frame.hud = HudSnapshot {
                armour: self.player.armour,
                shields: self.player.shields,
                missiles: self.missiles,
                missile_cooldown: self.missile_cooldown,
                weapon: self.weapon,
                palette_mode: self.palette_mode,
                phase: self.mission.phase,
                ticker: self.mission.ticker,
                comm_pending: self.mission.comm_pending,
                comm_phase: self.mission.comm_phase,
                home_marker: self.mission.home_marker,
            };
            // 20–22: tick wait, flip (presentation and host timing). 23. Pause.
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
            // E2: `Msn_E2FlightEvents`, E3: `Msn_E3TimedEvents`, after the flight menu
            // (e2-mission-logic §4.7.1, e3-mission-logic §4.6).
            match self.episode {
                Episode::E1 => {}
                Episode::E2 => self.e2_flight_events(events),
                Episode::E3 => self.e3_timed_events(),
            }
            // 26 is audio. 27. Shields and destruction.
            if self.player.shields_step(&self.classes, self.mission_over) {
                self.end_mission();
            }
            // 28. Movement: drift and drag; E3's hazard tiles (e3-mission-ships §5.1); map
            // collision (skipped while drawing late), carry and limits. The edge alert is
            // raised only while the ticker is idle (flight-model §4.4).
            self.player.drift();
            if self.episode == Episode::E3 {
                self.hazard_step(world, events);
            }
            if self.player.collide_and_carry(world, !self.late_draw) && self.mission.ticker.idle() {
                self.edge_hit = true;
            }
            // 29. Comm request (not once the mission is over) and the F5 tactical map (E1: not
            // in flight 14; E3: not in flight 16 from phase 5, e3-mission-logic §4.7.1).
            if self.keys.get(FlightKey::F6) && !self.mission_over {
                self.comm_request = true;
            }
            let map_allowed = match self.episode {
                Episode::E1 => self.number != 14,
                Episode::E2 => true,
                Episode::E3 => self.number != 16 || self.mission.phase < 5,
            };
            if self.keys.get(FlightKey::F5) && map_allowed {
                return self.interrupt(30, Modal::TacticalMap);
            }
        }
        // 30. Thrust and steering.
        let dirs = Dirs::read(&self.keys, self.joystick);
        self.player
            .thrust_and_steer(&self.classes, &dirs, self.mission_over);
        // 31. Mission-ship volleys on back page 1; 32. fire requests cleared every tick
        // (mission-ships §4.4.2).
        if self.draw_page == 1 {
            self.shots
                .ship_volleys(&self.ships, &world.muzzles, self.episode, events);
        }
        self.ships.clear_fire_requests();
        // 33–34. The missile cooldown counts down *before* the fire keys are handled (C-0011:
        // 4, 3, 2, 1, 0 in the ticks after a launch, where weapons.md §2 implies 3, 2, 1, 0),
        // then weapons and infra-red; the cheat chords checked at step 34 are not ported.
        self.missile_cooldown = self.missile_cooldown.saturating_sub(1);
        self.player_specials(world, events);
        // Flight 20: view beyond (170, 170) → armour 64, at the end of every tick (mission-logic
        // §5.20; e2-mission-logic §5.20, `td2:0E96:EA00..EA17`; not in E3).
        if self.number == 20
            && self.episode != Episode::E3
            && self.player.view.tile_x > 170
            && self.player.view.tile_y > 170
        {
            self.player.armour = 64;
        }
        // 35. End of flight.
        if self.aborted || self.end_ticks == 35 || self.lost || self.succeeded {
            TickResult::Finished
        } else {
            TickResult::Done
        }
    }
}

impl Flight {
    /// E3 hazard tiles 41h..46h (e3-mission-ships rev 1 §5.1, `td3:0C5D:D27A..D3CB`): after
    /// drift and drag, before collision and carry, every tick (also while the mission is over,
    /// OB-054), except in flights 14, 17 and 19. The cells are read from the view left by the
    /// previous carry; the third clause repeats the first, as in the original.
    fn hazard_step(&mut self, world: &World, events: &mut Vec<Event>) {
        if matches!(self.number, 14 | 17 | 19) {
            return;
        }
        let v = self.player.view;
        let hazard = |dx: i16, dy: i16| {
            // Cell index (viewX + dx) × 201 + (viewY + dy), 16-bit.
            let i = (v.tile_x + dx)
                .wrapping_mul(201)
                .wrapping_add(v.tile_y + dy) as u16;
            let (x, y) = (usize::from(i) / 201, usize::from(i) % 201);
            self.map_tile(world, x, y)
                .is_some_and(|t| (0x41..=0x46).contains(&t))
        };
        let hit = hazard(5, 2)
            || (hazard(4, 2) && v.sub_x <= 32)
            || (hazard(5, 2) && v.sub_y <= 32)
            || hazard(4, 3);
        if hit {
            // Effect 9 at the player (source FFh, distance 0).
            events.push(Event::Sound {
                kind: 9,
                source: None,
            });
            self.player.shields = 0;
            self.player.armour = self.player.armour.wrapping_sub(17);
        }
    }

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
        // M replays the HUD ticker message when it is idle.
        self.ticker_replay();
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
        h.bytes(&[
            self.episode as u8,
            u8::from(self.radar_set2),
            u8::from(self.cells_off),
        ]);
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
        self.mission.hash_state(h);
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
