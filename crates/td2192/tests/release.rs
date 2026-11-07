//! Release plumbing (M13-03, docs/RELEASE.md §1): the binary reports the workspace version, and
//! the changelog has a section for it. Neither needs game data.

use std::process::Command;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[test]
fn version_flag_prints_the_workspace_version() {
    let out = Command::new(env!("CARGO_BIN_EXE_td2192"))
        .arg("--version")
        .output()
        .expect("running td2192 --version");
    assert!(out.status.success(), "{out:?}");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        format!("td2192 {VERSION}")
    );
}

#[test]
fn help_flag_lists_the_player_options() {
    let out = Command::new(env!("CARGO_BIN_EXE_td2192"))
        .arg("--help")
        .output()
        .expect("running td2192 --help");
    assert!(out.status.success(), "{out:?}");
    let help = String::from_utf8_lossy(&out.stdout);
    for option in [
        "--data",
        "--episode",
        "--check",
        "--fullscreen",
        "--no-intro",
        "--version",
    ] {
        assert!(
            help.contains(option),
            "--help does not list {option}:\n{help}"
        );
    }
}

#[test]
fn changelog_has_a_section_for_this_version() {
    let changelog = include_str!("../../../CHANGELOG.md");
    let heading = format!("## [{VERSION}]");
    assert!(
        changelog.lines().any(|l| l.starts_with(&heading)),
        "CHANGELOG.md has no `{heading}` section: add one when the version changes \
         (docs/RELEASE.md §3.1)"
    );
}
