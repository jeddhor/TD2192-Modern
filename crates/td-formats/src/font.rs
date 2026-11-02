//! The three bitmap fonts: `td.lt2` (5x5), `td-let.cin` (8x8) and `td.ltr` (12x16).
//!
//! Spec: `docs/re/FORMATS.md` §9. Fidelity: T0 for glyph pixels.
//! Glyph geometry is CONFIRMED by decoding; the character maps (which glyph a character uses)
//! are LIKELY, from the visible glyph order, until the text routines are analysed (M2-08).
//! The fonts have upper-case letters, digits and a few symbols only; game text is lower case
//! and maps onto the same glyphs. Glyph colours are baked into the pixels (index 0 =
//! transparent).

use crate::{FormatError, Result};

/// Which original font.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontKind {
    /// `td.lt2`: 38 glyphs of 5x5 (A-Z, two block glyphs, 0-9).
    Small,
    /// `td-let.cin`: 40 glyphs of 8x8 (A-Z, 1-9, 0, `-`, `?`, `!`, `:`).
    Title,
    /// `td.ltr`: 78 cells of 12x16 (A-Z, 1-9, 0, blank, `-`, then blank cells).
    Large,
}

impl FontKind {
    /// Glyph width and height in pixels.
    pub const fn size(self) -> (usize, usize) {
        match self {
            FontKind::Small => (5, 5),
            FontKind::Title => (8, 8),
            FontKind::Large => (12, 16),
        }
    }

    /// File size of the font.
    pub const fn file_len(self) -> usize {
        match self {
            FontKind::Small => 950,
            FontKind::Title => 2560,
            FontKind::Large => 14976,
        }
    }

    /// Glyph index for a character, if the font has one (letters are case-insensitive).
    pub fn glyph_index(self, c: char) -> Option<usize> {
        let c = c.to_ascii_lowercase();
        if c.is_ascii_lowercase() {
            return Some(c as usize - 'a' as usize);
        }
        match self {
            FontKind::Small => c.to_digit(10).map(|d| 28 + d as usize),
            FontKind::Title | FontKind::Large => {
                let digit = c
                    .to_digit(10)
                    .map(|d| if d == 0 { 35 } else { 25 + d as usize });
                digit.or(match (self, c) {
                    (FontKind::Title, '-') => Some(36),
                    (FontKind::Title, '?') => Some(37),
                    (FontKind::Title, '!') => Some(38),
                    (FontKind::Title, ':') => Some(39),
                    (FontKind::Large, '-') => Some(37),
                    _ => None,
                })
            }
        }
    }
}

/// A decoded font.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Font {
    pub kind: FontKind,
    /// Glyphs, each `width * height` row-major palette indices.
    pub glyphs: Vec<Box<[u8]>>,
}

impl Font {
    /// Parses a font file of the given kind.
    pub fn parse(data: &[u8], kind: FontKind) -> Result<Self> {
        FormatError::check_size("font", data, kind.file_len())?;
        let (w, h) = kind.size();
        Ok(Self {
            kind,
            glyphs: data.chunks_exact(w * h).map(Into::into).collect(),
        })
    }

    /// The glyph for a character.
    pub fn glyph(&self, c: char) -> Option<&[u8]> {
        self.kind
            .glyph_index(c)
            .and_then(|i| self.glyphs.get(i))
            .map(AsRef::as_ref)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn character_maps() {
        assert_eq!(FontKind::Small.glyph_index('a'), Some(0));
        assert_eq!(FontKind::Small.glyph_index('Z'), Some(25));
        assert_eq!(FontKind::Small.glyph_index('0'), Some(28));
        assert_eq!(FontKind::Small.glyph_index('9'), Some(37));
        assert_eq!(FontKind::Small.glyph_index('.'), None);
        assert_eq!(FontKind::Title.glyph_index('1'), Some(26));
        assert_eq!(FontKind::Title.glyph_index('0'), Some(35));
        assert_eq!(FontKind::Title.glyph_index(':'), Some(39));
        assert_eq!(FontKind::Large.glyph_index('-'), Some(37));
    }

    #[test]
    fn parses_by_kind() {
        let f = Font::parse(&[1u8; 2560], FontKind::Title).unwrap();
        assert_eq!(f.glyphs.len(), 40);
        assert_eq!(f.glyph('q').unwrap().len(), 64);
        assert!(Font::parse(&[0u8; 100], FontKind::Small).is_err());
    }
}
