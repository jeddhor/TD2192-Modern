//! Sound-effect policy of Episode 1, Sound Blaster path: which effect id plays which VOC, how far
//! a source is from the view, whether a new effect may cut off the playing one, and the SB Pro
//! left/right volume.
//!
//! Spec: `docs/spec/audio/sfx-policy.md` rev 4 (§3.1, §4.1–§4.3); WORX's playing status per
//! `docs/spec/audio/voc-and-adpcm.md` rev 3 §2.3 ([`crate::worx`]). Fidelity: T1 for the
//! decisions (distance, priority, volume nibbles), T2 for what is heard and for the timing of
//! the playing status. The PC-speaker path (§4.5), which also draws on the gameplay RNG, belongs
//! with the simulation and is not here.
//!
//! [`DigitalVoice`] is the game's side of the Sound Blaster's single digital voice: the
//! arbiter's priority and WORX's playing status, which the flight reads at tick-order step 26
//! (priority reset) and at the end of the mission (wait). It lives with the game, not the
//! mixer, because the game must read the status; the mixer only plays what it is sent.

use crate::sound::Pcm8;
use crate::worx;

/// Effect ids the game uses and their Sound Blaster samples (sfx-policy §3.1, loaded by
/// `Snd_LoadEffects`, `file0001:1E2A:0724..0807`). Id 1 has no sample.
pub const EFFECTS: [(u8, Option<&str>); 13] = [
    (0, Some("afterb.voc")),
    (1, None),
    (2, Some("stiletto.voc")),
    (3, Some("vii.voc")),
    (4, Some("hornet.voc")),
    (5, Some("hornet.voc")),
    (6, Some("zeus.voc")),
    (7, Some("vulturei.voc")),
    (8, Some("vii.voc")),
    (9, Some("punyexp.voc")),
    (10, Some("missile.voc")),
    (100, Some("boomboom.voc")),
    (200, Some("income.voc")),
];

/// The VOC file effect `id` plays on a Sound Blaster, if any.
pub fn effect_voc(id: u8) -> Option<&'static str> {
    EFFECTS.iter().find(|e| e.0 == id).and_then(|e| e.1)
}

/// A tile position as the game passes it (bytes).
pub type TilePos = (u8, u8);

/// Distance in whole tiles from the centre of the view (5 tiles right, 3 down from `view`, the
/// view's top-left tile) to `source` (sfx-policy §4.1). `None` for no position (the player's own
/// sounds): distance 0.
///
/// Returns `None` when the 16-bit square comes out negative (a source about 181..256 tiles
/// away), where the original stops with RunError 207: OB-009, fixed as "too far, do not play".
/// Larger distances wrap the square back to a positive value and give a wrong, short distance,
/// as in the original.
pub fn distance(source: Option<TilePos>, view: TilePos) -> Option<u16> {
    let Some((sx, sy)) = source else {
        return Some(0);
    };
    let dx = i16::from(view.0) + 5 - i16::from(sx);
    let dy = i16::from(view.1) + 3 - i16::from(sy);
    let s = dy.wrapping_mul(dy).wrapping_add(dx.wrapping_mul(dx));
    let s = u16::try_from(s).ok()?;
    Some(round_sqrt(s))
}

/// `Round(Sqrt(Real(s)))` for 0 ≤ s ≤ 32767, in integers. Exact for the Real48 routines
/// (real48 §4.6, §4.9): their `Sqrt` can be 1 ulp (2^−39 relative) off, but `sqrt(s)` of an
/// integer s is never within 1/(8·182) of a half-integer in this range, and `Round` is half away
/// from zero, so the rounded result is the n with n² − n < s ≤ n² + n.
fn round_sqrt(s: u16) -> u16 {
    let s = u32::from(s);
    let mut n = s.isqrt();
    if s > n * n + n {
        n += 1;
    }
    n as u16
}

/// Base priority of effect `id`: `Id div 20` (0 for 0..10, 5 for 100, 10 for 200).
pub fn base_priority(id: u8) -> u8 {
    id / 20
}

/// Priority of a request (sfx-policy §4.2): `Id div 20 − dist div 15`, so ordinary effects
/// 15 or more tiles away (90 for id 100, 165 for id 200) are below the idle priority 0.
pub fn priority(id: u8, dist: u16) -> i16 {
    i16::from(base_priority(id)) - (dist / 15) as i16
}

/// Left and right volume, 0..15, before the mixer scaling (sfx-policy §4.3). `dist` is the
/// value from [`distance`].
pub fn stereo_levels(source: Option<TilePos>, view: TilePos, dist: u16) -> (u8, u8) {
    let Some((sx, _)) = source else {
        return (15, 15);
    };
    let (sx, vx) = (i16::from(sx), i16::from(view.0));
    let d = dist as i16 / 5;
    let mut l: i16 = if sx < vx + 10 { 18 } else { 0 };
    let mut r: i16 = if sx > vx { 18 } else { 0 };
    l = l - d + (15 - r) / 5; // uses R before its update
    r = r - d + (15 - l) / 5; // uses the updated L
    l = l.max(0);
    r = r.max(0);
    if l > 15 {
        r += l - 15;
        l = 15;
    }
    if r > 15 {
        l += r - 15;
        r = 15;
    }
    l = l.min(15);
    (l as u8, r as u8)
}

/// One channel's level as written to the SB Pro mixer: `12·level div 16` (0..11).
pub fn mixer_level(level: u8) -> u8 {
    12 * level.min(15) / 16
}

/// The SB Pro mixer voice-volume byte (register 04h) the game writes through WORX function 21h:
/// left nibble high, right nibble low.
pub fn mixer_voice_byte(left: u8, right: u8) -> u8 {
    (mixer_level(left) << 4) | mixer_level(right)
}

/// The Classic arbiter for the single digital voice (sfx-policy §4.2, U-021).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ClassicArbiter {
    current: u8,
}

impl ClassicArbiter {
    /// Priority of the sound now playing (0 = idle).
    pub fn current_priority(&self) -> u8 {
        self.current
    }

    /// Whether effect `id` at distance `dist` plays. A new effect pre-empts the current one when
    /// its priority is at least the current priority; the current priority then becomes
    /// `Id div 20` (distance not included).
    pub fn request(&mut self, id: u8, dist: u16) -> bool {
        if i16::from(self.current) > priority(id, dist) {
            return false;
        }
        self.current = base_priority(id);
        true
    }

    /// No digital sound is playing (checked on every flight tick) or a mission starts: the
    /// current priority returns to 0.
    pub fn idle(&mut self) {
        self.current = 0;
    }
}

/// The game's side of the single digital voice under Classic (device 2): `g_SfxPriority`
/// (DS:1F0A, [`ClassicArbiter`]) and WORX's "digital sound playing" status (function 08h), on
/// the caller's clock in seconds.
///
/// The status is set from an effect's start for [`worx::busy_seconds`] with the program's ring
/// offset r, not until the audio ends (voc §2.3, sfx-policy rev 3 §4.2): a `boomboom` holds
/// priority 5 for about 1.88 s in Episode 1, though its sound lasts 1.76 s.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DigitalVoice {
    arbiter: ClassicArbiter,
    ring: u16,
    /// When the playing status clears; `None` while idle.
    busy_until: Option<f64>,
}

impl DigitalVoice {
    /// An idle voice of a program whose WORX ring offset is `ring` ([`worx::RING_E1`] etc.).
    #[must_use]
    pub fn new(ring: u16) -> Self {
        Self {
            arbiter: ClassicArbiter::default(),
            ring,
            busy_until: None,
        }
    }

    /// `Snd_PlayEffect(id, …)` on a Sound Blaster at time `now` (sfx-policy §4.2), `dist` from
    /// [`distance`] and `sound` the effect's sample. Returns true when a sound starts: WORX
    /// function 06h stops the playing one and the status is set from `now`. An effect whose
    /// priority is below the current one does nothing.
    ///
    /// ASSUMPTION: an id without a sample (1) sets the priority but starts nothing and leaves
    /// the status alone; the spec does not say whether WORX is asked to play nothing.
    pub fn request(&mut self, id: u8, dist: u16, sound: Option<&Pcm8>, now: f64) -> bool {
        if !self.arbiter.request(id, dist) {
            return false;
        }
        let Some(sound) = sound else {
            return false;
        };
        self.busy_until = Some(now + worx::busy_seconds(sound, self.ring));
        true
    }

    /// WORX function 08h at time `now`: true while the last effect started is within its busy
    /// time.
    #[must_use]
    pub fn playing(&self, now: f64) -> bool {
        self.busy_until.is_some_and(|t| now < t)
    }

    /// Tick-order step 26 (`0E8B:E526..E536`): when WORX reports no digital sound playing,
    /// the current priority returns to 0.
    pub fn priority_reset(&mut self, now: f64) {
        if !self.playing(now) {
            self.busy_until = None;
            self.arbiter.idle();
        }
    }

    /// The current priority (0 = idle).
    #[must_use]
    pub fn priority(&self) -> u8 {
        self.arbiter.current_priority()
    }

    /// When the playing status clears (`None` while idle): the mission-end wait
    /// (`0E8B:F990..F99E`) lasts until then.
    #[must_use]
    pub fn busy_until(&self) -> Option<f64> {
        self.busy_until
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// sfx-policy §6 vectors 1–5: distance, levels and the mixer byte.
    #[test]
    fn spec_vectors_volume_and_panning() {
        let view = (10, 10);
        for (src, dist, levels, byte) in [
            (None, 0, (15, 15), 0xBB),
            (Some((15, 13)), 0, (15, 15), 0xBB),
            (Some((8, 13)), 7, (15, 5), 0xB3),
            (Some((22, 13)), 7, (5, 15), 0x3B),
            (Some((0, 0)), 20, (15, 2), 0xB1),
        ] {
            let d = distance(src, view).unwrap();
            assert_eq!(d, dist, "{src:?}");
            let (l, r) = stereo_levels(src, view, d);
            assert_eq!((l, r), levels, "{src:?}");
            assert_eq!(mixer_voice_byte(l, r), byte, "{src:?}");
        }
        assert_eq!(priority(9, 20), -1, "vector 5 is not played");
    }

    #[test]
    fn distance_rounds_like_real48() {
        // s = 394 → 19.85 → 20 (vector 5); exact squares; the half-way neighbours.
        assert_eq!(round_sqrt(394), 20);
        assert_eq!(round_sqrt(400), 20);
        assert_eq!(round_sqrt(420), 20); // 20.494
        assert_eq!(round_sqrt(421), 21); // 20.518
        assert_eq!(round_sqrt(32767), 181);
        for s in 0..=32767u16 {
            let exact = f64::from(s).sqrt().round() as u16;
            assert_eq!(round_sqrt(s), exact, "{s}");
        }
    }

    #[test]
    fn distance_overflow_is_too_far() {
        // dx = 190: 36100 > 32767, negative as a 16-bit Integer (OB-009).
        assert_eq!(distance(Some((0, 3)), (185, 0)), None);
        // dx = 255 + 5: 67600 wraps to 2064 (distance 45): kept as the original computes it.
        assert_eq!(distance(Some((0, 3)), (255, 0)), Some(45));
    }

    /// Vector 6: distance 14 plays, 15 does not (idle each time).
    #[test]
    fn effects_15_tiles_away_never_play() {
        let mut a = ClassicArbiter::default();
        assert!(a.request(9, 14));
        a.idle();
        assert!(!a.request(9, 15));
        assert!(!a.request(100, 90));
        assert!(a.request(100, 89));
        a.idle();
        assert!(a.request(200, 164) && !ClassicArbiter::default().request(200, 165));
    }

    /// Vector 7 and the equal-priority rule.
    #[test]
    fn higher_priority_effects_are_not_cut() {
        let mut a = ClassicArbiter::default();
        assert!(a.request(100, 0));
        assert_eq!(a.current_priority(), 5);
        assert!(!a.request(9, 0), "explosion keeps playing");
        assert!(a.request(100, 0), "equal priority pre-empts");
        assert!(a.request(200, 30), "income outranks it");
        assert_eq!(a.current_priority(), 10, "distance not included");
        assert!(!a.request(100, 0));
        a.idle();
        assert!(a.request(9, 0));
        assert!(a.request(2, 0), "id 0..10 replace each other");
    }

    #[test]
    fn effect_table() {
        assert_eq!(effect_voc(3), effect_voc(8));
        assert_eq!(effect_voc(4), Some("hornet.voc"));
        assert_eq!(effect_voc(1), None);
        assert_eq!(effect_voc(11), None);
        assert_eq!(base_priority(200), 10);
    }

    fn sample(rate: u32, bytes: u32, codes_per_byte: u8) -> Pcm8 {
        Pcm8 {
            rate,
            samples: Vec::new(),
            dma: crate::sound::DmaStream {
                bytes,
                codes_per_byte,
            },
        }
    }

    /// sfx-policy rev 3 §6 vector 10 (C-0072): E1 started directly (r = 58), flight 1 at
    /// 18.52 ticks/s; an enemy `boomboom` holds priority 5 from the tick of the explosion to
    /// the first tick after 1.903 s (observed: 0 in the dump 1.926 s later).
    #[test]
    fn boomboom_holds_its_priority_for_its_busy_time() {
        let boom = sample(6_250, 5_497, 2);
        let blast = sample(6_993, 1_142, 3);
        let tick = 1.0 / 18.521;
        let mut v = DigitalVoice::new(worx::RING_E1_DIRECT);
        assert!(v.request(100, 3, Some(&boom), 0.0));
        let mut reset = None;
        for n in 1..60 {
            let now = f64::from(n) * tick;
            v.priority_reset(now);
            if v.priority() == 0 {
                reset = Some(now);
                break;
            }
            assert!(!v.request(9, 0, Some(&blast), now), "tick {n}");
        }
        let reset = reset.unwrap();
        // "Predicted 1.903 s plus less than one tick" (voc §2.3). C-0072 observed 1.926 s from
        // the first dump showing the priority, which also counts the time the explosion's tick
        // had run before step 6 (not modelled: the port's ticks are instants).
        assert!(reset >= 1.903 && reset < 1.903 + tick, "{reset}");
        assert!(!v.playing(reset));
        assert!(v.request(9, 0, Some(&blast), reset));
    }

    /// The status follows the last effect started; a refused effect does not touch it; an id
    /// without a sample sets the priority only.
    #[test]
    fn digital_voice_status() {
        let pulse = sample(4_000, 897, 1);
        let mut v = DigitalVoice::new(worx::RING_E1);
        assert!(!v.playing(0.0));
        assert_eq!(v.busy_until(), None);
        assert!(v.request(200, 0, Some(&pulse), 1.0));
        let end = v.busy_until().unwrap();
        assert!((end - 1.0 - 0.3825).abs() < 1e-9);
        assert!(!v.request(9, 0, Some(&pulse), 1.1), "income outranks it");
        assert_eq!(v.busy_until(), Some(end));
        v.priority_reset(1.2);
        assert_eq!(v.priority(), 10, "still playing");
        v.priority_reset(end);
        assert_eq!((v.priority(), v.busy_until()), (0, None));
        assert!(!v.request(1, 0, None, 2.0), "nothing to start");
        assert!(!v.playing(2.0));
    }
}
