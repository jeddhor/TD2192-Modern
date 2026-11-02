//! Palette fades in the VGA 6-bit domain.
//!
//! Fidelity: provisional. ASSUMPTION(M5-02): a fade moves every component linearly from the
//! source to the target palette over `steps` ticks, truncating toward the source
//! (`src + (dst - src) * step / steps`). The original's step count, rate and rounding come from
//! its DAC-writing routines (`docs/spec/render/fades.md`, to be written).

use td_formats::palette::{Palette, Rgb6};

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
