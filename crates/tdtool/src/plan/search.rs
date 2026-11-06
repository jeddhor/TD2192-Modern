//! Beam search over held keys.
//!
//! Every [`STEP`] ticks each kept flight is cloned once per action (a direction on the keypad,
//! nothing, or a tap of F6 when a transmission waits) and run for [`STEP`] ticks. Space is held
//! by a fixed rule whenever a hostile ship lies in the line of fire and no friend does. The
//! clones are scored ([`score`]) and the best [`BEAM`] distinct ones are kept. A flight whose
//! player is destroyed or whose mission is lost is dropped; the first one that succeeds ends
//! the search.

use rayon::prelude::*;
use td_sim::flight::TickResult;
use td_sim::flight::logic::transmission;
use td_sim::{Flight, FlightKey, World};

use super::Plan;
use super::nav::{FAR, Field};
use super::schedule;
use crate::flight_setup::E1Flight;

/// Ticks per decision.
pub const STEP: usize = 3;
/// Flights kept per decision.
pub const BEAM: usize = 24;

/// Guidance markers (mission-logic §2).
const MARK_TARGET: u8 = 0x32;
const MARK_FRIEND: u8 = 0x39;

/// One decision: keypad direction keys, or an F6 tap with every key released.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Keys(&'static [FlightKey]),
    Comm,
}

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
    /// Keys down in the last tick.
    keys: Vec<FlightKey>,
    parent: usize,
    inputs: Vec<u64>,
    score: i64,
    /// The flight succeeded in this decision.
    won: bool,
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

fn home_views(flight: u8) -> Vec<(i16, i16)> {
    match flight {
        12 | 15 => vec![(102, 99), (102, 100)],
        13 => (175..=177)
            .flat_map(|x| (152..=154).map(move |y| (x, y)))
            .collect(),
        _ => vec![(102, 104)],
    }
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
    let d = heading_dir(f.player.heading.code())?;
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
        vci
    } else if phase >= 100 || targets.is_empty() {
        s -= view_term(&home_views(f.number)) * 400 / 32;
        &g.home_field
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
        &fire_field
    };
    s -= fine_distance(f, field) * 400 / 32;
    if line_of_fire(f, world) == Some(MARK_TARGET) {
        s += 30_000;
    }
    let hp = i32::from(f.player.armour) * 4 + i32::from(f.player.shields);
    s -= i64::from(g.start_hp - hp) * 1_500;
    s
}

/// Runs one decision from `node`: [`STEP`] ticks of `action`.
fn expand(node: &Node, action: Action, world: &World, g: &Goals) -> Option<Node> {
    let mut f = node.flight.clone();
    let mut prev = node.keys.clone();
    let mut inputs = Vec::with_capacity(STEP);
    for t in 0..STEP {
        let mut down: Vec<FlightKey> = match action {
            Action::Keys(k) => k.to_vec(),
            Action::Comm => vec![],
        };
        if action == Action::Comm && t == 0 {
            down.push(FlightKey::F6);
        }
        if action != Action::Comm && want_fire(&f, world) {
            down.push(FlightKey::Space);
        }
        let w = schedule::word(&prev, &down);
        inputs.push(w);
        let r = td_sim::replay::step(&mut f, world, w);
        prev = down;
        if f.lost || (f.mission_over && !f.succeeded) {
            return None;
        }
        if r == TickResult::Finished {
            if f.succeeded {
                let mut n = Node {
                    flight: f,
                    keys: prev,
                    parent: 0,
                    inputs,
                    score: i64::MAX,
                    won: true,
                };
                n.score = i64::MAX - n.inputs.len() as i64;
                return Some(n);
            }
            return None;
        }
    }
    let score = score(&f, world, g);
    Some(Node {
        flight: f,
        keys: prev,
        parent: 0,
        inputs,
        score,
        won: false,
    })
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
pub fn plan(setup: &E1Flight, max_ticks: u32, verbose: bool) -> Option<Plan> {
    let world = &setup.world;
    let start = setup.start();
    let home = home_views(setup.number)
        .into_iter()
        .map(|(x, y)| (x + 5, y + 3))
        .collect::<Vec<_>>();
    let g = Goals {
        home_field: Field::from_cells(world, &home),
        vci_field: (setup.number == 19).then(|| Field::from_cells(world, &vci_cells())),
        start_hp: i32::from(start.player.armour) * 4 + i32::from(start.player.shields),
    };
    let mut arena: Vec<(usize, Vec<u64>)> = vec![(usize::MAX, vec![])];
    let mut beam = vec![Node {
        score: score(&start, world, &g),
        flight: start,
        keys: vec![],
        parent: 0,
        inputs: vec![],
        won: false,
    }];
    let mut ticks = 0u32;
    while ticks < max_ticks {
        let children: Vec<Node> = beam
            .par_iter()
            .enumerate()
            .flat_map_iter(|(i, node)| {
                let comm = node.flight.mission.comm_pending
                    && transmission(node.flight.number, node.flight.mission.phase).is_some()
                    && !node.flight.comm_request;
                let mut actions: Vec<Action> = DIRS.iter().map(|&d| Action::Keys(d)).collect();
                if comm {
                    actions.push(Action::Comm);
                }
                actions
                    .into_iter()
                    .filter_map(|a| {
                        expand(node, a, world, &g).map(|mut n| {
                            n.parent = i;
                            n
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        if children.is_empty() {
            if verbose {
                eprintln!("tick {ticks}: every branch failed");
            }
            return None;
        }
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
        // Keep the best distinct children.
        let mut keys = std::collections::BTreeSet::new();
        let mut next = vec![];
        for c in children {
            if keys.insert(diversity_key(&c.flight)) {
                next.push(c);
                if next.len() == BEAM {
                    break;
                }
            }
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
            eprintln!(
                "tick {ticks}: score {} phase {} cell ({},{}) armour {} kills {} targets {}",
                new_beam[0].score,
                b.mission.phase,
                cell.0,
                cell.1,
                b.player.armour,
                b.kills,
                targets.join(" ")
            );
        }
        beam = new_beam;
    }
    None
}

/// Arena index of beam entry `i` (the beam is always the last `len` arena entries).
fn beam_arena_index(arena: &[(usize, Vec<u64>)], len: usize, i: usize) -> usize {
    arena.len() - len + i
}
