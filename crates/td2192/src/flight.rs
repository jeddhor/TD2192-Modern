//! Developer flight arena (M6-08): fly a mission's map with the original flight model.
//!
//! Runs `td_sim::Flight` at the flight tick rate and draws the playfield and HUD bar with
//! `td_render::flight`. Mission ships, weapons, the radar and the HUD gauges are not in yet
//! (their specs are drafts). Keys as in the original: keypad 8/2/4/6 or the arrow keys thrust,
//! keypad + or keypad 0 fire the afterburner (classes 2 and 5), P pauses (any key resumes),
//! Esc leaves.

use std::collections::BTreeMap;

use anyhow::{Result, bail};
use td_assets::DataDir;
use td_flow::{FLIGHT_TICK_HZ, FlowContext, Screen, Transition};
use td_formats::map::TileMap;
use td_formats::palette::Palette;
use td_formats::sprites::SpriteSheet;
use td_formats::tileset::Tileset;
use td_input::{Key, TickInput};
use td_render::flight::{self, HUD_H, HUD_W, ViewPos};
use td_render::{Framebuffer, HeadingSprites};
use td_sim::flight::player::Event;
use td_sim::flight::tables::episode1_tile_attributes;
use td_sim::{Flight, FlightKey, Modal, TickControls, TickResult, World};

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
    hud: Vec<u8>,
    flight: Flight,
    modal: Option<Modal>,
    binds: Vec<(Key, FlightKey)>,
    typematic: Typematic,
    events: Vec<Event>,
}

impl FlightArena {
    pub fn new(dir: &DataDir, number: u8) -> Result<Self> {
        if !(1..=21).contains(&number) {
            bail!("flight number must be 1..=21");
        }
        let base = SpriteSheet::parse_base(&dir.read("td.shp")?)?;
        let rot = [
            SpriteSheet::parse_rotation_set(&dir.read("td.sc3")?)?,
            SpriteSheet::parse_rotation_set(&dir.read("td.shr")?)?,
            SpriteSheet::parse_rotation_set(&dir.read("td.sc2")?)?,
        ];
        let Some(ships) = HeadingSprites::build(&base, [&rot[0], &rot[1], &rot[2]], 9) else {
            bail!("sprite sheets are inconsistent");
        };
        let hud = dir.read("td.win")?;
        if hud.len() != HUD_W * HUD_H {
            bail!("td.win has an unexpected size");
        }
        Ok(Self {
            world: World {
                map: TileMap::parse_episode(&dir.read("td.map")?)?,
                tile_attr: episode1_tile_attributes(),
            },
            tiles: Tileset::parse(&dir.read("td.blk")?)?,
            palette: Palette::parse(&dir.read("td.pal")?)?,
            ships,
            hud,
            flight: Flight::new(number),
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

    fn tick(&mut self, _ctx: &mut FlowContext<'_>, input: &TickInput) -> Transition {
        let c = self.typematic.controls(input, &self.binds);
        self.events.clear();
        let result = match self.modal.take() {
            None => self.flight.tick(&self.world, &c, &mut self.events),
            // The original waits until any key byte is set.
            Some(Modal::Pause) if input.pressed.is_empty() => {
                self.modal = Some(Modal::Pause);
                return Transition::None;
            }
            // The comm viewer and the ship-switch screen are not implemented yet: close them
            // at once.
            Some(_) => self.flight.resume(&self.world, &c, &mut self.events),
        };
        match result {
            TickResult::Done => Transition::None,
            TickResult::Modal(Modal::FlightMenu) | TickResult::Finished => Transition::Quit,
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
        let view = ViewPos {
            tile_x: i32::from(f.view.tile_x),
            tile_y: i32::from(f.view.tile_y),
            sub_x: i32::from(f.view.sub_x),
            sub_y: i32::from(f.view.sub_y),
        };
        let map = &self.world.map;
        flight::draw_tile_window(fb, map, &self.tiles, view);
        let class = usize::from(self.flight.player.class);
        if f.player_visible
            && let Some(frame) = self.ships.frame(class, f.heading.sprite_index())
        {
            flight::draw_player_ship(fb, frame, td_render::headings::CELL);
        }
        if !self.flight.late_draw {
            flight::redraw_overpasses(fb, map, &self.tiles, view);
        }
        flight::draw_hud_bar(fb, &self.hud);
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
            pressed: vec![],
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
