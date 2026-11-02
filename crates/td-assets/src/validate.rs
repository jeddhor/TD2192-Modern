//! Validation of a data directory against the manifest.
//!
//! A modified file (hash mismatch) is a warning, not an error: patched or alternative releases
//! exist. An episode is playable when every required file it uses is present.

use std::fs;

use sha2::{Digest, Sha256};

use crate::DataDir;
use crate::manifest::{self, Episode, ManifestEntry, Usage};

/// Outcome for one manifest file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileStatus {
    /// Present with the expected size and SHA-256.
    Ok,
    /// Not found in the directory.
    Missing,
    /// Present but with a different size.
    SizeMismatch { actual: u64 },
    /// Same size, different contents.
    HashMismatch,
    /// Present but unreadable.
    Unreadable(String),
}

impl FileStatus {
    /// True when the file is present and readable (even if modified).
    pub fn is_present(&self) -> bool {
        !matches!(self, FileStatus::Missing | FileStatus::Unreadable(_))
    }
}

/// The check result for one manifest entry.
#[derive(Debug, Clone)]
pub struct FileCheck {
    pub entry: &'static ManifestEntry,
    pub status: FileStatus,
}

/// Result of [`validate`].
#[derive(Debug, Clone)]
pub struct ValidationReport {
    pub checks: Vec<FileCheck>,
    /// Files in the directory that are not in the manifest (8.3 names).
    pub extra: Vec<String>,
}

impl ValidationReport {
    /// Number of manifest files that matched exactly.
    pub fn ok_count(&self) -> usize {
        self.checks
            .iter()
            .filter(|c| c.status == FileStatus::Ok)
            .count()
    }

    /// Checks that are not [`FileStatus::Ok`].
    pub fn problems(&self) -> impl Iterator<Item = &FileCheck> {
        self.checks.iter().filter(|c| c.status != FileStatus::Ok)
    }

    /// True if every required file used by `episode` is present.
    pub fn playable(&self, episode: Episode) -> bool {
        self.required_for(episode.usage())
            .all(|c| c.status.is_present())
    }

    /// True if every required file of `episode` is present and unmodified.
    pub fn exact(&self, episode: Episode) -> bool {
        self.required_for(episode.usage())
            .all(|c| c.status == FileStatus::Ok)
    }

    /// True if the intro sequence's files are all present.
    pub fn intro_available(&self) -> bool {
        self.checks
            .iter()
            .filter(|c| c.entry.used_by.contains(Usage::INTRO))
            .all(|c| c.status.is_present())
    }

    fn required_for(&self, usage: Usage) -> impl Iterator<Item = &FileCheck> {
        self.checks
            .iter()
            .filter(move |c| c.entry.required && c.entry.used_by.contains(usage))
    }
}

/// Checks every manifest file in `dir`.
pub fn validate(dir: &DataDir) -> ValidationReport {
    let checks = manifest::entries()
        .iter()
        .map(|entry| FileCheck {
            entry,
            status: check_file(dir, entry),
        })
        .collect();
    let extra = dir
        .names()
        .filter(|n| manifest::entry(n).is_none())
        .map(str::to_string)
        .collect();
    ValidationReport { checks, extra }
}

fn check_file(dir: &DataDir, entry: &ManifestEntry) -> FileStatus {
    let Some(path) = dir.resolve(entry.name) else {
        return FileStatus::Missing;
    };
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) => return FileStatus::Unreadable(e.to_string()),
    };
    if bytes.len() as u64 != entry.size {
        return FileStatus::SizeMismatch {
            actual: bytes.len() as u64,
        };
    }
    let digest = Sha256::digest(&bytes);
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    if hex == entry.sha256 {
        FileStatus::Ok
    } else {
        FileStatus::HashMismatch
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_directory_has_nothing_playable() {
        let dir = std::env::temp_dir().join(format!("td-assets-validate-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("td.cfg"), [2, 2, 2]).unwrap();
        fs::write(dir.join("notes.txt"), b"x").unwrap();
        let report = validate(&DataDir::open(&dir).unwrap());
        fs::remove_dir_all(&dir).unwrap();

        assert_eq!(report.ok_count(), 1, "the shipped td.cfg is 02 02 02");
        assert_eq!(report.extra, ["notes.txt"]);
        for ep in Episode::ALL {
            assert!(!report.playable(ep));
        }
        assert!(!report.intro_available());
    }

    #[test]
    fn original_installation_is_complete() {
        let data = td_test_support::require_data!();
        let report = validate(&DataDir::open(data).unwrap());
        let problems: Vec<_> = report.problems().collect();
        assert!(problems.is_empty(), "unexpected problems: {problems:#?}");
        assert_eq!(report.ok_count(), 218);
        for ep in Episode::ALL {
            assert!(report.exact(ep), "{ep} should be exact");
        }
        assert!(report.intro_available());
    }
}
