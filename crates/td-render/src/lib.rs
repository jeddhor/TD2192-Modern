//! Software renderer for Traffic Department 2192.
//!
//! All drawing targets an indexed 320x200 framebuffer with a 6-bit VGA palette (ADR-0004).
//! Conversion to RGBA and scaling happen only in `td-platform`. Implemented from
//! `docs/spec/render/`.
