//! Episode 3 in-flight screens from the original data: the E3 resources, the F5 entries of
//! `docs/spec/sim/e3-mission-setup.md` rev 1 §3.4 and the provisional transmissions (U-351).
//! Data-gated (TD2192_DATA). No game text is stored here.

use td_assets::{DataDir, Episode, ExeResources};
use td_flow::flight_screens::comm::Line;
use td_flow::flight_screens::map::EMPTY_LINE;
use td_flow::flight_screens::{CommScreen, FlightScreenAssets, ModalStep, TargetBoard};
use td_input::{Key, TickInput};
use td_render::Framebuffer;
use td_test_support::require_data;

fn assets() -> FlightScreenAssets {
    let data = td_test_support::data_dir().unwrap();
    let dir = DataDir::open(&data).unwrap();
    let res = ExeResources::load(&dir).unwrap();
    FlightScreenAssets::load_episode(&dir, &res, Episode::E3, false).unwrap()
}

#[test]
fn e3_assets_and_target_lines() {
    let _data = require_data!();
    let a = assets();
    assert_eq!(a.target_info.len(), 114);
    assert_eq!(a.radar_colours.len(), 80);
    assert_eq!(a.portraits.len(), 5 * 1024, "td3.ccm");
    // Every F5 line of every flight is a string of e3.target_info (or the empty line);
    // k = 2 is the blank line (30 spaces) every block uses.
    let mut board = TargetBoard::default();
    for f in (1..=19).chain([21]) {
        board.load_episode(Episode::E3, f);
        for lines in board.lines.iter().flatten() {
            assert!(
                lines
                    .iter()
                    .all(|&k| k == EMPTY_LINE || usize::from(k) < a.target_info.len()),
                "flight {f}"
            );
        }
    }
    assert_eq!(a.target_info[2], " ".repeat(30));
}

/// Plays transmission `n`: Space at every prompt, F6 when it waits to close.
fn play(a: &FlightScreenAssets, n: usize) -> bool {
    let mut s = CommScreen::new(Framebuffer::new(), a, n).unwrap();
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
        if let ModalStep::Close { abort } = s.tick(a, &input) {
            assert!(!abort);
            return true;
        }
    }
    false
}

#[test]
fn e3_transmissions_play_to_the_end() {
    // Comm screens 1, 2, 3 and 5 (e3-mission-logic §4.5); there is no 4.
    let _data = require_data!();
    let a = assets();
    assert_eq!(a.transmissions.len(), 5);
    assert!(a.transmissions[3].lines.is_empty(), "no comm 4");
    for n in [1, 3, 5] {
        let t = &a.transmissions[n - 1];
        assert_eq!(t.lines.last().map(|l| l.0), Some(Line::End), "comm {n}");
        // Comms 1 and 5 are dialogues; in comm 3 only the caller speaks.
        let answered = t
            .lines
            .iter()
            .any(|l| l.1 == 1 && matches!(l.0, Line::Text(_)));
        assert_eq!(answered, n != 3, "comm {n}");
    }
    // e3.comms.02 has no end marker: the screen waits to close after its lines.
    assert!(
        a.transmissions[1]
            .lines
            .iter()
            .all(|l| matches!(l.0, Line::Text(_)))
    );
    for n in [1, 2, 3, 5] {
        let t = &a.transmissions[n - 1];
        assert!(t.portraits.0 < 5 && t.portraits.1 == 0, "comm {n}");
        assert!(play(&a, n), "comm {n} never closed");
    }
}
