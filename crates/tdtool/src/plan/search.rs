//! Beam search over held keys.
//!
//! Every [`STEP`] ticks each kept flight is cloned once per action (a direction on the keypad,
//! nothing, or a tap of F6 when a transmission waits) and run for [`STEP`] ticks. Space is held
//! by a fixed rule whenever a hostile ship lies in the line of fire and no friend does. The
//! clones are scored ([`score`]) and the best [`BEAM`] distinct ones are kept. A flight whose
//! player is destroyed or whose mission is lost is dropped; the first one that succeeds ends
//! the search. Weapons mode ([`Options::weapons`], VE-5) adds Tab and afterburner taps and
//! fires missiles by taps (see [`weapons_fire`]).

use rayon::prelude::*;
use td_sim::flight::{Episode, TickResult};
use td_sim::{Flight, FlightKey, World};

use super::nav::{FAR, Field};
use super::{Plan, schedule};
use crate::flight_setup::FlightSetup;

/// Ticks per decision.
pub const STEP: usize = 3;
/// Flights kept per decision (default; `tdtool plan --beam`).
pub const BEAM: usize = 24;
/// Beams kept for backing off when every branch fails.
const HISTORY: usize = 40;
/// Back-offs before the search gives up.
const MAX_BACKTRACKS: u32 = 60;

/// Guidance markers (mission-logic §2).
const MARK_TARGET: u8 = 0x32;
const MARK_FRIEND: u8 = 0x39;

/// Search options (`tdtool plan`).
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Give up after this many ticks.
    pub max_ticks: u32,
    /// Print the search's progress.
    pub verbose: bool,
    /// Flights kept per decision.
    pub beam: usize,
    /// With alt muzzle bytes: tolerate a difference between the two worlds.
    pub dual: bool,
    /// Also use the missile weapon (Tab, then Space taps; weapons §4.1) and the afterburner
    /// (keypad +, classes 2 and 5; flight-model §4.7), with a score bonus for the first missiles
    /// and bursts so that the plan covers them (VE-5).
    pub weapons: bool,
}

/// One decision: keypad direction keys, an F6 tap with every key released, a Tab tap with
/// every key released, or keypad + tapped in the first tick with direction keys held.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Keys(&'static [FlightKey]),
    Comm,
    Tab,
    Burn(&'static [FlightKey]),
}

/// The directions offered with an afterburner tap.
const BURN_DIRS: [&[FlightKey]; 5] = [
    &[],
    &[FlightKey::Kp8],
    &[FlightKey::Kp2],
    &[FlightKey::Kp4],
    &[FlightKey::Kp6],
];

/// Weapons mode: longest Space hold in ticks. A missile launch clears the Space byte and the
/// keyboard's auto-repeat would set it again after 500 ms, which a replay cannot express
/// ([`schedule::MAX_CLEARABLE_HOLD`]).
const MAX_SPACE_HOLD: u32 = schedule::MAX_CLEARABLE_HOLD;
/// Weapons mode: score bonus per missile fired and per afterburner burst, for the first few.
const MISSILE_BONUS: i64 = 40_000;
const BURST_BONUS: i64 = 30_000;
const BONUS_CAP: u32 = 3;

const DIRS: [&[FlightKey]; 9] = [
    &[],
    &[FlightKey::Kp8],
    &[FlightKey::Kp2],
    &[FlightKey::Kp4],
    &[FlightKey::Kp6],
    &[FlightKey::Kp8, FlightKey::Kp4],
    &[FlightKey::Kp8, FlightKey::Kp6],
    &[FlightKey::Kp2, FlightKey::Kp4],
    &[FlightKey::Kp2, FlightKey::Kp6],
];

/// A kept flight and how it got there (arena of decisions).
#[derive(Debug, Clone)]
struct Node {
    flight: Flight,
    /// The same flight with the other muzzle bytes (`--alt-muzzles`), kept identical.
    alt: Option<Flight>,
    /// Keys down in the last tick.
    keys: Vec<FlightKey>,
    parent: usize,
    inputs: Vec<u128>,
    score: i64,
    /// The flight succeeded in this decision.
    won: bool,
    /// `alt` no longer matches `flight` (dual mode: both must still succeed on the same tick).
    diverged: bool,
    /// Consecutive ticks Space has been held (weapons mode).
    space_held: u32,
    /// Missiles launched and afterburner bursts started so far (weapons mode).
    missiles_fired: u32,
    bursts: u32,
}

/// Per-flight constants for scoring.
#[derive(Debug, Clone)]
struct Goals {
    /// Distances to the cells that count as home (the base test's view tiles + (5, 3)).
    home_field: Field,
    /// Flight 19: distances to the V.C.I. (`Msn_UpdateEvents`: phase 1, viewY = 70,
    /// 117 < viewX < 124; mission-logic §5.19), the goal while the phase is 1.
    vci_field: Option<Field>,
    /// Player armour + shields at the start.
    start_hp: i32,
    /// The base test has no phase condition: steer home from the start.
    home_any_phase: bool,
    /// The phase advances when the friends have left (escort flights whose friends drive to
    /// an exit): fewer live friends is progress, and the player waits (no distance term)
    /// instead of parking on their path before the phase is 100.
    friends_leave: bool,
    /// Weapons mode ([`Options::weapons`]).
    weapons: bool,
}

/// Escort flights whose phase advances when every friend has gone (E3 §5.6).
fn friends_leave(ep: Episode, flight: u8) -> bool {
    matches!((ep, flight), (Episode::E3, 6))
}

/// Flight 19's V.C.I. view tiles.
fn vci_views() -> Vec<(i16, i16)> {
    (118..=123).map(|x| (x, 70)).collect()
}

/// The same as player cells.
fn vci_cells() -> Vec<(i16, i16)> {
    vci_views()
        .into_iter()
        .map(|(x, y)| (x + 5, y + 3))
        .collect()
}

/// The view tiles that pass a flight's base test (E1 mission-logic §4.4; E2 e2-mission-logic
/// §4.4 and §5; E3 e3-mission-logic §4.3 and §5). The sub-tile limits of some tests
/// (`viewSubY <= 31..36`) always hold for a view at rest.
pub fn home_views(ep: Episode, flight: u8) -> Vec<(i16, i16)> {
    let all = |pred: &dyn Fn(i16, i16) -> bool| -> Vec<(i16, i16)> {
        (0..201i16)
            .flat_map(|x| (0..201i16).map(move |y| (x, y)))
            .filter(|&(x, y)| pred(x, y))
            .collect()
    };
    match (ep, flight) {
        (Episode::E1, 12 | 15) => vec![(102, 99), (102, 100)],
        (Episode::E1, 13) => all(&|x, y| (175..=177).contains(&x) && (152..=154).contains(&y)),
        (Episode::E1, _) => vec![(102, 104)],
        (Episode::E2, 5) => all(&|x, y| (178..=183).contains(&x) && y == 35),
        (Episode::E2, 6) => all(&|x, y| (118..=123).contains(&x) && y == 70),
        (Episode::E2, 8) => all(&|x, _| x == 7),
        (Episode::E2, 9..=19) => vec![(87, 28)],
        (Episode::E2, 20) => all(&|_, y| y <= 7),
        (Episode::E2, _) => vec![(102, 104)],
        (Episode::E3, 1..=5) => vec![(167, 66)],
        (Episode::E3, 6) => all(&|x, y| x == 6 && (101..=114).contains(&y)),
        (Episode::E3, 11) => all(&|x, y| (160..=161).contains(&x) && (75..=76).contains(&y)),
        (Episode::E3, 13) => all(&|x, y| (36..=37).contains(&x) && (44..=45).contains(&y)),
        (Episode::E3, 18) => vec![(23, 31)],
        (Episode::E3, _) => vec![(17, 164)],
    }
}

/// The base test's limit on `viewSubY`, where it has one (E2 §4.4: 36, 35, 31; E3 §4.3: 31
/// for flights 1–5, `home17` 35).
fn home_sub_y_max(ep: Episode, flight: u8) -> Option<i16> {
    match (ep, flight) {
        (Episode::E2, 1..=3) => Some(36),
        (Episode::E2, 7) => Some(35),
        (Episode::E2, 9..=19) => Some(31),
        (Episode::E3, 1..=5) => Some(31),
        (Episode::E3, 7..=10 | 12 | 14..=17 | 19 | 21) => Some(35),
        _ => None,
    }
}

/// Flights whose base test has no phase condition (E2 §5.6, §5.7; E3 §5.11, §5.13, §5.14,
/// §5.21).
fn home_any_phase(ep: Episode, flight: u8) -> bool {
    matches!(
        (ep, flight),
        (Episode::E2, 6 | 7) | (Episode::E3, 11 | 13 | 14 | 21)
    )
}

/// The player's cell on the map (the ship at rest sits on view + (5, 3)).
fn player_cell(f: &Flight) -> (i16, i16) {
    let v = f.player.view;
    (
        v.tile_x + 4 + (v.sub_x + 16) / 32,
        v.tile_y + 2 + (v.sub_y + 4) / 32,
    )
}

/// Direction of a principal heading (keypad code), else `None`.
fn heading_dir(h: u8) -> Option<(i32, i32)> {
    Some(match h {
        8 => (0, -1),
        2 => (0, 1),
        4 => (-1, 0),
        6 => (1, 0),
        9 => (1, -1),
        7 => (-1, -1),
        1 => (-1, 1),
        3 => (1, 1),
        _ => return None,
    })
}

/// Distance along the line of fire to the first hit on a ship at world pixel `w` by a volley
/// from `o` along `d` (shots step 32 px a tick for 25 ticks, hit box ±19 px), or `None`.
fn on_line(o: (i32, i32), d: (i32, i32), w: (i32, i32), slack: i32) -> Option<i32> {
    for n in 0..25 {
        let p = (o.0 + 32 * n * d.0, o.1 + 32 * n * d.1);
        if (w.0 - p.0).abs() < 20 + slack && (w.1 - p.1).abs() < 20 + slack {
            return Some(n);
        }
    }
    None
}

/// True when the volley path from `o` along `d` stays in open cells for `n` steps (the spawn
/// cell included: a shot spawned in a solid cell dies at once, weapons §4.3 step 2).
fn clear_path(world: &World, o: (i32, i32), d: (i32, i32), n: i32) -> bool {
    (0..=n).all(|k| {
        let p = (o.0 + 32 * k * d.0, o.1 + 32 * k * d.1);
        !world.solid((p.0.div_euclid(32)) as i16, (p.1.div_euclid(32)) as i16)
    })
}

/// What the current heading's line of fire reaches first: `Some(marker)` of the nearest live
/// ship on a clear line (friends shadow the ships behind them), `None` when nothing.
fn line_of_fire(f: &Flight, world: &World) -> Option<u8> {
    if f.mission_over {
        return None;
    }
    // The volley of the next tick (step 33) leaves on the heading after that tick's turn step
    // (step 3). Never fire on a two-digit heading: the muzzle offsets then come from DGROUP
    // bytes past the table that depend on the front end before the flight (U-160, weapons
    // rev 5 §3.3), so a plan that used them would hold only after a direct start.
    let p = &f.player;
    let next = match (p.target != p.heading, p.turn) {
        (true, td_sim::flight::player::Turn::Clockwise) => p.heading.cw(),
        (true, td_sim::flight::player::Turn::CounterClockwise) => p.heading.ccw(),
        _ => p.heading,
    };
    let d = heading_dir(next.code())?;
    let v = f.player.view;
    // Volley origin (weapons §4.2): tile (viewX + 4, viewY + 3), offsets ≈ (subX − 16 + 16,
    // subY − 28 + 11) for the middle muzzles.
    let o = (
        (i32::from(v.tile_x) + 4) * 32 + i32::from(v.sub_x),
        (i32::from(v.tile_y) + 3) * 32 + i32::from(v.sub_y) - 17,
    );
    f.ships
        .0
        .iter()
        .filter(|s| s.alive())
        .filter_map(|s| {
            let n = on_line(o, d, s.world(), 6)?;
            clear_path(world, o, d, n).then_some((n, s.marker))
        })
        .min_by_key(|&(n, m)| (n, m != MARK_FRIEND))
        .map(|(_, m)| m)
}

/// The fire rule: hold Space when a hostile ship is in the line of fire, closer than any friend.
fn want_fire(f: &Flight, world: &World) -> bool {
    line_of_fire(f, world).is_some_and(|m| m != MARK_FRIEND)
}

/// The missile weapon (4, 5 or 6) of the player's class, if it has one (weapons §3.2).
fn missile_weapon(f: &Flight) -> Option<u8> {
    td_sim::flight::tables::player_weapons(f.player.class)
        .map(|(_, m, _)| m)
        .filter(|&m| m > 3)
}

/// Weapons mode: whether Space goes down this tick. With the missile weapon selected, a new
/// press launches one missile (it clears the Space byte; weapons §4.1, §4.2): press when a
/// missile is ready and its target is hostile (kinds 4 and 5: the line of fire; kind 6, the
/// homing missile: the live slot nearest to the launch tile, which it steers to), never two
/// ticks in a row. With the volley weapon: the fire rule, released for a tick after
/// [`MAX_SPACE_HOLD`] ticks.
fn weapons_fire(f: &Flight, world: &World, space_held: u32) -> bool {
    if f.weapon > 3 {
        if space_held > 0 || f.missiles == 0 || f.missile_cooldown != 0 || f.mission_over {
            return false;
        }
        if f.weapon == 6 {
            let v = f.player.view;
            let (px, py) = (i32::from(v.tile_x) + 4, i32::from(v.tile_y) + 3);
            return f
                .ships
                .0
                .iter()
                .filter(|s| s.alive())
                .min_by_key(|s| {
                    let (dx, dy) = (i32::from(s.tile_x) - px, i32::from(s.tile_y) - py);
                    dx * dx + dy * dy
                })
                .is_some_and(|s| s.marker == MARK_TARGET);
        }
        return want_fire(f, world);
    }
    space_held < MAX_SPACE_HOLD && want_fire(f, world)
}

/// Cells from which a volley can reach a ship on cell `t`: along each principal direction,
/// 2..=9 cells away while the cells stay open.
fn firing_cells(world: &World, t: (i16, i16), out: &mut Vec<(i16, i16)>) {
    for d in [
        (0, 1),
        (0, -1),
        (1, 0),
        (-1, 0),
        (1, 1),
        (1, -1),
        (-1, 1),
        (-1, -1),
    ] {
        for k in 1..=9 {
            let c = (t.0 + d.0 * k, t.1 + d.1 * k);
            if world.solid(c.0, c.1) {
                break;
            }
            if k >= 2 {
                out.push(c);
            }
        }
    }
}

/// Distance to the goal of `field` in pixels: 32 per cell, less the player's progress inside
/// its cell toward the best neighbour, less a little for stored speed toward it (speeds up to
/// 16 do not move the ship yet, flight-model §4.2, but they will).
fn fine_distance(f: &Flight, field: &Field) -> i64 {
    let v = f.player.view;
    let fx = (i64::from(v.tile_x) + 4) * 32 + i64::from(v.sub_x) + 16;
    let fy = (i64::from(v.tile_y) + 2) * 32 + i64::from(v.sub_y) + 4;
    let (cx, cy) = (fx.div_euclid(32) as i16, fy.div_euclid(32) as i16);
    let (ox, oy) = (fx.rem_euclid(32), fy.rem_euclid(32));
    let here = field.at(cx, cy);
    if here == FAR {
        return 400 * 32;
    }
    let sp = f.player.speed;
    let mut best = i64::from(here) * 32;
    for (dx, dy) in [(1i16, 0i16), (-1, 0), (0, 1), (0, -1)] {
        let n = field.at(cx + dx, cy + dy);
        if n == FAR || n >= here {
            continue;
        }
        // Offset toward that neighbour (0..31) and the speed pointing at it.
        let (progress, speed) = match (dx, dy) {
            (1, _) => (ox, sp.right),
            (-1, _) => (31 - ox, sp.left),
            (_, 1) => (oy, sp.down),
            _ => (31 - oy, sp.up),
        };
        let d = i64::from(here) * 32 - progress - i64::from(speed) / 2;
        best = best.min(d);
    }
    best.min(400 * 32)
}

/// Scores a flight (higher is better). Phase progress dominates, then the hit points of live
/// targets, then the distance to the nearest target (or home once the objectives are done),
/// then the damage the player and friends took.
fn score(f: &Flight, world: &World, g: &Goals) -> i64 {
    let phase = i64::from(f.mission.phase);
    let rank = if phase >= 100 { 60 } else { phase };
    let mut s = rank * 1_000_000;
    let mut target_hp = 0i64;
    let mut targets = vec![];
    let mut friend_hp = 0i64;
    for (i, sh) in f.ships.0.iter().enumerate() {
        if !sh.alive() {
            continue;
        }
        match sh.marker {
            MARK_TARGET => {
                target_hp += i64::from(sh.armour.max(0)) + i64::from(sh.shields.max(0));
                targets.push(i);
            }
            MARK_FRIEND if g.friends_leave => friend_hp -= 10,
            MARK_FRIEND => friend_hp += i64::from(sh.armour.max(0)) + i64::from(sh.shields.max(0)),
            _ => {}
        }
    }
    s -= targets.len() as i64 * 20_000;
    s -= target_hp * 300;
    s += friend_hp * 2_000;
    let fire_field;
    // Goals defined by view tiles (the base test, the V.C.I.): near them, steer to the exact
    // view tile, which the cell distance alone does not resolve.
    let v = f.player.view;
    let view_term = |views: &[(i16, i16)]| {
        views
            .iter()
            .map(|&(x, y)| i64::from((v.tile_x - x).abs() + (v.tile_y - y).abs()))
            .min()
            .unwrap_or(0)
            .min(4)
            * 32
    };
    let field = if let (Some(vci), 1) = (&g.vci_field, phase) {
        s -= view_term(&vci_views()) * 400 / 32;
        Some(vci)
    } else if g.friends_leave && phase < 100 && targets.is_empty() {
        None
    } else if phase >= 100 || targets.is_empty() || g.home_any_phase {
        s -= view_term(&home_views(f.episode, f.number)) * 400 / 32;
        // Near the goal, the distance in pixels to the nearest goal view at rest (offsets
        // (32, 16), mission-setup §3.1), so that walls around a narrow goal are worked round;
        // on the goal tile, the base test's viewSubY limit.
        let px = |t: i16, s: i16| i64::from(t) * 32 + i64::from(s);
        let near = home_views(f.episode, f.number)
            .iter()
            .map(|&(x, y)| {
                (px(v.tile_x, v.sub_x) - px(x, 32)).abs()
                    + (px(v.tile_y, v.sub_y) - px(y, 16)).abs()
            })
            .min()
            .unwrap_or(0);
        // Capped, not cut off: a cut-off makes entering the radius cost score, and the beam
        // then parks just outside it (VE-5: E3 flight 7 at 261 px from `home17`).
        s -= near.min(256) * 20;
        if let Some(max) = home_sub_y_max(f.episode, f.number)
            && home_views(f.episode, f.number).contains(&(v.tile_x, v.tile_y))
        {
            s -= i64::from((v.sub_y - max).max(0)) * 400;
        }
        Some(&g.home_field)
    } else {
        // Distance to the nearest cell with a line of fire on a target.
        let mut cells = vec![];
        for &i in &targets {
            let sh = &f.ships.0[i];
            firing_cells(
                world,
                (i16::from(sh.tile_x), i16::from(sh.tile_y)),
                &mut cells,
            );
        }
        fire_field = Field::from_cells(world, &cells);
        Some(&fire_field)
    };
    if let Some(field) = field {
        s -= fine_distance(f, field) * 400 / 32;
    }
    if line_of_fire(f, world) == Some(MARK_TARGET) {
        s += 30_000;
    }
    let hp = i32::from(f.player.armour) * 4 + i32::from(f.player.shields);
    s -= i64::from(g.start_hp - hp) * 1_500;
    s
}

/// Weapons mode: the bonus for the first missiles and afterburner bursts.
fn weapons_bonus(n: &Node) -> i64 {
    i64::from(n.missiles_fired.min(BONUS_CAP)) * MISSILE_BONUS
        + i64::from(n.bursts.min(BONUS_CAP)) * BURST_BONUS
}

/// One replay tick ([`td_sim::replay::step`]) that also tells whether the comm viewer opened.
fn step(f: &mut Flight, world: &World, w: u128) -> (TickResult, bool) {
    let c = td_sim::TickControls::from_word(w);
    let mut r = f.tick(world, &c, &mut vec![]);
    let mut comm = false;
    while let TickResult::Modal(m) = r {
        comm |= m == td_sim::Modal::CommViewer;
        r = f.resume(world, &td_sim::TickControls::default(), &mut vec![]);
    }
    (r, comm)
}

/// Score handicap of a diverged node (dual mode), so that matching nodes are preferred.
const DIVERGED: i64 = 50_000;

/// Runs one decision from `node`: [`STEP`] ticks of `action`. With `alt_world`, the same inputs
/// also fly the flight with the capture's muzzle bytes (U-160): the candidate is dropped when
/// the two differ, unless `dual`; then both must stay alive, open the comm viewer in the same
/// ticks, and end with success on the same tick.
fn expand(
    node: &Node,
    action: Action,
    world: &World,
    alt_world: Option<&World>,
    g: &Goals,
    dual: bool,
) -> Option<Node> {
    let mut f = node.flight.clone();
    let mut alt = node.alt.clone();
    let mut diverged = node.diverged;
    let mut prev = node.keys.clone();
    let mut inputs = Vec::with_capacity(STEP);
    let (mut space_held, mut missiles_fired, mut bursts) =
        (node.space_held, node.missiles_fired, node.bursts);
    for t in 0..STEP {
        let mut down: Vec<FlightKey> = match action {
            Action::Keys(k) | Action::Burn(k) => k.to_vec(),
            Action::Comm | Action::Tab => vec![],
        };
        match action {
            Action::Comm if t == 0 => down.push(FlightKey::F6),
            Action::Tab if t == 0 => down.push(FlightKey::Tab),
            Action::Burn(_) if t == 0 => down.push(FlightKey::KpPlus),
            _ => {}
        }
        let fire = match action {
            Action::Comm => false,
            _ if g.weapons => weapons_fire(&f, world, space_held),
            _ => want_fire(&f, world),
        };
        if fire {
            down.push(FlightKey::Space);
        }
        space_held = if fire { space_held + 1 } else { 0 };
        let w = schedule::word(&prev, &down);
        inputs.push(w);
        // The bonus counts only what happens before the phase is 100: afterwards the way home
        // must not compete with it.
        let (missiles, burst, early) = (f.missiles, f.player.burst_frame, f.mission.phase < 100);
        let (r, comm) = step(&mut f, world, w);
        missiles_fired += u32::from(early && f.missiles < missiles);
        bursts += u32::from(early && burst == 0 && f.player.burst_frame != 0);
        if let (Some(a), Some(aw)) = (alt.as_mut(), alt_world) {
            let (ra, comm_a) = step(a, aw, w);
            if !diverged && super::core_hash(a) != super::core_hash(&f) {
                // The muzzle bytes past the table (U-160) changed the flight.
                if !dual {
                    return None;
                }
                diverged = true;
            }
            if comm_a != comm
                || (ra == TickResult::Finished) != (r == TickResult::Finished)
                || a.lost
                || (a.mission_over && !a.succeeded)
                || (ra == TickResult::Finished && !a.succeeded)
            {
                return None;
            }
        }
        prev = down;
        if f.lost || (f.mission_over && !f.succeeded) {
            return None;
        }
        if r == TickResult::Finished {
            if f.succeeded {
                let mut n = Node {
                    flight: f,
                    alt,
                    keys: prev,
                    parent: 0,
                    inputs,
                    score: i64::MAX,
                    won: true,
                    diverged,
                    space_held,
                    missiles_fired,
                    bursts,
                };
                n.score = i64::MAX - n.inputs.len() as i64;
                return Some(n);
            }
            return None;
        }
    }
    let mut score = score(&f, world, g);
    if diverged && let (Some(a), Some(aw)) = (alt.as_ref(), alt_world) {
        score = score.min(self::score(a, aw, g)) - DIVERGED;
    }
    let mut n = Node {
        flight: f,
        alt,
        keys: prev,
        parent: 0,
        inputs,
        score,
        won: false,
        diverged,
        space_held,
        missiles_fired,
        bursts,
    };
    if g.weapons {
        n.score += weapons_bonus(&n);
    }
    Some(n)
}

/// The decisions offered from `node`: the nine direction sets, F6 when a transmission waits,
/// and in weapons mode, before the phase is 100: a Tab tap (class with a missile weapon, the
/// missile weapon selected, or missiles left while fewer than [`BONUS_CAP`] were fired) and
/// afterburner taps (classes 2 and 5, no burst running, a principal heading, fewer than
/// [`BONUS_CAP`] bursts; flight-model §4.7).
fn actions(node: &Node, weapons: bool) -> Vec<Action> {
    let f = &node.flight;
    let mut v: Vec<Action> = DIRS.iter().map(|&d| Action::Keys(d)).collect();
    if f.mission.comm_pending && super::transmission_waits(f) && !f.comm_request {
        v.push(Action::Comm);
    }
    if weapons && !f.mission_over && f.mission.phase < 100 {
        if missile_weapon(f).is_some()
            && (f.weapon > 3 || (f.missiles > 0 && node.missiles_fired < BONUS_CAP))
        {
            v.push(Action::Tab);
        }
        if matches!(f.player.class, 2 | 5)
            && f.player.burst_frame == 0
            && f.player.heading.is_principal()
            && node.bursts < BONUS_CAP
        {
            v.extend(BURN_DIRS.iter().map(|&d| Action::Burn(d)));
        }
    }
    v
}

/// A coarse key for keeping the beam diverse: position to 8 px, speeds, heading, phase, the
/// live slots.
fn diversity_key(f: &Flight) -> (i16, i16, i16, i16, u8, u8, u32, [u8; 4]) {
    let v = f.player.view;
    let live = f
        .ships
        .0
        .iter()
        .enumerate()
        .filter(|(_, s)| s.alive())
        .fold(0u32, |m, (i, _)| m | 1 << i);
    let sp = f.player.speed;
    (
        v.tile_x,
        v.tile_y,
        v.sub_x / 8,
        v.sub_y / 8,
        f.player.heading.code(),
        f.mission.phase,
        live,
        [sp.up / 4, sp.down / 4, sp.left / 4, sp.right / 4],
    )
}

/// Searches for a winning plan.
pub fn plan(setup: &FlightSetup, alt: Option<&FlightSetup>, opts: &Options) -> Option<Plan> {
    let Options {
        max_ticks,
        verbose,
        beam: beam_width,
        dual,
        weapons,
    } = *opts;
    let world = &setup.world;
    let alt_world = alt.map(|a| &a.world);
    let start = setup.start();
    let home = home_views(setup.episode, setup.number)
        .into_iter()
        .map(|(x, y)| (x + 5, y + 3))
        .collect::<Vec<_>>();
    let g = Goals {
        home_field: Field::from_cells(world, &home),
        vci_field: (setup.episode == Episode::E1 && setup.number == 19)
            .then(|| Field::from_cells(world, &vci_cells())),
        start_hp: i32::from(start.player.armour) * 4 + i32::from(start.player.shields),
        home_any_phase: home_any_phase(setup.episode, setup.number),
        friends_leave: friends_leave(setup.episode, setup.number),
        weapons,
    };
    let mut arena: Vec<(usize, Vec<u128>)> = vec![(usize::MAX, vec![])];
    let mut beam = vec![Node {
        score: score(&start, world, &g),
        flight: start,
        alt: alt.map(FlightSetup::start),
        keys: vec![],
        parent: 0,
        inputs: vec![],
        won: false,
        diverged: false,
        space_held: 0,
        missiles_fired: 0,
        bursts: 0,
    }];
    let mut ticks = 0u32;
    // Recent beams (with the arena length and tick count after them) to back off to when every
    // branch fails, and the diversity keys of the nodes abandoned that way (taboo).
    let mut history: Vec<(Vec<Node>, usize, u32)> = vec![(beam.clone(), arena.len(), 0)];
    let mut taboo = std::collections::BTreeSet::new();
    let mut backtracks = 0u32;
    let mut streak = 0u32;
    let mut furthest = 0u32;
    while ticks < max_ticks {
        let children: Vec<Node> = beam
            .par_iter()
            .enumerate()
            .flat_map_iter(|(i, node)| {
                actions(node, weapons)
                    .into_iter()
                    .filter_map(|a| {
                        expand(node, a, world, alt_world, &g, dual).map(|mut n| {
                            n.parent = i;
                            n
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        // Keep the best distinct children that were not abandoned before.
        let mut children = children;
        children.sort_by_key(|n| std::cmp::Reverse(n.score));
        if let Some(w) = children.iter().find(|n| n.won) {
            let mut inputs = w.inputs.clone();
            let mut at = beam_arena_index(&arena, beam.len(), w.parent);
            while at != 0 {
                let (p, ins) = &arena[at];
                let mut v = ins.clone();
                v.extend(inputs);
                inputs = v;
                at = *p;
            }
            let comm_ticks = schedule::comm_ticks(setup, &inputs);
            return Some(Plan { inputs, comm_ticks });
        }
        let mut keys = std::collections::BTreeSet::new();
        let mut next = vec![];
        for c in children {
            let k = diversity_key(&c.flight);
            if !taboo.contains(&k) && keys.insert(k) {
                next.push(c);
                if next.len() == beam_width {
                    break;
                }
            }
        }
        if next.is_empty() {
            // Every branch failed (destroyed, lost, or changed by the muzzle bytes past the
            // table, U-160, often by an enemy's volley the player cannot prevent in 3 ticks):
            // back off 2, 4, ... 32 decisions and take other branches there.
            if verbose {
                eprintln!("tick {ticks}: every branch failed (backtrack {backtracks})");
            }
            if history.len() < 2 || backtracks >= MAX_BACKTRACKS {
                return None;
            }
            backtracks += 1;
            let k = (2usize << streak.min(4)).min(history.len() - 1);
            streak += 1;
            for (b, _, _) in history.iter().rev().take(k) {
                for n in b {
                    taboo.insert(diversity_key(&n.flight));
                }
            }
            history.truncate(history.len() - k);
            let (b, alen, t) = history.last().cloned().expect("a beam");
            beam = b;
            arena.truncate(alen);
            ticks = t;
            continue;
        }
        if ticks + STEP as u32 > furthest {
            furthest = ticks + STEP as u32;
            streak = 0;
        }
        // Record the kept nodes in the arena.
        let base = arena.len() - beam.len();
        let mut new_beam = vec![];
        for mut c in next {
            let parent_arena = base + c.parent;
            arena.push((parent_arena, std::mem::take(&mut c.inputs)));
            new_beam.push(c);
        }
        ticks += STEP as u32;
        if verbose && ticks.is_multiple_of(150) {
            let b = &new_beam[0].flight;
            let cell = player_cell(b);
            let here = Field::new(world, cell);
            let targets: Vec<String> = b
                .ships
                .0
                .iter()
                .enumerate()
                .filter(|(_, s)| s.alive() && s.marker == MARK_TARGET)
                .map(|(i, s)| {
                    format!(
                        "{}@({},{})a{}d{}",
                        i + 1,
                        s.tile_x,
                        s.tile_y,
                        s.armour,
                        here.at(i16::from(s.tile_x), i16::from(s.tile_y))
                    )
                })
                .collect();
            let v = b.player.view;
            let near: Vec<String> = b
                .ships
                .0
                .iter()
                .enumerate()
                .filter(|(_, s)| {
                    s.alive()
                        && (i16::from(s.tile_x) - cell.0).abs() <= 4
                        && (i16::from(s.tile_y) - cell.1).abs() <= 4
                })
                .map(|(i, s)| format!("{}:c{}@({},{})", i + 1, s.class, s.tile_x, s.tile_y))
                .collect();
            if !near.is_empty() {
                eprintln!("  near the player: {}", near.join(" "));
            }
            eprintln!(
                "tick {ticks}: score {} phase {} cell ({},{}) view ({},{})+({},{}) armour {} kills {} targets {}",
                new_beam[0].score,
                b.mission.phase,
                cell.0,
                cell.1,
                v.tile_x,
                v.tile_y,
                v.sub_x,
                v.sub_y,
                b.player.armour,
                b.kills,
                targets.join(" ")
            );
        }
        beam = new_beam;
        history.push((beam.clone(), arena.len(), ticks));
        if history.len() > HISTORY {
            history.remove(0);
        }
    }
    None
}

/// Arena index of beam entry `i` (the beam is always the last `len` arena entries).
fn beam_arena_index(arena: &[(usize, Vec<u128>)], len: usize, i: usize) -> usize {
    arena.len() - len + i
}
