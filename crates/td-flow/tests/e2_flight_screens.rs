//! Episode 2 in-flight screens from the original data: the E2 resources, the F5 entries of
//! `docs/spec/sim/e2-mission-setup.md` rev 1 §3.4, the transmissions and the ship library of
//! `docs/spec/flow/e2-flight-screens.md` rev 1 (§3.1, §3.3, §6), against oracle capture C-0044
//! (E2 flight 11: transmission 4 in RAM and on screen, the ship library). Data-gated
//! (TD2192_DATA; the capture comparisons also TD2192_ORACLE). No game text or captured pixels
//! are stored here.

use std::fs;
use std::path::Path;

use td_assets::{DataDir, Episode, ExeResources};
use td_flow::flight_screens::comm::{Line, SCROLL_ROWS, SCROLL_X, Transmission};
use td_flow::flight_screens::{CommScreen, FlightScreenAssets, ModalStep, TargetBoard};
use td_formats::palette::Palette;
use td_input::{Key, TickInput};
use td_render::Framebuffer;
use td_test_support::{require_data, require_oracle};

const W: usize = 320;

fn assets(radar_set2: bool) -> FlightScreenAssets {
    let data = td_test_support::data_dir().unwrap();
    let dir = DataDir::open(&data).unwrap();
    let res = ExeResources::load(&dir).unwrap();
    FlightScreenAssets::load_episode(&dir, &res, Episode::E2, radar_set2).unwrap()
}

#[test]
fn e2_assets_and_target_lines() {
    let _data = require_data!();
    let a = assets(false);
    assert_eq!(a.target_info.len(), 113);
    assert_eq!(a.radar_colours.len(), 80);
    assert_eq!(a.portraits.len(), 7 * 1024, "td2.ccm");
    assert_eq!(assets(true).radar_colours.len(), 80);
    assert_ne!(assets(true).radar_colours, a.radar_colours, "set 2 table");
    // Every F5 line of every flight is a string of e2.target_info; line 1 is blank.
    let mut board = TargetBoard::default();
    for f in 1..=20 {
        board.load_episode(Episode::E2, f);
        for lines in board.lines.iter().flatten() {
            assert!(lines.iter().all(|&k| usize::from(k) < a.target_info.len()));
        }
    }
    assert!(a.target_info[1].trim().is_empty());
}

#[test]
fn e2_transmissions_play_to_the_end() {
    let _data = require_data!();
    let a = assets(false);
    assert_eq!(a.transmissions.len(), 7);
    for (i, t) in a.transmissions.iter().enumerate() {
        let n = i + 1;
        let t = t.as_ref().unwrap();
        assert_eq!(t.lines.last().map(|l| l.0), Some(Line::End), "comm {n}");
        assert!(t.portraits.0 < 7 && t.portraits.1 < 7);
        // Space at every prompt, then F6 at the end.
        let mut s = CommScreen::new(Framebuffer::new(), &a, n).unwrap();
        let mut closed = false;
        for frame in 0..20_000 {
            let key = if s.waiting_to_close() {
                Key::F6
            } else {
                Key::SPACE
            };
            let input = TickInput {
                pressed: if frame % 2 == 0 {
                    vec![key]
                } else {
                    Vec::new()
                },
                ..TickInput::default()
            };
            if let ModalStep::Close { abort } = s.tick(&a, &input) {
                assert!(!abort);
                closed = true;
                break;
            }
        }
        assert!(closed, "comm {n} never closed");
    }
    assert!(CommScreen::new(Framebuffer::new(), &a, 8).is_none());
}

#[test]
fn e2_runs_hold_their_markers() {
    // e2-flight-screens §3.1: run sizes (screen 6 = .06 + .07), marker indices; every run's
    // last string is its end marker.
    let _data = require_data!();
    let a = assets(false);
    let runs = [
        (19, Some(4), 18),
        (10, Some(6), 9),
        (14, Some(7), 13),
        (25, Some(6), 24),
        (15, Some(5), 14),
        (12, None, 11),
        (15, Some(6), 14),
    ];
    for (n, (run, (count, prompt, end))) in (1..).zip(a.comms.iter().zip(runs)) {
        assert_eq!(run.len(), count, "screen {n}");
        assert_eq!(run[end], a.comm_prompts[1], "screen {n}: end marker");
        if let Some(p) = prompt {
            assert_eq!(run[p], a.comm_prompts[0], "screen {n}: prompt marker");
        }
        let markers = run.iter().filter(|s| a.comm_prompts.contains(s)).count();
        assert_eq!(markers, 1 + usize::from(prompt.is_some()), "screen {n}");
    }
}

fn oracle_file(oracle: &Path, dir: &str, name: &str) -> Vec<u8> {
    fs::read(oracle.join("C-0044").join(dir).join(name)).unwrap()
}

/// The text of slot `line` of `t` as the loader stores it: the run's string, or a marker.
fn slot_text<'a>(a: &'a FlightScreenAssets, run: &'a [String], line: Line) -> &'a str {
    match line {
        Line::Text(j) => &run[j],
        Line::Prompt => &a.comm_prompts[0],
        Line::End => &a.comm_prompts[1],
    }
}

/// A Pascal `string[50]` at DGROUP offset `ds` of a dump that starts at DS:31E0.
fn ram_string(ram: &[u8], ds: usize) -> &[u8] {
    let at = ds - 0x31E0;
    &ram[at + 1..at + 1 + usize::from(ram[at])]
}

#[test]
fn transmission_4_table_matches_c0044_ram() {
    // §6 vector 3: DS:31EC header A, DS:321F header B, line k at DS:321F + 51k, speaker byte
    // k at DS:384B + k (1 = box A), portraits at DS:386A/386B; dumped when the screen opened
    // (f1674) and before it closed (f3314).
    let _data = require_data!();
    let oracle = require_oracle!();
    let a = assets(false);
    let run = &a.comms[3];
    let t: &Transmission = a.transmissions[3].as_ref().unwrap();
    for name in [
        "f00001674_3EDA-31E0_6A0_commtable.bin",
        "f00003314_3EDA-31E0_6A0_commtable-end.bin",
    ] {
        let ram = oracle_file(&oracle, "mem", name);
        assert_eq!(
            ram_string(&ram, 0x31EC),
            run[0].as_bytes(),
            "{name}: header A"
        );
        assert_eq!(
            ram_string(&ram, 0x321F),
            run[1].as_bytes(),
            "{name}: header B"
        );
        assert_eq!(t.lines.len(), 27);
        for (k, &(line, side)) in (1..).zip(&t.lines) {
            let text = slot_text(&a, run, line);
            assert_eq!(
                ram_string(&ram, 0x321F + 51 * k),
                text.as_bytes(),
                "{name}: line {k}"
            );
            let speaker = ram[0x384B + k - 0x31E0];
            assert_eq!(speaker == 1, side == 0, "{name}: speaker of line {k}");
        }
        let portraits = (ram[0x386A - 0x31E0], ram[0x386B - 0x31E0]);
        assert_eq!(portraits, (4, 0), "{name}: portraits");
        assert_eq!(t.portraits, (4, 0));
    }
}

/// The panel's text area: headers and both boxes, x 92..271.
fn text_area(fb: &[u8]) -> Vec<u8> {
    let rows = (35..=40).chain(SCROLL_ROWS[0].0..=SCROLL_ROWS[0].1);
    let rows = rows
        .chain(87..=92)
        .chain(SCROLL_ROWS[1].0..=SCROLL_ROWS[1].1);
    rows.flat_map(|y| fb[y * W + SCROLL_X.0..=y * W + SCROLL_X.1].to_vec())
        .collect()
}

/// Pixels of a C-0044 frame (320 × 192) and its DAC.
fn frame(oracle: &Path, f: u32) -> (Palette, Vec<u8>) {
    let raw = oracle_file(oracle, "fb", &format!("f{f:08}_320x192_s.vga"));
    (Palette::parse_prefix(&raw).unwrap(), raw[768..].to_vec())
}

#[test]
fn transmission_4_screen_matches_c0044() {
    // §6 vector 3 and §6.1: the td.cmm panel and portraits td2.ccm 4 / 0, then at each of
    // the four prompts and at the end marker the whole text area equals a captured frame
    // (frames f1550..f3300 every 25 frames).
    let _data = require_data!();
    let oracle = require_oracle!();
    let a = assets(false);
    let mut c = CommScreen::new(Framebuffer::new(), &a, 4).unwrap();
    let (_, f) = frame(&oracle, 1700);
    let mut panel = 0;
    for y in 16..144 {
        for x in 32..288 {
            let p = c.canvas().pixels[y * W + x];
            let in_text = (92..=271).contains(&x) && (35..=124).contains(&y);
            if p != 0 && !in_text {
                assert_eq!(f[y * W + x], p, "panel ({x}, {y})");
                panel += 1;
            }
        }
    }
    assert!(panel > 10_000);
    let frames: Vec<Vec<u8>> = (1550..=3300)
        .step_by(25)
        .map(|f| text_area(&frame(&oracle, f).1))
        .collect();
    let space = TickInput {
        pressed: vec![Key::SPACE],
        ..TickInput::default()
    };
    let mut waits = 0;
    for _ in 0..20_000 {
        if c.waiting_for_space() || c.waiting_to_close() {
            let ours = text_area(&c.canvas().pixels);
            assert!(
                frames.contains(&ours),
                "wait {waits}: no captured frame shows this text"
            );
            waits += 1;
            if c.waiting_to_close() {
                break;
            }
            c.tick(&a, &space);
        } else {
            c.tick(&a, &TickInput::default());
        }
    }
    assert_eq!(waits, 5, "four prompts and the end marker");
}
