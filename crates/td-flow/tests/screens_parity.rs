//! Front-end screens never compared with the original before M12 (docs/parity/M12-checklist.md
//! §0 item 1), against VE-7's oracle captures C-0270.. (docs/parity/M12-screens.md): debriefs,
//! assignment orders, enemy and pilot dossiers, the story so far, the stills, the endings and the
//! episode end cards of all three episodes. Data- and oracle-gated (TD2192_DATA, TD2192_ORACLE).
//! No captured pixels are stored: every test reads the frame dumps of the capture.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use td_assets::{DataDir, Episode, ExeResources};
use td_flow::PlaceholderLauncher;
use td_flow::campaign::Step;
use td_flow::program::Program;
use td_flow::story::{self, StoryScreen};
use td_formats::palette::Palette;
use td_input::TickInput;
use td_render::Framebuffer;
use td_test_support::{require_data, require_oracle};

fn load() -> Option<(DataDir, ExeResources)> {
    let data = td_test_support::data_dir()?;
    let dir = DataDir::open(&data).unwrap();
    let res = ExeResources::load(&dir).unwrap();
    Some((dir, res))
}

fn program(dir: &DataDir, res: &ExeResources, episode: Episode) -> Program {
    Program::for_episode(dir, res, Box::new(PlaceholderLauncher::default()), episode).unwrap()
}

/// The frame dump `f<frame>_…` of a capture: its DAC and pixels.
fn frame(oracle: &Path, capture: &str, frame: u32) -> (Palette, Vec<u8>) {
    let dir = oracle.join(capture).join("fb");
    let prefix = format!("f{frame:08}_320x200");
    let name = fs::read_dir(&dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .find(|n| n.starts_with(&prefix))
        .unwrap_or_else(|| panic!("{capture}: no 320x200 dump of frame {frame}"));
    let raw = fs::read(dir.join(name)).unwrap();
    (Palette::parse_prefix(&raw).unwrap(), raw[768..].to_vec())
}

/// Which DAC entries a comparison checks.
#[derive(Clone, Copy)]
enum Dac {
    /// All 256.
    All,
    /// The entries the captured frame shows.
    Used,
}

/// Pixel differences (up to 12, and the count) and DAC differences of `ours` against frame
/// `f` of `capture`.
fn diff(ours: &Framebuffer, oracle: &Path, capture: &str, f: u32, dac: Dac) -> Vec<String> {
    let (pal, px) = frame(oracle, capture, f);
    let bad: Vec<usize> = (0..64_000).filter(|&i| ours.pixels[i] != px[i]).collect();
    let mut out: Vec<String> = bad
        .iter()
        .take(12)
        .map(|&i| {
            format!(
                "({}, {}): {} vs {}",
                i % 320,
                i / 320,
                ours.pixels[i],
                px[i]
            )
        })
        .collect();
    if bad.len() > 12 {
        out.push(format!("{} pixels in all", bad.len()));
    }
    let entries: Vec<usize> = match dac {
        Dac::All => (0..256).collect(),
        Dac::Used => px
            .iter()
            .copied()
            .collect::<BTreeSet<u8>>()
            .into_iter()
            .map(usize::from)
            .collect(),
    };
    for e in entries {
        if ours.palette.colors[e] != pal.colors[e] {
            out.push(format!(
                "DAC {e}: {:?} vs {:?}",
                ours.palette.colors[e], pal.colors[e]
            ));
        }
    }
    out
}

fn assert_frame(ours: &Framebuffer, oracle: &Path, capture: &str, f: u32, dac: Dac) {
    let d = diff(ours, oracle, capture, f, dac);
    assert!(d.is_empty(), "{capture} f{f}: {d:?}");
}

/// Steps a story screen `n` frames with no input.
fn run(mut s: StoryScreen, n: u32) -> StoryScreen {
    for _ in 0..n {
        s.step(&TickInput::default());
    }
    s
}

fn lit(p: &Palette) -> bool {
    p.colors[1..]
        .iter()
        .any(|c| c.r != 0 || c.g != 0 || c.b != 0)
}

/// The frames of `capture` dumped with `tag`, in order.
fn tagged(oracle: &Path, capture: &str, tag: &str) -> Vec<u32> {
    let suffix = format!("_320x200_{tag}.vga");
    let mut v: Vec<u32> = fs::read_dir(oracle.join(capture).join("fb"))
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(&suffix))
        .map(|n| n[1..9].parse().unwrap())
        .collect();
    v.sort_unstable();
    v
}

/// A capture frame and its differences from the port.
type Mismatch = (u32, Vec<String>);

/// Plays `s` against the dumps of `tag` from the first frame with a lit DAC to `until`: the
/// port's first lit frame is aligned with the capture's, then every dump is compared with the
/// port's frame at the same distance. Pixels always; the DAC (every entry) only where the
/// capture's DAC is complete, i.e. equal to the next frame's when that frame is dumped too (a
/// fade level is written over 1.5 frames, so a frame can start in the middle of a level; the
/// level-phase rule is screens §2.1).
///
/// Typing may run up to `ahead` frames early (M12-screens D1): a frame then passes when, for
/// some k in 0..=`ahead`, every pixel equals the port's frame k steps earlier or k + 1 steps
/// earlier (a dump can show a character half drawn, the beam passing while it is written),
/// with the DAC equal. Returns, per frame that passed only that way, its k, and the frames that
/// differ.
fn sequence(
    mut s: StoryScreen,
    oracle: &Path,
    capture: &str,
    tag: &str,
    until: u32,
    ahead: u32,
) -> (Vec<(u32, u32)>, Vec<Mismatch>) {
    let frames: Vec<u32> = tagged(oracle, capture, tag)
        .into_iter()
        .filter(|&f| f <= until)
        .collect();
    let first = *frames
        .iter()
        .find(|&&f| lit(&frame(oracle, capture, f).0))
        .expect("a lit frame");
    let mut step = 0;
    while !lit(&s.screen().palette) {
        s.step(&TickInput::default());
        step += 1;
        assert!(step < 200, "{capture} {tag}: the port never lights the DAC");
    }
    let lit_step = step;
    let last = lit_step + (frames.last().unwrap() - first);
    // The port's frames at the steps compared and the `ahead + 1` steps before each.
    let mut kept = std::collections::BTreeMap::new();
    let wanted: BTreeSet<u32> = frames
        .iter()
        .filter(|&&f| f >= first)
        .flat_map(|f| {
            let at = lit_step + (f - first);
            at.saturating_sub(ahead + 1)..=at
        })
        .collect();
    loop {
        if wanted.contains(&step) {
            kept.insert(step, s.screen().clone());
        }
        if step == last {
            break;
        }
        s.step(&TickInput::default());
        step += 1;
    }
    let (mut early, mut bad) = (Vec::new(), Vec::new());
    for (i, &f) in frames.iter().enumerate().filter(|&(_, &f)| f >= first) {
        let at = lit_step + (f - first);
        let complete = match frames.get(i + 1) {
            Some(&g) if g == f + 1 => frame(oracle, capture, f).0 == frame(oracle, capture, g).0,
            _ => true,
        };
        let mut d = diff(&kept[&at], oracle, capture, f, Dac::All);
        if !complete {
            d.retain(|x| !x.starts_with("DAC"));
        }
        if d.is_empty() {
            continue;
        }
        let dac_ok = !d.iter().any(|x| x.starts_with("DAC"));
        let px = frame(oracle, capture, f).1;
        let torn = (0..=ahead).find(|&k| {
            let (Some(a), Some(b)) = (kept.get(&(at - k)), kept.get(&(at - k - 1))) else {
                return false;
            };
            (0..64_000).all(|j| px[j] == a.pixels[j] || px[j] == b.pixels[j])
        });
        match torn {
            Some(k) if dac_ok => early.push((f, k)),
            _ => bad.push((f, d)),
        }
    }
    (early, bad)
}

/// [`sequence`] with no difference allowed but typing up to `ahead` frames early (M12-screens
/// D1). Returns the largest k seen.
fn assert_sequence(
    s: StoryScreen,
    oracle: &Path,
    capture: &str,
    tag: &str,
    until: u32,
    ahead: u32,
) -> u32 {
    let (early, bad) = sequence(s, oracle, capture, tag, until, ahead);
    for (f, d) in &bad {
        eprintln!("{capture} {tag} f{f}: {}", d[..d.len().min(4)].join("; "));
    }
    let first: Vec<_> = bad.iter().take(3).collect();
    assert!(
        bad.is_empty(),
        "{capture} {tag}: {} frames differ, first {first:?}",
        bad.len()
    );
    let k = early.iter().map(|&(_, k)| k).max().unwrap_or(0);
    eprintln!(
        "{capture} {tag}: {} frames typed early (up to {k})",
        early.len()
    );
    k
}

/// The debrief of `mission` after `steps` frames for each kill count 0..=60 that makes it equal
/// to frame `f` of `capture` (pixels and every DAC entry); the kill count of a flown flight is
/// asserted elsewhere (the L5 full-flight tests), so the debrief test only needs it to be unique.
fn debrief_kills(
    prog: &Program,
    (mission, kills_to_date, outcome): (u8, i16, bool),
    steps: u32,
    (oracle, capture, f): (&Path, &str, u32),
) -> Vec<i16> {
    (0..=60)
        .filter(|&k| {
            let s = run(prog.debrief(mission, k, kills_to_date, outcome), steps);
            diff(s.screen(), oracle, capture, f, Dac::All).is_empty()
        })
        .collect()
}

#[test]
fn e1_success_debrief_matches_c0200() {
    // screens §3.6, episode-program-flow §4.5: C-0200 (VE-5) flies flight 19 from a direct
    // start (kills to date 0); the debrief's DAC writes start at f1846 (`low.cmf` f1842). Frame
    // 2500 lies in its 10 s hold: every line typed, full brightness.
    let _data = require_data!();
    let oracle = require_oracle!();
    let (dir, res) = load().unwrap();
    let prog = program(&dir, &res, Episode::E1);
    let k = debrief_kills(&prog, (19, 0, true), 600, (&oracle, "C-0200", 2500));
    assert_eq!(k.len(), 1, "kill counts matching C-0200 f2500: {k:?}");
}

/// A debrief against its dumps in `capture` (tag `debrief`, from `low.cmf`'s open to after
/// the hold): the kill count is the unique one whose hold frame `hold` matches; then every dump
/// from the fade-in to `until` (the hold's last dump but one: the port's hold ends up to 6
/// frames before the original's, M12-screens D2), typing up to two frames early (D1).
fn check_debrief(
    episode: Episode,
    (mission, kills_to_date, outcome): (u8, i16, bool),
    capture: &str,
    (hold, until): (u32, u32),
) {
    let oracle = td_test_support::oracle_dir().unwrap();
    let (dir, res) = load().unwrap();
    let prog = program(&dir, &res, episode);
    let dumps = tagged(&oracle, capture, "debrief");
    let lit = *dumps
        .iter()
        .find(|&&f| lit(&frame(&oracle, capture, f).0))
        .unwrap();
    // The first dump at or after `hold`.
    let hold = *dumps.iter().find(|&&f| f >= hold).unwrap();
    let k = debrief_kills(
        &prog,
        (mission, kills_to_date, outcome),
        hold - lit + 2,
        (&oracle, capture, hold),
    );
    assert_eq!(k.len(), 1, "{capture}: kill counts matching f{hold}: {k:?}");
    let s = prog.debrief(mission, k[0], kills_to_date, outcome);
    assert_sequence(s, &oracle, capture, "debrief", until, 2);
}

#[test]
fn e1_failure_debrief_matches_c0274() {
    // screens §3.6: C-0274, flight 12 idle from a direct start with 45 kills to date: the
    // flight succeeds with outcome 0 (mission-logic §4.8, no convoy ship at y 199), so the
    // debrief shows labels[4] and the failure note instead of a rating.
    let _data = require_data!();
    let _oracle = require_oracle!();
    check_debrief(Episode::E1, (12, 45, false), "C-0274", (3700, 4073));
}

#[test]
fn e1_success_debriefs_match_c0275_and_c0282() {
    // screens §3.6, episode-program-flow §4.5 tiers: C-0275, flight 18 flown with e1f18.tdr
    // from a direct start with 150 kills to date (tier 8 or above); C-0282, flight 15 with
    // e1f15.tdr and 0 kills to date.
    let _data = require_data!();
    let _oracle = require_oracle!();
    check_debrief(Episode::E1, (18, 150, true), "C-0275", (4500, 4816));
    check_debrief(Episode::E1, (15, 0, true), "C-0282", (3700, 4094));
}

#[test]
fn e2_debriefs_match_c0277_c0283_c0286() {
    // E2 `UI_Debrief` is E1's (e2-program-flow §3.3) with the E2 texts (`e2.debrief`):
    // C-0277, flight 13 idle (success, outcome 1) with 300 kills to date (tier 11); C-0283,
    // flight 9 idle (success, outcome 0: the failure note) with 20; C-0286, flight 20 flown
    // with e2f20.tdr, 0 kills to date.
    let _data = require_data!();
    let _oracle = require_oracle!();
    check_debrief(Episode::E2, (13, 300, true), "C-0277", (2700, 3041));
    check_debrief(Episode::E2, (9, 20, false), "C-0283", (7500, 7824));
    check_debrief(Episode::E2, (20, 0, true), "C-0286", (3000, 3278));
}

#[test]
fn e3_debriefs_match_c0278_and_c0287() {
    // E3 `UI_Debrief` (e3-program-flow §3.3): 15 s, the keyboard flushed, `e3.debrief`:
    // C-0278, flight 5 idle (success, outcome 1) with 75 kills to date; C-0287, flight 2 idle
    // (success, outcome 0: E3's own failure note) with 12.
    let _data = require_data!();
    let _oracle = require_oracle!();
    check_debrief(Episode::E3, (5, 75, true), "C-0278", (5000, 5560));
    check_debrief(Episode::E3, (2, 12, false), "C-0287", (4600, 5176));
}

#[test]
fn e2_and_e3_pilot_dossiers_match_c0284_and_c0289() {
    // Ending A (e2-program-flow §4.5, e3-program-flow §4.5): C-0284, E2 flight 7 idle with 33
    // kills to date (the player is destroyed, Esc at the retry prompt); C-0289, E3 flight 8 on
    // C-0041's route into the hazard cells with 21.
    let _data = require_data!();
    let oracle = require_oracle!();
    let (dir, res) = load().unwrap();
    let e2 = program(&dir, &res, Episode::E2);
    assert_sequence(e2.player_dossier(33), &oracle, "C-0284", "dossier", 5982, 2);
    let e3 = program(&dir, &res, Episode::E3);
    assert_sequence(e3.player_dossier(21), &oracle, "C-0289", "dossier", 4440, 2);
}

#[test]
fn e1_hint_6_matches_c0280() {
    // screens §3.3: C-0280, block 6 with Esc in dialogue 24 (no skip test after it): hint 6.
    let _data = require_data!();
    let oracle = require_oracle!();
    let (dir, res) = load().unwrap();
    let prog = program(&dir, &res, Episode::E1);
    let s = prog.story(Step::H(6)).unwrap();
    assert_sequence(s, &oracle, "C-0280", "hint6", u32::MAX, 2);
}

#[test]
fn e1_stills_match_c0274_c0275_c0276_c0281_c0282() {
    // asset-load-sites §4, text §4.5, screens §3.9: the four still sets with their captions
    // (episode-program-flow §4.4: P1 block 16 pre-flight, P2 block 20, P3 block 12, P4 block
    // 15), `td1-15` (block 18) and the `td1-18` card (block 18), each from its fade-in to the
    // last dump but one of its hold (M12-screens D2).
    let _data = require_data!();
    let oracle = require_oracle!();
    let (dir, res) = load().unwrap();
    let prog = program(&dir, &res, Episode::E1);
    let cases = [
        (Step::P(1, [0xBF5, 0xC16, 0x5A4]), "C-0281", "td1-16", 5719),
        (Step::P(3, [0x96F, 0x989, 0x9A5]), "C-0274", "td1-14", 14285),
        (Step::P(4, [0xB6C, 0xB88, 0xBBE]), "C-0282", "td1-13", 11771),
        (Step::P15, "C-0275", "td1-15", 8176),
        (
            Step::C(td_flow::campaign::Bg::Td1_18, [0xCE7, 0xCFC, 0x5A4]),
            "C-0275",
            "td1-18",
            10091,
        ),
    ];
    for (step, capture, tag, until) in cases {
        let s = prog.story(step).unwrap();
        assert_sequence(s, &oracle, capture, tag, until, 0);
    }
    // P2 `td1-20` (BUG-13, U-522): the original shows the picture one pixel to the right of
    // asset-load-sites §4's (0, 50): screen (1 + c, 50 + r) = byte c of row r for c = 0..318,
    // column 0 black (as `td1-15` and the `.scr` scenes, OB-003). C-0276's hold equals the
    // port's frame with rows 50..149 so shifted; the captions (rows 170..199) do not move.
    let s = run(prog.story(Step::P(2, [0x5A4, 0x5A4, 0x5A4])).unwrap(), 400);
    let mut shifted = s.screen().clone();
    for y in 50..150 {
        let row = &mut shifted.pixels[y * 320..(y + 1) * 320];
        row.copy_within(0..319, 1);
        row[0] = 0;
    }
    assert_ne!(
        diff(s.screen(), &oracle, "C-0276", 8698, Dac::All),
        Vec::<String>::new(),
        "BUG-13 fixed: compare td1-20 with assert_sequence"
    );
    assert_frame(&shifted, &oracle, "C-0276", 8698, Dac::All);
}

#[test]
fn about_and_order_screens_match_c0290_and_c0291() {
    // E2 menu item 6 (e2-program-flow §3.3: `aboutus.scr`, 30 s; C-0290), E3 items 6 and 7
    // (e3-program-flow §3.3: 30 s and 45 s; C-0291), no keys, from the fade-in to the last
    // dump but one of the hold.
    let _data = require_data!();
    let oracle = require_oracle!();
    let (dir, res) = load().unwrap();
    let e2 = program(&dir, &res, Episode::E2);
    let s =
        td_flow::scenes_e2::scene_card(&dir, &e2.assets, td_flow::scenes_e2::Bg::Aboutus, [""; 3])
            .unwrap();
    assert_sequence(s, &oracle, "C-0290", "about", 2562, 0);
    let e3 = program(&dir, &res, Episode::E3);
    for (bg, tag, until) in [
        (td_flow::scenes_e3::Bg::Aboutus, "about", 2560),
        (td_flow::scenes_e3::Bg::Order, "order", 5883),
    ] {
        let s = td_flow::scenes_e3::scene_card(&dir, &e3.assets, bg, [""; 3]).unwrap();
        assert_sequence(s, &oracle, "C-0291", tag, until, 0);
    }
}

#[test]
fn ending_c_cards_match_c0279_and_c0285() {
    // Ending C: E1 C:intro2[0ED4/0E9B/0EC3] after flight 20 lost (episode-program-flow §4.5;
    // C-0279, flight 20 idle, Esc at the retry prompt); E2 in2[20.8/_/_] after flight 18 lost
    // (e2-program-flow §4.5; C-0285, flight 18 idle).
    let _data = require_data!();
    let oracle = require_oracle!();
    let (dir, res) = load().unwrap();
    let e1 = program(&dir, &res, Episode::E1);
    let s = e1
        .story(Step::C(
            td_flow::campaign::Bg::Intro2,
            [0xED4, 0xE9B, 0xEC3],
        ))
        .unwrap();
    assert_sequence(s, &oracle, "C-0279", "endingC", 11094, 0);
    let e2 = program(&dir, &res, Episode::E2);
    use td_flow::campaign_e2::{self, BLANK, Cap};
    let s = campaign_e2::story(
        &e2,
        campaign_e2::Step::S(td_flow::scenes_e2::Bg::In2, [Cap::C(20, 8), BLANK, BLANK]),
    )
    .unwrap();
    assert_sequence(s, &oracle, "C-0285", "endingC", 5916, 0);
}

#[test]
fn episode_end_cards_match_c0276_c0286_c0288() {
    // The end cards (title font over the palette the screen before left in g_Palette, text
    // rev 3 §4.5.2): E1 T[0E51] after the td1-20 still set (C-0276); E2 T(20.5) after
    // dialogue 127 (C-0286); E3 T(end) after dialogue 109 (C-0288). Pixels and the entries the
    // card shows, in its hold.
    let _data = require_data!();
    let oracle = require_oracle!();
    let (dir, res) = load().unwrap();
    let e1 = program(&dir, &res, Episode::E1);
    let s = run(e1.story(Step::T(0xE51)).unwrap(), 200);
    assert_frame(s.screen(), &oracle, "C-0276", 10304, Dac::Used);
    let e2 = program(&dir, &res, Episode::E2);
    let card = td_flow::campaign_e2::Step::Tc(td_flow::campaign_e2::Cap::C(20, 5));
    let s = run(td_flow::campaign_e2::story(&e2, card).unwrap(), 200);
    assert_frame(s.screen(), &oracle, "C-0286", 9809, Dac::Used);
    let e3 = program(&dir, &res, Episode::E3);
    let card = td_flow::campaign_e3::Step::TEnd;
    let s = run(td_flow::campaign_e3::story(&e3, card).unwrap(), 200);
    assert_frame(s.screen(), &oracle, "C-0288", 7417, Dac::Used);
}

#[test]
fn e1_title_cards_after_a_dialogue_match_c0274_and_c0275() {
    // T[0938] and T[05FC] in block 12's post-flight story (C-0274) and T[0560] in block 18's
    // (C-0275): pixels and shown entries in the hold.
    let _data = require_data!();
    let oracle = require_oracle!();
    let (dir, res) = load().unwrap();
    let prog = program(&dir, &res, Episode::E1);
    for (addr, capture, f) in [
        (0x938, "C-0274", 9752),
        (0x5FC, "C-0274", 11659),
        (0x560, "C-0275", 6220),
    ] {
        let s = run(prog.story(Step::T(addr)).unwrap(), 200);
        assert_frame(s.screen(), &oracle, capture, f, Dac::Used);
    }
}

#[test]
fn e1_pilot_dossier_matches_c0270() {
    // screens §3.5, episode-program-flow §4.5 ending A: C-0270, flight 1 idle from a direct
    // start with 7 kills to date, Esc at the retry prompt. Every dump from the fade-in through
    // the typing, the delays and the 25 s hold (f2915..f5047).
    let _data = require_data!();
    let oracle = require_oracle!();
    let (dir, res) = load().unwrap();
    let prog = program(&dir, &res, Episode::E1);
    assert_sequence(
        prog.player_dossier(7),
        &oracle,
        "C-0270",
        "dossier",
        5047,
        2,
    );
}

#[test]
fn e1_enemy_dossier_matches_c0271() {
    // screens §3.5: C-0271, block 5's pre-flight story without keys; the dossier from its
    // fade-in through line 8 (f2945..f3206).
    let _data = require_data!();
    let oracle = require_oracle!();
    let (dir, res) = load().unwrap();
    let prog = program(&dir, &res, Episode::E1);
    let s = prog.story(Step::A).unwrap();
    assert_sequence(s, &oracle, "C-0271", "enemy", 3206, 2);
    // BUG-12 / U-520: the original types line 10 straight after line 8 (f3211) and never types
    // line 9 (ink 39 over line 4 at pen y 109); the port follows screens §3.5 and types it.
    // In the hold (f3501) the screens differ exactly in line 9's pixels: rows 110..114, the
    // port's ink 39 or its opaque background where the original keeps line 4 in ink 40.
    let s = run(prog.story(Step::A).unwrap(), 900);
    let (pal, px) = frame(&oracle, "C-0271", 3501);
    assert_eq!(s.screen().palette, pal, "DAC in the hold");
    let differ: Vec<usize> = (0..64_000)
        .filter(|&i| s.screen().pixels[i] != px[i])
        .collect();
    assert!(!differ.is_empty(), "BUG-12 fixed: update this test");
    for i in differ {
        let (x, y, ours, theirs) = (i % 320, i / 320, s.screen().pixels[i], px[i]);
        assert!(
            (110..=114).contains(&y) && (40..160).contains(&x),
            "({x}, {y}): {ours} vs {theirs}"
        );
        assert!(ours == 39 || ours == 0, "({x}, {y}): {ours}");
        assert!(theirs == 40 || theirs == 0, "({x}, {y}): {theirs}");
    }
}

#[test]
fn e1_story_so_far_matches_c0272() {
    // screens §3.8: C-0272, menu item 5 with no keys: page 1 (`javelin.td`, 30 s) from its
    // fade-in (steady f439..f2420) and page 2 (`city.td`, 45 s, steady f2528..f5506).
    let _data = require_data!();
    let oracle = require_oracle!();
    let (dir, res) = load().unwrap();
    let prog = program(&dir, &res, Episode::E1);
    let lines = res.texts("e1.background_story").unwrap().to_vec();
    let page = || story::story_so_far(&dir, &prog.assets, &lines).unwrap();
    assert_sequence(page(), &oracle, "C-0272", "page1", 2420, 0);
    // Page 2: shown after page 1's wait and fade-out; its steady frame.
    let first = run(page(), 200).screen().pixels.clone();
    let mut s = page();
    let mut n = 0;
    while s.screen().pixels == first || !lit(&s.screen().palette) {
        s.step(&TickInput::default());
        n += 1;
        assert!(n < 4_000, "page 2 never shown");
    }
    let s = run(s, 300);
    assert_frame(s.screen(), &oracle, "C-0272", 3006, Dac::All);
}

#[test]
fn e1_assignment_orders_match_c0273() {
    // screens §3.4: C-0273, block 2's pre-flight story; the orders from the fade-in through
    // the typing into the 5 s hold (f2604..f2749).
    let _data = require_data!();
    let oracle = require_oracle!();
    let (dir, res) = load().unwrap();
    let prog = program(&dir, &res, Episode::E1);
    let s = prog.story(Step::O).unwrap();
    assert_sequence(s, &oracle, "C-0273", "orders", 2749, 2);
}

#[test]
fn e1_hint_2_matches_c0273() {
    // screens §3.3, text §4.5: hint 2 (the `secret.pic` banner and the typed lines) in C-0273
    // from the fade-in to the wait for a key (f3452..f3747).
    let _data = require_data!();
    let oracle = require_oracle!();
    let (dir, res) = load().unwrap();
    let prog = program(&dir, &res, Episode::E1);
    let s = prog.story(Step::H(2)).unwrap();
    assert_sequence(s, &oracle, "C-0273", "hint2", 3747, 2);
}
