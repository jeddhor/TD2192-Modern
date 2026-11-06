//! `tdtool`: developer CLI for inspecting original Traffic Department 2192 data.
//!
//! Outputs that contain decoded game content (images, audio, text) must be written **outside**
//! the repository (MDG §5.3). Commands that only print facts (sizes, hashes, structure) may
//! write to stdout.

mod exe_res;
mod export;
mod flight_setup;
mod gen_manifest;
mod inventory;
mod missions;
mod plan;
mod replay_cmd;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "tdtool", version, about = "TD2192-Modern developer tool")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Validate an original installation against the manifest.
    Inventory {
        /// Original game directory (defaults to $TD2192_DATA).
        #[arg(long)]
        data: Option<PathBuf>,
        /// List every file, not just problems.
        #[arg(long)]
        verbose: bool,
    },
    /// Decode original assets to PNG files in a directory OUTSIDE the repository.
    Export {
        /// What to export.
        #[arg(value_enum)]
        kind: export::Kind,
        /// Output directory (refused if inside a git repository).
        #[arg(long)]
        out: PathBuf,
        /// Original game directory (defaults to $TD2192_DATA).
        #[arg(long)]
        data: Option<PathBuf>,
    },
    /// Verify the executable resource maps (docs/re/exe-resources) against an installation.
    ExeRes {
        /// Original game directory (defaults to $TD2192_DATA).
        #[arg(long)]
        data: Option<PathBuf>,
        /// Also print the extracted values (terminal only; never commit them).
        #[arg(long)]
        show: bool,
    },
    /// Draw every mission's ship paths over a map (PNG files OUTSIDE the repository).
    MissionMap {
        /// Episode 1, 2 or 3.
        #[arg(long)]
        episode: u8,
        /// Map file to draw on (td.map, td2.map, td3.map, td4.map).
        #[arg(long)]
        map: String,
        /// Tileset (td.blk, td2.blk, td3.blk).
        #[arg(long)]
        blk: String,
        /// Output directory (refused if inside a git repository).
        #[arg(long)]
        out: PathBuf,
        /// Original game directory (defaults to $TD2192_DATA).
        #[arg(long)]
        data: Option<PathBuf>,
    },
    /// Print the OPL register writes our CMF player makes (ms, reg, value) for oracle comparison.
    CmfTrace {
        /// Song file name, e.g. intro.cmf.
        song: String,
        /// Seconds of playback to trace.
        #[arg(long, default_value_t = 30.0)]
        seconds: f64,
        /// Original game directory (defaults to $TD2192_DATA).
        #[arg(long)]
        data: Option<PathBuf>,
    },
    /// Find inputs that complete an Episode 1 flight with success in td-sim and write them as a
    /// TDR replay (VE-2, M9-21).
    Plan {
        /// Flight number 1..=20.
        flight: u8,
        /// Directory for `e1f<NN>.tdr` (normally tests/replays).
        #[arg(long, default_value = "tests/replays")]
        out: PathBuf,
        /// Give up after this many ticks.
        #[arg(long, default_value_t = 12000)]
        max_ticks: u32,
        /// Only run the flight without input and print what happens, for this many ticks.
        #[arg(long)]
        probe: Option<u32>,
        /// Print the search's progress.
        #[arg(long)]
        verbose: bool,
        /// Original game directory (defaults to $TD2192_DATA).
        #[arg(long)]
        data: Option<PathBuf>,
    },
    /// Run a TDR replay of an Episode 1 flight: check its state-hash checkpoints (--verify)
    /// and/or write its tick-keyed oracle scenario lines (--oracle-schedule).
    Replay {
        /// The replay file.
        file: PathBuf,
        /// Re-run the flight and check every checkpoint.
        #[arg(long)]
        verify: bool,
        /// Write the oracle scenario lines (ORACLE.md 5a) to this file.
        #[arg(long)]
        oracle_schedule: Option<PathBuf>,
        /// Original game directory (defaults to $TD2192_DATA).
        #[arg(long)]
        data: Option<PathBuf>,
    },
    /// Regenerate crates/td-assets/src/manifest_data.rs from docs/re/FILE_INVENTORY.md.
    GenManifest {
        /// Inventory markdown to read.
        #[arg(long, default_value = "docs/re/FILE_INVENTORY.md")]
        inventory: PathBuf,
        /// Rust file to write.
        #[arg(long, default_value = "crates/td-assets/src/manifest_data.rs")]
        out: PathBuf,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Inventory { data, verbose } => inventory::run(data.as_deref(), verbose),
        Command::GenManifest { inventory, out } => gen_manifest::run(&inventory, &out),
        Command::Plan {
            flight,
            out,
            max_ticks,
            probe,
            verbose,
            data,
        } => {
            let (root, _) = td_assets::discover_data_dir(data.as_deref(), None)
                .ok_or_else(|| anyhow::anyhow!("pass --data <dir> or set TD2192_DATA"))?;
            match probe {
                Some(ticks) => plan::probe(&root, flight, ticks),
                None => plan::run(&root, flight, &out, max_ticks, verbose),
            }
        }
        Command::Replay {
            file,
            verify,
            oracle_schedule,
            data,
        } => {
            let (root, _) = td_assets::discover_data_dir(data.as_deref(), None)
                .ok_or_else(|| anyhow::anyhow!("pass --data <dir> or set TD2192_DATA"))?;
            replay_cmd::run(&root, &file, verify, oracle_schedule.as_deref())
        }
        Command::MissionMap {
            episode,
            map,
            blk,
            out,
            data,
        } => {
            let (root, _) = td_assets::discover_data_dir(data.as_deref(), None)
                .ok_or_else(|| anyhow::anyhow!("pass --data <dir> or set TD2192_DATA"))?;
            missions::run(&root, &out, episode, &map, &blk)
        }
        Command::CmfTrace {
            song,
            seconds,
            data,
        } => {
            let (root, _) = td_assets::discover_data_dir(data.as_deref(), None)
                .ok_or_else(|| anyhow::anyhow!("pass --data <dir> or set TD2192_DATA"))?;
            let dir = td_assets::DataDir::open(&root)?;
            let cmf = std::sync::Arc::new(td_formats::cmf::Cmf::parse(&dir.read(&song)?)?);
            let mut player = td_audio::CmfPlayer::new_traced(cmf, false);
            let rate = player.sample_rate();
            let mut buf = vec![0i32; (seconds * f64::from(rate)) as usize];
            player.render(&mut buf);
            for w in player.take_trace() {
                println!(
                    "{:.3}	{:#04x}	{:#04x}",
                    w.sample as f64 * 1000.0 / f64::from(rate),
                    w.reg,
                    w.value
                );
            }
            Ok(())
        }
        Command::ExeRes { data, show } => {
            let (root, _) = td_assets::discover_data_dir(data.as_deref(), None)
                .ok_or_else(|| anyhow::anyhow!("pass --data <dir> or set TD2192_DATA"))?;
            exe_res::run(&root, show)
        }
        Command::Export { kind, out, data } => {
            let (root, _) = td_assets::discover_data_dir(data.as_deref(), None)
                .ok_or_else(|| anyhow::anyhow!("pass --data <dir> or set TD2192_DATA"))?;
            export::run(&root, &out, kind)
        }
    }
}
