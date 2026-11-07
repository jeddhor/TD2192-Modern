//! Headerless still images and fixed-geometry UI graphics.
//!
//! Spec: `docs/re/FORMATS.md` §11 and §12. Fidelity: T0.
//! The files carry no dimensions; the sizes below were derived from the data (row-to-row
//! similarity plus exact size fits, then visually confirmed) and are LIKELY until the loaders
//! are confirmed in the executables (M1-05). Some stills start with a 768-byte palette; two do
//! not and are drawn with the current palette.

use crate::palette::{PALETTE_LEN, Palette};
use crate::{FormatError, Result};

/// Geometry of a known still image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StillLayout {
    pub name: &'static str,
    pub width: usize,
    pub height: usize,
    /// Whether the file starts with its own 768-byte palette.
    pub has_palette: bool,
}

/// Every still image of the original game.
pub const STILLS: &[StillLayout] = &[
    still("td1-13.pic", 202, 86, true),
    still("td1-14.pic", 200, 200, true),
    still("td1-15.pic", 255, 121, true),
    still("td1-16.pic", 150, 170, true),
    still("td1-17.pic", 320, 90, false),
    still("td1-18.pic", 170, 104, true),
    still("td1-19.pic", 320, 72, false),
    still("td1-20.pic", 320, 100, true),
    still("secret.pic", 320, 32, true),
    still("pic4.td", 100, 150, true),
];

const fn still(name: &'static str, width: usize, height: usize, has_palette: bool) -> StillLayout {
    StillLayout {
        name,
        width,
        height,
        has_palette,
    }
}

/// Geometry of a known sheet of equally sized UI or sprite cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellLayout {
    pub name: &'static str,
    pub cell_width: usize,
    pub cell_height: usize,
    pub count: usize,
}

/// Fixed-geometry graphics (drawn with the in-flight palette `td.pal` unless noted in
/// FORMATS.md). `td.men`, `td.cmm`, `td.sel`, `td.wea` and `td.ltr` need code analysis (M2-09).
pub const CELL_SHEETS: &[CellLayout] = &[
    cells("td.win", 320, 32, 1),   // in-flight HUD bar
    cells("td.sss", 320, 19, 1),   // save-slot bar
    cells("td.fac", 28, 28, 5),    // "HIT F6" comm indicator animation
    cells("td.joy", 40, 40, 1),    // joystick icon
    cells("td.h2", 40, 40, 6),     // helicopter rotor frames
    cells("td.hel", 32, 32, 2),    // helicopter bodies
    cells("td.dun", 32, 32, 3),    // intro desert tiles
    cells("td1.ccm", 32, 32, 9),   // comm portraits, episode 1
    cells("td2.ccm", 32, 32, 7),   // comm portraits, episode 2
    cells("td3.ccm", 32, 32, 5),   // comm portraits, episode 3
    cells("mine.td", 32, 32, 1),   // mine
    cells("lander.td", 32, 32, 5), // lander pieces
    cells("td.exp", 32, 32, 7),    // explosion frames
];

const fn cells(
    name: &'static str,
    cell_width: usize,
    cell_height: usize,
    count: usize,
) -> CellLayout {
    CellLayout {
        name,
        cell_width,
        cell_height,
        count,
    }
}

/// A decoded still image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    /// The image's own palette, if the file has one.
    pub palette: Option<Palette>,
    pub width: usize,
    pub height: usize,
    pub pixels: Box<[u8]>,
}

impl Picture {
    /// Parses a still with the given layout; the file length must match exactly.
    pub fn parse(data: &[u8], layout: &StillLayout) -> Result<Self> {
        let pal_len = if layout.has_palette { PALETTE_LEN } else { 0 };
        // A caller's layout may be absurd (BUG-15): an overflowing size matches no input.
        let size = layout
            .width
            .checked_mul(layout.height)
            .and_then(|p| p.checked_add(pal_len))
            .ok_or_else(|| too_large("picture"))?;
        FormatError::check_size("picture", data, size)?;
        Ok(Self {
            palette: layout
                .has_palette
                .then(|| Palette::parse_prefix(data))
                .transpose()?,
            width: layout.width,
            height: layout.height,
            pixels: data[pal_len..].into(),
        })
    }

    /// The layout of a known still, by file name.
    pub fn layout(name: &str) -> Option<&'static StillLayout> {
        STILLS.iter().find(|s| s.name.eq_ignore_ascii_case(name))
    }
}

/// The layout of a known cell sheet, by file name.
pub fn cell_layout(name: &str) -> Option<&'static CellLayout> {
    CELL_SHEETS
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case(name))
}

/// Splits a cell sheet into its cells (row-major pixels each).
pub fn parse_cells(data: &[u8], layout: &CellLayout) -> Result<Vec<Box<[u8]>>> {
    // A caller's layout may be absurd: an empty cell (BUG-14) or an overflowing size (BUG-15).
    let cell = layout
        .cell_width
        .checked_mul(layout.cell_height)
        .filter(|&c| c > 0)
        .ok_or_else(|| too_large("cell sheet"))?;
    let size = cell
        .checked_mul(layout.count)
        .ok_or_else(|| too_large("cell sheet"))?;
    FormatError::check_size("cell sheet", data, size)?;
    Ok(data.chunks_exact(cell).map(Into::into).collect())
}

/// The error for a layout whose size is empty or does not fit in `usize`.
fn too_large(format: &'static str) -> FormatError {
    FormatError::Invalid {
        format,
        offset: 0,
        detail: "the layout's size is empty or overflows".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_with_and_without_palette() {
        let with = Picture::layout("PIC4.TD").unwrap();
        let mut data = vec![0u8; PALETTE_LEN + 100 * 150];
        data[PALETTE_LEN] = 7;
        let p = Picture::parse(&data, with).unwrap();
        assert!(p.palette.is_some());
        assert_eq!((p.width, p.height, p.pixels[0]), (100, 150, 7));
        let without = Picture::layout("td1-17.pic").unwrap();
        let p = Picture::parse(&vec![1u8; 320 * 90], without).unwrap();
        assert!(p.palette.is_none());
        assert!(Picture::parse(&[0u8; 10], without).is_err());
    }

    #[test]
    fn splits_cells() {
        let l = cell_layout("td.fac").unwrap();
        let cells = parse_cells(&vec![3u8; 28 * 28 * 5], l).unwrap();
        assert_eq!(cells.len(), 5);
        assert!(parse_cells(&[0u8; 100], l).is_err());
    }

    #[test]
    fn absurd_layouts_are_errors() {
        // BUG-14: an empty cell; BUG-15: sizes that overflow (M12-06 fuzzing).
        let empty = CellLayout {
            name: "x",
            cell_width: 0,
            cell_height: 0,
            count: 0,
        };
        assert!(parse_cells(&[], &empty).is_err());
        let huge = CellLayout {
            name: "x",
            cell_width: usize::MAX,
            cell_height: 2,
            count: 1,
        };
        assert!(parse_cells(&[], &huge).is_err());
        let many = CellLayout {
            name: "x",
            cell_width: 1 << 40,
            cell_height: 1,
            count: 1 << 40,
        };
        assert!(parse_cells(&[], &many).is_err());
        let still = StillLayout {
            name: "x",
            width: usize::MAX / 4 + 1,
            height: 5,
            has_palette: true,
        };
        assert!(Picture::parse(&[], &still).is_err());
    }
}
