//! The three bitmap fonts: `td.lt2` (5x5), `td-let.cin` (8x8) and `td.ltr` (12x16).
//!
//! Spec: `docs/re/FORMATS.md` §9 and `docs/spec/render/text.md` (rev 1) §3. Fidelity: T0 for
//! glyph pixels and character maps (both CONFIRMED). The files carry baked colours, but the game
//! recolours every glyph when it draws (`td-render::small_text`, `td-render::title_text`), so
//! the stored values only matter as "ink or not" (small font: non-zero; title font: `F7h`).
//! `td.ltr` is never opened by the game (U-017).

use crate::{FormatError, Result};

/// Which original font.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontKind {
    /// `td.lt2`: 38 glyphs of 5x5 (a-z, two solid blocks, 0-9).
    Small,
    /// `td-let.cin`: 40 glyphs of 8x8 (A-Z, 1-9, 0, `-`, `?`, `!`, `:`).
    Title,
    /// `td.ltr`: 78 cells of 12x16 (A-Z, 1-9, 0, blank, `-`, then blank cells). Unused.
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

    /// Glyph index for a character, if the font's table has one.
    ///
    /// * Small (text.md §3.1): `a`..`z` → 0..25, `{` → 26, `|` → 27 (the typing cursor),
    ///   `0`..`9` → 28..37. Case-sensitive: upper case and punctuation have no glyph (the six
    ///   marks `. , " ? - !` are drawn by code, text.md §3.3).
    /// * Title (text.md §3.4): the character is upper-cased first (`UpCase`, only `a`..`z`
    ///   change), then `A`..`Z` → 0..25, `1`..`9` → 26..34, `0` → 35, `-` → 36, `?` → 37,
    ///   `!` → 38, `:` → 39 (present in the file, but the game never loads it).
    /// * Large: same order as the title font for letters and digits, `-` → 37 (unused font).
    pub fn glyph_index(self, c: char) -> Option<usize> {
        match self {
            FontKind::Small => match c {
                'a'..='z' => Some(c as usize - 'a' as usize),
                '{' => Some(26),
                '|' => Some(27),
                '0'..='9' => Some(28 + (c as usize - '0' as usize)),
                _ => None,
            },
            FontKind::Title | FontKind::Large => {
                let c = c.to_ascii_uppercase();
                match (self, c) {
                    (_, 'A'..='Z') => Some(c as usize - 'A' as usize),
                    (_, '1'..='9') => Some(26 + (c as usize - '1' as usize)),
                    (_, '0') => Some(35),
                    (FontKind::Title, '-') => Some(36),
                    (FontKind::Title, '?') => Some(37),
                    (FontKind::Title, '!') => Some(38),
                    (FontKind::Title, ':') => Some(39),
                    (FontKind::Large, '-') => Some(37),
                    _ => None,
                }
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
        let small = FontKind::Small;
        assert_eq!(small.glyph_index('a'), Some(0));
        assert_eq!(small.glyph_index('z'), Some(25));
        assert_eq!(small.glyph_index('{'), Some(26));
        assert_eq!(small.glyph_index('|'), Some(27));
        assert_eq!(small.glyph_index('0'), Some(28));
        assert_eq!(small.glyph_index('9'), Some(37));
        for c in ['Z', 'A', '.', ',', '"', '!', '?', '-', ':', ' ', '\''] {
            assert_eq!(small.glyph_index(c), None, "{c:?}");
        }
        let title = FontKind::Title;
        assert_eq!(title.glyph_index('a'), Some(0), "UpCase first");
        assert_eq!(title.glyph_index('Z'), Some(25));
        assert_eq!(title.glyph_index('1'), Some(26));
        assert_eq!(title.glyph_index('9'), Some(34));
        assert_eq!(title.glyph_index('0'), Some(35));
        assert_eq!(title.glyph_index('-'), Some(36));
        assert_eq!(title.glyph_index('?'), Some(37));
        assert_eq!(title.glyph_index('!'), Some(38));
        assert_eq!(title.glyph_index(':'), Some(39));
        assert_eq!(title.glyph_index('.'), None);
        assert_eq!(title.glyph_index(' '), None);
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
