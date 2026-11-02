//! Software renderer for Traffic Department 2192.
//!
//! All drawing targets an indexed 320x200 [`Framebuffer`] with a 6-bit VGA palette (ADR-0004).
//! Conversion to RGBA and scaling happen only in `td-platform`. Behaviour is implemented from
//! `docs/spec/render/` and `docs/re/FORMATS.md`.

pub mod framebuffer;
pub mod headings;
pub mod tilemap;

pub use framebuffer::{Framebuffer, HEIGHT, Image, Rect, WIDTH};
pub use headings::{HEADINGS, HeadingSprites};
pub use tilemap::draw_tile_map;
