//! The phase of the 17-level fades across a screen change, against the DAC write cadence of
//! oracle capture C-0002 (M12-14; `td_render::fade::level_frames`, `first_level_frames`;
//! screens rev 3 §2.1; `main_menu::NEXT_SCREEN_INIT_FRAMES`). Needs `TD2192_DATA` and
//! `TD2192_ORACLE`.
//!
//! A level counts as shown in the first frame whose fb dump would hold it complete: the frame
//! after the one in which `frames.tsv` has written its last entry (a dump labelled N shows the
//! DAC at the start of frame N). The port shows a level in the frame it writes it.

use std::fs;
use std::path::Path;

use td_assets::{DataDir, ExeResources};
use td_flow::settings::Settings;
use td_flow::{FlowContext, PlaceholderLauncher, Screen};
use td_formats::palette::Palette;
use td_input::{Key, TickInput};
use td_render::Framebuffer;
use td_test_support::{require_data, require_oracle};

/// For a fade whose writes start in frame `start` of `capture`, `per` entries a level: the
/// frame, minus `anchor`, in which each of its 17 levels is first shown complete.
fn capture_levels(oracle: &Path, capture: &str, start: u64, per: u32, anchor: u64) -> Vec<u64> {
    let tsv = fs::read_to_string(oracle.join(capture).join("frames.tsv")).unwrap();
    let writes: Vec<(u64, u32)> = tsv
        .lines()
        .skip(1)
        .map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            (f[0].parse().unwrap(), f[5].parse().unwrap())
        })
        .filter(|&(f, _)| f >= start)
        .collect();
    let mut out = Vec::new();
    let mut cum = 0;
    for (f, n) in writes {
        cum += n;
        while out.len() < 17 && cum >= per * (out.len() as u32 + 1) {
            out.push(f + 1 - anchor);
        }
        if out.len() == 17 {
            break;
        }
    }
    out
}

/// Level of a component shown as `v` whose full value is `full` (Round(full × k / 16)).
fn level(v: u8, full: u8) -> Option<u8> {
    let ks: Vec<u8> = (0..=16u8)
        .filter(|&k| ((u16::from(full) * u16::from(k) + 8) / 16) as u8 == v)
        .collect();
    (ks.len() == 1).then(|| ks[0])
}

/// The first frame (index into `pals`) that shows each level in `levels` of entry `e` (its
/// largest component, full value `full`), searching from `from`.
fn port_levels(pals: &[Palette], e: usize, full: u8, from: usize, levels: &[u8]) -> Vec<u64> {
    levels
        .iter()
        .map(|&k| {
            let at = pals[from..]
                .iter()
                .position(|p| {
                    let c = p.colors[e];
                    level(c.r.max(c.g).max(c.b), full) == Some(k)
                })
                .unwrap_or_else(|| panic!("level {k} of entry {e} never shown"));
            (from + at) as u64
        })
        .collect()
}

#[test]
fn e1_title_card_follows_the_menu_as_c0002() {
    // C-0002: Enter at f1027; the menu's fade-out writes from f1027.92, its levels 15..0
    // complete in f1031, 1033, …, 1059, 1060 (shown 5, 7, …, 33, 34 frames after the Enter);
    // the title card's mode X initialisation f1060..1062; its fade-in writes from f1063.92 at
    // one frame a level (nearly black g_Palette), levels 1..16 shown 39..54 after the Enter.
    let data = require_data!();
    let oracle = require_oracle!();
    let menu_theirs = capture_levels(&oracle, "C-0002", 1027, 255, 1027);
    let card_theirs = capture_levels(&oracle, "C-0002", 1063, 255, 1027);
    let dir = DataDir::open(&data).unwrap();
    let res = ExeResources::load(&dir).unwrap();
    let mut ep =
        td_flow::Episode1::new(&dir, &res, Box::new(PlaceholderLauncher::default())).unwrap();
    let settings = Settings::default();
    let mut audio = Vec::new();
    let mut fb = Framebuffer::new();
    let mut step = |ep: &mut td_flow::Episode1, keys: Vec<Key>| {
        let mut ctx = FlowContext {
            data: &dir,
            resources: &res,
            settings: &settings,
            audio: &mut audio,
        };
        ep.tick(
            &mut ctx,
            &TickInput {
                pressed: keys,
                ..TickInput::default()
            },
        );
        ep.draw(&mut fb);
        fb.palette.clone()
    };
    let one = Key::digit(1).unwrap();
    for f in 0..1_000 {
        step(&mut ep, if f == 40 { vec![one] } else { Vec::new() });
        if ep.menu().waiting() && ep.menu().screen().pixels.iter().any(|&p| p != 0) {
            break;
        }
    }
    assert!(ep.menu().waiting());
    // The entry with the brightest component in the menu's buffer tracks its fade-out.
    let buf = ep.menu().palette_buffer().clone();
    let bright = |c: td_formats::palette::Rgb6| c.r.max(c.g).max(c.b);
    let e = (1..224).max_by_key(|&e| bright(buf.colors[e])).unwrap();
    let mut pals = vec![step(&mut ep, vec![Key::ENTER])];
    for _ in 0..80 {
        pals.push(step(&mut ep, Vec::new()));
    }
    let out_levels: Vec<u8> = (0..16).rev().collect();
    let menu_ours = port_levels(&pals, e, bright(buf.colors[e]), 0, &out_levels);
    let black = menu_ours[15] as usize;
    // The title card fades entry 85, the ink, to (0, 63, 0)-like full brightness.
    let in_levels: Vec<u8> = (1..=16).collect();
    let card_ours = port_levels(&pals, 85, 63, black + 1, &in_levels);
    eprintln!("menu: ours {menu_ours:?}, C-0002 {:?}", &menu_theirs[1..]);
    eprintln!("card: ours {card_ours:?}, C-0002 {:?}", &card_theirs[1..]);
    assert_eq!(menu_ours, menu_theirs[1..], "menu fade-out, levels 15..0");
    assert_eq!(
        card_ours,
        card_theirs[1..],
        "title card fade-in, levels 1..16"
    );
}
