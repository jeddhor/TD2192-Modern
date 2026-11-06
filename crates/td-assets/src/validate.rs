//! Validation of a data directory against the manifest.
//!
//! A modified file (hash mismatch) is a warning, not an error: patched or alternative releases
//! exist. An episode is playable when every required file it uses is present.

use sha2::{Digest, Sha256};

use crate::manifest::{self, Episode, ManifestEntry, Usage};
use crate::{AssetError, DataDir};

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

    /// The episodes that are playable, in order.
    pub fn playable_episodes(&self) -> [bool; 3] {
        Episode::ALL.map(|ep| self.playable(ep))
    }

    /// File counts by outcome.
    pub fn counts(&self) -> FileCounts {
        let mut c = FileCounts::default();
        for check in &self.checks {
            match check.status {
                FileStatus::Ok => c.ok += 1,
                FileStatus::SizeMismatch { .. } | FileStatus::HashMismatch => c.modified += 1,
                FileStatus::Missing => c.missing += 1,
                FileStatus::Unreadable(_) => c.unreadable += 1,
            }
        }
        c
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

/// How many manifest files had each outcome ([`ValidationReport::counts`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FileCounts {
    /// Present with the expected size and SHA-256.
    pub ok: usize,
    /// Present with another size or hash (patched or other releases; a warning).
    pub modified: usize,
    pub missing: usize,
    pub unreadable: usize,
}

impl FileCounts {
    /// Manifest files found (matching or modified).
    pub fn present(&self) -> usize {
        self.ok + self.modified
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
    let bytes = match dir.read(entry.name) {
        Ok(b) => b,
        Err(AssetError::NotFound(_)) => return FileStatus::Missing,
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
    use std::fs;

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
        assert_eq!(
            report.counts(),
            FileCounts {
                ok: 1,
                modified: 0,
                missing: 217,
                unreadable: 0
            }
        );
        assert_eq!(report.playable_episodes(), [false; 3]);
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
        assert_eq!(report.counts().present(), 218);
    }

    #[test]
    fn an_archive_validates_like_a_folder() {
        use zip::CompressionMethod::Stored;
        let dir =
            std::env::temp_dir().join(format!("td-assets-validate-zip-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("td.zip");
        // td.cfg as shipped (matches), td.pal of the right size with other bytes (modified),
        // and a file that is not in the manifest (extra), all in a folder of the archive.
        let pal = vec![0u8; manifest::entry("td.pal").unwrap().size as usize];
        let bytes = crate::archive::zip_of(&[
            ("game/td.cfg", &[2, 2, 2], Stored),
            ("game/td.pal", &pal, Stored),
            ("game/readme.txt", b"x", Stored),
        ]);
        fs::write(&path, bytes).unwrap();
        let report = validate(&DataDir::open(&path).unwrap());
        fs::remove_dir_all(&dir).unwrap();
        let c = report.counts();
        assert_eq!((c.ok, c.modified, c.missing), (1, 1, 216));
        assert_eq!(report.extra, ["readme.txt"]);
    }
}
