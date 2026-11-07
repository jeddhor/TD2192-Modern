//! Reduce flashing (accessibility, M12-18): limits how fast the colours on screen may change, at
//! the presentation stage only.
//!
//! Spec: `docs/spec/frontend/port-front-end.md` rev 5 §3.5 (design, T3; an accessibility option
//! outside the profile, off by default). Nothing here feeds back into the simulation: the flight
//! keeps its own DAC state (`flight_palette::FlightDac`) exact, and only what is presented is
//! limited, so the option can be switched at any time and replays are unaffected.
//!
//! Two tools, both in linear light (each channel 0..=1, here in 12-bit fixed point):
//!
//! - [`FlashLimiter`] moves every palette entry toward the colour the game asks for by at most
//!   [`RATE`] per second, measured as the sum of the three channels' changes (an L1 distance).
//!   A pixel's relative luminance (Rec. 709 weights, which sum to 1) and its red quantity
//!   max(0, R − G − B) then change by at most [`RATE`] per second as well. WCAG 2.3.1 counts a
//!   flash as a pair of opposing changes of at least 0.1 in luminance, or of more than 0.0625 in
//!   that red quantity for a saturated red (the `(R − G − B) × 320 > 20` rule); with 0.375 per
//!   second no one-second window can hold more than 1.9 general or 3 red flashes, whatever the
//!   game does. Changes faster than the limit become cross-fades; changes back and forth faster
//!   than it (a held infra-red key, which toggles the view 10.9 times a second) settle near the
//!   middle with a ripple of at most 0.034.
//! - [`cap_excursion`] keeps a flash colour within [`CAP`] (L1) of the steady colour, for flash
//!   steps whose picture changes around them (the flight-17 cut-scene), where a time limit would
//!   show the new picture in the old colours.

use std::sync::OnceLock;

use td_formats::palette::{Palette, Rgb6, expand6};

/// Full scale of the linear values.
const ONE: f64 = 4095.0;

/// The most the colours on screen may change per second: the L1 distance of linear RGB (each
/// channel 0..=1). See the module docs for why 0.375.
pub const RATE: f64 = 0.375;

/// The most a capped flash may differ from the steady colour: the L1 distance of linear RGB.
/// Below both WCAG thresholds (0.1 luminance, 0.0625 red), so a capped flash is no flash.
pub const CAP: f64 = 0.05;

/// 6-bit DAC values in 12-bit linear light.
struct Tables {
    lin: [u16; 64],
}

fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| {
        let decode = |c: f64| {
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        let lin = std::array::from_fn(|v| {
            (decode(f64::from(expand6(v as u8)) / 255.0) * ONE).round() as u16
        });
        Tables { lin }
    })
}

/// A 6-bit colour in linear light.
fn linear(c: Rgb6) -> [i32; 3] {
    let t = tables();
    [c.r, c.g, c.b].map(|v| i32::from(t.lin[usize::from(v.min(63))]))
}

/// The 6-bit colour of a linear one, each channel rounded toward `from`'s (so it is never
/// further from `from` than `l` is).
fn six_toward(l: [i32; 3], from: Rgb6) -> Rgb6 {
    let t = tables();
    let pick = |x: i32, f: u8| -> u8 {
        let x = x.clamp(0, ONE as i32);
        let f = f.min(63);
        if x >= i32::from(t.lin[usize::from(f)]) {
            // The largest value not above x (and not below f).
            (f..64)
                .rev()
                .find(|&v| i32::from(t.lin[usize::from(v)]) <= x)
                .unwrap_or(f)
        } else {
            // The smallest value not below x (and not above f).
            (0..=f)
                .find(|&v| i32::from(t.lin[usize::from(v)]) >= x)
                .unwrap_or(f)
        }
    };
    Rgb6 {
        r: pick(l[0], from.r),
        g: pick(l[1], from.g),
        b: pick(l[2], from.b),
    }
}

/// Moves `from` toward `to` by at most `budget` (L1, linear units); exact when within reach.
fn approach(from: [i32; 3], to: [i32; 3], budget: i32) -> [i32; 3] {
    let d = [0, 1, 2].map(|c| to[c] - from[c]);
    let l1: i32 = d.iter().map(|v| v.abs()).sum();
    if l1 <= budget {
        return to;
    }
    // Truncated toward zero: the move is at most `budget`.
    [0, 1, 2].map(|c| from[c] + d[c] * budget / l1)
}

/// One channel of the 6-bit colour shown for the linear value `x`, from `shown` (the 6-bit value
/// shown before): it moves only once `x` has passed a neighbouring 6-bit level, so a ripple
/// smaller than one level never shows, and every change of the 6-bit value is at most the
/// change of `x` (between turning points). It lands exactly on a 6-bit level that `x` reaches.
fn follow(shown: u8, x: i32) -> u8 {
    let t = tables();
    let lin = |v: u8| i32::from(t.lin[usize::from(v)]);
    let mut d = shown.min(63);
    while d < 63 && lin(d + 1) <= x {
        d += 1;
    }
    while d > 0 && lin(d - 1) >= x {
        d -= 1;
    }
    d
}

/// The palette on screen under the reduce-flashing option: each entry follows the palette the
/// game asks for at no more than [`RATE`] per second.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlashLimiter {
    /// The colours shown, linear.
    shown: [[i32; 3]; 256],
    /// The 6-bit colours shown ([`follow`]).
    out: Palette,
    /// The palette last asked for.
    target: Palette,
}

impl FlashLimiter {
    /// A limiter showing `palette`.
    #[must_use]
    pub fn new(palette: &Palette) -> Self {
        let mut l = Self {
            shown: [[0; 3]; 256],
            out: palette.clone(),
            target: palette.clone(),
        };
        l.snap(palette);
        l
    }

    /// Shows `palette` at once (a fade or a new picture: no flash, and the colours must match
    /// the pixels).
    pub fn snap(&mut self, palette: &Palette) {
        for (s, &c) in self.shown.iter_mut().zip(&palette.colors) {
            *s = linear(c);
        }
        self.out.clone_from(palette);
        self.target.clone_from(palette);
    }

    /// `dt` seconds have passed and the game now asks for `palette`: each entry moves toward it
    /// by at most [`RATE`] · `dt`.
    pub fn step(&mut self, palette: &Palette, dt: f64) {
        let budget = ((RATE * ONE * dt.max(0.0)).floor() as i32).max(1);
        for ((s, o), &c) in self
            .shown
            .iter_mut()
            .zip(self.out.colors.iter_mut())
            .zip(&palette.colors)
        {
            *s = approach(*s, linear(c), budget);
            *o = Rgb6 {
                r: follow(o.r, s[0]),
                g: follow(o.g, s[1]),
                b: follow(o.b, s[2]),
            };
        }
        self.target.clone_from(palette);
    }

    /// The colours shown: an entry that has reached its target shows exactly the target.
    #[must_use]
    pub fn palette(&self) -> Palette {
        self.out.clone()
    }

    /// True when every entry shows its target.
    #[must_use]
    pub fn settled(&self) -> bool {
        self.out == self.target
    }
}

/// `flash` kept within [`CAP`] of `steady` (linear L1): the colour a flash step shows under the
/// reduce-flashing option.
#[must_use]
pub fn cap_excursion(steady: Rgb6, flash: Rgb6) -> Rgb6 {
    let budget = (CAP * ONE).floor() as i32;
    let (s, f) = (linear(steady), linear(flash));
    if [0, 1, 2].map(|c| (f[c] - s[c]).abs()).iter().sum::<i32>() <= budget {
        return flash;
    }
    six_toward(approach(s, f, budget), steady)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flight_palette::{FlightDac, ir_palette};

    /// Relative luminance (Rec. 709, linear) of a 6-bit colour.
    fn luminance(c: Rgb6) -> f64 {
        let l = linear(c).map(|v| f64::from(v) / ONE);
        0.2126 * l[0] + 0.7152 * l[1] + 0.0722 * l[2]
    }

    /// WCAG's red quantity max(0, R − G − B), linear.
    fn red(c: Rgb6) -> f64 {
        let l = linear(c).map(|v| f64::from(v) / ONE);
        (l[0] - l[1] - l[2]).max(0.0)
    }

    fn rgb(r: u8, g: u8, b: u8) -> Rgb6 {
        Rgb6 { r, g, b }
    }

    /// A palette with every component value somewhere.
    fn test_palette() -> Palette {
        let mut p = Palette::default();
        for (i, c) in p.colors.iter_mut().enumerate() {
            *c = rgb((i % 64) as u8, ((i * 7) % 64) as u8, ((i * 13) % 64) as u8);
        }
        p
    }

    #[test]
    fn the_tables_round_trip_every_dac_value() {
        let t = tables();
        assert!(t.lin.windows(2).all(|w| w[0] < w[1]), "strictly increasing");
        for v in 0..64u8 {
            // Reached from anywhere, a 6-bit level is shown exactly.
            for from in [0, v / 2, v, 63] {
                assert_eq!(follow(from, i32::from(t.lin[usize::from(v)])), v);
            }
        }
        assert_eq!((t.lin[0], t.lin[63]), (0, 4095));
    }

    /// With nothing changing, the option changes nothing.
    #[test]
    fn a_steady_palette_is_shown_exactly() {
        let p = test_palette();
        let mut l = FlashLimiter::new(&p);
        assert_eq!(l.palette(), p);
        for _ in 0..50 {
            l.step(&p, 1.0 / 19.2015);
            assert_eq!(l.palette(), p);
        }
        assert!(l.settled());
        // A slow change is followed exactly once it is reached.
        let mut q = p.clone();
        q.colors[7] = rgb(40, 40, 40);
        for _ in 0..80 {
            l.step(&q, 1.0 / 19.2015);
        }
        assert_eq!(l.palette(), q);
    }

    /// The bound itself: per entry, the change in a second never exceeds RATE (L1, linear),
    /// whatever the target does.
    #[test]
    fn no_entry_changes_faster_than_the_rate() {
        let black = Palette::default();
        let mut white = Palette::default();
        white.colors.fill(rgb(63, 63, 63));
        for dt in [1.0 / 19.2015, 1.0 / 70.086] {
            let mut l = FlashLimiter::new(&black);
            let mut path = 0i64;
            let steps = (1.0 / dt) as usize;
            for i in 0..steps {
                let before = l.shown[5];
                l.step(if i % 2 == 0 { &white } else { &black }, dt);
                let after = l.shown[5];
                path += (0..3)
                    .map(|c| i64::from((after[c] - before[c]).abs()))
                    .sum::<i64>();
            }
            assert!(
                path as f64 <= RATE * ONE + 1.0,
                "{path} in one second at dt {dt}"
            );
        }
    }

    /// F1: the infra-red key held makes the game flip modes 1 and 2 about every 92 ms. Shown
    /// through the limiter the picture settles, and no transition reaches WCAG's 0.1 luminance
    /// or 0.0625 red.
    #[test]
    fn a_held_infra_red_key_does_not_flicker() {
        let p = test_palette();
        let (dark, bright) = (ir_palette(&p, 1), ir_palette(&p, 2));
        let mut l = FlashLimiter::new(&dark);
        let tick = 1.0 / 19.2015;
        let mut shown = Vec::new();
        // Typematic repeats every 91.7 ms: the mode flips on the ticks where a repeat falls.
        let mut mode2 = false;
        let mut next = 0.0;
        for i in 0..19 * 8 {
            let t = f64::from(i) * tick;
            if t >= next {
                mode2 = !mode2;
                next += 0.0917;
            }
            l.step(if mode2 { &bright } else { &dark }, tick);
            shown.push(l.palette());
        }
        // Without the limiter every entry that differs between the modes flashes 5.45 times a
        // second; with it, none does.
        let raw: Vec<f64> = (0..shown.len())
            .map(|i| if i % 4 < 2 { 0.0 } else { 0.5 })
            .collect();
        assert!(
            flashes(&raw, 0.1, tick) >= 4,
            "the counter sees the raw flicker"
        );
        for e in 0..256 {
            let lum: Vec<f64> = shown.iter().map(|p| luminance(p.colors[e])).collect();
            let reds: Vec<f64> = shown.iter().map(|p| red(p.colors[e])).collect();
            assert_eq!(flashes(&lum, 0.1, tick), 0, "entry {e}");
            assert_eq!(flashes(&reds, 0.0625, tick), 0, "entry {e}");
        }
    }

    /// The most flashes in any one-second window of `series` (one value per `dt`): WCAG's pairs
    /// of opposing transitions within the window, a transition being the change between a peak
    /// and the next trough (or back) when it is at least `threshold`.
    fn flashes(series: &[f64], threshold: f64, dt: f64) -> usize {
        // Turning points.
        let mut ext: Vec<(usize, f64)> = vec![(0, series[0])];
        for i in 1..series.len() {
            let (_, last) = *ext.last().unwrap_or(&(0, series[0]));
            let rising = series[i] > last;
            let prev_rising = ext.len() >= 2 && ext[ext.len() - 1].1 > ext[ext.len() - 2].1;
            if ext.len() >= 2 && rising == prev_rising && series[i] != last {
                *ext.last_mut().unwrap() = (i, series[i]);
            } else if series[i] != last {
                ext.push((i, series[i]));
            }
        }
        // (first, last) sample of each transition.
        let transitions: Vec<(usize, usize)> = ext
            .windows(2)
            .filter(|w| (w[1].1 - w[0].1).abs() >= threshold)
            .map(|w| (w[0].0, w[1].0))
            .collect();
        let window = (1.0 / dt).round() as usize;
        transitions
            .iter()
            .map(|&(t0, _)| {
                transitions
                    .iter()
                    .filter(|&&(a, b)| a >= t0 && b < t0 + window)
                    .count()
                    / 2
            })
            .max()
            .unwrap_or(0)
    }

    /// F3: a homing kill's white flash (four ticks of the DAC) and a strong hit flash are
    /// shown as small swells.
    #[test]
    fn white_and_hit_flashes_become_small_swells() {
        let p = test_palette();
        let mut dac = FlightDac::start(&p, 0);
        let mut l = FlashLimiter::new(dac.current());
        let tick = 1.0 / 19.2015;
        let mut most = 0.0f64;
        let steady = luminance(p.colors[20]);
        for frames in [4, 3, 2, 1, 0, 0, 0, 0, 0, 0] {
            dac.step(&p, 0, frames, false);
            l.step(dac.current(), tick);
            most = most.max((luminance(l.palette().colors[20]) - steady).abs());
        }
        assert!(most < 0.1, "white flash: {most}");
        dac.start_hit_flash(18);
        for _ in 0..12 {
            dac.step(&p, 0, 0, false);
            l.step(dac.current(), tick);
            most = most.max((luminance(l.palette().colors[20]) - steady).abs());
        }
        assert!(most < 0.1, "hit flash: {most}");
        for _ in 0..40 {
            l.step(dac.current(), tick);
        }
        assert_eq!(l.palette(), *dac.current(), "back to the game's palette");
    }

    #[test]
    fn a_capped_flash_stays_near_the_steady_colour() {
        let steady = rgb(20, 30, 10);
        for flash in [rgb(0, 0, 0), rgb(63, 63, 13), rgb(63, 0, 0)] {
            let c = cap_excursion(steady, flash);
            let d: i32 = (0..3)
                .map(|i| (linear(c)[i] - linear(steady)[i]).abs())
                .sum();
            assert!(f64::from(d) <= CAP * ONE, "{flash:?} -> {c:?}");
            assert!((luminance(c) - luminance(steady)).abs() < 0.1);
            assert!((red(c) - red(steady)).abs() < 0.0625);
        }
        // Within the cap nothing changes.
        assert_eq!(cap_excursion(steady, rgb(21, 30, 10)), rgb(21, 30, 10));
    }
}
