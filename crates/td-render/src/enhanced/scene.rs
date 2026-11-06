//! PLACEHOLDER (IM-13): IM-12 owns this file on its branch; replaced by theirs on merge.
//! The lighting inputs (`docs/spec/enhanced/presentation.md` rev 1 §6.1).

/// Lighting preset (§6.1). `off` is "no stage".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LightMode {
    #[default]
    Subtle,
    Full,
}

/// One light source, in base pixels of the W x 200 frame (margin included).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Light {
    pub pos: (f32, f32),
    pub color: [f32; 3],
    pub radius: f32,
    pub intensity: f32,
    pub shadows: bool,
}

/// Solid map cells: cell (i, j) covers `origin + 32·(i, j)`, 32 x 32 base pixels.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Occluders {
    pub origin: (i32, i32),
    pub cols: usize,
    pub rows: usize,
    pub solid: Vec<bool>,
}

/// Everything the light map is built from.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LightScene {
    pub mode: LightMode,
    pub lights: Vec<Light>,
    pub occluders: Occluders,
}

/// Palette indices that glow at full brightness and feed bloom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Emissive(pub [bool; 256]);

impl Default for Emissive {
    fn default() -> Self {
        Self([false; 256])
    }
}
