use thiserror::Error;

/// Result alias for format parsing.
pub type Result<T> = std::result::Result<T, FormatError>;

/// Why a byte slice could not be parsed as the requested format.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FormatError {
    /// The input length does not match the fixed size of the format.
    #[error("{format}: expected {expected} bytes, found {found}")]
    BadSize {
        format: &'static str,
        expected: usize,
        found: usize,
    },
    /// The input ended before a required field.
    #[error("{format}: unexpected end of data at offset {offset} (needed {needed} more bytes)")]
    Truncated {
        format: &'static str,
        offset: usize,
        needed: usize,
    },
    /// A magic number or signature did not match.
    #[error("{format}: bad signature")]
    BadMagic { format: &'static str },
    /// A field holds a value outside its valid range.
    #[error("{format}: invalid value at offset {offset}: {detail}")]
    Invalid {
        format: &'static str,
        offset: usize,
        detail: String,
    },
}

impl FormatError {
    /// Checks that `data` has exactly `expected` bytes.
    pub fn check_size(format: &'static str, data: &[u8], expected: usize) -> Result<()> {
        if data.len() == expected {
            Ok(())
        } else {
            Err(FormatError::BadSize {
                format,
                expected,
                found: data.len(),
            })
        }
    }
}
