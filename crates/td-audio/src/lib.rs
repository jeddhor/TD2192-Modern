//! Audio for Traffic Department 2192: Creative Voice File decoding (PCM and Creative ADPCM),
//! Creative Music File sequencing on an emulated OPL2, and mixing to a host sample rate.
//!
//! This crate consumes [`AudioCommand`]s and produces sample frames; it never touches devices or
//! simulation state. Implemented from `docs/spec/audio/`.

pub mod adpcm;
pub mod cmf_player;
pub mod mixer;
pub mod sound;

pub use cmf_player::CmfPlayer;
pub use mixer::{AudioCommand, Mixer, SfxPolicy};
pub use sound::{Pcm8, SoundError, decode_voc};
