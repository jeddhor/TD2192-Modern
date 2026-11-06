//! WORX's "digital sound playing" status (function 08h): how long it stays set after an effect
//! starts.
//!
//! Spec: `docs/spec/audio/voc-and-adpcm.md` rev 3 §2.3 (U-410). Fidelity: T2 (the status is
//! modelled from the effect's start, to within one WORX second-level period).
//!
//! WORX clears the byte only in a refill of its 4,096-byte DMA ring, and it refills only when
//! the high byte of the DMA address moves past one of twelve points per turn of the ring, 256
//! bytes apart from 400h + r, where r depends on where DOS loaded the program. The first
//! refill that accounts the ring up to the end of the data finds it used up; the **next** one
//! halts the DSP and clears the status. So the status lasts X / R seconds from the effect's
//! start (X = that refill point, R = the DMA byte rate), plus up to one second-level period,
//! whatever the length of the audio.
//!
//! ASSUMPTION(U-470): the port takes X / R exactly and does not add the second-level period
//! (0..20.8 ms while a 96 Hz song plays, depending on the phase of WORX's fast ticks). With the
//! intro's checks at frame starts this reproduces every busy time C-0081 measured, to the frame
//! (`whoosh` 317 frames, `pulse` and `scratch` 25–26, `ric3` 211, `home` 309); adding a fixed
//! 10.4 ms instead makes `whoosh` and `pulse` a frame too long.

use crate::sound::Pcm8;

/// Ring offset r of `file0001.exe` (Episode 1) started by `td1.exe`, the normal path (voc §2.3
/// table: WORX segment 3FDFh, buffer 416h, A mod 256 = 06h).
pub const RING_E1: u16 = 250;
/// Ring offset r of `td2.exe` (Episode 2) run directly (segment 2EEFh).
pub const RING_E2: u16 = 250;
/// Ring offset r of `td3.exe` (Episode 3) run directly (segment 2AFEh).
pub const RING_E3: u16 = 10;
/// Ring offset r of the intro `file0000.exe` started by `td1.exe` (segment 16A2h, buffer 435h).
pub const RING_INTRO: u16 = 171;
/// Ring offset r of `file0001.exe` run directly (C-0072, C-0073, C-0074; not a Classic path).
pub const RING_E1_DIRECT: u16 = 58;

/// X: the DMA position, in bytes since the start of the effect, of the refill that halts the
/// DSP and clears the status, for `data_bytes` (D) bytes of sound data and ring offset `r`
/// (voc §2.3 rule).
///
/// The events, in order: in each turn c of the ring (c = 0, 1, ...), a wrap refill (c ≥ 1)
/// at 4096c + 400h + r that accounts the ring up to 4096c, then the normal refills at
/// 4096c + 400h + r + 256j (j = 0..11), each accounting up to its own position. The first event
/// that accounts up to ≥ D finds the data used up; the position of the next event is X.
#[must_use]
pub fn clear_position(data_bytes: u32, r: u16) -> u32 {
    let base = 0x400 + u32::from(r % 256);
    let mut found = false;
    for c in 0u32.. {
        let turn = 4096 * c;
        let wrap = (c >= 1).then_some((turn + base, turn));
        let normals = (0..12).map(|j| {
            let p = turn + base + 256 * j;
            (p, p)
        });
        for (at, accounts) in wrap.into_iter().chain(normals) {
            if found {
                return at;
            }
            found = accounts >= data_bytes;
        }
    }
    unreachable!("the refill points grow without bound")
}

/// How long WORX reports `sound` as playing after it starts, in seconds: X / R, where
/// R = rate / samples per byte is the DMA byte rate (voc §2.3; rate per §2.2; the second-level
/// period is not added, U-470).
#[must_use]
pub fn busy_seconds(sound: &Pcm8, r: u16) -> f64 {
    let x = f64::from(clear_position(sound.dma.bytes, r));
    let per_byte = f64::from(sound.dma.codes_per_byte.max(1));
    x * per_byte / f64::from(sound.rate.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sound::DmaStream;

    fn voc(rate: u32, bytes: u32, codes_per_byte: u8) -> Pcm8 {
        Pcm8 {
            rate,
            samples: Vec::new(),
            dma: DmaStream {
                bytes,
                codes_per_byte,
            },
        }
    }

    /// The shipped effects (voc §2.2 table: rate, D, codec).
    fn effects() -> [(&'static str, Pcm8); 15] {
        [
            ("afterb", voc(6_024, 4_159, 2)),
            ("boomboom", voc(6_250, 5_497, 2)),
            ("home", voc(8_000, 16_116, 2)),
            ("hornet", voc(4_000, 4_409, 1)),
            ("income", voc(8_000, 5_573, 2)),
            ("missile", voc(6_410, 3_764, 2)),
            ("pulse", voc(4_000, 897, 1)),
            ("punyexp", voc(6_993, 1_142, 3)),
            ("ric3", voc(11_904, 16_764, 2)),
            ("scratch", voc(8_000, 595, 2)),
            ("stiletto", voc(8_000, 721, 4)),
            ("vii", voc(8_000, 1_460, 2)),
            ("vulturei", voc(6_024, 331, 4)),
            ("whoosh", voc(4_000, 17_689, 1)),
            ("zeus", voc(8_000, 1_827, 3)),
        ]
    }

    /// voc §2.3 "Busy time X / R by effect", every column, to the table's millisecond.
    #[test]
    fn busy_times_match_the_spec_table() {
        let table: [[f64; 4]; 15] = [
            [1.868, 1.788, 1.842, 1.804],
            [1.882, 1.887, 1.857, 1.903],
            [4.095, 4.354, 4.395, 4.367],
            [1.407, 1.347, 1.387, 1.359],
            [1.470, 1.474, 1.515, 1.486],
            [1.276, 1.601, 1.651, 1.616],
            [0.383, 0.323, 0.363, 0.335],
            [0.656, 0.663, 0.622, 0.684],
            [3.010, 2.969, 2.996, 2.977],
            [0.383, 0.323, 0.363, 0.335],
            [0.765, 0.645, 0.726, 0.669],
            [0.447, 0.451, 0.491, 0.463],
            [1.016, 0.857, 0.963, 0.888],
            [4.543, 4.546, 4.523, 4.495],
            [0.862, 0.868, 0.832, 0.790],
        ];
        for ((name, e), row) in effects().iter().zip(table) {
            for (r, want) in [RING_E1, RING_E3, RING_INTRO, RING_E1_DIRECT]
                .into_iter()
                .zip(row)
            {
                let got = busy_seconds(e, r);
                assert!(
                    (got - want).abs() < 0.0006,
                    "{name} r = {r}: {got:.4} s, spec {want}"
                );
            }
        }
        assert_eq!((RING_E2, RING_E1), (250, 250));
    }

    /// The worked examples: `pulse` (D = 897) is used up by the first refill, so the status
    /// clears at the second point; `home` at the wrap of turn 4.
    #[test]
    fn clear_positions() {
        assert_eq!(clear_position(897, RING_INTRO), 0x400 + 171 + 256);
        assert_eq!(clear_position(16_116, RING_INTRO), 4 * 4096 + 0x400 + 171);
        // Exactly at a point: that refill accounts up to D, the next one clears.
        assert_eq!(clear_position(0x400 + 250, 250), 0x400 + 250 + 256);
        // Accounted by a wrap refill (up to 4096): the turn's j = 0 refill clears, at the same
        // position.
        assert_eq!(clear_position(4096, 0), 4096 + 0x400);
    }

    /// C-0081 (intro, music on): the measured busy times of the playing byte, in whole frames
    /// from the frame the effect starts (the status reads 0 from the first frame at or after
    /// X / R; `pulse` and `scratch` measured 25 or 26).
    #[test]
    fn intro_busy_times_match_c0081_to_the_frame() {
        for (name, frames) in [
            ("whoosh", 317.0),
            ("pulse", 26.0),
            ("scratch", 26.0),
            ("ric3", 211.0),
            ("home", 309.0),
        ] {
            let e = &effects().into_iter().find(|e| e.0 == name).unwrap().1;
            let got = (busy_seconds(e, RING_INTRO) * 70.086).ceil();
            assert_eq!(got, frames, "{name}");
        }
    }
}
