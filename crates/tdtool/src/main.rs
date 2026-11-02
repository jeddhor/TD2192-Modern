//! `tdtool`: developer CLI for inspecting original Traffic Department 2192 data.
//!
//! Outputs that contain decoded game content (images, audio, text) must be written **outside**
//! the repository (MDG §5.3). Commands that only print facts (sizes, hashes, structure) may
//! write to stdout.

mod gen_manifest;
mod inventory;

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
    }
}
