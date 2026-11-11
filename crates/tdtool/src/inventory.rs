//! `tdtool inventory`: validate an installation and report per-episode playability.

use std::path::Path;

use anyhow::{Context, Result, bail};
use td_assets::exe_res::ExeResError;
use td_assets::{DataDir, Episode, ExeResources, FileStatus, discover_data_dir, validate};

pub fn run(data: Option<&Path>, verbose: bool) -> Result<()> {
    let Some((root, source)) = discover_data_dir(data, None) else {
        bail!("no data directory: pass --data <dir> or set TD2192_DATA");
    };
    let dir = DataDir::open(&root).with_context(|| format!("opening {}", root.display()))?;
    let report = validate(&dir);

    println!("Data directory: {} (from {source:?})", root.display());
    if verbose {
        for c in &report.checks {
            println!("  {:<14} {:>8}  {:?}", c.entry.name, c.entry.size, c.status);
        }
    }
    for c in report.problems() {
        let note = match &c.status {
            FileStatus::Missing if !c.entry.required => " (optional)",
            _ => "",
        };
        println!("  PROBLEM {:<14} {:?}{note}", c.entry.name, c.status);
    }
    for name in &report.extra {
        println!("  extra   {name}");
    }
    println!(
        "{}/{} files match the manifest",
        report.ok_count(),
        report.checks.len()
    );
    // D10 (MDG §14): ordinary asset presence is not enough when resource
    // executables have unknown offsets.
    let resource_check = ExeResources::load_supported(&dir);
    let resource_issue = match &resource_check {
        Ok(_) => None,
        Err(ExeResError::UnsupportedVersion { .. }) => Some("unsupported retail executable"),
        Err(_) => Some("executable resources unavailable"),
    };
    for ep in Episode::ALL {
        if let Some(issue) = resource_issue {
            println!("{ep}: NOT playable ({issue})");
        } else {
            let state = match (report.playable(ep), report.exact(ep)) {
                (true, true) => "playable (exact)",
                (true, false) => "playable (modified files)",
                _ => "NOT playable (required files missing)",
            };
            println!("{ep}: {state}");
        }
    }
    println!(
        "Intro: {}",
        if report.intro_available() {
            "available"
        } else {
            "missing"
        }
    );
    resource_check?;
    Ok(())
}
