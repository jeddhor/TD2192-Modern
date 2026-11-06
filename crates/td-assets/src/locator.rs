//! Data-directory discovery and file-name resolution.
//!
//! Spec: `docs/spec/assets/locator.md` (rev 1). Fidelity: T0.
//!
//! The original executables ask DOS for names such as `helihangar.scr`; DOS silently truncates
//! the base name to 8 characters and the extension to 3, so the file on disk is `helihang.scr`.
//! Names are also case-insensitive. [`DataDir`] reproduces both rules.
//!
//! A data directory is either a folder (an installation, used in place) or the original
//! distribution ZIP, read into memory by [`crate::archive`] (M13-02).

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use thiserror::Error;

use crate::archive::{self, ArchiveError};

/// Environment variable that names the original game directory.
pub const DATA_ENV: &str = "TD2192_DATA";

/// Errors from locating or reading original files.
#[derive(Debug, Error)]
pub enum AssetError {
    /// The data directory could not be listed.
    #[error("cannot read data directory {path}: {source}")]
    DirUnreadable { path: PathBuf, source: io::Error },
    /// A logical file name has no matching file in the data directory.
    #[error("original file not found: {0}")]
    NotFound(String),
    /// A file exists but could not be read.
    #[error("cannot read {path}: {source}")]
    ReadFailed { path: PathBuf, source: io::Error },
    /// The data directory is a file that is not a readable ZIP archive.
    #[error(transparent)]
    Archive(#[from] ArchiveError),
}

/// Converts a file name to the name DOS would open: lower case, base name truncated to 8
/// characters, extension truncated to 3.
pub fn dos83(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    match lower.rsplit_once('.') {
        Some((base, ext)) => {
            let base: String = base.chars().take(8).collect();
            let ext: String = ext.chars().take(3).collect();
            if ext.is_empty() {
                base
            } else {
                format!("{base}.{ext}")
            }
        }
        None => lower.chars().take(8).collect(),
    }
}

/// Where the data directory came from (MDG §8.8 precedence).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataDirSource {
    /// `--data <path>` on the command line.
    CommandLine,
    /// The `TD2192_DATA` environment variable.
    Environment,
    /// The `data_dir` entry of `settings.toml`.
    Settings,
}

/// Picks the data directory by precedence: command line, then environment, then settings.
///
/// The first candidate that is set is returned even if it does not exist, so that the caller can
/// report the problem instead of silently falling through to another source.
pub fn discover_data_dir(
    command_line: Option<&Path>,
    settings: Option<&Path>,
) -> Option<(PathBuf, DataDirSource)> {
    discover_with_env(
        command_line,
        std::env::var_os(DATA_ENV).map(PathBuf::from),
        settings,
    )
}

fn discover_with_env(
    command_line: Option<&Path>,
    env: Option<PathBuf>,
    settings: Option<&Path>,
) -> Option<(PathBuf, DataDirSource)> {
    if let Some(p) = command_line {
        return Some((p.to_path_buf(), DataDirSource::CommandLine));
    }
    if let Some(p) = env.filter(|p| !p.as_os_str().is_empty()) {
        return Some((p, DataDirSource::Environment));
    }
    settings.map(|p| (p.to_path_buf(), DataDirSource::Settings))
}

/// What a [`DataDir`] was opened on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataDirKind {
    /// A folder, used in place.
    Folder,
    /// A ZIP archive, read into memory.
    Archive,
}

/// Where one file's bytes are.
#[derive(Clone)]
enum Contents {
    /// On disk at the entry's path.
    OnDisk,
    /// In memory (an archive entry).
    Memory(Arc<[u8]>),
    /// An archive entry that could not be read, and why.
    Unreadable(String),
}

impl std::fmt::Debug for Contents {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Contents::OnDisk => f.write_str("OnDisk"),
            Contents::Memory(b) => write!(f, "Memory({} bytes)", b.len()),
            Contents::Unreadable(why) => write!(f, "Unreadable({why:?})"),
        }
    }
}

#[derive(Debug, Clone)]
struct Entry {
    /// The file on disk, or for an archive entry the archive's path joined with the entry's
    /// path inside it (not openable with `std::fs`).
    path: PathBuf,
    contents: Contents,
}

/// An original installation (a folder or the original ZIP), listed once and resolved with DOS
/// name rules.
#[derive(Debug, Clone)]
pub struct DataDir {
    root: PathBuf,
    kind: DataDirKind,
    /// Keyed by the DOS 8.3 form of each file's on-disk (or in-archive base) name.
    files: BTreeMap<String, Entry>,
}

impl DataDir {
    /// Opens `root`: a folder is listed (non-recursively) and every regular file indexed by its
    /// 8.3 name; a file is read as a ZIP archive ([`crate::archive`]).
    ///
    /// If two on-disk names collapse to the same 8.3 name, the first in sorted order wins,
    /// which matches a case-insensitive file system holding only one of them.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, AssetError> {
        let root = root.into();
        if fs::metadata(&root).is_ok_and(|m| m.is_file()) {
            return Self::open_archive(root);
        }
        let dir_err = |source| AssetError::DirUnreadable {
            path: root.clone(),
            source,
        };
        let mut names: Vec<(String, PathBuf)> = Vec::new();
        for entry in fs::read_dir(&root).map_err(dir_err)? {
            let entry = entry.map_err(dir_err)?;
            if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                let name = entry.file_name().to_string_lossy().into_owned();
                names.push((name, entry.path()));
            }
        }
        names.sort();
        let mut files = BTreeMap::new();
        for (name, path) in names {
            files.entry(dos83(&name)).or_insert(Entry {
                path,
                contents: Contents::OnDisk,
            });
        }
        Ok(Self {
            root,
            kind: DataDirKind::Folder,
            files,
        })
    }

    fn open_archive(root: PathBuf) -> Result<Self, AssetError> {
        let files = archive::read_zip(&root)?
            .into_iter()
            .map(|(name, f)| {
                let entry = Entry {
                    path: root.join(&f.path),
                    contents: match f.data {
                        Ok(bytes) => Contents::Memory(bytes),
                        Err(why) => Contents::Unreadable(why),
                    },
                };
                (name, entry)
            })
            .collect();
        Ok(Self {
            root,
            kind: DataDirKind::Archive,
            files,
        })
    }

    /// The folder or archive this was opened from.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Whether this is a folder or an archive.
    pub fn kind(&self) -> DataDirKind {
        self.kind
    }

    /// Resolves a logical name (as the original code spells it) to a path. For an archive the
    /// path names the entry inside the archive and cannot be opened with `std::fs`: use
    /// [`Self::read`] for the bytes.
    pub fn resolve(&self, logical: &str) -> Option<&Path> {
        self.files.get(&dos83(logical)).map(|e| e.path.as_path())
    }

    /// Reads a whole file by logical name.
    pub fn read(&self, logical: &str) -> Result<Vec<u8>, AssetError> {
        let entry = self
            .files
            .get(&dos83(logical))
            .ok_or_else(|| AssetError::NotFound(logical.to_string()))?;
        let failed = |source| AssetError::ReadFailed {
            path: entry.path.clone(),
            source,
        };
        match &entry.contents {
            Contents::OnDisk => fs::read(&entry.path).map_err(failed),
            Contents::Memory(bytes) => Ok(bytes.to_vec()),
            Contents::Unreadable(why) => Err(failed(io::Error::other(why.clone()))),
        }
    }

    /// The 8.3 names of every file present, in sorted order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.files.keys().map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dos83_truncates_like_dos() {
        assert_eq!(dos83("helihangar.scr"), "helihang.scr");
        assert_eq!(dos83("HANGAR.SCR"), "hangar.scr");
        assert_eq!(dos83("td.map"), "td.map");
        assert_eq!(dos83("newfaces.td2"), "newfaces.td2");
        assert_eq!(dos83("archive.tar.gz"), "archive..gz");
        assert_eq!(dos83("README"), "readme");
        assert_eq!(dos83("averyverylongname"), "averyver");
        assert_eq!(dos83("x.longext"), "x.lon");
    }

    #[test]
    fn precedence_is_cli_then_env_then_settings() {
        let cli = PathBuf::from("cli");
        let env = PathBuf::from("env");
        let set = PathBuf::from("settings");
        assert_eq!(
            discover_with_env(Some(&cli), Some(env.clone()), Some(&set)),
            Some((cli, DataDirSource::CommandLine))
        );
        assert_eq!(
            discover_with_env(None, Some(env.clone()), Some(&set)),
            Some((env, DataDirSource::Environment))
        );
        assert_eq!(
            discover_with_env(None, Some(PathBuf::new()), Some(&set)),
            Some((set.clone(), DataDirSource::Settings))
        );
        assert_eq!(discover_with_env(None, None, None), None);
    }

    #[test]
    fn resolves_case_insensitively_with_truncation() {
        let dir = std::env::temp_dir().join(format!("td-assets-locator-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("HELIHANG.SCR"), b"scr").unwrap();
        fs::write(dir.join("td.pal"), b"pal").unwrap();
        fs::create_dir_all(dir.join("subdir.td")).unwrap();

        let data = DataDir::open(&dir).unwrap();
        assert_eq!(data.read("helihangar.scr").unwrap(), b"scr");
        assert_eq!(data.read("TD.PAL").unwrap(), b"pal");
        assert!(
            data.resolve("subdir.td").is_none(),
            "directories are not files"
        );
        assert!(matches!(
            data.read("missing.voc"),
            Err(AssetError::NotFound(_))
        ));
        assert_eq!(data.names().collect::<Vec<_>>(), ["helihang.scr", "td.pal"]);
        assert_eq!(data.kind(), DataDirKind::Folder);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_zip_file_opens_as_an_archive() {
        use zip::CompressionMethod::{Deflated, Stored};
        let dir =
            std::env::temp_dir().join(format!("td-assets-locator-zip-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("TD2192.ZIP");
        let bytes = crate::archive::zip_of(&[
            ("TD2192/HELIHANG.SCR", b"scr", Stored),
            ("TD2192/TD.PAL", &[3u8; 768], Deflated),
        ]);
        fs::write(&path, bytes).unwrap();
        let data = DataDir::open(&path).unwrap();
        assert_eq!(data.kind(), DataDirKind::Archive);
        assert_eq!(data.root(), path);
        assert_eq!(data.read("helihangar.scr").unwrap(), b"scr");
        assert_eq!(data.read("td.pal").unwrap(), vec![3u8; 768]);
        assert!(data.resolve("td.pal").is_some());
        assert!(matches!(data.read("td.map"), Err(AssetError::NotFound(_))));
        // A file that is not a ZIP is an error, not an empty installation.
        let junk = dir.join("notes.txt");
        fs::write(&junk, b"hello").unwrap();
        assert!(matches!(DataDir::open(&junk), Err(AssetError::Archive(_))));
        fs::remove_dir_all(&dir).unwrap();
    }
}
