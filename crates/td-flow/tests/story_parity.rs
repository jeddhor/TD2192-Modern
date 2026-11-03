//! Story screens and the campaign against the original data and oracle capture C-0002
//! (`docs/spec/flow/episode-program-flow.md` §6; `docs/re/dynamic-observations.md`). Data-gated
//! (TD2192_DATA; the frame comparisons also TD2192_ORACLE). No captured pixels are stored.

use std::cell::Cell;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::rc::Rc;

use td_assets::{DataDir, ExeResources};
use td_audio::AudioCommand;
use td_flow::campaign::{Campaign, E1_BLOCKS, Step};
use td_flow::program::Program;
use td_flow::settings::Settings;
use td_flow::story::{self, StoryAssets, StoryScreen};
use td_flow::{FlightLauncher, FlightScreen, FlowContext, PlaceholderLauncher};
use td_formats::palette::Palette;
use td_input::{Key, TickInput};
use td_test_support::{require_data, require_oracle};

fn oracle_frame(oracle: &Path, name: &str) -> (Palette, Vec<u8>) {
    let raw = fs::read(oracle.join("C-0002").join("fb").join(name)).unwrap();
    (Palette::parse_prefix(&raw).unwrap(), raw[768..].to_vec())
}

/// Runs the fade-in (and any typing) of a story screen.
fn fade_in(s: &mut StoryScreen, frames: u32) {
    for _ in 0..frames {
        s.step(&TickInput::default());
    }
}

/// Pixel differences in `rows` and DAC differences on the entries the frame uses there.
fn compare(s: &StoryScreen, oracle: &Path, name: &str, rows: std::ops::Range<usize>) {
    let (dac, frame) = oracle_frame(oracle, name);
    let ours = s.screen();
    let range = rows.start * 320..rows.end * 320;
    let diffs: Vec<String> = range
        .clone()
        .filter(|&i| ours.pixels[i] != frame[i])
        .take(12)
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
    assert!(diffs.is_empty(), "{name}: pixels differ: {diffs:?}");
    let used: BTreeSet<u8> = frame[range].iter().copied().collect();
    for e in used {
        let e = usize::from(e);
        assert_eq!(
            ours.palette.colors[e], dac.colors[e],
            "{name}: DAC entry {e}"
        );
    }
}

fn load() -> Option<(DataDir, ExeResources)> {
    let data = td_test_support::data_dir()?;
    let dir = DataDir::open(&data).unwrap();
    let res = ExeResources::load(&dir).unwrap();
    Some((dir, res))
}

#[test]
fn every_string_reference_resolves() {
    let _data = require_data!();
    let (_, res) = load().unwrap();
    let mut refs = BTreeSet::new();
    for b in &E1_BLOCKS {
        for s in b
            .pre
            .iter()
            .chain(b.post)
            .chain(b.post_failed.unwrap_or(&[]))
        {
            match *s {
                Step::T(a) => {
                    refs.insert(a);
                }
                Step::C(_, l) | Step::P(_, l) => refs.extend(l),
                _ => {}
            }
        }
    }
    refs.extend([0xE6B, 0xE9B, 0xEC3, 0xED4]);
    for a in refs {
        assert!(
            res.text_at("file0001.exe", usize::from(a)).is_some(),
            "0000:{a:04X}"
        );
    }
}

#[test]
fn title_and_hangar_cards_match_the_original() {
    let _data = require_data!();
    let oracle = require_oracle!();
    let (dir, res) = load().unwrap();
    let assets = StoryAssets::load(&dir).unwrap();
    let text = |a: usize| res.text_at("file0001.exe", a).unwrap();
    // Mission 1: T[0478], then C:hangar[0494/04A1/04C1] (episode-program-flow §4.4).
    let mut t = story::text_card(&assets, text(0x478));
    fade_in(&mut t, 34);
    compare(&t, &oracle, "f00001162_320x200_mission-title.vga", 0..200);
    let mut h = story::scene_card(
        &dir,
        &assets,
        "hangar.scr",
        [text(0x494), text(0x4A1), text(0x4C1)],
    )
    .unwrap();
    fade_in(&mut h, 34);
    compare(&h, &oracle, "f00001630_320x200_hangar.vga", 0..200);
}

#[test]
fn hint_1_matches_the_original() {
    let _data = require_data!();
    let oracle = require_oracle!();
    let (dir, res) = load().unwrap();
    let assets = StoryAssets::load(&dir).unwrap();
    let hints = res.texts("e1.hints").unwrap();
    let mut h = story::hint(&dir, &assets, hints, 1).unwrap();
    fade_in(&mut h, 2000);
    // Banner and the first paragraph (rows 0..99); frame 3332 is still typing the second.
    compare(&h, &oracle, "f00003332_320x200_hint.vga", 0..100);
}

#[test]
fn main_menu_matches_the_original() {
    // ASSUMPTION(U-120): cursor colours from this frame; 5 pixels of the frame are not in
    // td.men (unexplained) and are tolerated.
    let _data = require_data!();
    let oracle = require_oracle!();
    let (dir, res) = load().unwrap();
    let ep = td_flow::Episode1::new(&dir, &res, Box::new(PlaceholderLauncher::default())).unwrap();
    let mut fb = td_render::Framebuffer::new();
    td_flow::Screen::draw(&ep, &mut fb);
    let (dac, frame) = oracle_frame(&oracle, "f00000997_320x200_main-menu.vga");
    let n = (0..64_000).filter(|&i| fb.pixels[i] != frame[i]).count();
    assert!(n <= 5, "{n} pixels differ");
    for e in 0..255 {
        assert_eq!(fb.palette.colors[e], dac.colors[e], "entry {e}");
    }
}

/// Plays one block, pressing `key(campaign)` every 40 frames; returns the result and the
/// audio commands.
fn play_block(
    mission: u8,
    key: impl Fn(&Campaign) -> Key,
) -> (td_flow::campaign::BlockResult, Vec<AudioCommand>) {
    let (dir, res) = load().unwrap();
    let settings = Settings::default();
    let mut audio = Vec::new();
    let mut all = Vec::new();
    let mut prog = Program::new(&dir, &res, Box::new(PlaceholderLauncher::default())).unwrap();
    let mut c = Campaign::new(mission, 0);
    for frame in 0..400_000u32 {
        let mut ctx = FlowContext {
            data: &dir,
            resources: &res,
            settings: &settings,
            audio: &mut audio,
        };
        let pressed = if frame % 40 == 0 {
            vec![key(&c)]
        } else {
            Vec::new()
        };
        let input = TickInput {
            pressed,
            ..TickInput::default()
        };
        let r = c.step(&mut prog, &mut ctx, &input);
        all.append(&mut audio);
        if let Some(r) = r {
            return (r, all);
        }
    }
    panic!("mission {mission} block never ended");
}

#[test]
fn every_block_plays_to_the_end() {
    let _data = require_data!();
    for m in 1..=20 {
        let (r, audio) = play_block(m, |_| Key::SPACE);
        assert!(r.advance && !r.game_over, "mission {m}: {r:?}");
        assert!(
            audio
                .iter()
                .any(|a| matches!(a, AudioCommand::PlayMusic { .. })),
            "mission {m}: music"
        );
    }
}

#[test]
fn death_without_retry_is_game_over() {
    let _data = require_data!();
    // D = died; Esc at the retry prompt declines; ending A.
    let d = Key::letter('d').unwrap();
    let (r, audio) = play_block(1, |c| if c.in_retry_prompt() { Key::ESCAPE } else { d });
    assert!(r.game_over && !r.advance, "{r:?}");
    assert!(
        audio
            .iter()
            .any(|a| matches!(a, AudioCommand::PlayMusic { looping: false, .. }))
    );
}

type Counter = Rc<Cell<u32>>;

/// A launcher that counts its `Random` calls and flights and returns `r` (placeholder
/// flights).
#[derive(Debug)]
struct CountingLauncher {
    r: u16,
    calls: Counter,
    launches: Counter,
}

impl FlightLauncher for CountingLauncher {
    fn launch(&mut self, ctx: &mut FlowContext<'_>, flight: u8) -> Box<dyn FlightScreen> {
        self.launches.set(self.launches.get() + 1);
        PlaceholderLauncher::default().launch(ctx, flight)
    }

    fn random(&mut self, _n: u16) -> u16 {
        self.calls.set(self.calls.get() + 1);
        self.r
    }
}

fn counting(r: u16) -> (CountingLauncher, Counter, Counter) {
    let (calls, launches) = (Counter::default(), Counter::default());
    let l = CountingLauncher {
        r,
        calls: calls.clone(),
        launches: launches.clone(),
    };
    (l, calls, launches)
}

/// Runs a block with `key(campaign, frame)` as the input of each frame until it ends or
/// `until()` holds; returns the frame count.
fn drive(
    prog: &mut Program,
    c: &mut Campaign,
    key: impl Fn(&Campaign, u32) -> Option<Key>,
    until: impl Fn() -> bool,
    audio: &mut Vec<AudioCommand>,
) -> u32 {
    let (dir, res) = (prog.data.clone(), prog.res.clone());
    let settings = Settings::default();
    for frame in 0..400_000u32 {
        let mut ctx = FlowContext {
            data: &dir,
            resources: &res,
            settings: &settings,
            audio,
        };
        let input = TickInput {
            pressed: key(c, frame).into_iter().collect(),
            ..TickInput::default()
        };
        if c.step(prog, &mut ctx, &input).is_some() || until() {
            return frame;
        }
    }
    panic!("block never ended");
}

#[test]
fn script_choice_matches_the_original() {
    // screens §3.2; C-0002 f710 (entries 85, 92, 102 and the layout).
    let _data = require_data!();
    let oracle = require_oracle!();
    let (dir, res) = load().unwrap();
    let settings = Settings::default();
    let mut audio = Vec::new();
    let mut ctx = FlowContext {
        data: &dir,
        resources: &res,
        settings: &settings,
        audio: &mut audio,
    };
    let launcher = Box::new(PlaceholderLauncher::default());
    let mut ep = td_flow::Episode1::new(&dir, &res, launcher).unwrap();
    for _ in 0..40 {
        td_flow::Screen::tick(&mut ep, &mut ctx, &TickInput::default());
    }
    let mut fb = td_render::Framebuffer::new();
    td_flow::Screen::draw(&ep, &mut fb);
    let (dac, frame) = oracle_frame(&oracle, "f00000710_320x200_script-choice.vga");
    let d: Vec<usize> = (0..64_000).filter(|&i| fb.pixels[i] != frame[i]).collect();
    let first = d.first().map(|i| (i % 320, i / 320));
    assert!(d.is_empty(), "{} pixels differ, first {first:?}", d.len());
    for e in frame.iter().copied().collect::<BTreeSet<u8>>() {
        let e = usize::from(e);
        assert_eq!(fb.palette.colors[e], dac.colors[e], "entry {e}");
    }
}

#[test]
fn retry_prompt_matches_the_original() {
    // screens §3.7; C-0002 f6300: exactly one Random(10) result gives the captured screen.
    let _data = require_data!();
    let oracle = require_oracle!();
    let (dir, res) = load().unwrap();
    let (dac, frame) = oracle_frame(&oracle, "f00006300_320x200_retry-screen.vga");
    let mut matches = Vec::new();
    for r in 0..10 {
        let (l, calls, _) = counting(r);
        let mut prog = Program::new(&dir, &res, Box::new(l)).unwrap();
        let mut s = prog.retry_prompt();
        assert_eq!(calls.get(), 1, "one Random call per prompt");
        fade_in(&mut s, 40);
        let ours = s.screen();
        if (0..64_000).all(|i| ours.pixels[i] == frame[i]) {
            for e in [39, 40] {
                assert_eq!(ours.palette.colors[e], dac.colors[e], "entry {e}");
            }
            matches.push(r);
        }
    }
    assert_eq!(matches.len(), 1, "taunts matching f6300: {matches:?}");
}

#[test]
fn a_retry_draws_one_random_number() {
    let _data = require_data!();
    let (dir, res) = load().unwrap();
    let (l, calls, _) = counting(3);
    let mut prog = Program::new(&dir, &res, Box::new(l)).unwrap();
    let mut c = Campaign::new(1, 0);
    let d = Key::letter('d').unwrap();
    let key = |c: &Campaign, f: u32| {
        f.is_multiple_of(40)
            .then_some(if c.in_retry_prompt() { Key::ESCAPE } else { d })
    };
    drive(&mut prog, &mut c, key, || false, &mut Vec::new());
    assert_eq!(calls.get(), 1);
}

#[test]
fn esc_on_the_title_card_goes_straight_to_the_flight() {
    // screens §3.12, test vector 3: no dialogue 1, no `low` cue and no hint before flight 1.
    let _data = require_data!();
    let (dir, res) = load().unwrap();
    let (l, _, launches) = counting(0);
    let mut prog = Program::new(&dir, &res, Box::new(l)).unwrap();
    let mut c = Campaign::new(1, 0);
    let mut audio = Vec::new();
    let esc = |_: &Campaign, f: u32| (f == 40).then_some(Key::ESCAPE);
    let launched = || launches.get() > 0;
    let frame = drive(&mut prog, &mut c, esc, launched, &mut audio);
    assert!(
        launched() && frame < 200,
        "flight launched at frame {frame}"
    );
    // The only music so far is the flight's own song, started by the launch.
    let songs = audio
        .iter()
        .filter(|a| matches!(a, AudioCommand::PlayMusic { .. }))
        .count();
    assert_eq!(songs, 1);
}
