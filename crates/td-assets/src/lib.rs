//! Locating, validating and loading an original Traffic Department 2192 installation.
//!
//! * [`locator`]: data-directory discovery and DOS 8.3, case-insensitive file resolution
//!   (spec: `docs/spec/assets/locator.md`). A data directory is a folder or the original ZIP.
//! * [`archive`]: reading the original distribution ZIP in place (M13-02).
//! * [`manifest`]: the SHA-256 manifest of the 218 original files, generated from
//!   `docs/re/FILE_INVENTORY.md` by `tdtool gen-manifest`.
//! * [`validate`]: checks a directory against the manifest and works out which episodes are
//!   playable.
//! * [`import`]: copies the game files of an archive into the per-user data directory, with
//!   the player's consent (M13-02).
//!
//! The original directory or archive is only ever read (MDG P10, §8.8).

pub mod archive;
pub mod exe_res;
pub mod import;
pub mod locator;
pub mod manifest;
pub mod validate;

pub use exe_res::{ExeResources, MapStatus, ResourceValue};
pub use import::{ImportError, ImportSummary, extract, import_dir};
pub use locator::{AssetError, DataDir, DataDirKind, DataDirSource, discover_data_dir, dos83};
pub use manifest::{Category, Episode, ManifestEntry, Usage};
pub use validate::{FileCheck, FileCounts, FileStatus, ValidationReport, validate};
