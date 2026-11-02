//! TD2192-Modern: the game executable (composition root).

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::Parser;
use td_assets::{DataDir, Episode, discover_data_dir, validate};

#[derive(Debug, Parser)]
#[command(
    name = "td2192",
    version,
    about = "Traffic Department 2192, reimplemented"
)]
struct Cli {
    /// Original game directory (overrides TD2192_DATA and settings).
    #[arg(long)]
    data: Option<PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let Some((root, _)) = discover_data_dir(cli.data.as_deref(), None) else {
        bail!("no data directory: pass --data <dir> or set TD2192_DATA");
    };
    let dir = DataDir::open(&root).with_context(|| format!("opening {}", root.display()))?;
    let report = validate(&dir);
    println!(
        "TD2192-Modern {} (SDL {})",
        env!("CARGO_PKG_VERSION"),
        td_platform::sdl_version()
    );
    for ep in Episode::ALL {
        println!(
            "{ep}: {}",
            if report.playable(ep) {
                "playable"
            } else {
                "unavailable"
            }
        );
    }
    Ok(())
}
