//! L5 parity: whole dialogue frames, played by [`DialogueScene`] from the script, compared pixel
//! for pixel with oracle capture C-0002 (`docs/re/dynamic-observations.md`;
//! `docs/spec/narrative/dialogue-player.md` §6). They need both TD2192_DATA and TD2192_ORACLE
//! and skip otherwise. No captured pixels are stored in the repository.
//!
//! C-0002 plays E1 scene 1 with the original script: the first page is on screen at frame
//! 2283; Space at frame 2323; the second page at 2433; Space at 2463; frame 2573 shows the
//! third page being typed; Esc at 2603.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use td_assets::{DataDir, Episode, ExeResources};
use td_flow::dialogue::{DialogueScene, ScriptChoice};
use td_formats::palette::Palette;
use td_input::{Key, TickInput};
use td_test_support::{require_data, require_oracle};

fn oracle_frame(oracle: &Path, name: &str) -> (Palette, Vec<u8>) {
    let raw = fs::read(oracle.join("C-0002").join("fb").join(name)).unwrap();
    assert_eq!(raw.len(), 768 + 320 * 200, "{name}");
    (Palette::parse_prefix(&raw).unwrap(), raw[768..].to_vec())
}

/// Compares every pixel, and the DAC entries that any pixel of the original frame uses plus
/// the two speakers' text entries and the prompt entry. (The other entries hold colours left
/// over from the previous screen, which nothing shows.)
fn assert_matches(scene: &DialogueScene, oracle: &Path, name: &str) {
    let (dac, frame) = oracle_frame(oracle, name);
    let ours = scene.screen();
    let diffs: Vec<String> = (0..frame.len())
        .filter(|&i| ours.pixels[i] != frame[i])
        .map(|i| {
            format!(
                "({}, {}): {} vs {}",
                i % 320,
                i / 320,
                ours.pixels[i],
                frame[i]
            )
        })
        .collect();
    assert!(
        diffs.is_empty(),
        "{name}: {} pixels differ; first (x, y): ours vs original: {:?}",
        diffs.len(),
        &diffs[..diffs.len().min(12)]
    );
    let used: BTreeSet<u8> = frame.iter().copied().chain([250, 251, 254]).collect();
    for i in used {
        let i = usize::from(i);
        assert_eq!(
            ours.palette.colors[i], dac.colors[i],
            "{name}: DAC entry {i}"
        );
    }
}

fn run_to_page_wait(s: &mut DialogueScene) {
    for _ in 0..2000 {
        if s.waiting_for_key() {
            return;
        }
        s.step(&TickInput::default());
    }
    panic!("no page break reached");
}

#[test]
fn scene_1_frames_match_the_original() {
    let data = require_data!();
    let oracle = require_oracle!();
    let dir = DataDir::open(&data).unwrap();
    let res = ExeResources::load(&dir).unwrap();
    let mut s = DialogueScene::load(&dir, &res, Episode::E1, ScriptChoice::Original, 1).unwrap();
    let space = TickInput {
        pressed: vec![Key::SPACE],
        ..TickInput::default()
    };

    // Vectors 1, 3, 4, 5: first page (dispatcher speaking).
    run_to_page_wait(&mut s);
    assert_matches(&s, &oracle, "f00002283_320x200_dialogue-1.vga");

    // Vector 2: second page (Velasquez speaking).
    s.step(&space);
    run_to_page_wait(&mut s);
    assert_matches(&s, &oracle, "f00002433_320x200_dialogue-2.vga");

    // Space at frame 2463, dump at frame 2573: with one typed character per frame
    // (U-062 assumption) the key's tick and the next 110 type 111 characters (a padding line,
    // a 70-character line, then 40 characters and the cursor of the next line).
    // (109 or 111 ticks differ from the frame by 37 and 33 pixels.)
    s.step(&space);
    for _ in 0..110 {
        s.step(&TickInput::default());
    }
    assert!(!s.waiting_for_key());
    assert_matches(&s, &oracle, "f00002573_320x200_dialogue-3.vga");

    // Esc at frame 2603, typed ahead: it is taken at the next page break, then the scene fades
    // out and ends.
    s.step(&TickInput {
        pressed: vec![Key::ESCAPE],
        ..TickInput::default()
    });
    for _ in 0..2000 {
        if s.finished() {
            break;
        }
        s.step(&TickInput::default());
    }
    assert!(s.finished() && s.skipped());
    assert!(
        s.screen().palette.colors[1..]
            .iter()
            .all(|c| c.r == 0 && c.g == 0 && c.b == 0)
    );
}

#[test]
fn every_scene_of_every_script_plays_to_the_end() {
    // L1-style robustness: each scene, answering every page with Space, ends by itself.
    let data = require_data!();
    let dir = DataDir::open(&data).unwrap();
    let res = ExeResources::load(&dir).unwrap();
    let space = TickInput {
        pressed: vec![Key::SPACE],
        ..TickInput::default()
    };
    let idle = TickInput::default();
    for episode in Episode::ALL {
        for choice in [ScriptChoice::Original, ScriptChoice::Edited] {
            let assets =
                td_flow::dialogue::DialogueAssets::load(&dir, &res, episode, choice).unwrap();
            let script = td_formats::dialogue::DialogueScript::parse(&assets.script).unwrap();
            for scene in &script.scenes {
                let mut s = DialogueScene::new(assets.clone(), episode, scene.number);
                let mut ticks = 0u32;
                while !s.finished() {
                    s.step(if s.waiting_for_key() { &space } else { &idle });
                    ticks += 1;
                    assert!(
                        ticks < 200_000,
                        "{episode} scene {} never ends",
                        scene.number
                    );
                }
                assert!(!s.skipped());
            }
        }
    }
}
