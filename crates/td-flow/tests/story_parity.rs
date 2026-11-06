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
use td_flow::main_menu::{MainMenu, MenuGraphics, Reentry};
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
    let mut t = story::text_card(&assets, text(0x478), &Palette::default());
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
fn every_e1_scene_card_puts_its_captions_where_text_md_says() {
    // text.md rev 2 §4.5.1: the captions (and their black copies) change only the pixels
    // around each card's own pens; the ink entry gets the card's colour.
    use td_flow::scene_card::Captions;
    let _data = require_data!();
    let (dir, _res) = load().unwrap();
    let assets = StoryAssets::load(&dir).unwrap();
    let caps = ["abc", "defgh", "ij"];
    for l in &story::E1_SCENE_CARDS {
        let bare = story::scene_card(&dir, &assets, l.file, ["", "", ""]).unwrap();
        let card = story::scene_card(&dir, &assets, l.file, caps).unwrap();
        // Pen and character count of each drawn string.
        let pens: Vec<((i32, i32), usize)> = match l.captions {
            Captions::Lines((x, y)) => (0..3)
                .map(|k| ((x, y + 10 * k), caps[k as usize].len()))
                .collect(),
            Captions::Joined(pen) => vec![(pen, td_flow::scene_card::joined(caps).len())],
            Captions::Centred(ys) => caps
                .iter()
                .zip(ys)
                .map(|(t, y)| ((td_flow::scene_card::centred_x(t), y), t.len()))
                .collect(),
            Captions::None => Vec::new(),
        };
        let inside = |x: i32, y: i32| {
            pens.iter().any(|&((px, py), n)| {
                (px - 1..=px + 2 + 6 * n as i32).contains(&x) && (py - 1..=py + 7).contains(&y)
            })
        };
        let (a, b) = (bare.palette_buffer(), card.palette_buffer());
        let mut changed = 0;
        for i in 0..64_000 {
            if bare.screen().pixels[i] != card.screen().pixels[i] {
                let (x, y) = ((i % 320) as i32, (i / 320) as i32);
                assert!(inside(x, y), "{}: ({x}, {y}) changed", l.file);
                changed += 1;
            }
        }
        assert!(changed > 0, "{}: no caption pixels", l.file);
        let e = usize::from(l.ink.0);
        assert_eq!(Some(b.colors[e]), l.ink.1, "{}: ink entry", l.file);
        assert_eq!(a.colors[e], b.colors[e]);
    }
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
    // screens §3.1, test vector 1: C-0002 f997, 162 frames after the menu's fade-in began
    // (f835), after the start-up pulse of cursor 0. The whole DAC must match, DAC 255 = white
    // included. The only pixels that differ are the 5 ink-0 MemAvail digits (not ported, T3).
    let _data = require_data!();
    let oracle = require_oracle!();
    let (dir, res) = load().unwrap();
    let mut ep =
        td_flow::Episode1::new(&dir, &res, Box::new(PlaceholderLauncher::default())).unwrap();
    let settings = Settings::default();
    let mut audio = Vec::new();
    let mut ctx = FlowContext {
        data: &dir,
        resources: &res,
        settings: &settings,
        audio: &mut audio,
    };
    // Script choice: `1` once it is shown, then wait until the menu has faded in.
    let one = Key::digit(1).unwrap();
    let mut frames_in_menu = 0;
    for f in 0..2_000 {
        let pressed = if f == 40 { vec![one] } else { Vec::new() };
        let input = TickInput {
            pressed,
            ..TickInput::default()
        };
        td_flow::Screen::tick(&mut ep, &mut ctx, &input);
        if ep.menu().screen().pixels.iter().any(|&p| p != 0) {
            frames_in_menu += 1;
        }
        if frames_in_menu == 162 {
            break;
        }
    }
    assert!(ep.menu().waiting(), "fade-in and pulse over");
    let mut fb = td_render::Framebuffer::new();
    td_flow::Screen::draw(&ep, &mut fb);
    let (dac, frame) = oracle_frame(&oracle, "f00000997_320x200_main-menu.vga");
    let d: Vec<(usize, usize)> = (0..64_000)
        .filter(|&i| fb.pixels[i] != frame[i])
        .map(|i| (i % 320, i / 320))
        .collect();
    assert_eq!(d, [(24, 11), (40, 11), (40, 13), (24, 15), (40, 15)]);
    for e in 0..256 {
        assert_eq!(fb.palette.colors[e], dac.colors[e], "entry {e}");
    }
}

/// Frames from the first to the last visible level of each title-card fade, as DAC entry 85
/// (the ink) shows them, and the frames in between: (fade-in, hold, fade-out).
fn title_card_fades(entry_85: &[u8]) -> (usize, usize, usize) {
    let a = entry_85.iter().position(|&r| r > 0).unwrap();
    let b = a + entry_85[a..].iter().position(|&r| r == 63).unwrap();
    let c = b + entry_85[b..].iter().position(|&r| r < 63).unwrap();
    let d = c + entry_85[c..].iter().position(|&r| r == 0).unwrap();
    (b - a + 1, c - b - 1, d - c + 1)
}

#[test]
fn the_first_title_card_fades_at_one_frame_per_level() {
    // BUG-6, screens rev 3 §2.1, text rev 3 §4.5.2: the script choice leaves 7 non-zero
    // components in g_Palette and the menu has its own buffer, so the first title card of a
    // new game fades entry 85 over a nearly black palette: one frame per level (C-0002: DAC
    // writes in f1063..1080 and f1409..1428). Visible levels 1..16: 16 frames each way.
    let _data = require_data!();
    let (dir, res) = load().unwrap();
    let mut ep =
        td_flow::Episode1::new(&dir, &res, Box::new(PlaceholderLauncher::default())).unwrap();
    let settings = Settings::default();
    let mut audio = Vec::new();
    let mut ctx = FlowContext {
        data: &dir,
        resources: &res,
        settings: &settings,
        audio: &mut audio,
    };
    let one = Key::digit(1).unwrap();
    let mut tick = |ep: &mut td_flow::Episode1, keys: Vec<Key>| {
        td_flow::Screen::tick(
            ep,
            &mut ctx,
            &TickInput {
                pressed: keys,
                ..TickInput::default()
            },
        );
    };
    for f in 0..400 {
        tick(&mut ep, if f == 40 { vec![one] } else { Vec::new() });
        if ep.menu().waiting() && ep.menu().screen().pixels.iter().any(|&p| p != 0) {
            break;
        }
    }
    assert!(ep.menu().waiting());
    // New game: the menu fades out, block 1 starts with T[0478].
    tick(&mut ep, vec![Key::ENTER]);
    let mut fb = td_render::Framebuffer::new();
    let mut ink = Vec::new();
    for _ in 0..600 {
        tick(&mut ep, Vec::new());
        td_flow::Screen::draw(&ep, &mut fb);
        ink.push(fb.palette.colors[85].r);
    }
    // From the end of the menu's fade-out on.
    let black = ink.iter().position(|&r| r == 0).unwrap();
    let (fade_in, hold, fade_out) = title_card_fades(&ink[black..]);
    eprintln!("first title card: fade-in {fade_in}, hold {hold}, fade-out {fade_out} frames");
    assert_eq!((fade_in, fade_out), (16, 16));
    // 5 s of DOS time (332 frames, BUG-7) plus the invisible level 16 of the fade-out.
    assert_eq!(hold, 332 + 1);
}

#[test]
fn a_title_card_after_the_story_so_far_fades_at_two_frames_per_level() {
    // BUG-6 (C-0170 f1003..1036, f1364..1397: 34 frames): the story so far leaves city.td's
    // palette (677 non-zero components) in g_Palette, so the next title card fades at two
    // frames per level: levels 1..16 visible over 31 frames each way.
    let _data = require_data!();
    let (dir, res) = load().unwrap();
    let mut prog = Program::new(&dir, &res, Box::new(PlaceholderLauncher::default())).unwrap();
    let first = prog.story(Step::T(0x478)).unwrap();
    assert_eq!(first.fade_frames_per_level(), 1, "g_Palette black at start");
    let lines = res.texts("e1.background_story").unwrap().to_vec();
    let mut s = story::story_so_far(&dir, &prog.assets, &lines).unwrap();
    let mut n = 0;
    while !s.finished() {
        let keys = if n % 50 == 49 {
            vec![Key::ESCAPE]
        } else {
            Vec::new()
        };
        s.step(&TickInput {
            pressed: keys,
            ..TickInput::default()
        });
        n += 1;
        assert!(n < 1_000);
    }
    prog.leave_palette(s.palette_buffer());
    assert!(td_render::fade::nonzero_components(&prog.scene_palette, 1) >= 380);
    let mut t = prog.story(Step::T(0x478)).unwrap();
    assert_eq!(t.fade_frames_per_level(), 2);
    let mut ink = Vec::new();
    while !t.finished() {
        t.step(&TickInput::default());
        ink.push(t.screen().palette.colors[85].r);
    }
    let (fade_in, hold, fade_out) = title_card_fades(&ink);
    assert_eq!((fade_in, fade_out), (31, 31));
    // 332 frames, then the fade-out's first wait and its invisible level 16 (3 frames).
    assert_eq!(hold, 332 + 3);
}

#[test]
fn main_menu_boxes_and_fades() {
    // screens §3.1: the quit box over rows 84..115 at half brightness, the menu rows restored
    // on a cancel; a selection fades the menu out before its action (C-0002: Enter at f1027,
    // the next screen's mode X initialisation at f1060).
    let _data = require_data!();
    let (dir, _) = load().unwrap();
    let gfx = MenuGraphics::load(&dir).unwrap();
    let mut m = MainMenu::new(Some(gfx));
    m.enter(0, false, Reentry::Loader { first: true });
    let idle = TickInput::default();
    let press = |k| TickInput {
        pressed: vec![k],
        ..TickInput::default()
    };
    while !m.waiting() {
        m.step(&idle);
    }
    let menu_rows = m.screen().pixels[84 * 320..116 * 320].to_vec();
    let full = m.screen().palette.clone();
    m.step(&press(Key::ESCAPE));
    for _ in 0..16 {
        m.step(&idle);
    }
    assert_ne!(
        m.screen().pixels[84 * 320..116 * 320],
        menu_rows[..],
        "box 1 shown"
    );
    for e in 0..256 {
        let b = m.palette_buffer().colors[e];
        let expect = if (192..=209).contains(&e) {
            b
        } else {
            td_formats::palette::Rgb6 {
                r: b.r / 2,
                g: b.g / 2,
                b: b.b / 2,
            }
        };
        assert_eq!(m.screen().palette.colors[e], expect, "entry {e}");
    }
    m.step(&press(Key::SPACE));
    assert_eq!(m.screen().pixels[84 * 320..116 * 320], menu_rows[..]);
    while !m.waiting() {
        m.step(&idle);
    }
    assert_eq!(m.screen().palette.colors[1..224], full.colors[1..224]);
    assert_eq!(m.cursor(), 0);
    m.step(&press(Key::ENTER));
    let mut n = 0;
    while m.step(&idle).is_none() {
        n += 1;
        assert!(n < 100);
    }
    assert_eq!(n + 1, 34, "frames from Enter to the action");
    assert!(
        m.screen().palette.colors[1..]
            .iter()
            .all(|c| c.r == 0 && c.g == 0 && c.b == 0)
    );
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

#[test]
fn keys_typed_ahead_carry_into_the_next_screen() {
    // screens §2.2 (U-123): Space ends the title card's wait; Esc, typed while the card fades
    // out, stays in the BIOS buffer, ends the hangar card's wait as soon as it starts and sets
    // the skip flag, so flight 1 starts without dialogue 1 or the hint.
    let _data = require_data!();
    let (dir, res) = load().unwrap();
    let (l, _, launches) = counting(0);
    let mut prog = Program::new(&dir, &res, Box::new(l)).unwrap();
    let mut c = Campaign::new(1, 0);
    let keys = |_: &Campaign, f: u32| match f {
        40 => Some(Key::SPACE),
        41 => Some(Key::ESCAPE),
        _ => None,
    };
    let launched = || launches.get() > 0;
    let frame = drive(&mut prog, &mut c, keys, launched, &mut Vec::new());
    // Title fade-out, then the hangar card's fade-in and fade-out: about 40 + 3 × 34 frames.
    assert!(
        launched() && frame < 200,
        "flight launched at frame {frame}"
    );
}

/// A flight that runs at the flight rate and never ends.
#[derive(Debug)]
struct Endless;

impl td_flow::Screen for Endless {
    fn tick_hz(&self) -> f64 {
        19.2
    }
    fn tick(&mut self, _ctx: &mut FlowContext<'_>, _input: &TickInput) -> td_flow::Transition {
        td_flow::Transition::None
    }
    fn draw(&self, fb: &mut td_render::Framebuffer) {
        fb.clear(0);
    }
}

impl FlightScreen for Endless {
    fn result(&self) -> td_flow::FlightResult {
        td_flow::FlightResult::default()
    }
}

#[derive(Debug)]
struct EndlessLauncher;

impl FlightLauncher for EndlessLauncher {
    fn launch(&mut self, _ctx: &mut FlowContext<'_>, _flight: u8) -> Box<dyn FlightScreen> {
        Box::new(Endless)
    }
    fn random(&mut self, _n: u16) -> u16 {
        0
    }
}

#[test]
fn a_campaign_flight_runs_at_the_flight_rate() {
    // BUG-5 (docs/parity/M12-timing.md row 2): the block reports its flight child's tick rate,
    // so the run loop paces the flight at 19.2 Hz, not at the 70 Hz retrace rate.
    let _data = require_data!();
    let (dir, res) = load().unwrap();
    let mut prog = Program::new(&dir, &res, Box::new(EndlessLauncher)).unwrap();
    let mut c = Campaign::new(1, 0);
    assert_eq!(c.tick_hz(), td_flow::RETRACE_HZ);
    let rate = Rc::new(Cell::new(0.0));
    let seen = rate.clone();
    let key = move |c: &Campaign, f: u32| {
        seen.set(c.tick_hz());
        f.is_multiple_of(20).then_some(Key::ESCAPE)
    };
    let r = rate.clone();
    drive(
        &mut prog,
        &mut c,
        key,
        move || r.get() == 19.2,
        &mut Vec::new(),
    );
    assert_eq!(c.tick_hz(), 19.2);
}
