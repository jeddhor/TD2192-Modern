//! Test helpers shared by the workspace.
//!
//! * [`data_dir`] / [`require_data!`]: locate the user's original installation through the
//!   `TD2192_DATA` environment variable. Tests that need data **skip with a visible message**
//!   when it is not set, so `cargo test --workspace` passes with and without data (MDG §10.3).
//! * [`sha256_hex`] / [`assert_sha256!`]: golden values are hashes of decoded output, never
//!   decoded assets (MDG §10.1).

use std::path::PathBuf;

use sha2::{Digest, Sha256};

/// Environment variable naming the original game directory.
pub const DATA_ENV: &str = "TD2192_DATA";

/// The original game directory from `TD2192_DATA`, if set and present on disk.
pub fn data_dir() -> Option<PathBuf> {
    let path = PathBuf::from(std::env::var_os(DATA_ENV)?);
    path.is_dir().then_some(path)
}

/// Returns the data directory, or prints a skip notice and returns from the calling test.
#[macro_export]
macro_rules! require_data {
    () => {
        match $crate::data_dir() {
            Some(dir) => dir,
            None => {
                eprintln!(
                    "SKIPPED: {} not set (data-gated test {})",
                    $crate::DATA_ENV,
                    module_path!()
                );
                return;
            }
        }
    };
}

/// Lower-case hex SHA-256 of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// Asserts that the SHA-256 of `bytes` equals the expected lower-case hex string.
#[macro_export]
macro_rules! assert_sha256 {
    ($bytes:expr, $expected:expr $(,)?) => {{
        let actual = $crate::sha256_hex($bytes);
        assert_eq!(
            actual, $expected,
            "SHA-256 mismatch; inspect the output before updating a golden value (ADG §10.3)"
        );
    }};
    ($bytes:expr, $expected:expr, $($msg:tt)+) => {{
        let actual = $crate::sha256_hex($bytes);
        assert_eq!(actual, $expected, $($msg)+);
    }};
}

#[cfg(test)]
mod tests {
    #[test]
    fn hashes_known_vector() {
        assert_sha256!(
            b"abc",
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
