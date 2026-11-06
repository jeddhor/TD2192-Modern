//! Mixing music and sound effects to interleaved stereo `f32` at the device rate.
//!
//! Music: one [`CmfPlayer`] for the whole session, as the original keeps one WORX driver (its
//! state carries over between songs, cmf §4.10). A new song stops the current one first and
//! always starts from the top (music-cues §4.1).
//!
//! Sound effects: `docs/spec/audio/sfx-policy.md` rev 4. [`SfxPolicy::Classic`] is the original
//! single digital voice: a new effect stops the playing one (WORX function 06h);
//! [`SfxPolicy::Polyphonic`] is the Enhanced option. Both apply the distance cut-off and the
//! SB Pro left/right levels (§4.3). Which effects may start (the priority pre-emption of §4.2)
//! is the game's decision, made with [`crate::sfx::DigitalVoice`] on the game's clock, because
//! the game also reads WORX's playing status (voc-and-adpcm rev 3 §2.3); the mixer plays what
//! it is sent.
//!
//! Fidelity: T2. Resampling is linear interpolation. Levels and balance are tuned to the oracle's
//! WAV captures (VE-3, `docs/parity/M3-audio.md` rows 16-17): a mixer level n gives
//! 4 dB × (15 − n) of attenuation ([`sb_level_gain`]), and the FM music plays at
//! [`MUSIC_GAIN`] relative to an effect at level 15. With equal music and effects volumes this is
//! the original's balance.

use std::sync::Arc;

use td_formats::cmf::Cmf;

use crate::cmf_player::CmfPlayer;
use crate::sfx::{self, TilePos};
use crate::sound::Pcm8;

/// How many sound effects may play at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SfxPolicy {
    /// The original: one digital voice. A new effect replaces the playing one; the game decides
    /// which effects start (sfx-policy §4.2, [`crate::sfx::DigitalVoice`]).
    Classic,
    /// Up to `n` simultaneous voices (Enhanced profile); the oldest is dropped when full. Effects
    /// out of the original's earshot still do not play.
    Polyphonic(usize),
}

/// Commands from the game to the audio system.
#[derive(Debug, Clone)]
pub enum AudioCommand {
    /// Sets the loop flag, stops the current song and starts `song` from the top.
    PlayMusic {
        song: Arc<Cmf>,
        looping: bool,
    },
    /// Stops the song (key-offs; notes release).
    StopMusic,
    /// Sets the music loop flag without touching the song (WORX function 1Dh).
    SetMusicLoop(bool),
    /// A new program's music driver takes over (the intro hands over to the game): the driver
    /// state returns to start-up, the chip keeps playing. See [`CmfPlayer::restart_driver`].
    RestartMusicDriver,
    /// An unpositioned sound with the SB Pro mixer voice volume set to `level` (0..15) on both
    /// channels first (`WORX_SetVoiceVolume(level, level)`, the intro's effects: 10, 12, 13 of
    /// 15, intro §3.6). Under [`SfxPolicy::Classic`] it replaces the playing sound.
    PlaySfx {
        sound: Arc<Pcm8>,
        level: u8,
    },
    /// A game sound effect the game's arbiter let start: `Snd_PlayEffect(id, source, view)` on
    /// a Sound Blaster (sfx-policy §4.2–§4.3). `sound` is the effect's sample
    /// ([`sfx::effect_voc`]); `None` for an id without one. `source` is the sound's tile,
    /// `None` for the player's own sounds (SrcX = FFh); `view` is the view's top-left tile.
    /// Effects out of earshot (priority below 0, or OB-009) never play.
    PlayEffect {
        id: u8,
        sound: Option<Arc<Pcm8>>,
        source: Option<TilePos>,
        view: TilePos,
    },
    StopSfx,
    SetVolumes {
        music: f32,
        sfx: f32,
    },
}

/// Linear-interpolating reader over a stream of mono samples.
#[derive(Debug, Clone, Default)]
struct Interp {
    prev: f32,
    cur: f32,
    frac: f64,
    primed: bool,
}

impl Interp {
    /// Produces the next output sample, pulling input with `next` (None = end of input).
    fn next(&mut self, step: f64, mut next: impl FnMut() -> Option<f32>) -> Option<f32> {
        if !self.primed {
            self.prev = next()?;
            self.cur = next().unwrap_or(self.prev);
            self.primed = true;
        }
        while self.frac >= 1.0 {
            self.prev = self.cur;
            self.cur = next()?;
            self.frac -= 1.0;
        }
        let out = self.prev + (self.cur - self.prev) * self.frac as f32;
        self.frac += step;
        Some(out)
    }
}

#[derive(Debug)]
struct Music {
    player: CmfPlayer,
    buf: Vec<i32>,
    pos: usize,
    interp: Interp,
}

#[derive(Debug)]
struct Voice {
    sound: Arc<Pcm8>,
    pos: usize,
    interp: Interp,
    /// Left and right gain.
    gain: (f32, f32),
}

/// The audio mixer.
#[derive(Debug)]
pub struct Mixer {
    rate: u32,
    policy: SfxPolicy,
    /// Created by the first song; kept for the session afterwards.
    music: Option<Music>,
    voices: Vec<Voice>,
    music_volume: f32,
    sfx_volume: f32,
}

/// Linear gain of a Sound Blaster mixer voice level (0..15, the nibble the game writes to
/// register 04h), relative to level 15.
///
/// Measured (VE-3, `docs/parity/M3-audio.md` row 16): the oracle (DOSBox-X SB16, preset P1)
/// plays a full-scale sample at WAV amplitude 0.0997 at level 10, 0.158 at 11, 0.238 at 12 and
/// 0.364 at 13 (C-0071, C-0072). Model: 4 dB per level with level 15 at 0 dB, within 0.8 dB of
/// all four; level 0 mutes. Levels 0..9, 14 and 15 are extrapolated.
pub fn sb_level_gain(level: u8) -> f32 {
    if level == 0 {
        return 0.0;
    }
    10f32.powf(-0.2 * f32::from(15 - level.min(15)))
}

/// FM music relative to an effect at mixer level 15: the oracle's music RMS is 0.21 × the raw
/// [`CmfPlayer`] render (0.208 for `intro1.cmf` in C-0070, 0.212 for `menu.cmf` in C-0073; WORX
/// sets the FM volume to 10). Measured by VE-3, `docs/parity/M3-audio.md` row 17.
pub const MUSIC_GAIN: f32 = 0.21;

/// Overall output scale: the loudest level the game uses (13, intro effects) at the default
/// effects volume (0.8) stays under full scale.
pub const OUTPUT_SCALE: f32 = 3.0;

/// Gain of a voice at SB Pro left/right level `level` (0..15, before the game's 12/16 scaling).
fn level_gain(level: u8) -> f32 {
    OUTPUT_SCALE * sb_level_gain(sfx::mixer_level(level))
}

impl Mixer {
    /// A mixer producing `rate` Hz stereo.
    pub fn new(rate: u32, policy: SfxPolicy) -> Self {
        Self {
            rate,
            policy,
            music: None,
            voices: Vec::new(),
            music_volume: 0.8,
            sfx_volume: 0.8,
        }
    }

    /// Output rate in Hz.
    pub fn rate(&self) -> u32 {
        self.rate
    }

    /// Changes the SFX policy (takes effect for new sounds).
    pub fn set_policy(&mut self, policy: SfxPolicy) {
        self.policy = policy;
    }

    /// True while a song is playing.
    pub fn music_playing(&self) -> bool {
        self.music.as_ref().is_some_and(|m| m.player.playing())
    }

    /// Number of sound effects currently playing.
    pub fn active_sfx(&self) -> usize {
        self.voices.len()
    }

    /// Applies a command.
    pub fn command(&mut self, cmd: AudioCommand) {
        match cmd {
            AudioCommand::PlayMusic { song, looping } => match self.music.as_mut() {
                Some(m) => m.player.play(song, looping),
                None => {
                    self.music = Some(Music {
                        player: CmfPlayer::new(song, looping),
                        buf: Vec::new(),
                        pos: 0,
                        interp: Interp::default(),
                    });
                }
            },
            AudioCommand::StopMusic => {
                if let Some(m) = self.music.as_mut() {
                    m.player.stop();
                }
            }
            AudioCommand::SetMusicLoop(looping) => {
                if let Some(m) = self.music.as_mut() {
                    m.player.set_looping(looping);
                }
            }
            AudioCommand::RestartMusicDriver => {
                if let Some(m) = self.music.as_mut() {
                    m.player.restart_driver();
                }
            }
            AudioCommand::PlaySfx { sound, level } => {
                // The level is the nibble written to register 04h itself (no 12/16 scaling).
                let gain = OUTPUT_SCALE * sb_level_gain(level);
                self.start_voice(sound, (gain, gain));
            }
            AudioCommand::PlayEffect {
                id,
                sound,
                source,
                view,
            } => self.play_effect(id, sound, source, view),
            AudioCommand::StopSfx => self.voices.clear(),
            AudioCommand::SetVolumes { music, sfx } => {
                self.music_volume = music.clamp(0.0, 1.0);
                self.sfx_volume = sfx.clamp(0.0, 1.0);
            }
        }
    }

    /// `Snd_PlayEffect` with a Sound Blaster once the game's arbiter let it start
    /// (sfx-policy §4.1–§4.3): the earshot test, the stereo levels, then the voice.
    fn play_effect(
        &mut self,
        id: u8,
        sound: Option<Arc<Pcm8>>,
        source: Option<TilePos>,
        view: TilePos,
    ) {
        // OB-009: a distance the original cannot compute is "too far".
        let Some(dist) = sfx::distance(source, view) else {
            return;
        };
        // 15 tiles or more (90 for id 100, 165 for 200): below the idle priority, never heard.
        if sfx::priority(id, dist) < 0 {
            return;
        }
        // An id without a sample (1) starts nothing (sfx::DigitalVoice::request).
        let Some(sound) = sound else {
            return;
        };
        let (l, r) = sfx::stereo_levels(source, view, dist);
        self.start_voice(sound, (level_gain(l), level_gain(r)));
    }

    /// Starts a voice: under [`SfxPolicy::Classic`] it replaces the playing one (WORX function
    /// 06h stops the playing sound first); under [`SfxPolicy::Polyphonic`] the oldest voice is
    /// dropped when all are busy.
    fn start_voice(&mut self, sound: Arc<Pcm8>, gain: (f32, f32)) {
        match self.policy {
            SfxPolicy::Classic => self.voices.clear(),
            SfxPolicy::Polyphonic(n) => {
                while self.voices.len() >= n.max(1) {
                    self.voices.remove(0);
                }
            }
        }
        self.voices.push(Voice {
            sound,
            pos: 0,
            interp: Interp::default(),
            gain,
        });
    }

    /// Fills `out` (interleaved stereo) with the next `out.len() / 2` frames.
    pub fn render(&mut self, out: &mut [f32]) {
        let rate = f64::from(self.rate);
        for frame in out.chunks_exact_mut(2) {
            let (mut left, mut right) = (0.0f32, 0.0f32);
            if let Some(m) = self.music.as_mut() {
                let step = f64::from(m.player.sample_rate()) / rate;
                let Music {
                    player,
                    buf,
                    pos,
                    interp,
                } = m;
                let v = interp.next(step, || {
                    if *pos >= buf.len() {
                        buf.resize(512, 0);
                        player.render(buf);
                        *pos = 0;
                    }
                    let x = buf[*pos];
                    *pos += 1;
                    Some(x as f32 / 32768.0)
                });
                let s = v.unwrap_or(0.0) * OUTPUT_SCALE * MUSIC_GAIN * self.music_volume;
                left += s;
                right += s;
            }
            self.voices.retain_mut(|v| {
                let step = f64::from(v.sound.rate) / rate;
                let Voice {
                    sound,
                    pos,
                    interp,
                    gain,
                } = v;
                match interp.next(step, || {
                    let x = *sound.samples.get(*pos)?;
                    *pos += 1;
                    Some((f32::from(x) - 128.0) / 128.0)
                }) {
                    Some(x) => {
                        left += x * gain.0 * self.sfx_volume;
                        right += x * gain.1 * self.sfx_volume;
                        true
                    }
                    None => false,
                }
            });
            frame[0] = left.clamp(-1.0, 1.0);
            frame[1] = right.clamp(-1.0, 1.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(rate: u32, n: usize) -> Arc<Pcm8> {
        Arc::new(Pcm8::new(
            rate,
            (0..n).map(|i| if i % 2 == 0 { 200 } else { 56 }).collect(),
        ))
    }

    fn sfx(sound: Arc<Pcm8>) -> AudioCommand {
        AudioCommand::PlaySfx { sound, level: 11 }
    }

    fn effect(id: u8, source: Option<TilePos>) -> AudioCommand {
        AudioCommand::PlayEffect {
            id,
            sound: Some(tone(8000, 800)),
            source,
            view: (10, 10),
        }
    }

    #[test]
    fn classic_policy_keeps_one_voice() {
        let mut m = Mixer::new(48_000, SfxPolicy::Classic);
        m.command(sfx(tone(8000, 800)));
        m.command(sfx(tone(8000, 800)));
        assert_eq!(m.active_sfx(), 1);
        m.set_policy(SfxPolicy::Polyphonic(4));
        m.command(sfx(tone(8000, 800)));
        assert_eq!(m.active_sfx(), 2);
    }

    /// Classic: every effect the game sends replaces the playing one (the game's arbiter has
    /// already decided, sfx::DigitalVoice).
    #[test]
    fn classic_effects_replace_the_playing_one() {
        let mut m = Mixer::new(48_000, SfxPolicy::Classic);
        m.command(effect(100, None));
        let first = Arc::as_ptr(&m.voices[0].sound);
        m.command(effect(9, None));
        assert_ne!(Arc::as_ptr(&m.voices[0].sound), first);
        assert_eq!(m.active_sfx(), 1);
    }

    /// The intro's levels (intro §3.6): `WORX_SetVoiceVolume(v, v)` writes v itself.
    #[test]
    fn play_sfx_uses_its_level() {
        for (level, gain) in [(10u8, 0.0997f32), (12, 0.238), (13, 0.364)] {
            let mut m = Mixer::new(48_000, SfxPolicy::Classic);
            m.command(AudioCommand::PlaySfx {
                sound: tone(8000, 800),
                level,
            });
            let (l, r) = m.voices[0].gain;
            assert_eq!(l, r);
            let db = 20.0 * (l / (OUTPUT_SCALE * gain)).log10();
            assert!(db.abs() < 0.8, "level {level}: {db:+.2} dB");
        }
    }

    #[test]
    fn far_and_overflowing_effects_never_play() {
        for policy in [SfxPolicy::Classic, SfxPolicy::Polyphonic(4)] {
            let mut m = Mixer::new(48_000, policy);
            m.command(effect(9, Some((0, 0)))); // 20 tiles
            m.command(AudioCommand::PlayEffect {
                id: 200,
                sound: Some(tone(8000, 800)),
                source: Some((0, 3)),
                view: (185, 0), // 16-bit square overflows (OB-009)
            });
            assert_eq!(m.active_sfx(), 0, "{policy:?}");
            m.command(effect(9, Some((8, 13)))); // 7 tiles
            assert_eq!(m.active_sfx(), 1, "{policy:?}");
        }
    }

    #[test]
    fn panning_follows_the_source() {
        let mut m = Mixer::new(48_000, SfxPolicy::Classic);
        m.command(effect(9, Some((8, 13)))); // left of centre: L 15, R 5 -> levels 11, 3
        let (l, r) = m.voices[0].gain;
        assert!((l - OUTPUT_SCALE * 0.1585).abs() < 1e-3, "left {l}");
        assert!(
            (r - OUTPUT_SCALE * 10f32.powf(-2.4)).abs() < 1e-4,
            "right {r}"
        );
        let mut buf = vec![0.0f32; 2 * 1_000];
        m.render(&mut buf);
        let (l, r) = buf.chunks_exact(2).fold((0.0f32, 0.0f32), |(l, r), f| {
            (l.max(f[0].abs()), r.max(f[1].abs()))
        });
        assert!(l > 2.0 * r, "left {l} right {r}");
    }

    /// The measured oracle levels (M3-audio rows 16-17): within 0.8 dB of the model, and the
    /// music/effect balance of the WAV captures.
    #[test]
    fn levels_match_the_oracle_measurements() {
        for (level, measured) in [(10u8, 0.0997f32), (11, 0.158), (12, 0.238), (13, 0.364)] {
            let db = 20.0 * (sb_level_gain(level) / measured).log10();
            assert!(db.abs() < 0.8, "level {level}: {db:+.2} dB");
        }
        assert_eq!(sb_level_gain(0), 0.0);
        assert_eq!(sb_level_gain(15), 1.0);
        // Oracle: music RMS / gain of a level-11 effect = 0.21 / 0.158.
        let ratio = MUSIC_GAIN / sb_level_gain(11);
        assert!((ratio - 0.21 / 0.158).abs() < 0.01, "{ratio}");
        // Level 13 (intro) at the default effects volume stays under full scale.
        assert!(OUTPUT_SCALE * sb_level_gain(13) * 0.8 <= 1.0);
    }

    #[test]
    fn effect_without_a_sample_keeps_the_current_sound() {
        let mut m = Mixer::new(48_000, SfxPolicy::Classic);
        m.command(effect(9, None));
        m.command(AudioCommand::PlayEffect {
            id: 1,
            sound: None,
            source: None,
            view: (10, 10),
        });
        assert_eq!(m.active_sfx(), 1);
    }

    #[test]
    fn sfx_lasts_its_duration_and_then_stops() {
        let mut m = Mixer::new(48_000, SfxPolicy::Classic);
        m.command(sfx(tone(8000, 800))); // 0.1 s
        let mut buf = vec![0.0f32; 2 * 4_700];
        m.render(&mut buf);
        assert_eq!(m.active_sfx(), 1, "still playing before 0.1 s");
        assert!(buf.iter().any(|&x| x.abs() > 0.1));
        m.render(&mut buf);
        assert_eq!(m.active_sfx(), 0, "finished after 0.1 s");
    }

    #[test]
    fn interpolation_is_linear() {
        let mut i = Interp::default();
        let mut src = [0.0f32, 1.0, 2.0].into_iter();
        let out: Vec<f32> = std::iter::from_fn(|| i.next(0.5, || src.next())).collect();
        assert_eq!(out, [0.0, 0.5, 1.0, 1.5]);
    }
}
