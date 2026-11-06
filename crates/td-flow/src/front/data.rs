//! What the game-data page shows and asks for (M11-03 data directory, M13-02 first run).
//!
//! The page lives in the options screens ([`super::options`]), and the first run opens it on
//! its own to show what was found. The screens cannot open the system's file dialogs or touch
//! the disk (`td-flow` has no platform), so they work through the [`super::SettingsPort`]:
//! - the host describes the data in use and any new choice in a [`DataInfo`];
//! - the page asks for a [`DataRequest`] (a folder, a ZIP file, or extracting the ZIP); the
//!   host opens the dialog, validates the choice against the manifest and updates the info;
//! - a usable new choice is saved at once and is used from the start screen on: the start
//!   screen asks the host to switch when it is on top
//!   ([`super::SettingsPort::want_data_switch`]), then reloads its fonts and episodes.
//!
//! Precedence (MDG §8.8): `--data` > `TD2192_DATA` > `settings.toml` > the first-run picker.
//! While the command line or the environment names the data, the page shows it but cannot
//! change it: a choice saved to `settings.toml` would not be used.

use std::path::PathBuf;

use td_assets::{DataDir, DataDirKind, DataDirSource, FileCounts, ValidationReport};

/// Where the data in use was named.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataSource {
    /// `--data` on the command line.
    CommandLine,
    /// The `TD2192_DATA` environment variable.
    Environment,
    /// `data_dir` in `settings.toml`.
    Settings,
    /// Chosen in this session (the first run's picker or this page).
    Chosen,
}

impl DataSource {
    /// True if a choice saved to the settings would not be used (MDG §8.8).
    pub fn overrides_settings(self) -> bool {
        matches!(self, DataSource::CommandLine | DataSource::Environment)
    }
}

impl From<DataDirSource> for DataSource {
    fn from(s: DataDirSource) -> Self {
        match s {
            DataDirSource::CommandLine => DataSource::CommandLine,
            DataDirSource::Environment => DataSource::Environment,
            DataDirSource::Settings => DataSource::Settings,
        }
    }
}

/// One validated folder or archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataSummary {
    pub path: PathBuf,
    /// A ZIP archive (read in place) rather than a folder.
    pub archive: bool,
    pub counts: FileCounts,
    /// Episodes 1, 2, 3.
    pub playable: [bool; 3],
}

impl DataSummary {
    /// The summary of `dir` validated as `report`.
    pub fn of(dir: &DataDir, report: &ValidationReport) -> Self {
        Self {
            path: dir.root().to_path_buf(),
            archive: dir.kind() == DataDirKind::Archive,
            counts: report.counts(),
            playable: report.playable_episodes(),
        }
    }

    /// True if the port can start with it: Episode 1's files are all present (the start
    /// screen and the port's own screens use them).
    pub fn usable(&self) -> bool {
        self.playable[0]
    }

    /// One line on the files, for example `218 of 218 files match` or
    /// `200 match - 3 modified - 15 missing`.
    pub fn files_line(&self) -> String {
        let c = self.counts;
        let total = c.ok + c.modified + c.missing + c.unreadable;
        if c.ok == total {
            return format!("{} of {total} files match", c.ok);
        }
        let mut parts = vec![format!("{} match", c.ok)];
        for (n, what) in [
            (c.modified, "modified"),
            (c.missing, "missing"),
            (c.unreadable, "unreadable"),
        ] {
            if n > 0 {
                parts.push(format!("{n} {what}"));
            }
        }
        parts.join(" - ")
    }

    /// The playable episodes, for example `episodes 1 2 3`, `episode 1` or `none`.
    pub fn playable_line(&self) -> String {
        let eps: Vec<String> = (1..=3)
            .filter(|&n| self.playable[n - 1])
            .map(|n| n.to_string())
            .collect();
        match eps.len() {
            0 => "none".into(),
            1 => format!("episode {}", eps[0]),
            _ => format!("episodes {}", eps.join(" ")),
        }
    }
}

/// What the host tells the game-data page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataInfo {
    /// The data in use.
    pub current: DataSummary,
    pub source: DataSource,
    /// A usable new choice of this session, saved and used from the start screen on.
    pub pending: Option<DataSummary>,
    /// The outcome of the last request, for example `nothing changed - the folder has no
    /// episode 1`.
    pub message: Option<String>,
    /// Where extracting a ZIP puts the game files.
    pub import_dir: PathBuf,
}

impl DataInfo {
    /// The data the page describes: the new choice if there is one, else the data in use.
    pub fn shown(&self) -> &DataSummary {
        self.pending.as_ref().unwrap_or(&self.current)
    }

    /// True if the page may change the data (no command-line or environment override).
    pub fn changeable(&self) -> bool {
        !self.source.overrides_settings()
    }
}

/// What the game-data page asks the host to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataRequest {
    /// Show the folder picker and validate the folder chosen.
    PickFolder,
    /// Show the file picker for a ZIP archive and validate it (read in place).
    PickZip,
    /// Copy the game files of the ZIP shown into [`DataInfo::import_dir`] and use the copy.
    Extract,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(counts: FileCounts, playable: [bool; 3]) -> DataSummary {
        DataSummary {
            path: PathBuf::from("C:/Games/TD2192"),
            archive: false,
            counts,
            playable,
        }
    }

    #[test]
    fn lines_describe_the_files_and_episodes() {
        let all = summary(
            FileCounts {
                ok: 218,
                ..FileCounts::default()
            },
            [true; 3],
        );
        assert_eq!(all.files_line(), "218 of 218 files match");
        assert_eq!(all.playable_line(), "episodes 1 2 3");
        assert!(all.usable());
        let some = summary(
            FileCounts {
                ok: 200,
                modified: 3,
                missing: 15,
                unreadable: 0,
            },
            [true, false, false],
        );
        assert_eq!(some.files_line(), "200 match - 3 modified - 15 missing");
        assert_eq!(some.playable_line(), "episode 1");
        let none = summary(FileCounts::default(), [false, true, false]);
        assert_eq!(none.playable_line(), "episode 2");
        assert!(!none.usable(), "the port needs episode 1");
        assert_eq!(
            summary(FileCounts::default(), [false; 3]).playable_line(),
            "none"
        );
    }

    #[test]
    fn overrides_lock_the_page() {
        let info = |source| DataInfo {
            current: summary(FileCounts::default(), [true; 3]),
            source,
            pending: None,
            message: None,
            import_dir: PathBuf::from("game"),
        };
        assert!(!info(DataSource::CommandLine).changeable());
        assert!(!info(DataSource::Environment).changeable());
        assert!(info(DataSource::Settings).changeable());
        assert!(info(DataSource::Chosen).changeable());
    }
}
