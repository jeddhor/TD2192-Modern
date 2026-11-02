//! Decoded sound effects: Creative Voice Files to unsigned 8-bit PCM.
//!
//! Spec: `docs/spec/audio/voc-and-adpcm.md` (rev 1).

use td_formats::voc::{Block, Codec, Voc, sample_rate};
use thiserror::Error;

use crate::adpcm::{self, AdpcmState, Width};

/// Errors decoding a sound effect.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SoundError {
    #[error(transparent)]
    Format(#[from] td_formats::FormatError),
    #[error("voice file has no sound data")]
    Empty,
    #[error("continuation block without a preceding sound block")]
    OrphanContinuation,
}

/// Mono unsigned 8-bit PCM at a fixed rate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pcm8 {
    pub rate: u32,
    pub samples: Vec<u8>,
}

impl Pcm8 {
    /// Duration in seconds.
    pub fn seconds(&self) -> f64 {
        self.samples.len() as f64 / f64::from(self.rate.max(1))
    }
}

fn width(codec: Codec) -> Option<Width> {
    match codec {
        Codec::Pcm8 => None,
        Codec::Adpcm4 => Some(Width::Four),
        Codec::Adpcm3 => Some(Width::Three),
        Codec::Adpcm2 => Some(Width::Two),
    }
}

/// Decodes a voice file. The rate of the first sound block applies to the whole result (the
/// original files hold a single sound block).
pub fn decode_voc(bytes: &[u8]) -> Result<Pcm8, SoundError> {
    let voc = Voc::parse(bytes)?;
    let mut samples = Vec::new();
    let mut rate = None;
    let mut current: Option<(Codec, AdpcmState)> = None;
    for block in &voc.blocks {
        match *block {
            Block::Sound {
                time_constant,
                codec,
                data,
            } => {
                rate.get_or_insert(sample_rate(time_constant));
                let state = match width(codec) {
                    None => {
                        samples.extend_from_slice(data);
                        AdpcmState::default()
                    }
                    Some(w) => adpcm::decode_block_with_reference(data, w, &mut samples),
                };
                current = Some((codec, state));
            }
            Block::Continue { data } => {
                let (codec, state) = current.as_mut().ok_or(SoundError::OrphanContinuation)?;
                match width(*codec) {
                    None => samples.extend_from_slice(data),
                    Some(w) => adpcm::decode_continuation(data, w, state, &mut samples),
                }
            }
            Block::Silence {
                samples: n,
                time_constant,
            } => {
                rate.get_or_insert(sample_rate(time_constant));
                samples.extend(std::iter::repeat_n(0x80u8, n as usize));
            }
            Block::Ignored { .. } => {}
        }
    }
    let rate = rate.ok_or(SoundError::Empty)?;
    Ok(Pcm8 { rate, samples })
}
