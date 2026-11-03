//! The in-flight instruments (`UI_DrawInstruments`, tick-order step 19): bars, missile lamp and
//! column, radar, weapon icon, message ticker, F6 indicator, compass lamps and guidance line.
//!
//! Spec: `docs/spec/render/hud.md` rev 2 (section numbers below are its own). Fidelity: T0 for
//! geometry, positions and colours. CONFIRMED against C-0002 f3543 and C-0010 by
//! `tests/hud_parity.rs` (bars, missile column, radar, weapon icon, ticker window, lamps,
//! guidance dots); the F6 frames are LIKELY (no capture shows a transmission yet).
//!
//! The original draws the instruments straight into VGA rows 0..31, which the split screen
//! shows below the playfield; the bar is never redrawn as a whole, so it keeps what earlier
//! ticks drew. [`Hud`] keeps that bar, the off-screen ticker strip (VGA rows 735..742) and the
//! palette entries the instruments set. Call [`Hud::step`] once per tick with the values the
//! original reads at step 19, and [`Hud::draw`] after the playfield.

use std::cmp::Ordering;

use td_formats::font::{Font, FontKind};
use td_formats::map::TileMap;
use td_formats::palette::Rgb6;
use td_sim::Flight;
use td_sim::real48::Real48;

use crate::flight::{HUD_H, HUD_W, HUD_Y, PLAYFIELD, ViewPos};
use crate::framebuffer::{Canvas, Framebuffer, WIDTH};
use crate::small_text::{self, Mode};

/// Weapon icon size and HUD position (§3.3, §4.9).
pub const ICON_W: usize = 36;
pub const ICON_H: usize = 18;
pub const ICON_POS: (usize, usize) = (179, 7);
/// Number of icons in `td.wea`.
pub const ICONS: usize = 6;
/// F6 indicator frame size, frame count and HUD position (§4.8).
pub const FAC: usize = 28;
pub const FAC_FRAMES: usize = 5;
pub const FAC_POS: (usize, usize) = (220, 2);
/// Radar size and HUD position (§4.6).
pub const RADAR: usize = 20;
pub const RADAR_POS: (usize, usize) = (148, 6);
/// Ticker window position and size (§4.7).
pub const TICKER_POS: (usize, usize) = (256, 13);
pub const TICKER_W: usize = 56;
pub const TICKER_H: usize = 5;
/// The ticker strip: VGA rows 735..742 of 352 pixels (`asset-load-sites.md` §1.1).
pub const STRIP_Y: i32 = 735;
pub const STRIP_ROWS: usize = 8;
pub const VGA_ROW: usize = 352;
/// Ink of objectives, jurisdiction warnings and most mission messages, and of transmissions
/// (mission-logic §3.2).
pub const INK_MESSAGE: u8 = 37;
pub const INK_TRANSMISSION: u8 = 42;

/// Palette entries the instruments animate (§3.2): the bars' background, the missile lamp, and
/// the compass lamps in the order E, NE, N, NW, W, SW, S, SE.
pub const BAR_BACKGROUND: u8 = 0x90;
pub const MISSILE_LAMP: u8 = 0x26;
pub const LAMPS: [u8; 8] = [0x31, 0xC1, 0x86, 0x85, 0x2F, 0x2E, 0x30, 0x24];
const E: usize = 0;
const NE: usize = 1;
const N: usize = 2;
const NW: usize = 3;
const W: usize = 4;
const SW: usize = 5;
const S: usize = 6;
const SE: usize = 7;

/// Radar colours (§3.4).
const RADAR_SHIP: u8 = 0x55;
const RADAR_SHOT: u8 = 0x22;
const RADAR_PLAYER: u8 = 0x46;

/// Guidance markers (§3.4) and their dot colours.
pub const MARKER_TARGET: u8 = 0x32;
pub const MARKER_FRIEND: u8 = 0x39;
pub const MARKER_HOME: u8 = 0x3D;
const DOTS_TARGET: [u8; 6] = [0xB0, 0xA5, 0xF9, 0x7A, 0x5C, 0x5C];
const DOTS_FRIEND: [u8; 6] = [0xB2, 0xA9, 0x98, 0x82, 0x66, 0x66];
const DOTS_HOME: [u8; 6] = [0xB3, 0xAB, 0x9C, 0xD1, 0x50, 0x50];

/// Guidance constants, Real48 bytes in memory order (§3.5).
const SLOPE_LO: Real48 = Real48::from_bytes([0x7F, 0x17, 0xD9, 0xCE, 0xF7, 0x53]);
const SLOPE_HI: Real48 = Real48::from_bytes([0x82, 0xD7, 0xA3, 0x70, 0x3D, 0x1A]);
const NEG_SLOPE_LO: Real48 = Real48::from_bytes([0x7F, 0x17, 0xD9, 0xCE, 0xF7, 0xD3]);
const NEG_SLOPE_HI: Real48 = Real48::from_bytes([0x82, 0xD7, 0xA3, 0x70, 0x3D, 0x9A]);
const HALF_PI: Real48 = Real48::from_bytes([0x81, 0x21, 0xA2, 0xDA, 0x0F, 0x49]);
const THREE_HALF_PI: Real48 = Real48::from_bytes([0x83, 0x99, 0xF9, 0xE3, 0xCB, 0x16]);
const PI: Real48 = Real48::from_bytes([0x82, 0x21, 0xA2, 0xDA, 0x0F, 0x49]);

/// A data file of the wrong size.
#[derive(Debug, thiserror::Error)]
pub enum HudError {
    #[error("{0} has an unexpected size")]
    Size(&'static str),
}

/// The files and tables the instruments draw from.
#[derive(Debug, Clone)]
pub struct HudAssets {
    win: Vec<u8>,
    weapons: Vec<u8>,
    fac: Vec<u8>,
    font: Font,
    radar_colours: Vec<u8>,
}

impl HudAssets {
    /// `td.win`, `td.wea`, `td.fac`, `td.lt2` and the exe table `hud.radar_tile_colours`.
    /// As the loader does, colour 38 becomes 39 in `td.fac` frames 0..3 (FORMATS §12).
    pub fn new(
        win: Vec<u8>,
        weapons: Vec<u8>,
        mut fac: Vec<u8>,
        lt2: &[u8],
        radar_colours: Vec<u8>,
    ) -> Result<Self, HudError> {
        let check = |ok: bool, name| {
            if ok {
                Ok(())
            } else {
                Err(HudError::Size(name))
            }
        };
        check(win.len() == HUD_W * HUD_H, "td.win")?;
        check(weapons.len() == ICONS * ICON_W * ICON_H, "td.wea")?;
        check(fac.len() == FAC_FRAMES * FAC * FAC, "td.fac")?;
        check(radar_colours.len() == 71, "hud.radar_tile_colours")?;
        let font = Font::parse(lt2, FontKind::Small).map_err(|_| HudError::Size("td.lt2"))?;
        for c in &mut fac[..4 * FAC * FAC] {
            if *c == 38 {
                *c = 39;
            }
        }
        Ok(Self {
            win,
            weapons,
            fac,
            font,
            radar_colours,
        })
    }

    /// Background of opaque ticker text: byte 650 of the loaded `td.lt2` block, the first pixel
    /// of glyph 26 (text.md §2; the flight never clears it, unlike the menu screens).
    pub fn ticker_background(&self) -> u8 {
        self.font.glyph('{').map_or(0, |g| g[0])
    }
}

/// What the instruments read of one mission slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlotMark {
    /// Class code; FFh = no ship.
    pub class: u8,
    pub tile_x: u8,
    pub tile_y: u8,
    /// Guidance marker: 0, 32h target, 39h friend.
    pub marker: u8,
}

impl SlotMark {
    pub const EMPTY: SlotMark = SlotMark {
        class: 0xFF,
        tile_x: 0,
        tile_y: 0,
        marker: 0,
    };
}

/// The game state `UI_DrawInstruments` reads at tick step 19.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HudState {
    /// Flight number 1..=21 (home point, §3.6).
    pub flight: u8,
    /// Ticks since flight start (`missionTicks`, the low-armour pulse).
    pub ticks: u32,
    /// The view the tile window was drawn from.
    pub view: ViewPos,
    pub armour: i8,
    pub shields: i8,
    pub missiles: u8,
    pub missile_cooldown: u8,
    /// `g_Weapon` 1..=6.
    pub weapon: u8,
    /// `g_PaletteMode` 0, 1, 2.
    pub palette_mode: u8,
    /// Slots 1..=20.
    pub slots: [SlotMark; 20],
    /// Projectile slots 0..=50 as (tile X, tile Y); tile X FFh = free.
    pub shots: [(u8, u8); 51],
    /// Element 21 of `g_SlotMarker`: the home point's marker.
    pub home_marker: u8,
    /// `g_MissionPhase` (only flight 19's home point depends on it).
    pub phase: u8,
    /// Strip position the ticker window copies this tick (`UI_HudTickerBlit(pos)`), if it does.
    pub ticker_blit: Option<u16>,
    /// `g_CommIndicatorPhase` after this tick's step (0 = no transmission pending).
    pub comm_phase: u8,
}

impl HudState {
    /// The state of a flight after a tick.
    ///
    /// The view and the shots are those the tick drew (steps 4 and 6); the ship slots are read
    /// after the tick (nothing changes them after step 18). Armour, shields, missiles,
    /// cooldown, weapon and palette mode are read after the whole tick; the original draws
    /// them at step 19, before steps 27 and 33 change them, so they can lead it by one tick.
    /// Mission logic (ticker, transmissions, home marker, phase) is not in `Flight` yet; those
    /// fields are left at their flight-start values.
    pub fn from_flight(f: &Flight) -> Self {
        let v = f.frame.view;
        let mut slots = [SlotMark::EMPTY; 20];
        for (m, s) in slots.iter_mut().zip(f.ships.0.iter()) {
            *m = SlotMark {
                class: s.class,
                tile_x: s.tile_x,
                tile_y: s.tile_y,
                marker: s.marker,
            };
        }
        let mut shots = [(0xFF, 0); 51];
        for (d, s) in shots.iter_mut().zip(f.drawn.shots.0.iter()) {
            *d = (s.tile_x, s.tile_y);
        }
        Self {
            flight: f.number,
            ticks: f.ticks,
            view: ViewPos {
                tile_x: i32::from(v.tile_x),
                tile_y: i32::from(v.tile_y),
                sub_x: i32::from(v.sub_x),
                sub_y: i32::from(v.sub_y),
            },
            armour: f.player.armour,
            shields: f.player.shields,
            missiles: f.missiles,
            missile_cooldown: f.missile_cooldown,
            weapon: f.weapon,
            palette_mode: f.palette_mode,
            slots,
            shots,
            home_marker: if f.number == 19 {
                MARKER_TARGET
            } else {
                MARKER_HOME
            },
            phase: 1,
            ticker_blit: None,
            comm_phase: 0,
        }
    }
}

/// Which glyph routine draws a ticker text (§4.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TickerFont {
    /// `Txt_FlightDrawStringOpaque`: objectives and jurisdiction warnings (text.md advances).
    Flight,
    /// The mission unit's copy (`2404:0000`): digits other than 6 advance 0.
    Mission,
}

/// The home point (§3.6): the guidance target when no marked ship is live.
pub fn home_point(flight: u8, phase: u8) -> (i16, i16) {
    match flight {
        12 | 15 => (107, 102),
        13 => (180, 155),
        19 if phase < 100 => (123, 72),
        _ => (107, 104),
    }
}

/// The off-screen ticker strip, addressed like VGA memory: a pixel past x = 351 lands in the
/// next row.
struct Strip<'a>(&'a mut [u8]);

impl Canvas for Strip<'_> {
    fn put(&mut self, x: i32, y: i32, color: u8) {
        let (Ok(x), Ok(row)) = (usize::try_from(x), usize::try_from(y - STRIP_Y)) else {
            return;
        };
        if let Some(p) = self.0.get_mut(row * VGA_ROW + x) {
            *p = color;
        }
    }
}

/// The instruments' persistent display state.
#[derive(Debug, Clone)]
pub struct Hud {
    /// VGA rows 0..31, visible part (320 × 32).
    bar: Box<[u8]>,
    /// VGA rows 735..742, 352 pixels each.
    strip: Box<[u8]>,
    /// Red of palette entry 90h in the flight palette (the low-armour pulse keeps it).
    bar_red: u8,
    /// DAC entries set by the last step, as stored (the 6-bit DAC shows `v & 63`).
    entries: Vec<(u8, [u8; 3])>,
    /// Guidance pixels (screen x, y, colour) in drawing order.
    dots: Vec<(i32, i32, u8)>,
}

impl Hud {
    /// Mission start (§4.1): `td.win` in the bar; compass lamps and entry 90h black.
    pub fn new(assets: &HudAssets) -> Self {
        let mut entries: Vec<_> = LAMPS.iter().map(|&e| (e, [0; 3])).collect();
        entries.push((MISSILE_LAMP, [0; 3]));
        entries.push((BAR_BACKGROUND, [0; 3]));
        Self {
            bar: assets.win.clone().into_boxed_slice(),
            strip: vec![0; STRIP_ROWS * VGA_ROW].into_boxed_slice(),
            bar_red: 0,
            entries,
            dots: Vec::new(),
        }
    }

    /// Draws a message into the ticker strip (§4.7): opaque, pen (0, 735), over whatever the
    /// strip held, without clearing it first. The caller restarts the ticker.
    pub fn post(&mut self, assets: &HudAssets, text: &str, ink: u8, font: TickerFont) {
        let mode = Mode::Opaque {
            background: assets.ticker_background(),
        };
        let mut strip = Strip(&mut self.strip);
        let mut offset = 1;
        for ch in text.chars() {
            if matches!(ch, 'x' | 'y') {
                offset -= 1;
            }
            small_text::draw_glyph(&mut strip, &assets.font, ch, (offset, STRIP_Y), ink, mode);
            offset += match font {
                TickerFont::Mission if ch.is_ascii_digit() && ch != '6' => 0,
                _ => small_text::advance(ch),
            };
        }
    }

    /// One tick of `UI_DrawInstruments` (§4.2), in the original order.
    pub fn step(&mut self, assets: &HudAssets, map: &TileMap, s: &HudState) {
        self.entries.clear();
        self.guidance(s);
        if s.armour <= 6 {
            self.bar_red = (8 - (s.ticks % 8) as u8) * 8;
        }
        self.draw_bar(23, s.armour, s.palette_mode);
        self.draw_bar(11, s.shields, s.palette_mode);
        let lamp = if s.missiles == 0 {
            [0; 3]
        } else {
            [4u8.wrapping_sub(s.missile_cooldown).wrapping_mul(15), 5, 8]
        };
        self.entries.push((MISSILE_LAMP, lamp));
        self.entries.push((BAR_BACKGROUND, [self.bar_red, 0, 0]));
        self.missile_column(s.missiles);
        self.radar(assets, map, s);
        // Drawn at flight start and on every weapon change (§4.9); nothing else writes there,
        // so drawing it every tick gives the same picture.
        self.weapon_icon(assets, s.weapon);
        if let Some(pos) = s.ticker_blit {
            self.ticker_blit(pos);
        }
        self.indicator(assets, s.comm_phase);
    }

    /// Draws the guidance dots into the playfield, the bar into rows 160..191 and the animated
    /// palette entries into the palette. Call after the playfield is complete.
    pub fn draw(&self, fb: &mut Framebuffer) {
        for &(x, y, c) in &self.dots {
            if (0..PLAYFIELD.w).contains(&x) && (PLAYFIELD.y..PLAYFIELD.h).contains(&y) {
                fb.put(x, y, c);
            }
        }
        let start = HUD_Y as usize * WIDTH;
        fb.pixels[start..start + self.bar.len()].copy_from_slice(&self.bar);
        for &(e, [r, g, b]) in &self.entries {
            fb.palette.colors[usize::from(e)] = Rgb6 {
                r: r & 63,
                g: g & 63,
                b: b & 63,
            };
        }
    }

    /// The bar as drawn so far (320 × 32).
    pub fn bar(&self) -> &[u8] {
        &self.bar
    }

    /// Palette entries set by the last step, as stored (the DAC shows `v & 63`).
    pub fn entries(&self) -> &[(u8, [u8; 3])] {
        &self.entries
    }

    /// Guidance pixels of the last step (screen x, y, colour), in drawing order.
    pub fn dots(&self) -> &[(i32, i32, u8)] {
        &self.dots
    }

    fn put(&mut self, x: usize, y: usize, c: u8) {
        self.bar[y * HUD_W + x] = c;
    }

    /// Shield or armour bar (§4.4): segment J covers pixels 2J + 6 and 2J + 7 of 4 rows.
    fn draw_bar(&mut self, row: usize, v: i8, palette_mode: u8) {
        let v = i32::from(v);
        let segment = |hud: &mut Self, j: i32, (l, r): (u8, u8)| {
            let x = (2 * j + 6) as usize;
            for y in row..row + 4 {
                hud.put(x, y, l);
                hud.put(x + 1, y, r);
            }
        };
        let red_mode = palette_mode == 2;
        if v > 0 {
            for j in 0..=v.min(6) {
                segment(self, j, (0xB0, 0x7A));
            }
            let mid = if red_mode { (0xB0, 0xA5) } else { (0x48, 0x96) };
            for j in 7..=v.min(12) {
                segment(self, j, mid);
            }
            let top = if red_mode { (0xB0, 0xA5) } else { (0xB2, 0xA9) };
            for j in 13..=v.min(28) {
                segment(self, j, top);
            }
        }
        if (0..28).contains(&v) {
            for j in (if v == 0 { 0 } else { v + 1 })..=28 {
                segment(self, j, (BAR_BACKGROUND, BAR_BACKGROUND));
            }
        }
    }

    /// Missile column (§4.5): `Fill4x2` blocks from the bottom up.
    fn missile_column(&mut self, missiles: u8) {
        let fill = |hud: &mut Self, i: u8, c: u8| {
            let y = usize::from(28 - i);
            for yy in y..y + 2 {
                for x in 100..104 {
                    hud.put(x, yy, c);
                }
            }
        };
        let lit = |i: u8| MISSILE_LAMP + i % 2;
        match missiles {
            0 => (0..=26).for_each(|i| fill(self, i, 0)),
            1..=26 => {
                (0..=missiles).for_each(|i| fill(self, i, lit(i)));
                (missiles + 1..=26).for_each(|i| fill(self, i, 0));
            }
            _ => (0..=26).for_each(|i| fill(self, i, lit(i))),
        }
    }

    /// Radar (§4.6): map, ships, shots, player; ships and shots one pixel east (OB-019).
    fn radar(&mut self, assets: &HudAssets, map: &TileMap, s: &HudState) {
        let (vx, vy) = (s.view.tile_x, s.view.tile_y);
        let (x0, y0) = RADAR_POS;
        for r in 0..RADAR {
            for k in 0..RADAR {
                let (tx, ty) = (vx - 6 + k as i32, vy - 6 + r as i32);
                let tile = usize::try_from(tx)
                    .ok()
                    .zip(usize::try_from(ty).ok())
                    .and_then(|(x, y)| map.get(x, y));
                let c = tile
                    .and_then(|t| assets.radar_colours.get(usize::from(t)))
                    .copied()
                    .unwrap_or(0);
                self.put(x0 + k, y0 + r, c);
            }
        }
        let plot = |hud: &mut Self, (tx, ty): (u8, u8), c: u8| {
            let k = i32::from(tx) - vx + 7;
            let r = i32::from(ty) - vy + 6;
            if (0..RADAR as i32).contains(&k) && (0..RADAR as i32).contains(&r) {
                hud.put(x0 + k as usize, y0 + r as usize, c);
            }
        };
        for m in s.slots.iter().filter(|m| m.class != 0xFF) {
            plot(self, (m.tile_x, m.tile_y), RADAR_SHIP);
        }
        for &shot in s.shots[1..].iter().filter(|s| s.0 != 0xFF) {
            plot(self, shot, RADAR_SHOT);
        }
        self.put(x0 + 11, y0 + 9, RADAR_PLAYER);
    }

    /// Weapon icon `weapon` (1..=6) at (179, 7) (§4.9).
    fn weapon_icon(&mut self, assets: &HudAssets, weapon: u8) {
        if !(1..=ICONS as u8).contains(&weapon) {
            return;
        }
        let icon = &assets.weapons[usize::from(weapon - 1) * ICON_W * ICON_H..];
        for row in 0..ICON_H {
            let dst = (ICON_POS.1 + row) * HUD_W + ICON_POS.0;
            self.bar[dst..dst + ICON_W].copy_from_slice(&icon[row * ICON_W..(row + 1) * ICON_W]);
        }
    }

    /// `UI_HudTickerBlit(pos)` (§4.7): strip x 4·pos.., rows 736..740 to HUD (256..311, 13..17).
    fn ticker_blit(&mut self, pos: u16) {
        for r in 0..TICKER_H {
            for i in 0..TICKER_W {
                let src = (1 + r) * VGA_ROW + 4 * usize::from(pos) + i;
                let c = self.strip.get(src).copied().unwrap_or(0);
                self.put(TICKER_POS.0 + i, TICKER_POS.1 + r, c);
            }
        }
    }

    /// F6 indicator (§4.8): frame `phase` for 1..4, `8 − phase` for 5..7, frame 0 (the blank
    /// lamp, identical to that part of `td.win`) when no transmission is pending.
    fn indicator(&mut self, assets: &HudAssets, phase: u8) {
        let frame = match phase {
            0..=4 => usize::from(phase),
            5..=7 => usize::from(8 - phase),
            _ => return,
        };
        let src = &assets.fac[frame * FAC * FAC..(frame + 1) * FAC * FAC];
        for row in 0..FAC {
            let dst = (FAC_POS.1 + row) * HUD_W + FAC_POS.0;
            self.bar[dst..dst + FAC].copy_from_slice(&src[row * FAC..(row + 1) * FAC]);
        }
    }

    /// Compass lamps and guidance dots (§4.3).
    fn guidance(&mut self, s: &HudState) {
        let p = ((s.view.tile_x + 5) as i16, (s.view.tile_y + 3) as i16);
        let home = home_point(s.flight, s.phase);
        let live = || {
            s.slots
                .iter()
                .filter(|m| m.class != 0xFF && m.marker != 0)
                .map(|m| ((i16::from(m.tile_x), i16::from(m.tile_y)), m.marker))
        };
        let mut red = [0u16; 8];
        let mut blue = [0u8; 8];
        for (t, _) in live() {
            red[sector(p, t)] += 10;
        }
        let home_sector = sector(p, home);
        blue[home_sector] = blue[home_sector].wrapping_add(63);
        for (d, &entry) in LAMPS.iter().enumerate() {
            let r = if red[d] == 0 {
                0
            } else {
                (red[d] + 20).min(63)
            };
            self.entries.push((entry, [r as u8, 0, blue[d]]));
        }

        let (target, marker) = live().next().unwrap_or((home, s.home_marker));
        let theta = angle(p, target, s.view.tile_y as i16);
        let (cos, sin) = (theta.cos(), theta.sin());
        let round = |v: Real48, r: i32| {
            v.checked_mul(Real48::from_i32(r))
                .and_then(Real48::round)
                .unwrap_or(0)
        };
        let pos: Vec<(i32, i32)> = (0..6)
            .map(|j| (160 + round(cos, j + 20), 85 + round(sin, j + 20)))
            .collect();
        self.dots.clear();
        for &(x, y) in &pos {
            self.dots
                .extend([(x + 1, y, 0), (x + 1, y + 1, 0), (x, y + 1, 0)]);
        }
        let colours = match marker {
            MARKER_TARGET => DOTS_TARGET,
            MARKER_FRIEND => DOTS_FRIEND,
            MARKER_HOME if s.palette_mode == 2 => {
                // The flight-7 phase rule at this point is mission logic (mission-logic §5.7).
                let entry = LAMPS[home_sector];
                self.entries.push((entry, [0, 0, 63]));
                [entry; 6]
            }
            MARKER_HOME => DOTS_HOME,
            _ => return,
        };
        self.dots
            .extend(pos.iter().zip(colours).map(|(&(x, y), c)| (x, y, c)));
    }
}

/// Real48 comparisons.
fn lt(a: Real48, b: Real48) -> bool {
    a.compare(b) == Ordering::Less
}
fn gt(a: Real48, b: Real48) -> bool {
    a.compare(b) == Ordering::Greater
}

/// `Real(a) / Real(b)` for 16-bit integers, `b ≠ 0` (cannot overflow).
fn ratio(a: i16, b: i16) -> Real48 {
    Real48::from_i32(a.into())
        .checked_div(Real48::from_i32(b.into()))
        .unwrap_or(Real48::ZERO)
}

/// Compass sector of `t` seen from `p` (§4.3): the five Real48 tests in the original order.
fn sector(p: (i16, i16), t: (i16, i16)) -> usize {
    let north = t.1 < p.1;
    let vertical = if north { N } else { S };
    if t.0 == p.0 {
        return vertical;
    }
    let east = t.0 > p.0;
    let q = ratio(t.1.wrapping_sub(p.1), t.0.wrapping_sub(p.0));
    let mut s = E;
    if lt(q, SLOPE_LO) && gt(q, NEG_SLOPE_LO) {
        s = if east { E } else { W };
    }
    if !lt(q, SLOPE_LO) && !gt(q, SLOPE_HI) {
        s = if east { SE } else { NW };
    }
    if gt(q, SLOPE_HI) {
        s = vertical;
    }
    if lt(q, NEG_SLOPE_HI) {
        s = vertical;
    }
    if !gt(q, NEG_SLOPE_LO) && !lt(q, NEG_SLOPE_HI) {
        s = if east { NE } else { SW };
    }
    s
}

/// Angle θ from the player tile `p` to `t` (§4.3); `view_y` is the view's tile row.
fn angle(p: (i16, i16), t: (i16, i16), view_y: i16) -> Real48 {
    let dxp = p.0.wrapping_sub(t.0);
    let dyp = p.1.wrapping_sub(t.1);
    if t.0 == p.0 {
        // `viewY − ty + 2`, not `+ 3` (OB-020).
        return if view_y.wrapping_sub(t.1).wrapping_add(2) > 0 {
            THREE_HALF_PI
        } else {
            HALF_PI
        };
    }
    let a = ratio(dyp, dxp).arctan();
    if dxp < 0 {
        a
    } else {
        a.checked_add(PI).unwrap_or(a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assets() -> HudAssets {
        let mut wea = vec![0u8; ICONS * ICON_W * ICON_H];
        for (i, c) in wea.chunks_mut(ICON_W * ICON_H).enumerate() {
            c.fill(100 + i as u8);
        }
        let mut lt2 = vec![0u8; 950];
        lt2[650] = 178;
        for b in &mut lt2[..26 * 25] {
            *b = 1;
        }
        HudAssets::new(
            vec![7; HUD_W * HUD_H],
            wea,
            vec![38; FAC_FRAMES * FAC * FAC],
            &lt2,
            (0..71).collect(),
        )
        .unwrap()
    }

    /// C-0002 f3543 (hud.md §6 vectors 2–5): view (102, 104, 32, 16), slots 1 and 2 live and
    /// marked at (124, 75) and (125, 75).
    fn state() -> HudState {
        let mut slots = [SlotMark::EMPTY; 20];
        slots[0] = SlotMark {
            class: 0,
            tile_x: 124,
            tile_y: 75,
            marker: MARKER_TARGET,
        };
        slots[1] = SlotMark {
            tile_x: 125,
            ..slots[0]
        };
        HudState {
            flight: 1,
            ticks: 70,
            view: ViewPos {
                tile_x: 102,
                tile_y: 104,
                sub_x: 32,
                sub_y: 16,
            },
            armour: 25,
            shields: 25,
            missiles: 4,
            missile_cooldown: 0,
            weapon: 2,
            palette_mode: 0,
            slots,
            shots: [(0xFF, 0); 51],
            home_marker: MARKER_HOME,
            phase: 1,
            ticker_blit: None,
            comm_phase: 0,
        }
    }

    fn map() -> TileMap {
        TileMap::parse_episode(&vec![16u8; 201 * 201]).unwrap()
    }

    #[test]
    fn guidance_dots_and_lamps_match_the_capture_vectors() {
        let a = assets();
        let mut hud = Hud::new(&a);
        hud.step(&a, &map(), &state());
        // Vector 4: the final colour of each dot position.
        let mut fb = Framebuffer::new();
        fb.clear(1);
        hud.draw(&mut fb);
        for (x, y, c) in [
            (169, 67, 0xB0),
            (170, 66, 0xF9),
            (171, 65, 0x7A),
            (171, 64, 0x5C),
            (172, 63, 0x5C),
        ] {
            assert_eq!(fb.get(x, y), Some(c), "dot at ({x}, {y})");
        }
        assert_eq!(fb.get(173, 63), Some(0), "shadow right of the last dot");
        // Vector 3: NE lamp red 40, N lamp blue 63 (home), the rest black; missile lamp.
        let e = |n| hud.entries().iter().rev().find(|x| x.0 == n).unwrap().1;
        assert_eq!(e(0xC1), [40, 0, 0]);
        assert_eq!(e(0x86), [0, 0, 63]);
        for n in [0x31, 0x85, 0x2F, 0x2E, 0x30, 0x24] {
            assert_eq!(e(n), [0, 0, 0]);
        }
        assert_eq!(e(MISSILE_LAMP), [60, 5, 8]);
        assert_eq!(e(BAR_BACKGROUND), [0, 0, 0]);
    }

    #[test]
    fn sectors_follow_the_slope_tests() {
        let p = (107, 107);
        assert_eq!(sector(p, (107, 100)), N);
        assert_eq!(sector(p, (107, 107)), S);
        assert_eq!(sector(p, (117, 107)), E);
        assert_eq!(sector(p, (97, 108)), W);
        assert_eq!(sector(p, (110, 110)), SE);
        assert_eq!(sector(p, (104, 104)), NW);
        assert_eq!(sector(p, (110, 104)), NE);
        assert_eq!(sector(p, (104, 110)), SW);
        // q = 0.4 < 0.414 (E) and q = 2.5 > 2.41 (S).
        assert_eq!(sector(p, (117, 111)), E);
        assert_eq!(sector(p, (109, 112)), S);
    }

    #[test]
    fn same_column_uses_the_plus_two_test() {
        // Target one row above the player tile gets the "down" angle (OB-020).
        let view_y = 104;
        assert_eq!(angle((107, 107), (107, 106), view_y), HALF_PI);
        assert_eq!(angle((107, 107), (107, 105), view_y), THREE_HALF_PI);
    }

    #[test]
    fn bars_column_radar_icon() {
        let a = assets();
        let mut hud = Hud::new(&a);
        let mut s = state();
        s.armour = 3;
        s.shields = 8;
        s.missiles = 1;
        s.missile_cooldown = 3;
        s.shots[1] = (103, 104);
        hud.step(&a, &map(), &s);
        let px = |x: usize, y: usize| hud.bar()[y * HUD_W + x];
        // Armour 3: J = 0..3 lit, 4..28 background.
        assert_eq!((px(12, 23), px(13, 26)), (0xB0, 0x7A));
        assert_eq!((px(14, 23), px(63, 26)), (0x90, 0x90));
        // Shields 8: J 7, 8 in the middle colours.
        assert_eq!((px(20, 11), px(23, 14)), (0x48, 0x96));
        assert_eq!(px(24, 11), 0x90);
        // One missile: rows 28, 29 lit, 2..27 black.
        assert_eq!((px(100, 29), px(103, 28), px(100, 27)), (0x26, 0x27, 0));
        // Radar: map colour of tile 16, player at (159, 15), the shot one pixel east.
        assert_eq!(px(148, 6), 16);
        assert_eq!(px(159, 15), 0x46);
        assert_eq!(px(148 + 8, 6 + 6), 0x22);
        assert_eq!(px(179, 7), 101, "weapon icon 2");
        assert_eq!(px(220, 2), 39, "F6 frame 0, colour 38 replaced");
        // Low armour: (8 − 70 mod 8) × 8 = 16 red; the missile lamp fades with the cooldown.
        let e = |n| hud.entries().iter().find(|x| x.0 == n).unwrap().1;
        assert_eq!(e(BAR_BACKGROUND), [16, 0, 0]);
        assert_eq!(e(MISSILE_LAMP), [15, 5, 8]);
        s.armour = 30;
        s.ticks = 72;
        hud.step(&a, &map(), &s);
        let e = |n| hud.entries().iter().find(|x| x.0 == n).unwrap().1;
        assert_eq!(e(BAR_BACKGROUND), [16, 0, 0], "kept after recovery");
        s.armour = 0;
        hud.step(&a, &map(), &s);
        let e = |n| hud.entries().iter().find(|x| x.0 == n).unwrap().1;
        assert_eq!(e(BAR_BACKGROUND), [64, 0, 0], "stored 64; the DAC shows 0");
    }

    #[test]
    fn missile_column_with_many_missiles() {
        let a = assets();
        let mut hud = Hud::new(&a);
        let mut s = state();
        for (m, top) in [(26, 0x26), (27, 0x26), (25, 0x27)] {
            s.missiles = m;
            hud.step(&a, &map(), &s);
            let px = |y: usize| hud.bar()[y * HUD_W + 100];
            assert_eq!(px(2), if m == 25 { 0 } else { top }, "{m} missiles");
            assert_eq!(px(29), 0x26);
        }
    }

    #[test]
    fn ticker_strip_wraps_into_the_next_row() {
        let a = assets();
        let mut hud = Hud::new(&a);
        // 100 spaces: cells at x 2..402 of rows 736..740; x ≥ 352 lands one row lower.
        hud.post(&a, &" ".repeat(100), INK_MESSAGE, TickerFont::Flight);
        let strip = |hud: &Hud, x: usize, row: usize| hud.strip[(row - 735) * VGA_ROW + x];
        assert_eq!(strip(&hud, 2, 736), 178);
        assert_eq!(strip(&hud, 1, 736), 0);
        assert_eq!(strip(&hud, 0, 736), 0);
        assert_eq!(strip(&hud, 0, 737), 178, "wrapped background");
        assert_eq!(strip(&hud, 40, 741), 178, "wrapped background");
        assert_eq!(strip(&hud, 60, 741), 0);
        hud.post(&a, "ab", INK_MESSAGE, TickerFont::Flight);
        assert_eq!(strip(&hud, 2, 736), INK_MESSAGE, "glyph ink");
        let mut s = state();
        s.ticker_blit = Some(0);
        hud.step(&a, &map(), &s);
        assert_eq!(hud.bar()[13 * HUD_W + 258], INK_MESSAGE);
    }
}
