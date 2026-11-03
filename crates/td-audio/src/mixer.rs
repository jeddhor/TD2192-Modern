//! Mixing music and sound effects to interleaved stereo `f32` at the device rate.
//!
//! Music: one [`CmfPlayer`] for the whole session, as the original keeps one WORX driver (its
//! state carries over between songs, cmf §4.10). A new song stops the current one first and
//! always starts from the top (music-cues §4.1).
//!
//! Sound effects: `docs/spec/audio/sfx-policy.md` rev 1. [`SfxPolicy::Classic`] is the original
//! single digital voice with priority pre-emption (§4.2); [`SfxPolicy::Polyphonic`] is the
//! Enhanced option. Both apply the distance cut-off and the SB Pro left/right levels (§4.3).
//!
//! Fidelity: T2. Resampling is linear interpolation. The music/SFX balance is untuned, and the SB
//! Pro mixer's attenuation curve is not modelled: a mixer level n (0..11) gives the linear gain
//! n/11, so an effect at the view centre plays at full gain.

use std::sync::Arc;

use td_formats::cmf::Cmf;

use crate::cmf_player::CmfPlayer;
use crate::sfx::{self, ClassicArbiter, TilePos};
use crate::sound::Pcm8;

/// How many sound effects may play at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SfxPolicy {
    /// The original: one digital voice. A new effect replaces the playing one when its priority
    /// is at least the current priority (sfx-policy §4.2).
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
    /// An unpositioned sound at full volume with the lowest priority (like effect ids 0..10
    /// without a position).
    PlaySfx {
        sound: Arc<Pcm8>,
    },
    /// A game sound effect: `Snd_PlayEffect(id, source, view)` on a Sound Blaster
    /// (sfx-policy §4). `sound` is the effect's sample ([`sfx::effect_voc`]); `None` for an id
    /// without one. `source` is the sound's tile, `None` for the player's own sounds (SrcX =
    /// FFh); `view` is the view's top-left tile.
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
    arbiter: ClassicArbiter,
    music_volume: f32,
    sfx_volume: f32,
}

/// Linear gain of an SB Pro mixer level (0..11; see the module comment).
fn level_gain(level: u8) -> f32 {
    f32::from(sfx::mixer_level(level)) / 11.0
}

impl Mixer {
    /// A mixer producing `rate` Hz stereo.
    pub fn new(rate: u32, policy: SfxPolicy) -> Self {
        Self {
            rate,
            policy,
            music: None,
            voices: Vec::new(),
            arbiter: ClassicArbiter::default(),
            music_volume: 0.6,
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

    /// Priority of the playing digital effect under [`SfxPolicy::Classic`] (0 = idle).
    pub fn sfx_priority(&self) -> u8 {
        if self.voices.is_empty() {
            0
        } else {
            self.arbiter.current_priority()
        }
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
            AudioCommand::PlaySfx { sound } => self.play_effect(0, Some(sound), None, (0, 0)),
            AudioCommand::PlayEffect {
                id,
                sound,
                source,
                view,
            } => self.play_effect(id, sound, source, view),
            AudioCommand::StopSfx => {
                self.voices.clear();
                self.arbiter.idle();
            }
            AudioCommand::SetVolumes { music, sfx } => {
                self.music_volume = music.clamp(0.0, 1.0);
                self.sfx_volume = sfx.clamp(0.0, 1.0);
            }
        }
    }

    /// `Snd_PlayEffect` with a Sound Blaster (sfx-policy §4.1–§4.3).
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
        match self.policy {
            SfxPolicy::Classic => {
                if self.voices.is_empty() {
                    self.arbiter.idle();
                }
                if !self.arbiter.request(id, dist) {
                    return;
                }
                // ASSUMPTION: an id without a sample (1) sets the priority but leaves the playing
                // sound alone; the spec does not say whether WORX is asked to play nothing.
                let Some(sound) = sound else {
                    return;
                };
                self.voices.clear(); // WORX function 06h stops the playing sound first
                self.push_voice(sound, source, view, dist);
            }
            SfxPolicy::Polyphonic(n) => {
                if sfx::priority(id, dist) < 0 {
                    return;
                }
                let Some(sound) = sound else {
                    return;
                };
                while self.voices.len() >= n.max(1) {
                    self.voices.remove(0);
                }
                self.push_voice(sound, source, view, dist);
            }
        }
    }

    fn push_voice(&mut self, sound: Arc<Pcm8>, source: Option<TilePos>, view: TilePos, dist: u16) {
        let (l, r) = sfx::stereo_levels(source, view, dist);
        self.voices.push(Voice {
            sound,
            pos: 0,
            interp: Interp::default(),
            gain: (level_gain(l), level_gain(r)),
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
                let s = v.unwrap_or(0.0) * self.music_volume;
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
        Arc::new(Pcm8 {
            rate,
            samples: (0..n).map(|i| if i % 2 == 0 { 200 } else { 56 }).collect(),
        })
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
        m.command(AudioCommand::PlaySfx {
            sound: tone(8000, 800),
        });
        m.command(AudioCommand::PlaySfx {
            sound: tone(8000, 800),
        });
        assert_eq!(m.active_sfx(), 1);
        m.set_policy(SfxPolicy::Polyphonic(4));
        m.command(AudioCommand::PlaySfx {
            sound: tone(8000, 800),
        });
        assert_eq!(m.active_sfx(), 2);
    }

    #[test]
    fn classic_priority_pre_emption() {
        let mut m = Mixer::new(48_000, SfxPolicy::Classic);
        m.command(effect(100, None));
        assert_eq!(m.sfx_priority(), 5);
        let first = Arc::as_ptr(&m.voices[0].sound);
        m.command(effect(9, None));
        assert_eq!(Arc::as_ptr(&m.voices[0].sound), first, "explosion not cut");
        m.command(effect(200, Some((15, 13))));
        assert_ne!(Arc::as_ptr(&m.voices[0].sound), first, "income cuts it");
        assert_eq!((m.active_sfx(), m.sfx_priority()), (1, 10));
        // The voice ends: idle again, so an ordinary effect plays.
        let mut buf = vec![0.0f32; 2 * 9_600];
        m.render(&mut buf);
        assert_eq!(m.sfx_priority(), 0);
        m.command(effect(9, None));
        assert_eq!(m.active_sfx(), 1);
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
        m.command(effect(9, Some((8, 13)))); // left of centre: L 15, R 5
        assert_eq!(m.voices[0].gain, (1.0, 3.0 / 11.0));
        let mut buf = vec![0.0f32; 2 * 1_000];
        m.render(&mut buf);
        let (l, r) = buf.chunks_exact(2).fold((0.0f32, 0.0f32), |(l, r), f| {
            (l.max(f[0].abs()), r.max(f[1].abs()))
        });
        assert!(l > 2.0 * r, "left {l} right {r}");
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
        m.command(AudioCommand::PlaySfx {
            sound: tone(8000, 800),
        }); // 0.1 s
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
