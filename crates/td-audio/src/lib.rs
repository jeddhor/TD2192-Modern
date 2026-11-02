//! Audio for Traffic Department 2192: Creative Voice File decoding (PCM and Creative ADPCM),
//! Creative Music File sequencing on an emulated OPL2, and mixing to a host sample rate.
//!
//! This crate consumes audio commands and produces sample frames; it never touches devices or
//! simulation state. Implemented from `docs/spec/audio/`.
