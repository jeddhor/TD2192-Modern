//! The Episode 2 front end against the original data and the oracle captures C-0015 and C-0016
//! (`docs/spec/flow/e2-program-flow.md` rev 1 §4, §6, §8), and the block sequence of
//! `td_flow::campaign_e2`. Data-gated (TD2192_DATA; the frame comparisons also
//! TD2192_ORACLE). No captured pixels are stored.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::rc::Rc;

use td_assets::{DataDir, Episode, ExeResources};
use td_audio::AudioCommand;
use td_flow::campaign_e2::{self, BlockResult, CampaignE2, E2_BLOCKS, Step};
use td_flow::program::Program;
use td_flow::scenes_e2::{self, Bg};
use td_flow::settings::Settings;
use td_flow::story::{StoryAssets, StoryScreen};
use td_flow::{
    Episode2, FlightLauncher, FlightResult, FlightScreen, FlowContext, Screen, Transition,
};
use td_formats::palette::Palette;
use td_input::{Key, TickInput};
use td_render::Framebuffer;
use td_test_support::{require_data, require_oracle};

fn load() -> Option<(DataDir, ExeResources)> {
    let data = td_test_support::data_dir()?;
    let dir = DataDir::open(&data).unwrap();
    let res = ExeResources::load(&dir).unwrap();
    Some((dir, res))
}

fn oracle_frame(oracle: &Path, capture: &str, name: &str) -> (Palette, Vec<u8>) {
    let raw = fs::read(oracle.join(capture).join("fb").join(name)).unwrap();
    (Palette::parse_prefix(&raw).unwrap(), raw[768..].to_vec())
}

/// Every pixel, and the DAC entries the captured frame uses.
fn compare(ours: &Framebuffer, oracle: &Path, capture: &str, name: &str) {
    let (dac, frame) = oracle_frame(oracle, capture, name);
    let diffs: Vec<String> = (0..64_000)
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
    assert!(
        diffs.is_empty(),
        "{capture} {name}: pixels differ: {diffs:?}"
    );
    for e in frame.iter().copied().collect::<BTreeSet<u8>>() {
        let e = usize::from(e);
        assert_eq!(
            ours.palette.colors[e], dac.colors[e],
            "{capture} {name}: DAC entry {e}"
        );
    }
}

fn faded_in(mut s: StoryScreen) -> StoryScreen {
    for _ in 0..40 {
        s.step(&TickInput::default());
    }
    s
}

/// A flight that ends at once with a scripted result.
#[derive(Debug)]
struct Instant(FlightResult);

impl Screen for Instant {
    fn tick(&mut self, _ctx: &mut FlowContext<'_>, _input: &TickInput) -> Transition {
        Transition::Pop
    }
    fn draw(&self, fb: &mut Framebuffer) {
        fb.clear(0);
    }
}

impl FlightScreen for Instant {
    fn result(&self) -> FlightResult {
        self.0
    }
}

type Shared<T> = Rc<RefCell<T>>;

/// Flies the scripted results in order (then plain successes), counting `Random` calls.
#[derive(Debug, Default)]
struct Scripted {
    results: Shared<Vec<FlightResult>>,
    flights: Shared<Vec<u8>>,
    randoms: Rc<Cell<u32>>,
}

impl FlightLauncher for Scripted {
    fn launch(&mut self, _ctx: &mut FlowContext<'_>, flight: u8) -> Box<dyn FlightScreen> {
        self.flights.borrow_mut().push(flight);
        let mut results = self.results.borrow_mut();
        let r = if results.is_empty() {
            FlightResult {
                kills: 2,
                ..FlightResult::default()
            }
        } else {
            results.remove(0)
        };
        Box::new(Instant(r))
    }

    fn random(&mut self, _n: u16) -> u16 {
        self.randoms.set(self.randoms.get() + 1);
        0
    }
}

fn died() -> FlightResult {
    FlightResult {
        died: true,
        kills: 1,
        ..FlightResult::default()
    }
}

fn result(f: impl FnOnce(&mut FlightResult)) -> FlightResult {
    let mut r = FlightResult {
        kills: 4,
        ..FlightResult::default()
    };
    f(&mut r);
    r
}

struct Run {
    block: BlockResult,
    flights: Vec<u8>,
    randoms: u32,
    audio: Vec<AudioCommand>,
    /// Story steps in the order they were shown.
    shown: Vec<Step>,
}

/// Plays block `mission` with the scripted flight results; at a retry prompt presses
/// `retry(n)` for the n-th prompt (true = fly again), elsewhere Space every 20 frames.
fn play(mission: u8, results: Vec<FlightResult>, retry: impl Fn(u32) -> bool) -> Run {
    let (dir, res) = load().unwrap();
    let launcher = Scripted::default();
    let (flights, randoms) = (launcher.flights.clone(), launcher.randoms.clone());
    *launcher.results.borrow_mut() = results;
    let mut prog = Program::for_episode(&dir, &res, Box::new(launcher), Episode::E2).unwrap();
    let settings = Settings::default();
    let (mut audio, mut all, mut shown) = (Vec::new(), Vec::new(), Vec::new());
    let mut c = CampaignE2::new(mission, 10);
    let mut prompts = 0;
    for frame in 0..2_000_000u32 {
        let mut ctx = FlowContext {
            data: &dir,
            resources: &res,
            settings: &settings,
            audio: &mut audio,
        };
        if let Some(s) = c.current_step()
            && shown.last() != Some(&s)
        {
            shown.push(s);
        }
        let key = if frame % 20 != 0 {
            None
        } else if c.in_retry_prompt() {
            prompts += 1;
            Some(if retry(prompts) {
                Key::SPACE
            } else {
                Key::ESCAPE
            })
        } else {
            Some(Key::SPACE)
        };
        let input = TickInput {
            pressed: key.into_iter().collect(),
            ..TickInput::default()
        };
        let r = c.step(&mut prog, &mut ctx, &input);
        all.append(&mut audio);
        if let Some(block) = r {
            return Run {
                block,
                flights: flights.borrow().clone(),
                randoms: randoms.get(),
                audio: all,
                shown,
            };
        }
    }
    panic!("block {mission} never ended");
}

fn songs(audio: &[AudioCommand]) -> Vec<(usize, bool)> {
    audio
        .iter()
        .filter_map(|a| match a {
            AudioCommand::PlayMusic { song, looping } => Some((song.events.len(), *looping)),
            _ => None,
        })
        .collect()
}

#[test]
fn every_caption_and_title_resolves() {
    let _data = require_data!();
    let (dir, res) = load().unwrap();
    let prog = Program::for_episode(
        &dir,
        &res,
        Box::new(td_flow::PlaceholderLauncher::default()),
        Episode::E2,
    )
    .unwrap();
    for (b, block) in E2_BLOCKS.iter().enumerate() {
        for s in block.pre.iter().chain(block.post) {
            match *s {
                Step::T(n) => assert!(
                    res.text(&format!("e2.mission.{n:02}.title")).is_some(),
                    "title {n}"
                ),
                Step::S(_, caps) => {
                    for c in caps {
                        let (id, k) = c.resource();
                        let list = res.texts(&id).unwrap_or_else(|| panic!("{id}"));
                        assert!(k < list.len(), "block {}: {id}[{k}]", b + 1);
                    }
                }
                Step::Tc(c) => assert!(!campaign_e2::caption(&prog, c).is_empty()),
                _ => {}
            }
        }
    }
    assert_eq!(campaign_e2::caption(&prog, campaign_e2::BLANK), " ");
}

#[test]
fn every_block_plays_to_the_end() {
    let _data = require_data!();
    for m in 1..=20 {
        let r = play(m, Vec::new(), |_| false);
        assert!(
            r.block.advance && !r.block.game_over,
            "block {m}: {:?}",
            r.block
        );
        assert_eq!(r.block.mission_kills, 2);
        assert_eq!(r.flights, [m], "block {m}: one flight");
        assert_eq!(r.randoms, 0, "block {m}: no retry prompt");
    }
}

#[test]
fn block_1_follows_the_spec() {
    // §4.4 block 1 and §6 vector 4: marauder card, D1, T01, ..., D4, flight 1, debrief,
    // hangar, D5, bunker, D6.
    let _data = require_data!();
    let r = play(1, Vec::new(), |_| false);
    let screens: Vec<String> = r
        .shown
        .iter()
        .map(|s| match s {
            Step::S(bg, _) => format!("{bg:?}"),
            Step::D(n) => format!("D{n}"),
            Step::T(n) => format!("T{n}"),
            other => format!("{other:?}"),
        })
        .collect();
    assert_eq!(
        screens,
        [
            "Mar", "D1", "T1", "Lab", "D2", "Sof", "D3", "Hg", "D4", "Hg", "D5", "Bun", "D6"
        ]
    );
}

#[test]
fn death_declined_is_ending_a() {
    let _data = require_data!();
    let r = play(4, vec![died()], |_| false);
    assert!(r.block.game_over && !r.block.advance, "{:?}", r.block);
    assert_eq!(r.randoms, 1, "one Random(10) per prompt");
    assert!(r.shown.contains(&Step::Dossier));
    // death.cmf starts looping, then the loop flag goes off and on again.
    let loops: Vec<bool> = r
        .audio
        .iter()
        .filter_map(|a| match a {
            AudioCommand::SetMusicLoop(l) => Some(*l),
            _ => None,
        })
        .collect();
    assert_eq!(loops, [false, true]);
}

#[test]
fn death_accepted_flies_again() {
    let _data = require_data!();
    let r = play(4, vec![died()], |_| true);
    assert!(r.block.advance && !r.block.game_over);
    assert_eq!(r.flights, [4, 4]);
    assert_eq!(r.block.mission_kills, 2, "only the last attempt counts");
}

#[test]
fn abort_returns_without_advance() {
    let _data = require_data!();
    let r = play(5, vec![result(|r| r.aborted = true)], |_| false);
    assert_eq!(
        r.block,
        BlockResult {
            advance: false,
            game_over: false,
            mission_kills: 4,
            replay: false,
            ..BlockResult::default()
        }
    );
    assert!(!r.shown.contains(&Step::D(25)), "no post-flight story");
}

#[test]
fn block_3_outcome_rule() {
    let _data = require_data!();
    // Outcome 1: D14, then the bunker card and D15.
    let ok = play(3, Vec::new(), |_| false);
    assert!(ok.shown.contains(&Step::D(14)) && !ok.shown.contains(&Step::D(247)));
    // Outcome 0, declined: D247, the prompt, ending A (§6 vector 9).
    let failed = play(3, vec![result(|r| r.outcome = false)], |_| false);
    assert!(failed.shown.contains(&Step::D(247)));
    assert!(failed.block.game_over && !failed.block.advance);
    assert!(failed.shown.contains(&Step::Dossier));
    // Accepted: flight 3 again, a second debrief, then outcome 1 and D14.
    let again = play(3, vec![result(|r| r.outcome = false)], |_| true);
    assert_eq!(again.flights, [3, 3]);
    assert!(again.shown.contains(&Step::D(14)) && again.block.advance);
}

#[test]
fn blocks_8_and_17_replay_after_a_declined_outcome_retry() {
    // §6 vector 7: the block is offered again and the attempt's kills count (OB-027).
    let _data = require_data!();
    for m in [8, 17] {
        let r = play(m, vec![result(|r| r.outcome = false)], |_| false);
        assert_eq!(
            r.block,
            BlockResult {
                advance: true,
                game_over: false,
                mission_kills: 4,
                replay: true,
                ..BlockResult::default()
            },
            "block {m}"
        );
        assert!(!r.shown.contains(&Step::D(48)) && !r.shown.contains(&Step::D(102)));
    }
}

#[test]
fn block_16_ending_b() {
    // Unreachable in the original (OB-025), kept: outcome 0, declined → ending B.
    let _data = require_data!();
    let r = play(16, vec![result(|r| r.outcome = false)], |_| false);
    assert!(r.block.game_over);
    assert!(
        r.shown
            .iter()
            .any(|s| matches!(s, Step::S(Bg::In2, [campaign_e2::Cap::C(20, 6), ..])))
    );
}

#[test]
fn lost_flights_17_and_18() {
    let _data = require_data!();
    // §6 vector 10: lost, declined → ending C (no dossier).
    for m in [17, 18] {
        let r = play(m, vec![result(|r| r.lost = true)], |_| false);
        assert!(r.block.game_over && r.randoms == 1, "block {m}");
        assert!(!r.shown.contains(&Step::Dossier));
        assert!(
            r.shown
                .iter()
                .any(|s| matches!(s, Step::S(Bg::In2, [campaign_e2::Cap::C(20, 8), ..])))
        );
    }
    // OB-042: died and lost: two prompts; declining both gives ending C.
    let both = play(
        17,
        vec![result(|r| {
            r.lost = true;
            r.died = true;
        })],
        |_| false,
    );
    assert_eq!(both.randoms, 2);
    assert!(both.block.game_over && !both.shown.contains(&Step::Dossier));
    // Lost in another block means nothing (only blocks 17 and 18 test it).
    let other = play(12, vec![result(|r| r.lost = true)], |_| false);
    assert!(other.block.advance && other.randoms == 0);
}

#[test]
fn esc_at_the_first_card_goes_straight_to_the_flight() {
    // §8: Esc at the marauder card skips D1..D4; vulture keeps playing until the flight.
    let _data = require_data!();
    let (dir, res) = load().unwrap();
    let launcher = Scripted::default();
    let flights = launcher.flights.clone();
    let mut prog = Program::for_episode(&dir, &res, Box::new(launcher), Episode::E2).unwrap();
    let settings = Settings::default();
    let mut audio = Vec::new();
    let mut c = CampaignE2::new(1, 0);
    for frame in 0..200 {
        let mut ctx = FlowContext {
            data: &dir,
            resources: &res,
            settings: &settings,
            audio: &mut audio,
        };
        let pressed = if frame == 60 {
            vec![Key::ESCAPE]
        } else {
            Vec::new()
        };
        c.step(
            &mut prog,
            &mut ctx,
            &TickInput {
                pressed,
                ..TickInput::default()
            },
        );
        if !flights.borrow().is_empty() {
            break;
        }
    }
    assert_eq!(*flights.borrow(), [1]);
    // vulture, then the flight song (td01).
    assert_eq!(songs(&audio).len(), 2);
}

#[test]
fn block_20_end_sequence_plays_death_looping() {
    let _data = require_data!();
    let r = play(20, Vec::new(), |_| false);
    assert!(r.block.advance);
    assert!(
        !r.audio
            .iter()
            .any(|a| matches!(a, AudioCommand::SetMusicLoop(_))),
        "no loop-off call in block 20"
    );
    assert!(songs(&r.audio).iter().all(|&(_, looping)| looping));
    assert_eq!(r.shown.last(), Some(&Step::Tc(campaign_e2::Cap::C(20, 5))));
}

/// Runs the program for `frames` frames with `keys(frame)`.
fn run(ep: &mut Episode2, frames: u32, keys: impl Fn(u32) -> Vec<Key>) -> Vec<AudioCommand> {
    run_until(ep, frames, keys, |_| false)
}

/// As [`run`], stopping before a frame in which `stop` holds.
fn run_until(
    ep: &mut Episode2,
    frames: u32,
    keys: impl Fn(u32) -> Vec<Key>,
    stop: impl Fn(&Episode2) -> bool,
) -> Vec<AudioCommand> {
    let (dir, res) = load().unwrap();
    let settings = Settings::default();
    let mut audio = Vec::new();
    for f in 0..frames {
        if f > 0 && stop(ep) {
            break;
        }
        let mut ctx = FlowContext {
            data: &dir,
            resources: &res,
            settings: &settings,
            audio: &mut audio,
        };
        let input = TickInput {
            pressed: keys(f),
            ..TickInput::default()
        };
        if matches!(ep.tick(&mut ctx, &input), Transition::Quit) {
            break;
        }
    }
    audio
}

fn program(launcher: Scripted) -> Episode2 {
    let (dir, res) = load().unwrap();
    Episode2::new(&dir, &res, Box::new(launcher)).unwrap()
}

#[test]
fn start_up_to_the_menu_and_a_completed_block() {
    // §6 vectors 5 and 6.
    let _data = require_data!();
    let mut ep = program(Scripted::default());
    let two = Key::digit(2).unwrap();
    run(
        &mut ep,
        100,
        |f| if f == 50 { vec![two] } else { Vec::new() },
    );
    assert_eq!(ep.state(), (0, 0, 0));
    let c = ep.item_colours();
    assert_eq!(c[0], td_formats::palette::Rgb6 { r: 63, g: 0, b: 0 });
    assert!(
        c[1..]
            .iter()
            .all(|&e| e == td_formats::palette::Rgb6 { r: 16, g: 0, b: 0 })
    );
    // New game (Enter), then Space through block 1, until the menu is back.
    let keys = |f| match f {
        0 => vec![Key::ENTER],
        f if f % 20 == 0 => vec![Key::SPACE],
        _ => Vec::new(),
    };
    run_until(&mut ep, 400_000, keys, |ep| ep.state().0 == 2);
    let (mission, kills, cursor) = ep.state();
    assert_eq!((mission, kills, cursor), (2, 2, 3));
    assert_eq!(
        ep.item_colours()[3],
        td_formats::palette::Rgb6 { r: 63, g: 0, b: 0 },
        "OB-041: red, not yellow"
    );
}

#[test]
fn episode_complete_and_game_over_bookkeeping() {
    // §6 vector 8: block 20 completed → mission 0, cursor 7. A restored game at 20 is played
    // from item 3.
    let _data = require_data!();
    let mut ep = program(Scripted::default());
    run(&mut ep, 3, |_| Vec::new());
    ep.restored(20, 50);
    assert_eq!(ep.state(), (20, 50, 3));
    let space = |f| match f {
        0 => vec![Key::ENTER],
        f if f % 20 == 0 => vec![Key::SPACE],
        _ => Vec::new(),
    };
    run_until(&mut ep, 600_000, space, |ep| ep.state().0 != 20);
    assert_eq!(ep.state(), (0, 52, 7));
    // Item 3 path, game over: mission 0, cursor 0.
    let launcher = Scripted::default();
    launcher.results.borrow_mut().push(died());
    let mut ep = program(launcher);
    run(&mut ep, 3, |_| Vec::new());
    ep.restored(6, 0);
    let esc = |f| match f {
        0 => vec![Key::ENTER],
        f if f % 20 == 0 => vec![Key::ESCAPE],
        _ => Vec::new(),
    };
    run_until(&mut ep, 400_000, esc, |ep| ep.state().0 != 6);
    assert_eq!(ep.state(), (0, 0, 0));
}

#[test]
fn script_choice_matches_c0015() {
    let _data = require_data!();
    let oracle = require_oracle!();
    let mut ep = program(Scripted::default());
    run(&mut ep, 40, |_| Vec::new());
    let mut fb = Framebuffer::new();
    ep.draw(&mut fb);
    compare(
        &fb,
        &oracle,
        "C-0015",
        "f00000146_320x200_script-choice.vga",
    );
}

#[test]
fn main_menu_matches_c0015_and_c0016() {
    // OB-041: DAC 224 = (63, 0, 0), 225..233 = (16, 0, 0) including 227; DAC 255 is
    // tdmen.pal's.
    let _data = require_data!();
    let oracle = require_oracle!();
    let mut ep = program(Scripted::default());
    let two = Key::digit(2).unwrap();
    run(
        &mut ep,
        100,
        |f| if f == 10 { vec![two] } else { Vec::new() },
    );
    let mut fb = Framebuffer::new();
    ep.draw(&mut fb);
    for capture in ["C-0015", "C-0016"] {
        compare(&fb, &oracle, capture, "f00000375_320x200_main-menu.vga");
        let (dac, _) = oracle_frame(&oracle, capture, "f00000375_320x200_main-menu.vga");
        for e in 0..256 {
            assert_eq!(fb.palette.colors[e], dac.colors[e], "{capture}: entry {e}");
        }
    }
}

#[test]
fn menu_after_a_restore_matches_c0016() {
    // C-0016 f750: restored mission 9; the loader ran with the cursor on item 2 (226 bright),
    // then the cursor moved to item 3 without a ramp (227 dim red).
    let _data = require_data!();
    let oracle = require_oracle!();
    let mut ep = program(Scripted::default());
    run(&mut ep, 3, |_| Vec::new());
    ep.restored(9, 0);
    let mut fb = Framebuffer::new();
    ep.draw(&mut fb);
    let name = "f00000750_320x200_menu2.vga";
    compare(&fb, &oracle, "C-0016", name);
    let (dac, _) = oracle_frame(&oracle, "C-0016", name);
    for e in 0..256 {
        assert_eq!(fb.palette.colors[e], dac.colors[e], "entry {e}");
    }
}

fn caption_texts(res: &ExeResources, caps: [campaign_e2::Cap; 3]) -> [String; 3] {
    caps.map(|c| {
        let (id, k) = c.resource();
        res.texts(&id).unwrap()[k].clone()
    })
}

#[test]
fn marauder_card_matches_c0015() {
    // Block 1's first card: marauder with the preamble (C-0015 f542).
    let _data = require_data!();
    let oracle = require_oracle!();
    let (dir, res) = load().unwrap();
    let assets = StoryAssets::load(&dir).unwrap();
    use campaign_e2::Cap::P;
    let t = caption_texts(&res, [P(0), P(1), P(2)]);
    let s = scenes_e2::scene_card(&dir, &assets, Bg::Mar, [&t[0], &t[1], &t[2]]).unwrap();
    let s = faded_in(s);
    compare(
        s.screen(),
        &oracle,
        "C-0015",
        "f00000542_320x200_marauder.vga",
    );
}

#[test]
fn tdoffic2_card_matches_c0016() {
    // Block 9's first card after its title: to2[8.6/8.7/2.1] (C-0016 f1318).
    let _data = require_data!();
    let oracle = require_oracle!();
    let (dir, res) = load().unwrap();
    let assets = StoryAssets::load(&dir).unwrap();
    use campaign_e2::Cap::C;
    let t = caption_texts(&res, [C(8, 6), C(8, 7), C(2, 1)]);
    let s = scenes_e2::scene_card(&dir, &assets, Bg::To2, [&t[0], &t[1], &t[2]]).unwrap();
    let s = faded_in(s);
    compare(s.screen(), &oracle, "C-0016", "f00001318_320x200_to2.vga");
}
