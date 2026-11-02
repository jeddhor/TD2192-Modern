//! Mixing music and sound effects to interleaved stereo `f32` at the device rate.
//!
//! Fidelity: T2. Resampling is linear interpolation; the music/SFX balance and the Classic SFX
//! policy (one digital voice, newest sound wins) are provisional until U-021 is resolved.

use std::sync::Arc;

use td_formats::cmf::Cmf;

use crate::cmf_player::CmfPlayer;
use crate::sound::Pcm8;

/// How many sound effects may play at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SfxPolicy {
    /// One digital voice, like a Sound Blaster DSP: a new effect replaces the current one.
    Classic,
    /// Up to `n` simultaneous voices (Enhanced profile); the oldest is dropped when full.
    Polyphonic(usize),
}

/// Commands from the game to the audio system.
#[derive(Debug, Clone)]
pub enum AudioCommand {
    PlayMusic { song: Arc<Cmf>, looping: bool },
    StopMusic,
    PlaySfx { sound: Arc<Pcm8> },
    StopSfx,
    SetVolumes { music: f32, sfx: f32 },
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
}

/// The audio mixer.
#[derive(Debug)]
pub struct Mixer {
    rate: u32,
    policy: SfxPolicy,
    music: Option<Music>,
    voices: Vec<Voice>,
    music_volume: f32,
    sfx_volume: f32,
}

impl Mixer {
    /// A mixer producing `rate` Hz stereo.
    pub fn new(rate: u32, policy: SfxPolicy) -> Self {
        Self {
            rate,
            policy,
            music: None,
            voices: Vec::new(),
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

    /// True while music is playing.
    pub fn music_playing(&self) -> bool {
        self.music.as_ref().is_some_and(|m| !m.player.finished())
    }

    /// Number of sound effects currently playing.
    pub fn active_sfx(&self) -> usize {
        self.voices.len()
    }

    /// Applies a command.
    pub fn command(&mut self, cmd: AudioCommand) {
        match cmd {
            AudioCommand::PlayMusic { song, looping } => {
                self.music = Some(Music {
                    player: CmfPlayer::new(song, looping),
                    buf: Vec::new(),
                    pos: 0,
                    interp: Interp::default(),
                });
            }
            AudioCommand::StopMusic => self.music = None,
            AudioCommand::PlaySfx { sound } => {
                let limit = match self.policy {
                    SfxPolicy::Classic => 1,
                    SfxPolicy::Polyphonic(n) => n.max(1),
                };
                while self.voices.len() >= limit {
                    self.voices.remove(0);
                }
                self.voices.push(Voice {
                    sound,
                    pos: 0,
                    interp: Interp::default(),
                });
            }
            AudioCommand::StopSfx => self.voices.clear(),
            AudioCommand::SetVolumes { music, sfx } => {
                self.music_volume = music.clamp(0.0, 1.0);
                self.sfx_volume = sfx.clamp(0.0, 1.0);
            }
        }
    }

    /// Fills `out` (interleaved stereo) with the next `out.len() / 2` frames.
    pub fn render(&mut self, out: &mut [f32]) {
        let rate = f64::from(self.rate);
        for frame in out.chunks_exact_mut(2) {
            let mut s = 0.0f32;
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
                        if player.finished() {
                            return None;
                        }
                        buf.resize(512, 0);
                        player.render(buf);
                        *pos = 0;
                    }
                    let x = buf[*pos];
                    *pos += 1;
                    Some(x as f32 / 32768.0)
                });
                s += v.unwrap_or(0.0) * self.music_volume;
            }
            self.voices.retain_mut(|v| {
                let step = f64::from(v.sound.rate) / rate;
                let Voice { sound, pos, interp } = v;
                match interp.next(step, || {
                    let x = *sound.samples.get(*pos)?;
                    *pos += 1;
                    Some((f32::from(x) - 128.0) / 128.0)
                }) {
                    Some(x) => {
                        s += x * self.sfx_volume;
                        true
                    }
                    None => false,
                }
            });
            let s = s.clamp(-1.0, 1.0);
            frame[0] = s;
            frame[1] = s;
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
