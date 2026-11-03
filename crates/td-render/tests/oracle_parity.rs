//! L5 parity tests: our rendering compared pixel-for-pixel with frames captured from the
//! original game (oracle, `docs/re/dynamic-observations.md`). They need both TD2192_DATA and
//! TD2192_ORACLE and skip otherwise. No captured pixels are stored in the repository.

use std::fs;

use td_formats::dialogue::{DialogueScript, Item};
use td_formats::font::{Font, FontKind};
use td_render::{Framebuffer, Rect, small_text};
use td_test_support::{require_data, require_oracle};

fn oracle_frame(name: &str) -> Vec<u8> {
    let dir = require_oracle_dir();
    let bytes = fs::read(dir.join("C-0002").join("fb").join(name)).unwrap();
    bytes[768..].to_vec()
}

fn require_oracle_dir() -> std::path::PathBuf {
    td_test_support::oracle_dir().expect("checked by require_oracle!")
}

#[test]
fn first_dialogue_line_matches_the_original() {
    let data = require_data!();
    let _oracle = require_oracle!();
    let font = Font::parse(&fs::read(data.join("td.lt2")).unwrap(), FontKind::Small).unwrap();
    let script = DialogueScript::parse(&fs::read(data.join("dialogue.dat")).unwrap()).unwrap();
    let scene = script.scene(1).unwrap();
    // The first speech block: blank line, then the spoken line (second item).
    let Item::Line { text, .. } = &scene.segments[0][1] else {
        panic!("expected a text line");
    };
    let mut fb = Framebuffer::new();
    small_text::draw(&mut fb, &font, text, (4, 162), 251, Rect::SCREEN);
    let frame = oracle_frame("f00002283_320x200_dialogue-1.vga");
    let rows = 162 * 320..167 * 320;
    let diff = fb.pixels[rows.clone()]
        .iter()
        .zip(&frame[rows])
        .filter(|(a, b)| a != b)
        .count();
    assert_eq!(
        diff, 0,
        "pixels differing from the original in rows 162-166"
    );
}
