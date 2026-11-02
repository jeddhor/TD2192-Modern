//! SDL3 platform layer: window, presentation of the indexed framebuffer, input devices, audio
//! output and timing.
//!
//! SDL types never leave this crate (MDG §8.2). Everything here is thin glue; game logic lives
//! in the testable crates.

use thiserror::Error;

/// Errors raised by the platform layer.
#[derive(Debug, Error)]
pub enum PlatformError {
    /// SDL failed to initialise a subsystem.
    #[error("SDL error: {0}")]
    Sdl(String),
}

/// Version of the SDL library linked into the program, as `major.minor.patch`.
pub fn sdl_version() -> String {
    let v = sdl3::version::version();
    format!("{}.{}.{}", v.major, v.minor, v.patch)
}

#[cfg(test)]
mod tests {
    #[test]
    fn links_sdl3() {
        assert!(super::sdl_version().starts_with('3'));
    }
}
