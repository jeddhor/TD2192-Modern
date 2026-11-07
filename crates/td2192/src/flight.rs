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
//! Episode 3 presentation (M10-44.c): the E3 F5 entries (flight 18's point offered from tick
//! 2160), texts and `td3.ccm` transmissions, the overlay tiles of e3-mission-setup rev 1 §3.6,
//! the flight-12 cells drawn as tile 26h until tick 6480 (`Flight::map_tile`), and flight 16's
//! magenta fade and tint (e3-mission-logic rev 1 §4.6.4, §4.7; OB-049 kept): five retraces of
//! fade after the tick in which the tint starts, then the half tint in the DAC until a white
//! flash ends, re-applied to every entry after the Esc menu.
//!
//! The real-time screens of the E1 mission events (flight-screens rev 4 §4.10, BUG-3): flight
//! 17's cut-scene and flight 19's V.C.I. screen run inside the requesting tick
//! (`td_sim::Modal::Scene` after step 18, `td_flow::flight_screens::Interlude`); the tick then
//! goes on with the keys held at the screen's end, the DAC faded in, and, when the tick's back
//! page was page 0, a black playfield for that frame.
//!
//! The muzzle world of a flight holds the two-digit-heading bytes built from the program run's
//! front-end state (weapons rev 5 §3.3, §3.3.2, §3.3.3, OB-017 preserved): what the campaign's
//! screens left, or a direct start's for the developer arena.
//!
//! Mission ships and their target brackets follow `mission-ships.md` §4.7 (with U-127); the
//! afterburner flame, shots and explosions follow `flight-screens.md` rev 1 §4.6–§4.8; the
//! mission-specific sprites (mines, lander, helicopter and its rotor, the slot-0 reload of
//! OB-057) and the heading blits (mirrors, one-pixel shifts, boxes drawn from the sprite's
//! corner; U-360) follow `flight-screens.md` rev 3 §4.9.
//!
//! Presentation around the ticks (M12-14): the flight opens with the original's `FadeIn` from
//! black over a black playfield and the bar as setup drew it, 34 retraces with tick 1 in the
//! last ([`Opening`]; hud rev 2 §4.1, flight-screens rev 5 §4.5.3), and ends with its `FadeOut`
//! ([`Teardown`]); both pace their levels by `td_render::fade::level_frames`.
//!
//! Sound effects (sfx-policy rev 4 §4.2, voc-and-adpcm rev 3 §2.3): the arena keeps the game's
//! side of the single digital voice ([`DigitalVoice`]) on its own clock (flight ticks at
//! `FLIGHT_TICK_HZ`, modal screens and the teardown at the retrace rate): which effects start,
//! the priority reset at tick-order step 26 (where `Flight::sound_reset_at` says it fell among
//! the tick's sounds) and, after the last tick, the flight fade-out followed by the wait for
//! WORX's playing status (mission-logic §4.8), music still playing, before the campaign goes on.
//! Under the polyphonic option every effect in earshot is sent; the model still decides the
//! wait.
//!
//! Enhanced presentation (`docs/spec/enhanced/presentation.md` rev 1): `draw_frame` draws into a
//! framebuffer of any width W ≥ 320, a widescreen frame when W > 320 (`td_render::flight`
//! viewport rules). When the run loop asks for it through the shared [`EnhancedView`], the
//! arena also composes that frame and its light inputs (`lights`) each time it is drawn.

mod lights;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

use anyhow::{Result, bail};
use td_assets::{DataDir, ExeResources, ResourceValue};
use td_audio::{AudioCommand, DigitalVoice, Pcm8, decode_voc};
use td_flow::flight_screens::{
    CommScreen, ComputerMenu, FlightModal, FlightScreenAssets, Interlude, ModalStep, Pause,
    ShipDot, TacticalMap, TargetBoard, draw_ir_notice, draw_pause_line, shows_ir_notice,
};
use td_flow::settings::SfxMode;
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
use td_render::enhanced::widescreen;
use td_render::flight::{self, ShotLook, ViewPos, Viewport};
use td_render::flight_palette::{FADE_STEPS, FlightDac, fade_target, write_fade_step};
use td_render::headings::SpriteBuffers;
use td_render::hud::{Hud, HudAssets, HudState, TickerFont};
use td_render::{Framebuffer, HeadingSprites};
use td_sim::flight::Session;
use td_sim::flight::episode::{self, Episode};
use td_sim::flight::explosions::{FIRST_SPRITE, IDLE};
use td_sim::flight::front_end::{FrontEnd, MenuBytes, direct_start_world, muzzle_world};
use td_sim::flight::heading::Heading;
use td_sim::flight::player::Event;
use td_sim::{ChordKey, Flight, FlightKey, Joystick, Modal, TickControls, TickResult, World};

use crate::enhanced::{EnhancedView, SharedView};

/// Typematic delay and repeat period of an AT keyboard at its power-on defaults (500 ms,
/// 10.9 characters per second). The original's key bytes are re-set by every repeated make
/// code, which re-arms one-shot keys such as the afterburner while a key is held.
const TYPEMATIC_DELAY_US: u64 = 500_000;
const TYPEMATIC_PERIOD_US: u64 = 91_743;

/// Host keys for each original key byte: the `[controls.keyboard]` settings (M11-03), whose
/// defaults are the original's keys (`td_flow::controls`; the grey Insert key is a different
/// byte in the original and is not read in flight). Bound controller buttons arrive as these
/// keys too (`ControlsSettings::pad_keys`, applied by the host).
fn bindings(settings: &td_flow::Settings) -> Vec<(Key, FlightKey)> {
    settings.controls.flight_keys()
}

/// A key byte of the original's table the host feeds: a flight key, or a letter only the
/// cheat chords read (`td_sim::ChordKey`, tick-order step 34, D9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum KeyByte {
    Flight(FlightKey),
    Chord(ChordKey),
}

/// The chord letters with the host keys that set them: the keyboard's own letter keys, except
/// a letter the player bound to a flight key, which then acts only as that key (so a
/// rebinding never fires a chord by accident; with the default bindings every chord letter
/// is free).
fn chord_keys(binds: &[(Key, FlightKey)]) -> Vec<(Key, ChordKey)> {
    ChordKey::ALL
        .into_iter()
        .filter_map(|k| Key::letter(k.letter()).map(|host| (host, k)))
        .filter(|(host, _)| binds.iter().all(|(b, _)| b != host))
        .collect()
}

/// Turns tick-sampled host key state into the original's make codes, with typematic repeats.
#[derive(Debug, Default)]
struct Typematic {
    /// Microseconds each held key has been down.
    held_us: BTreeMap<KeyByte, u64>,
}

impl Typematic {
    fn controls(&mut self, input: &TickInput, binds: &[(Key, FlightKey)]) -> TickControls {
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
            let (down, made) = self.key(input, host, KeyByte::Flight(key));
            if down {
                c.set_down(key);
            }
            if made {
                c.set_made(key);
            }
        }
        for (host, key) in chord_keys(binds) {
            let (down, made) = self.key(input, host, KeyByte::Chord(key));
            if down {
                c.set_chord_down(key);
            }
            if made {
                c.set_chord_made(key);
            }
        }
        c
    }

    /// One key byte: whether `host` is down, and whether a make code (the press or a
    /// typematic repeat) arrived since the previous tick.
    fn key(&mut self, input: &TickInput, host: Key, key: KeyByte) -> (bool, bool) {
        let tick_us = (1e6 / FLIGHT_TICK_HZ) as u64;
        let down = input.held.contains(&host);
        if input.pressed.contains(&host) {
            self.held_us.insert(key, 0);
            return (down, true);
        }
        if !down {
            self.held_us.remove(&key);
            return (false, false);
        }
        let t0 = *self.held_us.get(&key).unwrap_or(&0);
        let t1 = t0 + tick_us;
        self.held_us.insert(key, t1);
        // A repeat fires at DELAY + n·PERIOD; did one fall in (t0, t1]?
        let count = |t: u64| {
            t.checked_sub(TYPEMATIC_DELAY_US)
                .map_or(0, |d| d / TYPEMATIC_PERIOD_US + 1)
        };
        (true, count(t1) > count(t0))
    }
}

#[derive(Debug)]
pub struct FlightArena {
    world: World,
    tiles: Tileset,
    palette: Palette,
    /// Where `ships` and `bank` come from: the sprite bank and rotation buffers after the
    /// mission-specific loads (flight-screens rev 2 §4.9).
    sprites: SpriteSources,
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
    /// E3 flight 16: the magenta fade step on screen (1..=5), while it runs.
    magenta: Option<u8>,
    /// The tick whose back page a real-time screen left cleared (flight-screens §4.10): its
    /// frame shows a black playfield with the instruments.
    blank_page: Option<u32>,
    /// The Enhanced frame shared with the run loop.
    enhanced: Option<SharedView>,
    /// Enhanced lighting: the colours that glow (`lights`).
    emissive: td_render::enhanced::scene::Emissive,
    /// The game's side of the digital voice: `g_SfxPriority` and WORX's playing status.
    voice: DigitalVoice,
    /// Seconds since the flight started: flight ticks and retrace frames as they pass.
    clock: f64,
    /// Before tick 1: the opening fade-in.
    opening: Option<Opening>,
    /// After the last tick: the fade-out and the wait for the playing status.
    teardown: Option<Teardown>,
}

/// The flight's teardown after its last tick (mission-logic §4.8, tick-order §4): `FadeOut`
/// (flight-screens rev 5 §4.5.3: 17 levels, each shown from the frame after its last write,
/// `td_render::fade::first_level_frames`; C-0069 f1809: levels shown 3, 5, …, 33, 34 frames
/// after the last tick's frame), then the wait until WORX reports no digital sound playing
/// (sfx-policy rev 4 §4.2, `0E8B:F990..F99E`), then the campaign goes on (it stops the music).
#[derive(Debug, Clone)]
struct Teardown {
    /// The DAC as shown.
    dac: Palette,
    /// The palette being faded out.
    target: Palette,
    /// The first entry the fade writes (episode-deltas rev 3 §6: E2 1, E1 and E3 0).
    first: usize,
    /// The next fade level to write (16 down to 0); `None` once the fade is over.
    level: Option<u8>,
    /// Frames until that level is shown.
    wait: u32,
}

impl Teardown {
    /// Starts the fade-out from `dac` (what is on screen) towards black: `target` is the flight
    /// palette in the current mode. After a Quit the screen is already black (the menu returns
    /// without fading back in), so only the wait remains.
    fn new(dac: &Palette, target: Palette, first: usize, aborted: bool) -> Self {
        if aborted {
            return Self {
                dac: Palette::default(),
                target,
                first,
                level: None,
                wait: 0,
            };
        }
        Self {
            dac: dac.clone(),
            wait: td_render::fade::first_level_frames(&target, first, FADE_STEPS),
            target,
            first,
            level: Some(FADE_STEPS),
        }
    }

    /// One retrace frame of the fade; true once it is over.
    fn step(&mut self) -> bool {
        let Some(s) = self.level else {
            return true;
        };
        self.wait = self.wait.saturating_sub(1);
        if self.wait == 0 {
            write_fade_step(&mut self.dac, &self.target, s, self.first);
            self.level = s.checked_sub(1);
            if let Some(next) = self.level {
                self.wait = td_render::fade::level_frames(&self.target, self.first, next);
            }
        }
        false
    }
}

/// The flight's opening `FadeIn` (hud rev 2 §4.1 step 6, tick-order §4 setup, flight-screens
/// rev 5 §4.5.3): before tick 1 the screen shows a black playfield (page 1, cleared) and the
/// HUD band as setup drew it ([`Hud::start`]), while DAC entries `first..=255` rise from black
/// to the flight palette in the start mode over 17 levels. Level k is shown 2k + 2 frames after
/// the flight opens (`td_render::fade::first_level_frames`), level 16 in frame 34: C-0002
/// f3439..3472 and C-0017 f458..491 with tick 1 in the next frame, E2 C-0015 f631..664, E3
/// C-0036 f660..693. Tick 1 runs in the frame of level 16: it draws into the page on screen, so
/// its playfield appears with the last level (C-0002 f3473). In infra-red mode 1 (E1 flights 4
/// and 8, E2 3 and 10) the start formula then replaces the fade's (B / 4, entry 37), which
/// takes the next frame (C-0019 f490..491), and tick 1 runs then. The key table is cleared and
/// the keyboard handler hooked before the fade, so keys pressed during it reach tick 1.
#[derive(Debug, Clone)]
struct Opening {
    /// The DAC as shown.
    dac: Palette,
    /// `g_FlightPalette` in the start mode (the fade's target).
    target: Palette,
    /// The first entry the fade writes (episode-deltas rev 3 §6: E3 0, E1 and E2 1).
    first: usize,
    /// The next level to write; `None` after level 16.
    level: Option<u8>,
    /// Frames until that level is shown (then until the infra-red palette is).
    wait: u32,
    /// Mode 1 at the start, in an episode that applies the start formula after the fade.
    infra_red: bool,
    /// Keys pressed while the fade runs.
    pressed: Vec<Key>,
}

impl Opening {
    fn new(target: Palette, first: usize, infra_red: bool) -> Self {
        Self {
            dac: Palette::default(),
            wait: td_render::fade::first_level_frames(&target, first, 0),
            target,
            first,
            level: Some(0),
            infra_red,
            pressed: Vec::new(),
        }
    }

    /// One retrace frame; true once tick 1 is due in this frame.
    fn step(&mut self, input: &TickInput) -> bool {
        self.pressed.extend(input.pressed.iter().copied());
        self.wait = self.wait.saturating_sub(1);
        if self.wait > 0 {
            return false;
        }
        let Some(s) = self.level else {
            // The infra-red start formula is on screen (the arena's `FlightDac`).
            return true;
        };
        write_fade_step(&mut self.dac, &self.target, s, self.first);
        if s < FADE_STEPS {
            self.level = Some(s + 1);
            self.wait = td_render::fade::level_frames(&self.target, self.first, s + 1);
            return false;
        }
        self.level = None;
        self.wait = OPENING_IR_FRAMES;
        !self.infra_red
    }
}

/// Frames from the opening fade's last level to the infra-red start palette (mode 1): C-0019
/// (E1 flight 4) writes the formula's 257 entries from f490.2 to f491.9, right after the last
/// level, so it is first shown one frame after that level. T2.
const OPENING_IR_FRAMES: u32 = 1;

/// WORX's ring offset r in `episode`'s program as Classic runs it (voc-and-adpcm rev 3 §2.3:
/// E1 started by `td1.exe`, E2 and E3 run directly).
fn worx_ring(episode: Episode) -> u16 {
    match episode {
        Episode::E1 => td_audio::worx::RING_E1,
        Episode::E2 => td_audio::worx::RING_E2,
        Episode::E3 => td_audio::worx::RING_E3,
    }
}

/// The flight's sprite data as `Sim_FlyMission` leaves it (flight-screens rev 2 §4.9): the
/// 20-slot `td.shp` part of the sprite bank, the rotation buffers `td.sc3`, `td.shr`,
/// `td.sc2`, `td.exp` (bank slots 20..26) and the stock `td.shp` record 0 that the ship
/// library and the comm screen reload into slot 0.
#[derive(Debug)]
struct SpriteSources {
    shp: Vec<u8>,
    rot: [Vec<u8>; 3],
    exp: Vec<u8>,
    stock_record0: Vec<u8>,
}

/// One 32 × 32 sprite record.
const RECORD: usize = 1024;

impl SpriteSources {
    /// Reads the files fresh and applies the mission-specific loads of flight `number`
    /// (flight-screens rev 2 §4.9; E2 flight 8: e2-mission-setup rev 1 §4.1 step 6):
    ///
    /// - E1 flight 7: `mine.td` into bank + C00h (slot 3);
    /// - E1 flight 20: `lander.td` records 0, 1 into bank + 0 (slots 0, 1) and records 2..4
    ///   into bank + 1000h (slots 4..6);
    /// - E1 flights 12, 15, 21 and E2 flight 8: `td.hel` into bank + 800h (slots 2, 3) and
    ///   `td.h2`'s three 3,200-byte parts into + C80h (cells 2, 3) of `td.sc2`, `td.shr` and
    ///   `td.sc3`, in that order.
    fn load(dir: &DataDir, ep: Episode, number: u8) -> Result<Self> {
        let mut shp = dir.read("td.shp")?;
        let stock_record0 = shp.get(..RECORD).unwrap_or_default().to_vec();
        let (mut sc3, mut shr, mut sc2) = (
            dir.read("td.sc3")?,
            dir.read("td.shr")?,
            dir.read("td.sc2")?,
        );
        match (ep, number) {
            (Episode::E1, 7) => overwrite(&mut shp, 3 * RECORD, &dir.read("mine.td")?)?,
            (Episode::E1, 20) => {
                let lander = dir.read("lander.td")?;
                overwrite(&mut shp, 0, lander.get(..2 * RECORD).unwrap_or_default())?;
                let rest = lander.get(2 * RECORD..5 * RECORD).unwrap_or_default();
                overwrite(&mut shp, 4 * RECORD, rest)?;
            }
            (Episode::E1, 12 | 15 | 21) | (Episode::E2, 8) => {
                overwrite(&mut shp, 2 * RECORD, &dir.read("td.hel")?)?;
                let h2 = dir.read("td.h2")?;
                for (k, set) in [&mut sc2, &mut shr, &mut sc3].into_iter().enumerate() {
                    let part = h2.get(k * 0xC80..(k + 1) * 0xC80);
                    overwrite(set, 0xC80, part.unwrap_or_default())?;
                }
            }
            _ => {}
        }
        Ok(Self {
            shp,
            rot: [sc3, shr, sc2],
            exp: dir.read("td.exp")?,
            stock_record0,
        })
    }

    /// Every ship sprite at every heading, and the 27-slot bank. Headings N, E, S, W draw the
    /// bank slot, the others the rotation cells, each with its blit's transform, from the
    /// buffers as the flight lays them out in memory (flight-screens rev 3 §4.9,
    /// `td_render::headings`), so a file that replaces only bank slots changes those four
    /// headings only.
    fn build(&self) -> Result<(HeadingSprites, SpriteSheet)> {
        SpriteSheet::parse_base(&self.shp)?;
        for set in &self.rot {
            SpriteSheet::parse_rotation_set(set)?;
        }
        let mut bank_bytes = self.shp.clone();
        bank_bytes.extend_from_slice(&self.exp);
        let [sc3, shr, sc2] = &self.rot;
        let buffers = SpriteBuffers::flight(sc3, sc2, shr, &bank_bytes);
        let ships = HeadingSprites::from_buffers(&buffers, 20);
        Ok((ships, SpriteSheet::parse(&bank_bytes, 32)?))
    }

    /// The ship library and the comm screen re-read `td.shp` record 0 into bank slot 0
    /// (flight-screens rev 2 §4.9). True if that changed the slot: in E1 flight 20 it replaces
    /// lander piece 0 for the rest of the flight (OB-057, preserved).
    fn reload_record0(&mut self) -> bool {
        let slot0 = &mut self.shp[..RECORD];
        if slot0 == self.stock_record0.as_slice() {
            return false;
        }
        slot0.copy_from_slice(&self.stock_record0);
        true
    }
}

/// `g_PlayerSprite` (flight-screens rev 2 §4.9, tick-order step 6): the player's class, but
/// while the player is drawn late sprite 3 if the back page at step 6 is 0, else sprite 2.
/// In the helicopter flights (E1 12, 15, 21; E2 8) these are the two `td.hel` bodies, so the
/// rotor alternates every tick.
fn player_sprite(class: u8, late: bool, page: i32) -> usize {
    match (late, page) {
        (false, _) => usize::from(class),
        (true, 0) => 3,
        (true, _) => 2,
    }
}

/// What one program run keeps across its flights.
#[derive(Debug, Default)]
pub struct RunState {
    /// RNG, class tables, ship records, mission state and `g_Weapon`
    /// (`td_sim::flight::Session`).
    pub session: Session,
    /// The F5 target entries (`Msn_LoadTargetInfo` rewrites only part of them per flight).
    pub board: TargetBoard,
    /// The front-end state the campaign handed over before the flight (weapons rev 5 §3.3,
    /// §3.3.2, §3.3.3).
    pub front_end: FrontEnd,
}

impl FlightArena {
    /// A flight of `episode` as the first of a program run (developer arena), Classic.
    pub fn new(
        dir: &DataDir,
        resources: &ExeResources,
        episode: Episode,
        number: u8,
    ) -> Result<Self> {
        Self::open(dir, resources, episode, number, None, false)
    }

    /// Flight `number` of the program run `run` (a fresh Episode 1 run when `None`); the
    /// episode is the run's. `fixes`: the Enhanced profile's original-bug fixes as the flight
    /// starts (`Settings::enhanced_fixes`), for the radar table (OB-024); the fixes applied
    /// per tick follow the settings live.
    pub fn start(
        dir: &DataDir,
        resources: &ExeResources,
        number: u8,
        run: Option<Rc<RefCell<RunState>>>,
        fixes: bool,
    ) -> Result<Self> {
        let ep = run
            .as_ref()
            .map_or(Episode::E1, |r| r.borrow().session.episode);
        Self::open(dir, resources, ep, number, run, fixes)
    }

    fn open(
        dir: &DataDir,
        resources: &ExeResources,
        ep: Episode,
        number: u8,
        run: Option<Rc<RefCell<RunState>>>,
        fixes: bool,
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
        // The muzzle table is byte-identical in every episode (episode-deltas §11). The bytes
        // the two-digit headings read past it come from the run's front-end state, or a direct
        // start's in the developer arena (weapons rev 5 §3.3, §3.3.2, §3.3.3; OB-017).
        let Some(ResourceValue::Bytes(table)) = resources.get("flight.muzzle_offsets").cloned()
        else {
            bail!("file0001.exe resources are unavailable (muzzle table)");
        };
        let muzzles = match &run {
            Some(r) => {
                let menu = MenuBytes::load(|f| dir.read(f))?;
                muzzle_world(ep, &table, &menu, &r.borrow().front_end)
            }
            None => direct_start_world(ep, number, &table, |f| dir.read(f))?,
        };
        // E1 flight 21 loads no mission file and flies the ship records it inherits.
        let mission = match episode::mission_file(ep, number) {
            None => None,
            Some(file) => Some(Mission::parse(&dir.read(&file)?)?),
        };
        let sprites = SpriteSources::load(dir, ep, number)?;
        let (ships, bank) = sprites.build()?;
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
        let mut session = run
            .as_ref()
            .map_or_else(|| Session::for_episode(ep), |r| r.borrow().session.clone());
        // What the front end did on the flight heap since the last flight goes over the path
        // arrays (mission-ships rev 6 §4.1.1, U-500): E1 flight 21 loads no mission file and
        // places its ships from them. The uses belong to this flight only.
        if let Some(r) = &run {
            let uses = std::mem::take(&mut r.borrow_mut().front_end.heap);
            Arc::make_mut(&mut session.paths).apply(&uses, |f| dir.read(f))?;
        }
        let mut flight = Flight::start(number, mission.as_ref(), &world, session);
        // The host shows the mission events' real-time screens inside the tick.
        flight.stop_for_scenes = true;
        flight.enhanced_fixes = fixes;
        // E2: the map-set-2 colours replace the table for the rest of the run (OB-024,
        // preserved in Classic); the Enhanced fixes restore it for map set 1.
        let radar_set2 = radar_set2_shown(ep, number, flight.radar_set2, fixes);
        let radar_id = match (ep, radar_set2) {
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
        // Setup draws the weapon icon and the missile column before the fade-in (hud §4.1).
        let mut hud = Hud::new(&hud_assets);
        hud.start(
            &hud_assets,
            flight.weapon,
            flight.missiles,
            flight.missile_cooldown,
        );
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
        let screens = FlightScreenAssets::load_episode(dir, resources, ep, radar_set2)?;
        let board = {
            let mut fresh = TargetBoard::default();
            let mut shared = run.as_ref().map(|r| r.borrow_mut());
            let board = shared.as_mut().map_or(&mut fresh, |r| &mut r.board);
            board.load_episode(ep, number);
            board.clone()
        };
        let palette = Palette::parse(&dir.read("td.pal")?)?;
        let emissive = lights::emissive_of(&bank);
        // The opening FadeIn of the start mode: E3 writes entry 0 too and never starts in
        // infra-red; E1 and E2 apply the start formula after the fade in mode 1.
        let opening = Opening::new(
            fade_target(&flight_palette_of(&palette, &hud), flight.palette_mode),
            usize::from(ep != Episode::E3),
            flight.palette_mode == 1 && ep != Episode::E3,
        );
        Ok(Self {
            world,
            tiles: Tileset::parse(&dir.read(episode::tileset_file(ep, number))?)?,
            dac: FlightDac::start(&palette, flight.palette_mode),
            palette,
            sprites,
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
            binds: bindings(&td_flow::Settings::default()),
            typematic: Typematic::default(),
            events: Vec::new(),
            magenta: None,
            blank_page: None,
            enhanced: None,
            emissive,
            voice: DigitalVoice::new(worx_ring(ep)),
            clock: 0.0,
            opening: Some(opening),
            teardown: None,
        })
    }

    /// Composes Enhanced frames into `view` when the run loop asks for them.
    #[must_use]
    pub fn with_enhanced(mut self, view: Option<SharedView>) -> Self {
        self.enhanced = view;
        self
    }
}

impl Screen for FlightArena {
    fn tick_hz(&self) -> f64 {
        // The opening fade-in, the modal screens, E3's magenta fade and the teardown step once
        // per vertical retrace (fades, pulses, typing, the wait for the sound).
        if self.opening.is_some()
            || self.screen.is_some()
            || self.magenta.is_some()
            || self.teardown.is_some()
        {
            RETRACE_HZ
        } else {
            FLIGHT_TICK_HZ
        }
    }

    fn tick(&mut self, ctx: &mut FlowContext<'_>, input: &TickInput) -> Transition {
        // The bindings may change in the options screen at any time (M11-03), and so may the
        // profile: the Enhanced original-bug fixes apply from the next tick.
        self.binds = bindings(ctx.settings);
        let fixes = ctx.settings.enhanced_fixes();
        self.flight.enhanced_fixes = fixes;
        // The original reads the joystick only when it is the chosen controller; the port
        // uses it whenever one is connected.
        self.flight.joystick_enabled = input.pad.is_some();
        self.events.clear();
        // This host tick lasts one period of the rate it runs at.
        let before = self.clock;
        self.clock += 1.0 / self.tick_hz();
        let now = self.clock;
        // The opening fade-in: no tick until its last level (or the infra-red palette) is on
        // screen; tick 1 then runs in that frame with the keys pressed during the fade.
        let mut first_input = None;
        if let Some(o) = &mut self.opening {
            if !o.step(input) {
                return Transition::None;
            }
            first_input = Some(TickInput {
                pressed: std::mem::take(&mut o.pressed),
                ..input.clone()
            });
            self.opening = None;
        }
        let input = first_input.as_ref().unwrap_or(input);
        if let Some(t) = &mut self.teardown {
            // mission-logic §4.8: fade out, then wait while WORX reports a sound playing.
            if t.step() && !self.voice.playing(now) {
                return Transition::Pop;
            }
            return Transition::None;
        }
        if let Some(step) = self.magenta {
            // E3 flight 16 (§4.6.4): one fade step per retrace; no tick runs meanwhile.
            self.dac.magenta_step(&self.palette, step);
            self.magenta = (step < FlightDac::MAGENTA_STEPS).then_some(step + 1);
            return Transition::None;
        }
        let tinted = self.flight.mission.tint;
        let mode = self.flight.palette_mode;
        let mut result = if let Some(screen) = &mut self.screen {
            match screen.tick(&self.screens, input) {
                ModalStep::Continue => return Transition::None,
                ModalStep::Close { abort } => {
                    let menu = matches!(screen, FlightModal::Menu(_));
                    let scene = matches!(screen, FlightModal::Interlude(_));
                    let record0 = match screen {
                        FlightModal::Menu(m) => m.library_shown(),
                        FlightModal::Comm(_) => true,
                        _ => false,
                    };
                    self.screen = None;
                    self.modal = None;
                    if record0 {
                        self.record0_reloaded(fixes);
                    }
                    if abort {
                        // Quit: g_MissionAborted := 1; the tick then leaves the loop.
                        self.flight.aborted = true;
                    } else if menu {
                        // The menu's last FadeIn restored the mode (flight-screens §4.1); every
                        // fade level zeroed g_HitFlash (hit-flash.md §4.2).
                        if self.flight.episode == Episode::E3 {
                            // E3's FadeIn includes entry 0; flight 16 re-tints every entry
                            // after it (e3-mission-logic §4.7.2, OB-049).
                            self.dac
                                .faded_in_from_0(&self.palette, self.flight.palette_mode);
                            if self.flight.mission.tint {
                                self.dac.magenta_tint_all(&self.palette);
                            }
                        } else {
                            self.dac.faded_in(&self.palette, self.flight.palette_mode);
                        }
                        self.flight.hit_flash = 0;
                    }
                    let r = if scene {
                        // The screen's closing FadeIn (mode 0 in flights 17 and 19) ended any hit
                        // flash; its key changes reach the tick's input steps (§4.10). The page
                        // it cleared is this tick's back page when that is page 0.
                        self.dac.faded_in(&self.palette, self.flight.palette_mode);
                        self.flight.hit_flash = 0;
                        if self.flight.draw_page == 0 {
                            self.blank_page = Some(self.flight.ticks);
                        }
                        let c = held_controls(input, &self.binds);
                        self.flight.resume(&self.world, &c, &mut self.events)
                    } else {
                        let c = self.typematic.controls(input, &self.binds);
                        self.flight.resume(&self.world, &c, &mut self.events)
                    };
                    // The rest of the tick runs as the screen closes.
                    self.issue_sounds(ctx, 0, now, now);
                    r
                }
            }
        } else {
            let c = self.typematic.controls(input, &self.binds);
            if cfg!(debug_assertions) && input.pressed(Key(0x42)) {
                // F9, debug builds only: complete the mission.
                self.flight.succeeded = true;
            }
            let r = self.flight.tick(&self.world, &c, &mut self.events);
            // Steps 1..19 ran right after the previous tick's end; step 26 and on at this one's.
            self.issue_sounds(ctx, 0, before, now);
            // 14. Palette effects, in the mode the step saw (Alt acts later, at step 33), from
            // the hit-flash level the step found (hit-flash.md §4.3).
            self.dac.set_hit_flash(self.flight.frame.hit_flash);
            // OB-032: the Enhanced fixes restore mode 2 after a white flash.
            self.dac
                .step(&self.palette, mode, self.flight.frame.white_flash, fixes);
            r
        };
        while let TickResult::Modal(m) = result {
            self.modal = Some(m);
            if let Some(screen) = self.open_screen(m) {
                self.screen = Some(screen);
                break;
            }
            // Nothing to show: continue the tick at once (a scene keeps the key bytes).
            self.modal = None;
            let c = if m == Modal::Scene {
                held_controls(input, &self.binds)
            } else {
                TickControls::default()
            };
            let start = self.events.len();
            result = self.flight.resume(&self.world, &c, &mut self.events);
            self.issue_sounds(ctx, start, now, now);
        }
        // 33. Alt toggled infra-red: the start formula of the new mode. E2 back into mode 1
        // rewrites only DAC 255 and 37 (OB-023, BUG-9; preserved in Classic, fixed by the
        // Enhanced fixes).
        if self.flight.palette_mode != mode {
            if self.flight.episode == Episode::E2 && !fixes {
                self.dac
                    .toggle_e2_classic(&self.palette, self.flight.palette_mode);
            } else {
                self.dac.toggle(&self.palette, self.flight.palette_mode);
            }
        }
        // E3 flight 16: the event routine (after step 24) started the tint in this tick.
        if !tinted && self.flight.mission.tint && self.screen.is_none() {
            self.magenta = Some(1);
        }
        match result {
            TickResult::Done | TickResult::Modal(_) => Transition::None,
            TickResult::Finished => {
                // Hand the session to the next flight of the program run.
                if let Some(run) = &self.run {
                    run.borrow_mut().session = self.flight.session();
                }
                // Teardown (mission-logic §4.8): fade out, wait for the sound, then pop. The
                // fade reads g_FlightPalette (the lamp entries as the instruments stored them).
                let target = fade_target(&self.flight_palette(), self.flight.palette_mode);
                self.teardown = Some(Teardown::new(
                    self.dac.current(),
                    target,
                    usize::from(self.flight.episode == Episode::E2),
                    self.flight.aborted,
                ));
                Transition::None
            }
        }
    }

    fn draw(&self, fb: &mut Framebuffer) {
        self.draw_frame(fb);
        self.compose_enhanced();
    }
}

impl FlightArena {
    /// After the ship library or the comm screen: both re-read `td.shp` record 0 into bank
    /// slot 0 (flight-screens rev 2 §4.9), which in E1 flight 20 replaces lander piece 0 for
    /// the rest of the flight (OB-057, preserved in Classic). With the Enhanced fixes the
    /// flight keeps its own slot 0.
    fn record0_reloaded(&mut self, fixes: bool) {
        if fixes || !self.sprites.reload_record0() {
            return;
        }
        match self.sprites.build() {
            Ok((ships, bank)) => (self.ships, self.bank) = (ships, bank),
            Err(e) => tracing::warn!("sprite slot 0 reload: {e}"),
        }
    }

    /// `Snd_PlayEffect` for the sounds the last `tick`/`resume` call requested,
    /// `self.events[start..]`: those before tick-order step 26 at time `before`, then the
    /// priority reset (step 26: idle again when WORX reports no sound playing), then the rest at
    /// `now` (sfx-policy rev 4 §4.2). Effects the arbiter lets start go to the mixer.
    fn issue_sounds(&mut self, ctx: &mut FlowContext<'_>, start: usize, before: f64, now: f64) {
        let reset = self.flight.sound_reset_at.filter(|&i| i >= start);
        let events: Vec<Event> = self.events.get(start..).unwrap_or_default().to_vec();
        for (i, &Event::Sound { kind, source }) in (start..).zip(&events) {
            if reset == Some(i) {
                self.voice.priority_reset(now);
            }
            let at = if reset.is_some_and(|r| i >= r) {
                now
            } else {
                before
            };
            self.play_sound(ctx, kind, source, at);
        }
        if reset == Some(start + events.len()) {
            self.voice.priority_reset(now);
        }
    }

    /// One `Snd_PlayEffect(kind, source, view)` at time `at` on a Sound Blaster.
    fn play_sound(
        &mut self,
        ctx: &mut FlowContext<'_>,
        kind: u8,
        source: Option<(u8, u8)>,
        at: f64,
    ) {
        let v = self.flight.player.view;
        let view = (v.tile_x as u8, v.tile_y as u8);
        // OB-009: a distance the original cannot compute is "too far"; nothing happens.
        let Some(dist) = td_audio::sfx::distance(source, view) else {
            return;
        };
        let sound = self.effects.get(&kind).cloned();
        let starts = self.voice.request(kind, dist, sound.as_deref(), at);
        let polyphonic = ctx.settings.effective_sfx_mode() == SfxMode::Polyphonic;
        if starts || (polyphonic && sound.is_some()) {
            ctx.audio.push(AudioCommand::PlayEffect {
                id: kind,
                sound,
                source,
                view,
            });
        }
    }

    /// Composes the Enhanced frame when the run loop asked for one (module docs). The pause
    /// is a text overlay, so it is drawn over the wide frame like the infra-red notice (spec
    /// enhanced/presentation rev 4 §4.3); the other modal screens keep their Classic picture,
    /// which the pipeline centres.
    fn compose_enhanced(&self) {
        let Some(view) = &self.enhanced else {
            return;
        };
        // The opening fade-in keeps its Classic picture (a black playfield).
        if self.opening.is_some() {
            return;
        }
        let paused = match &self.screen {
            None => false,
            Some(FlightModal::Pause(_)) => true,
            Some(_) => return,
        };
        // A draw from inside `compose` (none today) would find the view borrowed: skip it.
        let Ok(mut view) = view.try_borrow_mut() else {
            return;
        };
        let view: &mut EnhancedView = &mut view;
        view.compose(
            |fb| {
                self.draw_play(fb);
                if paused {
                    widescreen::overlay_centred(fb, |fb| draw_pause_line(fb, &self.screens));
                }
            },
            |margin, mode| self.light_scene(margin, mode),
            self.emissive(),
            |margin, width| {
                let display = self.hud.borrow();
                td_render::enhanced::guidance::marks(&display.hud, margin, width)
            },
        );
    }

    /// The picture: the modal screen, or the flight frame in `fb`'s width (Classic at 320,
    /// widescreen when wider).
    fn draw_frame(&self, fb: &mut Framebuffer) {
        if let Some(screen) = &self.screen {
            screen.draw(fb);
            return;
        }
        self.draw_play(fb);
    }

    /// The flight frame in `fb`'s width (Classic at 320, widescreen when wider). During the
    /// pause it is the frame the pause was drawn over: the tick is suspended, so the state and
    /// the instruments are those of that frame.
    fn draw_play(&self, fb: &mut Framebuffer) {
        let vp = Viewport::of(fb);
        fb.clear(0);
        if let Some(o) = &self.opening {
            // Before tick 1: page 1 is clear and the bar shows what setup drew; the DAC is the
            // fade's alone.
            widescreen::draw_hud(fb, &self.hud.borrow().hud);
            fb.palette = o.dac.clone();
            return;
        }
        fb.palette = match &self.teardown {
            Some(t) => t.dac.clone(),
            None => self.dac.current().clone(),
        };
        let f = &self.flight.frame;
        let view = ViewPos {
            tile_x: i32::from(f.view.tile_x),
            tile_y: i32::from(f.view.tile_y),
            sub_x: i32::from(f.view.sub_x),
            sub_y: i32::from(f.view.sub_y),
        };
        let patched = self.drawn_map();
        let map = patched.as_ref().unwrap_or(&self.world.map);
        // After a real-time screen that cleared this tick's back page only the instruments
        // and the overlays are drawn into it (flight-screens §4.10).
        if self.blank_page != Some(self.flight.ticks) {
            self.draw_world(fb, map, view, vp);
        }
        self.draw_instruments(fb, map);
        self.draw_overlays(fb);
    }

    /// Steps 4..16 of the frame: tiles, flame, shots, ships, overpasses, explosions.
    fn draw_world(&self, fb: &mut Framebuffer, map: &TileMap, view: ViewPos, vp: Viewport) {
        let f = &self.flight.frame;
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
            let look = flight::shot_look_in(
                view,
                s.kind,
                s.heading,
                drawn_shot_tile(s),
                (s.off_x, s.off_y),
                vp,
            );
            match look {
                Some(ShotLook::Pixels(pixels)) => {
                    for (x, y, c) in pixels {
                        flight::put_playfield(fb, x, y, c);
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
        let late = self.flight.late_draw;
        let class = player_sprite(self.flight.player.class, late, self.drawn_page());
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
                Episode::E3 => {
                    flight::redraw_overlays_e3(fb, map, &self.tiles, view, self.flight.number);
                }
                Episode::E1 => flight::redraw_overpasses(fb, map, &self.tiles, view),
            }
        }
        // 13. Explosions: bank slot 19 + frame (flight-screens §4.7).
        let page = self.drawn_page();
        for e in drawn.explosions.0.iter().filter(|e| e.frame < IDLE) {
            let (tile, off) = ((e.tile_x, e.tile_y), (e.sub_x, e.sub_y));
            let at = flight::explosion_sprite_in(view, page, tile, off, vp);
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
    }

    /// Step 19: the guidance dots go into the playfield, the rest below it.
    fn draw_instruments(&self, fb: &mut Framebuffer, map: &TileMap) {
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
            display.hud.step(&self.hud_assets, map, &state);
            display.tick = Some(f.ticks);
        }
        widescreen::draw_hud(fb, &display.hud);
    }

    /// `g_FlightPalette`: `td.pal` with the entries the instruments store into it (the
    /// compass, missile and low-armour lamps, hud §4.3–§4.5), which the flight fades read.
    fn flight_palette(&self) -> Palette {
        flight_palette_of(&self.palette, &self.hud.borrow().hud)
    }
}

/// `g_FlightPalette` from `td.pal` and the entries `hud` stored last (hud §4.1 step 5, §4.3–§4.5).
fn flight_palette_of(palette: &Palette, hud: &Hud) -> Palette {
    let mut p = palette.clone();
    for &(e, [r, g, b]) in hud.entries() {
        p.colors[usize::from(e)] = td_formats::palette::Rgb6 {
            r: r & 63,
            g: g & 63,
            b: b & 63,
        };
    }
    p
}

/// The key bytes after a real-time screen of the mission events (flight-screens §4.10): the
/// INT 09h handler kept running, so a key released meanwhile has cleared its byte and a key
/// still down has it set (pressed meanwhile, or held for more than the 500 ms typematic delay,
/// whose repeats re-set a byte the game had cleared; both screens last longer).
fn held_controls(input: &TickInput, binds: &[(Key, FlightKey)]) -> TickControls {
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
        if input.held.contains(&host) {
            c.set_down(key);
            c.set_made(key);
        }
    }
    for (host, key) in chord_keys(binds) {
        if input.held.contains(&host) {
            c.set_chord_down(key);
            c.set_chord_made(key);
        }
    }
    c
}

/// Whether the radar shows the map-set-2 colours (`k_TileRadarColour`, e2-mission-setup rev 1
/// §3.1; episode-deltas §4.3). E2 copies them over the table in a set-2 flight and never
/// restores it, so the session's flag stays set (OB-024, preserved in Classic). With the
/// Enhanced fixes the table is restored: the flight's own map set decides. E1 and E3 never
/// copy it.
fn radar_set2_shown(ep: Episode, number: u8, session_set2: bool, fixes: bool) -> bool {
    if fixes {
        ep == Episode::E2 && episode::map_set(ep, number) == 2
    } else {
        session_set2
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
    /// The map as the flight shows it when it differs from the loaded one: E3 flight 12's
    /// cells are tile 26h until tick 6480 (`Flight::map_tile`, e3-mission-logic §4.6.3).
    fn drawn_map(&self) -> Option<TileMap> {
        if !self.flight.cells_off {
            return None;
        }
        let map = &self.world.map;
        let size = map.size();
        let mut cells = map.cells().to_vec();
        for &(x, y, _) in &td_sim::flight::setup::e3::FLIGHT12_CELLS {
            let (x, y) = (usize::from(x), usize::from(y));
            if let Some(t) = self.flight.map_tile(&self.world, x, y) {
                cells[x * size + y] = t;
            }
        }
        TileMap::parse_episode(&cells).ok()
    }

    /// The back page the last tick drew into: the tick toggles it at step 24, unless it is
    /// suspended by a modal screen before that step.
    fn drawn_page(&self) -> i32 {
        let page = i32::from(self.flight.draw_page);
        match self.modal {
            Some(Modal::CommViewer | Modal::Pause | Modal::Scene) => page,
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
            let Some(pos) = flight::mission_ship_pos_in(view, page, &mark, Viewport::of(fb)) else {
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
            widescreen::overlay_centred(fb, |fb| draw_ir_notice(fb, &self.screens));
        }
    }

    /// The screen for modal `m`, over the picture on screen now; `None` if there is nothing
    /// to show.
    fn open_screen(&self, m: Modal) -> Option<FlightModal> {
        let mut shown = Framebuffer::new();
        self.draw_frame(&mut shown);
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
            // The mission events asked for a real-time screen at step 18 (§4.10).
            Modal::Scene => FlightModal::Interlude(Box::new(Interlude::new(
                f.mission.scene,
                shown,
                &self.flight_palette(),
                f.palette_mode,
                &self.screens,
            )?)),
            Modal::TacticalMap => {
                let dots: Vec<ShipDot> = f
                    .ships
                    .0
                    .iter()
                    .map(|s| ShipDot {
                        class: s.class,
                        tile: (s.tile_x, s.tile_y),
                        // E2/E3 show every live ship, marked or not (episode-deltas §6).
                        marked: s.marker != 0 || f.episode != Episode::E1,
                    })
                    .collect();
                let mut board = self.board.clone();
                if f.episode == Episode::E3 && f.number == 18 {
                    // DS:1EAD, set at tick 2160 (e3-mission-logic §4.9).
                    board.flags[21] = f.e3_point_offered();
                }
                let v = f.player.view;
                let map = self.drawn_map();
                FlightModal::Map(Box::new(TacticalMap::new(
                    &shown.palette,
                    &self.screens,
                    map.as_ref().unwrap_or(&self.world.map),
                    (i32::from(v.tile_x), i32::from(v.tile_y)),
                    &dots,
                    &board,
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
    /// The Enhanced frame shared with the run loop.
    enhanced: Option<SharedView>,
}

impl ArenaLauncher {
    /// The flights of `episode`'s program run (program start of that executable).
    pub fn for_episode(episode: Episode) -> Self {
        let run = RunState {
            session: Session::for_episode(episode),
            board: TargetBoard::default(),
            front_end: FrontEnd::launcher(),
        };
        Self {
            run: Rc::new(RefCell::new(run)),
            fallback: PlaceholderLauncher::default(),
            enhanced: None,
        }
    }

    /// Flights compose Enhanced frames into `view` (see [`FlightArena::with_enhanced`]).
    #[must_use]
    pub fn with_enhanced(mut self, view: SharedView) -> Self {
        self.enhanced = Some(view);
        self
    }
}

impl td_flow::FlightLauncher for ArenaLauncher {
    fn set_front_end(&mut self, state: &FrontEnd) {
        self.run.borrow_mut().front_end = state.clone();
    }

    fn launch(&mut self, ctx: &mut FlowContext<'_>, flight: u8) -> Box<dyn FlightScreen> {
        let fixes = ctx.settings.enhanced_fixes();
        match FlightArena::start(
            ctx.data,
            ctx.resources,
            flight,
            Some(self.run.clone()),
            fixes,
        ) {
            Ok(arena) => Box::new(arena.with_enhanced(self.enhanced.clone())),
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
    use td_render::flight::SHIP_POS;
    use td_test_support::{require_data, require_oracle};

    #[test]
    fn player_sprite_follows_the_page_only_when_drawn_late() {
        // flight-screens rev 2 §4.9: 3 on page 0, 2 on page 1; otherwise the class.
        assert_eq!(player_sprite(3, true, 0), 3);
        assert_eq!(player_sprite(3, true, 1), 2);
        assert_eq!(player_sprite(4, false, 0), 4);
        assert_eq!(player_sprite(4, false, 1), 4);
    }

    #[test]
    fn mission_sprites_land_where_flight_screens_says() {
        // flight-screens rev 2 §4.9 loads; a bank-only file changes headings N, E, S, W only.
        let data = require_data!();
        let dir = DataDir::open(&data).unwrap();
        let read = |name: &str| dir.read(name).unwrap();
        let stock = SpriteSources::load(&dir, Episode::E1, 1).unwrap();
        assert_eq!(stock.shp, read("td.shp"));
        let slot = |s: &SpriteSources, n: usize| s.shp[n * RECORD..(n + 1) * RECORD].to_vec();
        // Flight 7: the mine in slot 3; the rotation cells stay stock.
        let mines = SpriteSources::load(&dir, Episode::E1, 7).unwrap();
        assert_eq!(slot(&mines, 3), read("mine.td"));
        assert_eq!(mines.rot, stock.rot);
        let (ships, _) = mines.build().unwrap();
        let (stock_ships, _) = stock.build().unwrap();
        for h in 0..16 {
            let same = ships.frame(3, h) == stock_ships.frame(3, h);
            assert_eq!(same, h % 4 != 0, "sprite 3, heading index {h}");
        }
        // Flight 20: lander records 0, 1 in slots 0, 1 and 2..4 in slots 4..6.
        let lander = read("lander.td");
        let l = SpriteSources::load(&dir, Episode::E1, 20).unwrap();
        for (rec, s) in [(0, 0), (1, 1), (2, 4), (3, 5), (4, 6)] {
            assert_eq!(slot(&l, s), lander[rec * RECORD..(rec + 1) * RECORD]);
        }
        assert_eq!(
            (slot(&l, 2), slot(&l, 3)),
            (slot(&stock, 2), slot(&stock, 3))
        );
        // Helicopter flights: td.hel in slots 2, 3 and td.h2 in cells 2, 3 of every set.
        let (hel, h2) = (read("td.hel"), read("td.h2"));
        for (ep, n) in [
            (Episode::E1, 12),
            (Episode::E1, 15),
            (Episode::E1, 21),
            (Episode::E2, 8),
        ] {
            let s = SpriteSources::load(&dir, ep, n).unwrap();
            assert_eq!(s.shp[2 * RECORD..4 * RECORD], hel[..], "{ep} {n}");
            // h2 parts go to td.sc2, td.shr, td.sc3; `rot` is [sc3, shr, sc2].
            for (k, set) in [2, 1, 0].into_iter().enumerate() {
                let part = &h2[k * 0xC80..(k + 1) * 0xC80];
                assert_eq!(&s.rot[set][0xC80..0x1900], part, "{ep} {n} part {k}");
            }
        }
        assert_eq!(
            SpriteSources::load(&dir, Episode::E2, 7).unwrap().shp,
            stock.shp
        );
    }

    #[test]
    fn lander_bank_and_library_reload_match_c0045() {
        // flight-screens rev 2 §6.2 vector 19 / OB-057: bank slots 0..6 (7215:0000, 1C00h
        // bytes) before the ship library (f541) and after it (f1771: slot 0 = td.shp 0).
        let data = require_data!();
        let oracle = require_oracle!();
        let dir = DataDir::open(&data).unwrap();
        let mem = |name: &str| std::fs::read(oracle.join("C-0045").join("mem").join(name)).unwrap();
        let mut s = SpriteSources::load(&dir, Episode::E1, 20).unwrap();
        let before = mem("f00000541_7215-0000_1C00_bank-before.bin");
        assert_eq!(s.shp[..0x1C00], before[..]);
        assert!(s.reload_record0());
        let after = mem("f00001771_7215-0000_1C00_bank-after.bin");
        assert_eq!(s.shp[..0x1C00], after[..]);
        assert!(!s.reload_record0(), "a second reload changes nothing");
        // Flights without a slot-0 load are unaffected.
        let mut heli = SpriteSources::load(&dir, Episode::E1, 12).unwrap();
        assert!(!heli.reload_record0());
    }

    /// OB-057: after the ship library in E1 flight 20, Classic draws lander piece 0 with the
    /// stock record-0 sprite (preserved); the Enhanced fixes keep the lander's own sprite.
    #[test]
    fn ob057_library_reload_only_in_classic() {
        let data = require_data!();
        let dir = DataDir::open(&data).unwrap();
        let res = ExeResources::load(&dir).unwrap();
        let lander = dir.read("lander.td").unwrap();
        let stock = dir.read("td.shp").unwrap();
        for fixes in [false, true] {
            let mut arena = FlightArena::new(&dir, &res, Episode::E1, 20).unwrap();
            let frame = |a: &FlightArena| a.bank.frame(0).map(<[u8]>::to_vec);
            let before = frame(&arena);
            assert_eq!(arena.sprites.shp[..RECORD], lander[..RECORD]);
            arena.record0_reloaded(fixes);
            let slot0 = arena.sprites.shp[..RECORD].to_vec();
            if fixes {
                assert_eq!(slot0, lander[..RECORD], "Enhanced keeps the lander piece");
                assert_eq!(frame(&arena), before);
            } else {
                assert_eq!(slot0, stock[..RECORD], "Classic reloads td.shp record 0");
                assert_ne!(frame(&arena), before);
            }
        }
    }

    /// OB-024: E2 keeps the map-set-2 radar colours for the rest of the run in Classic; the
    /// Enhanced fixes show each flight's own set. E1 and E3 never use set 2.
    #[test]
    fn ob024_radar_table_restored_only_with_the_fixes() {
        let set2 = |n| episode::map_set(Episode::E2, n) == 2;
        let (one, two) = (
            (1..=20).find(|&n| !set2(n)).unwrap(),
            (1..=20).find(|&n| set2(n)).unwrap(),
        );
        // A set-1 flight after a set-2 flight (session flag set).
        assert!(radar_set2_shown(Episode::E2, one, true, false), "Classic");
        assert!(!radar_set2_shown(Episode::E2, one, true, true), "Enhanced");
        // The set-2 flight itself and a set-1 flight before any set-2 flight.
        for fixes in [false, true] {
            assert!(radar_set2_shown(Episode::E2, two, true, fixes));
            assert!(!radar_set2_shown(Episode::E2, one, false, fixes));
            assert!(!radar_set2_shown(Episode::E3, one, false, fixes));
            assert!(!radar_set2_shown(Episode::E1, one, false, fixes));
        }
    }

    #[test]
    fn helicopter_rotor_matches_c0027() {
        // flight-screens rev 2 §6.2 vector 17: E1 flight 12, player class 3 at heading 8
        // drawn late. Every flight frame f525..f1890 shows at (144, 68) the cell the port
        // draws for page 1 − DS:3A30 (the back page flips at step 24, after the draw).
        let data = require_data!();
        let oracle = require_oracle!();
        let dir = DataDir::open(&data).unwrap();
        let (ships, _) = SpriteSources::load(&dir, Episode::E1, 12)
            .unwrap()
            .build()
            .unwrap();
        let north = Heading::N.sprite_index();
        let cell = |page: i32| {
            let mut fb = Framebuffer::new();
            let frame = ships.frame(player_sprite(3, true, page), north).unwrap();
            flight::draw_player_ship(&mut fb, frame, td_render::headings::CELL);
            fb.pixels
        };
        let cells = [cell(0), cell(1)];
        assert_ne!(cells[0], cells[1]);
        let c27 = oracle.join("C-0027");
        let mut seen = [0; 2];
        for f in (525..=1890).step_by(35) {
            let fb = std::fs::read(c27.join("fb").join(format!("f{f:08}_320x192_fl.vga"))).unwrap()
                [768..]
                .to_vec();
            let ds = std::fs::read(
                c27.join("mem")
                    .join(format!("f{f:08}_4467-0000_9DF0_ds.bin")),
            )
            .unwrap();
            let page = usize::from(ds[0x3A30] ^ 1);
            let (x0, y0) = (SHIP_POS.0 as usize, SHIP_POS.1 as usize);
            let mut inked = 0;
            for y in y0..y0 + 32 {
                for x in x0..x0 + 32 {
                    let p = cells[page][y * 320 + x];
                    if p != 0 {
                        assert_eq!(fb[y * 320 + x], p, "f{f} page {page} ({x}, {y})");
                        inked += 1;
                    }
                }
            }
            assert!(inked > 300, "f{f}");
            seen[page] += 1;
        }
        assert!(seen[0] > 0 && seen[1] > 0, "both rotor frames: {seen:?}");
    }

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
    fn bindings_come_from_the_settings() {
        // The defaults are the table this file had before M11-03, plus Tab and M (which the
        // flight code reads, globals.md §3).
        let letter = |c| Key::letter(c).unwrap();
        let mut old = vec![
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
            (Key::TAB, FlightKey::Tab),
            (letter('m'), FlightKey::M),
        ];
        let mut now = bindings(&td_flow::Settings::default());
        old.sort();
        now.sort();
        assert_eq!(now, old);
        // A rebinding reaches the flight's key bytes.
        let mut s = td_flow::Settings::default();
        s.controls.keyboard.bind(FlightKey::Space, Some(Key::LCTRL));
        let mut t = Typematic::default();
        let mut input = TickInput::default();
        input.held.insert(Key::LCTRL);
        input.pressed.push(Key::LCTRL);
        let c = t.controls(&input, &bindings(&s));
        let mut want = TickControls::default();
        want.set_down(FlightKey::Space);
        want.set_made(FlightKey::Space);
        assert_eq!(c, want);
    }

    /// D9: the chord letters reach the sim as the keyboard's letter keys, with make codes and
    /// typematic repeats like the flight keys; a letter bound to a flight key is that key only.
    #[test]
    fn chord_letters_reach_the_key_table() {
        let mut binds = bindings(&td_flow::Settings::default());
        let letters: Vec<ChordKey> = chord_keys(&binds).iter().map(|&(_, k)| k).collect();
        assert_eq!(
            letters,
            ChordKey::ALL,
            "every chord letter is free by default"
        );
        let k = Key::letter('k').unwrap();
        let mut t = Typematic::default();
        let mut first = TickInput::default();
        first.held.insert(k);
        first.pressed.push(k);
        let c = t.controls(&first, &binds);
        assert!(c.is_chord_down(ChordKey::K) && c.any_made());
        let held = TickInput {
            held: first.held.clone(),
            ..TickInput::default()
        };
        let c = t.controls(&held, &binds);
        assert!(c.is_chord_down(ChordKey::K) && !c.any_made());
        // K bound to fire: the letter now only fires.
        binds.push((k, FlightKey::Space));
        assert!(!chord_keys(&binds).iter().any(|&(_, c)| c == ChordKey::K));
        let c = t.controls(&first, &binds);
        assert!(!c.is_chord_down(ChordKey::K) && c.is_down(FlightKey::Space));
        // After a real-time screen a held letter is set again (held_controls).
        let c = held_controls(&held, &bindings(&td_flow::Settings::default()));
        assert!(c.is_chord_down(ChordKey::K) && c.any_made());
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
        let made = |c: TickControls| c.any_made();
        assert!(made(t.controls(&first, &binds)));
        // 500 ms ≈ 9.6 ticks at 19.2 Hz: no repeat for 9 ticks, then one about every 1.8.
        let repeats: Vec<bool> = (1..=14).map(|_| made(t.controls(&held, &binds))).collect();
        assert_eq!(&repeats[..9], &[false; 9]);
        assert!(repeats[9]);
        assert!(repeats[9..].iter().filter(|&&r| r).count() >= 2);
    }

    /// E1 flight 1 after `ticks` ticks (P pressed in the last one when `pause`), composing
    /// Enhanced frames into the returned view.
    fn flown_arena(
        dir: &DataDir,
        resources: &ExeResources,
        ticks: u32,
        pause: bool,
    ) -> (FlightArena, crate::enhanced::SharedView) {
        let view = crate::enhanced::SharedView::default();
        let mut arena = FlightArena::new(dir, resources, Episode::E1, 1)
            .unwrap()
            .with_enhanced(Some(view.clone()));
        let settings = td_flow::Settings::default();
        let mut audio = Vec::new();
        // The opening fade-in; its last frame runs tick 1.
        while arena.opening.is_some() {
            let mut ctx = FlowContext {
                data: dir,
                resources,
                settings: &settings,
                audio: &mut audio,
            };
            arena.tick(&mut ctx, &TickInput::default());
        }
        assert_eq!(arena.flight.ticks, 1);
        for t in 2..=ticks {
            let mut input = TickInput::default();
            if pause && t == ticks {
                let p = Key::letter('p').unwrap();
                input.held.insert(p);
                input.pressed.push(p);
            }
            let mut ctx = FlowContext {
                data: dir,
                resources,
                settings: &settings,
                audio: &mut audio,
            };
            arena.tick(&mut ctx, &input);
            audio.clear();
        }
        (arena, view)
    }

    /// The Enhanced output of `arena` for `settings` and the Classic frame drawn with it.
    fn enhanced_output(
        arena: &FlightArena,
        view: &crate::enhanced::SharedView,
        settings: &td_flow::Settings,
    ) -> (Option<crate::enhanced::RgbaImage>, Framebuffer) {
        let mut e = crate::enhanced::Enhanced::new(
            settings,
            view.clone(),
            None,
            Box::new(td_render::enhanced::NoPost),
        );
        let on = e.begin_frame((1280, 960), td_platform::ScaleMode::Aspect4x3);
        let mut classic = Framebuffer::new();
        arena.draw(&mut classic);
        if !on {
            return (None, classic);
        }
        // What `present` hands the window.
        let out = e.render(&classic).unwrap();
        let img = crate::enhanced::RgbaImage {
            rgba: out.rgba.to_vec(),
            width: out.size.0,
            height: out.size.1,
        };
        (Some(img), classic)
    }

    /// M12-04: with the colour-blind guidance setting off the flight is presented as before
    /// (Classic path); with it on, only the guidance line's pixels differ from the Classic
    /// frame, the dots in the marker's core colour.
    #[test]
    fn colour_blind_guidance_changes_only_the_guidance_line() {
        use td_render::enhanced::guidance::{self, OUTLINE};
        let data = require_data!();
        let dir = DataDir::open(&data).unwrap();
        let resources = ExeResources::load(&dir).unwrap();
        let (arena, view) = flown_arena(&dir, &resources, 30, false);
        let mut settings = td_flow::Settings::default();
        let (off, _) = enhanced_output(&arena, &view, &settings);
        assert!(off.is_none(), "setting off: the Classic path");
        settings.accessibility.colorblind_guidance = true;
        let (on, classic) = enhanced_output(&arena, &view, &settings);
        let img = on.expect("setting on: the RGBA path");
        assert_eq!((img.width, img.height), (320, 200));
        let rgba = img.rgba;
        let display = arena.hud.borrow();
        let marker = display.hud.guidance_marker().expect("a guidance line");
        let core = guidance::colour(marker).unwrap();
        let marks = guidance::marks(&display.hud, 0, 320);
        let reference = classic.to_rgba();
        let mut changed = 0;
        for i in 0..320 * 200 {
            let (x, y) = ((i % 320) as i32, (i / 320) as i32);
            let px = &rgba[i * 4..i * 4 + 4];
            if px != &reference[i * 4..i * 4 + 4] {
                changed += 1;
                let m = marks.iter().rev().find(|m| (m.x, m.y) == (x, y));
                let m = m.unwrap_or_else(|| panic!("({x}, {y}) changed but is no mark"));
                assert_eq!(px, &[m.rgb[0], m.rgb[1], m.rgb[2], 255]);
            }
        }
        let dots = marks.iter().filter(|m| m.rgb == core).count();
        assert_eq!(dots, 6);
        assert!(changed >= 6, "{changed}");
        assert!(marks.iter().any(|m| m.rgb == OUTLINE));
    }

    /// M12-09 follow-up: the pause line is drawn over the wide frame (centred like the
    /// infra-red notice), not over a centred Classic picture.
    #[test]
    fn pause_is_drawn_over_the_wide_frame() {
        let data = require_data!();
        let dir = DataDir::open(&data).unwrap();
        let resources = ExeResources::load(&dir).unwrap();
        let (arena, view) = flown_arena(&dir, &resources, 30, true);
        assert!(
            matches!(arena.screen, Some(FlightModal::Pause(_))),
            "paused"
        );
        let mut paused = Framebuffer::new();
        arena.draw_frame(&mut paused);
        let mut play = Framebuffer::new();
        arena.draw_play(&mut play);
        let line: Vec<(i32, i32)> = (0..320 * 200)
            .filter(|&i| paused.pixels[i] != play.pixels[i])
            .map(|i| ((i % 320) as i32, (i / 320) as i32))
            .collect();
        assert!(!line.is_empty(), "the pause line differs from the frame");
        let mut settings = td_flow::Settings::default();
        settings.profile = td_flow::settings::Profile::Enhanced;
        settings.enhanced.widescreen = Some(td_flow::settings::Widescreen::W16x9);
        let mut e = crate::enhanced::Enhanced::new(
            &settings,
            view.clone(),
            None,
            Box::new(td_render::enhanced::NoPost),
        );
        assert!(e.begin_frame((1920, 1080), td_platform::ScaleMode::Aspect4x3));
        let mut classic = Framebuffer::new();
        arena.draw(&mut classic);
        let v = view.borrow();
        let wide = v.composed().expect("the pause composes the wide frame");
        assert_eq!(wide.width(), 428);
        let m = 54;
        for &(x, y) in &line {
            assert_eq!(
                wide.get(x + m, y),
                paused.get(x, y),
                "pause pixel ({x}, {y})"
            );
        }
        // The side panels show the city, not black.
        assert!((0..160).any(|y| wide.get(0, y) != Some(0)));
    }
    /// The teardown's fade-out (flight-screens rev 5 §4.5.3, M12-14): levels 16..1 shown 3, 5,
    /// …, 33 frames after the last tick and level 0 at 34 (C-0069 last tick f1808, fade-out
    /// f1809.92: levels complete at f1812, 1814, …, 1842, 1843); after a Quit nothing is left
    /// to fade.
    #[test]
    fn teardown_fades_out_in_34_frames() {
        let mut pal = Palette::default();
        for (i, c) in pal.colors.iter_mut().enumerate() {
            *c = td_formats::palette::Rgb6 {
                r: (i % 64) as u8,
                g: 20,
                b: 63,
            };
        }
        let mut t = Teardown::new(&pal, pal.clone(), 0, false);
        let mut frames = 0;
        let mut shown = Vec::new();
        let mut last = t.dac.clone();
        while !t.step() {
            frames += 1;
            if t.dac != last {
                shown.push(frames);
                last = t.dac.clone();
            }
            assert!(frames < 100);
        }
        assert_eq!(frames, 34, "the last level in the 34th frame");
        // Level 16 equals the palette shown, so the first change is level 15.
        let mut want: Vec<u32> = (2..=16).map(|k| 2 * k + 1).collect();
        want.push(34);
        assert_eq!(shown, want);
        assert!(t.dac.colors.iter().all(|c| *c == Default::default()));
        let mut t = Teardown::new(&pal, pal.clone(), 0, true);
        assert!(t.step(), "after Quit: only the wait");
        assert!(t.dac.colors.iter().all(|c| *c == Default::default()));
        // E2 fades entries 1..255 only (episode-deltas rev 3 §6).
        let mut t = Teardown::new(&pal, pal.clone(), 1, false);
        while !t.step() {}
        assert_eq!(t.dac.colors[0], pal.colors[0]);
        assert!(t.dac.colors[1..].iter().all(|c| *c == Default::default()));
    }

    /// Runs `arena` until tick 1 has run; returns, per frame, the DAC drawn and the tick count.
    fn open(arena: &mut FlightArena, dir: &DataDir, res: &ExeResources) -> Vec<(Palette, u32)> {
        let settings = td_flow::Settings::default();
        let mut audio = Vec::new();
        let mut out = Vec::new();
        while arena.flight.ticks == 0 {
            assert!(
                (arena.tick_hz() - RETRACE_HZ).abs() < 1e-9,
                "the fade runs at the retrace rate"
            );
            let mut ctx = FlowContext {
                data: dir,
                resources: res,
                settings: &settings,
                audio: &mut audio,
            };
            arena.tick(&mut ctx, &TickInput::default());
            let mut fb = Framebuffer::new();
            arena.draw_frame(&mut fb);
            out.push((fb.palette.clone(), arena.flight.ticks));
            assert!(out.len() < 100);
        }
        out
    }

    /// The opening fade-in (hud §4.1 step 6, flight-screens rev 5 §4.5.3, M12-14): level k of
    /// the flight palette shown 2k + 2 frames after the flight opens, tick 1 in the frame of
    /// level 16 (C-0002 f3439..3473, C-0017 f458..492, E2 C-0015 f631..665, E3 C-0036
    /// f660..694); in infra-red mode 1 the start formula and tick 1 one frame later (C-0019).
    /// Before tick 1 the playfield is black and the bar shows `td.win`, the weapon icon and the
    /// missile column. Keys pressed during the fade reach tick 1.
    #[test]
    fn opening_fades_in_before_tick_1() {
        use td_render::flight_palette::ir_palette;
        let data = require_data!();
        let dir = DataDir::open(&data).unwrap();
        let res = ExeResources::load(&dir).unwrap();
        for (ep, n, first) in [
            (Episode::E1, 1, 1),
            (Episode::E2, 1, 1),
            (Episode::E3, 1, 0),
        ] {
            let mut arena = FlightArena::new(&dir, &res, ep, n).unwrap();
            let target = fade_target(&arena.flight_palette(), 0);
            let frames = open(&mut arena, &dir, &res);
            assert_eq!(frames.len(), 34, "{ep} flight {n}: tick 1 in frame 34");
            for (i, (dac, ticks)) in frames.iter().enumerate() {
                let f = i as u32 + 1;
                if f < 34 {
                    assert_eq!(*ticks, 0);
                    let mut want = Palette::default();
                    if f >= 2 {
                        write_fade_step(&mut want, &target, (f / 2 - 1) as u8, first);
                    }
                    assert_eq!(dac, &want, "{ep} flight {n}, frame {f}");
                }
            }
            // Tick 1's frame: the fade's last level under the instruments' lamp entries.
            let (dac, _) = &frames[33];
            assert_eq!(dac.colors[1..36], target.colors[1..36], "{ep}");
        }
        // E1 flight 4 starts in infra-red mode 1: the fade reaches the fade formula (B / 8),
        // then the start formula (B / 4, entry 37 readable) comes with tick 1.
        let mut arena = FlightArena::new(&dir, &res, Episode::E1, 4).unwrap();
        assert_eq!(arena.flight.palette_mode, 1);
        // The target as setup left g_FlightPalette (tick 1 then lights the compass lamps).
        let fade = fade_target(&arena.flight_palette(), 1);
        let frames = open(&mut arena, &dir, &res);
        assert_eq!(frames.len(), 35);
        assert_eq!(frames[33].0.colors[1..], fade.colors[1..]);
        assert_eq!(
            frames[34].0.colors[37],
            ir_palette(&arena.palette, 1).colors[37]
        );
        // A key pressed during the fade and still held has its byte set at tick 1 (the key
        // table was cleared and the handler hooked before the fade); one held since before the
        // flight waits for a typematic repeat.
        for (pressed_at, set) in [(Some(10), true), (None, false)] {
            let mut arena = FlightArena::new(&dir, &res, Episode::E1, 2).unwrap();
            let settings = td_flow::Settings::default();
            let mut audio = Vec::new();
            for f in 1..=34 {
                let mut input = TickInput::default();
                if pressed_at.is_none_or(|p| f >= p) {
                    input.held.insert(Key::UP);
                }
                if pressed_at == Some(f) {
                    input.pressed.push(Key::UP);
                }
                let mut ctx = FlowContext {
                    data: &dir,
                    resources: &res,
                    settings: &settings,
                    audio: &mut audio,
                };
                arena.tick(&mut ctx, &input);
            }
            assert_eq!(arena.flight.ticks, 1);
            assert_eq!(arena.flight.keys.get(FlightKey::Up), set, "{pressed_at:?}");
        }
    }

    /// Every flight of every episode opens (fade-in, tick 1) and flies a few ticks, drawn each
    /// frame. Until M12-14 no E2 or E3 arena could be built (the radar table size check).
    #[test]
    fn every_flight_opens_and_flies() {
        let data = require_data!();
        let dir = DataDir::open(&data).unwrap();
        let res = ExeResources::load(&dir).unwrap();
        let settings = td_flow::Settings::default();
        let flights = (1..=21u8)
            .map(|n| (Episode::E1, n))
            .chain((1..=20).map(|n| (Episode::E2, n)))
            .chain((1..=21).filter(|&n| n != 20).map(|n| (Episode::E3, n)));
        for (ep, n) in flights {
            let mut arena = FlightArena::new(&dir, &res, ep, n)
                .unwrap_or_else(|e| panic!("{ep} flight {n}: {e:#}"));
            let mut audio = Vec::new();
            let mut fb = Framebuffer::new();
            for _ in 0..34 + 20 {
                let mut ctx = FlowContext {
                    data: &dir,
                    resources: &res,
                    settings: &settings,
                    audio: &mut audio,
                };
                arena.tick(&mut ctx, &TickInput::default());
                arena.draw(&mut fb);
                audio.clear();
            }
            assert!(arena.flight.ticks >= 19, "{ep} flight {n}");
        }
    }

    /// The opening picture against the original's frames inside the fade: C-0017 f490 (E1
    /// flight 2 at a direct start), C-0015 f660 (E2 flight 1), C-0036 f690 (E3 flight 1), the
    /// fades starting in f458, f631 and f660. Pixels exact (black playfield, the bar as setup
    /// drew it); every DAC entry at the level the port shows in that frame or at the next one
    /// (the original writes a level over two frames).
    #[test]
    fn opening_frames_match_the_original() {
        let data = require_data!();
        let oracle = require_oracle!();
        let dir = DataDir::open(&data).unwrap();
        let res = ExeResources::load(&dir).unwrap();
        for (ep, n, capture, start, dump) in [
            (Episode::E1, 2, "C-0017", 458, "f00000490_320x200_fl.vga"),
            (
                Episode::E2,
                1,
                "C-0015",
                631,
                "f00000660_320x200_flight.vga",
            ),
            (
                Episode::E3,
                1,
                "C-0036",
                660,
                "f00000690_320x200_flight.vga",
            ),
        ] {
            let raw = std::fs::read(oracle.join(capture).join("fb").join(dump)).unwrap();
            let theirs = Palette::parse_prefix(&raw).unwrap();
            let px = &raw[768..];
            let f: usize = dump[1..9].parse().unwrap();
            let mut arena = FlightArena::new(&dir, &res, ep, n).unwrap();
            let mut fb = Framebuffer::new();
            arena.draw_frame(&mut fb);
            assert_eq!(fb.pixels.len(), px.len());
            let diff: Vec<usize> = (0..px.len()).filter(|&i| fb.pixels[i] != px[i]).collect();
            assert!(
                diff.is_empty(),
                "{capture} f{f}: {} pixels differ, first at ({}, {})",
                diff.len(),
                diff[0] % 320,
                diff[0] / 320
            );
            let frames = open(&mut arena, &dir, &res);
            let k = f - start;
            let (now, next) = (&frames[k - 1].0, &frames[k + 1].0);
            let mut at_next = 0;
            for e in 0..256 {
                let c = theirs.colors[e];
                assert!(
                    c == now.colors[e] || c == next.colors[e],
                    "{capture} f{f}: DAC entry {e} {c:?}, ours {:?} or {:?}",
                    now.colors[e],
                    next.colors[e]
                );
                at_next += usize::from(c != now.colors[e]);
            }
            eprintln!("{capture} f{f}: {at_next} entries already at the next level");
        }
    }

    /// sfx-policy rev 4 §4.2 in the host: sounds before tick-order step 26 are judged before
    /// the priority reset, the rest after it; the reset needs WORX's playing status to have
    /// cleared (voc-and-adpcm rev 3 §2.3), not merely the audio to have ended.
    #[test]
    fn step_26_resets_the_priority_only_after_the_busy_time() {
        let data = require_data!();
        let dir = DataDir::open(&data).unwrap();
        let resources = ExeResources::load(&dir).unwrap();
        let mut arena = FlightArena::new(&dir, &resources, Episode::E1, 1).unwrap();
        let settings = td_flow::Settings::default();
        let sound = |kind: u8| Event::Sound { kind, source: None };
        // Runs one call's sounds: `events`, step 26 before entry `reset`, at time `now`.
        let run = |arena: &mut FlightArena, events: Vec<Event>, reset: usize, now: f64| {
            let mut audio = Vec::new();
            let mut ctx = FlowContext {
                data: &dir,
                resources: &resources,
                settings: &settings,
                audio: &mut audio,
            };
            arena.events = events;
            arena.flight.sound_reset_at = Some(reset);
            arena.issue_sounds(&mut ctx, 0, now - 0.05, now);
            audio
                .iter()
                .filter_map(|c| match c {
                    AudioCommand::PlayEffect { id, .. } => Some(*id),
                    _ => None,
                })
                .collect::<Vec<u8>>()
        };
        // An explosion before step 26 (at 0.0 s), a hit after it: the hit is refused.
        assert_eq!(run(&mut arena, vec![sound(100), sound(9)], 1, 0.05), [100]);
        assert_eq!(arena.voice.priority(), 5);
        // boomboom.voc's sound ends at 1.759 s, but WORX reports it playing until 1.882 s
        // (r = 250): at 1.85 s the reset does not happen.
        assert_eq!(run(&mut arena, vec![sound(9)], 0, 1.85), [] as [u8; 0]);
        assert_eq!(arena.voice.priority(), 5);
        // At 1.90 s it does, and the hit plays.
        assert_eq!(run(&mut arena, vec![sound(9)], 0, 1.90), [9]);
        assert_eq!(arena.voice.priority(), 0);
    }
}
