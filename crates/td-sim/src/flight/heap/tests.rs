//! The flight heap's path arrays (mission-ships rev 6 §4.1.1, U-500): writer layout on
//! synthetic files, and the step-1 tiles of the spec's two vectors from the installation.

use super::*;
use td_formats::mission::MISSION_FILE_LEN;

#[test]
fn free_size_words_and_rounding() {
    // §4.1.1 writer table: 976 bytes → 00 00 3D 00, 16,000 bytes → 00 00 E8 03.
    assert_eq!(free_size_word(FONT_BLOCK_LEN), [0x00, 0x00, 0x3D, 0x00]);
    assert_eq!(free_size_word(PORTRAIT_LEN), [0x00, 0x00, 0xE8, 0x03]);
    assert_eq!(heap_round(975), FONT_BLOCK_LEN);
    assert_eq!(heap_round(1_980), 1_984, "hangar.cmf (vector 2)");
    assert_eq!(heap_round(12_299), 12_304, "vulture.cmf (vector 2)");
    assert_eq!(heap_round(0), 0, "no song");
}

#[test]
fn writes_land_only_in_the_arrays() {
    let mut m = PathMemory::new();
    // Across the start of X: only the bytes from M + 9,328 on.
    m.write(PATH_X_AT - 3, &[1, 2, 3, 4, 5]);
    assert_eq!(&m.x()[..3], &[4, 5, 0]);
    // Across the end of X, the 4-byte gap and the start of Y.
    let bytes: Vec<u8> = (1..=10).collect();
    m.write(PATH_X_AT + PATH_ARRAY_LEN - 3, &bytes);
    assert_eq!(&m.x()[PATH_ARRAY_LEN - 3..], &[1, 2, 3]);
    assert_eq!(PATH_Y_AT - (PATH_X_AT + PATH_ARRAY_LEN), 4, "the gap");
    assert_eq!(&m.y()[..4], &[8, 9, 10, 0]);
    // Past the end of Y: nothing.
    m.write(PATH_Y_AT + PATH_ARRAY_LEN, &[7; 8]);
    assert_eq!(m.y().len(), PATH_ARRAY_LEN);
}

/// A mission file whose X steps are 10 + slot and Y steps 20 + slot, with slot 2's steps 1..3
/// FFh in both arrays.
fn mission() -> Mission {
    let mut b = vec![0u8; MISSION_FILE_LEN];
    for s in 0..SLOTS {
        for k in 1..=PATH_LEN {
            let at = s * PATH_STRIDE + k - 1;
            if at < 10_000 {
                b[80 + at] = 10 + s as u8;
                b[10_080 + at] = 20 + s as u8;
            }
        }
    }
    for k in 1..=3 {
        b[80 + PATH_STRIDE + k - 1] = 0xFF;
        b[10_080 + PATH_STRIDE + k - 1] = 0xFF;
    }
    Mission::parse(&b).unwrap()
}

#[test]
fn a_mission_load_fills_both_arrays_with_slot_2_fixed() {
    let mut m = PathMemory::new();
    m.write(PATH_Y_AT, &[9; PATH_ARRAY_LEN]);
    m.load_mission(&mission());
    assert_eq!(m.step(0, 1), (10, 20));
    assert_eq!(m.step(0, 0), (0, 0), "element (1, 0) is not in the file");
    assert_eq!(m.step(1, 1), (0, 0), "slot 2's FFh become 0");
    assert_eq!(m.step(1, 4), (11, 21));
    assert_eq!(m.step(19, 481), (29, 39));
    assert_eq!(m.step(19, 482), (0, 0), "past the file: 0");
}

/// Synthetic installation files: `low.cmf` 4,293 bytes of 0x41, `vulture.cmf` 12,299 of 0x56,
/// `td.lt2` 950 of 0x7F, `newfaces.td` two faces with pixels 5 (face 1) and 200 (face 2).
fn read(name: &str) -> Result<Vec<u8>, String> {
    Ok(match name {
        "low.cmf" => vec![0x41; 4_293],
        "vulture.cmf" => vec![0x56; 12_299],
        "hangar.cmf" => vec![0x48; 1_980],
        FONT_FILE => vec![0x7F; FONT_READ_LEN],
        PORTRAIT_FILE => {
            let mut f = vec![0u8; 2 * PORTRAIT_RECORD_LEN];
            f[768..PORTRAIT_RECORD_LEN].fill(5);
            f[PORTRAIT_RECORD_LEN + 768..].fill(200);
            f
        }
        _ => return Err(name.to_owned()),
    })
}

/// The byte at heap offset `at` (inside one of the arrays).
fn at(m: &PathMemory, at: usize) -> u8 {
    if at >= PATH_Y_AT {
        m.y()[at - PATH_Y_AT]
    } else {
        m.x()[at - PATH_X_AT]
    }
}

#[test]
fn block_19_to_the_voffice_card_layers_as_vector_1() {
    // §4.1.1 vector 1 (C-0200): X = vulture.cmf to 12,298, the debrief portrait, the voffice
    // font block with its size field, the debrief portrait again; Y = the debrief portrait to
    // 21,271, then m19.td's Y block.
    let mut m = PathMemory::new();
    m.load_mission(&mission());
    m.apply(&block_19_to_voffice(true), read).unwrap();
    let face1_right = 5 + FACE_COLOURS;
    assert_eq!(at(&m, PATH_X_AT), 0x56);
    assert_eq!(at(&m, 12_298), 0x56, "the song's last byte");
    for o in 12_299..12_304 {
        assert_eq!(at(&m, o), face1_right, "{o}: debrief portrait");
    }
    // The debrief's font block at r(low) = 4,296 lies below the arrays; voffice's at 12,304.
    assert_eq!(at(&m, 12_304), 0x7F);
    assert_eq!(
        [12_308, 12_309, 12_310, 12_311].map(|o| at(&m, o)),
        [0x00, 0x00, 0x3D, 0x00]
    );
    assert_eq!(at(&m, 13_253), 0x7F, "the font's last byte");
    assert_eq!(at(&m, 13_254), face1_right, "the block's unwritten tail");
    assert_eq!(at(&m, 21_271), face1_right, "the portrait's last byte");
    // Y offset 21,272 is memory index 1,920: slot index 3 (bytes 1,503..2,003).
    assert_eq!(at(&m, 21_272), 20 + 3, "m19.td's Y block from there");
    // The debrief portrait's own size field (4,296 + 976 + 4) is below the arrays.
}

#[test]
fn music_off_puts_the_blocks_at_the_mark() {
    // U-540 (static): no song, so the debrief's font is at M and its portrait at 976..16,975.
    let mut m = PathMemory::new();
    m.load_mission(&mission());
    m.apply(&block_19_to_voffice(false), read).unwrap();
    assert_eq!(m.x()[0], 5 + FACE_COLOURS);
    assert_eq!(m.x()[16_975 - PATH_X_AT], 5 + FACE_COLOURS);
    assert_eq!(
        m.x()[16_976 - PATH_X_AT],
        10 + 15,
        "m19.td from 16,976 (slot 16)"
    );
}

#[test]
fn portraits_drop_high_pixels_and_move_right_ones_up() {
    let mut m = PathMemory::new();
    let left = PortraitLoad {
        face: 2,
        right: false,
    };
    m.apply(&[HeapUse::Screen(vec![left])], read).unwrap();
    // No song: the portrait at 976..16,975; face 2's pixels (200) become 0 on the left.
    assert_eq!(m.x()[0], 0);
    let right = PortraitLoad { face: 2, ..left };
    let right = PortraitLoad {
        right: true,
        ..right
    };
    m.apply(&[HeapUse::Screen(vec![right])], read).unwrap();
    assert_eq!(m.x()[0], FACE_COLOURS);
    // A face past the end of the file loads nothing.
    let mut n = PathMemory::new();
    let none = PortraitLoad {
        face: 3,
        right: true,
    };
    n.apply(&[HeapUse::Screen(vec![none])], read).unwrap();
    assert_eq!(n.x()[0], 0);
}

/// Flight 21's step-1 tiles of slots 1..10 after `uses`, from the installation's files.
fn step1_tiles(uses: &[HeapUse]) -> Option<Vec<(u8, u8)>> {
    let data = td_test_support::data_dir()?;
    let dir = td_assets::DataDir::open(&data).unwrap();
    let read = |f: &str| dir.read(f);
    let mut m = PathMemory::new();
    m.load_mission(&Mission::parse(&read("m19.td").unwrap()).unwrap());
    m.apply(uses, read).unwrap();
    Some((0..10).map(|s| m.step(s, 1)).collect())
}

#[test]
fn flight_21_vector_1_c0200() {
    // §4.1.1 vector 1: the debrief times out, Esc at the voffice card.
    let Some(tiles) = step1_tiles(&block_19_to_voffice(true)) else {
        eprintln!("SKIPPED: TD2192_DATA not set");
        return;
    };
    assert_eq!(
        tiles,
        [
            (0, 112),
            (0, 137),
            (0, 164),
            (69, 164),
            (145, 64),
            (0, 59),
            (0, 108),
            (247, 88),
            (133, 88),
            (130, 75)
        ]
    );
}

#[test]
fn flight_21_vector_2_c0300() {
    // §4.1.1 vector 2: the whole story, D88 (faces 24 left, 11 right) and D89 (1 left, 2 right).
    let p = |face, right| PortraitLoad { face, right };
    let mut uses = block_19_to_voffice(true);
    uses.extend([
        HeapUse::Screen(vec![p(24, false), p(11, true)]),
        HeapUse::Song("hangar.cmf".to_owned()),
        HeapUse::card(),
        HeapUse::Screen(vec![p(1, false), p(2, true)]),
    ]);
    let Some(tiles) = step1_tiles(&uses) else {
        eprintln!("SKIPPED: TD2192_DATA not set");
        return;
    };
    assert_eq!(
        tiles,
        [
            (118, 112),
            (112, 112),
            (112, 112),
            (122, 221),
            (112, 220),
            (133, 138),
            (134, 112),
            (112, 112),
            (120, 112),
            (112, 112)
        ]
    );
}
