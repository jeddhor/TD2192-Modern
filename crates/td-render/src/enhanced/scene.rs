//! Inputs of the lighting and bloom stages (`docs/spec/enhanced/presentation.md` rev 1 §6.1),
//! built each frame by the flight screen.
//!
//! These types are the interface between the pipeline skeleton (M12-08) and the lighting and
//! bloom stages (M12-11, M12-12), which live in sibling modules. Positions are base pixels of the
//! composed W x 200 frame, widescreen margin included (x = Classic x + m).

/// How strong the lighting is (`lighting = "subtle" | "full"`; "off" skips the stages).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LightMode {
    /// Daylight ambient, no shadows; lights only add glow (§6.1).
    #[default]
    Subtle,
    /// Night ambient with soft shadows from solid cells (§6.1).
    Full,
}

/// One light source (§6.1 table).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Light {
    /// Centre in base pixels of the W x 200 frame.
    pub pos: (f32, f32),
    /// Linear RGB colour.
    pub color: [f32; 3],
    /// Radius in base pixels.
    pub radius: f32,
    pub intensity: f32,
    /// Whether solid cells cast shadows from this light.
    pub shadows: bool,
}

/// The solid flags of the map cells around the drawn window (§6.1 occluder grid).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Occluders {
    /// Base-pixel position of cell (0, 0)'s top-left corner; cell (i, j) covers
    /// `origin + 32 * (i, j)`, 32 x 32 pixels.
    pub origin: (i32, i32),
    pub cols: usize,
    pub rows: usize,
    /// Row-major, `cols * rows` flags.
    pub solid: Vec<bool>,
}

/// Everything the light map needs for one frame.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LightScene {
    pub mode: LightMode,
    pub lights: Vec<Light>,
    pub occluders: Occluders,
}

/// Palette indices that glow at full brightness and feed bloom (§6.1), built once when a flight
/// loads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Emissive(pub [bool; 256]);

impl Default for Emissive {
    fn default() -> Self {
        Self([false; 256])
    }
}
