//! Input recordings with periodic state-hash checkpoints (L4 replay tests, MDG §10.2).
//!
//! A replay is *our* data (inputs and hashes), never original game content, so replay files
//! may be committed under `tests/replays/`.
//!
//! Layout (little-endian): magic `TDR1`, `u16` version, `u8` episode, `u8` mission, `u32` RNG
//! seed at start, `u32` tick count, `u32` checkpoint interval, `u32` checkpoint count, then one
//! `u128` input word per tick ([`crate::input::TickControls`]), then the checkpoint hashes
//! (`u64`, one after every `interval` ticks). Version 2 widened the input word from 32 to 64
//! bits, version 3 to 128 bits for the cheat-chord letters (D9). Version 2 files still load:
//! their 64-bit words are the low halves of the same version-3 words.

use thiserror::Error;

const MAGIC: &[u8; 4] = b"TDR1";
const VERSION: u16 = 3;
/// The older version that still loads (64-bit input words).
const VERSION_64: u16 = 2;

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
    pub inputs: Vec<u128>,
    /// Ticks between checkpoints (0 = no checkpoints).
    pub interval: u32,
    /// State hash after every `interval` ticks.
    pub checkpoints: Vec<u64>,
}

impl Replay {
    /// Serialises to bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(24 + self.inputs.len() * 16 + self.checkpoints.len() * 8);
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
        let word = match version {
            VERSION => 16,
            VERSION_64 => 8,
            _ => return Err(ReplayError::Version(version)),
        };
        let episode = take(6, 1)?[0];
        let mission = take(7, 1)?[0];
        let seed = u32_at(8)?;
        let ticks = u32_at(12)? as usize;
        let interval = u32_at(16)?;
        let n_checks = u32_at(20)? as usize;
        let mut at = 24;
        // Reserve no more than the input can hold (BUG-17): the counts are only claims.
        let rest = b.len().saturating_sub(at);
        let mut inputs = Vec::with_capacity(ticks.min(rest / word));
        for _ in 0..ticks {
            let s = take(at, word)?;
            let mut w = [0u8; 16];
            w[..word].copy_from_slice(s);
            inputs.push(u128::from_le_bytes(w));
            at += word;
        }
        let mut checkpoints = Vec::with_capacity(n_checks.min(b.len().saturating_sub(at) / 8));
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

/// Advances `flight` by one replay tick: the tick runs with input `word`
/// ([`crate::input::TickControls::from_word`]). A modal screen the tick opens (comm viewer,
/// pause, flight menu, tactical map) is closed at once with no input, as in the oracle captures,
/// where no key is held while a screen is open; [`crate::Flight::resume`] then clears the key
/// table and finishes the tick.
pub fn step(
    flight: &mut crate::Flight,
    world: &crate::World,
    word: u128,
) -> crate::flight::TickResult {
    use crate::flight::TickResult;
    let c = crate::input::TickControls::from_word(word);
    let mut events = Vec::new();
    let mut r = flight.tick(world, &c, &mut events);
    while let TickResult::Modal(_) = r {
        r = flight.resume(world, &crate::input::TickControls::default(), &mut events);
    }
    r
}

/// Re-runs `replay` through `step` (which advances one tick with the given input word and
/// returns the state hash after it) and reports the first checkpoint that differs.
pub fn verify(replay: &Replay, mut step: impl FnMut(u128) -> u64) -> Result<(), (usize, u64, u64)> {
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

    /// Version-2 files (64-bit words) load as the same words zero-extended.
    #[test]
    fn huge_declared_counts_are_truncation_errors() {
        // BUG-17 (M12-06 fuzzing): the counts are claims; the reservations follow the input.
        let mut b = sample().to_bytes();
        b[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(Replay::from_bytes(&b), Err(ReplayError::Truncated));
        let mut b = sample().to_bytes();
        b[20..24].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(Replay::from_bytes(&b), Err(ReplayError::Truncated));
    }

    #[test]
    fn version_2_files_still_load() {
        let mut r = sample();
        r.inputs = vec![0x0000_0001_0000_0001, u128::from(u64::MAX)];
        let mut v2 = r.to_bytes();
        // Rewrite as version 2: the version field and 8-byte words.
        v2[4..6].copy_from_slice(&2u16.to_le_bytes());
        let mut body = Vec::new();
        for w in &r.inputs {
            body.extend_from_slice(&(*w as u64).to_le_bytes());
        }
        v2.splice(24..24 + 2 * 16, body);
        assert_eq!(Replay::from_bytes(&v2).unwrap(), r);
        let mut v1 = r.to_bytes();
        v1[4..6].copy_from_slice(&1u16.to_le_bytes());
        assert_eq!(Replay::from_bytes(&v1), Err(ReplayError::Version(1)));
        // A chord letter survives a version-3 round trip.
        r.inputs = vec![1u128 << 64 | 1u128 << 96];
        assert_eq!(Replay::from_bytes(&r.to_bytes()).unwrap(), r);
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
