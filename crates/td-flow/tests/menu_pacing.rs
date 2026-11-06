//! The E2 and E3 main menus' fades and highlight ramp against the DAC write cadence of oracle
//! captures C-0120 (E2) and C-0125 (E3): e2-program-flow rev 2 §4.2 items 3–4,
//! e3-program-flow rev 2 §4.2 item 4. Needs `TD2192_DATA` and `TD2192_ORACLE`.
//!
//! The captures dump a framebuffer only every 70 frames, so the comparison uses each run's
//! `frames.tsv` (DAC index and data writes per emulated frame): the loader's fade-in is the
//! run of frames with DAC writes that ends in the frame before the first frame of 11 ramp
//! steps (11 index writes); the ramp runs on to the next frame without writes; the fade-out is
//! the next run of DAC writes (the Enter that starts the block).

use std::fs;
use std::path::Path;

use td_assets::{DataDir, ExeResources};
use td_flow::program_e2::{EpisodeCampaign, LaterEpisode};
use td_flow::settings::Settings;
use td_flow::{FlowContext, PlaceholderLauncher, Screen};
use td_input::{Key, TickInput};
use td_test_support::{require_data, require_oracle};

/// (frame, DAC index writes, DAC data writes) of a capture's `frames.tsv`.
fn dac_writes(oracle: &Path, capture: &str) -> Vec<(u64, u32, u32)> {
    fs::read_to_string(oracle.join(capture).join("frames.tsv"))
        .unwrap()
        .lines()
        .skip(1)
        .map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            (
                f[0].parse().unwrap(),
                f[5].parse().unwrap(),
                f[6].parse().unwrap(),
            )
        })
        .collect()
}

/// Frames of the capture's menu: (fade-in, ramp frames after the fade-in's last frame,
/// fade-out), each fade counted from its first to its last frame with DAC writes.
fn capture_menu(oracle: &Path, capture: &str) -> (u64, u64, u64) {
    let w = dac_writes(oracle, capture);
    let writes = |i: usize| w[i].2 > 0;
    let ramp = w.iter().position(|&(_, idx, _)| idx == 11).unwrap();
    let last = ramp - 1;
    let mut first = last;
    while writes(first - 1) {
        first -= 1;
    }
    let mut end = last;
    while writes(end + 1) {
        end += 1;
    }
    let mut out_first = end + 1;
    while !writes(out_first) {
        out_first += 1;
    }
    let mut out_last = out_first;
    while writes(out_last + 1) {
        out_last += 1;
    }
    (
        w[last].0 - w[first].0 + 1,
        w[end].0 - w[last].0,
        w[out_last].0 - w[out_first].0 + 1,
    )
}

/// Our menu: script `2`, then (frames from the menu's entry to its last fade-in level, frames
/// from there to the end of the start cursor's ramp, frames of the fade-out: from the Enter
/// to the frame before the action, which starts one frame after the last level's).
fn our_menu<C: EpisodeCampaign>(mut ep: LaterEpisode<C>) -> (u64, u64, u64) {
    let data = td_test_support::data_dir().unwrap();
    let dir = DataDir::open(&data).unwrap();
    let res = ExeResources::load(&dir).unwrap();
    let settings = Settings::default();
    let mut audio = Vec::new();
    let mut tick = |ep: &mut LaterEpisode<C>, keys: Vec<Key>| {
        let mut ctx = FlowContext {
            data: &dir,
            resources: &res,
            settings: &settings,
            audio: &mut audio,
        };
        let input = TickInput {
            pressed: keys,
            ..TickInput::default()
        };
        ep.tick(&mut ctx, &input);
    };
    let two = Key::digit(2).unwrap();
    let mut f = 0u64;
    // Until the menu image is in video memory (the loader ran in this frame).
    while ep.menu().screen().pixels.iter().all(|&p| p == 0) {
        tick(&mut ep, if f == 30 { vec![two] } else { Vec::new() });
        f += 1;
        assert!(f < 1_000);
    }
    let entered = f;
    let target = ep.menu().palette_buffer().clone();
    let mut last_level = None;
    while !ep.menu().waiting() {
        tick(&mut ep, Vec::new());
        f += 1;
        let dac = &ep.menu().screen().palette;
        // Level 16: every entry but the cursor's (already ramping) and 0 at the buffer's.
        if last_level.is_none() && dac.colors[1..224] == target.colors[1..224] {
            last_level = Some(f);
        }
    }
    let last_level = last_level.unwrap();
    let ramp_end = f;
    assert_eq!(ep.item_colours()[0].r, 63, "start cursor ramped up");
    tick(&mut ep, vec![Key::ENTER]);
    let enter = f;
    f += 1;
    while ep.state().0 == 0 {
        tick(&mut ep, Vec::new());
        f += 1;
        assert!(f < enter + 200);
    }
    (last_level - entered, ramp_end - last_level, f - enter - 1)
}

fn program<C: EpisodeCampaign>() -> LaterEpisode<C> {
    let data = td_test_support::data_dir().unwrap();
    let dir = DataDir::open(&data).unwrap();
    let res = ExeResources::load(&dir).unwrap();
    LaterEpisode::new(&dir, &res, Box::new(PlaceholderLauncher::default())).unwrap()
}

#[test]
fn e2_menu_fades_and_ramp_match_c0120() {
    // C-0120: fade-in f272..305 (34 frames: 17 levels × 2), the start cursor's ramp in the
    // last level's frame and four more (f305..309: 11, 11, 11, 11, 4 steps), fade-out on
    // Enter f435..468 (34 frames), then the block.
    let _data = require_data!();
    let oracle = require_oracle!();
    let theirs = capture_menu(&oracle, "C-0120");
    assert_eq!(theirs, (34, 4, 34), "C-0120 cadence");
    let ours = our_menu(program::<td_flow::campaign_e2::CampaignE2>());
    eprintln!("E2 menu: ours {ours:?}, C-0120 {theirs:?}");
    assert_eq!(ours, theirs);
}

#[test]
fn e3_menu_fades_and_ramp_match_c0125() {
    // C-0125: fade-in f270..303, ramp f303..307, fade-out f433..466.
    let _data = require_data!();
    let oracle = require_oracle!();
    let theirs = capture_menu(&oracle, "C-0125");
    assert_eq!(theirs, (34, 4, 34), "C-0125 cadence");
    let ours = our_menu(program::<td_flow::campaign_e3::CampaignE3>());
    eprintln!("E3 menu: ours {ours:?}, C-0125 {theirs:?}");
    assert_eq!(ours, theirs);
}
