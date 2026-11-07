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

/// A frame dump of capture `capture`: its DAC and pixels (`None` if that frame was not dumped).
fn capture_frame(oracle: &Path, capture: &str, f: u32) -> Option<(Palette, Vec<u8>)> {
    let raw = fs::read(
        oracle
            .join(capture)
            .join("fb")
            .join(format!("f{f:08}_320x200.vga")),
    )
    .ok()?;
    assert_eq!(raw.len(), 768 + 320 * 200, "{capture} f{f}");
    Some((Palette::parse_prefix(&raw).unwrap(), raw[768..].to_vec()))
}

/// Pixel and DAC differences of the port's screen against a dump: every pixel, and the DAC
/// entries the dump's pixels use plus the two header speakers' text entries (no prompt outside
/// scene 1: entry 254 keeps the previous screen's colour, dialogue-player §4).
fn dump_diff(ours: &td_render::Framebuffer, (dac, px): &(Palette, Vec<u8>)) -> Vec<String> {
    let mut out: Vec<String> = (0..px.len())
        .filter(|&i| ours.pixels[i] != px[i])
        .take(8)
        .map(|i| {
            format!(
                "({}, {}): {} vs {}",
                i % 320,
                i / 320,
                ours.pixels[i],
                px[i]
            )
        })
        .collect();
    let used: BTreeSet<u8> = px.iter().copied().chain([250, 251]).collect();
    for e in used.into_iter().map(usize::from) {
        if ours.palette.colors[e] != dac.colors[e] {
            out.push(format!(
                "DAC {e}: {:?} vs {:?}",
                ours.palette.colors[e], dac.colors[e]
            ));
        }
    }
    out
}

/// E1 ending B (episode-program-flow rev 7 §4.5, §6 vector 5): block 17's failed post-flight
/// story plays dialogue 75 between the voffice card and the ending-B card (VE-9). C-0324 (RE-19)
/// opens `dialogue.dat` in frame 7703, starts the fade-in in f7713 (the first frame of the run of
/// DAC-writing frames that leads into the first lit dump, `frames.tsv`) and answers the scene's
/// five page breaks with one Space each (down in frames 7743, 7823, 7903, 7983, 8063, all typed
/// ahead of their page break); it dumps every 10th frame. The port's scene gets the same Spaces
/// at the same distance from the fade start. Port step s is capture frame `start + s` during the
/// fade-in; from the first line on the original is [`D75_LEAD`] frames ahead (BUG-33), and one
/// frame further behind after each retrace its typing loop missed (`frames.tsv`: 3DAh polled, no
/// retrace seen; text rev 5 §4.3 item 6; five in this scene, with music). Under that alignment
/// every dump from the open to the scene's end is compared: all pixels, and the DAC entries the
/// frame shows plus the speakers' entries 250 and 251, wherever no DAC write happened in that
/// frame (a fade level is written over about 1.5 frames).
#[test]
fn ending_b_dialogue_75_matches_c0324() {
    let data = require_data!();
    let oracle = require_oracle!();
    let cap = "C-0324";
    let dir = DataDir::open(&data).unwrap();
    let res = ExeResources::load(&dir).unwrap();
    let open = 7703u32;
    // D75's fade-out ends by f8330; f8340 shows the ending-B card being drawn (its fade-in
    // starts in f8353, `screens_parity`).
    let last = 8330u32;
    let dumps: Vec<u32> = (open..=last)
        .filter(|&f| capture_frame(&oracle, cap, f).is_some())
        .collect();
    assert!(dumps.len() > 50, "{cap}: D75 dumps {}", dumps.len());
    let column = |name: &str| -> std::collections::BTreeMap<u32, u32> {
        let tsv = fs::read_to_string(oracle.join(cap).join("frames.tsv")).unwrap();
        let mut lines = tsv.lines();
        let head: Vec<&str> = lines.next().unwrap().split('\t').collect();
        let col = head.iter().position(|&c| c == name).unwrap();
        lines
            .map(|l| {
                let v: Vec<&str> = l.split('\t').collect();
                (v[0].parse().unwrap(), v[col].parse().unwrap())
            })
            .collect()
    };
    let (dac_writes, polls, in_retrace) = (column("dac_3c9"), column("p3da"), column("p3da_vr"));
    let events = fs::read_to_string(oracle.join(cap).join("events.tsv")).unwrap();
    let spaces: Vec<u32> = events
        .lines()
        .filter_map(|l| {
            let v: Vec<&str> = l.split('\t').collect();
            (v.len() > 4 && v[2] == "KEY" && v[3] == "space" && v[4] == "down")
                .then(|| v[0].parse::<u32>().unwrap())
        })
        .filter(|&f| (open..last).contains(&f))
        .collect();
    assert_eq!(spaces, [7743, 7823, 7903, 7983, 8063], "{cap}: D75 Spaces");
    let lit = |p: &Palette| p.colors[1..].iter().any(|c| (c.r, c.g, c.b) != (0, 0, 0));
    let first_lit = *dumps
        .iter()
        .find(|&&f| lit(&capture_frame(&oracle, cap, f).unwrap().0))
        .expect("a lit D75 dump");
    let mut start = first_lit;
    while dac_writes.get(&(start - 1)).is_some_and(|&w| w > 0) {
        start -= 1;
    }
    assert_eq!(start, 7713, "{cap}: D75 fade start");
    // The fade-in: 17 levels, 2 frames each (dialogue-player §5 item 6); typing from then on.
    let typing = start + 34;
    let missed: Vec<u32> = (typing..last)
        .filter(|f| polls[f] > 0 && in_retrace[f] == 0)
        .collect();
    assert_eq!(
        missed,
        [7872, 7907, 8088, 8129, 8234],
        "{cap}: missed retraces"
    );

    // The port's frames from `start` on, with the Spaces at their capture frames.
    let mut s = DialogueScene::load(&dir, &res, Episode::E1, ScriptChoice::Original, 75).unwrap();
    let space = TickInput {
        pressed: vec![Key::SPACE],
        ..TickInput::default()
    };
    let idle = TickInput::default();
    let mut frames = vec![s.screen().clone()];
    for f in start + 1..=last + 4 {
        s.step(if spaces.contains(&f) { &space } else { &idle });
        frames.push(s.screen().clone());
    }
    let step = |f: u32, lead: u32| -> usize {
        let behind = missed.iter().filter(|&&m| m < f).count() as u32;
        (if f < typing {
            f - start
        } else {
            f - start + lead - behind
        }) as usize
    };
    let complete = |f: u32| dac_writes.get(&f).is_some_and(|&w| w == 0);
    let mismatches = |lead: u32| -> Vec<(u32, Vec<String>)> {
        let mut bad = vec![];
        for &f in dumps.iter().filter(|&&f| f > start) {
            let mut d = dump_diff(
                &frames[step(f, lead)],
                &capture_frame(&oracle, cap, f).unwrap(),
            );
            if !complete(f) {
                d.retain(|x| !x.starts_with("DAC"));
            }
            if !d.is_empty() {
                bad.push((f, d));
            }
        }
        bad
    };
    let counts: Vec<(u32, usize)> = (0..=4).map(|l| (l, mismatches(l).len())).collect();
    let bad = mismatches(D75_LEAD);
    for (f, d) in &bad {
        eprintln!("{cap} D75 f{f}: {}", d[..d.len().min(4)].join("; "));
    }
    assert!(
        bad.is_empty(),
        "{cap} D75 (fade start f{start}; differing dumps per lead {counts:?}): {} of {} dumps \
         differ",
        bad.len(),
        dumps.len()
    );
    eprintln!(
        "{cap} D75: {} dumps f{}..f{} equal (fade start f{start}, lead {D75_LEAD}, missed \
         retraces {missed:?}; differing dumps per lead {counts:?})",
        dumps.len(),
        dumps[0],
        dumps[dumps.len() - 1]
    );
}

/// Frames by which the original's dialogue typing runs ahead of the port's from the first line
/// after the fade-in (C-0324; BUG-33).
const D75_LEAD: u32 = 2;

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
