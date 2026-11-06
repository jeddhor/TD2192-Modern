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
    /// Player armour + shields at the start.
    start_hp: i32,
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

/// True when the volley path from `o` along `d` stays in open cells for `n` steps.
fn clear_path(world: &World, o: (i32, i32), d: (i32, i32), n: i32) -> bool {
    (1..=n).all(|k| {
        let p = (o.0 + 32 * k * d.0, o.1 + 32 * k * d.1);
        !world.solid((p.0.div_euclid(32)) as i16, (p.1.div_euclid(32)) as i16)
    })
}

/// The fire rule: hold Space when a hostile ship is in the line of fire, closer than any friend.
fn want_fire(f: &Flight, world: &World) -> bool {
    if f.mission_over {
        return false;
    }
    let Some(d) = heading_dir(f.player.heading.code()) else {
        return false;
    };
    let v = f.player.view;
    // Volley origin (weapons §4.2): tile (viewX + 4, viewY + 3), offsets ≈ (subX − 16 + 16,
    // subY − 28 + 11) for the middle muzzles.
    let o = (
        (i32::from(v.tile_x) + 4) * 32 + i32::from(v.sub_x),
        (i32::from(v.tile_y) + 3) * 32 + i32::from(v.sub_y) - 17,
    );
    let mut hostile = i32::MAX;
    let mut friend = i32::MAX;
    for s in f.ships.0.iter().filter(|s| s.alive()) {
        let Some(n) = on_line(o, d, s.world(), 6) else {
            continue;
        };
        if !clear_path(world, o, d, n) {
            continue;
        }
        if s.marker == MARK_FRIEND {
            friend = friend.min(n);
        } else {
            hostile = hostile.min(n);
        }
    }
    hostile < friend
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
    let cell = player_cell(f);
    let dist = if phase >= 100 || targets.is_empty() {
        i64::from(g.home_field.at(cell.0, cell.1).min(400))
    } else {
        let here = Field::new(world, cell);
        targets
            .iter()
            .map(|&i| {
                let sh = &f.ships.0[i];
                let d = here.at(i16::from(sh.tile_x), i16::from(sh.tile_y));
                if d == FAR { 400 } else { i64::from(d) }
            })
            .min()
            .unwrap_or(400)
    };
    s -= dist * 400;
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
            eprintln!(
                "tick {ticks}: score {} phase {} view ({},{}) armour {} kills {} live {}",
                new_beam[0].score,
                b.mission.phase,
                b.player.view.tile_x,
                b.player.view.tile_y,
                b.player.armour,
                b.kills,
                b.ships.0.iter().filter(|s| s.alive()).count()
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
