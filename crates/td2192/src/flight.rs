//! Developer flight arena (M6-08): fly a mission's map with the original flight model.
//!
//! Runs `td_sim::Flight` at the flight tick rate and draws the playfield and HUD bar with
//! `td_render::flight`. The mission's ships sit at their path starts (movement and AI are M8);
//! weapons work. The radar and HUD gauges are not in yet. Keys as in the original: keypad
//! 8/2/4/6 or the arrow keys thrust, Space fires, Tab switches to missiles, keypad + or keypad
//! 0 fire the afterburner (classes 2 and 5), P pauses (any key resumes), Esc aborts the flight
//! (the in-flight computer menu is not built yet). Debug builds only: F9 completes the mission
//! (mission logic, M8-05, is not in yet; this lets the campaign be played through).
//!
//! [`FlightLauncher`] hands these flights to the Episode 1 campaign (`td_flow::Episode1`).
//!
//! Presentation details still unspecified (T2 placeholders): where ship, shot and explosion
//! sprites sit relative to their world position, what shots look like, and the flame sprite.

use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::{Result, bail};
use td_assets::{DataDir, ExeResources, ResourceValue};
use td_audio::{AudioCommand, Pcm8, decode_voc};
use td_flow::{
    FLIGHT_TICK_HZ, FlightResult, FlightScreen, FlowContext, PlaceholderLauncher, Screen,
    Transition,
};
use td_formats::map::TileMap;
use td_formats::mission::Mission;
use td_formats::palette::Palette;
use td_formats::sprites::SpriteSheet;
use td_formats::tileset::Tileset;
use td_input::{Key, TickInput};
use td_render::flight::{self, HUD_H, HUD_W, ViewPos};
use td_render::{Framebuffer, HeadingSprites, Image};
use td_sim::flight::explosions::{FIRST_SPRITE, IDLE};
use td_sim::flight::heading::Heading;
use td_sim::flight::player::Event;
use td_sim::flight::shots::Muzzles;
use td_sim::flight::tables::episode1_tile_attributes;
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
    hud: Vec<u8>,
    flight: Flight,
    modal: Option<Modal>,
    binds: Vec<(Key, FlightKey)>,
    typematic: Typematic,
    events: Vec<Event>,
}

impl FlightArena {
    pub fn new(dir: &DataDir, resources: &ExeResources, number: u8) -> Result<Self> {
        if !(1..=21).contains(&number) {
            bail!("flight number must be 1..=21");
        }
        let Some(ResourceValue::Bytes(muzzles)) = resources.get("flight.muzzle_offsets").cloned()
        else {
            bail!("file0001.exe resources are unavailable (muzzle table)");
        };
        let mission = Mission::parse(&dir.read(&format!("m{number}.td"))?)?;
        let base = SpriteSheet::parse_base(&dir.read("td.shp")?)?;
        let rot = [
            SpriteSheet::parse_rotation_set(&dir.read("td.sc3")?)?,
            SpriteSheet::parse_rotation_set(&dir.read("td.shr")?)?,
            SpriteSheet::parse_rotation_set(&dir.read("td.sc2")?)?,
        ];
        let Some(ships) = HeadingSprites::build(&base, [&rot[0], &rot[1], &rot[2]], 9) else {
            bail!("sprite sheets are inconsistent");
        };
        let mut bank_bytes = dir.read("td.shp")?;
        bank_bytes.extend(dir.read("td.exp")?);
        let bank = SpriteSheet::parse(&bank_bytes, 32)?;
        let mut effects = BTreeMap::new();
        for &(id, voc) in td_audio::sfx::EFFECTS.iter() {
            if let Some(voc) = voc {
                effects.insert(id, Arc::new(decode_voc(&dir.read(voc)?)?));
            }
        }
        let hud = dir.read("td.win")?;
        if hud.len() != HUD_W * HUD_H {
            bail!("td.win has an unexpected size");
        }
        let world = World {
            map: TileMap::parse_episode(&dir.read("td.map")?)?,
            tile_attr: episode1_tile_attributes(),
            muzzles: Muzzles(muzzles),
        };
        let flight = Flight::with_mission(number, &mission, &world);
        Ok(Self {
            world,
            tiles: Tileset::parse(&dir.read("td.blk")?)?,
            palette: Palette::parse(&dir.read("td.pal")?)?,
            ships,
            bank,
            effects,
            hud,
            flight,
            modal: None,
            binds: bindings(),
            typematic: Typematic::default(),
            events: Vec::new(),
        })
    }
}

impl Screen for FlightArena {
    fn tick_hz(&self) -> f64 {
        FLIGHT_TICK_HZ
    }

    fn tick(&mut self, ctx: &mut FlowContext<'_>, input: &TickInput) -> Transition {
        let c = self.typematic.controls(input, &self.binds);
        // The original reads the joystick only when it is the chosen controller; the port
        // uses it whenever one is connected.
        self.flight.joystick_enabled = input.pad.is_some();
        self.events.clear();
        if cfg!(debug_assertions) && input.pressed(Key(0x42)) {
            // F9, debug builds only: complete the mission.
            self.flight.succeeded = true;
        }
        let result = match self.modal.take() {
            None => self.flight.tick(&self.world, &c, &mut self.events),
            // The original waits until any key byte is set.
            Some(Modal::Pause) if input.pressed.is_empty() => {
                self.modal = Some(Modal::Pause);
                return Transition::None;
            }
            // The comm viewer and the ship-switch screen are not implemented yet: close them
            // at once.
            // The flight menu is not built yet: Esc aborts the flight.
            Some(Modal::FlightMenu) => {
                self.flight.aborted = true;
                self.flight.resume(&self.world, &c, &mut self.events)
            }
            Some(_) => self.flight.resume(&self.world, &c, &mut self.events),
        };
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
            TickResult::Done => Transition::None,
            TickResult::Finished => Transition::Pop,
            TickResult::Modal(m) => {
                self.modal = Some(m);
                Transition::None
            }
        }
    }

    fn draw(&self, fb: &mut Framebuffer) {
        fb.clear(0);
        fb.palette = self.palette.clone();
        let f = &self.flight.frame;
        if f.white_flash > 1 {
            fb.palette.colors.fill(td_formats::palette::Rgb6 {
                r: 63,
                g: 63,
                b: 63,
            });
        }
        let view = ViewPos {
            tile_x: i32::from(f.view.tile_x),
            tile_y: i32::from(f.view.tile_y),
            sub_x: i32::from(f.view.sub_x),
            sub_y: i32::from(f.view.sub_y),
        };
        let map = &self.world.map;
        flight::draw_tile_window(fb, map, &self.tiles, view);
        let (ox, oy) = view.page_origin();
        let origin = (view.tile_x * 32 + ox, view.tile_y * 32 + oy);
        let to_screen = |(x, y): (i32, i32)| (x - origin.0, y - origin.1);
        let drawn = &self.flight.drawn;
        // 6. Shots (placeholder look: a 3-pixel dot in the kind's colour, drawn 16 px up so
        // that the muzzle offsets line up with the guns of the ship sprites).
        for s in drawn.shots.0.iter().filter(|s| s.alive()) {
            let (x, y) = to_screen(s.world());
            let colour = [0x3C, 0x3D, 0x35, 0xBE]
                .get(usize::from(s.kind))
                .copied()
                .unwrap_or(0xBE);
            for d in -1..=1 {
                if flight::PLAYFIELD.y <= y - 16 && y - 16 < flight::PLAYFIELD.h {
                    fb.put(x + d, y - 16, colour);
                }
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
        // 10. Mission ships (placeholder placement: frame centred on the ship's position).
        for s in drawn.ships.0.iter().filter(|s| s.alive()) {
            let heading = Heading::from_code(s.heading).unwrap_or(Heading::N);
            if let Some(frame) = self
                .ships
                .frame(usize::from(s.class), heading.sprite_index())
            {
                let (x, y) = to_screen(s.world());
                let cell = td_render::headings::CELL;
                let half = cell as i32 / 2;
                fb.blit(
                    Image::new(frame, cell, cell),
                    x - half,
                    y - half,
                    flight::PLAYFIELD,
                    Some(0),
                );
            }
        }
        // 12. Overpasses.
        if !late {
            flight::redraw_overpasses(fb, map, &self.tiles, view);
        }
        // 13. Explosions (placeholder placement: centred).
        for e in drawn.explosions.0.iter().filter(|e| e.frame < IDLE) {
            let world = (
                i32::from(e.tile_x) * 32 + i32::from(e.sub_x),
                i32::from(e.tile_y) * 32 + i32::from(e.sub_y),
            );
            if let Some(frame) = self.bank.frame(usize::from(FIRST_SPRITE + e.frame)) {
                let (x, y) = to_screen(world);
                fb.blit(
                    Image::new(frame, 32, 32),
                    x - 16,
                    y - 16,
                    flight::PLAYFIELD,
                    Some(0),
                );
            }
        }
        // 16. Player ship drawn late.
        if late {
            draw_player(fb);
        }
        flight::draw_hud_bar(fb, &self.hud);
    }
}

impl FlightScreen for FlightArena {
    fn result(&self) -> FlightResult {
        let f = &self.flight;
        FlightResult {
            died: f.end_ticks == 35,
            aborted: f.aborted,
            lost: f.lost,
            // The mission-specific outcome tests (teardown) come with mission logic (M8-05).
            outcome: true,
            kills: f.kills,
        }
    }
}

/// Starts real flights for the campaign; falls back to the placeholder flight if a flight's
/// data cannot be loaded.
#[derive(Debug, Default)]
pub struct ArenaLauncher;

impl td_flow::FlightLauncher for ArenaLauncher {
    fn launch(&mut self, ctx: &mut FlowContext<'_>, flight: u8) -> Box<dyn FlightScreen> {
        match FlightArena::new(ctx.data, ctx.resources, flight) {
            Ok(arena) => Box::new(arena),
            Err(e) => {
                tracing::error!("flight {flight} could not start: {e:#}");
                PlaceholderLauncher.launch(ctx, flight)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
