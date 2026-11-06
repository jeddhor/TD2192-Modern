//! `tdtool plan`: finds inputs that complete an Episode 1 flight with success in td-sim, and
//! writes them as a TDR replay with state-hash checkpoints plus the matching tick-keyed oracle
//! scenario (VE-2, M9-21; `docs/parity/M9-playthrough.md`).
//!
//! This is a Verifier tool. It only drives the port's simulation (`td_sim::Flight` is `Clone`
//! and deterministic) and never looks at the original program; the plan it finds is then flown
//! in the original by the oracle and compared tick by tick.

mod nav;
mod schedule;
mod search;

use std::path::Path;

use anyhow::{Result, bail};
use td_sim::flight::TickResult;
use td_sim::flight::ships::SHIP_SLOTS;
use td_sim::{Flight, TickControls};

use crate::flight_setup::E1Flight;
pub use schedule::{
    Plan, check as check_inputs, comm_ticks, oracle_lines, replay as replay_of, write_outputs,
};
pub use search::BEAM;

/// The state hash without shots on two-digit headings: those differ between muzzle tables
/// (U-160) but live only until their first move; what they hit shows in the rest of the state.
pub fn core_hash(f: &Flight) -> u64 {
    let mut c = f.clone();
    // Freed slots keep their old fields, so normalise every slot last used on such a heading.
    for s in c.shots.0.iter_mut().filter(|s| s.heading > 9) {
        s.tile_x = 0xFF;
        s.off_x = 0;
        s.off_y = 0;
    }
    td_sim::state_hash(&c)
}

/// Ticks between replay checkpoints.
pub const CHECKPOINT_INTERVAL: u32 = 100;

/// Runs flight `number` with no input for `ticks` ticks and prints what happens (phase changes,
/// spawns and removals, kills, damage, the end), to study a flight before planning it.
pub fn probe(root: &Path, number: u8, ticks: u32) -> Result<()> {
    let setup = E1Flight::load(root, number)?;
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
pub fn run(
    root: &Path,
    number: u8,
    out: &Path,
    max_ticks: u32,
    verbose: bool,
    alt_muzzles: Option<&Path>,
    beam: usize,
) -> Result<()> {
    let setup = E1Flight::load(root, number)?;
    let alt =
        match alt_muzzles {
            Some(p) => Some(setup.with_muzzles(
                std::fs::read(p).map_err(|e| anyhow::anyhow!("{}: {e}", p.display()))?,
            )),
            None => None,
        };
    // Flights are large values; give the search a big stack (the main thread's is 1 MB).
    let found = std::thread::scope(|s| {
        std::thread::Builder::new()
            .stack_size(256 << 20)
            .spawn_scoped(s, || {
                search::plan(&setup, alt.as_ref(), max_ticks, verbose, beam)
            })
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
    write_outputs(&setup, &plan, out)?;
    println!(
        "flight {number}: {} ticks, kills {}, outcome {}, comm viewers at {:?}",
        n,
        f.kills,
        f.outcome(),
        plan.comm_ticks
    );
    Ok(())
}
