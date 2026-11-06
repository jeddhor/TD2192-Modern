//! TD2192-Modern: the game executable (composition root).

mod flight;
mod mapview;
mod run;
mod shell;

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
    /// Which episode's program to run: 1 (the default, `file0001.exe`), 2 (`td2.exe`) or 3
    /// (`td3.exe`; the original starts both directly).
    #[arg(long, value_name = "N", default_value_t = 1)]
    episode: u8,
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
    /// Start Episode 1 without the intro.
    #[arg(long)]
    no_intro: bool,
    /// Developer: play the intro alone.
    #[arg(long)]
    dev_intro: bool,
    /// Developer: show the credits, as the intro flyover (a, the default) or as pages (b).
    #[arg(long, value_name = "MODE", num_args = 0..=1, default_missing_value = "a")]
    dev_credits: Option<String>,
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
        1 => Episode::E1,
        2 => Episode::E2,
        3 => Episode::E3,
        n => bail!("--episode takes 1, 2 or 3, not {n}"),
    };
    if episode != Episode::E1 && !report.playable(episode) {
        bail!(
            "{} does not contain a complete Episode {} installation \
             (run `tdtool inventory` for details).",
            root.display(),
            cli.episode
        );
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
        _ if cli.dev_intro => shell::dev_intro(&dir, &resources, &settings)?,
        _ if cli.dev_credits.is_some() => {
            let mode = cli.dev_credits.as_deref().unwrap_or("a");
            shell::dev_credits(&dir, &resources, &settings, mode)?
        }
        (Some(spec), _) => {
            let (episode, n) = parse_flight_spec(&spec)?;
            Box::new(flight::FlightArena::new(&dir, &resources, episode, n)?)
        }
        (None, Some(spec)) => {
            let (episode, choice, id) = td_flow::dialogue::parse_scene_spec(spec)?;
            Box::new(td_flow::DialogueScene::load(
                &dir, &resources, episode, choice, id,
            )?)
        }
        (None, None) => match cli.dev_mapview.as_deref() {
            Some(map) => Box::new(mapview::MapView::new(&dir, map)?),
            // The Episode 2 program (`td2.exe`, started directly in the original, U-005).
            None if episode == Episode::E2 => Box::new(
                td_flow::Episode2::new(
                    &dir,
                    &resources,
                    Box::new(flight::ArenaLauncher::for_episode(Episode::E2)),
                )?
                .with_save_dir(td_flow::save_screens::save_dir(&user_dir)),
            ),
            // The Episode 3 program (`td3.exe`, started directly, e3-program-flow §8).
            None if episode == Episode::E3 => Box::new(
                td_flow::Episode3::new(
                    &dir,
                    &resources,
                    Box::new(flight::ArenaLauncher::for_episode(Episode::E3)),
                )?
                .with_save_dir(td_flow::save_screens::save_dir(&user_dir)),
            ),
            // The Episode 1 program: script choice, menu, campaign, flights. Saved games live
            // in the user data directory, never in the game folder. The intro plays first
            // (launcher §4.10).
            None => shell::episode1_start(
                &dir,
                &resources,
                &mut settings,
                &settings_path,
                cli.no_intro,
                Box::new(
                    td_flow::Episode1::new(
                        &dir,
                        &resources,
                        Box::new(flight::ArenaLauncher::default()),
                    )?
                    .with_save_dir(td_flow::save_screens::save_dir(&user_dir)),
                ),
            )?,
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
