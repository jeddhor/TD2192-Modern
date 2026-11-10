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

/// A GUI-subsystem executable must still report script errors through redirected stderr.
#[test]
fn invalid_arguments_report_stderr_and_fail_without_a_dialog() {
    let out = Command::new(env!("CARGO_BIN_EXE_td2192"))
        .arg("--this-option-does-not-exist")
        .output()
        .expect("running td2192 with an invalid option");
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("unexpected argument"));
    assert!(out.stdout.is_empty());
}

/// The Windows loader must not create a console when launched from Explorer.
#[cfg(windows)]
#[test]
fn windows_executable_uses_the_gui_subsystem() {
    let exe = std::fs::read(env!("CARGO_BIN_EXE_td2192")).unwrap();
    let pe = u32::from_le_bytes(exe[0x3c..0x40].try_into().unwrap()) as usize;
    assert_eq!(&exe[pe..pe + 4], b"PE\0\0");
    // IMAGE_OPTIONAL_HEADER32/64: Subsystem is 68 bytes after the optional header starts.
    let subsystem = pe + 4 + 20 + 68;
    assert_eq!(
        u16::from_le_bytes(exe[subsystem..subsystem + 2].try_into().unwrap()),
        2
    );
}
