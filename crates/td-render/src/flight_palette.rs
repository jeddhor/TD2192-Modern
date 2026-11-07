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
//!
//! Two original bugs are fix-enhanced (`docs/spec/original-bugs.md`; the host passes the
//! Enhanced profile's original-bug fixes): OB-032, the last white-flash frame restores nothing
//! in mode 2 ([`FlightDac::step`]); OB-023, E2's Alt from mode 2 back to mode 1 rewrites only
//! DAC 255 and 37 ([`FlightDac::toggle_e2_classic`]; the fix is [`FlightDac::toggle`]).

use td_formats::palette::{Palette, Rgb6};
use td_sim::flight::Episode;

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

    /// Tick-order step 14 (§4.5.4) of E1 in palette `mode`, with the white-flash counter
    /// `flash_frames` (`g_FlashFrames`) as the step read it (4 = first white frame .. 1 =
    /// restore, 0 = none): the hit-flash step, the cancel (a white flash ends the hit flash),
    /// then the white-flash writes. In mode 2 the last frame restores nothing (OB-032), unless
    /// `fix_ob032` (Enhanced), where it writes the mode-2 palette of the Alt toggle.
    pub fn step(&mut self, p: &Palette, mode: u8, flash_frames: u8, fix_ob032: bool) {
        self.hit_flash_step(p, mode);
        if flash_frames > 0 {
            self.hit_flash = 0;
        }
        self.white_flash_writes(p, mode, flash_frames, fix_ob032);
    }

    /// Tick-order step 14 of E2 and E3 (episode-deltas rev 5 §4.1, U-204, BUG-32): the
    /// white-flash writes, then the cancel, then the hit-flash step (`Gfx_HitFlashStep`, an
    /// exact copy of E1's block), which therefore writes nothing while a white flash runs. The
    /// result differs from E1's [`FlightDac::step`] only in white-flash frame 1 in palette mode
    /// 2 with a hit flash running (Classic): E1 leaves the hit-flash entries over frame 2's
    /// colours, E2 and E3 leave frame 2's colours.
    pub fn step_e2_e3(&mut self, p: &Palette, mode: u8, flash_frames: u8, fix_ob032: bool) {
        self.white_flash_writes(p, mode, flash_frames, fix_ob032);
        if flash_frames > 0 {
            self.hit_flash = 0;
        }
        self.hit_flash_step(p, mode);
    }

    /// Tick-order step 14 in `episode`'s order: [`FlightDac::step`] (E1) or
    /// [`FlightDac::step_e2_e3`].
    pub fn step_for(
        &mut self,
        episode: Episode,
        p: &Palette,
        mode: u8,
        flash_frames: u8,
        fix_ob032: bool,
    ) {
        match episode {
            Episode::E1 => self.step(p, mode, flash_frames, fix_ob032),
            Episode::E2 | Episode::E3 => self.step_e2_e3(p, mode, flash_frames, fix_ob032),
        }
    }

    /// The hit-flash block (§4.5.4; E2/E3 `Gfx_HitFlashStep`): decay, then the brightened
    /// entries, or the mode's palette when the flash ends.
    fn hit_flash_step(&mut self, p: &Palette, mode: u8) {
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
    }

    /// The white-flash DAC writes of frame `flash_frames` (4..=1; 0 writes nothing).
    fn white_flash_writes(&mut self, p: &Palette, mode: u8, flash_frames: u8, fix_ob032: bool) {
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
            // Mode 2 restores nothing (OB-032, preserved in Classic; fix-enhanced).
            1 if mode < 2 || fix_ob032 => self.dac = ir_palette(p, mode),
            _ => {}
        }
    }

    /// The Alt toggle into `new_mode` (1 or 2; tick-order step 33): the start formula of that
    /// mode, and the hit flash ends. E1 and E3 always; E2 under the Enhanced fixes (OB-023
    /// fixed, the full mode-1 palette).
    pub fn toggle(&mut self, p: &Palette, new_mode: u8) {
        self.dac = ir_palette(p, new_mode);
        self.hit_flash = 0;
    }

    /// E2's Alt toggle as the original runs it (Classic; OB-023, original-bugs.md;
    /// episode-deltas §4.1 step 31): into mode 2 as [`FlightDac::toggle`]; back into mode 1
    /// the per-entry loop has lost its body, so only DAC 255 and DAC 37 are rewritten and
    /// every other entry keeps its mode-2 colour until the next full write (a hit flash
    /// ending, a white flash, a fade); the hit flash ends.
    ///
    /// ASSUMPTION (U-530): DAC 255 gets the mode-1 start formula of P[255] (the write left
    /// after the loop, with the loop index at 255) and DAC 37 := P[255], as in the full
    /// toggle; the spec names the two entries, not their values.
    pub fn toggle_e2_classic(&mut self, p: &Palette, new_mode: u8) {
        if new_mode != 1 {
            self.toggle(p, new_mode);
            return;
        }
        let full = ir_palette(p, 1);
        self.dac.colors[IR_INK_SOURCE] = full.colors[IR_INK_SOURCE];
        self.dac.colors[IR_INK] = full.colors[IR_INK];
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

    /// E3's `FadeIn` writes entries **0**..=255 (episode-deltas rev 3 §6): as
    /// [`FlightDac::faded_in`], with entry 0 at the target too.
    pub fn faded_in_from_0(&mut self, p: &Palette, mode: u8) {
        let target = fade_target(p, mode);
        write_fade_step(&mut self.dac, &target, FADE_STEPS, 0);
        self.hit_flash = 0;
    }

    /// One step of E3 flight 16's magenta fade (`Msn_E3TimedEvents`, e3-mission-logic rev 1
    /// §4.6.4): step 1..=5 writes entries 1..=255 from the flight palette `p` as half, quarter,
    /// flat (63, 0, 63), quarter, half; entry 0 keeps its colour. Step 5 is the tint the game
    /// then runs in (OB-049).
    ///
    /// # Panics
    /// If `step` is not 1..=5.
    pub fn magenta_step(&mut self, p: &Palette, step: u8) {
        let shift = match step {
            1 | 5 => Some(1),
            2 | 4 => Some(2),
            3 => None,
            _ => panic!("magenta fade step {step} out of range"),
        };
        for (d, c) in self.dac.colors.iter_mut().zip(&p.colors).skip(1) {
            *d = shift.map_or(rgb(63, 0, 63), |k| magenta(*c, k));
        }
    }

    /// E3 flight 16 after phase 5: `UI_FlightMenu` re-tints every DAC entry, 0 included, after
    /// its closing fade-in (e3-mission-logic §4.7.2).
    pub fn magenta_tint_all(&mut self, p: &Palette) {
        for (d, c) in self.dac.colors.iter_mut().zip(&p.colors) {
            *d = magenta(*c, 1);
        }
    }
}

/// (63 − (63 − R) >> k, G >> k, 63 − (63 − B) >> k): the magenta wash of E3 flight 16
/// (k = 1 half, k = 2 quarter; unsigned arithmetic on 0..=63 components).
fn magenta(c: Rgb6, k: u32) -> Rgb6 {
    let up = |v: u8| 63 - ((63 - u16::from(v.min(63))) >> k);
    rgb(up(c.r), u16::from(c.g) >> k, up(c.b))
}

impl FlightDac {
    /// The number of magenta fade steps (one per vertical retrace).
    pub const MAGENTA_STEPS: u8 = 5;
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
        d.step(&p, 0, 0, false);
        assert_eq!(d.hit_flash(), 3);
        assert_eq!(d.current().colors[10], c(43, 23, 30));
        assert_eq!(d.current().colors[36], c(9, 9, 9), "36 skipped");
        assert_eq!(d.current().colors[50], c(8, 8, 8), "above 45 untouched");
        d.step(&p, 0, 0, false); // 1
        d.step(&p, 0, 0, false); // -1 -> 0: restore
        assert_eq!(d.hit_flash(), 0);
        assert_eq!(d.current(), &p);
        // Mode 1: decays by 3, ends with truncating shifts (B >> 3, not B / 4).
        let mut d = FlightDac::start(&p, 1);
        d.start_hit_flash(3);
        d.step(&p, 1, 0, false);
        assert_eq!(d.current().colors[10], c(5, 2, 3));
        // Mode 2: (3R) >> 1 when it ends; Round when entered by Alt.
        let p2 = pal(&[(5, (41, 0, 0))]);
        let mut d = FlightDac::start(&p2, 0);
        d.toggle(&p2, 2);
        assert_eq!(d.current().colors[5], c(62, 0, 0));
        d.start_hit_flash(10);
        d.step(&p2, 2, 0, false);
        assert_eq!(d.current().colors[5], c(63, 0, 0));
        d.start_hit_flash(1);
        d.hit_flash = 2;
        d.step(&p2, 2, 0, false);
        assert_eq!(d.current().colors[5], c(61, 0, 0));
    }

    #[test]
    fn white_flash_frames() {
        let p = pal(&[(10, (40, 20, 30))]);
        for fix in [false, true] {
            for mode in 0..=2 {
                let mut d = FlightDac::start(&p, mode);
                d.start_hit_flash(10);
                d.step(&p, mode, 4, fix);
                assert_eq!(d.hit_flash(), 0);
                assert_eq!(d.current().colors[10], c(63, 63, 63));
                d.step(&p, mode, 3, fix);
                assert_eq!(d.current().colors[10], c(63, 63, 30));
                d.step(&p, mode, 2, fix);
                assert_eq!(d.current().colors[10], c(63, 30, 30));
                d.step(&p, mode, 1, fix);
                let want = match (mode, fix) {
                    (0, _) => c(40, 20, 30),
                    (1, _) => c(5, 3, 8),
                    (_, false) => c(63, 30, 30), // OB-032: nothing restored (Classic)
                    (_, true) => c(60, 0, 0),    // OB-032 fixed: the mode-2 palette
                };
                assert_eq!(d.current().colors[10], want, "mode {mode}, fix {fix}");
            }
        }
    }

    /// BUG-32 (episode-deltas rev 5 §4.1): white-flash frame 1 in palette mode 2 (Classic)
    /// with a hit flash running. E1 decays and draws the hit flash before the frame, which
    /// writes nothing in mode 2 (OB-032), so the hit-flash entries stay over frame 2's colours;
    /// E2 and E3 write the frame, cancel the hit flash, and their hit-flash step then writes
    /// nothing: frame 2's colours stay.
    #[test]
    fn e2_e3_white_flash_runs_before_the_hit_flash() {
        let p = pal(&[(10, (40, 20, 30)), (50, (8, 8, 8))]);
        let run = |ep: Episode| {
            let mut d = FlightDac::start(&p, 2);
            for frames in [4, 3, 2] {
                d.step_for(ep, &p, 2, frames, false);
            }
            // A distance flash in the tick of frame 1.
            d.start_hit_flash(10);
            d.step_for(ep, &p, 2, 1, false);
            assert_eq!(
                d.hit_flash(),
                0,
                "{ep}: the white flash cancels the hit flash"
            );
            d
        };
        let e1 = run(Episode::E1);
        // Decay 10 → 8; mode 2 brightens (3R >> 1) + 8 = 68 → 63; nothing restores it.
        assert_eq!(e1.current().colors[10], c(63, 0, 0));
        assert_eq!(e1.current().colors[50], c(63, 8, 8), "above 45: frame 2");
        for ep in [Episode::E2, Episode::E3] {
            let d = run(ep);
            assert_eq!(
                d.current().colors[10],
                c(63, 30, 30),
                "{ep}: frame 2's colour"
            );
            assert_eq!(d.current().colors[50], c(63, 8, 8), "{ep}");
        }
    }

    /// Every other case of step 14 ends the same in both orders: no white flash, frames 4..2
    /// (all 256 entries rewritten), frame 1 in modes 0 and 1, frame 1 in mode 2 with the
    /// OB-032 fix, or no hit flash.
    #[test]
    fn step_orders_agree_but_for_frame_1_in_mode_2() {
        let mut p = Palette::default();
        for (i, c) in p.colors.iter_mut().enumerate() {
            let v = (i * 7 % 64) as u8;
            *c = Rgb6 {
                r: v,
                g: 63 - v,
                b: (i % 64) as u8,
            };
        }
        for mode in 0..=2 {
            for fix in [false, true] {
                for frames in 0..=4 {
                    for flash in [0, 1, 5, 25] {
                        let mut e1 = FlightDac::start(&p, mode);
                        e1.set_hit_flash(flash);
                        let mut e3 = e1.clone();
                        e1.step_for(Episode::E1, &p, mode, frames, fix);
                        e3.step_for(Episode::E3, &p, mode, frames, fix);
                        assert_eq!(e1.hit_flash(), e3.hit_flash());
                        let differs = mode == 2 && frames == 1 && !fix && flash > 0;
                        assert_eq!(
                            e1 != e3,
                            differs,
                            "mode {mode}, fix {fix}, frames {frames}, flash {flash}"
                        );
                    }
                }
            }
        }
    }

    /// OB-023 (BUG-9): E2 Classic leaves every entry but 255 and 37 red after Alt back into
    /// mode 1; the full toggle (E1, E3, E2 with the Enhanced fix) writes the whole mode-1
    /// palette. Into mode 2 both write everything.
    #[test]
    fn e2_alt_back_to_mode_1_rewrites_only_255_and_37() {
        let p = pal(&[(10, (40, 20, 30)), (37, (8, 8, 8)), (255, (50, 40, 36))]);
        let mut full = FlightDac::start(&p, 1);
        let mut e2 = FlightDac::start(&p, 1);
        full.toggle(&p, 2);
        e2.toggle_e2_classic(&p, 2);
        assert_eq!(full, e2, "into mode 2: the whole palette in every episode");
        assert_eq!(e2.current().colors[10], c(60, 0, 0));
        full.start_hit_flash(5);
        e2.start_hit_flash(5);
        full.toggle(&p, 1);
        e2.toggle_e2_classic(&p, 1);
        assert_eq!(full.current(), &ir_palette(&p, 1));
        assert_eq!(full.current().colors[10], c(5, 3, 8));
        // E2 Classic: entry 10 keeps its mode-2 red; 255 and 37 take the mode-1 values.
        assert_eq!(e2.current().colors[10], c(60, 0, 0));
        assert_eq!(e2.current().colors[255], c(6, 5, 9));
        assert_eq!(e2.current().colors[IR_INK], c(50, 40, 36));
        assert_eq!((full.hit_flash(), e2.hit_flash()), (0, 0));
        // The red stays until a full write: here a hit flash ending in mode 1.
        e2.start_hit_flash(3);
        e2.step(&p, 1, 0, false);
        assert_eq!(e2.current().colors[10], c(5, 2, 3));
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
    fn magenta_fade_and_tint() {
        // e3-mission-logic §4.6.4 examples: (0, 0, 0) and (63, 40, 10).
        let p = pal(&[(0, (0, 0, 0)), (1, (0, 0, 0)), (2, (63, 40, 10))]);
        let mut d = FlightDac::start(&p, 0);
        let want = [
            (c(32, 0, 32), c(63, 20, 37)),
            (c(48, 0, 48), c(63, 10, 50)),
            (c(63, 0, 63), c(63, 0, 63)),
            (c(48, 0, 48), c(63, 10, 50)),
            (c(32, 0, 32), c(63, 20, 37)),
        ];
        for (k, w) in (1..=FlightDac::MAGENTA_STEPS).zip(want) {
            d.magenta_step(&p, k);
            assert_eq!(
                (d.current().colors[1], d.current().colors[2]),
                w,
                "step {k}"
            );
            assert_eq!(
                d.current().colors[0],
                c(0, 0, 0),
                "entry 0 is never written"
            );
        }
        // The white flash's last frame restores the untinted colours (§4.7.3).
        d.step(&p, 0, 1, false);
        assert_eq!(d.current(), &p);
        // After the Esc menu the tint covers entry 0 as well (OB-049).
        d.faded_in_from_0(&p, 0);
        d.magenta_tint_all(&p);
        assert_eq!(d.current().colors[0], c(32, 0, 32));
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
