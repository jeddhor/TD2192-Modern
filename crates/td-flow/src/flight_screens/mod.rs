//! In-flight modal screens: the Esc computer menu (ship library, help), the F6 transmission
//! screen, the F5 tactical map and the pause line, plus the infra-red notice, and the two
//! real-time screens of the E1 mission events ([`interlude`]: flight 17's cut-scene, flight
//! 19's V.C.I. screen).
//!
//! Spec: `docs/spec/flow/flight-screens.md` rev 1 (§3, §4.1–§4.5.2), rev 4 §4.10 for the
//! interludes. Fidelity: T0 for the
//! pictures (CONFIRMED by C-0012..C-0014), T1 for what they change (abort flag, key bytes),
//! T2 for their pacing.
//!
//! The original runs these screens *inside* a tick: the simulation stops at the tick-order
//! step that opened the screen and continues when it closes (`td_sim::Flight::resume`). The
//! flight host keeps one [`FlightModal`] while a screen is open, ticks it once per vertical
//! retrace ([`crate::RETRACE_HZ`]: fades, highlight pulses and typing step once per retrace)
//! and shows its picture instead of the flight's.
//!
//! Each screen draws into its own canvas, starting from the picture that was on screen when
//! it opened (the comm panel and the pause line are drawn over the playfield), and the canvas
//! carries the DAC the screen sets. Port policy: the screens read keys from [`KeyBytes`], a
//! model of the original's key-state table that sees make codes and releases but not the
//! keyboard's typematic repeats.

pub mod comm;
pub mod interlude;
pub mod map;
pub mod menu;

use std::collections::BTreeSet;

use td_assets::{AssetError, DataDir, Episode, ExeResources, ResourceValue};
use td_formats::FormatError;
use td_formats::font::{Font, FontKind};
use td_formats::palette::Palette;
use td_input::{Key, TickInput};
use td_render::Framebuffer;
use td_render::small_text;
use thiserror::Error;

pub use comm::CommScreen;
pub use interlude::Interlude;
pub use map::{ShipDot, TacticalMap, TargetBoard};
pub use menu::ComputerMenu;

/// Rows the flight display shows (CRTC vertical display end 17Fh: 384 scan lines).
pub const FLIGHT_ROWS: usize = 192;

/// Byte of `td.lt2` that holds the opaque-text background (`g_Font5` + 650, §2).
pub const FONT_BACKGROUND_BYTE: usize = 650;

/// Ink of the pause line and the IR notice (`0x25`, §3.5).
pub const NOTICE_INK: u8 = 37;
/// Screen pen of the pause line and the IR notice (§3.5: x R + 80, y 60 + viewSubY of the
/// page, i.e. screen (96, 76)).
pub const NOTICE_PEN: (i32, i32) = (96, 76);

/// Black outline of the pause line: the offsets drawn in ink 0 before the ink-37 pass (§4.4).
const PAUSE_OUTLINE: [(i32, i32); 8] = [
    (1, 0),
    (-1, 0),
    (1, 1),
    (-1, 1),
    (1, -1),
    (-1, -1),
    (0, 1),
    (0, -1),
];

/// Errors while loading the screens' data.
#[derive(Debug, Error)]
pub enum FlightScreensError {
    #[error(transparent)]
    Asset(#[from] AssetError),
    #[error(transparent)]
    Format(#[from] FormatError),
    #[error("{0} is unavailable (file0001.exe resources)")]
    Resource(&'static str),
    #[error("{0} has an unexpected size")]
    Size(&'static str),
}

/// What the screens read from the data files and from `file0001.exe`.
#[derive(Debug, Clone)]
pub struct FlightScreenAssets {
    /// The program the screens belong to (transmission tables, ship-library rules).
    pub episode: Episode,
    /// `td.lt2`.
    pub font: Font,
    /// `td.lt2` byte 650: the flight's opaque-text background (178 in the shipped file).
    pub font_background: u8,
    /// `td.men` bytes 0..16,000: the 160 × 100 computer menu.
    pub menu: Vec<u8>,
    /// `td.shp` records 0..8 (32 × 32 each), for the ship library.
    pub library: Vec<Vec<u8>>,
    /// `td.hlp`: palette, then two 320 × 200 pages.
    pub help: Vec<u8>,
    /// `td.cmm`: 32 cells of 32 × 32 (the transmission panel).
    pub panel: Vec<u8>,
    /// `td1.ccm` (E2: `td2.ccm`, E3: `td3.ccm`): comm portraits, 32 × 32 each.
    pub portraits: Vec<u8>,
    /// Transmission screens 1.. as the comm loaders fill them ([`comm::transmission_for`]);
    /// `None` for a screen the episode lacks (E3 4).
    pub transmissions: Vec<Option<comm::Transmission>>,
    /// `ship_library`.
    pub ship_library: Vec<String>,
    /// `comm.prompts`: [0] the "press Space" prompt, [1] the closing line.
    pub comm_prompts: Vec<String>,
    /// The string run of each transmission: `e1.comms.01` .. `e1.comms.09`; E2 `e2.comms.01`
    /// .. `.05`, `.06` + `.07`, `.08` (episode-deltas rev 2 §10).
    pub comms: Vec<Vec<String>>,
    /// `flight.map_prompts`.
    pub map_prompts: Vec<String>,
    /// `flight.prompts`: [0] the IR notice, [1] the pause line.
    pub flight_prompts: Vec<String>,
    /// `e1.target_info` (E2: `e2.target_info`, E3: `e3.target_info`).
    pub target_info: Vec<String>,
    /// `hud.radar_tile_colours` (`k_TileRadarColour`, tiles 0..70); E2 the map-set-1 or, once
    /// a set-2 flight has run, the set-2 table (OB-024); E3 its own table for both maps.
    pub radar_colours: Vec<u8>,
    /// E1: `td1-17.pic`, the flight-17 cut-scene's picture (320 × 90, no palette); empty in E2
    /// and E3 (§4.10.3: no cut-scene).
    pub scene_picture: Vec<u8>,
    /// E1: `e1.flight_events.02`, the cut-scene's five lines.
    pub scene_texts: Vec<String>,
    /// E1: `e1.flight_events.01`, the flight-19 F5 lines and (index 5) the V.C.I. screen's line.
    pub vci_texts: Vec<String>,
}

/// The resource ids that differ per episode: ship library, comm prompts, map prompts, flight
/// prompts, target info, the comm string runs.
struct Ids {
    ship_library: &'static str,
    comm_prompts: &'static str,
    map_prompts: &'static str,
    flight_prompts: &'static str,
    target_info: &'static str,
    comms: &'static [&'static [&'static str]],
}

const E1_IDS: Ids = Ids {
    ship_library: "ship_library",
    comm_prompts: "comm.prompts",
    map_prompts: "flight.map_prompts",
    flight_prompts: "flight.prompts",
    target_info: "e1.target_info",
    comms: &[
        &["e1.comms.01"],
        &["e1.comms.02"],
        &["e1.comms.03"],
        &["e1.comms.04"],
        &["e1.comms.05"],
        &["e1.comms.06"],
        &["e1.comms.07"],
        &["e1.comms.08"],
        &["e1.comms.09"],
    ],
};

/// E2. The ship library draws the E2 strings at E1's pens and inks (flow/e2-flight-screens
/// rev 1 §3.3, U-313 resolved; C-0044, `tests/e2_flight_screens.rs`).
const E2_IDS: Ids = Ids {
    ship_library: "e2.ship_library",
    comm_prompts: "e2.comm.prompts",
    map_prompts: "e2.flight.map_prompts",
    flight_prompts: "e2.flight.prompts",
    target_info: "e2.target_info",
    comms: &[
        &["e2.comms.01"],
        &["e2.comms.02"],
        &["e2.comms.03"],
        &["e2.comms.04"],
        &["e2.comms.05"],
        &["e2.comms.06", "e2.comms.07"],
        &["e2.comms.08"],
    ],
};

/// E3 (e2-flight-screens rev 1 §3.2, §3.3): comm screens 1, 2, 3 and 5 on the runs
/// `e3.comms.01`, `.02`, `.03` and `.04` (there is no screen 4).
const E3_IDS: Ids = Ids {
    ship_library: "e3.ship_library",
    comm_prompts: "e3.comm.prompts",
    map_prompts: "e3.flight.map_prompts",
    flight_prompts: "e3.flight.prompts",
    target_info: "e3.target_info",
    comms: &[
        &["e3.comms.01"],
        &["e3.comms.02"],
        &["e3.comms.03"],
        &[],
        &["e3.comms.04"],
    ],
};

fn texts(res: &ExeResources, id: &'static str) -> Result<Vec<String>, FlightScreensError> {
    res.texts(id)
        .map(<[String]>::to_vec)
        .ok_or(FlightScreensError::Resource(id))
}

impl FlightScreenAssets {
    /// Loads everything (Episode 1).
    pub fn load(data: &DataDir, res: &ExeResources) -> Result<Self, FlightScreensError> {
        Self::load_episode(data, res, Episode::E1, false)
    }

    /// Loads everything for `episode`. `radar_set2`: E2's radar table holds the map-set-2
    /// colours (`Session::radar_set2`); ignored in E1 and E3.
    pub fn load_episode(
        data: &DataDir,
        res: &ExeResources,
        episode: Episode,
        radar_set2: bool,
    ) -> Result<Self, FlightScreensError> {
        let ids = match episode {
            Episode::E1 => &E1_IDS,
            Episode::E2 => &E2_IDS,
            Episode::E3 => &E3_IDS,
        };
        let font_bytes = data.read("td.lt2")?;
        let font_background = *font_bytes
            .get(FONT_BACKGROUND_BYTE)
            .ok_or(FlightScreensError::Size("td.lt2"))?;
        let mut menu = data.read("td.men")?;
        if menu.len() < 16_000 {
            return Err(FlightScreensError::Size("td.men"));
        }
        menu.truncate(16_000);
        let shp = data.read("td.shp")?;
        if shp.len() < 9 * 1024 {
            return Err(FlightScreensError::Size("td.shp"));
        }
        let library = shp.chunks_exact(1024).take(9).map(<[u8]>::to_vec).collect();
        let help = data.read("td.hlp")?;
        if help.len() != 768 + 2 * 64_000 {
            return Err(FlightScreensError::Size("td.hlp"));
        }
        let panel = data.read("td.cmm")?;
        if panel.len() != 32 * 1024 {
            return Err(FlightScreensError::Size("td.cmm"));
        }
        // UI_DrawCommPortraitA/B read td2.ccm / td3.ccm (episode-deltas rev 3 §6).
        let portraits = data.read(match episode {
            Episode::E1 => "td1.ccm",
            Episode::E2 => "td2.ccm",
            Episode::E3 => "td3.ccm",
        })?;
        let radar_id = match (episode, radar_set2) {
            (Episode::E1, _) => "hud.radar_tile_colours",
            (Episode::E2, false) => "e2.hud.radar_tile_colours",
            (Episode::E2, true) => "e2.hud.radar_tile_colours_set2",
            // One table for td3.map and td4.map (e3-mission-setup §3.1).
            (Episode::E3, _) => "e3.hud.radar_tile_colours",
        };
        let Some(ResourceValue::Bytes(radar_colours)) = res.get(radar_id).cloned() else {
            return Err(FlightScreensError::Resource(radar_id));
        };
        let comms: Vec<Vec<String>> = ids
            .comms
            .iter()
            .map(|run| {
                run.iter()
                    .map(|id| texts(res, id))
                    .collect::<Result<Vec<_>, _>>()
                    .map(|parts| parts.concat())
            })
            .collect::<Result<_, _>>()?;
        let comm_prompts = texts(res, ids.comm_prompts)?;
        let transmissions = (1..=comms.len())
            .map(|n| comm::transmission_for(episode, n))
            .collect();
        // The real-time screens of the E1 mission events (§4.10).
        let (scene_picture, scene_texts, vci_texts) = if episode == Episode::E1 {
            (
                data.read("td1-17.pic")?,
                texts(res, "e1.flight_events.02")?,
                texts(res, "e1.flight_events.01")?,
            )
        } else {
            Default::default()
        };
        Ok(Self {
            episode,
            font: Font::parse(&font_bytes, FontKind::Small)?,
            font_background,
            menu,
            library,
            help,
            panel,
            portraits,
            transmissions,
            ship_library: texts(res, ids.ship_library)?,
            comm_prompts,
            comms,
            map_prompts: texts(res, ids.map_prompts)?,
            flight_prompts: texts(res, ids.flight_prompts)?,
            target_info: texts(res, ids.target_info)?,
            radar_colours,
            scene_picture,
            scene_texts,
            vci_texts,
        })
    }

    /// The help palette (`td.hlp` bytes 0..768).
    pub fn help_palette(&self) -> Palette {
        Palette::parse_prefix(&self.help).unwrap_or_default()
    }

    /// Help page `n` (0 or 1), 320 × 200.
    pub fn help_page(&self, n: usize) -> &[u8] {
        let start = 768 + n * 64_000;
        &self.help[start..start + 64_000]
    }

    /// One string of a resource list, or "" (a missing index draws nothing).
    fn text(list: &[String], i: usize) -> &str {
        list.get(i).map_or("", String::as_str)
    }
}

/// The original's key-state table as the screens see it: a byte is set by a key's make code
/// and cleared by its break code or by the screen.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyBytes(BTreeSet<Key>);

impl KeyBytes {
    /// Applies one frame of input: releases first, then this frame's presses (a key tapped
    /// within one frame is seen for that frame).
    pub fn update(&mut self, input: &TickInput) {
        self.0.retain(|k| input.held.contains(k));
        self.0.extend(input.pressed.iter().copied());
    }

    /// True while `key`'s byte is set.
    pub fn get(&self, key: Key) -> bool {
        self.0.contains(&key)
    }

    /// Clears `key`'s byte.
    pub fn clear(&mut self, key: Key) {
        self.0.remove(&key);
    }

    /// Clears every byte ("clear key bytes 1..600").
    pub fn clear_all(&mut self) {
        self.0.clear();
    }

    /// True if any byte is set.
    pub fn any(&self) -> bool {
        !self.0.is_empty()
    }
}

/// Host key of the keypad 8 key (scan code 48h without the E0 prefix).
pub const KEY_KP8: Key = Key(0x60);
/// Host key of the keypad 2 key (50h without E0).
pub const KEY_KP2: Key = Key(0x5A);
/// Host key of F5 (3Fh).
pub const KEY_F5: Key = Key(0x3E);

/// How a screen's frame ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalStep {
    /// The screen stays open.
    Continue,
    /// The screen closed; `abort` is the computer menu's Quit (`g_MissionAborted` := 1).
    Close { abort: bool },
}

/// Draws the pause line (`flight.prompts[1]`) with its black outline (§4.4).
pub fn draw_pause_line(fb: &mut Framebuffer, assets: &FlightScreenAssets) {
    let text = FlightScreenAssets::text(&assets.flight_prompts, 1);
    let (x, y) = NOTICE_PEN;
    for (dx, dy) in PAUSE_OUTLINE {
        small_text::draw(fb, &assets.font, text, (x + dx, y + dy), 0);
    }
    small_text::draw(fb, &assets.font, text, NOTICE_PEN, NOTICE_INK);
}

/// Draws the infra-red notice (`flight.prompts[0]`, tick-order step 21, §4.5.2). The caller
/// decides when: palette mode 1 and mission ticks ≤ [`IR_NOTICE_TICKS`].
pub fn draw_ir_notice(fb: &mut Framebuffer, assets: &FlightScreenAssets) {
    let text = FlightScreenAssets::text(&assets.flight_prompts, 0);
    small_text::draw(fb, &assets.font, text, NOTICE_PEN, NOTICE_INK);
}

/// The IR notice shows while the mission tick counter is at most this (§4.5.2).
pub const IR_NOTICE_TICKS: u32 = 200;

/// True when the IR notice is drawn this tick.
pub fn shows_ir_notice(palette_mode: u8, ticks: u32) -> bool {
    palette_mode == 1 && ticks <= IR_NOTICE_TICKS
}

/// The pause (tick-order step 23, §4.4): the line over the frame just shown, until any key.
#[derive(Debug, Clone)]
pub struct Pause {
    canvas: Framebuffer,
}

impl Pause {
    /// Pauses over `shown`, the frame on screen.
    #[must_use]
    pub fn new(mut shown: Framebuffer, assets: &FlightScreenAssets) -> Self {
        draw_pause_line(&mut shown, assets);
        Self { canvas: shown }
    }

    /// The key bytes were cleared when the pause began, so only a new make code ends it; that
    /// key stays down and acts in the resumed tick.
    pub fn tick(&mut self, input: &TickInput) -> ModalStep {
        if input.any_pressed() {
            ModalStep::Close { abort: false }
        } else {
            ModalStep::Continue
        }
    }

    pub fn canvas(&self) -> &Framebuffer {
        &self.canvas
    }
}

/// The screen open inside a flight tick.
#[derive(Debug, Clone)]
pub enum FlightModal {
    Menu(Box<ComputerMenu>),
    Comm(Box<CommScreen>),
    Map(Box<TacticalMap>),
    Pause(Box<Pause>),
    /// A real-time screen of the mission events (§4.10), shown inside the tick that requested
    /// it (`td_sim::Modal::Scene`).
    Interlude(Box<Interlude>),
}

impl FlightModal {
    /// Advances one retrace.
    pub fn tick(&mut self, assets: &FlightScreenAssets, input: &TickInput) -> ModalStep {
        match self {
            Self::Menu(m) => m.tick(assets, input),
            Self::Comm(c) => c.tick(assets, input),
            Self::Map(m) => m.tick(assets, input),
            Self::Pause(p) => p.tick(input),
            Self::Interlude(i) => i.tick(assets),
        }
    }

    /// Shows the screen (pixels and DAC).
    pub fn draw(&self, fb: &mut Framebuffer) {
        let canvas = match self {
            Self::Menu(m) => m.canvas(),
            Self::Comm(c) => c.canvas(),
            Self::Map(m) => m.canvas(),
            Self::Pause(p) => p.canvas(),
            Self::Interlude(i) => i.canvas(),
        };
        fb.clone_from(canvas);
    }
}

/// Blanks the rows below the flight display's 192 (the computer menu, ship library and help
/// keep the 192-row display).
fn blank_below_display(fb: &mut Framebuffer) {
    fb.pixels[FLIGHT_ROWS * td_render::WIDTH..].fill(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_bytes_see_taps_and_releases() {
        let mut k = KeyBytes::default();
        let tap = TickInput {
            pressed: vec![Key::SPACE],
            ..TickInput::default()
        };
        k.update(&tap);
        assert!(k.get(Key::SPACE), "a tap within one frame is seen once");
        k.update(&TickInput::default());
        assert!(!k.get(Key::SPACE));
        let mut held = TickInput::default();
        held.held.insert(Key::UP);
        held.pressed.push(Key::UP);
        k.update(&held);
        held.pressed.clear();
        k.update(&held);
        assert!(k.get(Key::UP), "held keys stay set");
        k.clear(Key::UP);
        k.update(&held);
        assert!(
            !k.get(Key::UP),
            "no typematic repeat re-sets a cleared byte"
        );
    }

    #[test]
    fn ir_notice_window() {
        assert!(shows_ir_notice(1, 1));
        assert!(shows_ir_notice(1, 200));
        assert!(!shows_ir_notice(1, 201));
        assert!(!shows_ir_notice(2, 5));
        assert!(!shows_ir_notice(0, 5));
    }
}
