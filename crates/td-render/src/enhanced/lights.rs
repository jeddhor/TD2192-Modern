//! The light list of a flight frame (M12-11): the §6.1 sources taken from what the tick drew,
//! and the occluder grid of solid map cells.
//!
//! Spec: `docs/spec/enhanced/presentation.md` rev 3 §6.1. Positions use the flight screen's
//! mapping (`crate::flight`; `camera-viewport.md`, `flight-screens.md` rev 1 §4.6–§4.8,
//! `mission-ships.md` rev 2 §4.7) shifted right by the widescreen margin m: screen = world −
//! view origin, origin = (viewX·32 + 4·Round(subX / 4) − 16, viewY·32 + subY − 16).
//! Presentation only: reads `Flight::frame` and `Flight::drawn`, writes nothing back.

use td_formats::palette::Palette;
use td_sim::flight::explosions::IDLE;
use td_sim::flight::heading::Heading;
use td_sim::flight::shots::Shot;
use td_sim::{Flight, World};

use super::lighting::{CELL, LIT_ROWS, linear_rgb};
use super::scene::{Light, LightMode, LightScene, Occluders};
use crate::flight::{self, SHIP_POS, ShipMark, ShotLook, ViewPos, Viewport};

/// One row of the §6.1 light table: colour (linear RGB), radius (px), intensity, shadows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Source {
    pub color: [f32; 3],
    pub radius: f32,
    pub intensity: f32,
    pub shadows: bool,
}

impl Source {
    /// This source at base pixel `pos`.
    pub fn at(self, pos: (f32, f32)) -> Light {
        Light {
            pos,
            color: self.color,
            radius: self.radius,
            intensity: self.intensity,
            shadows: self.shadows,
        }
    }
}

/// Explosion (intensity by frame in [`EXPLOSION_INTENSITY`]).
pub const EXPLOSION: Source = Source {
    color: [1.0, 0.55, 0.15],
    radius: 56.0,
    intensity: 1.6,
    shadows: true,
};
/// Explosion intensity for frames 1..=6.
pub const EXPLOSION_INTENSITY: [f32; 6] = [1.6, 1.4, 1.1, 0.8, 0.5, 0.25];
/// Cannon shots of kinds 0, 1 and 2 take the current colour of these entries.
pub const SHOT_COLOUR: [u8; 3] = [0x3D, 0x33, 0x35];
/// Cannon shot radius by kind.
pub const SHOT_RADIUS: [f32; 3] = [12.0, 14.0, 18.0];
/// Cannon shot intensity.
pub const SHOT_INTENSITY: f32 = 0.5;
/// Missiles (kinds 4..=6).
pub const MISSILE: Source = Source {
    color: [1.0, 0.8, 0.5],
    radius: 20.0,
    intensity: 1.0,
    shadows: false,
};
/// Muzzle flash, on the tick a volley's shots are first drawn.
pub const MUZZLE: Source = Source {
    color: [1.0, 0.9, 0.6],
    radius: 24.0,
    intensity: 1.2,
    shadows: false,
};
/// Afterburner flame.
pub const FLAME: Source = Source {
    color: [0.6, 0.8, 1.0],
    radius: 32.0,
    intensity: 1.3,
    shadows: true,
};
/// The player's engine glow, behind the ship.
pub const PLAYER_ENGINE: Source = Source {
    color: [0.5, 0.7, 1.0],
    radius: 22.0,
    intensity: 0.5,
    shadows: false,
};
/// A mission ship's engine glow, behind it.
pub const SHIP_ENGINE: Source = Source {
    color: [1.0, 0.6, 0.4],
    radius: 18.0,
    intensity: 0.4,
    shadows: false,
};
/// Distance of the engine glows behind the ship's centre (px).
pub const PLAYER_ENGINE_BACK: f32 = 10.0;
pub const SHIP_ENGINE_BACK: f32 = 8.0;

/// What [`flight_scene`] reads besides the flight.
#[derive(Debug, Clone, Copy)]
pub struct FrameContext<'a> {
    pub world: &'a World,
    /// The palette on screen (the DAC, with the infra-red and flash effects).
    pub palette: &'a Palette,
    /// The back page the drawn tick used (mission ships and explosions, §4.7).
    pub page: i32,
    /// Widescreen margin m = (W − 320) / 2.
    pub margin: i32,
}

/// The view the tick drew.
fn drawn_view(f: &Flight) -> ViewPos {
    let v = f.frame.view;
    ViewPos {
        tile_x: i32::from(v.tile_x),
        tile_y: i32::from(v.tile_y),
        sub_x: i32::from(v.sub_x),
        sub_y: i32::from(v.sub_y),
    }
}

/// The tile a shot was drawn at: `Flight::drawn` holds the shots after their move of one
/// tile along the heading (`weapons.md` §4.3 step 8); as `td2192`'s flight screen.
fn drawn_shot_tile(s: &Shot) -> (u8, u8) {
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

/// Window pixel of a shot (tile offset from the view × 32 + offset), as `flight::shot_look`.
fn shot_window(view: ViewPos, s: &Shot) -> (i32, i32) {
    let (tx, ty) = drawn_shot_tile(s);
    (
        (i32::from(tx) - view.tile_x) * 32 + i32::from(s.off_x),
        (i32::from(ty) - view.tile_y) * 32 + i32::from(s.off_y),
    )
}

/// Whether a cannon shot (kinds 0..=2) is drawn: `flight::shot_look_in` without building its
/// pixel list (a principal heading, the centre within the playfield rows, and the Classic
/// window test or, in a wider frame, the wider window).
fn cannon_drawn(view: ViewPos, s: &Shot, vp: Viewport) -> bool {
    if s.kind > 2 || !matches!(s.heading, 1..=4 | 6..=9) {
        return false;
    }
    let (x, y) = shot_window(view, s);
    let classic = (0..352).contains(&(x + 16));
    let wide = !vp.is_classic() && vp.spans(x + 16 - view.page_origin().0 - 3, 6);
    (34..192).contains(&(y + 16)) && (classic || wide)
}

/// Screen centre (classic coordinates) of a cannon shot's pixel pattern: the pattern's (0, 0)
/// pixel of `flight::shot_look`, whether or not the shot is drawn.
fn cannon_centre(view: ViewPos, s: &Shot) -> (i32, i32) {
    let (x, y) = shot_window(view, s);
    (x + 16 - view.page_origin().0, y - view.sub_y)
}

/// Screen centre (classic coordinates) of a missile's 32 × 32 frame.
fn missile_centre(view: ViewPos, s: &Shot) -> (i32, i32) {
    let (x, y) = flight::sprite_screen(view, shot_window(view, s));
    (x + 16, y + 16)
}

/// Unit vector of a heading's nose in screen space (y down).
fn nose(h: Heading) -> (f32, f32) {
    // sin(i · 22.5°) for i = 0..=4.
    const S: [f32; 5] = [0.0, 0.382_683_43, 0.707_106_77, 0.923_879_5, 1.0];
    let sin = |i: usize| match i % 16 {
        i @ 0..=4 => S[i],
        i @ 5..=8 => S[8 - i],
        i @ 9..=12 => -S[i - 8],
        i => -S[16 - i],
    };
    // Sprite index 0 is nose up, counter-clockwise.
    let i = h.sprite_index();
    (-sin(i), -sin(i + 4))
}

/// The occluder grid around the drawn window: one cell more than the window on each side,
/// widened by the margin.
fn occluders(world: &World, view: ViewPos, margin: i32, occ: &mut Occluders) {
    let (ox, oy) = view.page_origin();
    let extra = (margin.max(0) + CELL - 1) / CELL + 1;
    let (i0, j0) = (-extra, -1);
    occ.cols = (12 + 2 * extra) as usize;
    occ.rows = 8;
    occ.origin = (i0 * CELL - ox + margin, j0 * CELL - oy);
    occ.solid.clear();
    for j in 0..occ.rows as i32 {
        for i in 0..occ.cols as i32 {
            let (x, y) = (view.tile_x + i0 + i, view.tile_y + j0 + j);
            occ.solid.push(world.solid(x as i16, y as i16));
        }
    }
}

/// Builds the light scene of the frame the tick drew into `scene` (§6.1), reusing its
/// buffers. `mode` is the user's setting; infra-red modes limit it to `subtle` (§6.4).
pub fn flight_scene(f: &Flight, ctx: &FrameContext<'_>, mode: LightMode, scene: &mut LightScene) {
    scene.mode = if f.palette_mode != 0 {
        LightMode::Subtle
    } else {
        mode
    };
    let view = drawn_view(f);
    occluders(ctx.world, view, ctx.margin, &mut scene.occluders);
    let lights = &mut scene.lights;
    lights.clear();
    let m = ctx.margin;
    let width = 320 + 2 * m;
    let vp = Viewport::new(width.max(320) as usize);
    let mut push = |src: Source, (x, y): (f32, f32)| {
        let r = src.radius;
        let reach = (-r..width as f32 + r).contains(&x) && (-r..LIT_ROWS as f32 + r).contains(&y);
        if reach {
            lights.push(src.at((x, y)));
        }
    };
    let at = |(x, y): (i32, i32)| ((x + m) as f32, y as f32);

    // Explosions (frames 1..=6), drawn or not: their light reaches past the window.
    let (ox, oy) = view.page_origin();
    for e in f.drawn.explosions.0.iter().filter(|e| e.frame < IDLE) {
        let Some(&intensity) = EXPLOSION_INTENSITY.get(usize::from(e.frame).wrapping_sub(1)) else {
            continue;
        };
        let x = (i32::from(e.tile_x) - view.tile_x) * 32 + i32::from(e.sub_x);
        let y = (i32::from(e.tile_y) - view.tile_y) * 32 + i32::from(e.sub_y) + 1;
        let centre = ((x & !3) - ox + 16, y - 32 - oy + 16);
        push(
            Source {
                intensity,
                ..EXPLOSION
            },
            at(centre),
        );
    }

    // Shots where they are drawn, and one muzzle flash per owner at the centre of the shots
    // it fired last tick (first drawn: cannon age 2, missiles age 1; homing missiles keep no
    // age and get none).
    let mut volleys = [((0i32, 0i32), 0i32); 21];
    for s in f.drawn.shots.0.iter().filter(|s| s.alive()) {
        let (tile, off) = (drawn_shot_tile(s), (s.off_x, s.off_y));
        let (centre, first) = match s.kind {
            k @ 0..=2 => {
                let k = usize::from(k);
                let centre = cannon_centre(view, s);
                if cannon_drawn(view, s, vp) {
                    let rgb = ctx.palette.colors[usize::from(SHOT_COLOUR[k])].to_rgb8();
                    let src = Source {
                        color: linear_rgb(rgb),
                        radius: SHOT_RADIUS[k],
                        intensity: SHOT_INTENSITY,
                        shadows: false,
                    };
                    let (x, y) = at(centre);
                    push(src, (x + 0.5, y + 0.5));
                }
                (centre, s.life == 2)
            }
            4..=6 => {
                let drawn = matches!(
                    flight::shot_look_in(view, s.kind, s.heading, tile, off, vp),
                    Some(ShotLook::Sprite { .. })
                );
                let centre = missile_centre(view, s);
                if drawn {
                    push(MISSILE, at(centre));
                }
                (centre, s.kind != 6 && s.life == 1)
            }
            _ => continue,
        };
        if let (true, Some(v)) = (first, volleys.get_mut(usize::from(s.owner))) {
            v.0.0 += centre.0;
            v.0.1 += centre.1;
            v.1 += 1;
        }
    }
    for &((sx, sy), n) in volleys.iter().filter(|v| v.1 > 0) {
        let n = n as f32;
        push(MUZZLE, ((sx + m) as f32 / n + 0.5, sy as f32 / n + 0.5));
    }

    // Afterburner flame (sprite frame top-left from flight-screens §4.6).
    if f.frame.flame.is_some()
        && let Some((x, y)) = flight::flame_pos(f.player.burst_heading.code())
    {
        push(FLAME, at((x + 16, y + 16)));
    }

    // Engine glows behind the player and the drawn mission ships.
    if f.frame.player_visible {
        let (x, y) = at((SHIP_POS.0 + 16, SHIP_POS.1 + 16));
        let (nx, ny) = nose(f.frame.heading);
        let b = PLAYER_ENGINE_BACK;
        push(PLAYER_ENGINE, (x - nx * b, y - ny * b));
    }
    for s in f.drawn.ships.0.iter().filter(|s| s.alive()) {
        let mark = ShipMark {
            tile_x: s.tile_x,
            tile_y: s.tile_y,
            sub_x: s.sub_x,
            sub_y: s.sub_y,
            marker: s.marker,
        };
        let Some((x, y)) = flight::mission_ship_pos_in(view, ctx.page, &mark, vp) else {
            continue;
        };
        let (x, y) = at(((x & !3) + 16, y + 16));
        let (nx, ny) = nose(Heading::from_code(s.heading).unwrap_or(Heading::N));
        let b = SHIP_ENGINE_BACK;
        push(SHIP_ENGINE, (x - nx * b, y - ny * b));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use td_formats::map::TileMap;
    use td_sim::flight::explosions::Explosion;
    use td_sim::flight::player::View;
    use td_sim::flight::shots::Muzzles;
    use td_sim::flight::tables::TILE_OPEN;

    const VIEW: View = View {
        tile_x: 102,
        tile_y: 104,
        sub_x: 32,
        sub_y: 16,
    };

    /// Open everywhere except tile 1 at cells (106, 105) and (110, 107).
    fn world() -> World {
        let mut cells = vec![0u8; 201 * 201];
        cells[106 * 201 + 105] = 1;
        cells[110 * 201 + 107] = 1;
        let mut attr = [0u8; 256];
        attr[0] = TILE_OPEN;
        World {
            map: TileMap::parse_episode(&cells).unwrap(),
            tile_attr: attr,
            muzzles: Muzzles(vec![0; 342]),
        }
    }

    fn flight() -> Flight {
        let mut f = Flight::new(1);
        f.frame.view = VIEW;
        f.frame.player_visible = false;
        f.frame.flame = None;
        f
    }

    fn palette() -> Palette {
        let mut p = Palette::default();
        p.colors[0x3D] = td_formats::palette::Rgb6 { r: 63, g: 0, b: 0 };
        p
    }

    fn scene_of(f: &Flight, margin: i32, mode: LightMode) -> LightScene {
        let (w, pal) = (world(), palette());
        let ctx = FrameContext {
            world: &w,
            palette: &pal,
            page: 1,
            margin,
        };
        let mut scene = LightScene::default();
        flight_scene(f, &ctx, mode, &mut scene);
        scene
    }

    #[test]
    fn occluders_follow_the_tile_window() {
        let s = scene_of(&flight(), 0, LightMode::Full);
        let occ = &s.occluders;
        // Page origin (16, 0): window cell (i, j) at (32i − 16, 32j); the grid starts at
        // window cell (−1, −1).
        assert_eq!(occ.origin, (-48, -32));
        assert_eq!((occ.cols, occ.rows), (14, 8));
        // Map cell (106, 105) = window (4, 1) = grid (5, 2): screen x 112..144, y 32..64;
        // (110, 107) = window (8, 3) = grid (9, 4).
        let solid: Vec<usize> = (0..occ.solid.len()).filter(|&i| occ.solid[i]).collect();
        assert_eq!(solid, [2 * 14 + 5, 4 * 14 + 9]);
        // Widescreen: the margin shifts and widens the grid.
        let wide = scene_of(&flight(), 54, LightMode::Full).occluders;
        assert_eq!(wide.origin, (-3 * 32 - 16 + 54, -32));
        assert_eq!(wide.cols, 18);
        let i = wide.solid.iter().position(|&s| s).unwrap();
        let cell = (i % wide.cols, i / wide.cols);
        let x = wide.origin.0 + 32 * cell.0 as i32;
        assert_eq!((x, cell.1), (112 + 54, 2), "same screen cell, plus m");
    }

    #[test]
    fn explosions_by_frame_and_infra_red_mode() {
        let mut f = flight();
        for (i, frame) in [1u8, 3, 6, 7, IDLE].into_iter().enumerate() {
            f.drawn.explosions.0[i] = Explosion {
                frame,
                tile_x: 107,
                tile_y: 107,
                sub_x: 16,
                sub_y: 16,
            };
        }
        let s = scene_of(&f, 0, LightMode::Full);
        assert_eq!(s.mode, LightMode::Full);
        let got: Vec<_> = s.lights.iter().map(|l| (l.pos, l.intensity)).collect();
        // explosion_sprite → (176, 113); sprite_screen → (160, 81); centre + 16.
        assert_eq!(
            got,
            [
                ((176.0, 97.0), 1.6),
                ((176.0, 97.0), 1.1),
                ((176.0, 97.0), 0.25)
            ]
        );
        assert!(s.lights.iter().all(|l| l.shadows && l.radius == 56.0));
        f.palette_mode = 1;
        assert_eq!(scene_of(&f, 0, LightMode::Full).mode, LightMode::Subtle);
        // Far away: no light.
        f.drawn.explosions.0[0].tile_x = 150;
        assert_eq!(scene_of(&f, 0, LightMode::Full).lights.len(), 2);
    }

    #[test]
    fn shots_muzzle_flashes_and_engines() {
        let mut f = flight();
        // A player kind-0 shot heading north, drawn at (107, 106) + (16, 16): it has moved one
        // tile on, and it is fresh (age 2).
        let shot = |kind, life, owner, tile_y| Shot {
            tile_x: 107,
            tile_y,
            off_x: 16,
            off_y: 16,
            heading: 8,
            kind,
            life,
            owner,
        };
        f.drawn.shots.0[0] = shot(0, 2, 0, 105);
        f.drawn.shots.0[1] = shot(0, 5, 3, 104);
        f.drawn.shots.0[2] = shot(3, 2, 4, 105);
        let s = scene_of(&f, 0, LightMode::Full);
        // The look puts its centre pixel at (176, 64).
        let Some(ShotLook::Pixels(p)) =
            flight::shot_look(drawn_view(&f), 0, 8, (107, 106), (16, 16))
        else {
            panic!("drawn");
        };
        assert_eq!((p[1].0, p[1].1), (176, 64));
        let red = linear_rgb([255, 0, 0]);
        let lights: Vec<_> = s
            .lights
            .iter()
            .map(|l| (l.pos, l.radius, l.color))
            .collect();
        assert_eq!(
            lights,
            [
                ((176.5, 64.5), 12.0, red),
                ((176.5, 32.5), 12.0, red),
                ((176.5, 64.5), MUZZLE.radius, MUZZLE.color),
            ],
            "kind 3 is never drawn and flashes nothing; only the fresh shot flashes"
        );
        // Widescreen moves everything by m; the player's engine glows behind the ship.
        f.frame.player_visible = true;
        f.frame.heading = Heading::E;
        let s = scene_of(&f, 32, LightMode::Full);
        assert_eq!(s.lights[0].pos, (176.5 + 32.0, 64.5));
        let engine = s.lights.last().unwrap();
        assert_eq!(engine.radius, PLAYER_ENGINE.radius);
        assert_eq!(engine.pos, (160.0 + 32.0 - PLAYER_ENGINE_BACK, 84.0));
    }

    #[test]
    fn noses_point_along_the_heading() {
        let near =
            |a: (f32, f32), b: (f32, f32)| (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6;
        assert!(near(nose(Heading::N), (0.0, -1.0)));
        assert!(near(nose(Heading::W), (-1.0, 0.0)));
        assert!(near(nose(Heading::S), (0.0, 1.0)));
        assert!(near(nose(Heading::E), (1.0, 0.0)));
        let ne = nose(Heading::NE);
        assert!(ne.0 > 0.7 && ne.1 < -0.7);
    }

    #[test]
    fn cannon_visibility_matches_the_shot_look() {
        let view = drawn_view(&flight());
        for width in [320, 428, 560] {
            let vp = Viewport::new(width);
            for kind in 0..=3 {
                for heading in [8, 3, 4, 0x24, 5] {
                    for tx in 92..=118u8 {
                        for ty in 100..=112u8 {
                            for off in [-20i16, 0, 16, 31, 45] {
                                let s = Shot {
                                    tile_x: tx,
                                    tile_y: ty,
                                    off_x: off,
                                    off_y: off / 2,
                                    heading,
                                    kind,
                                    life: 3,
                                    owner: 0,
                                };
                                let look = flight::shot_look_in(
                                    view,
                                    kind,
                                    heading,
                                    drawn_shot_tile(&s),
                                    (s.off_x, s.off_y),
                                    vp,
                                );
                                let want = matches!(look, Some(ShotLook::Pixels(_)));
                                assert_eq!(cannon_drawn(view, &s, vp), want, "{s:?} in {width}");
                                if let Some(ShotLook::Pixels(p)) = look {
                                    let c = cannon_centre(view, &s);
                                    let at = p.iter().any(|&(x, y, _)| (x, y) == c);
                                    assert!(at, "the centre is one of the shot's pixels");
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn flame_and_mission_ship_engines() {
        let mut f = flight();
        f.frame.flame = Some(9);
        f.player.burst_heading = Heading::N;
        f.drawn.ships.0[0].class = 3;
        f.drawn.ships.0[0].tile_x = 107;
        f.drawn.ships.0[0].tile_y = 107;
        f.drawn.ships.0[0].sub_x = 32;
        f.drawn.ships.0[0].sub_y = 32;
        f.drawn.ships.0[0].heading = 2;
        let s = scene_of(&f, 0, LightMode::Full);
        let flame = s.lights[0];
        assert_eq!(
            (flame.pos, flame.radius, flame.shadows),
            ((160.0, 116.0), 32.0, true)
        );
        // mission_ship_pos → (176, 96); heading south: the glow sits above the centre.
        let ship = s.lights[1];
        assert_eq!(ship.pos, (192.0, 112.0 - SHIP_ENGINE_BACK));
        assert_eq!(s.lights.len(), 2);
    }
}
