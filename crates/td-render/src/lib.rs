//! Software renderer for Traffic Department 2192.
//!
//! All drawing targets an indexed 320x200 [`Framebuffer`] with a 6-bit VGA palette (ADR-0004).
//! Conversion to RGBA and scaling happen only in `td-platform`. Behaviour is implemented from
//! `docs/spec/render/` and `docs/re/FORMATS.md`. The opt-in Enhanced presentation (wider
//! frames, upscaling, lighting; ADR-0006) is the [`enhanced`] module.

pub mod dialogue;
pub mod enhanced;
pub mod fade;
pub mod flight;
pub mod flight_palette;
pub mod framebuffer;
pub mod headings;
pub mod hud;
pub mod small_text;
pub mod text;
pub mod tilemap;
pub mod title_text;

pub use fade::Fade;
pub use framebuffer::{Framebuffer, HEIGHT, Image, Rect, WIDTH};
pub use headings::{HEADINGS, HeadingSprites};
pub use text::{TextColor, draw_text, text_width};
pub use tilemap::draw_tile_map;
