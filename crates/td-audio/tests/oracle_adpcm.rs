//! L5 parity: the ADPCM decoder against the oracle's audio (voc-and-adpcm rev 3 §4.3, BUG-4).
//! Needs `TD2192_DATA` (the effects) and `TD2192_ORACLE` (the WAV captures).
//!
//! C-0071 (intro, music off) and C-0072 (E1 flight 1, music off) were recorded by DOSBox-X at
//! 48 kHz, 16-bit stereo, the effects unpositioned (L = R). Each effect is decoded with §3 at the
//! rate of §2.2, resampled linearly to 48 kHz and correlated, mean removed, with the left channel
//! at the best lag within ±10 ms of the spec's onset. The spec's targets are ≥ 0.98 for every
//! codec (measured by RE-16: 0.984..0.999); the rest is the oracle's output filter and resampler.

use std::fs;
use std::path::Path;

use td_audio::decode_voc;
use td_test_support::{require_data, require_oracle};

const WAV_RATE: f64 = 48_000.0;
/// ±10 ms at 48 kHz.
const MAX_LAG: isize = 480;

/// The left channel of a 16-bit stereo PCM WAV, as -1..1.
fn wav_left(path: &Path) -> Vec<f64> {
    let d = fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert_eq!(
        (&d[..4], &d[8..12]),
        (&b"RIFF"[..], &b"WAVE"[..]),
        "RIFF/WAVE"
    );
    let u16_at = |i: usize| u16::from_le_bytes([d[i], d[i + 1]]);
    let u32_at = |i: usize| u32::from_le_bytes([d[i], d[i + 1], d[i + 2], d[i + 3]]) as usize;
    let mut i = 12;
    let mut format = None;
    while i + 8 <= d.len() {
        let (id, size) = (&d[i..i + 4], u32_at(i + 4));
        let body = i + 8;
        if id == b"fmt " {
            format = Some((
                u16_at(body),
                u16_at(body + 2),
                u32_at(body + 4),
                u16_at(body + 14),
            ));
        } else if id == b"data" {
            assert_eq!(
                format,
                Some((1, 2, 48_000, 16)),
                "PCM, 2 channels, 48 kHz, 16 bits"
            );
            let data = &d[body..(body + size).min(d.len())];
            return data
                .chunks_exact(4)
                .map(|f| f64::from(i16::from_le_bytes([f[0], f[1]])) / 32768.0)
                .collect();
        }
        i = body + size + (size & 1);
    }
    panic!("{}: no data chunk", path.display());
}

/// `samples` (unsigned 8-bit at `rate`) resampled linearly to 48 kHz, centred.
fn resample(samples: &[u8], rate: u32) -> Vec<f64> {
    let step = f64::from(rate) / WAV_RATE;
    let n = ((samples.len() as f64 - 1.0) / step).floor().max(0.0) as usize;
    (0..n)
        .map(|k| {
            let pos = k as f64 * step;
            let i = pos.floor() as usize;
            let frac = pos - i as f64;
            let a = f64::from(samples[i]) - 128.0;
            let b = f64::from(*samples.get(i + 1).unwrap_or(&samples[i])) - 128.0;
            a + (b - a) * frac
        })
        .collect()
}

/// Mean-removed correlation of `x` with `y[at..]` over `x.len()` samples.
fn correlation(x: &[f64], y: &[f64], at: usize) -> f64 {
    let y = &y[at..at + x.len()];
    let n = x.len() as f64;
    let (mx, my) = (x.iter().sum::<f64>() / n, y.iter().sum::<f64>() / n);
    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
    for (a, b) in x.iter().zip(y) {
        let (a, b) = (a - mx, b - my);
        sxy += a * b;
        sxx += a * a;
        syy += b * b;
    }
    sxy / (sxx * syy).sqrt().max(f64::MIN_POSITIVE)
}

/// The best correlation of `decoded` against `wav` within ±10 ms of `onset_s`, over
/// `length_s`, and its lag in samples. The lag is found on the first 0.3 s and refined over the
/// whole length.
fn best_match(decoded: &[f64], wav: &[f64], onset_s: f64, length_s: f64) -> (f64, isize) {
    let len = ((length_s * WAV_RATE) as usize).min(decoded.len());
    let onset = (onset_s * WAV_RATE).round() as isize;
    let at = |lag: isize| usize::try_from(onset + lag).expect("onset inside the WAV");
    let head = &decoded[..len.min((0.3 * WAV_RATE) as usize)];
    let coarse = (-MAX_LAG..=MAX_LAG)
        .max_by(|&a, &b| correlation(head, wav, at(a)).total_cmp(&correlation(head, wav, at(b))))
        .expect("lags");
    let whole = &decoded[..len];
    ((coarse - 3).max(-MAX_LAG)..=(coarse + 3).min(MAX_LAG))
        .map(|lag| (correlation(whole, wav, at(lag)), lag))
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .expect("lags")
}

#[test]
fn adpcm_effects_correlate_with_the_oracle_wavs() {
    let data = require_data!();
    let oracle = require_oracle!();
    let c0071 = wav_left(&oracle.join("C-0071/capture/td1_000.wav"));
    let c0072 = wav_left(&oracle.join("C-0072/capture/file0001_000.wav"));
    // voc §4.3: effect, capture, onset (s), length compared (s), RE-16's correlation.
    for (name, wav, onset, length, measured) in [
        ("boomboom.voc", &c0072, 20.568, 1.7, 0.9975),
        ("home.voc", &c0071, 36.790, 4.0, 0.9878),
        ("ric3.voc", &c0071, 31.939, 2.8, 0.9838),
        ("punyexp.voc", &c0072, 32.276, 0.49, 0.9969),
        ("vulturei.voc", &c0072, 24.065, 0.22, 0.9992),
    ] {
        let pcm = decode_voc(&fs::read(data.join(name)).unwrap()).unwrap();
        let decoded = resample(&pcm.samples, pcm.rate);
        let (r, lag) = best_match(&decoded, wav, onset, length);
        eprintln!(
            "{name}: correlation {r:.4} (spec {measured}) at {:+.2} ms from {onset} s, {} Hz",
            lag as f64 * 1000.0 / WAV_RATE,
            pcm.rate
        );
        assert!(r >= 0.98, "{name}: correlation {r:.4} below 0.98");
    }
}
