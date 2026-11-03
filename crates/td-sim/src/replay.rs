//! Input recordings with periodic state-hash checkpoints (L4 replay tests, MDG §10.2).
//!
//! A replay is *our* data (inputs and hashes), never original game content, so replay files
//! may be committed under `tests/replays/`.
//!
//! Layout (little-endian): magic `TDR1`, `u16` version, `u8` episode, `u8` mission, `u32` RNG
//! seed at start, `u32` tick count, `u32` checkpoint interval, `u32` checkpoint count, then one
//! `u64` input word per tick ([`crate::input::TickControls`]), then the checkpoint hashes (`u64`,
//! one after every `interval` ticks). Version 2 widened the input word from 32 to 64 bits.

use thiserror::Error;

const MAGIC: &[u8; 4] = b"TDR1";
const VERSION: u16 = 2;

/// Errors reading a replay.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReplayError {
    #[error("not a replay file")]
    BadMagic,
    #[error("unsupported replay version {0}")]
    Version(u16),
    #[error("replay file is truncated")]
    Truncated,
}

/// A recorded run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replay {
    pub episode: u8,
    pub mission: u8,
    pub seed: u32,
    /// One input word per tick ([`crate::input::TickControls::to_word`]).
    pub inputs: Vec<u64>,
    /// Ticks between checkpoints (0 = no checkpoints).
    pub interval: u32,
    /// State hash after every `interval` ticks.
    pub checkpoints: Vec<u64>,
}

impl Replay {
    /// Serialises to bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(24 + self.inputs.len() * 8 + self.checkpoints.len() * 8);
        v.extend_from_slice(MAGIC);
        v.extend_from_slice(&VERSION.to_le_bytes());
        v.push(self.episode);
        v.push(self.mission);
        v.extend_from_slice(&self.seed.to_le_bytes());
        v.extend_from_slice(&(self.inputs.len() as u32).to_le_bytes());
        v.extend_from_slice(&self.interval.to_le_bytes());
        v.extend_from_slice(&(self.checkpoints.len() as u32).to_le_bytes());
        for i in &self.inputs {
            v.extend_from_slice(&i.to_le_bytes());
        }
        for c in &self.checkpoints {
            v.extend_from_slice(&c.to_le_bytes());
        }
        v
    }

    /// Parses bytes produced by [`Self::to_bytes`].
    pub fn from_bytes(b: &[u8]) -> Result<Self, ReplayError> {
        let take = |at: usize, n: usize| b.get(at..at + n).ok_or(ReplayError::Truncated);
        let u32_at = |at: usize| -> Result<u32, ReplayError> {
            let s = take(at, 4)?;
            Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        };
        if take(0, 4)? != MAGIC {
            return Err(ReplayError::BadMagic);
        }
        let version = u16::from_le_bytes([take(4, 2)?[0], take(4, 2)?[1]]);
        if version != VERSION {
            return Err(ReplayError::Version(version));
        }
        let episode = take(6, 1)?[0];
        let mission = take(7, 1)?[0];
        let seed = u32_at(8)?;
        let ticks = u32_at(12)? as usize;
        let interval = u32_at(16)?;
        let n_checks = u32_at(20)? as usize;
        let mut at = 24;
        let mut inputs = Vec::with_capacity(ticks.min(1 << 24));
        for _ in 0..ticks {
            let s = take(at, 8)?;
            inputs.push(u64::from_le_bytes(s.try_into().expect("8 bytes")));
            at += 8;
        }
        let mut checkpoints = Vec::with_capacity(n_checks.min(1 << 20));
        for _ in 0..n_checks {
            let s = take(at, 8)?;
            checkpoints.push(u64::from_le_bytes(s.try_into().expect("8 bytes")));
            at += 8;
        }
        Ok(Self {
            episode,
            mission,
            seed,
            inputs,
            interval,
            checkpoints,
        })
    }
}

/// Re-runs `replay` through `step` (which advances one tick with the given input word and
/// returns the state hash after it) and reports the first checkpoint that differs.
pub fn verify(replay: &Replay, mut step: impl FnMut(u64) -> u64) -> Result<(), (usize, u64, u64)> {
    let mut check = 0;
    for (tick, &input) in replay.inputs.iter().enumerate() {
        let hash = step(input);
        if replay.interval > 0 && (tick as u32 + 1).is_multiple_of(replay.interval) {
            if let Some(&expected) = replay.checkpoints.get(check)
                && expected != hash
            {
                return Err((tick + 1, expected, hash));
            }
            check += 1;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Replay {
        Replay {
            episode: 1,
            mission: 3,
            seed: 0,
            inputs: vec![0, 1, 3, 2],
            interval: 2,
            checkpoints: vec![10, 20],
        }
    }

    #[test]
    fn round_trips() {
        let r = sample();
        assert_eq!(Replay::from_bytes(&r.to_bytes()).unwrap(), r);
        let bytes = r.to_bytes();
        assert_eq!(
            Replay::from_bytes(&bytes[..10]),
            Err(ReplayError::Truncated)
        );
        assert_eq!(Replay::from_bytes(b"XXXX"), Err(ReplayError::BadMagic));
    }

    #[test]
    fn verify_reports_first_divergence() {
        let r = sample();
        let mut t = 0u64;
        assert_eq!(
            verify(&r, |_| {
                t += 1;
                t * 5
            }),
            Ok(())
        );
        let mut t = 0u64;
        assert_eq!(
            verify(&r, |_| {
                t += 1;
                if t == 4 { 99 } else { t * 5 }
            }),
            Err((4, 20, 99))
        );
    }
}
