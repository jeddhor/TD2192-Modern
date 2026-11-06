//! Episode 2 in-flight screens from the original data: the E2 resources, the F5 entries of
//! `docs/spec/sim/e2-mission-setup.md` rev 1 §3.4 and the provisional transmissions (U-310).
//! Data-gated (TD2192_DATA). No game text is stored here.

use td_assets::{DataDir, Episode, ExeResources};
use td_flow::flight_screens::comm::Line;
use td_flow::flight_screens::{CommScreen, FlightScreenAssets, ModalStep, TargetBoard};
use td_input::{Key, TickInput};
use td_render::Framebuffer;
use td_test_support::require_data;

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
