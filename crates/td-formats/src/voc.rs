//! Creative Voice File container.
//!
//! Spec: `docs/spec/audio/voc-and-adpcm.md` §2 (rev 1). Fidelity: T0.
//! Only the block types used by the original files are interpreted; repeat, extended and
//! new-format blocks are reported as unsupported.

use crate::{ByteReader, FormatError, Result};

const MAGIC: &[u8; 20] = b"Creative Voice File\x1a";

/// Sample encoding of a sound-data block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
    /// 8-bit unsigned PCM.
    Pcm8,
    /// Creative 4-bit ADPCM.
    Adpcm4,
    /// Creative 2.6-bit (3-bit codes) ADPCM.
    Adpcm3,
    /// Creative 2-bit ADPCM.
    Adpcm2,
}

impl Codec {
    fn from_byte(b: u8) -> Option<Self> {
        Some(match b {
            0 => Codec::Pcm8,
            1 => Codec::Adpcm4,
            2 => Codec::Adpcm3,
            3 => Codec::Adpcm2,
            _ => return None,
        })
    }
}

/// One interpreted block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block<'a> {
    /// Type 1: new sound data.
    Sound {
        time_constant: u8,
        codec: Codec,
        data: &'a [u8],
    },
    /// Type 2: continuation of the previous sound data.
    Continue { data: &'a [u8] },
    /// Type 3: silence of `samples` samples.
    Silence { samples: u32, time_constant: u8 },
    /// Types 4 and 5: marker or text, ignored by playback.
    Ignored { kind: u8 },
}

/// A parsed voice file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Voc<'a> {
    pub version: u16,
    pub blocks: Vec<Block<'a>>,
}

/// Sample rate in Hz for a time constant: `1_000_000 / (256 - tc)`.
pub fn sample_rate(time_constant: u8) -> u32 {
    1_000_000 / (256 - time_constant as u32)
}

impl<'a> Voc<'a> {
    /// Parses the container. Blocks borrow from `data`.
    pub fn parse(data: &'a [u8]) -> Result<Self> {
        let mut r = ByteReader::new("voc", data);
        if r.bytes(20)? != MAGIC {
            return Err(FormatError::BadMagic { format: "voc" });
        }
        let first = r.u16_le()? as usize;
        let version = r.u16_le()?;
        r.seek(first)?;
        let mut blocks = Vec::new();
        loop {
            if r.remaining() == 0 {
                break; // tolerate a missing terminator
            }
            let kind = r.u8()?;
            if kind == 0 {
                break;
            }
            let len = r.u24_le()? as usize;
            let body = r.bytes(len)?;
            let mut b = ByteReader::new("voc block", body);
            blocks.push(match kind {
                1 => {
                    let time_constant = b.u8()?;
                    let codec_byte = b.u8()?;
                    let codec = Codec::from_byte(codec_byte)
                        .ok_or_else(|| b.invalid(format!("unknown codec {codec_byte}")))?;
                    Block::Sound {
                        time_constant,
                        codec,
                        data: &body[2..],
                    }
                }
                2 => Block::Continue { data: body },
                3 => {
                    let samples = b.u16_le()? as u32 + 1;
                    let time_constant = b.u8()?;
                    Block::Silence {
                        samples,
                        time_constant,
                    }
                }
                4 | 5 => Block::Ignored { kind },
                other => {
                    return Err(r.invalid(format!("unsupported block type {other}")));
                }
            });
        }
        Ok(Self { version, blocks })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(blocks: &[u8]) -> Vec<u8> {
        let mut v = MAGIC.to_vec();
        v.extend_from_slice(&26u16.to_le_bytes());
        v.extend_from_slice(&0x010Au16.to_le_bytes());
        v.extend_from_slice(&0x1129u16.to_le_bytes());
        v.extend_from_slice(blocks);
        v
    }

    #[test]
    fn parses_blocks() {
        let data = file(&[
            1, 4, 0, 0, 0x83, 1, 0x80, 0x70, // sound: tc 0x83, 4-bit, 2 data bytes
            3, 3, 0, 0, 9, 0, 0x83, // 10 samples of silence
            2, 1, 0, 0, 0x11, // continuation
            0,
        ]);
        let v = Voc::parse(&data).unwrap();
        assert_eq!(v.version, 0x010A);
        assert_eq!(
            v.blocks,
            vec![
                Block::Sound {
                    time_constant: 0x83,
                    codec: Codec::Adpcm4,
                    data: &[0x80, 0x70]
                },
                Block::Silence {
                    samples: 10,
                    time_constant: 0x83
                },
                Block::Continue { data: &[0x11] },
            ]
        );
        assert_eq!(sample_rate(0x83), 8000);
    }

    #[test]
    fn rejects_bad_input() {
        assert!(Voc::parse(b"not a voice file....").is_err());
        assert!(
            Voc::parse(&file(&[1, 9, 0, 0, 0x83])).is_err(),
            "truncated block"
        );
        assert!(
            Voc::parse(&file(&[1, 2, 0, 0, 0x83, 7])).is_err(),
            "unknown codec"
        );
        assert!(
            Voc::parse(&file(&[6, 2, 0, 0, 1, 0])).is_err(),
            "repeat unsupported"
        );
    }
}
