//! Palette fades in the VGA 6-bit domain.
//!
//! Fidelity: provisional. ASSUMPTION(M5-02): a fade moves every component linearly from the
//! source to the target palette over `steps` ticks, truncating toward the source
//! (`src + (dst - src) * step / steps`). The original's step count, rate and rounding come from
//! its DAC-writing routines (`docs/spec/render/fades.md`, to be written).

use td_formats::palette::{Palette, Rgb6};

/// Brightness levels of the original's scene fades: k = 0..=16, so 17 steps.
/// Spec: `docs/spec/narrative/dialogue-player.md` (rev 1) §3.4, §4.6; original
/// `file0001:051A:0016` (fade in) and `051A:013F` (fade out).
pub const LEVELS: u8 = 17;

/// One step of the original's scene fades: DAC entries 1..=255 become
/// Round(component × k / 16) of `src` (Real48 `Round`, halves away from zero; exact here
/// because the division is by a power of two). Entry 0 is not written. Spec:
/// dialogue-player.md §3.4, §4.6. Fidelity: T0 per step (the pacing is the caller's, T2).
///
/// # Panics
/// If `k` > 16.
pub fn write_level(dac: &mut Palette, src: &Palette, k: u8) {
    assert!(k < LEVELS, "fade level {k} out of range");
    let scale = |v: u8| ((u16::from(v) * u16::from(k) + 8) / 16) as u8;
    for (d, s) in dac.colors.iter_mut().zip(&src.colors).skip(1) {
        *d = Rgb6 {
            r: scale(s.r),
            g: scale(s.g),
            b: scale(s.b),
        };
    }
}

/// Non-zero palette components from which a Real48 fade level takes two retrace frames instead
/// of one at preset P1. Spec: `docs/spec/flow/screens.md` rev 3 §2.1 (BUG-6): a level costs one
/// retrace wait plus 765 Real48 conversions, divisions and roundings, which are cheap on a zero
/// component; ≤ 10 non-zero components take 1 frame (C-0002 script choice and first title card,
/// C-0160), 677 take 2 (C-0170). ASSUMPTION(U-461): the crossover is about 380, the estimate
/// from the measured level times; no shipped flow fades a palette in between.
pub const SLOW_FADE_NONZERO: usize = 380;

/// The number of non-zero components (r, g, b counted separately) of entries `first..=255`.
#[must_use]
pub fn nonzero_components(pal: &Palette, first: usize) -> usize {
    pal.colors
        .iter()
        .skip(first)
        .map(|c| usize::from(c.r != 0) + usize::from(c.g != 0) + usize::from(c.b != 0))
        .sum()
}

/// Retrace frames each level of a 17-level fade of `pal` takes at preset P1, where the fade
/// writes entries `first..=255` (1 for the scene and menu fades and the flight `FadeIn`, 0 for
/// the flight `FadeOut`): 2 when at least [`SLOW_FADE_NONZERO`] of their components are
/// non-zero, else 1. Spec: screens rev 3 §2.1, flight-screens rev 5 §4.5.3, timing rev 4 §4.4
/// item 3. Fidelity: T2.
#[must_use]
pub fn frames_per_level(pal: &Palette, first: usize) -> u32 {
    if nonzero_components(pal, first) >= SLOW_FADE_NONZERO {
        2
    } else {
        1
    }
}

/// Retrace frames level `s` of a 17-level fade of `pal` occupies at preset P1 (entries
/// `first..=255` written): one for level 0, whose components are all zero (the Real48 work of
/// screens rev 3 §2.1 is cheap on zero operands), otherwise [`frames_per_level`].
///
/// Rule from the captures (M12-14, IM-21; screens rev 3 §2.1, flight-screens rev 5 §4.5.3):
/// each level starts at a retrace, and the next level waits for the first retrace after the
/// last write, so a fade of a picture palette takes 2 + 15 × 2 + 1 frames whichever way it
/// runs: C-0002 menu in f835.92 → 868.44, out f1027.92 → 1060.1; C-0012 Esc menu out
/// f3555.92 → 3588.1, in f3595.92 → 3628.44; C-0017 flight start f458.92 → 491.44 (the
/// fractions are where each `frames.tsv` frame's writes start and stop: level 0 writes its
/// 255 entries in 0.2 frame, a picture level in 1.5).
#[must_use]
pub fn level_frames(pal: &Palette, first: usize, s: u8) -> u32 {
    if s == 0 {
        1
    } else {
        frames_per_level(pal, first)
    }
}

/// Frames from the frame in which a fade starts to the first frame that shows its first level
/// `s` (16 for a fade-out, 0 for a fade-in): [`level_frames`] + 1. A level is shown from the
/// first frame that begins after all of its writes, and the first level's writes begin at the
/// retrace that ends the frame in which the fade was called. Every later level is shown
/// [`level_frames`] after the one before it; what follows the fade starts in the frame of its
/// last level. Measured (frame of the first complete DAC dump minus the frame the fade starts
/// in, levels in fade order): picture palette in 2, 4, …, 34 (C-0002 menu, C-0017 and C-0002
/// flight start, C-0120 menu), out 3, 5, …, 33, 34 (C-0002 and C-0120 menus, C-0012 Esc
/// menu); nearly black palette 2, 3, …, 18 (C-0002 title card). Fidelity: T2.
#[must_use]
pub fn first_level_frames(pal: &Palette, first: usize, s: u8) -> u32 {
    level_frames(pal, first, s) + 1
}

/// An in-progress palette fade.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fade {
    from: Palette,
    to: Palette,
    step: u32,
    steps: u32,
}

impl Fade {
    /// A fade from `from` to `to` over `steps` ticks (at least 1).
    pub fn new(from: Palette, to: Palette, steps: u32) -> Self {
        Self {
            from,
            to,
            step: 0,
            steps: steps.max(1),
        }
    }

    /// A fade from `pal` to black.
    pub fn out(pal: Palette, steps: u32) -> Self {
        Self::new(pal, Palette::default(), steps)
    }

    /// A fade from black to `pal`.
    pub fn r#in(pal: Palette, steps: u32) -> Self {
        Self::new(Palette::default(), pal, steps)
    }

    /// Advances one tick; returns the palette to show.
    pub fn tick(&mut self) -> Palette {
        self.step = (self.step + 1).min(self.steps);
        self.current()
    }

    /// The palette at the current step.
    pub fn current(&self) -> Palette {
        let lerp = |a: u8, b: u8| -> u8 {
            let (a, b) = (i32::from(a), i32::from(b));
            (a + (b - a) * self.step as i32 / self.steps as i32) as u8
        };
        let mut out = Palette::default();
        for ((o, f), t) in out
            .colors
            .iter_mut()
            .zip(&self.from.colors)
            .zip(&self.to.colors)
        {
            *o = Rgb6 {
                r: lerp(f.r, t.r),
                g: lerp(f.g, t.g),
                b: lerp(f.b, t.b),
            };
        }
        out
    }

    /// True once the target palette has been reached.
    pub fn done(&self) -> bool {
        self.step >= self.steps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_fade_levels_round_halves_up_and_skip_entry_0() {
        let mut src = Palette::default();
        src.colors[0] = Rgb6 {
            r: 63,
            g: 63,
            b: 63,
        };
        src.colors[1] = Rgb6 { r: 63, g: 1, b: 8 };
        let mut dac = Palette::default();
        write_level(&mut dac, &src, 8);
        // 63 × 8 / 16 = 31.5 -> 32; 1 × 8 / 16 = 0.5 -> 1; 8 × 8 / 16 = 4.
        assert_eq!(dac.colors[1], Rgb6 { r: 32, g: 1, b: 4 });
        assert_eq!(dac.colors[0], Rgb6::default(), "entry 0 is never written");
        write_level(&mut dac, &src, 1);
        // 63 / 16 = 3.94 -> 4; 1 / 16 -> 0; 8 / 16 = 0.5 -> 1.
        assert_eq!(dac.colors[1], Rgb6 { r: 4, g: 0, b: 1 });
        write_level(&mut dac, &src, 16);
        assert_eq!(dac.colors[1], src.colors[1]);
        write_level(&mut dac, &src, 0);
        assert_eq!(dac.colors[1], Rgb6::default());
    }

    #[test]
    fn fade_speed_follows_the_non_zero_components() {
        // screens rev 3 §2.1: the script choice's 7 components fade at 1 frame per level, a
        // picture palette at 2; the crossover (U-461) at 380.
        let mut pal = Palette::default();
        for (e, (r, g, b)) in [
            (1, (10, 10, 10)),
            (85, (0, 33, 0)),
            (92, (40, 0, 0)),
            (102, (63, 0, 0)),
            (103, (63, 0, 0)),
        ] {
            pal.colors[e] = Rgb6 { r, g, b };
        }
        assert_eq!(nonzero_components(&pal, 1), 7);
        assert_eq!(frames_per_level(&pal, 1), 1);
        let mut full = Palette::default();
        for c in full.colors.iter_mut().take(127) {
            *c = Rgb6 { r: 1, g: 2, b: 3 };
        }
        // Entries 0..126: 381 components, 378 of them from entry 1 on.
        assert_eq!(nonzero_components(&full, 0), 381);
        assert_eq!(frames_per_level(&full, 0), 2);
        assert_eq!(frames_per_level(&full, 1), 1, "entry 0 is not faded");
        full.colors[200].b = 5;
        full.colors[201].g = 5;
        assert_eq!(frames_per_level(&full, 1), 2, "380 from entry 1");
    }

    #[test]
    fn level_zero_takes_one_frame_and_the_first_level_one_more() {
        let mut pal = Palette::default();
        for c in pal.colors.iter_mut().skip(1) {
            *c = Rgb6 { r: 1, g: 2, b: 3 };
        }
        assert_eq!(level_frames(&pal, 1, 0), 1);
        assert_eq!(level_frames(&pal, 1, 1), 2);
        assert_eq!(level_frames(&pal, 1, 16), 2);
        assert_eq!(first_level_frames(&pal, 1, 0), 2, "fade-in");
        assert_eq!(first_level_frames(&pal, 1, 16), 3, "fade-out");
        // The frames each level is first shown in, counted from the fade's start: a fade-in
        // 2, 4, …, 34 and a fade-out 3, 5, …, 33, 34, as C-0002, C-0012, C-0017 and C-0120.
        let shown = |levels: &mut dyn Iterator<Item = u8>| {
            let mut at = 0;
            let mut out = Vec::new();
            for (i, s) in levels.enumerate() {
                at += if i == 0 {
                    first_level_frames(&pal, 1, s)
                } else {
                    level_frames(&pal, 1, s)
                };
                out.push(at);
            }
            out
        };
        let fade_in: Vec<u32> = (1..=17).map(|k| 2 * k).collect();
        assert_eq!(shown(&mut (0..=16)), fade_in);
        let mut fade_out: Vec<u32> = (1..=16).map(|k| 2 * k + 1).collect();
        fade_out.push(34);
        assert_eq!(shown(&mut (0..=16).rev()), fade_out);
        // A nearly black palette: 2, 3, …, 18 both ways (C-0002 f1063, f1409).
        let black = Palette::default();
        assert_eq!(first_level_frames(&black, 1, 16), 2);
        assert_eq!(level_frames(&black, 1, 5), 1);
    }

    #[test]
    fn fades_out_linearly() {
        let mut pal = Palette::default();
        pal.colors[1] = Rgb6 { r: 63, g: 32, b: 1 };
        let mut f = Fade::out(pal.clone(), 4);
        assert_eq!(f.current().colors[1], pal.colors[1]);
        assert_eq!(f.tick().colors[1], Rgb6 { r: 48, g: 24, b: 1 });
        f.tick();
        f.tick();
        assert!(!f.done());
        assert_eq!(f.tick().colors[1], Rgb6::default());
        assert!(f.done());
        assert_eq!(f.tick().colors[1], Rgb6::default(), "stays at the target");
    }
}
