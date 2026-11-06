//! Copying the game files of an installation or archive into the per-user data directory
//! (M13-02, MDG §5.5).
//!
//! The port reads an installation folder in place and can read the original ZIP in place too
//! ([`crate::archive`]). With the player's consent, the files of an archive are extracted
//! instead, so the archive is no longer needed. [`extract`] writes **only manifest files**, each
//! under its manifest (8.3, lower-case) name: the names inside the archive never become paths
//! on disk, and files that are not part of the game (read-me files, other programs) are left
//! out. Modified files (another size or hash) are copied too, because validation counts them
//! as present (a warning, MDG §9.1.2).
//!
//! The destination is a folder the port owns ([`IMPORT_DIR`] under the user data directory).
//! A manifest file left there by an earlier import that the new source lacks is removed, so
//! the folder always validates like its source. Nothing else in it is touched.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::DataDir;
use crate::manifest;

/// The folder under the user data directory that extracted game files go into.
pub const IMPORT_DIR: &str = "game";

/// Where [`extract`] puts the game files for the user data directory `user_dir`.
pub fn import_dir(user_dir: &Path) -> PathBuf {
    user_dir.join(IMPORT_DIR)
}

/// Errors from [`extract`].
#[derive(Debug, Error)]
pub enum ImportError {
    /// The source and the destination are the same folder.
    #[error("{0} is already the destination folder")]
    SameFolder(PathBuf),
    /// Writing the destination failed.
    #[error("cannot write {path}: {source}")]
    Write { path: PathBuf, source: io::Error },
}

/// What [`extract`] did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ImportSummary {
    /// Manifest files written.
    pub written: usize,
    /// Stale manifest files removed from the destination.
    pub removed: usize,
}

/// Copies every manifest file that `src` holds and can read into `dest` (created if needed),
/// replacing what an earlier import left there. Each file is written to a temporary name and
/// then renamed, so a file is never left half-written under its real name.
pub fn extract(src: &DataDir, dest: &Path) -> Result<ImportSummary, ImportError> {
    let write_err = |path: &Path| {
        let path = path.to_path_buf();
        move |source| ImportError::Write { path, source }
    };
    if same_folder(src.root(), dest) {
        return Err(ImportError::SameFolder(dest.to_path_buf()));
    }
    fs::create_dir_all(dest).map_err(write_err(dest))?;
    let mut summary = ImportSummary::default();
    for entry in manifest::entries() {
        let target = dest.join(entry.name);
        match src.read(entry.name) {
            Ok(bytes) => {
                let part = dest.join(format!("{}.part", entry.name));
                fs::write(&part, &bytes).map_err(write_err(&part))?;
                fs::rename(&part, &target).map_err(write_err(&target))?;
                summary.written += 1;
            }
            Err(_) => {
                if target.is_file() {
                    fs::remove_file(&target).map_err(write_err(&target))?;
                    summary.removed += 1;
                }
            }
        }
    }
    Ok(summary)
}

/// True if `a` and `b` name the same existing folder.
fn same_folder(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate;
    use zip::CompressionMethod::{Deflated, Stored};

    fn temp(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("td-assets-import-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn extracts_only_manifest_files_under_their_83_names() {
        let dir = temp("zip");
        let zip = dir.join("TD2192.ZIP");
        let pal = vec![9u8; manifest::entry("td.pal").unwrap().size as usize];
        fs::write(
            &zip,
            crate::archive::zip_of(&[
                ("TD2192/TD.CFG", &[2, 2, 2], Stored),
                ("TD2192/TD.PAL", &pal, Deflated),
                ("TD2192/README.TXT", b"not a game file", Stored),
                ("../../escape.cfg", b"never written", Stored),
            ]),
        )
        .unwrap();
        let src = DataDir::open(&zip).unwrap();
        let dest = dir.join("game");
        let summary = extract(&src, &dest).unwrap();
        assert_eq!(
            summary,
            ImportSummary {
                written: 2,
                removed: 0
            }
        );
        let mut names: Vec<_> = fs::read_dir(&dest)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        assert_eq!(names, ["td.cfg", "td.pal"]);
        assert!(!dir.join("escape.cfg").exists());
        let before = validate(&src).counts();
        let after = validate(&DataDir::open(&dest).unwrap()).counts();
        assert_eq!(before, after, "the copy validates like the archive");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_new_import_replaces_the_old_one() {
        let dir = temp("again");
        let dest = dir.join("game");
        fs::create_dir_all(&dest).unwrap();
        fs::write(dest.join("td.pal"), b"stale").unwrap();
        fs::write(dest.join("td.cfg"), b"old").unwrap();
        fs::write(dest.join("mine.txt"), b"kept").unwrap();
        let src_dir = dir.join("install");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("TD.CFG"), [2, 2, 2]).unwrap();
        let summary = extract(&DataDir::open(&src_dir).unwrap(), &dest).unwrap();
        assert_eq!(
            summary,
            ImportSummary {
                written: 1,
                removed: 1
            }
        );
        assert_eq!(fs::read(dest.join("td.cfg")).unwrap(), [2, 2, 2]);
        assert!(!dest.join("td.pal").exists(), "a stale manifest file goes");
        assert!(dest.join("mine.txt").exists(), "other files stay");
        // Importing a folder into itself is refused.
        assert!(matches!(
            extract(&DataDir::open(&dest).unwrap(), &dest),
            Err(ImportError::SameFolder(_))
        ));
        fs::remove_dir_all(&dir).unwrap();
    }

    /// The original files, zipped at test time into a temporary folder (in a top-level folder,
    /// upper-case names, Deflate), read in place and extracted: both validate exactly.
    #[test]
    fn the_original_files_zipped_read_in_place_and_extract_exactly() {
        let data = td_test_support::require_data!();
        let install = DataDir::open(&data).unwrap();
        let contents: Vec<(String, Vec<u8>)> = install
            .names()
            .map(|n| {
                (
                    format!("TD2192/{}", n.to_uppercase()),
                    install.read(n).unwrap(),
                )
            })
            .collect();
        let files: Vec<(&str, &[u8], zip::CompressionMethod)> = contents
            .iter()
            .map(|(n, b)| (n.as_str(), b.as_slice(), Deflated))
            .collect();
        let dir = temp("original");
        let zip = dir.join("TD2192.ZIP");
        fs::write(&zip, crate::archive::zip_of(&files)).unwrap();
        let src = DataDir::open(&zip).unwrap();
        assert_eq!(validate(&src).ok_count(), 218);
        let dest = import_dir(&dir);
        let summary = extract(&src, &dest).unwrap();
        assert_eq!(summary.written, 218);
        let report = validate(&DataDir::open(&dest).unwrap());
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(report.ok_count(), 218);
        assert!(report.extra.is_empty(), "{:?}", report.extra);
    }
}
