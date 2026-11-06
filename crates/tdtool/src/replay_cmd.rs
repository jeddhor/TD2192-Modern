//! `tdtool replay`: runs a TDR replay of an Episode 1 flight (ADG §10.5, M6-02).
//!
//! `--verify` re-runs the inputs from a direct start of the flight (`Session::new()`, the
//! oracle method of `docs/parity/M8-missions.md`) and checks every state-hash checkpoint; it
//! also reports how the flight ended. `--oracle-schedule` writes the tick-keyed oracle scenario
//! lines for the same inputs (ORACLE.md §5a; the tick rule is in [`crate::plan`]'s schedule).

use std::path::Path;

use anyhow::{Context, Result, bail};
use td_sim::flight::TickResult;
use td_sim::replay::Replay;
use td_sim::state_hash;

use crate::flight_setup::{FlightSetup, episode_of};
use crate::plan;

/// The flight a replay is of.
pub fn setup_of(root: &Path, r: &Replay) -> Result<FlightSetup> {
    FlightSetup::load(root, episode_of(r.episode)?, r.mission)
}

/// How a verified replay ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verified {
    pub ticks: usize,
    pub checkpoints: usize,
    pub finished_at: Option<usize>,
    pub succeeded: bool,
    pub lost: bool,
    pub died: bool,
    pub kills: i16,
    pub outcome: bool,
    /// The view at the end: tile X, tile Y, sub X, sub Y.
    pub view: (i16, i16, i16, i16),
}

/// Re-runs `r` and checks its checkpoints.
pub fn verify(root: &Path, r: &Replay) -> Result<Verified> {
    let setup = setup_of(root, r)?;
    let start = setup.session().rng.seed();
    if r.seed != start {
        bail!(
            "replay seed {:08X} is not a direct start (program-start seed {start:08X})",
            r.seed
        );
    }
    let mut f = setup.start();
    let mut finished_at = None;
    let mut checked = 0;
    for (i, &w) in r.inputs.iter().enumerate() {
        if finished_at.is_some() {
            bail!("the flight ended at tick {} but the replay goes on", i);
        }
        if td_sim::replay::step(&mut f, &setup.world, w) == TickResult::Finished {
            finished_at = Some(i + 1);
        }
        let t = i as u32 + 1;
        if r.interval > 0 && t.is_multiple_of(r.interval) {
            let Some(&want) = r.checkpoints.get(checked) else {
                bail!("missing checkpoint for tick {t}");
            };
            let got = state_hash(&f);
            if got != want {
                bail!("checkpoint at tick {t}: state hash {got:016X}, replay has {want:016X}");
            }
            checked += 1;
        }
    }
    if checked != r.checkpoints.len() {
        bail!(
            "{} checkpoints in the file, {checked} reached",
            r.checkpoints.len()
        );
    }
    Ok(Verified {
        ticks: r.inputs.len(),
        checkpoints: checked,
        finished_at,
        succeeded: f.succeeded,
        lost: f.lost,
        died: f.died(),
        kills: f.kills,
        outcome: f.outcome(),
        view: (
            f.player.view.tile_x,
            f.player.view.tile_y,
            f.player.view.sub_x,
            f.player.view.sub_y,
        ),
    })
}

/// Runs the inputs in the installation's world (a direct start's muzzle bytes, weapons rev 5
/// §3.3, §3.3.2, §3.3.3) and in `alt` (other muzzle bytes, U-160) and prints the first tick
/// where the two flights differ, with what changed in each.
pub fn compare_muzzles(root: &Path, r: &Replay, alt: Vec<u8>) -> Result<()> {
    let base = setup_of(root, r)?;
    let other = base.with_muzzles(alt);
    let (mut a, mut b) = (base.start(), other.start());
    for (i, &w) in r.inputs.iter().enumerate() {
        let (pa, pb) = (a.clone(), b.clone());
        td_sim::replay::step(&mut a, &base.world, w);
        td_sim::replay::step(&mut b, &other.world, w);
        if plan::core_hash(&a) != plan::core_hash(&b) {
            let t = i as u32 + 1;
            println!("first difference after tick {t}");
            println!("  direct start:   {:?}", plan::describe(t, &pa, &a));
            println!("  other bytes:    {:?}", plan::describe(t, &pb, &b));
            for (k, (x, y)) in a.shots.0.iter().zip(&b.shots.0).enumerate() {
                if x != y {
                    println!("  shot {k}: direct {x:?}\n          other  {y:?}");
                }
            }
            for (k, (x, y)) in pa.shots.0.iter().zip(&pb.shots.0).enumerate() {
                if x.alive() && x.heading > 9 && x == y {
                    println!("  before: shot {k} on a two-digit heading {x:?}");
                }
            }
            for line in plan::debug_diff(&a, &b, 30) {
                println!("  state: {line}");
            }
            return Ok(());
        }
    }
    println!("no difference in {} ticks", r.inputs.len());
    Ok(())
}

/// The command.
pub fn run(
    root: &Path,
    file: &Path,
    verify_it: bool,
    schedule: Option<&Path>,
    alt_muzzles: Option<&Path>,
    rewrite: Option<&Path>,
) -> Result<()> {
    let bytes = std::fs::read(file).with_context(|| format!("read {}", file.display()))?;
    let r = Replay::from_bytes(&bytes).map_err(|e| anyhow::anyhow!("{}: {e}", file.display()))?;
    println!(
        "{}: episode {} flight {}, {} ticks, checkpoint every {} ticks ({} checkpoints)",
        file.display(),
        r.episode,
        r.mission,
        r.inputs.len(),
        r.interval,
        r.checkpoints.len()
    );
    if verify_it {
        let v = verify(root, &r)?;
        println!(
            "verified: {} checkpoints match; finished at tick {:?}; succeeded {} lost {} died {} kills {} outcome {} view {:?}",
            v.checkpoints, v.finished_at, v.succeeded, v.lost, v.died, v.kills, v.outcome, v.view
        );
    }
    if let Some(out) = rewrite {
        // Same inputs, checkpoints recomputed with the current td-sim (ADG §10.5: only after a
        // Verifier confirmed the new behaviour against the original).
        let setup = setup_of(root, &r)?;
        let plan = plan::Plan {
            inputs: r.inputs.clone(),
            comm_ticks: vec![],
        };
        let new = plan::replay_of(&setup, &plan);
        std::fs::write(out, new.to_bytes()).with_context(|| format!("write {}", out.display()))?;
        println!("wrote {} with recomputed checkpoints", out.display());
    }
    if let Some(alt) = alt_muzzles {
        let bytes = std::fs::read(alt).with_context(|| format!("read {}", alt.display()))?;
        compare_muzzles(root, &r, bytes)?;
    }
    if let Some(out) = schedule {
        let setup = setup_of(root, &r)?;
        plan::check_inputs(&r.inputs)?;
        let modals = plan::modal_ticks(&setup, &r.inputs);
        let comm = plan::comm_ticks(&setup, &r.inputs);
        let lines = plan::oracle_lines(&r.inputs, &modals);
        std::fs::write(out, lines.join("\n") + "\n")
            .with_context(|| format!("write {}", out.display()))?;
        println!(
            "wrote {} ({} lines; comm viewers at ticks {comm:?})",
            out.display(),
            lines.len()
        );
    }
    Ok(())
}
