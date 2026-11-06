//! `tdtool plan`: finds inputs that complete a flight of any episode with success in td-sim,
//! and writes them as a TDR replay with state-hash checkpoints plus the matching tick-keyed
//! oracle scenario (VE-2, M9-21, `docs/parity/M9-playthrough.md`; E2 and E3: VE-4, M10-45,
//! `docs/parity/M10-playthrough.md`).
//!
//! This is a Verifier tool. It only drives the port's simulation (`td_sim::Flight` is `Clone`
//! and deterministic) and never looks at the original program; the plan it finds is then flown
//! in the original by the oracle and compared tick by tick.

mod nav;
mod schedule;
mod search;

use std::path::Path;

use anyhow::{Result, bail};
use td_sim::flight::ships::SHIP_SLOTS;
use td_sim::flight::{Episode, TickResult, logic};
use td_sim::{Flight, TickControls};

use crate::flight_setup::FlightSetup;
pub use schedule::{
    Plan, check as check_inputs, comm_ticks, modal_ticks, oracle_lines, replay as replay_of,
    write_outputs,
};
pub use search::BEAM;

/// The state hash without shots on two-digit headings: those differ between muzzle tables
/// (U-160) but live only until their first move; what they hit shows in the rest of the state.
pub fn core_hash(f: &Flight) -> u64 {
    td_sim::state_hash(&normalised(f))
}

/// The flight with every shot slot last used on a two-digit heading cleared (freed slots keep
/// their old fields, so every such slot is normalised).
pub fn normalised(f: &Flight) -> Flight {
    let mut c = f.clone();
    for s in c.shots.0.iter_mut().filter(|s| s.heading > 9) {
        s.tile_x = 0xFF;
        s.off_x = 0;
        s.off_y = 0;
    }
    c
}

/// The lines of the pretty debug output of two normalised flights that differ (at most `n`),
/// with the path of enclosing fields, to see what a difference in [`core_hash`] is.
pub fn debug_diff(a: &Flight, b: &Flight, n: usize) -> Vec<String> {
    let (x, y) = (
        format!("{:#?}", normalised(a)),
        format!("{:#?}", normalised(b)),
    );
    let mut out = vec![];
    let mut path: Vec<&str> = vec![];
    for (la, lb) in x.lines().zip(y.lines()) {
        let depth = la.len() - la.trim_start().len();
        path.truncate(depth / 4);
        if la != lb {
            out.push(format!(
                "{} | {} | {}",
                path.join("."),
                la.trim(),
                lb.trim()
            ));
            if out.len() == n {
                break;
            }
        }
        if la.trim_end().ends_with('{') || la.trim_end().ends_with('[') {
            path.push(
                la.trim()
                    .trim_end_matches(['{', '[', ' '])
                    .trim_end_matches(':'),
            );
        }
    }
    out
}

/// True when the comm viewer has a transmission for the flight's current phase (the episode's
/// table: mission-logic §4.6, e2-mission-logic and e3-mission-logic §4.6).
pub fn transmission_waits(f: &Flight) -> bool {
    let phase = f.mission.phase;
    match f.episode {
        Episode::E1 => logic::transmission(f.number, phase).is_some(),
        Episode::E2 => logic::e2::transmission(f.number, phase).is_some(),
        Episode::E3 => logic::e3::transmission(f.number, phase).is_some(),
    }
}

/// Ticks between replay checkpoints.
pub const CHECKPOINT_INTERVAL: u32 = 100;

/// Runs flight `number` with no input for `ticks` ticks and prints what happens (phase changes,
/// spawns and removals, kills, damage, the end), to study a flight before planning it. A flight
/// that follows another in its block starts after the predecessor's replay in `dir`.
pub fn probe(root: &Path, episode: Episode, number: u8, ticks: u32, dir: &Path) -> Result<()> {
    let setup = FlightSetup::load_chain(root, episode, number, Some(dir))?;
    let mut f = setup.start();
    println!(
        "flight {number}: class {} view {:?} phase {} palette {} late {}",
        f.player.class, f.player.view, f.mission.phase, f.palette_mode, f.late_draw
    );
    for (i, s) in f.ships.0.iter().enumerate() {
        if s.alive() || s.marker != 0 {
            println!(
                "  slot {:2}: class {:3} tile ({:3},{:3}) marker {:02X} armour {} shields {}",
                i + 1,
                s.class,
                s.tile_x,
                s.tile_y,
                s.marker,
                s.armour,
                s.shields
            );
        }
    }
    let mut prev = f.clone();
    for t in 1..=ticks {
        let mut ev = vec![];
        let mut r = f.tick(&setup.world, &TickControls::default(), &mut ev);
        while let TickResult::Modal(m) = r {
            println!("t{t}: modal {m:?}");
            r = f.resume(&setup.world, &TickControls::default(), &mut ev);
        }
        for line in describe(t, &prev, &f) {
            println!("{line}");
        }
        if r == TickResult::Finished {
            println!(
                "t{t}: finished: succeeded {} lost {} died {} outcome {}",
                f.succeeded,
                f.lost,
                f.died(),
                f.outcome()
            );
            break;
        }
        prev = f.clone();
    }
    Ok(())
}

/// Writes `<out>/e<E>f<NN>-idle.tdr`: no input at all until the flight ends (in the flights
/// where the enemies destroy the parked ship: the death and the end sequence, which no success
/// plan covers), at most `max_ticks` ticks. With `modals`, `e<E>f<NN>-modals.tdr`: the same with
/// one tap each of P (pause) in tick 100, F5 (tactical map) in tick 200 and Esc (flight menu)
/// in tick 300, which open their screens with no other key held (the oracle schedule closes
/// them, [`schedule::oracle_lines`]). `suffix` replaces the file name's `-idle`/`-modals` (E1
/// flight 21 flies itself and ends after 25 ticks: `e1f21.tdr` is an idle replay).
pub fn idle(
    root: &Path,
    episode: Episode,
    number: u8,
    out: &Path,
    max_ticks: u32,
    modals: bool,
    suffix: Option<&str>,
) -> Result<()> {
    let setup = FlightSetup::load_chain(root, episode, number, Some(out))?;
    let mut f = setup.start();
    let mut inputs = vec![];
    loop {
        if inputs.len() as u32 == max_ticks {
            bail!("flight {number}: still flying after {max_ticks} idle ticks");
        }
        let t = inputs.len() + 1;
        let key = match t {
            100 => Some(td_sim::FlightKey::P),
            200 => Some(td_sim::FlightKey::F5),
            300 => Some(td_sim::FlightKey::Esc),
            _ => None,
        };
        let w = match key {
            Some(k) if modals => schedule::word(&[], &[k]),
            _ => 0,
        };
        inputs.push(w);
        if td_sim::replay::step(&mut f, &setup.world, w) == TickResult::Finished {
            break;
        }
    }
    let plan = Plan {
        comm_ticks: comm_ticks(&setup, &inputs),
        inputs,
    };
    let suffix = suffix.unwrap_or(if modals { "-modals" } else { "-idle" });
    write_outputs(&setup, &plan, out, suffix)?;
    println!(
        "flight {number}: {} idle ticks; succeeded {} lost {} died {} outcome {}",
        plan.inputs.len(),
        f.succeeded,
        f.lost,
        f.died(),
        f.outcome()
    );
    Ok(())
}

/// Writes the replay `out` of `ticks` ticks of flight `number` keyed by the `tick:T down|up
/// <key>` and `tick:T press <key> [hold]` lines of an oracle scenario (ORACLE.md §5a; other
/// lines are ignored, a key line with a frame offset is refused). By the tick rule (see
/// [`schedule`]) port tick T sees the keys down after the transitions of `tick:T`, and a key is
/// made in T when it goes down there. The flight starts as its replays do
/// ([`FlightSetup::load_chain`], predecessors from `out`'s directory).
pub fn from_schedule(
    root: &Path,
    episode: Episode,
    number: u8,
    scenario: &Path,
    out: &Path,
    ticks: u32,
) -> Result<()> {
    use std::collections::BTreeMap;
    use td_sim::FlightKey;
    let text = std::fs::read_to_string(scenario)
        .map_err(|e| anyhow::anyhow!("{}: {e}", scenario.display()))?;
    let key_of = |name: &str| {
        FlightKey::ALL
            .iter()
            .copied()
            .find(|&k| schedule::key_name(k) == name)
    };
    // tick -> transitions (key, down) in file order
    let mut at: BTreeMap<u32, Vec<(FlightKey, bool)>> = BTreeMap::new();
    for line in text.lines().map(str::trim) {
        let mut tok = line.split_whitespace();
        let (Some(when), Some(act), Some(key)) = (tok.next(), tok.next(), tok.next()) else {
            continue;
        };
        let Some(t) = when.strip_prefix("tick:") else {
            continue;
        };
        if !matches!(act, "down" | "up" | "press") {
            continue;
        }
        if t.contains(['+', '-']) {
            bail!("{line}: key lines with frame offsets or ranges cannot be replayed");
        }
        let t: u32 = t.parse()?;
        let Some(k) = key_of(key) else {
            bail!("{line}: not a flight key");
        };
        match act {
            "press" => {
                let hold: u32 = tok.next().map(str::parse).transpose()?.unwrap_or(1).max(1);
                at.entry(t).or_default().push((k, true));
                at.entry(t + hold).or_default().push((k, false));
            }
            _ => at.entry(t).or_default().push((k, act == "down")),
        }
    }
    let setup = FlightSetup::load_chain(root, episode, number, out.parent())?;
    let mut down: Vec<FlightKey> = vec![];
    let mut inputs = vec![];
    for t in 1..=ticks {
        let prev = down.clone();
        let mut made = vec![];
        for &(k, d) in at.get(&t).map(Vec::as_slice).unwrap_or(&[]) {
            down.retain(|&x| x != k);
            if d {
                down.push(k);
                made.push(k);
            }
        }
        let mut c = TickControls::default();
        for &k in &down {
            c.set_down(k);
            if made.contains(&k) || !prev.contains(&k) {
                c.set_made(k);
            }
        }
        inputs.push(c.to_word());
    }
    check_inputs(&inputs)?;
    let plan = Plan {
        comm_ticks: comm_ticks(&setup, &inputs),
        inputs,
    };
    let r = replay_of(&setup, &plan);
    std::fs::write(out, r.to_bytes()).map_err(|e| anyhow::anyhow!("{}: {e}", out.display()))?;
    println!(
        "wrote {} ({} ticks, comm viewers at {:?})",
        out.display(),
        ticks,
        plan.comm_ticks
    );
    Ok(())
}

/// What changed from `a` to `b` in tick `t`.
pub fn describe(t: u32, a: &Flight, b: &Flight) -> Vec<String> {
    let mut out = vec![];
    if a.mission.phase != b.mission.phase {
        out.push(format!(
            "t{t}: phase {} -> {}",
            a.mission.phase, b.mission.phase
        ));
    }
    if a.mission.comm_pending != b.mission.comm_pending {
        out.push(format!("t{t}: comm pending {}", b.mission.comm_pending));
    }
    if a.kills != b.kills {
        out.push(format!("t{t}: kills {}", b.kills));
    }
    if a.player.armour != b.player.armour {
        out.push(format!(
            "t{t}: player armour {} shields {}",
            b.player.armour, b.player.shields
        ));
    }
    for i in 0..SHIP_SLOTS {
        let (x, y) = (&a.ships.0[i], &b.ships.0[i]);
        if x.class != y.class {
            out.push(format!(
                "t{t}: slot {} class {:3} -> {:3} at ({},{}) marker {:02X}",
                i + 1,
                x.class,
                y.class,
                y.tile_x,
                y.tile_y,
                y.marker
            ));
        }
    }
    if a.mission_over != b.mission_over {
        out.push(format!("t{t}: mission over"));
    }
    if a.succeeded != b.succeeded {
        out.push(format!("t{t}: succeeded {}", b.succeeded));
    }
    if a.lost != b.lost {
        out.push(format!("t{t}: lost {}", b.lost));
    }
    out
}

/// Plans flight `number` and writes the replay and the oracle scenario lines.
#[allow(clippy::too_many_arguments)]
pub fn run(
    root: &Path,
    episode: Episode,
    number: u8,
    out: &Path,
    max_ticks: u32,
    verbose: bool,
    alt_muzzles: Option<&Path>,
    beam: usize,
    dual: bool,
    weapons: bool,
    suffix: &str,
) -> Result<()> {
    let setup = FlightSetup::load_chain(root, episode, number, Some(out))?;
    let alt =
        match alt_muzzles {
            Some(p) => Some(setup.with_muzzles(
                std::fs::read(p).map_err(|e| anyhow::anyhow!("{}: {e}", p.display()))?,
            )),
            None => None,
        };
    let opts = search::Options {
        max_ticks,
        verbose,
        beam,
        dual,
        weapons,
    };
    // Flights are large values; give the search a big stack (the main thread's is 1 MB).
    let found = std::thread::scope(|s| {
        std::thread::Builder::new()
            .stack_size(256 << 20)
            .spawn_scoped(s, || search::plan(&setup, alt.as_ref(), &opts))
            .map(|h| h.join())
    });
    let Ok(Ok(found)) = found else {
        bail!("flight {number}: the search thread failed");
    };
    let Some(plan) = found else {
        bail!("flight {number}: no winning plan found within {max_ticks} ticks");
    };
    // Re-run the plan from scratch: it must end in success on its last tick.
    let mut f = setup.start();
    let n = plan.inputs.len();
    for (i, &w) in plan.inputs.iter().enumerate() {
        let r = td_sim::replay::step(&mut f, &setup.world, w);
        if (r == TickResult::Finished) != (i + 1 == n) {
            bail!("flight {number}: re-run ended at tick {} of {n}", i + 1);
        }
    }
    if !f.succeeded || f.died() || f.lost {
        bail!("flight {number}: re-run did not succeed");
    }
    // With the capture's muzzle bytes the flight must end the same way on the same tick.
    let mut same = true;
    if let Some(a) = &alt {
        let mut g = a.start();
        for (i, &w) in plan.inputs.iter().enumerate() {
            let r = td_sim::replay::step(&mut g, &a.world, w);
            if (r == TickResult::Finished) != (i + 1 == n) {
                bail!("flight {number}: alt re-run ended at tick {} of {n}", i + 1);
            }
        }
        if !g.succeeded || g.died() || g.lost {
            bail!("flight {number}: alt re-run did not succeed");
        }
        same = core_hash(&g) == core_hash(&f);
    }
    write_outputs(&setup, &plan, out, suffix)?;
    let start = setup.start();
    println!(
        "flight {number}: {} ticks, kills {}, outcome {}, comm viewers at {:?}, same course with the alt bytes {same}; missiles fired {}, afterburner bursts {}, Tab presses {}",
        n,
        f.kills,
        f.outcome(),
        plan.comm_ticks,
        start.missiles.saturating_sub(f.missiles),
        schedule::presses(&plan.inputs, td_sim::FlightKey::KpPlus),
        schedule::presses(&plan.inputs, td_sim::FlightKey::Tab),
    );
    Ok(())
}
