//! L1 tests: parse every original file of the graphics formats decoded so far.
//! Data-gated: they skip when TD2192_DATA is not set.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use td_formats::map::TileMap;
use td_formats::palette::Palette;
use td_formats::screen::{PagedScreens, Screen};
use td_formats::sprites::SpriteSheet;
use td_formats::tileset::{TILE_COUNT, Tileset};
use td_test_support::require_data;

const SCREENS: &[&str] = &[
    "aboutus.scr",
    "bb.scr",
    "briefing.scr",
    "bunker.scr",
    "command.scr",
    "convers.scr",
    "dieseche.scr",
    "eggbase.scr",
    "elevator.scr",
    "greenblo.scr",
    "hangar.scr",
    "hangar2.scr",
    "helihang.scr",
    "hyde.scr",
    "hydedead.scr",
    "i-port.scr",
    "intro0.scr",
    "intro1.scr",
    "intro2.scr",
    "ionstat.scr",
    "kolorvci.scr",
    "maraud2.scr",
    "maraud3.scr",
    "marauder.scr",
    "melting.scr",
    "mining1.scr",
    "mining2.scr",
    "order.scr",
    "sbar.scr",
    "sel-pain.scr",
    "sewer.scr",
    "shooting.scr",
    "shuttle.scr",
    "shuttle0.scr",
    "soffice.scr",
    "tdlab.scr",
    "tdoffic2.scr",
    "velsroom.scr",
    "velvswlf.scr",
    "vhang.scr",
    "vidcom.scr",
    "vlab.scr",
    "voffice.scr",
    "v-port.scr",
    "city.td",
    "javelin.td",
];

fn read(dir: &Path, name: &str) -> Vec<u8> {
    fs::read(dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

#[test]
fn palettes_parse() {
    let dir = require_data!();
    for name in ["td.pal", "tdmen.pal"] {
        Palette::parse(&read(&dir, name)).unwrap();
    }
}

#[test]
fn all_full_screens_parse() {
    let dir = require_data!();
    assert_eq!(SCREENS.len(), 46);
    for name in SCREENS {
        let s = Screen::parse(&read(&dir, name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        let distinct: BTreeSet<u8> = s.pixels.iter().copied().collect();
        assert!(distinct.len() > 8, "{name} looks blank");
    }
}

#[test]
fn help_file_has_two_pages() {
    let dir = require_data!();
    let h = PagedScreens::parse(&read(&dir, "td.hlp")).unwrap();
    assert_eq!(h.pages.len(), 2);
}

#[test]
fn tilesets_parse() {
    let dir = require_data!();
    for (name, trailing) in [("td.blk", 0), ("td2.blk", 864), ("td3.blk", 864)] {
        let t = Tileset::parse(&read(&dir, name)).unwrap();
        assert_eq!(t.tiles.len(), TILE_COUNT, "{name}");
        assert_eq!(t.trailing.len(), trailing, "{name}");
    }
}

#[test]
fn maps_parse_and_reference_valid_tiles() {
    let dir = require_data!();
    // (file, distinct tile indices, highest index) as recorded in FORMATS.md §5.
    for (name, distinct, max) in [
        ("td.map", 65, 70),
        ("td2.map", 64, 68),
        ("td3.map", 41, 40),
        ("td4.map", 69, 70),
    ] {
        let m = TileMap::parse_episode(&read(&dir, name)).unwrap();
        let set: BTreeSet<u8> = m.cells().iter().copied().collect();
        assert_eq!(set.len(), distinct, "{name}");
        assert_eq!(*set.last().unwrap(), max, "{name}");
        assert!((max as usize) < TILE_COUNT);
    }
    // intro.map is 20 x 2000 (not a square map); its tiles stop at 73 (intro §3.1).
    let intro = read(&dir, "intro.map");
    assert_eq!(intro.len(), 20 * 2000);
    assert_eq!(intro.iter().max(), Some(&73));
}

#[test]
fn sprite_sheets_parse() {
    let dir = require_data!();
    let base = SpriteSheet::parse_base(&read(&dir, "td.shp")).unwrap();
    assert_eq!(base.len(), 20);
    for name in ["td.sc3", "td.shr", "td.sc2"] {
        let set = SpriteSheet::parse_rotation_set(&read(&dir, name)).unwrap();
        assert_eq!(set.len(), 24, "{name}");
    }
    assert_eq!(
        SpriteSheet::parse(&read(&dir, "mine.td"), 32)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        SpriteSheet::parse(&read(&dir, "lander.td"), 32)
            .unwrap()
            .len(),
        5
    );
    assert_eq!(
        SpriteSheet::parse(&read(&dir, "td.exp"), 32).unwrap().len(),
        7
    );
}

#[test]
fn portraits_stills_and_cell_sheets_parse() {
    use td_formats::picture::{CELL_SHEETS, Picture, STILLS, parse_cells};
    use td_formats::portraits::Portraits;
    let dir = require_data!();
    for (name, faces) in [
        ("newfaces.td", 26),
        ("newfaces.td2", 27),
        ("newfaces.td3", 27),
    ] {
        assert_eq!(
            Portraits::parse(&read(&dir, name)).unwrap().faces.len(),
            faces,
            "{name}"
        );
    }
    for layout in STILLS {
        let p = Picture::parse(&read(&dir, layout.name), layout)
            .unwrap_or_else(|e| panic!("{}: {e}", layout.name));
        let distinct: BTreeSet<u8> = p.pixels.iter().copied().collect();
        assert!(distinct.len() > 8, "{} looks blank", layout.name);
    }
    for layout in CELL_SHEETS {
        let cells = parse_cells(&read(&dir, layout.name), layout)
            .unwrap_or_else(|e| panic!("{}: {e}", layout.name));
        assert_eq!(cells.len(), layout.count);
    }
}

#[test]
fn fonts_parse_with_visible_glyphs() {
    use td_formats::font::{Font, FontKind};
    let dir = require_data!();
    for (name, kind) in [
        ("td.lt2", FontKind::Small),
        ("td-let.cin", FontKind::Title),
        ("td.ltr", FontKind::Large),
    ] {
        let f = Font::parse(&read(&dir, name), kind).unwrap();
        for c in ('a'..='z').chain('0'..='9') {
            let g = f
                .glyph(c)
                .unwrap_or_else(|| panic!("{name}: no glyph for {c}"));
            assert!(g.iter().any(|&p| p != 0), "{name}: glyph {c} is blank");
        }
    }
}

#[test]
fn map_orientation_matches_oracle_flight_frame() {
    // Tile indices visible in oracle capture C-0002 frame 3500 (E1M1 flight), read off the
    // frame by exact 32x32 tile matching; -1 = covered by the player's ship. The block's
    // top-left visible tile is (103, 104) in column-major order (FORMATS.md §5).
    const SEEN: [[i16; 9]; 5] = [
        [49, 50, 59, 62, 63, 64, 49, 50, 61],
        [50, 49, 61, 65, -1, 67, 49, 49, 61],
        [50, 49, 61, 68, -1, 70, 49, 50, 61],
        [49, 50, 61, 61, -1, 60, 50, 49, 50],
        [50, 49, 49, 49, 49, 49, 50, 50, 49],
    ];
    let dir = require_data!();
    let m = TileMap::parse_episode(&read(&dir, "td.map")).unwrap();
    for (r, row) in SEEN.iter().enumerate() {
        for (c, &t) in row.iter().enumerate() {
            if t >= 0 {
                assert_eq!(m.get(103 + c, 104 + r), Some(t as u8), "row {r} col {c}");
            }
        }
    }
}
