//! Data-directory discovery and file-name resolution.
//!
//! Spec: `docs/spec/assets/locator.md` (rev 1). Fidelity: T0.
//!
//! The original executables ask DOS for names such as `helihangar.scr`; DOS silently truncates
//! the base name to 8 characters and the extension to 3, so the file on disk is `helihang.scr`.
//! Names are also case-insensitive. [`DataDir`] reproduces both rules.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;

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

/// An original installation directory, listed once and resolved with DOS name rules.
#[derive(Debug, Clone)]
pub struct DataDir {
    root: PathBuf,
    /// Keyed by the DOS 8.3 form of each file's on-disk name.
    files: BTreeMap<String, PathBuf>,
}

impl DataDir {
    /// Lists `root` (non-recursively) and indexes every regular file by its 8.3 name.
    ///
    /// If two on-disk names collapse to the same 8.3 name, the first in sorted order wins,
    /// which matches a case-insensitive file system holding only one of them.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, AssetError> {
        let root = root.into();
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
            files.entry(dos83(&name)).or_insert(path);
        }
        Ok(Self { root, files })
    }

    /// The directory this was opened from.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolves a logical name (as the original code spells it) to a path on disk.
    pub fn resolve(&self, logical: &str) -> Option<&Path> {
        self.files.get(&dos83(logical)).map(PathBuf::as_path)
    }

    /// Reads a whole file by logical name.
    pub fn read(&self, logical: &str) -> Result<Vec<u8>, AssetError> {
        let path = self
            .resolve(logical)
            .ok_or_else(|| AssetError::NotFound(logical.to_string()))?;
        fs::read(path).map_err(|source| AssetError::ReadFailed {
            path: path.to_path_buf(),
            source,
        })
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

        fs::remove_dir_all(&dir).unwrap();
    }
}
