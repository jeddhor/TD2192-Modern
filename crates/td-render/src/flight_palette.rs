//! In-flight palettes: the infra-red modes, the per-tick hit and white flashes, and the
//! flight fades.
//!
//! Spec: `docs/spec/flow/flight-screens.md` rev 1 §4.5. Fidelity: T0 for the formulas
//! (CONFIRMED by C-0014 for the flight start, the Alt toggle and the fade after the computer
//! menu; LIKELY for the flash formulas).
//!
//! The DAC is state: which IR formula is on screen depends on how mode 1 or 2 was last entered
//! (flight start and Alt use B/4 and keep entry 37 readable, the flight fades use B/8, the end
//! of a hit flash truncates; OB-029). [`FlightDac`] holds that state; the flight host updates
//! it at the points the spec names and draws with [`FlightDac::current`].
//!
//! `P` below is the flight palette `g_FlightPalette` (`td.pal` as the flight patched it); the
//! modes never change it. "Round" is the Real48 `Round` of an exact quotient (halves up).

use td_formats::palette::{Palette, Rgb6};

/// Brightness steps of a flight fade: s = 0..=16 (§4.5.3).
pub const FADE_STEPS: u8 = 16;

/// Palette entry of the IR notice and pause ink, kept at the normal colour of
/// [`IR_INK_SOURCE`] by the mode-1 formula of the flight start and Alt (§4.5.1).
pub const IR_INK: usize = 37;
/// Entry whose normal colour [`IR_INK`] takes in mode 1.
pub const IR_INK_SOURCE: usize = 255;

/// Highest entry a hit flash brightens, and the entries it skips (§4.5.4; the code's other
/// exclusions 85h, 86h, C1h, 90h are above 45 and have no effect, U-225).
const HIT_FLASH_LAST: usize = 45;
const HIT_FLASH_SKIP: [usize; 2] = [36, 38];

fn rgb(r: u16, g: u16, b: u16) -> Rgb6 {
    // Callers keep every component within 0..=63.
    Rgb6 {
        r: r as u8,
        g: g as u8,
        b: b as u8,
    }
}

/// Round(v / d) for v ≥ 0 (halves up).
fn round_div(v: u16, d: u16) -> u16 {
    (2 * v + d) / (2 * d)
}

/// Mode 2 red: min(63, Round(1.5 R)).
fn red_rounded(r: u8) -> u16 {
    round_div(3 * u16::from(r), 2).min(63)
}

/// Mode 2 red when a hit flash ends: min(63, (3R) >> 1).
fn red_truncated(r: u8) -> u16 {
    ((3 * u16::from(r)) >> 1).min(63)
}

fn map(p: &Palette, f: impl Fn(Rgb6) -> Rgb6) -> Palette {
    let mut out = p.clone();
    for c in &mut out.colors {
        *c = f(*c);
    }
    out
}

/// The infra-red DAC of the flight start and of the Alt toggle (§4.5.1, first two rows):
/// mode 1 is (Round(R/8), Round(G/8), Round(B/4)) with entry 37 := P[255]; mode 2 is
/// (min(63, Round(1.5 R)), 0, 0). Mode 0 is `p` itself.
#[must_use]
pub fn ir_palette(p: &Palette, mode: u8) -> Palette {
    match mode {
        1 => {
            let mut out = map(p, |c| {
                rgb(
                    round_div(c.r.into(), 8),
                    round_div(c.g.into(), 8),
                    round_div(c.b.into(), 4),
                )
            });
            out.colors[IR_INK] = p.colors[IR_INK_SOURCE];
            out
        }
        2 => map(p, |c| rgb(red_rounded(c.r), 0, 0)),
        _ => p.clone(),
    }
}

/// The target of a flight fade (`FadeIn`/`FadeOut`, §4.5.1 rows 3–4): mode 1 is
/// (Round(R/8), Round(G/8), Round(B/8)) with no entry-37 fix; mode 2 as [`ir_palette`];
/// mode 0 is `p`.
#[must_use]
pub fn fade_target(p: &Palette, mode: u8) -> Palette {
    match mode {
        1 => map(p, |c| {
            rgb(
                round_div(c.r.into(), 8),
                round_div(c.g.into(), 8),
                round_div(c.b.into(), 8),
            )
        }),
        _ => ir_palette(p, mode),
    }
}

/// One step of a flight fade (§4.5.3): DAC entries `first..=255` := Round(component · s / 16)
/// of `target`. `FadeIn` runs s = 0..=16 over entries 1..=255, `FadeOut` s = 16..=0 over
/// entries 0..=255.
///
/// # Panics
/// If `s` > 16.
pub fn write_fade_step(dac: &mut Palette, target: &Palette, s: u8, first: usize) {
    assert!(s <= FADE_STEPS, "fade step {s} out of range");
    let scale = |v: u8| round_div(u16::from(v) * u16::from(s), 16);
    for (d, t) in dac.colors.iter_mut().zip(&target.colors).skip(first) {
        *d = rgb(scale(t.r), scale(t.g), scale(t.b));
    }
}

/// The DAC as the flight shows it, with the hit-flash level (`g_HitFlash`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlightDac {
    dac: Palette,
    hit_flash: u16,
}

impl FlightDac {
    /// The DAC after the flight's first fade-in: a flight starting in mode 1 gets the
    /// flight-start formula ([`ir_palette`], `0E8B:CAD2`); mode 0 shows `p`.
    #[must_use]
    pub fn start(p: &Palette, mode: u8) -> Self {
        Self {
            dac: ir_palette(p, mode),
            hit_flash: 0,
        }
    }

    /// The colours on screen.
    pub fn current(&self) -> &Palette {
        &self.dac
    }

    /// The hit-flash level.
    pub fn hit_flash(&self) -> u16 {
        self.hit_flash
    }

    /// Starts or strengthens a hit flash (the distance flash raises it to `max(flash, v)`,
    /// weapons §7).
    pub fn start_hit_flash(&mut self, v: u16) {
        self.hit_flash = self.hit_flash.max(v);
    }

    /// Sets the hit-flash level to the simulation's `g_HitFlash` as tick-order step 14 found
    /// it (`td_sim::flight::DrawnFrame::hit_flash`), before [`FlightDac::step`] decays and
    /// draws it (sim/hit-flash.md §4.3).
    pub fn set_hit_flash(&mut self, v: u16) {
        self.hit_flash = v;
    }

    /// Tick-order step 14 (§4.5.4) in palette `mode`, with the white-flash counter
    /// `flash_frames` (`g_FlashFrames`) as the step read it (4 = first white frame .. 1 =
    /// restore, 0 = none).
    pub fn step(&mut self, p: &Palette, mode: u8, flash_frames: u8) {
        if self.hit_flash > 0 && mode <= 2 {
            let decay = if mode == 1 { 3 } else { 2 };
            self.hit_flash = self.hit_flash.wrapping_sub(decay);
            // u16 compare: a level that went below 0 also ends the flash.
            if self.hit_flash > 30 {
                self.hit_flash = 0;
            }
            let f = self.hit_flash;
            if f == 0 {
                self.dac = match mode {
                    0 => p.clone(),
                    1 => map(p, |c| {
                        rgb((c.r >> 3).into(), (c.g >> 3).into(), (c.b >> 3).into())
                    }),
                    _ => map(p, |c| rgb(red_truncated(c.r), 0, 0)),
                };
            } else {
                for i in (0..=HIT_FLASH_LAST).filter(|i| !HIT_FLASH_SKIP.contains(i)) {
                    let c = p.colors[i];
                    let add = |v: u16| (v + f).min(63);
                    self.dac.colors[i] = match mode {
                        0 => rgb(add(c.r.into()), add(c.g.into()), c.b.into()),
                        1 => rgb(
                            add((c.r >> 3).into()),
                            add((c.g >> 3).into()),
                            (c.b >> 3).into(),
                        ),
                        _ => rgb(add(red_truncated(c.r)), 0, 0),
                    };
                }
            }
        }
        if flash_frames > 0 {
            self.hit_flash = 0;
        }
        match flash_frames {
            4 => self.dac.colors.fill(rgb(63, 63, 63)),
            3 => {
                for (d, c) in self.dac.colors.iter_mut().zip(&p.colors) {
                    *d = rgb(63, 63, c.b.into());
                }
            }
            2 => {
                for (d, c) in self.dac.colors.iter_mut().zip(&p.colors) {
                    *d = rgb(63, c.b.into(), c.b.into());
                }
            }
            // Mode 2 restores nothing (OB-032, preserved).
            1 if mode < 2 => self.dac = ir_palette(p, mode),
            _ => {}
        }
    }

    /// The Alt toggle into `new_mode` (1 or 2; tick-order step 33): the start formula of that
    /// mode, and the hit flash ends.
    pub fn toggle(&mut self, p: &Palette, new_mode: u8) {
        self.dac = ir_palette(p, new_mode);
        self.hit_flash = 0;
    }

    /// The end of a flight `FadeIn` in `mode` (after the computer menu): entries 1..=255 at
    /// the fade target, entry 0 black (the fade-out before it cleared it), and the hit flash
    /// ends.
    pub fn faded_in(&mut self, p: &Palette, mode: u8) {
        let target = fade_target(p, mode);
        self.dac.colors[0] = Rgb6::default();
        write_fade_step(&mut self.dac, &target, FADE_STEPS, 1);
        self.hit_flash = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pal(entries: &[(usize, (u8, u8, u8))]) -> Palette {
        let mut p = Palette::default();
        for &(i, (r, g, b)) in entries {
            p.colors[i] = Rgb6 { r, g, b };
        }
        p
    }

    fn c(r: u8, g: u8, b: u8) -> Rgb6 {
        Rgb6 { r, g, b }
    }

    #[test]
    fn mode_1_start_formula_and_entry_37() {
        // Test vector 7: P = (40, 20, 30) -> (5, 3, 8).
        let p = pal(&[(10, (40, 20, 30)), (37, (1, 2, 3)), (255, (50, 60, 61))]);
        let ir = ir_palette(&p, 1);
        assert_eq!(ir.colors[10], c(5, 3, 8));
        assert_eq!(ir.colors[37], c(50, 60, 61), "entry 37 keeps P[255]");
        assert_eq!(ir.colors[255], c(6, 8, 15));
    }

    #[test]
    fn mode_2_formula() {
        // Test vector 8: R = 41 -> Round(61.5) = 62; 63 caps.
        let p = pal(&[(5, (41, 20, 30)), (6, (63, 1, 1))]);
        let ir = ir_palette(&p, 2);
        assert_eq!(ir.colors[5], c(62, 0, 0));
        assert_eq!(ir.colors[6], c(63, 0, 0));
    }

    #[test]
    fn fade_target_uses_b_over_8_and_no_entry_37_fix() {
        let p = pal(&[(10, (40, 20, 30)), (37, (63, 47, 61)), (255, (50, 60, 61))]);
        let t = fade_target(&p, 1);
        assert_eq!(t.colors[10], c(5, 3, 4));
        assert_eq!(t.colors[37], c(8, 6, 8));
        assert_eq!(fade_target(&p, 0), p);
    }

    #[test]
    fn hit_flash_decays_and_restores_per_mode() {
        let p = pal(&[(10, (40, 20, 30)), (36, (9, 9, 9)), (50, (8, 8, 8))]);
        let mut d = FlightDac::start(&p, 0);
        d.start_hit_flash(5);
        d.step(&p, 0, 0);
        assert_eq!(d.hit_flash(), 3);
        assert_eq!(d.current().colors[10], c(43, 23, 30));
        assert_eq!(d.current().colors[36], c(9, 9, 9), "36 skipped");
        assert_eq!(d.current().colors[50], c(8, 8, 8), "above 45 untouched");
        d.step(&p, 0, 0); // 1
        d.step(&p, 0, 0); // -1 -> 0: restore
        assert_eq!(d.hit_flash(), 0);
        assert_eq!(d.current(), &p);
        // Mode 1: decays by 3, ends with truncating shifts (B >> 3, not B / 4).
        let mut d = FlightDac::start(&p, 1);
        d.start_hit_flash(3);
        d.step(&p, 1, 0);
        assert_eq!(d.current().colors[10], c(5, 2, 3));
        // Mode 2: (3R) >> 1 when it ends; Round when entered by Alt.
        let p2 = pal(&[(5, (41, 0, 0))]);
        let mut d = FlightDac::start(&p2, 0);
        d.toggle(&p2, 2);
        assert_eq!(d.current().colors[5], c(62, 0, 0));
        d.start_hit_flash(10);
        d.step(&p2, 2, 0);
        assert_eq!(d.current().colors[5], c(63, 0, 0));
        d.start_hit_flash(1);
        d.hit_flash = 2;
        d.step(&p2, 2, 0);
        assert_eq!(d.current().colors[5], c(61, 0, 0));
    }

    #[test]
    fn white_flash_frames() {
        let p = pal(&[(10, (40, 20, 30))]);
        for mode in 0..=2 {
            let mut d = FlightDac::start(&p, mode);
            d.start_hit_flash(10);
            d.step(&p, mode, 4);
            assert_eq!(d.hit_flash(), 0);
            assert_eq!(d.current().colors[10], c(63, 63, 63));
            d.step(&p, mode, 3);
            assert_eq!(d.current().colors[10], c(63, 63, 30));
            d.step(&p, mode, 2);
            assert_eq!(d.current().colors[10], c(63, 30, 30));
            d.step(&p, mode, 1);
            let want = match mode {
                0 => c(40, 20, 30),
                1 => c(5, 3, 8),
                _ => c(63, 30, 30), // OB-032: nothing restored
            };
            assert_eq!(d.current().colors[10], want, "mode {mode}");
        }
    }

    #[test]
    fn fades_round_halves_up() {
        let target = pal(&[(0, (63, 63, 63)), (1, (63, 1, 9))]);
        let mut dac = Palette::default();
        write_fade_step(&mut dac, &target, 8, 1);
        assert_eq!(dac.colors[0], c(0, 0, 0), "entry 0 not written by FadeIn");
        assert_eq!(dac.colors[1], c(32, 1, 5));
        write_fade_step(&mut dac, &target, 16, 0);
        assert_eq!(dac, target);
    }

    #[test]
    fn faded_in_after_the_menu() {
        let p = pal(&[(0, (1, 1, 1)), (37, (63, 47, 61)), (255, (50, 60, 61))]);
        let mut d = FlightDac::start(&p, 1);
        assert_eq!(d.current().colors[37], c(50, 60, 61));
        d.faded_in(&p, 1);
        assert_eq!(d.current().colors[37], c(8, 6, 8), "OB-029");
        assert_eq!(d.current().colors[0], c(0, 0, 0));
    }
}
