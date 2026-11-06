//! L5 parity: the sixteen heading blits (`docs/spec/flow/flight-screens.md` rev 3 §4.9, U-360,
//! test vector 20) against C-0080: E1 flight 2 started directly, the player (class 4) steering
//! kp4, kp2, kp6, kp8 in turn so that the heading runs anticlockwise through all 16 ring
//! entries; framebuffer and DGROUP dumps every frame f560..900.
//!
//! Which heading a frame shows: most dumps fall in the tick wait, after the tick has turned the
//! player (step 3) and drawn the back page, before the start address flips (step 22) and the
//! draw page `g_DrawPage` (DS:3A30) toggles (step 24). So the picture on screen was drawn by
//! the previous tick, whose heading `g_PlayerHeading` (DS:1A56) is the one in the dumps taken
//! before the last change of the draw page. A dump can also fall between the flip and the
//! toggle; it then shows this tick's heading, the one in its own dump.
//!
//! The 32 × 32 or 40 × 40 box at screen (144, 68) is compared with the port's frame for that
//! heading on the frame's opaque pixels. Needs TD2192_DATA and TD2192_ORACLE; skips otherwise.
//! No captured pixels are stored here.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use td_render::flight::SHIP_POS;
use td_render::headings::{CELL, HEADINGS, HeadingSprites, SpriteBuffers};
use td_test_support::{require_data, require_oracle};

/// The heading codes in `td_render::headings` order: anticlockwise from N (structures §7;
/// `td_sim::flight::heading::Heading::sprite_index`).
const BY_INDEX: [u8; HEADINGS] = [8, 78, 7, 74, 4, 14, 1, 12, 2, 32, 3, 36, 6, 96, 9, 98];

/// The player's class in flight 2.
const CLASS: usize = 4;

/// Test vector 20: frames where nothing covers the player, by heading code. The vector says
/// f658..899, but its counts (183 frames, 86 of heading 8) are those of f658..900: f900, the
/// last dump, shows heading 8 like f899 (reported as a spec erratum).
const EXPECTED: [(u8, usize); HEADINGS] = [
    (8, 86),
    (0x4E, 3),
    (0x07, 4),
    (0x4A, 4),
    (0x04, 14),
    (0x0E, 4),
    (0x01, 3),
    (0x0C, 4),
    (0x02, 18),
    (0x20, 7),
    (0x03, 4),
    (0x24, 3),
    (0x06, 15),
    (0x60, 4),
    (0x09, 6),
    (0x62, 4),
];

fn sprites(data: &Path) -> HeadingSprites {
    let read = |n: &str| fs::read(data.join(n)).unwrap();
    let (sc3, sc2, shr) = (read("td.sc3"), read("td.sc2"), read("td.shr"));
    let mut bank = read("td.shp");
    bank.extend(read("td.exp"));
    HeadingSprites::from_buffers(&SpriteBuffers::flight(&sc3, &sc2, &shr, &bank), 20)
}

/// Pixels of the model's opaque part that differ from the captured screen.
fn opaque_diff(model: &[u8], screen: &[u8]) -> usize {
    let (x0, y0) = (SHIP_POS.0 as usize, SHIP_POS.1 as usize);
    (0..CELL * CELL)
        .filter(|&i| {
            let v = model[i];
            v != 0 && screen[(y0 + i / CELL) * 320 + x0 + i % CELL] != v
        })
        .count()
}

#[test]
fn vector_20_every_heading_matches_c0080() {
    let data = require_data!();
    let oracle = require_oracle!();
    let hs = sprites(&data);
    let dir = oracle.join("C-0080");
    // (heading code, draw page) per dump.
    let mut ram = BTreeMap::new();
    for f in 600..=900u32 {
        let name = format!("f{f:08}_4467-0000_9DF0_ds.bin");
        let ds = fs::read(dir.join("mem").join(name)).unwrap();
        ram.insert(f, (ds[0x1A56], ds[0x3A30]));
    }
    let mut matched: BTreeMap<u8, usize> = BTreeMap::new();
    let mut covered = Vec::new();
    let mut flipped = Vec::new();
    for f in 658..=900u32 {
        let (now, page) = ram[&f];
        let before = (600..f).rev().find(|g| ram[g].1 != page).unwrap();
        let index = |code: u8| BY_INDEX.iter().position(|&c| c == code).unwrap();
        let name = format!("f{f:08}_320x192_fl.vga");
        let screen = &fs::read(dir.join("fb").join(name)).unwrap()[768..];
        let diffs: Vec<usize> = (0..HEADINGS)
            .map(|k| opaque_diff(hs.frame(CLASS, k).unwrap(), screen))
            .collect();
        // The previous tick's heading; or this tick's, when the dump fell between the flip of
        // the start address (step 22) and the page toggle (step 24).
        let (h_prev, h_now) = (index(ram[&before].0), index(now));
        let h = if diffs[h_now] < diffs[h_prev] {
            flipped.push(f);
            h_now
        } else {
            h_prev
        };
        let code = BY_INDEX[h];
        // Even where something covers the player, no other heading fits better.
        let best = (0..HEADINGS).min_by_key(|&k| diffs[k]).unwrap();
        assert_eq!(
            diffs[best], diffs[h],
            "f{f}: heading {code} shown, {} fits better",
            BY_INDEX[best]
        );
        if diffs[h] == 0 {
            *matched.entry(code).or_default() += 1;
            // Every other heading's model differs in at least 256 pixels.
            for (k, &d) in diffs.iter().enumerate() {
                assert!(
                    k == h || d >= 256,
                    "f{f}: heading {} differs in {d}",
                    BY_INDEX[k]
                );
            }
        } else {
            covered.push((f, code, diffs[h]));
        }
    }
    eprintln!(
        "vector 20: 0 differing pixels by heading {matched:?}; dumps between flip and toggle \
         {flipped:?}; covered {covered:?}"
    );
    assert!(flipped.len() <= 4, "{flipped:?}");
    assert_eq!(matched.values().sum::<usize>() + covered.len(), 243);
    for (code, n) in EXPECTED {
        assert_eq!(
            matched.get(&code).copied().unwrap_or(0),
            n,
            "frames of heading {code:#04X} with 0 differing pixels"
        );
    }
}
