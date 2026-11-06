//! The F5 tactical map (`UI_TacticalMap`) and its target entries (`Msn_LoadTargetInfo`).
//!
//! Spec: `docs/spec/flow/flight-screens.md` rev 1 §3.4, §3.5, §4.3. Fidelity: T0 (C-0012:
//! map pixels, info lines, the flight-1 entries in RAM). The screen changes no game state.
//!
//! Port policy (U-220): the map reads the BIOS keyboard, whose typematic repeat of a held F5
//! can close it again at once; the port takes only new key presses.

use td_assets::Episode;
use td_formats::map::TileMap;
use td_input::{Key, TickInput};
use td_render::small_text;
use td_render::{Framebuffer, WIDTH};

use super::{FlightScreenAssets, KEY_F5, ModalStep};

/// Entries 0..31: 0 the player, 1..20 the mission slots, 21..31 fixed points (§3.4).
pub const ENTRIES: usize = 32;
/// Highest entry Tab selects.
pub const LAST_SELECTABLE: usize = 30;
/// Characters of an info line (`string[30]`).
pub const LINE_LEN: usize = 30;
/// Map size drawn: tiles 1..=200 on both axes, tile (x, y) at screen (x, y − 1).
pub const MAP_TILES: i32 = 200;
/// Colours (§4.3): player dot, ship dots (class + base), the selection cross, the text.
pub const PLAYER_COLOUR: u8 = 0x46;
pub const SHIP_COLOUR_BASE: u8 = 0x4E;
pub const CROSS_COLOUR: u8 = 0x5C;
pub const TEXT_INK: u8 = 92;
/// Text pen x and the pen y of the five info lines and the two prompts (§3.5, §4.3).
pub const TEXT_X: i32 = 205;
pub const INFO_Y: [i32; 5] = [15, 25, 35, 105, 115];
pub const PROMPT_Y: [i32; 2] = [165, 185];
/// The selection cross around entry (X, Y): a plus with a 3-pixel hole (§4.3).
pub const CROSS: [(i32, i32); 8] = [
    (-3, -1),
    (-2, -1),
    (2, -1),
    (3, -1),
    (0, -4),
    (0, -3),
    (0, 1),
    (0, 2),
];

/// A fixed point: (entry, map x, map y).
type Fixed = (usize, u8, u8);

/// The per-flight entry lines and fixed points of §3.4, in the spec's notation: entries (a
/// list of `a` or `a..b`), then the five `e1.target_info` indices. Flights 14 and 21 write
/// nothing.
const TARGETS: [(&str, &[Fixed]); 21] = [
    (
        "0: 2 3 4 1 1; 1..2: 5 6 7 8 9; 3..4: 5 6 7 10 9; 5..7, 9..10: 5 6 7 11 12; \
         8: 13 6 7 14 15; 11: 13 6 7 16 1; 12..20: 17 18 7 1 1; 21: 0 1 1 1 1",
        &[(21, 107, 104)],
    ),
    (
        "0: 19 3 4 1 1; 1: 13 6 7 20 1; 2: 5 3 21 22 23; 3..4: 5 6 7 24 1; 5: 13 6 7 25 1; \
         6..7: 13 6 7 26 1; 21: 0 1 1 1 1",
        &[(21, 107, 104)],
    ),
    (
        "0: 19 3 4 1 1; 1: 19 3 27 1 1; 2..3, 6..7, 12: 13 6 7 26 1; 4: 19 3 28 29 30; \
         5: 31 6 7 32 33; 8: 31 6 7 32 34; 14..15: 17 18 7 1 1; 21: 0 1 1 1 1",
        &[(21, 107, 104)],
    ),
    (
        "0: 19 3 4 1 1; 1: 19 3 35 1 1; 2..3: 13 6 7 26 1; 4..5, 7: 36 6 7 26 1; \
         6: 5 37 7 1 1; 8: 17 18 7 38 1; 9: 17 18 7 39 1; 21: 0 1 1 1 1",
        &[(21, 107, 104)],
    ),
    (
        "0: 19 3 4 1 1; 1: 17 3 40 41 42; 3..5, 7: 13 6 7 26 1; 21: 0 1 1 1 1",
        &[(21, 107, 104)],
    ),
    (
        "0: 43 3 4 1 1; 3: 36 6 44 26 1; 5, 7..9: 13 6 7 26 1; 21: 0 1 1 1 1",
        &[(21, 107, 104)],
    ),
    (
        "0: 43 3 4 1 1; 1: 45 6 46 47 48; 2..12: 49 50 1 51 52; 13..14: 36 6 46 26 1; \
         21: 0 1 1 1 1",
        &[(21, 107, 104)],
    ),
    (
        "0: 43 3 4 1 1; 1: 43 3 35 1 1; 3..6: 13 53 7 1 1; 8..9: 36 6 44 26 1; \
         12..13: 13 6 7 26 1; 15: 54 6 7 55 56; 16..18: 54 6 7 57 56; 21: 0 1 1 1 1",
        &[(21, 107, 104)],
    ),
    (
        "0: 58 3 4 1 1; 1..4: 59 6 7 60 61; 5..12: 13 6 7 62 63; 13..18: 13 53 7 1 1; \
         21: 0 1 1 1 1",
        &[(21, 107, 104)],
    ),
    (
        "0: 58 3 4 1 1; 1: 19 3 35 1 1; 3..5, 14..16: 17 53 7 64 65; 7..11: 13 6 7 66 1; \
         12: 36 6 67 68 69; 21: 0 1 1 1 1",
        &[(21, 107, 104)],
    ),
    (
        "0: 43 3 4 1 1; 1..3: 43 72 7 73 74; 4: 19 3 70 1 1; 5: 19 3 71 1 1; \
         7: 36 6 67 1 1; 8..9, 17..18: 13 6 7 75 1; 11..15: 17 18 7 76 1; 21: 0 1 1 1 1",
        &[(21, 107, 104)],
    ),
    (
        "0: 78 3 4 1 1; 1..5: 5 79 7 80 81; 7..11: 13 6 7 82 83; 12..13: 36 6 7 82 83; \
         15: 19 3 70 1 1; 16: 19 3 71 1 1; 21: 77 1 1 1 1",
        &[(21, 107, 102)],
    ),
    (
        "0: 36 6 4 1 1; 1..2: 13 6 7 1 1; 8..9: 43 3 7 66 1; 11..13: 19 3 7 66 1; \
         15..17: 85 3 7 66 1; 21: 84 1 1 1 1",
        &[(21, 180, 155)],
    ),
    ("", &[]),
    (
        "0: 78 3 4 1 1; 1..2: 17 6 7 86 1; 3..5, 10..12: 87 6 7 88 89; 8..9: 54 6 7 86 1; \
         15: 19 3 90 1 1; 16: 19 3 91 1 1; 21: 77 1 1 1 1",
        &[(21, 107, 102)],
    ),
    (
        "0: 19 3 4 1 1; 1..3, 8..10: 92 6 7 93 94; 4..5, 11..13: 95 6 7 96 97; \
         6: 87 6 7 96 97; 17: 85 3 90 1 1; 21: 0 1 1 1 1",
        &[(21, 107, 104)],
    ),
    (
        "0: 43 3 4 1 1; 1: 19 3 98 99 1; 3: 13 6 46 1 1; 5..6, 8, 12..14: 13 6 46 100 101; \
         9..10: 36 6 46 100 101; 21: 0 1 1 1 1",
        &[(21, 107, 104)],
    ),
    (
        "0: 43 3 4 1 1; 1: 43 3 90 1 1; 3, 7: 104 6 46 102 103; \
         4..6, 8..9, 12..14, 16: 13 6 46 102 103; 10..11, 15, 17..18: 105 6 46 102 103; \
         21: 0 1 1 1 1",
        &[(21, 107, 104)],
    ),
    (
        "0: 5 6 4 1 1; 1..4, 9..10: 36 6 46 109 1; 5..8: 13 6 46 109 1; \
         19: 5 3 110 111 112; 21: 0 1 1 1 1; 22: 106 1 1 107 108",
        &[(21, 107, 104), (22, 125, 73)],
    ),
    (
        "0: 43 3 4 1 1; 1..5: 113 6 46 114 115; 7..8: 36 6 46 116 117; 21: 0 1 1 1 1",
        &[(21, 107, 104)],
    ),
    ("", &[]),
];

/// Episode 2 (`docs/spec/sim/e2-mission-setup.md` rev 1 §3.4, final state per flight; indices
/// into `e2.target_info`). Flight 20 writes no fixed point.
const TARGETS_E2: [(&str, &[Fixed]); 20] = [
    (
        "0: 2 3 4 1 1; 1: 2 3 5 1 1; 3: 6 3 7 8 1; 4: 6 3 9 8 1; 6..8: 10 11 12 13 14; \
         9..13: 15 11 12 13 14; 15, 16: 16 17 12 18 19; 21: 0 1 1 1 1",
        &[(21, 107, 104)],
    ),
    (
        "0: 2 3 4 1 1; 1, 2: 2 3 12 1 1; 4..8: 20 21 12 22 1; 12..18: 15 23 12 22 1; \
         21: 0 1 1 1 1",
        &[(21, 107, 104)],
    ),
    (
        "0: 2 3 4 1 1; 1: 24 25 12 1 1; 3..5, 7, 8: 15 21 12 26 1; 6: 10 21 12 26 1; \
         21: 0 1 1 1 1",
        &[(21, 107, 104)],
    ),
    (
        "0: 6 3 4 1 1; 1..3: 27 23 12 26 1; 8..11: 15 23 12 28 29; 14..16: 20 21 12 30 1; \
         21: 0 1 1 1 1",
        &[(21, 107, 104)],
    ),
    (
        "0: 32 23 4 1 1; 1..3: 27 21 12 33 1; 5..9: 20 21 12 34 35; 11: 32 23 36 30 1; \
         21: 31 1 1 1 1",
        &[(21, 184, 36)],
    ),
    (
        "0: 32 23 4 1 1; 2..11: 10 11 12 39 40; 13: 32 23 41 30 1; 15, 16: 20 21 12 42 43; \
         21: 37 1 1 38 1",
        &[(21, 125, 73)],
    ),
    (
        "0: 2 3 4 1 1; 1: 44 11 45 46 47; 2..12: 48 49 1 50 1; 13, 14: 10 11 45 51 1; \
         21: 0 1 1 1 1",
        &[(21, 107, 104)],
    ),
    (
        "0: 52 3 4 1 1; 2, 3, 5, 6, 8..10: 53 3 12 54 55; 4, 7, 11: 56 3 12 54 55; \
         15, 19: 10 11 12 51 1; 16..18: 15 11 12 51 1; 21: 0 1 1 1 1",
        &[(21, 107, 104)],
    ),
    (
        "0: 57 3 4 1 1; 2, 3: 58 3 12 1 1; 5: 53 11 12 59 60; 8..10, 12..14: 15 11 12 61 62; \
         11, 15: 10 11 12 61 62; 21: 0 1 1 1 1",
        &[(21, 92, 30)],
    ),
    (
        "0: 63 3 4 1 1; 2: 16 3 64 65 66; 3: 2 3 67 68 1; 4, 6..14: 15 11 12 1 1; \
         5: 10 11 12 1 1; 15, 16: 53 11 12 69 1; 21: 0 1 1 1 1",
        &[(21, 92, 30)],
    ),
    (
        "0: 70 3 4 1 1; 2, 6, 11: 73 11 12 71 72; 3: 10 11 12 71 72; \
         4, 5, 7..10, 12: 15 11 12 71 72; 21: 0 1 1 1 1",
        &[(21, 92, 30)],
    ),
    (
        "0: 70 3 4 1 1; 2..4: 74 75 12 76 77; 8..13: 15 11 12 78 79; 21: 0 1 1 1 1",
        &[(21, 92, 30)],
    ),
    (
        "0: 70 3 4 1 1; 2: 70 3 80 1 1; 4..8, 10..12, 14, 15: 15 11 12 81 82; \
         9, 13: 10 11 12 81 82; 21: 0 1 1 1 1",
        &[(21, 92, 30)],
    ),
    (
        "0: 2 3 4 1 1; 2: 2 3 83 1 1; 4..8: 15 11 12 81 82; 10..13: 84 11 12 81 82; \
         21: 0 1 1 1 1",
        &[(21, 92, 30)],
    ),
    (
        "0: 2 3 4 1 1; 2: 70 3 91 92 1; 4, 5: 85 11 12 86 62; 6, 7: 87 11 12 86 62; \
         10..12: 88 17 12 89 90; 21: 0 1 1 1 1",
        &[(21, 92, 30)],
    ),
    (
        "0: 2 3 4 1 1; 2: 16 3 93 94 1; 5, 6: 87 11 12 96 97; 7: 85 11 12 96 97; \
         8..10: 95 11 12 96 97; 21: 0 1 1 1 1",
        &[(21, 92, 30)],
    ),
    (
        "0: 2 3 4 1 1; 2..4: 16 3 45 98 99; 5: 6 3 100 98 99; 6: 6 3 101 98 99; \
         8, 9: 87 11 45 102 103; 10, 11, 14: 85 11 45 102 103; 12, 13: 95 11 45 102 103; \
         21: 0 1 1 1 1",
        &[(21, 92, 30)],
    ),
    (
        "0: 2 3 4 1 1; 2: 6 3 104 1 1; 3: 6 3 105 1 1; 4: 6 3 106 1 1; \
         6..11: 85 11 45 102 107; 21: 0 1 1 1 1",
        &[(21, 92, 30)],
    ),
    (
        "0: 2 3 4 1 1; 2: 10 11 108 109 110; 3..7: 15 11 45 1 1; 21: 0 1 1 1 1",
        &[(21, 92, 30)],
    ),
    (
        "0: 70 3 4 1 1; 2..5: 6 3 45 1 1; 6..8: 2 3 45 1 1; 10, 11: 70 3 45 1 1; \
         12..15: 15 11 45 111 112; 17, 18: 16 3 45 1 1; 19, 20: 53 3 45 1 1",
        &[],
    ),
];

/// Parses one flight's §3.4 line table into (entry, line indices) pairs.
fn parse_lines(spec: &str) -> Vec<(usize, [u8; 5])> {
    let mut out = Vec::new();
    for group in spec.split(';').map(str::trim).filter(|g| !g.is_empty()) {
        let Some((entries, idx)) = group.split_once(':') else {
            continue;
        };
        let mut lines = [0u8; 5];
        for (l, v) in lines.iter_mut().zip(idx.split_whitespace()) {
            *l = v.parse().unwrap_or(0);
        }
        for part in entries.split(',').map(str::trim) {
            let (a, b) = part.split_once("..").unwrap_or((part, part));
            if let (Ok(a), Ok(b)) = (a.parse::<usize>(), b.parse::<usize>()) {
                out.extend((a..=b).map(|e| (e, lines)));
            }
        }
    }
    out
}

/// The F5 entries kept across the flights of one program run (§3.4): selectable flags, map
/// positions and the five info lines per entry (`None` = the zero-length strings of start-up).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetBoard {
    pub flags: [bool; ENTRIES],
    pub pos: [(u8, u8); ENTRIES],
    pub lines: [Option<[u8; 5]>; ENTRIES],
}

impl Default for TargetBoard {
    fn default() -> Self {
        Self {
            flags: [false; ENTRIES],
            pos: [(0, 0); ENTRIES],
            lines: [None; ENTRIES],
        }
    }
}

impl TargetBoard {
    /// `Msn_LoadTargetInfo` at the start of Episode 1 flight `flight` (1..=21): clear the flags
    /// of entries 1..30, write the flight's lines and fixed points; other entries keep their
    /// lines.
    pub fn load(&mut self, flight: u8) {
        self.load_table(&TARGETS, flight);
    }

    /// `Msn_LoadTargetInfo` of `episode` (E2: `td2:1E0B:0DCC`, e2-mission-setup rev 1 §3.4,
    /// §4.5; the lines index `e2.target_info`).
    pub fn load_episode(&mut self, episode: Episode, flight: u8) {
        match episode {
            Episode::E2 => self.load_table(&TARGETS_E2, flight),
            _ => self.load(flight),
        }
    }

    fn load_table(&mut self, table: &[(&str, &[Fixed])], flight: u8) {
        self.flags[1..=LAST_SELECTABLE].fill(false);
        let Some((lines, fixed)) = usize::from(flight)
            .checked_sub(1)
            .and_then(|i| table.get(i))
        else {
            return;
        };
        for (e, l) in parse_lines(lines) {
            if let Some(slot) = self.lines.get_mut(e) {
                *slot = Some(l);
            }
        }
        for &(e, x, y) in *fixed {
            self.flags[e] = true;
            self.pos[e] = (x, y);
        }
    }
}

/// A mission slot as the map sees it on entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShipDot {
    pub class: u8,
    pub tile: (u8, u8),
    /// The slot has a guidance marker (marker ≠ 0).
    pub marked: bool,
}

impl ShipDot {
    /// Shown and selectable: a live ship with a marker (E1).
    fn shown(&self) -> bool {
        self.class != 0xFF && self.marked
    }
}

/// The tactical map (§4.3).
#[derive(Debug, Clone)]
pub struct TacticalMap {
    canvas: Framebuffer,
    /// The map colours (for erasing the cross).
    base: Vec<u8>,
    flags: [bool; ENTRIES],
    pos: [(i32, i32); ENTRIES],
    lines: [Option<[u8; 5]>; ENTRIES],
    player: (i32, i32),
    ships: Vec<ShipDot>,
    sel: usize,
}

/// Radar colour of map tile (x, y) (`k_TileRadarColour`; tiles outside the table or the map
/// draw 0).
fn radar(map: &TileMap, colours: &[u8], x: i32, y: i32) -> u8 {
    let (Ok(x), Ok(y)) = (usize::try_from(x), usize::try_from(y)) else {
        return 0;
    };
    map.get(x, y)
        .and_then(|t| colours.get(usize::from(t)).copied())
        .unwrap_or(0)
}

impl TacticalMap {
    /// Opens the map: `view` is the view tile (viewX, viewY), `ships` the 20 mission slots,
    /// `board` the entries the flight loaded. The DAC stays the flight's.
    pub fn new(
        dac: &td_formats::palette::Palette,
        assets: &FlightScreenAssets,
        map: &TileMap,
        view: (i32, i32),
        ships: &[ShipDot],
        board: &TargetBoard,
    ) -> Self {
        let mut canvas = Framebuffer::new();
        canvas.palette = dac.clone();
        for y in 1..=MAP_TILES {
            for x in 1..=MAP_TILES {
                canvas.put(x, y - 1, radar(map, &assets.radar_colours, x, y));
            }
        }
        let base = canvas.pixels.to_vec();
        for (i, y) in PROMPT_Y.into_iter().enumerate() {
            let text = FlightScreenAssets::text(&assets.map_prompts, i);
            small_text::draw_opaque(&mut canvas, &assets.font, text, (TEXT_X, y), TEXT_INK, 0);
        }
        let mut flags = board.flags;
        let mut pos = board.pos.map(|(x, y)| (i32::from(x), i32::from(y)));
        flags[0] = true;
        pos[0] = (view.0 + 5, view.1 + 3);
        for (s, dot) in ships.iter().enumerate().take(20) {
            flags[s + 1] = dot.shown();
            pos[s + 1] = (i32::from(dot.tile.0), i32::from(dot.tile.1));
        }
        let mut m = Self {
            canvas,
            base,
            flags,
            pos,
            lines: board.lines,
            player: (view.0 + 5, view.1 + 2),
            ships: ships.to_vec(),
            sel: 0,
        };
        m.iterate(assets, false);
        m
    }

    /// The picture on screen.
    pub fn canvas(&self) -> &Framebuffer {
        &self.canvas
    }

    /// The selected entry.
    pub fn selected(&self) -> usize {
        self.sel
    }

    /// One pass of the loop body (§4.3): dots, the Tab step, the cross and the lines.
    fn iterate(&mut self, assets: &FlightScreenAssets, tab: bool) {
        self.canvas.put(self.player.0, self.player.1, PLAYER_COLOUR);
        for dot in self.ships.iter().filter(|d| d.shown()) {
            let (x, y) = (i32::from(dot.tile.0), i32::from(dot.tile.1));
            self.canvas
                .put(x, y - 1, dot.class.wrapping_add(SHIP_COLOUR_BASE));
        }
        if tab {
            let (x, y) = self.pos[self.sel];
            for (dx, dy) in CROSS {
                let (px, py) = (x + dx, y + dy);
                let base = usize::try_from(py)
                    .ok()
                    .and_then(|py| Some(py * WIDTH + usize::try_from(px).ok()?))
                    .and_then(|i| self.base.get(i).copied())
                    .unwrap_or(0);
                self.canvas.put(px, py, base);
            }
            let next = (self.sel + 1..=LAST_SELECTABLE).find(|&e| self.flags[e]);
            self.sel = next
                .or_else(|| (0..=LAST_SELECTABLE).find(|&e| self.flags[e]))
                .unwrap_or(0);
        }
        if self.sel == 0 || self.flags[self.sel] {
            let (x, y) = self.pos[self.sel];
            for (dx, dy) in CROSS {
                self.canvas.put(x + dx, y + dy, CROSS_COLOUR);
            }
            if let Some(lines) = self.lines[self.sel] {
                for (idx, y) in lines.into_iter().zip(INFO_Y) {
                    let text = FlightScreenAssets::text(&assets.target_info, usize::from(idx));
                    let text: String = text.chars().take(LINE_LEN).collect();
                    small_text::draw_opaque(
                        &mut self.canvas,
                        &assets.font,
                        &text,
                        (TEXT_X, y),
                        TEXT_INK,
                        0,
                    );
                }
            }
        }
    }

    /// Takes this retrace's key presses in order, as `ReadKey` would return them.
    pub fn tick(&mut self, assets: &FlightScreenAssets, input: &TickInput) -> ModalStep {
        for &key in &input.pressed {
            match key {
                Key::ESCAPE | KEY_F5 => return ModalStep::Close { abort: false },
                // Modifier keys put nothing in the BIOS buffer.
                Key::LCTRL | Key::LSHIFT | Key::LALT | Key::RCTRL | Key::RSHIFT | Key::RALT => {}
                k => self.iterate(assets, k == Key::TAB),
            }
        }
        ModalStep::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ranges_and_lists() {
        let v = parse_lines("0: 2 3 4 1 1; 5..7, 9: 5 6 7 11 12");
        let entries: Vec<usize> = v.iter().map(|p| p.0).collect();
        assert_eq!(entries, [0, 5, 6, 7, 9]);
        assert_eq!(v[4].1, [5, 6, 7, 11, 12]);
    }

    #[test]
    fn board_load_keeps_unwritten_entries() {
        let mut b = TargetBoard::default();
        b.load(1);
        // Test vector 5: entry 0 lines 2 3 4 1 1, entry 21 a fixed point at (107, 104).
        assert_eq!(b.lines[0], Some([2, 3, 4, 1, 1]));
        assert_eq!(b.lines[8], Some([13, 6, 7, 14, 15]));
        assert!(b.flags[21] && b.pos[21] == (107, 104));
        assert!(!b.flags[1]);
        b.load(2);
        assert_eq!(b.lines[12], Some([17, 18, 7, 1, 1]), "kept from flight 1");
        assert_eq!(b.lines[1], Some([13, 6, 7, 20, 1]));
        b.load(14);
        assert_eq!(
            b.lines[0],
            Some([19, 3, 4, 1, 1]),
            "flight 14 writes nothing"
        );
        b.load(19);
        assert!(b.flags[22] && b.pos[22] == (125, 73));
        b.load(20);
        assert!(!b.flags[22], "flags 1..30 cleared at each load");
        // Every index is a valid e1.target_info line (118 strings).
        for f in 1..=21 {
            for (_, l) in parse_lines(TARGETS[f - 1].0) {
                assert!(l.iter().all(|&i| i < 118), "flight {f}");
            }
        }
    }

    #[test]
    fn e2_board() {
        let mut b = TargetBoard::default();
        b.load_episode(Episode::E2, 1);
        assert_eq!(b.lines[16], Some([16, 17, 12, 18, 19]));
        assert!(b.flags[21] && b.pos[21] == (107, 104));
        b.load_episode(Episode::E2, 5);
        assert_eq!(b.pos[21], (184, 36));
        assert_eq!(b.lines[21], Some([31, 1, 1, 1, 1]));
        b.load_episode(Episode::E2, 20);
        assert!(!b.flags[21], "flight 20 writes no fixed point");
        assert_eq!(
            b.lines[16],
            Some([16, 17, 12, 18, 19]),
            "kept from flight 1"
        );
        // Every index is a valid e2.target_info line (113 strings); every slot 1..20 that a
        // flight writes, and entry 0, has five lines.
        for f in 1..=20 {
            let v = parse_lines(TARGETS_E2[f - 1].0);
            assert!(v.iter().any(|(e, _)| *e == 0), "flight {f}");
            for (e, l) in v {
                assert!(e <= 21 && l.iter().all(|&i| i < 113), "flight {f}");
            }
        }
    }
}
