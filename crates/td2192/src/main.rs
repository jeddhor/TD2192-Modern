//! TD2192-Modern: the game executable (composition root).

mod enhanced;
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
    /// Start this episode's program directly: 1 (`file0001.exe`), 2 (`td2.exe`) or 3
    /// (`td3.exe`), as the original starts them. Without it the start screen offers the
    /// installed episodes, and an episode's quit item returns there.
    #[arg(long, value_name = "N")]
    episode: Option<u8>,
    /// Developer map viewer for a map: e1, e2a, e2b, e3a, e3b or intro.
    #[arg(long, value_name = "MAP")]
    dev_mapview: Option<String>,
    /// Developer flight arena: fly flight N of Episode 1 (1..21), or eN:N (e2:9 = Episode 2
    /// flight 9).
    #[arg(long, value_name = "FLIGHT")]
    dev_flight: Option<String>,
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
    /// Developer: Enhanced presentation for this run, as `key=value` pairs of the `[enhanced]`
    /// settings (e.g. `widescreen=16:9,upscaler=xbr3`); implies `profile = "enhanced"`.
    #[arg(long, value_name = "SPEC")]
    dev_enhanced: Option<String>,
}

/// Parses `--dev-flight`: `N` (Episode 1) or `eN:N`.
fn parse_flight_spec(spec: &str) -> Result<(Episode, u8)> {
    let (episode, n) = match spec.split_once(':') {
        None => (Episode::E1, spec),
        Some(("e1" | "E1", n)) => (Episode::E1, n),
        Some(("e2" | "E2", n)) => (Episode::E2, n),
        Some(("e3" | "E3", n)) => (Episode::E3, n),
        Some(_) => bail!("--dev-flight takes N or eN:N (for example e2:9), not {spec:?}"),
    };
    let n = n
        .parse()
        .with_context(|| format!("--dev-flight: bad flight number in {spec:?}"))?;
    Ok((episode, n))
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
    let episode = match cli.episode {
        None => None,
        Some(1) => Some(Episode::E1),
        Some(2) => Some(Episode::E2),
        Some(3) => Some(Episode::E3),
        Some(n) => bail!("--episode takes 1, 2 or 3, not {n}"),
    };
    if let Some(ep) = episode.filter(|&ep| ep != Episode::E1 && !report.playable(ep)) {
        bail!(
            "{} does not contain a complete {ep} installation \
             (run `tdtool inventory` for details).",
            root.display(),
        );
    }
    if !report.playable(Episode::E1) {
        bail!(
            "{} does not contain a complete Traffic Department 2192 installation \
             (run `tdtool inventory` for details).",
            root.display()
        );
    }
    // The file's settings, before the command-line overrides of this session.
    let saved = settings.clone();
    if cli.fullscreen {
        settings.video.fullscreen = true;
    }
    if let Some(spec) = cli.dev_enhanced.as_deref() {
        settings.profile = td_flow::settings::Profile::Enhanced;
        settings.enhanced = enhanced::parse_spec(spec).context("--dev-enhanced")?;
    }
    let view = enhanced::SharedView::default();
    let port = td_flow::front::SettingsPort::new();

    let resources = td_assets::ExeResources::load(&dir)?;
    let first: Box<dyn td_flow::Screen> = match (cli.dev_flight, cli.dev_dialogue.as_deref()) {
        (Some(spec), _) => {
            let (episode, n) = parse_flight_spec(&spec)?;
            Box::new(
                flight::FlightArena::new(&dir, &resources, episode, n)?
                    .with_enhanced(Some(view.clone())),
            )
        }
        (None, Some(spec)) => {
            let (episode, choice, id) = td_flow::dialogue::parse_scene_spec(spec)?;
            Box::new(td_flow::DialogueScene::load(
                &dir, &resources, episode, choice, id,
            )?)
        }
        (None, None) => match (cli.dev_mapview.as_deref(), episode) {
            (Some(map), _) => Box::new(mapview::MapView::new(&dir, map)?),
            (None, Some(ep)) => episode_program(&dir, &resources, ep, &view, &user_dir)?,
            // The port's start screen (M11-01).
            (None, None) => Box::new(td_flow::front::StartScreen::new(
                td_flow::front::UiAssets::load(&dir),
                [Episode::E1, Episode::E2, Episode::E3].map(|ep| report.playable(ep)),
                Box::new(Programs {
                    view: view.clone(),
                    user_dir: user_dir.clone(),
                }),
                port.clone(),
            )),
        },
    };
    let options = run::RunOptions {
        exit_after_ticks: cli.dev_exit_after,
    };
    let rd = run::RunData {
        data: &dir,
        resources: &resources,
        settings: &settings,
        saved,
        user_dir: &user_dir,
        enhanced: view,
        port,
    };
    run::run(td_flow::Flow::new(first), &rd, &options)
}

/// An episode's program from its start. Saved games live in the user data directory, never in
/// the game folder.
fn episode_program(
    dir: &DataDir,
    resources: &td_assets::ExeResources,
    episode: Episode,
    view: &enhanced::SharedView,
    user_dir: &std::path::Path,
) -> Result<Box<dyn td_flow::Screen>> {
    let saves = td_flow::save_screens::save_dir(user_dir);
    let launcher = flight::ArenaLauncher::for_episode(episode).with_enhanced(view.clone());
    Ok(match episode {
        // Script choice, menu, campaign, flights (`file0001.exe`).
        Episode::E1 => Box::new(
            td_flow::Episode1::new(dir, resources, Box::new(launcher))?.with_save_dir(saves),
        ),
        // `td2.exe`, started directly in the original (U-005).
        Episode::E2 => Box::new(
            td_flow::Episode2::new(dir, resources, Box::new(launcher))?.with_save_dir(saves),
        ),
        // `td3.exe`, started directly (e3-program-flow §8).
        Episode::E3 => Box::new(
            td_flow::Episode3::new(dir, resources, Box::new(launcher))?.with_save_dir(saves),
        ),
    })
}

/// Starts the episode programs for the start screen: each launch is a new program run (its
/// own flight session and RNG stream, as when the original starts that executable).
#[derive(Debug)]
struct Programs {
    view: enhanced::SharedView,
    user_dir: PathBuf,
}

impl td_flow::front::EpisodeLauncher for Programs {
    fn launch(
        &mut self,
        ctx: &mut td_flow::FlowContext<'_>,
        episode: Episode,
    ) -> std::result::Result<Box<dyn td_flow::Screen>, String> {
        episode_program(ctx.data, ctx.resources, episode, &self.view, &self.user_dir)
            .map_err(|e| format!("{e:#}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flight_specs() {
        assert_eq!(parse_flight_spec("7").unwrap(), (Episode::E1, 7));
        assert_eq!(parse_flight_spec("e2:9").unwrap(), (Episode::E2, 9));
        assert!(parse_flight_spec("e4:1").is_err());
        assert!(parse_flight_spec("e2:x").is_err());
    }
}
