//! Reading the original distribution ZIP in place (M13-02, MDG §5.5).
//!
//! A [`crate::DataDir`] can be opened on a `.zip` file instead of a folder. The archive is read
//! once into memory (the whole installation is about 9 MB): every file entry is decompressed
//! and indexed by the DOS 8.3 form of its base name, whatever folder it sits in, so a ZIP with
//! the game files at its root and one with a top-level folder (`TD2192/FILE0001.EXE`) both
//! work. When two entries collapse to the same 8.3 name, the one nearest the root wins, then
//! the first in path order.
//!
//! Supported compression: stored, Deflate, and the PKZIP 1.x methods Shrink, Reduce and Implode
//! that archives from the early 1990s may use. An entry that cannot be read (an unsupported
//! method, a damaged entry, encryption) stays listed as unreadable, so validation reports it
//! instead of calling the file missing.
//!
//! Limits guard against damaged or hostile archives: entries larger than [`MAX_ENTRY`] are
//! skipped (no original file is near that size), and an archive whose files add up to more
//! than [`MAX_TOTAL`] or that holds more than [`MAX_ENTRIES`] entries is refused. Nothing is
//! written anywhere; names inside the archive are never used as paths on disk.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use thiserror::Error;

use crate::locator::dos83;

/// Largest entry that is read (bytes). The largest original file is 267,264 bytes.
pub const MAX_ENTRY: u64 = 8 << 20;
/// Largest total of the entries read (bytes).
pub const MAX_TOTAL: u64 = 256 << 20;
/// Most entries an archive may hold.
pub const MAX_ENTRIES: usize = 20_000;

/// Errors opening an archive.
#[derive(Debug, Error)]
pub enum ArchiveError {
    /// The file could not be opened or read.
    #[error("cannot read {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    /// Not a ZIP archive, or a damaged one.
    #[error("{path} is not a readable ZIP archive: {reason}")]
    NotZip { path: PathBuf, reason: String },
    /// Over [`MAX_TOTAL`] or [`MAX_ENTRIES`].
    #[error("{path} is too large to be the original game archive")]
    TooLarge { path: PathBuf },
}

/// One file of an archive.
#[derive(Clone)]
pub struct ArchiveFile {
    /// The entry's path inside the archive (`/`-separated).
    pub path: String,
    /// Its contents, or why they could not be read.
    pub data: Result<Arc<[u8]>, String>,
}

impl std::fmt::Debug for ArchiveFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArchiveFile")
            .field("path", &self.path)
            .field(
                "data",
                &self.data.as_ref().map(|b| format!("{} bytes", b.len())),
            )
            .finish()
    }
}

/// Folder depth of an archive path (0 at the root).
fn depth(path: &str) -> usize {
    path.trim_matches('/').matches('/').count()
}

/// The base name of an archive path (either separator).
fn base_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// Reads every file entry of the ZIP at `path`, keyed by the 8.3 form of its base name (see
/// the module docs for duplicates and limits).
pub fn read_zip(path: &Path) -> Result<BTreeMap<String, ArchiveFile>, ArchiveError> {
    let file = File::open(path).map_err(|source| ArchiveError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    read_zip_from(BufReader::new(file), path)
}

/// [`read_zip`] from any reader; `path` names it in errors.
pub fn read_zip_from(
    reader: impl Read + std::io::Seek,
    path: &Path,
) -> Result<BTreeMap<String, ArchiveFile>, ArchiveError> {
    let not_zip = |e: zip::result::ZipError| ArchiveError::NotZip {
        path: path.to_path_buf(),
        reason: e.to_string(),
    };
    let mut zip = zip::ZipArchive::new(reader).map_err(not_zip)?;
    if zip.len() > MAX_ENTRIES {
        return Err(ArchiveError::TooLarge {
            path: path.to_path_buf(),
        });
    }
    let mut total: u64 = 0;
    let mut files: BTreeMap<String, ArchiveFile> = BTreeMap::new();
    for i in 0..zip.len() {
        let listed = zip
            .name_for_index(i)
            .and_then(Result::ok)
            .map(|n| n.into_owned());
        let mut entry = match zip.by_index(i) {
            Ok(e) => e,
            Err(e) => {
                // The entry's header is damaged or uses an unsupported feature; list it by
                // the name the central directory gives it.
                if let Some(name) = listed.filter(|n| !n.ends_with('/')) {
                    insert(&mut files, name, Err(e.to_string()));
                }
                continue;
            }
        };
        if !entry.is_file() {
            continue;
        }
        let name = match entry.name() {
            Ok(n) => n.into_owned(),
            Err(_) => String::from_utf8_lossy(entry.name_raw()).into_owned(),
        };
        if entry.size() > MAX_ENTRY {
            tracing::debug!("archive: skipping {name} ({} bytes)", entry.size());
            continue;
        }
        let data = if entry.encrypted() {
            Err("encrypted".to_string())
        } else {
            let mut bytes = Vec::with_capacity(entry.size() as usize);
            match (&mut entry).take(MAX_ENTRY + 1).read_to_end(&mut bytes) {
                Ok(n) if n as u64 > MAX_ENTRY => {
                    tracing::debug!("archive: skipping {name} (larger than declared)");
                    continue;
                }
                Ok(n) => {
                    total += n as u64;
                    if total > MAX_TOTAL {
                        return Err(ArchiveError::TooLarge {
                            path: path.to_path_buf(),
                        });
                    }
                    Ok(Arc::from(bytes))
                }
                Err(e) => Err(e.to_string()),
            }
        };
        insert(&mut files, name, data);
    }
    Ok(files)
}

/// Adds `name` unless an entry nearer the root (or as near and earlier in path order) already
/// holds its 8.3 name.
fn insert(
    files: &mut BTreeMap<String, ArchiveFile>,
    name: String,
    data: Result<Arc<[u8]>, String>,
) {
    let base = base_name(&name);
    if base.is_empty() {
        return;
    }
    let key = dos83(base);
    let better = match files.get(&key) {
        None => true,
        Some(old) => (depth(&name), &name) < (depth(&old.path), &old.path),
    };
    if better {
        files.insert(key, ArchiveFile { path: name, data });
    }
}

/// A synthetic archive for tests: (path, contents, method); a path ending in `/` is a folder.
#[cfg(test)]
pub(crate) fn zip_of(files: &[(&str, &[u8], zip::CompressionMethod)]) -> Vec<u8> {
    use std::io::{Cursor, Write};
    use zip::write::SimpleFileOptions;
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for &(name, bytes, method) in files {
        if name.ends_with('/') {
            w.add_directory(name, SimpleFileOptions::default()).unwrap();
            continue;
        }
        w.start_file(
            name,
            SimpleFileOptions::default().compression_method(method),
        )
        .unwrap();
        w.write_all(bytes).unwrap();
    }
    w.finish().unwrap().into_inner()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use zip::CompressionMethod;

    fn read(bytes: Vec<u8>) -> Result<BTreeMap<String, ArchiveFile>, ArchiveError> {
        read_zip_from(Cursor::new(bytes), Path::new("test.zip"))
    }

    #[test]
    fn reads_stored_and_deflated_entries_by_their_83_names() {
        let big = vec![7u8; 5000];
        let files = read(zip_of(&[
            ("TD2192/", b"", CompressionMethod::Stored),
            ("TD2192/HELIHANGAR.SCR", b"scr", CompressionMethod::Stored),
            ("TD2192/Td.Pal", &big, CompressionMethod::Deflated),
        ]))
        .unwrap();
        assert_eq!(files.keys().collect::<Vec<_>>(), ["helihang.scr", "td.pal"]);
        assert_eq!(files["helihang.scr"].data.as_deref().unwrap(), b"scr");
        assert_eq!(files["td.pal"].data.as_deref().unwrap(), big.as_slice());
        assert_eq!(files["td.pal"].path, "TD2192/Td.Pal");
    }

    #[test]
    fn the_entry_nearest_the_root_wins() {
        let files = read(zip_of(&[
            ("deep/er/td.pal", b"deep", CompressionMethod::Stored),
            ("TD.PAL", b"root", CompressionMethod::Stored),
            ("b/td.pal", b"b", CompressionMethod::Stored),
            ("a/td.pal", b"a", CompressionMethod::Stored),
        ]))
        .unwrap();
        assert_eq!(files["td.pal"].data.as_deref().unwrap(), b"root");
        let files = read(zip_of(&[
            ("b/td.pal", b"b", CompressionMethod::Stored),
            ("a/td.pal", b"a", CompressionMethod::Stored),
        ]))
        .unwrap();
        assert_eq!(files["td.pal"].data.as_deref().unwrap(), b"a");
    }

    #[test]
    fn rejects_what_is_not_a_zip() {
        assert!(matches!(
            read(b"not a zip archive at all".to_vec()),
            Err(ArchiveError::NotZip { .. })
        ));
        assert!(matches!(read(Vec::new()), Err(ArchiveError::NotZip { .. })));
    }

    #[test]
    fn a_damaged_entry_is_listed_as_unreadable() {
        // Incompressible contents, so the compressed stream is about as long.
        let mut x: u32 = 1;
        let noise: Vec<u8> = (0..3000)
            .map(|_| {
                x = x.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                (x >> 16) as u8
            })
            .collect();
        let mut bytes = zip_of(&[("td.pal", &noise, CompressionMethod::Deflated)]);
        // Corrupt the middle of the compressed stream (well past the local header).
        for b in &mut bytes[1000..1100] {
            *b ^= 0xA5;
        }
        let files = read(bytes).unwrap();
        assert!(files["td.pal"].data.is_err(), "{:?}", files["td.pal"]);
    }

    #[test]
    fn names_and_depths() {
        assert_eq!(base_name("a/b/c.txt"), "c.txt");
        assert_eq!(base_name("a\\b\\C.TXT"), "C.TXT");
        assert_eq!(depth("x.txt"), 0);
        assert_eq!(depth("a/b/x.txt"), 2);
    }
}
