//! Optional proportional stick thrust, frontend/analog-steering rev 1 (T3).
use crate::{Pad, PadButton};

/// One tick's original-sign axes: thrust may pulse, facing stays steady.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Steering {
    pub thrust_x: i8,
    pub thrust_y: i8,
    pub facing_x: i8,
    pub facing_y: i8,
}

/// Integer pulse-density conversion, sampled only when a simulation tick runs.
#[derive(Debug, Default)]
pub struct AnalogSteering {
    axes: [Axis; 2],
}

#[derive(Debug, Default)]
struct Axis {
    remainder: i32,
    sign: i8,
}

// Port tuning constants, not original-game facts; frontend/analog-steering rev 1.
const DEAD_ZONE: i32 = 8_000;
const RANGE: i32 = 32_767 - DEAD_ZONE;

impl Axis {
    fn sample(&mut self, raw: i16) -> (i8, i8) {
        let magnitude = i32::from(raw).abs().min(32_767);
        let sign = if magnitude <= DEAD_ZONE {
            0
        } else {
            -raw.signum() as i8
        };
        if sign != self.sign || sign == 0 {
            self.remainder = 0;
        }
        self.sign = sign;
        if sign == 0 {
            return (0, 0);
        }
        self.remainder += magnitude - DEAD_ZONE;
        let pulse = if self.remainder >= RANGE {
            self.remainder -= RANGE;
            sign
        } else {
            0
        };
        (pulse, sign)
    }
}

impl AnalogSteering {
    /// Clear fractional thrust on disable, disconnect, D-pad or keyboard takeover.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// D-pad directions take full-strength precedence over the stick. The caller handles
    /// keyboard takeover. Buttons and menus are unaffected.
    pub fn sample(&mut self, pad: Pad) -> Steering {
        let x = i8::from(pad.held(PadButton::DPadLeft)) - i8::from(pad.held(PadButton::DPadRight));
        let y = i8::from(pad.held(PadButton::DPadUp)) - i8::from(pad.held(PadButton::DPadDown));
        let dpad = [
            PadButton::DPadLeft,
            PadButton::DPadRight,
            PadButton::DPadUp,
            PadButton::DPadDown,
        ]
        .into_iter()
        .any(|b| pad.held(b));
        if dpad {
            self.reset();
            return Steering {
                thrust_x: x,
                thrust_y: y,
                facing_x: x,
                facing_y: y,
            };
        }
        let (thrust_x, facing_x) = self.axes[0].sample(pad.stick_x);
        let (thrust_y, facing_y) = self.axes[1].sample(pad.stick_y);
        Steering {
            thrust_x,
            thrust_y,
            facing_x,
            facing_y,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn proportion_and_facing_are_independent() {
        let mut a = AnalogSteering::default();
        let p = Pad {
            stick_x: 20_384,
            stick_y: -32_768,
            ..Pad::default()
        };
        let mut pulses = 0;
        for _ in 0..1000 {
            let s = a.sample(p);
            assert_eq!((s.facing_x, s.facing_y), (-1, 1));
            assert_eq!(s.thrust_y, 1);
            pulses += i32::from(s.thrust_x != 0);
        }
        assert_eq!(pulses, 500);
    }
    #[test]
    fn dead_zone_reversal_and_dpad_reset_fractional_thrust() {
        let mut a = AnalogSteering::default();
        for x in [-8000, 0, 8000] {
            assert_eq!(
                a.sample(Pad {
                    stick_x: x,
                    ..Pad::default()
                }),
                Steering::default()
            );
        }
        let p = Pad {
            stick_x: 20_384,
            ..Pad::default()
        };
        assert_eq!(a.sample(p).thrust_x, 0);
        assert_eq!(
            a.sample(Pad {
                stick_x: -20_384,
                ..p
            })
            .thrust_x,
            0
        );
        let d = a.sample(Pad {
            buttons: PadButton::DPadLeft.bit(),
            ..p
        });
        assert_eq!(
            (d.thrust_x, d.facing_x),
            (1, 1),
            "D-pad overrides opposite stick"
        );
        assert_eq!(a.sample(p).thrust_x, 0);
        a.reset();
        assert_eq!(a.sample(p).thrust_x, 0);
    }
}
