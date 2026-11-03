//! TD2192-Modern: the game executable (composition root).

mod flight;
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
    /// Developer flight arena: fly flight N (1..21) on the Episode 1 map.
    #[arg(long, value_name = "N")]
    dev_flight: Option<u8>,
    /// Developer: play one dialogue scene: ID (episode 1), eN:ID, or eN-safe:ID (edited script).
    #[arg(long, value_name = "SCENE")]
    dev_dialogue: Option<String>,
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

/// First run: ask for the original game folder until a valid one is chosen or the user cancels.
fn first_run_pick() -> Result<PathBuf> {
    td_platform::show_info(
        "TD2192-Modern",
        "Welcome! Please choose the folder where Traffic Department 2192 is installed \
         (the one containing TD1.EXE and FILE0001.EXE).",
    );
    loop {
        let Some(path) = td_platform::pick_folder()? else {
            bail!("No game folder was chosen. Start again, or run with --data <folder>.");
        };
        let ok = DataDir::open(&path)
            .map(|d| validate(&d).playable(Episode::E1))
            .unwrap_or(false);
        if ok {
            return Ok(path);
        }
        td_platform::show_info(
            "TD2192-Modern",
            &format!(
                "{} does not contain a complete Traffic Department 2192 installation. \
                 Please choose another folder.",
                path.display()
            ),
        );
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

    let root = match discover_data_dir(cli.data.as_deref(), settings.data_dir.as_deref()) {
        Some((root, _)) => root,
        None => {
            let root = first_run_pick()?;
            settings.data_dir = Some(root.clone());
            settings.save(&settings_path)?;
            root
        }
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

    let resources = td_assets::ExeResources::load(&dir)?;
    let first: Box<dyn td_flow::Screen> = match (cli.dev_flight, cli.dev_dialogue.as_deref()) {
        (Some(n), _) => Box::new(flight::FlightArena::new(&dir, &resources, n)?),
        (None, Some(spec)) => {
            let (episode, choice, id) = td_flow::dialogue::parse_scene_spec(spec)?;
            Box::new(td_flow::DialogueScene::load(
                &dir, &resources, episode, choice, id,
            )?)
        }
        (None, None) => match cli.dev_mapview.as_deref() {
            Some(map) => Box::new(mapview::MapView::new(&dir, map)?),
            // The Episode 1 program: script choice, menu, campaign, flights.
            None => Box::new(td_flow::Episode1::new(
                &dir,
                &resources,
                Box::new(flight::ArenaLauncher),
            )?),
        },
    };
    let options = run::RunOptions {
        exit_after_ticks: cli.dev_exit_after,
    };
    let rd = run::RunData {
        data: &dir,
        resources: &resources,
        settings: &settings,
        user_dir: &user_dir,
    };
    run::run(td_flow::Flow::new(first), &rd, &options)
}
