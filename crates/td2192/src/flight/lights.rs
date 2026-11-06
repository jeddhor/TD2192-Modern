//! Light inputs of the flight picture (`docs/spec/enhanced/presentation.md` rev 1 §6.1).
//!
//! Placeholder until M12-11 (IM-13): no lights and no emissive colours, so the light map stage
//! has nothing to do.

use td_render::enhanced::scene::{Emissive, LightMode, LightScene};

use super::FlightArena;

impl FlightArena {
    /// The light sources and occluders of the current frame for a widescreen margin of
    /// `margin` pixels, or `None` while a modal screen is shown.
    pub(crate) fn light_scene(&self, margin: i32, mode: LightMode) -> Option<LightScene> {
        let _ = (margin, mode);
        None
    }

    /// The palette indices that glow at full brightness (built when the flight loads).
    pub(crate) fn emissive(&self) -> &Emissive {
        static NONE: Emissive = Emissive([false; 256]);
        &NONE
    }
}
