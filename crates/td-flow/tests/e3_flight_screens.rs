//! Episode 3 in-flight screens from the original data: the E3 resources, the F5 entries of
//! `docs/spec/sim/e3-mission-setup.md` rev 1 §3.4 and the transmission tables of `docs/spec/flow/e2-flight-screens.md` rev 1 §3.2.
//! Data-gated (TD2192_DATA). No game text is stored here.

use td_assets::{DataDir, Episode, ExeResources};
use td_flow::flight_screens::map::EMPTY_LINE;
use td_flow::flight_screens::menu::{draw_ship_library, library_layout};
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
    // Comm screens 1, 2, 3 and 5 (e2-flight-screens §3.2); there is no 4.
    let _data = require_data!();
    let a = assets();
    assert_eq!(a.transmissions.len(), 5);
    assert!(a.transmissions[3].is_none(), "no comm 4");
    assert!(CommScreen::new(Framebuffer::new(), &a, 4).is_none());
    let runs = [(1, 13), (2, 16), (3, 7), (5, 21)];
    for (n, count) in runs {
        assert_eq!(a.comms[n - 1].len(), count, "comm {n}: run size");
        let t = a.transmissions[n - 1].as_ref().unwrap();
        // The end marker of each run is its last string, and the P lines are markers.
        assert_eq!(
            a.comms[n - 1].last(),
            a.comm_prompts.get(1),
            "comm {n}: end marker"
        );
        assert!(t.portraits.0 <= 4 && t.portraits.1 == 0, "comm {n}");
        assert!(play(&a, n), "comm {n} never closed");
    }
}

#[test]
fn e3_ship_library_shows_all_nine_blocks() {
    // e2-flight-screens §3.3 and §6 vector 5: nine four-line blocks, nothing hatched.
    let _data = require_data!();
    let a = assets();
    let layout = library_layout(Episode::E3);
    assert_eq!(layout.blocks.len(), 9);
    assert!(layout.hatch.is_empty());
    let mut sprites: Vec<usize> = layout.blocks.iter().map(|b| b.sprite).collect();
    sprites.sort_unstable();
    assert_eq!(sprites, (0..9).collect::<Vec<_>>());
    // Every index is one of the 31 e3.ship_library strings; flight 1 draws them all.
    assert_eq!(a.ship_library.len(), 31);
    assert!(
        layout
            .blocks
            .iter()
            .all(|b| b.text_from <= 1 && b.lines.iter().all(|l| l.1 < 31))
    );
    let mut fb = Framebuffer::new();
    draw_ship_library(&mut fb, &a, 1);
    // Sprite 6 at (4, 152) is not covered by the checkerboard of colour 1.
    let hatch = (0..32)
        .flat_map(|r| (0..32).map(move |c| (r, c)))
        .filter(|(r, c)| (r + c) % 2 == 0)
        .all(|(r, c)| fb.get(4 + c, 152 + r) == Some(1));
    assert!(!hatch);
}
