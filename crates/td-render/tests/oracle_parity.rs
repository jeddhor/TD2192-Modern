//! L5 parity tests: our text and dialogue drawing compared pixel for pixel with frames captured
//! from the original game (oracle capture C-0002, `docs/re/dynamic-observations.md`). They need
//! both TD2192_DATA and TD2192_ORACLE and skip otherwise. No captured pixels are stored in the
//! repository.
//!
//! These are component checks of `docs/spec/render/text.md` §6 and
//! `docs/spec/narrative/dialogue-player.md` §6. Whole dialogue frames, played by the scene
//! player, are checked in `td-flow/tests/dialogue_parity.rs`.

use std::fs;
use std::path::Path;

use td_assets::{DataDir, ExeResources};
use td_formats::font::{Font, FontKind};
use td_formats::palette::Palette;
use td_formats::portraits::Portraits;
use td_render::dialogue::{self, Side};
use td_render::{Framebuffer, small_text, title_text};
use td_test_support::{require_data, require_oracle};

/// A C-0002 framebuffer dump: the DAC palette and 320x200 pixels.
fn oracle_frame(oracle: &Path, name: &str) -> (Palette, Vec<u8>) {
    let raw = fs::read(oracle.join("C-0002").join("fb").join(name)).unwrap();
    assert_eq!(raw.len(), 768 + 320 * 200, "{name}");
    (Palette::parse_prefix(&raw).unwrap(), raw[768..].to_vec())
}

const F2283: &str = "f00002283_320x200_dialogue-1.vga";
const F2433: &str = "f00002433_320x200_dialogue-2.vga";

/// Pixels that differ in rows `rows`, as (x, y, ours, original), at most 12 listed.
fn diff_rows(ours: &[u8], theirs: &[u8], rows: std::ops::Range<usize>) -> (usize, Vec<String>) {
    let idx: Vec<usize> = (rows.start * 320..rows.end * 320)
        .filter(|&i| ours[i] != theirs[i])
        .collect();
    let first = idx
        .iter()
        .take(12)
        .map(|&i| format!("({}, {}): {} vs {}", i % 320, i / 320, ours[i], theirs[i]))
        .collect();
    (idx.len(), first)
}

fn small_font(data: &Path) -> Font {
    Font::parse(&fs::read(data.join("td.lt2")).unwrap(), FontKind::Small).unwrap()
}

#[test]
fn typed_dialogue_line_matches_the_original() {
    // text.md §6 vector 4 / dialogue-player.md §6 vector 5: the second line of scene 1 (two
    // leading spaces), typed at pen (2, 175) in entry 251 and scrolled by two lines.
    let data = require_data!();
    let oracle = require_oracle!();
    let font = small_font(&data);
    let raw = fs::read(data.join("dialogue.dat")).unwrap();
    let lines = td_formats::dialogue::read_scene(&raw, 1).unwrap();
    let text = lines
        .iter()
        .find_map(|c| match c {
            td_formats::dialogue::Command::Line { text, .. } if text.trim().len() > 1 => {
                Some(text.clone())
            }
            _ => None,
        })
        .unwrap();
    let mut fb = Framebuffer::new();
    let mut t = small_text::Typewriter::new(&text, dialogue::TEXT_PEN, 251, 0);
    t.finish(&mut fb, &font);
    dialogue::scroll_text_area(&mut fb);
    dialogue::scroll_text_area(&mut fb);
    let (_, frame) = oracle_frame(&oracle, F2283);
    let (n, first) = diff_rows(&fb.pixels, &frame, 162..167);
    assert_eq!(n, 0, "rows 162-166 differ: {first:?}");
}

#[test]
fn portraits_names_and_palette_match_the_original() {
    // dialogue-player.md §6 vectors 1, 3 and 4; text.md §6 vector 2.
    let data = require_data!();
    let oracle = require_oracle!();
    let font = small_font(&data);
    let faces = Portraits::parse(&fs::read(data.join("newfaces.td")).unwrap()).unwrap();
    let td_pal = Palette::parse(&fs::read(data.join("td.pal")).unwrap()).unwrap();
    // Scene 1: slot 1 = face 1, colour 2Eh, 13-character name; slot 2 = face 2, colour 39h.
    let raw = fs::read(data.join("dialogue.dat")).unwrap();
    let cmds = td_formats::dialogue::read_scene(&raw, 1).unwrap();
    let mut fb = Framebuffer::new();
    let mut pal = Palette::default();
    let mut entry = 250u8;
    for c in &cmds {
        if let td_formats::dialogue::Command::Speaker {
            slot,
            face,
            color,
            name,
        } = c
        {
            let side = Side::of_slot(*slot);
            let portrait = faces.by_face_byte(*face).unwrap();
            pal.colors[usize::from(entry)] = td_pal.colors[usize::from(*color)];
            dialogue::load_face_palette(&mut pal, side, &portrait.palette);
            dialogue::draw_portrait(&mut fb, side, &portrait.pixels);
            dialogue::draw_name(&mut fb, &font, side, name, entry);
            entry += 1;
        }
    }
    assert_eq!(entry, 252, "two header speakers");
    for (frame_name, speaker) in [(F2283, Side::Right), (F2433, Side::Left)] {
        let (dac, frame) = oracle_frame(&oracle, frame_name);
        let (n, first) = diff_rows(&fb.pixels, &frame, 0..100);
        assert_eq!(n, 0, "{frame_name}: rows 0-99 differ: {first:?}");
        let mut ours = Palette::default();
        td_render::fade::write_level(&mut ours, &pal, 16);
        dialogue::highlight(&mut ours, &pal, speaker);
        for i in (0..224).chain([250, 251]) {
            assert_eq!(
                ours.colors[i], dac.colors[i],
                "{frame_name}: palette entry {i}"
            );
        }
    }
}

#[test]
fn dialogue_prompt_matches_the_original() {
    // text.md §6 vector 3: the scene-1 prompt at pen (1, 191) in entry 254, 539 ink pixels.
    let data = require_data!();
    let oracle = require_oracle!();
    let dir = DataDir::open(&data).unwrap();
    let res = ExeResources::load(&dir).unwrap();
    let prompt = res
        .text("dialogue.prompt")
        .expect("file0001.exe resource map");
    let font = small_font(&data);
    let mut fb = Framebuffer::new();
    dialogue::draw_prompt(&mut fb, &font, prompt);
    let ink = fb
        .pixels
        .iter()
        .filter(|&&p| p == dialogue::PROMPT_ENTRY)
        .count();
    assert_eq!(ink, 539);
    let (dac, frame) = oracle_frame(&oracle, F2433);
    let (n, first) = diff_rows(&fb.pixels, &frame, 190..200);
    assert_eq!(n, 0, "rows 190-199 differ: {first:?}");
    assert_eq!(dac.colors[254], dialogue::PROMPT_RGB);
}

#[test]
fn mission_title_card_matches_the_original() {
    // text.md §6 vector 1: mission 1 title, measured width 185, pen x 67, y 90, entry 85.
    let data = require_data!();
    let oracle = require_oracle!();
    let dir = DataDir::open(&data).unwrap();
    let res = ExeResources::load(&dir).unwrap();
    let title = res
        .text("e1.mission.01.title")
        .expect("file0001.exe resource map");
    assert_eq!(title.len(), 27);
    let font = Font::parse(&fs::read(data.join("td-let.cin")).unwrap(), FontKind::Title).unwrap();
    assert_eq!(title_text::width(title), 185);
    let x = title_text::centred_x(title);
    assert_eq!(x, 67);
    let mut fb = Framebuffer::new();
    title_text::draw(&mut fb, &font, title, (x, 90), 85);
    assert_eq!(fb.pixels.iter().filter(|&&p| p == 85).count(), 729);
    let (_, frame) = oracle_frame(&oracle, "f00001162_320x200_mission-title.vga");
    let (n, first) = diff_rows(&fb.pixels, &frame, 0..200);
    assert_eq!(n, 0, "title card differs: {first:?}");
}
