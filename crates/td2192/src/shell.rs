//! The port's launcher shell (`docs/spec/flow/launcher.md` rev 1 §4.10): the intro before
//! Episode 1, and the developer entries for the intro and the credits.
//!
//! The original's `td1.exe` runs the intro (`file0000.exe`) once, then Episode 1; Episodes 2 and
//! 3 start without it. Esc skips the intro (its exit fade, then the episode).

use std::path::Path;

use anyhow::{Context, Result, bail};
use td_assets::{DataDir, ExeResources};
use td_flow::credits::{Credits, CreditsMode};
use td_flow::intro::{Intro, IntroOptions};
use td_flow::{Screen, Settings};

/// True when the Enhanced profile is selected.
///
/// ASSUMPTION: `td_flow::Settings` has no profile on this branch yet (IM-12 adds
/// `profile == Profile::Enhanced`); until it lands, everything runs as Classic.
pub fn enhanced(_settings: &Settings) -> bool {
    false
}

/// The intro's options for these settings: OB-058 is fixed only in Enhanced.
pub fn intro_options(settings: &Settings) -> IntroOptions {
    IntroOptions {
        fix_ob058: enhanced(settings),
    }
}

/// Episode 1, preceded by the intro unless `--no-intro`, the settings (Enhanced only) or
/// missing intro files say otherwise. A first-launch policy is recorded in the settings file.
pub fn episode1_start(
    dir: &DataDir,
    res: &ExeResources,
    settings: &mut Settings,
    settings_path: &Path,
    no_intro: bool,
    episode1: Box<dyn Screen>,
) -> Result<Box<dyn Screen>> {
    if no_intro || !settings.intro.plays(enhanced(settings)) {
        return Ok(episode1);
    }
    match Intro::load(dir, res, intro_options(settings)) {
        Ok(intro) => {
            if !settings.intro.seen {
                settings.intro.seen = true;
                settings.save(settings_path)?;
            }
            Ok(Box::new(intro.then(episode1)))
        }
        Err(e) => {
            tracing::warn!("intro unavailable ({e}); starting Episode 1");
            Ok(episode1)
        }
    }
}

/// `--dev-intro`: the intro alone.
pub fn dev_intro(dir: &DataDir, res: &ExeResources, settings: &Settings) -> Result<Box<dyn Screen>> {
    Ok(Box::new(
        Intro::load(dir, res, intro_options(settings)).context("loading the intro")?,
    ))
}

/// `--dev-credits [a|b]`: the credits screen in mode A (the flyover) or B (pages).
pub fn dev_credits(
    dir: &DataDir,
    res: &ExeResources,
    settings: &Settings,
    mode: &str,
) -> Result<Box<dyn Screen>> {
    let mode = match mode {
        "a" | "A" => CreditsMode::Flyover,
        "b" | "B" => CreditsMode::Pages,
        other => bail!("--dev-credits takes a or b, not {other:?}"),
    };
    Ok(Box::new(
        Credits::load(dir, res, mode, intro_options(settings)).context("loading the credits")?,
    ))
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use crate::Cli;

    #[test]
    fn intro_and_credits_flags() {
        let cli = Cli::try_parse_from(["td2192", "--no-intro"]).unwrap();
        assert!(cli.no_intro && !cli.dev_intro && cli.dev_credits.is_none());
        let cli = Cli::try_parse_from(["td2192", "--dev-credits"]).unwrap();
        assert_eq!(cli.dev_credits.as_deref(), Some("a"));
        let cli = Cli::try_parse_from(["td2192", "--dev-credits", "b"]).unwrap();
        assert_eq!(cli.dev_credits.as_deref(), Some("b"));
        let cli = Cli::try_parse_from(["td2192", "--dev-intro"]).unwrap();
        assert!(cli.dev_intro);
    }
}
