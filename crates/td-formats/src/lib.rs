//! Pure, panic-free parsers for the original Traffic Department 2192 file formats.
//!
//! Every parser takes a byte slice and returns typed data or a [`FormatError`]. Parsers never
//! panic on malformed input, perform no I/O and know nothing about rendering, audio or
//! simulation. Format knowledge lives in `docs/re/FORMATS.md`; each module's doc comment
//! summarises the layout it implements and the confidence level of that knowledge.

mod error;
mod reader;

pub mod cmf;
pub mod dialogue;
pub mod font;
pub mod map;
pub mod mission;
pub mod mz;
pub mod palette;
pub mod picture;
pub mod portraits;
pub mod screen;
pub mod sprites;
pub mod tileset;
pub mod voc;

pub use error::{FormatError, Result};
pub use reader::ByteReader;
