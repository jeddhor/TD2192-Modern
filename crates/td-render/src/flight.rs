//! The in-flight picture: scrolling playfield (rows 0..160) and HUD bar (rows 160..192).
//!
//! Spec: `docs/spec/sim/camera-viewport.md`, `docs/spec/render/hud.md`, tick-order step 12
//! (overpass redraw), and the Verifier's composition check (`docs/re/dynamic-observations.md`
//! §13, T0 against C-0002 f3500/f3543). The original draws an 11 × 6 tile window into a page
//! and scrolls with the CRTC start address: horizontally in 4-pixel steps, vertically per
//! pixel. The ship stays at a fixed screen position; the world moves.
//!
//! **Widescreen** (Enhanced, `docs/spec/enhanced/presentation.md` rev 1 §4.2): every position
//! these functions take or return is a *Classic* screen position (the 320 × 160 playfield). A
//! framebuffer wider than 320 is a widescreen frame of width W whose Classic column 0 is at
//! frame column m = (W − 320) / 2 (its [`Viewport`]). The drawing functions move everything right
//! by m and clip to the W × 160 playfield, and the tile window and overpass redraw cover the
//! extra tile columns. The `*_in` functions apply the Classic visibility tests, relaxed by m on
//! each side when the viewport is wide. On a 320-wide framebuffer and with
//! [`Viewport::CLASSIC`] everything behaves exactly as in the Classic profile.

use std::ops::RangeInclusive;

use td_formats::map::TileMap;
use td_formats::tileset::{TILE_SIZE, Tileset};

use crate::framebuffer::{Framebuffer, Image, Rect, WIDTH};

/// The scrolling playfield.
pub const PLAYFIELD: Rect = Rect {
    x: 0,
    y: 0,
    w: 320,
    h: 160,
};
/// First screen row of the HUD bar (`td.win`).
pub const HUD_Y: i32 = 160;
/// HUD bar size.
pub const HUD_W: usize = 320;
pub const HUD_H: usize = 32;
/// Screen position of a 32 × 32 ship frame's top-left corner.
pub const SHIP_POS: (i32, i32) = (144, 68);
/// Map tiles redrawn over the ships each tick (overpasses).
pub const OVERPASS_TILES: [u8; 4] = [0x42, 0x44, 0x45, 0x46];

/// The horizontal extent of the flight picture (Enhanced widescreen, spec
/// enhanced/presentation rev 1 §4): a playfield W × 160 with the Classic picture in its centre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    width: i32,
}

impl Viewport {
    /// The Classic 320-wide picture.
    pub const CLASSIC: Viewport = Viewport {
        width: WIDTH as i32,
    };

    /// A picture `width` pixels wide (at least 320).
    pub fn new(width: usize) -> Self {
        Self {
            width: width.max(WIDTH) as i32,
        }
    }

    /// The viewport of a framebuffer: its width.
    pub fn of(fb: &Framebuffer) -> Self {
        Self::new(fb.width())
    }

    /// W.
    pub fn width(self) -> i32 {
        self.width
    }

    /// m = (W − 320) / 2: the frame column of Classic column 0.
    pub fn margin(self) -> i32 {
        (self.width - WIDTH as i32) / 2
    }

    /// True for the Classic 320-wide picture.
    pub fn is_classic(self) -> bool {
        self.width == WIDTH as i32
    }

    /// The playfield in frame coordinates: W × 160.
    pub fn playfield(self) -> Rect {
        Rect {
            w: self.width,
            ..PLAYFIELD
        }
    }

    /// True if an object covering Classic columns `x .. x + w` shows in the window.
    pub fn spans(self, x: i32, w: i32) -> bool {
        let m = self.margin();
        x + w > -m && x < WIDTH as i32 + m
    }

    /// The tile window columns drawn: 0..=10 in Classic; widescreen adds the columns needed to
    /// fill the margins (from view tile X − ⌈m / 32⌉ at most).
    fn columns(self, view: ViewPos) -> RangeInclusive<i32> {
        if self.is_classic() {
            return 0..=10;
        }
        let (ox, m) = (view.page_origin().0, self.margin());
        let size = TILE_SIZE as i32;
        (ox - m).div_euclid(size)..=(WIDTH as i32 + m + ox - 1).div_euclid(size)
    }

    /// The margin strips of the playfield in frame coordinates (empty in Classic).
    fn margins(self) -> [Rect; 2] {
        let m = self.margin();
        [
            Rect { w: m, ..PLAYFIELD },
            Rect {
                x: m + WIDTH as i32,
                w: m,
                ..PLAYFIELD
            },
        ]
    }
}

/// The view as the simulation stores it: a map tile plus a pixel offset per axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewPos {
    pub tile_x: i32,
    pub tile_y: i32,
    pub sub_x: i32,
    pub sub_y: i32,
}

impl ViewPos {
    /// Page pixel shown at the playfield's top-left corner: `4·Round(subX / 4) − 16`
    /// (Round halves away from zero) and `subY − 16`.
    pub fn page_origin(&self) -> (i32, i32) {
        (4 * (self.sub_x + 2).div_euclid(4) - 16, self.sub_y - 16)
    }
}

fn tile_at(map: &TileMap, view: ViewPos, i: i32, j: i32) -> Option<u8> {
    let (x, y) = (view.tile_x + i, view.tile_y + j);
    map.get(usize::try_from(x).ok()?, usize::try_from(y).ok()?)
}

/// Draws one window tile (column `i`, row `j` of the 11 × 6 window) opaquely, clipped to the
/// playfield.
fn draw_window_tile(
    fb: &mut Framebuffer,
    map: &TileMap,
    tiles: &Tileset,
    view: ViewPos,
    i: i32,
    j: i32,
) {
    let clip = Viewport::of(fb).playfield();
    draw_window_tile_clipped(fb, map, tiles, view, (i, j), clip);
}

/// [`draw_window_tile`] clipped to `clip` (frame coordinates).
fn draw_window_tile_clipped(
    fb: &mut Framebuffer,
    map: &TileMap,
    tiles: &Tileset,
    view: ViewPos,
    (i, j): (i32, i32),
    clip: Rect,
) {
    let Some(tile) = tile_at(map, view, i, j).and_then(|t| tiles.tiles.get(usize::from(t))) else {
        return;
    };
    let (ox, oy) = view.page_origin();
    let m = Viewport::of(fb).margin();
    let size = TILE_SIZE as i32;
    let img = Image::new(tile, TILE_SIZE, TILE_SIZE);
    fb.blit(img, i * size - ox + m, j * size - oy, clip, None);
}

/// Draws the tile window for `view` into the playfield (with the extra columns of a widescreen
/// frame).
pub fn draw_tile_window(fb: &mut Framebuffer, map: &TileMap, tiles: &Tileset, view: ViewPos) {
    for i in Viewport::of(fb).columns(view) {
        for j in 0..6 {
            draw_window_tile(fb, map, tiles, view, i, j);
        }
    }
}

/// True if window cell (i, j) is redrawn over the ships in Episode 1: tiles 42h, 44h, 45h,
/// 46h, and every cell with map x 7Ah..=7Fh at map y 47h.
fn is_overpass(map: &TileMap, view: ViewPos, i: i32, j: i32) -> bool {
    let special = (0x7A..=0x7F).contains(&(view.tile_x + i)) && view.tile_y + j == 0x47;
    special || tile_at(map, view, i, j).is_some_and(|t| OVERPASS_TILES.contains(&t))
}

/// Redraws the overpass tiles of window columns 3..=6, rows 1..=4 over the ships (tick-order
/// step 12): tiles 42h, 44h, 45h, 46h, and every cell with map x 7Ah..=7Fh at map y 47h.
///
/// In a widescreen frame the redraw also covers the margins: every overpass cell of the wider
/// window, clipped to the margin strips so that the Classic centre stays as it is.
pub fn redraw_overpasses(fb: &mut Framebuffer, map: &TileMap, tiles: &Tileset, view: ViewPos) {
    for i in 3..=6 {
        for j in 1..=4 {
            if is_overpass(map, view, i, j) {
                draw_window_tile(fb, map, tiles, view, i, j);
            }
        }
    }
    let vp = Viewport::of(fb);
    if vp.is_classic() {
        return;
    }
    for i in vp.columns(view) {
        for j in 0..6 {
            if is_overpass(map, view, i, j) {
                for clip in vp.margins() {
                    draw_window_tile_clipped(fb, map, tiles, view, (i, j), clip);
                }
            }
        }
    }
}

/// Episode 2's overlay tiles (tick-order step 12, `docs/spec/sim/e2-mission-setup.md` rev 1
/// §3.6): every cell of the 11 × 6 window. Map set 1 (`td.map`): E1's tiles 42h, 44h, 45h,
/// 46h and the cells x 7Ah..=7Fh at y 47h; map set 2 (`td2.map`): tiles 33h..=3Bh, 1Eh, 1Fh,
/// 20h. The caller skips it while late draw is set. A widescreen frame covers its extra
/// columns too.
pub fn redraw_overlays_e2(
    fb: &mut Framebuffer,
    map: &TileMap,
    tiles: &Tileset,
    view: ViewPos,
    map_set: u8,
) {
    for i in Viewport::of(fb).columns(view) {
        for j in 0..6 {
            let tile = tile_at(map, view, i, j);
            let overlay = if map_set == 2 {
                tile.is_some_and(|t| (0x33..=0x3B).contains(&t) || (0x1E..=0x20).contains(&t))
            } else {
                let special = (0x7A..=0x7F).contains(&(view.tile_x + i)) && view.tile_y + j == 0x47;
                special || tile.is_some_and(|t| OVERPASS_TILES.contains(&t))
            };
            if overlay {
                draw_window_tile(fb, map, tiles, view, i, j);
            }
        }
    }
}

/// Draws a ship frame (colour 0 transparent) at the player's screen position. `frame` is a
/// square frame of `size` pixels centred like a 32 × 32 base frame (40 × 40 heading frames
/// have the base frame at offset (4, 4)).
pub fn draw_player_ship(fb: &mut Framebuffer, frame: &[u8], size: usize) {
    let inset = (size as i32 - 32) / 2;
    let vp = Viewport::of(fb);
    fb.blit(
        Image::new(frame, size, size),
        SHIP_POS.0 - inset + vp.margin(),
        SHIP_POS.1 - inset,
        vp.playfield(),
        Some(0),
    );
}

/// Pixel offsets of one bracket colour, with that colour for markers 32h and 39h.
pub type BracketPixels = (&'static [(i32, i32)], u8, u8);

/// Target brackets around a marked mission ship (`mission-ships.md` rev 2 §4.7): pixel offsets
/// from the 32 × 32 frame's top-left corner, with their colours for markers 32h and 39h.
pub const BRACKETS: [BracketPixels; 3] = [
    (&[(1, 1), (1, 30), (30, 1), (30, 30)], 0x5C, 0x66),
    (
        &[
            (2, 1),
            (1, 2),
            (1, 29),
            (2, 30),
            (29, 1),
            (30, 2),
            (30, 29),
            (29, 30),
        ],
        0x7A,
        0x82,
    ),
    (
        &[
            (3, 1),
            (1, 3),
            (1, 28),
            (3, 30),
            (28, 1),
            (30, 3),
            (30, 28),
            (28, 30),
        ],
        0xF9,
        0x98,
    ),
];

/// One mission ship as the drawing step sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShipMark {
    pub tile_x: u8,
    pub tile_y: u8,
    pub sub_x: i16,
    pub sub_y: i16,
    /// 0, 32h (target bracket) or 39h (friend bracket).
    pub marker: u8,
}

/// Screen position of a mission ship's 32 × 32 frame (top-left corner) drawn into back page
/// `page`, or `None` when the original's window test leaves it out (`mission-ships.md` rev 2
/// §4.7).
///
/// The spec's y, `(tileY − viewY) × 32 + offsetY + 232 × page`, is a VGA row, not a row of
/// the page: page 0 starts at VGA row 32 (`hud.md` §2), so the corner sits 32 page rows above
/// `(tileY − viewY) × 32 + offsetY`, and the test `y > 32` keeps page-0 sprites out of the HUD
/// rows (a ship at the top edge is drawn on page-1 ticks only). Black-box (U-127,
/// `tests/hud_parity.rs`): this reading puts every bracket pixel of C-0009 where the capture
/// has it.
pub fn mission_ship_pos(view: ViewPos, page: i32, s: &ShipMark) -> Option<(i32, i32)> {
    let (tx, ty) = (i32::from(s.tile_x), i32::from(s.tile_y));
    let (sx, sy) = (i32::from(s.sub_x), i32::from(s.sub_y));
    let (x, row) = ((tx - view.tile_x) * 32 + sx, (ty - view.tile_y) * 32 + sy);
    let inside = view.tile_x - 1 < tx
        && tx < view.tile_x + 10
        && (view.tile_y..=view.tile_y + 5).contains(&ty)
        && (tx < view.tile_x + 9 || sx < view.sub_x)
        && row + 232 * page > 32;
    let (ox, oy) = view.page_origin();
    inside.then_some((x - ox, row - 32 - oy))
}

/// [`mission_ship_pos`] for viewport `vp`: in a widescreen frame a ship also shows whenever its
/// frame (40 × 40 at most) overlaps the wider window, with the Classic row tests.
pub fn mission_ship_pos_in(
    view: ViewPos,
    page: i32,
    s: &ShipMark,
    vp: Viewport,
) -> Option<(i32, i32)> {
    let classic = mission_ship_pos(view, page, s);
    if classic.is_some() || vp.is_classic() {
        return classic;
    }
    let (tx, ty) = (i32::from(s.tile_x), i32::from(s.tile_y));
    let (sx, sy) = (i32::from(s.sub_x), i32::from(s.sub_y));
    let (x, row) = ((tx - view.tile_x) * 32 + sx, (ty - view.tile_y) * 32 + sy);
    let (ox, oy) = view.page_origin();
    let pos = (x - ox, row - 32 - oy);
    let rows_ok = (view.tile_y..=view.tile_y + 5).contains(&ty) && row + 232 * page > 32;
    (rows_ok && vp.spans((pos.0 & !3) - 4, 40)).then_some(pos)
}

/// Draws a mission ship at `pos` (from [`mission_ship_pos`]): its heading frame (colour 0
/// transparent; `size` 32, or 40 with the 32 × 32 base frame at (4, 4)) with x rounded down to
/// a multiple of 4 like every planar sprite blit (`flight-screens.md` §2), then the target
/// brackets of its marker, which are single pixels at the exact x.
pub fn draw_mission_ship(
    fb: &mut Framebuffer,
    frame: Option<(&[u8], usize)>,
    (x, y): (i32, i32),
    marker: u8,
) {
    if let Some((frame, size)) = frame {
        draw_sprite(fb, frame, size, (x & !3, y));
    }
    for (pixels, target, friend) in BRACKETS {
        let c = match marker {
            0x32 => target,
            0x39 => friend,
            _ => return,
        };
        for &(dx, dy) in pixels {
            put_playfield(fb, x + dx, y + dy, c);
        }
    }
}

/// Plots one pixel at Classic screen position (x, y) if it lies in the playfield rows (shots,
/// brackets).
pub fn put_playfield(fb: &mut Framebuffer, x: i32, y: i32, color: u8) {
    if (PLAYFIELD.y..PLAYFIELD.h).contains(&y) {
        let m = Viewport::of(fb).margin();
        fb.put(x + m, y, color);
    }
}

/// Draws a sprite frame (colour 0 transparent) with the 32 × 32 base frame's top-left corner at
/// screen `pos`; `size` 40 frames have the base frame at (4, 4).
pub fn draw_sprite(fb: &mut Framebuffer, frame: &[u8], size: usize, (x, y): (i32, i32)) {
    let inset = (size as i32 - 32) / 2;
    let vp = Viewport::of(fb);
    fb.blit(
        Image::new(frame, size, size),
        x - inset + vp.margin(),
        y - inset,
        vp.playfield(),
        Some(0),
    );
}

/// Screen position of a sprite blitted at sprite coordinates (x, y) (`flight-screens.md` rev 1
/// §2): the planar blit rounds x down to a multiple of 4, and sprite y is the page row + 32.
pub fn sprite_screen(view: ViewPos, (x, y): (i32, i32)) -> (i32, i32) {
    let (ox, oy) = view.page_origin();
    ((x & !3) - ox, y - 32 - oy)
}

/// Sprite coordinates of an explosion drawn into back page `page`, or `None` when it is not
/// drawn (`flight-screens.md` rev 1 §4.7; an explosion in the top tile row shows on page 1
/// only, OB-033). The sprite is bank slot 19 + frame.
pub fn explosion_sprite(
    view: ViewPos,
    page: i32,
    tile: (u8, u8),
    off: (u8, u8),
) -> Option<(i32, i32)> {
    let (tx, ty) = (i32::from(tile.0), i32::from(tile.1));
    let (dx, dy) = (i32::from(off.0), i32::from(off.1));
    let row = (ty - view.tile_y) * 32 + dy;
    let visible = (view.tile_x..=view.tile_x + 9).contains(&tx)
        && (view.tile_y..=view.tile_y + 5).contains(&ty)
        && !(tx == view.tile_x + 9 && dx >= view.sub_x)
        && (page == 1 || row > 32);
    // The code passes x + 352, one VGA row further down.
    visible.then_some(((tx - view.tile_x) * 32 + dx, row + 1))
}

/// [`explosion_sprite`] for viewport `vp`: in a widescreen frame an explosion also shows
/// whenever its sprite overlaps the wider window, with the Classic row and page tests.
pub fn explosion_sprite_in(
    view: ViewPos,
    page: i32,
    tile: (u8, u8),
    off: (u8, u8),
    vp: Viewport,
) -> Option<(i32, i32)> {
    let classic = explosion_sprite(view, page, tile, off);
    if classic.is_some() || vp.is_classic() {
        return classic;
    }
    let (tx, ty) = (i32::from(tile.0), i32::from(tile.1));
    let (dx, dy) = (i32::from(off.0), i32::from(off.1));
    let row = (ty - view.tile_y) * 32 + dy;
    let at = ((tx - view.tile_x) * 32 + dx, row + 1);
    let rows_ok = (view.tile_y..=view.tile_y + 5).contains(&ty) && (page == 1 || row > 32);
    (rows_ok && vp.spans(sprite_screen(view, at).0, 32)).then_some(at)
}

/// How a projectile looks (`flight-screens.md` rev 1 §4.8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShotLook {
    /// Kinds 0..2: single pixels (screen x, y, colour) in drawing order.
    Pixels(Vec<(i32, i32, u8)>),
    /// Kinds 4..6: sprite bank `slot` with the blit of `heading` at sprite coordinates `at`.
    Sprite {
        slot: usize,
        heading: u8,
        at: (i32, i32),
    },
}

/// Pixel offsets and colours of shot kinds 0..2 for a principal heading (§4.8).
fn shot_pattern(kind: u8, heading: u8) -> Option<Vec<(i32, i32, u8)>> {
    #[derive(Clone, Copy)]
    enum G {
        V,
        H,
        D1,
        D2,
    }
    let g = match heading {
        8 | 2 => G::V,
        4 | 6 => G::H,
        9 | 1 => G::D1,
        7 | 3 => G::D2,
        _ => return None,
    };
    let (a, b, c, d) = (0x3C, 0x3D, 0x32, 0x33);
    let (e, f) = (0xBE, 0x35);
    Some(match (kind, g) {
        (0, G::V) => vec![(0, -1, a), (0, 0, b), (0, 1, a)],
        (0, G::H) => vec![(1, 0, a), (0, 0, b), (-1, 0, a)],
        (0, G::D1) => vec![(1, -1, a), (0, 0, b), (-1, 1, a)],
        (0, G::D2) => vec![(1, 1, a), (0, 0, b), (-1, -1, a)],
        (1, G::V) => vec![
            (0, -1, c),
            (0, 0, d),
            (0, 1, d),
            (0, 2, c),
            (1, -1, c),
            (1, 0, d),
            (1, 1, d),
            (1, 2, d),
        ],
        (1, G::H) => vec![
            (1, 0, c),
            (0, 0, d),
            (-1, 0, d),
            (-2, 0, c),
            (1, 1, c),
            (0, 1, d),
            (-1, 1, d),
            (-2, 1, d),
        ],
        (1, G::D1) => vec![
            (1, -1, c),
            (0, 0, d),
            (-1, 1, d),
            (-2, 2, c),
            (2, -1, c),
            (1, 0, d),
            (0, 1, d),
            (-1, 2, c),
        ],
        (1, G::D2) => vec![
            (1, 1, c),
            (0, 0, d),
            (-1, -1, d),
            (-2, -2, c),
            (2, 1, c),
            (1, 0, d),
            (0, -1, d),
            (-1, -2, c),
        ],
        (2, G::V) => (-1..=2)
            .flat_map(|x| [(x, -2, e), (x, -1, f), (x, 0, f), (x, 1, f), (x, 2, e)])
            .collect(),
        (2, G::H) => (-1..=2)
            .flat_map(|y| [(2, y, e), (1, y, f), (0, y, f), (-1, y, f), (-2, y, e)])
            .collect(),
        (2, G::D1) => (0..=3)
            .flat_map(|k| {
                [
                    (2, k - 3, e),
                    (1, k - 2, f),
                    (0, k - 1, f),
                    (-1, k, f),
                    (-2, k + 1, e),
                ]
            })
            .collect(),
        (2, G::D2) => (0..=3)
            .flat_map(|k| {
                [
                    (2, k + 1, e),
                    (1, k, f),
                    (0, k - 1, f),
                    (-1, k - 2, f),
                    (-2, k - 3, e),
                ]
            })
            .collect(),
        _ => return None,
    })
}

/// How a live projectile is drawn this tick, or `None` when it is not (`flight-screens.md` rev 1
/// §4.8). Kind 3 and shots on a two-digit heading are never drawn.
pub fn shot_look(
    view: ViewPos,
    kind: u8,
    heading: u8,
    tile: (u8, u8),
    off: (i16, i16),
) -> Option<ShotLook> {
    let (tx, ty) = (i32::from(tile.0), i32::from(tile.1));
    let x = (tx - view.tile_x) * 32 + i32::from(off.0);
    let y = (ty - view.tile_y) * 32 + i32::from(off.1);
    match kind {
        0..=2 => {
            // The centre (world pixel + 16) inside the 352 × 192 tile window, below the HUD
            // rows (the page test Y + 1 <= 261 is implied).
            let visible = (0..352).contains(&(x + 16)) && (34..192).contains(&(y + 16));
            if !visible {
                return None;
            }
            shot_pixels(view, kind, heading, (x, y))
        }
        4..=6 => {
            let visible = (view.tile_x..=view.tile_x + 8).contains(&tx)
                && (view.tile_y + 2..=view.tile_y + 5).contains(&ty)
                && matches!(heading, 1..=4 | 6..=9);
            visible.then(|| missile_look(kind, heading, (x, y)))
        }
        _ => None,
    }
}

/// The pixels of a kind 0..2 shot at window pixel (x, y).
fn shot_pixels(view: ViewPos, kind: u8, heading: u8, (x, y): (i32, i32)) -> Option<ShotLook> {
    let r = view.page_origin().0 + 16;
    let (sx, sy) = (x + 32 - r, y - view.sub_y);
    let pixels = shot_pattern(kind, heading)?;
    Some(ShotLook::Pixels(
        pixels
            .into_iter()
            .map(|(dx, dy, c)| (sx + dx, sy + dy, c))
            .collect(),
    ))
}

/// The sprite of a kind 4..6 missile at window pixel `at`.
fn missile_look(kind: u8, heading: u8, at: (i32, i32)) -> ShotLook {
    let slot = [16, 15, 9][usize::from(kind - 4)];
    ShotLook::Sprite { slot, heading, at }
}

/// [`shot_look`] for viewport `vp`: in a widescreen frame a shot also shows whenever it lies in
/// the wider window, with the Classic row tests (kinds 0..2: the centre within the playfield
/// rows; kinds 4..6: tile rows 2..5 and a principal heading).
pub fn shot_look_in(
    view: ViewPos,
    kind: u8,
    heading: u8,
    tile: (u8, u8),
    off: (i16, i16),
    vp: Viewport,
) -> Option<ShotLook> {
    let classic = shot_look(view, kind, heading, tile, off);
    if classic.is_some() || vp.is_classic() {
        return classic;
    }
    let (tx, ty) = (i32::from(tile.0), i32::from(tile.1));
    let x = (tx - view.tile_x) * 32 + i32::from(off.0);
    let y = (ty - view.tile_y) * 32 + i32::from(off.1);
    match kind {
        0..=2 => {
            let centre = x + 16 - view.page_origin().0;
            let visible = (34..192).contains(&(y + 16)) && vp.spans(centre - 3, 6);
            if visible {
                shot_pixels(view, kind, heading, (x, y))
            } else {
                None
            }
        }
        4..=6 => {
            let visible = (view.tile_y + 2..=view.tile_y + 5).contains(&ty)
                && matches!(heading, 1..=4 | 6..=9)
                && vp.spans(sprite_screen(view, (x, y)).0 - 4, 40);
            visible.then(|| missile_look(kind, heading, (x, y)))
        }
        _ => None,
    }
}

/// Screen top-left of the afterburner flame for a burst heading (`flight-screens.md` rev 1
/// §4.6); the flame is sprite bank slot 9 + burst frame, drawn with that heading's blit.
pub fn flame_pos(heading: u8) -> Option<(i32, i32)> {
    Some(match heading {
        8 => (144, 100),
        2 => (144, 36),
        6 => (112, 68),
        4 => (176, 68),
        7 => (168, 92),
        1 => (168, 44),
        9 => (120, 92),
        3 => (120, 44),
        _ => return None,
    })
}

/// Copies the 320 × 32 HUD bar (`td.win`) to rows 160..192 (at x = m in a widescreen frame).
///
/// # Panics
/// If `win` is not 320 × 32 bytes.
pub fn draw_hud_bar(fb: &mut Framebuffer, win: &[u8]) {
    assert_eq!(win.len(), HUD_W * HUD_H, "td.win is 320x32");
    let (w, m) = (fb.width(), Viewport::of(fb).margin() as usize);
    for (r, src) in win.chunks_exact(HUD_W).enumerate() {
        let start = (HUD_Y as usize + r) * w + m;
        fb.pixels[start..start + HUD_W].copy_from_slice(src);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horizontal_scroll_is_quantised_to_four_pixels() {
        let v = |sub_x| ViewPos {
            tile_x: 0,
            tile_y: 0,
            sub_x,
            sub_y: 16,
        };
        assert_eq!(v(32).page_origin(), (16, 0));
        assert_eq!(v(17).page_origin(), (0, 0));
        assert_eq!(v(18).page_origin(), (4, 0), "Round(4.5) = 5");
        assert_eq!(v(21).page_origin(), (4, 0));
        assert_eq!(v(48).page_origin(), (32, 0));
    }

    #[test]
    fn window_and_overpass_layout() {
        // Tile t is filled with colour t; the map holds tile 1 everywhere except an overpass
        // (0x42) at window cell (5, 2).
        let mut blk = vec![0u8; 80 * 1024];
        for t in 0..80 {
            blk[t * 1024..(t + 1) * 1024].fill(t as u8);
        }
        let tiles = Tileset::parse(&blk).unwrap();
        let mut cells = vec![1u8; 201 * 201];
        let view = ViewPos {
            tile_x: 102,
            tile_y: 104,
            sub_x: 32,
            sub_y: 16,
        };
        cells[107 * 201 + 106] = 0x42;
        let map = TileMap::parse_episode(&cells).unwrap();
        let mut fb = Framebuffer::new();
        draw_tile_window(&mut fb, &map, &tiles, view);
        // Window cell (5, 2) spans page x 160..192, y 64..96 → screen x 144..176, y 64..96.
        assert_eq!(fb.get(144, 64), Some(0x42));
        assert_eq!(fb.get(143, 64), Some(1));
        let ship = vec![9u8; 32 * 32];
        draw_player_ship(&mut fb, &ship, 32);
        assert_eq!(fb.get(150, 70), Some(9));
        assert_eq!(fb.get(150, 99), Some(9));
        redraw_overpasses(&mut fb, &map, &tiles, view);
        assert_eq!(fb.get(150, 70), Some(0x42), "overpass covers the ship");
        assert_eq!(
            fb.get(150, 99),
            Some(9),
            "below the overpass the ship shows"
        );
        assert_eq!(fb.get(150, 160), Some(0), "playfield clip");
    }

    const VIEW: ViewPos = ViewPos {
        tile_x: 102,
        tile_y: 104,
        sub_x: 32,
        sub_y: 16,
    };

    #[test]
    fn sprites_use_planar_x_and_sprite_rows() {
        assert_eq!(sprite_screen(VIEW, (163, 100)), (144, 68));
        let mut fb = Framebuffer::new();
        let frame = vec![9u8; 32 * 32];
        draw_mission_ship(&mut fb, Some((&frame, 32)), (145, 50), 0x32);
        assert_eq!(fb.get(144, 50), Some(9), "sprite x rounded down to 144");
        assert_eq!(fb.get(146, 51), Some(0x5C), "bracket at the exact x + 1");
        let ship = ShipMark {
            tile_x: 107,
            tile_y: 107,
            sub_x: 32,
            sub_y: 32,
            marker: 0x32,
        };
        // Row 3 × 32 + 32 is a VGA row: 32 page rows higher on screen (U-127).
        assert_eq!(mission_ship_pos(VIEW, 0, &ship), Some((176, 96)));
        let top = ShipMark {
            tile_y: 104,
            sub_y: 20,
            ..ship
        };
        assert_eq!(
            mission_ship_pos(VIEW, 0, &top),
            None,
            "page 0 skips the top row"
        );
        assert_eq!(mission_ship_pos(VIEW, 1, &top), Some((176, -12)));
    }

    #[test]
    fn explosions_follow_the_window_and_page_rules() {
        assert_eq!(
            explosion_sprite(VIEW, 0, (107, 107), (16, 16)),
            Some((176, 113))
        );
        assert_eq!(
            explosion_sprite(VIEW, 0, (107, 104), (16, 16)),
            None,
            "OB-033"
        );
        assert_eq!(
            explosion_sprite(VIEW, 1, (107, 104), (16, 16)),
            Some((176, 17))
        );
        assert_eq!(
            explosion_sprite(VIEW, 1, (111, 105), (32, 16)),
            None,
            "right column"
        );
        assert_eq!(
            explosion_sprite(VIEW, 1, (111, 105), (31, 16)),
            Some((319, 49))
        );
    }

    #[test]
    fn shots_by_kind_and_heading() {
        let look = shot_look(VIEW, 0, 8, (107, 106), (16, 16));
        assert_eq!(
            look,
            Some(ShotLook::Pixels(vec![
                (176, 63, 0x3C),
                (176, 64, 0x3D),
                (176, 65, 0x3C)
            ]))
        );
        let Some(ShotLook::Pixels(p)) = shot_look(VIEW, 2, 9, (107, 106), (16, 16)) else {
            panic!("kind 2 is drawn as pixels");
        };
        assert_eq!(p.len(), 20);
        assert_eq!(shot_look(VIEW, 3, 8, (107, 106), (16, 16)), None, "kind 3");
        assert_eq!(
            shot_look(VIEW, 0, 0x24, (107, 106), (16, 16)),
            None,
            "two-digit heading"
        );
        assert_eq!(
            shot_look(VIEW, 0, 8, (107, 104), (16, 0)),
            None,
            "under the HUD rows"
        );
        assert_eq!(
            shot_look(VIEW, 4, 8, (103, 105), (16, 16)),
            None,
            "rows 2..5 only"
        );
        assert_eq!(
            shot_look(VIEW, 4, 2, (103, 106), (16, 16)),
            Some(ShotLook::Sprite {
                slot: 16,
                heading: 2,
                at: (48, 80)
            })
        );
        assert_eq!(flame_pos(8), Some((144, 100)));
        assert_eq!(flame_pos(0x24), None);
    }

    /// A tileset where tile t is filled with colour t, and a map of tile 1 + (x + y) % 7 with
    /// an overpass (42h) at window cell (`ox`, 2) for each `ox`.
    fn striped_world(overpass_cols: &[i32]) -> (TileMap, Tileset) {
        let mut blk = vec![0u8; 80 * 1024];
        for t in 0..80 {
            blk[t * 1024..(t + 1) * 1024].fill(t as u8);
        }
        let mut cells: Vec<u8> = (0..201 * 201)
            .map(|i| 1 + ((i % 201 + i / 201) % 7) as u8)
            .collect();
        for &i in overpass_cols {
            let x = (VIEW.tile_x + i) as usize;
            cells[(VIEW.tile_y as usize + 2) * 201 + x] = 0x42;
        }
        (
            TileMap::parse_episode(&cells).unwrap(),
            Tileset::parse(&blk).unwrap(),
        )
    }

    #[test]
    fn widescreen_centre_equals_classic_and_margins_are_filled() {
        let (map, tiles) = striped_world(&[-2, 5, 12]);
        let ship = vec![9u8; 32 * 32];
        let draw = |width: usize, view: ViewPos| {
            let mut fb = Framebuffer::with_width(width);
            draw_tile_window(&mut fb, &map, &tiles, view);
            draw_player_ship(&mut fb, &ship, 32);
            // A mission ship in the left margin, under the overpass of column −2.
            let mark = ShipMark {
                tile_x: (view.tile_x - 2) as u8,
                tile_y: (view.tile_y + 2) as u8,
                sub_x: 0,
                sub_y: 32,
                marker: 0x32,
            };
            if let Some(pos) = mission_ship_pos_in(view, 1, &mark, Viewport::of(&fb)) {
                draw_mission_ship(&mut fb, Some((&ship, 32)), pos, mark.marker);
            }
            redraw_overpasses(&mut fb, &map, &tiles, view);
            fb
        };
        for sub_x in [16, 18, 32, 47] {
            let view = ViewPos { sub_x, ..VIEW };
            let classic = draw(320, view);
            for width in [384, 428, 560] {
                let wide = draw(width, view);
                let m = (width - 320) / 2;
                for y in 0..160 {
                    assert_eq!(
                        &wide.pixels[y * width + m..y * width + m + 320],
                        &classic.pixels[y * 320..(y + 1) * 320],
                        "{width}/{sub_x}: row {y}"
                    );
                    let row = &wide.pixels[y * width..(y + 1) * width];
                    assert!(
                        row.iter().all(|&p| p != 0),
                        "{width}/{sub_x}: row {y} has gaps"
                    );
                }
                assert!(
                    wide.pixels[160 * width..].iter().all(|&p| p == 0),
                    "nothing below the playfield"
                );
            }
        }
        // The margin overpass covers the mission ship drawn under it.
        let wide = draw(560, VIEW);
        let (ox, oy) = VIEW.page_origin();
        let (x, y) = (-2 * 32 - ox + 120 + 8, 2 * 32 - oy + 8);
        assert_eq!(wide.get(x, y), Some(0x42), "overpass over the ship");
        assert_eq!(
            mission_ship_pos(
                VIEW,
                1,
                &ShipMark {
                    tile_x: (VIEW.tile_x - 2) as u8,
                    tile_y: (VIEW.tile_y + 2) as u8,
                    sub_x: 0,
                    sub_y: 32,
                    marker: 0x32,
                }
            ),
            None,
            "Classic leaves the ship out"
        );
    }

    #[test]
    fn widescreen_relaxes_visibility_by_the_margin() {
        let vp = Viewport::new(428);
        assert_eq!((vp.margin(), vp.playfield().w), (54, 428));
        assert!(Viewport::CLASSIC.is_classic() && !vp.is_classic());
        assert!(vp.spans(-54 - 31, 32) && !vp.spans(-54 - 32, 32));
        assert!(vp.spans(320 + 53, 32) && !vp.spans(320 + 54, 32));
        // An explosion two columns right of the Classic window.
        assert_eq!(explosion_sprite(VIEW, 1, (113, 105), (0, 16)), None);
        let at = explosion_sprite_in(VIEW, 1, (113, 105), (0, 16), vp).unwrap();
        assert_eq!(sprite_screen(VIEW, at), (11 * 32 - 16, 17));
        assert_eq!(
            explosion_sprite_in(VIEW, 1, (113, 105), (0, 16), Viewport::CLASSIC),
            None
        );
        // A shot left of the window; kind 3 stays invisible.
        let look = shot_look_in(VIEW, 0, 8, (100, 106), (16, 16), vp);
        assert_eq!(
            look,
            Some(ShotLook::Pixels(vec![
                (-48, 63, 0x3C),
                (-48, 64, 0x3D),
                (-48, 65, 0x3C)
            ]))
        );
        assert_eq!(shot_look(VIEW, 0, 8, (100, 106), (16, 16)), None);
        assert_eq!(shot_look_in(VIEW, 3, 8, (100, 106), (16, 16), vp), None);
        // Inside the Classic window the Classic answer stands.
        assert_eq!(
            shot_look_in(VIEW, 0, 8, (107, 106), (16, 16), vp),
            shot_look(VIEW, 0, 8, (107, 106), (16, 16))
        );
        // Single pixels move by the margin.
        let mut fb = Framebuffer::with_width(428);
        put_playfield(&mut fb, -54, 0, 5);
        put_playfield(&mut fb, 0, 160, 5);
        assert_eq!(fb.get(0, 0), Some(5));
        assert_eq!(fb.pixels.iter().filter(|&&p| p == 5).count(), 1);
    }
}
