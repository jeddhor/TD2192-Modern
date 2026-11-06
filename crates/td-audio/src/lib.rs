//! Audio for Traffic Department 2192: Creative Voice File decoding (PCM and Creative ADPCM),
//! Creative Music File playback on an emulated OPL2 as the WORX driver does it, the music cue
//! table, the sound-effect policy, and mixing to a host sample rate.
//!
//! This crate consumes [`AudioCommand`]s and produces sample frames; it never touches devices or
//! simulation state. Implemented from `docs/spec/audio/`.

pub mod adpcm;
pub mod cmf_player;
pub mod mixer;
pub mod music_cues;
pub mod sfx;
pub mod sound;
pub mod worx;

pub use cmf_player::{CmfDriver, CmfPlayer, OplSink, OplWrite};
pub use mixer::{AudioCommand, Mixer, SfxPolicy};
pub use music_cues::{CueRestart, CueTable, MusicCues};
pub use sfx::DigitalVoice;
pub use sound::{DmaStream, Pcm8, SoundError, decode_voc, dsp_rate};
