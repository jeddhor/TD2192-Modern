//! TD2192-Modern: the game executable (composition root).

mod mapview;
mod run;

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::Parser;
use td_assets::{DataDir, Episode, discover_data_dir, validate};
use td_flow::Settings;

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
    /// Developer map viewer for a map: e1, e2a, e2b, e3a, e3b or intro.
    #[arg(long, value_name = "MAP")]
    dev_mapview: Option<String>,
    /// Start in fullscreen (overrides settings).
    #[arg(long)]
    fullscreen: bool,
    /// Print the installation check and exit.
    #[arg(long)]
    check: bool,
    /// Developer: save a screenshot and exit after this many simulation ticks.
    #[arg(long, value_name = "TICKS")]
    dev_exit_after: Option<u64>,
}

fn main() {
    if let Err(e) = real_main() {
        let msg = format!("{e:#}");
        td_platform::show_error("TD2192-Modern", &msg);
        std::process::exit(1);
    }
}

fn real_main() -> Result<()> {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("TD2192_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();
    let user_dir = td_platform::user_data_dir().context("locating the user data directory")?;
    let settings_path = user_dir.join("settings.toml");
    let mut settings = Settings::load(&settings_path)?;
    if !settings_path.exists() {
        settings.save(&settings_path)?;
    }

    let Some((root, _)) = discover_data_dir(cli.data.as_deref(), settings.data_dir.as_deref())
    else {
        bail!(
            "No Traffic Department 2192 data directory configured.\n\nStart the game with \
             --data <folder>, set the TD2192_DATA environment variable, or set data_dir in\n{}",
            settings_path.display()
        );
    };
    let dir = DataDir::open(&root).with_context(|| format!("opening {}", root.display()))?;
    let report = validate(&dir);
    if cli.check {
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
        return Ok(());
    }
    if !report.playable(Episode::E1) {
        bail!(
            "{} does not contain a complete Traffic Department 2192 installation \
             (run `tdtool inventory` for details).",
            root.display()
        );
    }
    if cli.fullscreen {
        settings.video.fullscreen = true;
    }

    let map = cli.dev_mapview.as_deref().unwrap_or("e1");
    let app = mapview::MapView::new(&dir, map)?;
    let options = run::RunOptions {
        exit_after_ticks: cli.dev_exit_after,
    };
    run::run(app, &settings, &user_dir, &options)
}
