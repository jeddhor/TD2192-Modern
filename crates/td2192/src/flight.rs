//! Developer flight arena (M6-08): fly a mission's map with the original flight model.
//!
//! Runs `td_sim::Flight` at the flight tick rate and draws the playfield with
//! `td_render::flight` and the instruments with `td_render::hud` (M6-07). The ticker window and
//! the F6 indicator wait for the mission-logic state (M8-05) and stay blank. Keys as in the original: keypad
//! 8/2/4/6 or the arrow keys thrust, Space fires, Tab switches to missiles, keypad + or keypad
//! 0 fire the afterburner (classes 2 and 5), P pauses (any key resumes), Esc opens the computer
//! menu, F6 reads a transmission, F5 shows the tactical map, Alt toggles infra-red (the modal
//! screens and palettes are `td_flow::flight_screens` and `td_render::flight_palette`,
//! flight-screens.md rev 1). Debug builds only: F9 completes the mission.
//!
//! [`ArenaLauncher`] hands these flights to the campaign (`td_flow::Episode1`,
//! `td_flow::Episode2`) and keeps what the original carries from one flight of a program run
//! to the next: the `td_sim` [`Session`] (RNG, class tables, ship records, mission state) and
//! the F5 target entries.
//!
//! Episode 2 presentation (M10-44.b): the E2 F5 entries and texts, `td2.ccm` transmissions,
//! the radar table of map set 2, the overlay tiles of e2-mission-setup rev 1 §3.6 and the
//! flight-8 `td.hel`/`td.h2` sprites (§4.1 step 6).
//!
//! Mission ships and their target brackets follow `mission-ships.md` §4.7 (with U-127); the
//! afterburner flame, shots and explosions follow `flight-screens.md` rev 1 §4.6–§4.8.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

use anyhow::{Result, bail};
use td_assets::{DataDir, ExeResources, ResourceValue};
use td_audio::{AudioCommand, Pcm8, decode_voc};
use td_flow::flight_screens::{
    CommScreen, ComputerMenu, FlightModal, FlightScreenAssets, ModalStep, Pause, ShipDot,
    TacticalMap, TargetBoard, draw_ir_notice, shows_ir_notice,
};
use td_flow::{
    FLIGHT_TICK_HZ, FlightResult, FlightScreen, FlowContext, PlaceholderLauncher, RETRACE_HZ,
    Screen, Transition,
};
use td_formats::map::TileMap;
use td_formats::mission::Mission;
use td_formats::palette::Palette;
use td_formats::sprites::SpriteSheet;
use td_formats::tileset::Tileset;
use td_input::{Key, TickInput};
use td_render::flight::{self, ShotLook, ViewPos};
use td_render::flight_palette::FlightDac;
use td_render::hud::{Hud, HudAssets, HudState, TickerFont};
use td_render::{Framebuffer, HeadingSprites};
use td_sim::flight::Session;
use td_sim::flight::episode::{self, Episode};
use td_sim::flight::explosions::{FIRST_SPRITE, IDLE};
use td_sim::flight::heading::Heading;
use td_sim::flight::player::Event;
use td_sim::{Flight, FlightKey, Joystick, Modal, TickControls, TickResult, World};

/// Typematic delay and repeat period of an AT keyboard at its power-on defaults (500 ms,
/// 10.9 characters per second). The original's key bytes are re-set by every repeated make
/// code, which re-arms one-shot keys such as the afterburner while a key is held.
const TYPEMATIC_DELAY_US: u64 = 500_000;
const TYPEMATIC_PERIOD_US: u64 = 91_743;

/// Host keys for each original key byte (the grey Insert key is a different byte in the
/// original and is not read in flight).
fn bindings() -> Vec<(Key, FlightKey)> {
    let letter = |c| Key::letter(c).expect("letter");
    vec![
        (Key(0x60), FlightKey::Kp8),
        (Key::UP, FlightKey::Up),
        (Key(0x5A), FlightKey::Kp2),
        (Key::DOWN, FlightKey::Down),
        (Key(0x5C), FlightKey::Kp4),
        (Key::LEFT, FlightKey::Left),
        (Key(0x5E), FlightKey::Kp6),
        (Key::RIGHT, FlightKey::Right),
        (Key::KP_PLUS, FlightKey::KpPlus),
        (Key(0x62), FlightKey::Kp0),
        (Key::LALT, FlightKey::Alt),
        (Key::RALT, FlightKey::RightAlt),
        (Key::SPACE, FlightKey::Space),
        (Key::ESCAPE, FlightKey::Esc),
        (letter('p'), FlightKey::P),
        (Key(0x3E), FlightKey::F5),
        (Key::F6, FlightKey::F6),
        (letter('s'), FlightKey::S),
    ]
}

/// Turns tick-sampled host key state into the original's make codes, with typematic repeats.
#[derive(Debug, Default)]
struct Typematic {
    /// Microseconds each held key has been down.
    held_us: BTreeMap<FlightKey, u64>,
}

impl Typematic {
    fn controls(&mut self, input: &TickInput, binds: &[(Key, FlightKey)]) -> TickControls {
        let tick_us = (1e6 / FLIGHT_TICK_HZ) as u64;
        let mut c = TickControls::default();
        if let Some(p) = input.pad {
            c.set_joystick(Joystick {
                x: p.x,
                y: p.y,
                button1: p.button1,
                button2: p.button2,
            });
        }
        for &(host, key) in binds {
            let down = input.held.contains(&host);
            let pressed = input.pressed.contains(&host);
            if down {
                c.set_down(key);
            }
            if pressed {
                c.set_made(key);
                self.held_us.insert(key, 0);
                continue;
            }
            if !down {
                self.held_us.remove(&key);
                continue;
            }
            let t0 = *self.held_us.get(&key).unwrap_or(&0);
            let t1 = t0 + tick_us;
            self.held_us.insert(key, t1);
            // A repeat fires at DELAY + n·PERIOD; did one fall in (t0, t1]?
            let count = |t: u64| {
                t.checked_sub(TYPEMATIC_DELAY_US)
                    .map_or(0, |d| d / TYPEMATIC_PERIOD_US + 1)
            };
            if count(t1) > count(t0) {
                c.set_made(key);
            }
        }
        c
    }
}

#[derive(Debug)]
pub struct FlightArena {
    world: World,
    tiles: Tileset,
    palette: Palette,
    ships: HeadingSprites,
    /// The 27-sprite bank: `td.shp` frames 0..20, then `td.exp` frames 0..7.
    bank: SpriteSheet,
    /// Effect samples by effect id.
    effects: BTreeMap<u8, Arc<Pcm8>>,
    hud_assets: HudAssets,
    /// The instruments, stepped by `draw` once per simulation tick.
    hud: RefCell<HudDisplay>,
    /// Ticker texts by resource id.
    ticker_texts: BTreeMap<&'static str, Vec<String>>,
    flight: Flight,
    /// The modal the current tick is suspended at.
    modal: Option<Modal>,
    /// Its screen (flight-screens.md rev 1).
    screen: Option<FlightModal>,
    screens: FlightScreenAssets,
    /// The DAC: infra-red modes and the hit and white flashes (flight-screens §4.5).
    dac: FlightDac,
    /// The F5 entries of this program run (flight-screens §3.4).
    board: TargetBoard,
    /// The program run this flight belongs to, when the campaign launched it.
    run: Option<Rc<RefCell<RunState>>>,
    binds: Vec<(Key, FlightKey)>,
    typematic: Typematic,
    events: Vec<Event>,
}

/// What one program run keeps across its flights.
#[derive(Debug, Default)]
pub struct RunState {
    /// RNG, class tables, ship records and mission state (`td_sim::flight::Session`).
    pub session: Session,
    /// The F5 target entries (`Msn_LoadTargetInfo` rewrites only part of them per flight).
    pub board: TargetBoard,
}

impl FlightArena {
    /// A flight of `episode` as the first of a program run (developer arena).
    pub fn new(
        dir: &DataDir,
        resources: &ExeResources,
        episode: Episode,
        number: u8,
    ) -> Result<Self> {
        Self::open(dir, resources, episode, number, None)
    }

    /// Flight `number` of the program run `run` (a fresh Episode 1 run when `None`); the
    /// episode is the run's.
    pub fn start(
        dir: &DataDir,
        resources: &ExeResources,
        number: u8,
        run: Option<Rc<RefCell<RunState>>>,
    ) -> Result<Self> {
        let ep = run
            .as_ref()
            .map_or(Episode::E1, |r| r.borrow().session.episode);
        Self::open(dir, resources, ep, number, run)
    }

    fn open(
        dir: &DataDir,
        resources: &ExeResources,
        ep: Episode,
        number: u8,
        run: Option<Rc<RefCell<RunState>>>,
    ) -> Result<Self> {
        let last = match ep {
            Episode::E1 | Episode::E3 => 21,
            Episode::E2 => 20,
        };
        if !(1..=last).contains(&number) {
            bail!("{ep} flight number must be 1..={last}");
        }
        // E3 never flies flight 20: its setup block is dead (e3-mission-setup §3.2).
        if ep == Episode::E3 && number == 20 {
            bail!("{ep} has no flight 20 (flights 1..=19 and 21)");
        }
        // The muzzle table is byte-identical in every episode (episode-deltas §11).
        let Some(ResourceValue::Bytes(muzzles)) = resources.get("flight.muzzle_offsets").cloned()
        else {
            bail!("file0001.exe resources are unavailable (muzzle table)");
        };
        // E1 flight 21 loads no mission file and flies the ship records it inherits.
        let mission = match episode::mission_file(ep, number) {
            None => None,
            Some(file) => Some(Mission::parse(&dir.read(&file)?)?),
        };
        let mut shp = dir.read("td.shp")?;
        let (mut sc3, mut shr, mut sc2) = (
            dir.read("td.sc3")?,
            dir.read("td.shr")?,
            dir.read("td.sc2")?,
        );
        if ep == Episode::E2 && number == 8 {
            // E2 flight 8 (e2-mission-setup rev 1 §4.1 step 6): td.hel into the sprite bank at
            // +800h (slots 2..3), td.h2 into +C80h of the rotation buffers td.sc2, td.shr,
            // td.sc3 (cells 2..3, C80h bytes each; FORMATS §6.3).
            overwrite(&mut shp, 0x800, &dir.read("td.hel")?)?;
            let h2 = dir.read("td.h2")?;
            for (k, set) in [&mut sc2, &mut shr, &mut sc3].into_iter().enumerate() {
                let part = h2.get(k * 0xC80..(k + 1) * 0xC80);
                overwrite(set, 0xC80, part.unwrap_or_default())?;
            }
        }
        let base = SpriteSheet::parse_base(&shp)?;
        let rot = [
            SpriteSheet::parse_rotation_set(&sc3)?,
            SpriteSheet::parse_rotation_set(&shr)?,
            SpriteSheet::parse_rotation_set(&sc2)?,
        ];
        let Some(ships) = HeadingSprites::build(&base, [&rot[0], &rot[1], &rot[2]], 20) else {
            bail!("sprite sheets are inconsistent");
        };
        let mut bank_bytes = shp;
        bank_bytes.extend(dir.read("td.exp")?);
        let bank = SpriteSheet::parse(&bank_bytes, 32)?;
        let mut effects = BTreeMap::new();
        for &(id, voc) in td_audio::sfx::EFFECTS.iter() {
            if let Some(voc) = voc {
                effects.insert(id, Arc::new(decode_voc(&dir.read(voc)?)?));
            }
        }
        let world = episode::world(
            ep,
            number,
            TileMap::parse_episode(&dir.read(episode::map_file(ep, number))?)?,
            muzzles,
        );
        let session = run
            .as_ref()
            .map_or_else(|| Session::for_episode(ep), |r| r.borrow().session.clone());
        let flight = Flight::start(number, mission.as_ref(), &world, session);
        // E2: the map-set-2 colours replace the table for the rest of the run (OB-024).
        let radar_id = match (ep, flight.radar_set2) {
            (Episode::E2, false) => "e2.hud.radar_tile_colours",
            (Episode::E2, true) => "e2.hud.radar_tile_colours_set2",
            // E3 never copies the second table (e3-mission-setup §3.1).
            (Episode::E3, _) => "e3.hud.radar_tile_colours",
            _ => "hud.radar_tile_colours",
        };
        let Some(ResourceValue::Bytes(radar)) = resources.get(radar_id).cloned() else {
            bail!("executable resources are unavailable (radar colours {radar_id})");
        };
        let hud_assets = HudAssets::new(
            dir.read("td.win")?,
            dir.read("td.wea")?,
            dir.read("td.fac")?,
            &dir.read("td.lt2")?,
            radar,
        )?;
        let hud = Hud::new(&hud_assets);
        // Texts the mission logic can post to the ticker, by resource id (the sim carries ids).
        let mut ticker_texts = BTreeMap::new();
        for id in TICKER_RESOURCES {
            // Lists, or single texts (the E2 jurisdiction warning and flight-18 event).
            let texts = resources
                .texts(id)
                .map(<[String]>::to_vec)
                .or_else(|| resources.text(id).map(|t| vec![t.to_owned()]));
            if let Some(texts) = texts {
                ticker_texts.insert(id, texts);
            }
        }
        let screens = FlightScreenAssets::load_episode(dir, resources, ep, flight.radar_set2)?;
        let board = {
            let mut fresh = TargetBoard::default();
            let mut shared = run.as_ref().map(|r| r.borrow_mut());
            let board = shared.as_mut().map_or(&mut fresh, |r| &mut r.board);
            board.load_episode(ep, number);
            board.clone()
        };
        let palette = Palette::parse(&dir.read("td.pal")?)?;
        Ok(Self {
            world,
            tiles: Tileset::parse(&dir.read(episode::tileset_file(ep, number))?)?,
            dac: FlightDac::start(&palette, flight.palette_mode),
            palette,
            ships,
            bank,
            effects,
            hud_assets,
            hud: RefCell::new(HudDisplay {
                hud,
                tick: None,
                draws: None,
            }),
            ticker_texts,
            flight,
            modal: None,
            screen: None,
            screens,
            board,
            run,
            binds: bindings(),
            typematic: Typematic::default(),
            events: Vec::new(),
        })
    }
}

impl Screen for FlightArena {
    fn tick_hz(&self) -> f64 {
        // The modal screens step once per vertical retrace (fades, pulses, typing).
        if self.screen.is_some() {
            RETRACE_HZ
        } else {
            FLIGHT_TICK_HZ
        }
    }

    fn tick(&mut self, ctx: &mut FlowContext<'_>, input: &TickInput) -> Transition {
        // The original reads the joystick only when it is the chosen controller; the port
        // uses it whenever one is connected.
        self.flight.joystick_enabled = input.pad.is_some();
        self.events.clear();
        let mode = self.flight.palette_mode;
        let mut result = if let Some(screen) = &mut self.screen {
            match screen.tick(&self.screens, input) {
                ModalStep::Continue => return Transition::None,
                ModalStep::Close { abort } => {
                    let menu = matches!(screen, FlightModal::Menu(_));
                    self.screen = None;
                    self.modal = None;
                    if abort {
                        // Quit: g_MissionAborted := 1; the tick then leaves the loop.
                        self.flight.aborted = true;
                    } else if menu {
                        // The menu's last FadeIn restored the mode (flight-screens §4.1); every
                        // fade level zeroed g_HitFlash (hit-flash.md §4.2).
                        self.dac.faded_in(&self.palette, self.flight.palette_mode);
                        self.flight.hit_flash = 0;
                    }
                    let c = self.typematic.controls(input, &self.binds);
                    self.flight.resume(&self.world, &c, &mut self.events)
                }
            }
        } else {
            let c = self.typematic.controls(input, &self.binds);
            if cfg!(debug_assertions) && input.pressed(Key(0x42)) {
                // F9, debug builds only: complete the mission.
                self.flight.succeeded = true;
            }
            let r = self.flight.tick(&self.world, &c, &mut self.events);
            // 14. Palette effects, in the mode the step saw (Alt acts later, at step 33), from
            // the hit-flash level the step found (hit-flash.md §4.3).
            self.dac.set_hit_flash(self.flight.frame.hit_flash);
            self.dac
                .step(&self.palette, mode, self.flight.frame.white_flash);
            r
        };
        while let TickResult::Modal(m) = result {
            self.modal = Some(m);
            if let Some(screen) = self.open_screen(m) {
                self.screen = Some(screen);
                break;
            }
            // Nothing to show: continue the tick at once.
            self.modal = None;
            result = self
                .flight
                .resume(&self.world, &TickControls::default(), &mut self.events);
        }
        // 33. Alt toggled infra-red: the start formula of the new mode.
        if self.flight.palette_mode != mode {
            self.dac.toggle(&self.palette, self.flight.palette_mode);
        }
        let v = self.flight.player.view;
        for &Event::Sound { kind, source } in &self.events {
            ctx.audio.push(AudioCommand::PlayEffect {
                id: kind,
                sound: self.effects.get(&kind).cloned(),
                source,
                view: (v.tile_x as u8, v.tile_y as u8),
            });
        }
        match result {
            TickResult::Done | TickResult::Modal(_) => Transition::None,
            TickResult::Finished => {
                // Hand the session to the next flight of the program run.
                if let Some(run) = &self.run {
                    run.borrow_mut().session = self.flight.session();
                }
                Transition::Pop
            }
        }
    }

    fn draw(&self, fb: &mut Framebuffer) {
        if let Some(screen) = &self.screen {
            screen.draw(fb);
            return;
        }
        fb.clear(0);
        fb.palette = self.dac.current().clone();
        let f = &self.flight.frame;
        let view = ViewPos {
            tile_x: i32::from(f.view.tile_x),
            tile_y: i32::from(f.view.tile_y),
            sub_x: i32::from(f.view.sub_x),
            sub_y: i32::from(f.view.sub_y),
        };
        let map = &self.world.map;
        flight::draw_tile_window(fb, map, &self.tiles, view);
        let drawn = &self.flight.drawn;
        let heading_frame = |sprite: usize, code: u8| {
            let h = Heading::from_code(code)?;
            self.ships.frame(sprite, h.sprite_index())
        };
        let cell = td_render::headings::CELL;
        // 5. Afterburner flame: sprite 9 + burst frame with the burst heading's blit
        // (flight-screens §4.6).
        if let Some(sprite) = f.flame {
            let code = self.flight.player.burst_heading.code();
            if let (Some(pos), Some(frame)) = (
                flight::flame_pos(code),
                heading_frame(usize::from(sprite), code),
            ) {
                flight::draw_sprite(fb, frame, cell, pos);
            }
        }
        // 6. Shots (flight-screens §4.8), where they were drawn: before their move.
        for s in drawn.shots.0.iter().filter(|s| s.alive()) {
            let look = flight::shot_look(
                view,
                s.kind,
                s.heading,
                drawn_shot_tile(s),
                (s.off_x, s.off_y),
            );
            match look {
                Some(ShotLook::Pixels(pixels)) => {
                    for (x, y, c) in pixels {
                        if (0..flight::PLAYFIELD.h).contains(&y) {
                            fb.put(x, y, c);
                        }
                    }
                }
                Some(ShotLook::Sprite { slot, heading, at }) => {
                    if let Some(frame) = heading_frame(slot, heading) {
                        flight::draw_sprite(fb, frame, cell, flight::sprite_screen(view, at));
                    }
                }
                None => {}
            }
        }
        // 7. Player ship.
        let class = usize::from(self.flight.player.class);
        let late = self.flight.late_draw;
        let draw_player = |fb: &mut Framebuffer| {
            if f.player_visible
                && let Some(frame) = self.ships.frame(class, f.heading.sprite_index())
            {
                flight::draw_player_ship(fb, frame, td_render::headings::CELL);
            }
        };
        if !late {
            draw_player(fb);
        }
        // 10. Mission ships.
        self.draw_mission_ships(fb, view);
        // 12. Overpasses (E2: overlay tiles of the whole window, e2-mission-setup §3.6).
        if !late {
            match self.flight.episode {
                Episode::E2 => {
                    let set = episode::map_set(Episode::E2, self.flight.number);
                    flight::redraw_overlays_e2(fb, map, &self.tiles, view, set);
                }
                _ => flight::redraw_overpasses(fb, map, &self.tiles, view),
            }
        }
        // 13. Explosions: bank slot 19 + frame (flight-screens §4.7).
        let page = self.drawn_page();
        for e in drawn.explosions.0.iter().filter(|e| e.frame < IDLE) {
            let at = flight::explosion_sprite(view, page, (e.tile_x, e.tile_y), (e.sub_x, e.sub_y));
            if let (Some(at), Some(frame)) =
                (at, self.bank.frame(usize::from(FIRST_SPRITE + e.frame)))
            {
                flight::draw_sprite(fb, frame, 32, flight::sprite_screen(view, at));
            }
        }
        // 16. Player ship drawn late.
        if late {
            draw_player(fb);
        }
        // 19. Instruments: the guidance dots go into the playfield, the rest below it.
        let mut display = self.hud.borrow_mut();
        if display.tick != Some(self.flight.ticks) {
            let f = &self.flight;
            // The mission logic drew a (new or repeated) text into the ticker strip.
            let ticker = f.frame.hud.ticker;
            if display.draws != Some(ticker.draws)
                && let Some(message) = ticker.message
            {
                let (id, index) = message.resource();
                if let Some(text) = self.ticker_texts.get(id).and_then(|v| v.get(index)) {
                    let font = match message {
                        m if m.flight_font() => TickerFont::Flight,
                        _ => TickerFont::Mission,
                    };
                    display.hud.post(&self.hud_assets, text, ticker.ink, font);
                }
            }
            display.draws = Some(ticker.draws);
            // Instruments read the state as it was at tick-order step 19.
            let snap = f.frame.hud;
            let state = HudState {
                armour: snap.armour,
                shields: snap.shields,
                missiles: snap.missiles,
                missile_cooldown: snap.missile_cooldown,
                weapon: snap.weapon,
                palette_mode: snap.palette_mode,
                phase: snap.phase,
                home_marker: snap.home_marker,
                comm_phase: snap.comm_phase,
                ticker_blit: (ticker.window != 0).then_some(ticker.window),
                ..HudState::from_flight(f)
            };
            display.hud.step(&self.hud_assets, &self.world.map, &state);
            display.tick = Some(f.ticks);
        }
        display.hud.draw(fb);
        drop(display);
        self.draw_overlays(fb);
    }
}

/// The instruments and the tick they were last stepped for (`draw` may run more or less often
/// than the simulation ticks).
#[derive(Debug)]
struct HudDisplay {
    hud: Hud,
    tick: Option<u32>,
    /// The sim's ticker draw counter at the last post.
    draws: Option<u16>,
}

/// Resource ids of the texts the mission logic posts (`td_sim::flight::logic::Message`).
const TICKER_RESOURCES: [&str; 14] = [
    "e1.objectives",
    "e1.flight_messages.01",
    "e1.flight_messages.02",
    "jurisdiction.warnings",
    "e1.flight_events.01",
    "e2.objectives",
    "e2.flight_messages.01",
    "e2.flight_messages.02",
    "e2.jurisdiction.warnings",
    "e2.flight_events",
    "e3.objectives",
    "e3.flight_messages.01",
    "e3.flight_messages.02",
    "e3.jurisdiction.warnings",
];

/// Writes `bytes` into `buf` at `at` (a mission's extra sprites over the loaded sheets).
fn overwrite(buf: &mut [u8], at: usize, bytes: &[u8]) -> Result<()> {
    match buf.get_mut(at..at + bytes.len()) {
        Some(dst) if !bytes.is_empty() => {
            dst.copy_from_slice(bytes);
            Ok(())
        }
        _ => bail!(
            "sprite overlay of {} bytes at {at:#X} does not fit",
            bytes.len()
        ),
    }
}

/// The tile a shot was drawn at: `Flight::drawn` holds the shots after step 6, whose last part
/// moves every live shot one tile along its heading (weapons.md §4.3 step 8) after drawing it.
/// A shot freed by the age limit in that step was drawn once more by the original but is no
/// longer in the slot table, so it is missing here.
fn drawn_shot_tile(s: &td_sim::flight::shots::Shot) -> (u8, u8) {
    let (dx, dy): (u8, u8) = match s.heading {
        8 => (0, 255),
        2 => (0, 1),
        4 => (255, 0),
        6 => (1, 0),
        9 => (1, 255),
        7 => (255, 255),
        1 => (255, 1),
        3 => (1, 1),
        _ => (0, 0),
    };
    (s.tile_x.wrapping_sub(dx), s.tile_y.wrapping_sub(dy))
}

impl FlightArena {
    /// The back page the last tick drew into: the tick toggles it at step 24, unless it is
    /// suspended by a modal screen before that step.
    fn drawn_page(&self) -> i32 {
        let page = i32::from(self.flight.draw_page);
        match self.modal {
            Some(Modal::CommViewer | Modal::Pause) => page,
            _ => page ^ 1,
        }
    }

    /// Mission ships and their target brackets (`mission-ships.md` rev 2 §4.7, T2, with the
    /// row reading of U-127): sprite = class code at the slot's heading.
    ///
    /// A ship removed at the map edge is drawn once more by the original (with sprite FFh),
    /// but only inside the drawing window, which never reaches the edge rows and columns (the
    /// view stays within tiles 6..183 × 7..190), so it is never visible and is not drawn here.
    fn draw_mission_ships(&self, fb: &mut Framebuffer, view: ViewPos) {
        let page = self.drawn_page();
        for s in self.flight.drawn.ships.0.iter().filter(|s| s.alive()) {
            let mark = flight::ShipMark {
                tile_x: s.tile_x,
                tile_y: s.tile_y,
                sub_x: s.sub_x,
                sub_y: s.sub_y,
                marker: s.marker,
            };
            let Some(pos) = flight::mission_ship_pos(view, page, &mark) else {
                continue;
            };
            let heading = Heading::from_code(s.heading).unwrap_or(Heading::N);
            let frame = self
                .ships
                .frame(usize::from(s.class), heading.sprite_index())
                .map(|f| (f, td_render::headings::CELL));
            flight::draw_mission_ship(fb, frame, pos, s.marker);
        }
    }

    /// Text the tick draws over the finished picture: the infra-red notice (step 21).
    fn draw_overlays(&self, fb: &mut Framebuffer) {
        if shows_ir_notice(self.flight.palette_mode, self.flight.ticks) {
            draw_ir_notice(fb, &self.screens);
        }
    }

    /// The screen for modal `m`, over the picture on screen now; `None` if there is nothing
    /// to show.
    fn open_screen(&self, m: Modal) -> Option<FlightModal> {
        let mut shown = Framebuffer::new();
        self.draw(&mut shown);
        let f = &self.flight;
        Some(match m {
            Modal::Pause => FlightModal::Pause(Box::new(Pause::new(shown, &self.screens))),
            Modal::FlightMenu => FlightModal::Menu(Box::new(ComputerMenu::new(
                shown,
                &self.palette,
                f.palette_mode,
                f.number,
            ))),
            // The mission logic chose the transmission and advanced the phase (step 15).
            Modal::CommViewer => {
                let n = usize::from(f.mission.transmission);
                FlightModal::Comm(Box::new(CommScreen::new(shown, &self.screens, n)?))
            }
            Modal::TacticalMap => {
                let dots: Vec<ShipDot> = f
                    .ships
                    .0
                    .iter()
                    .map(|s| ShipDot {
                        class: s.class,
                        tile: (s.tile_x, s.tile_y),
                        marked: s.marker != 0,
                    })
                    .collect();
                let v = f.player.view;
                FlightModal::Map(Box::new(TacticalMap::new(
                    &shown.palette,
                    &self.screens,
                    &self.world.map,
                    (i32::from(v.tile_x), i32::from(v.tile_y)),
                    &dots,
                    &self.board,
                )))
            }
        })
    }
}

impl FlightScreen for FlightArena {
    fn result(&self) -> FlightResult {
        let f = &self.flight;
        // Teardown (mission-logic.md rev 1 §4.8).
        FlightResult {
            died: f.died(),
            aborted: f.aborted,
            lost: f.lost,
            outcome: f.outcome(),
            kills: f.kills,
        }
    }
}

/// Starts real flights for the campaign; falls back to the placeholder flight if a flight's
/// data cannot be loaded. One launcher is one program run: it owns the [`RunState`], so the
/// RNG stream starts at program start (seed 0, then the AI unit's 20 `Random(2)`,
/// mission-ships §4.1) and runs through every flight and the front end's `Random` calls.
#[derive(Debug, Default)]
pub struct ArenaLauncher {
    run: Rc<RefCell<RunState>>,
    fallback: PlaceholderLauncher,
}

impl ArenaLauncher {
    /// The flights of `episode`'s program run (program start of that executable).
    pub fn for_episode(episode: Episode) -> Self {
        let run = RunState {
            session: Session::for_episode(episode),
            board: TargetBoard::default(),
        };
        Self {
            run: Rc::new(RefCell::new(run)),
            fallback: PlaceholderLauncher::default(),
        }
    }
}

impl td_flow::FlightLauncher for ArenaLauncher {
    fn launch(&mut self, ctx: &mut FlowContext<'_>, flight: u8) -> Box<dyn FlightScreen> {
        match FlightArena::start(ctx.data, ctx.resources, flight, Some(self.run.clone())) {
            Ok(arena) => Box::new(arena),
            Err(e) => {
                tracing::error!("flight {flight} could not start: {e:#}");
                self.fallback.launch(ctx, flight)
            }
        }
    }

    fn random(&mut self, n: u16) -> u16 {
        self.run.borrow_mut().session.rng.random(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use td_flow::FlightLauncher;

    #[test]
    fn launcher_random_continues_the_program_stream() {
        // Program start: seed 0, then the AI unit's 20 Random(2); the front end's Random(10)
        // comes from the same stream as the flights.
        let mut launcher = ArenaLauncher::default();
        let mut session = Session::new();
        for _ in 0..3 {
            assert_eq!(launcher.random(10), session.rng.random(10));
        }
        assert_eq!(launcher.run.borrow().session.rng, session.rng);
    }

    #[test]
    fn typematic_repeats_after_the_delay() {
        let binds = [(Key::SPACE, FlightKey::Space)];
        let mut t = Typematic::default();
        let mut first = TickInput::default();
        first.held.insert(Key::SPACE);
        first.pressed.push(Key::SPACE);
        let held = TickInput {
            held: first.held.clone(),
            ..TickInput::default()
        };
        let made = |c: TickControls| c.to_word() >> 32 != 0;
        assert!(made(t.controls(&first, &binds)));
        // 500 ms ≈ 9.6 ticks at 19.2 Hz: no repeat for 9 ticks, then one about every 1.8.
        let repeats: Vec<bool> = (1..=14).map(|_| made(t.controls(&held, &binds))).collect();
        assert_eq!(&repeats[..9], &[false; 9]);
        assert!(repeats[9]);
        assert!(repeats[9..].iter().filter(|&&r| r).count() >= 2);
    }
}
