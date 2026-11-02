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
        assert!(m.reserved_tail.iter().all(|&b| b == 0), "{name}");
        if name != "m18.td3" {
            assert!(m.reserved_head.iter().all(|&b| b == 0), "{name} (U-014)");
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
