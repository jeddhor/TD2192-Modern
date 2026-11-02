//! Locating, validating and loading an original Traffic Department 2192 installation.
//!
//! * [`locator`]: data-directory discovery and DOS 8.3, case-insensitive file resolution
//!   (spec: `docs/spec/assets/locator.md`).
//! * [`manifest`]: the SHA-256 manifest of the 218 original files, generated from
//!   `docs/re/FILE_INVENTORY.md` by `tdtool gen-manifest`.
//! * [`validate`]: checks a directory against the manifest and works out which episodes are
//!   playable.
//!
//! The original directory is only ever read (MDG P10, §8.8).

pub mod locator;
pub mod manifest;
pub mod validate;

pub use locator::{AssetError, DataDir, DataDirSource, discover_data_dir, dos83};
pub use manifest::{Category, Episode, ManifestEntry, Usage};
pub use validate::{FileCheck, FileStatus, ValidationReport, validate};
