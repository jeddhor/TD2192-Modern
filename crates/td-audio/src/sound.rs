//! Decoded sound effects: Creative Voice Files to unsigned 8-bit PCM at the rate the Sound
//! Blaster DSP plays them.
//!
//! Spec: `docs/spec/audio/voc-and-adpcm.md` (rev 3): §2 (container), §2.2 (the DSP's
//! time-constant clamp per codec, U-402), §3 (ADPCM, [`crate::adpcm`]).

use td_formats::voc::{Block, Codec, Voc};
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

/// What WORX streams to the DSP for one sound (voc §2.1): the bytes of its sound data and how
/// many samples the DSP makes of each byte. WORX's "playing" status follows the DMA position
/// in these bytes, not the samples ([`crate::worx`], voc §2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DmaStream {
    /// D: the data bytes of the sound blocks (the ADPCM reference byte included).
    pub bytes: u32,
    /// Samples per byte: 1 (PCM), 2 (4-bit), 3 (2.6-bit), 4 (2-bit ADPCM).
    pub codes_per_byte: u8,
}

/// Mono unsigned 8-bit PCM at a fixed rate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pcm8 {
    /// Playback rate in Hz (voc §2.2).
    pub rate: u32,
    pub samples: Vec<u8>,
    /// The bytes behind the samples, as the DSP receives them.
    pub dma: DmaStream,
}

impl Pcm8 {
    /// Plain PCM: one DMA byte per sample.
    #[must_use]
    pub fn new(rate: u32, samples: Vec<u8>) -> Self {
        let bytes = u32::try_from(samples.len()).unwrap_or(u32::MAX);
        Self {
            rate,
            samples,
            dma: DmaStream {
                bytes,
                codes_per_byte: 1,
            },
        }
    }

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

/// The largest time constant the DSP accepts for `codec`'s play command (voc §2.2: the
/// oracle's SB16 with its default sample-rate limits, Creative's documented maxima): 8-bit
/// PCM 234 (45,454 Hz), 4-bit ADPCM 172 (11,904 Hz), 2.6-bit 179 (12,987 Hz), 2-bit 165
/// (10,989 Hz).
#[must_use]
pub const fn time_constant_limit(codec: Codec) -> u8 {
    match codec {
        Codec::Pcm8 => 234,
        Codec::Adpcm4 => 172,
        Codec::Adpcm3 => 179,
        Codec::Adpcm2 => 165,
    }
}

/// The rate the DSP plays a block at: 1,000,000 div (256 − min(tc, limit)) Hz (voc §2.2,
/// U-402). Only `ric3.voc` (4-bit, tc 173) is above its limit: 11,904 Hz, not the header's
/// 12,048.
#[must_use]
pub fn dsp_rate(time_constant: u8, codec: Codec) -> u32 {
    let tc = time_constant.min(time_constant_limit(codec));
    1_000_000 / (256 - u32::from(tc))
}

/// Decodes a voice file. The rate of the first sound block applies to the whole result (the
/// original files hold a single sound block).
pub fn decode_voc(bytes: &[u8]) -> Result<Pcm8, SoundError> {
    let voc = Voc::parse(bytes)?;
    let mut samples = Vec::new();
    let mut rate = None;
    let mut current: Option<(Codec, AdpcmState)> = None;
    let mut dma_bytes = 0usize;
    let mut codes_per_byte = None;
    for block in &voc.blocks {
        match *block {
            Block::Sound {
                time_constant,
                codec,
                data,
            } => {
                rate.get_or_insert(dsp_rate(time_constant, codec));
                codes_per_byte.get_or_insert(width(codec).map_or(1, Width::codes_per_byte));
                dma_bytes += data.len();
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
                dma_bytes += data.len();
                match width(*codec) {
                    None => samples.extend_from_slice(data),
                    Some(w) => adpcm::decode_continuation(data, w, state, &mut samples),
                }
            }
            Block::Silence {
                samples: n,
                time_constant,
            } => {
                rate.get_or_insert(dsp_rate(time_constant, Codec::Pcm8));
                samples.extend(std::iter::repeat_n(0x80u8, n as usize));
            }
            Block::Ignored { .. } => {}
        }
    }
    let rate = rate.ok_or(SoundError::Empty)?;
    Ok(Pcm8 {
        rate,
        samples,
        dma: DmaStream {
            bytes: u32::try_from(dma_bytes).unwrap_or(u32::MAX),
            codes_per_byte: codes_per_byte.unwrap_or(1),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// voc §2.2: the clamp per codec; `ric3.voc` plays at 11,904 Hz.
    #[test]
    fn dsp_rates_clamp_per_codec() {
        assert_eq!(dsp_rate(173, Codec::Adpcm4), 11_904);
        assert_eq!(dsp_rate(173, Codec::Pcm8), 12_048);
        assert_eq!(dsp_rate(131, Codec::Adpcm4), 8_000);
        assert_eq!(dsp_rate(255, Codec::Adpcm3), 12_987);
        assert_eq!(dsp_rate(255, Codec::Adpcm2), 10_989);
        assert_eq!(dsp_rate(255, Codec::Pcm8), 45_454);
        assert_eq!(dsp_rate(6, Codec::Pcm8), 4_000);
    }

    fn voc(tc: u8, codec: u8, data: &[u8]) -> Vec<u8> {
        let mut v = b"Creative Voice File\x1a".to_vec();
        v.extend_from_slice(&[26, 0, 0x0A, 0x01, 0x29, 0x11]);
        let len = data.len() + 2;
        v.extend_from_slice(&[1, len as u8, (len >> 8) as u8, 0, tc, codec]);
        v.extend_from_slice(data);
        v.push(0);
        v
    }

    #[test]
    fn decodes_adpcm_without_the_reference_byte() {
        let p = decode_voc(&voc(173, 1, &[0x80, 0x70])).unwrap();
        assert_eq!(p.rate, 11_904);
        assert_eq!(p.samples, [0x87, 0x88]);
        assert_eq!(
            p.dma,
            DmaStream {
                bytes: 2,
                codes_per_byte: 2
            }
        );
        let p = decode_voc(&voc(6, 0, &[1, 2, 3])).unwrap();
        assert_eq!((p.rate, p.samples.as_slice()), (4_000, &[1u8, 2, 3][..]));
        assert_eq!(p, Pcm8::new(4_000, vec![1, 2, 3]));
    }
}
