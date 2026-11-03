//! L1 tests: dialogue scripts, mission files and executable headers over every original file.
//! Data-gated: they skip when TD2192_DATA is not set.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use td_formats::dialogue::{DialogueScript, Item};
use td_formats::mission::{EMPTY_SLOT, Mission};
use td_formats::mz::MzHeader;
use td_test_support::require_data;

fn read(dir: &Path, name: &str) -> Vec<u8> {
    fs::read(dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

#[test]
fn dialogue_scripts_parse_strictly() {
    let dir = require_data!();
    // (uncut, edited, scenes, segments) from FORMATS.md §8.
    for (uncut, edited, scenes, segments) in [
        ("dialogue.dat", "safedial.dat", 103, 103),
        ("dialogu2.dat", "safedia2.dat", 125, 126),
        ("dialogu3.dat", "safedia3.dat", 111, 112),
    ] {
        let a = DialogueScript::parse(&read(&dir, uncut)).unwrap();
        let b = DialogueScript::parse(&read(&dir, edited)).unwrap();
        for (name, s) in [(uncut, &a), (edited, &b)] {
            assert_eq!(s.scenes.len(), scenes, "{name}");
            let segs: usize = s.scenes.iter().map(|sc| sc.segments.len()).sum();
            assert_eq!(segs, segments, "{name}");
            for sc in &s.scenes {
                assert!(sc.speakers.iter().all(|sp| sp.slot == 1 || sp.slot == 2));
                for item in sc.segments.iter().flatten() {
                    if let Item::Line { param, .. } = item {
                        assert_eq!(*param, 1, "{name} scene {}", sc.number);
                    }
                }
            }
        }
        let nums = |s: &DialogueScript| s.scenes.iter().map(|x| x.number).collect::<Vec<_>>();
        assert_eq!(
            nums(&a),
            nums(&b),
            "{uncut} and {edited} have the same scenes"
        );
        let unique: BTreeSet<u8> = nums(&a).into_iter().collect();
        assert_eq!(
            unique.len(),
            scenes,
            "{uncut}: scene IDs are unique (not sequential)"
        );
    }
}

#[test]
fn player_reader_agrees_with_the_strict_parser() {
    use td_formats::dialogue::{Command, read_scene};
    let dir = require_data!();
    for name in [
        "dialogue.dat",
        "safedial.dat",
        "dialogu2.dat",
        "safedia2.dat",
        "dialogu3.dat",
        "safedia3.dat",
    ] {
        let data = read(&dir, name);
        let script = DialogueScript::parse(&data).unwrap();
        for sc in &script.scenes {
            let cmds = read_scene(&data, sc.number)
                .unwrap_or_else(|| panic!("{name}: scene {} not found", sc.number));
            let count = |f: fn(&Command) -> bool| cmds.iter().filter(|c| f(c)).count();
            let lines = sc.segments.iter().flatten();
            let items = |f: fn(&Item) -> bool| lines.clone().filter(|i| f(i)).count();
            let what = format!("{name} scene {}", sc.number);
            assert_eq!(
                count(|c| matches!(c, Command::Line { .. })),
                items(|i| matches!(i, Item::Line { .. })),
                "{what}"
            );
            assert_eq!(
                count(|c| matches!(c, Command::PageBreak)),
                items(|i| matches!(i, Item::PageBreak)),
                "{what}"
            );
            assert_eq!(
                count(|c| matches!(c, Command::Speaker { .. })),
                sc.speakers.len() + items(|i| matches!(i, Item::Speaker(_) | Item::Face { .. })),
                "{what}"
            );
            assert_eq!(
                count(|c| *c == Command::SegmentStart),
                sc.segments.len(),
                "{what}"
            );
            assert_eq!(
                count(|c| *c == Command::SegmentEnd),
                sc.segments.len(),
                "{what}"
            );
        }
    }
}

#[test]
fn mission_files_parse() {
    let dir = require_data!();
    let mut files = Vec::new();
    for (ext, range) in [("td", 1..=20), ("td2", 1..=20), ("td3", 1..=21)] {
        for n in range {
            if ext == "td3" && n == 20 {
                continue; // E3 ships m1..m19 + m21 (U-012)
            }
            files.push(format!("m{n}.{ext}"));
        }
    }
    assert_eq!(files.len(), 60);
    let mut classes = BTreeSet::new();
    for name in &files {
        let m = Mission::parse(&read(&dir, name)).unwrap();
        classes.extend(m.slot_class.iter().copied().filter(|&c| c != EMPTY_SLOT));
        assert!(m.hdr_b.iter().chain(&m.hdr_d).all(|&b| b == 0), "{name}");
        if name != "m18.td3" {
            assert!(m.hdr_a.iter().all(|&b| b == 0), "{name} (U-038)");
        }
        if name == "m1.td" {
            // structures.md §2 check: slot 1 starts at (7Ch, 49h), slot 2 at (7Dh, 49h).
            assert_eq!(m.path(0).unwrap().next(), Some((0x7C, 0x49)));
            assert_eq!(m.path(1).unwrap().next(), Some((0x7D, 0x49)));
        }
        // Path coordinates are plain 0..=255 tile values; runs beyond 200 are off-map approaches
        // (FORMATS.md §7), so no bounds are asserted here.
        for s in m.used_slots() {
            assert_eq!(m.path(s).unwrap().count(), 500, "{name}");
        }
    }
    assert_eq!(classes, (0..=8).collect(), "class codes 0..=8 (U-008)");
}

#[test]
fn executable_headers_match_executables_md() {
    let dir = require_data!();
    for (name, header, cs, ip) in [
        ("td1.exe", 544, 0x0000, 0x01FE),
        ("file0000.exe", 1600, 0x0000, 0x0FC0),
        ("file0001.exe", 19136, 0x0000, 0x4526),
        ("td2.exe", 16768, 0x0000, 0x4365),
        ("td3.exe", 14544, 0x0000, 0x3E8A),
    ] {
        let data = read(&dir, name);
        let h = MzHeader::parse(&data).unwrap();
        assert_eq!(h.header_bytes, header, "{name}");
        assert_eq!(h.image_end, data.len(), "{name}: no overlay");
        assert_eq!((h.initial_cs, h.initial_ip), (cs, ip), "{name}");
    }
}

#[test]
fn audio_containers_parse() {
    use td_formats::cmf::Cmf;
    use td_formats::voc::{Block, Voc};
    let dir = require_data!();
    // (file, end tick) from docs/spec/audio/cmf.md; all clocks are 96 ticks/s.
    for (name, end) in [
        ("climax.cmf", 5160),
        ("death.cmf", 2764),
        ("hangar.cmf", 4616),
        ("intro.cmf", 8064),
        ("intro1.cmf", 2069),
        ("intro2.cmf", 9676),
        ("low.cmf", 8063),
        ("menu.cmf", 4841),
        ("mood.cmf", 4821),
        ("mood2.cmf", 2461),
        ("mood3.cmf", 2461),
        ("moodmaj.cmf", 5376),
        ("satair.cmf", 6144),
        ("td01.cmf", 11136),
        ("td02.cmf", 6912),
        ("td03.cmf", 6093),
        ("vulture.cmf", 9600),
    ] {
        let c = Cmf::parse(&read(&dir, name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!((c.ticks_per_second, c.end_tick), (96, end), "{name}");
        assert!(!c.events.is_empty());
    }
    for name in [
        "afterb.voc",
        "boomboom.voc",
        "home.voc",
        "hornet.voc",
        "income.voc",
        "missile.voc",
        "poink.voc",
        "pulse.voc",
        "punyexp.voc",
        "ric2.voc",
        "ric3.voc",
        "scratch.voc",
        "stiletto.voc",
        "vii.voc",
        "vulturei.voc",
        "whoosh.voc",
        "zeus.voc",
    ] {
        let bytes = read(&dir, name);
        let v = Voc::parse(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(
            matches!(v.blocks.first(), Some(Block::Sound { .. })),
            "{name}"
        );
    }
}
