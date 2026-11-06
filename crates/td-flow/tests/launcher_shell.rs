//! The port's launcher shell (`docs/spec/flow/launcher.md` rev 1 §4.10) with the original data:
//! the intro hands over to Episode 1, Esc skips it, and Episode 1's menu items 6 and 7 show
//! `aboutus.scr` / `order.scr` with `intro.cmf` and come back to the same item. Data-gated
//! (TD2192_DATA). No game data is stored.

use td_assets::{DataDir, ExeResources};
use td_audio::AudioCommand;
use td_flow::credits::{Credits, CreditsMode};
use td_flow::intro::{Intro, IntroOptions, Mark};
use td_flow::settings::Settings;
use td_flow::{Episode1, Flow, FlowContext, PlaceholderLauncher, Screen};
use td_formats::cmf::Cmf;
use td_formats::screen::Screen as ScrImage;
use td_input::{Key, TickInput};
use td_render::Framebuffer;
use td_test_support::require_data;

fn load() -> (DataDir, ExeResources) {
    let data = td_test_support::data_dir().unwrap();
    let dir = DataDir::open(&data).unwrap();
    let res = ExeResources::load(&dir).unwrap();
    (dir, res)
}

/// Ticks `screen` for `frames` frames, pressing `key` on the first; returns the audio.
fn run(
    screen: &mut dyn Screen,
    dir: &DataDir,
    res: &ExeResources,
    key: Option<Key>,
    frames: u32,
) -> Vec<AudioCommand> {
    let settings = Settings::default();
    let mut audio = Vec::new();
    for f in 0..frames {
        let mut ctx = FlowContext {
            data: dir,
            resources: res,
            settings: &settings,
            audio: &mut audio,
        };
        let input = TickInput {
            pressed: if f == 0 {
                key.into_iter().collect()
            } else {
                Vec::new()
            },
            ..TickInput::default()
        };
        screen.tick(&mut ctx, &input);
    }
    audio
}

fn song(dir: &DataDir, name: &str) -> Cmf {
    Cmf::parse(&dir.read(name).unwrap()).unwrap()
}

fn plays(audio: &[AudioCommand], want: &Cmf) -> bool {
    audio
        .iter()
        .any(|c| matches!(c, AudioCommand::PlayMusic { song, .. } if **song == *want))
}

#[test]
fn e1_items_6_and_7_show_the_pictures_with_intro_cmf() {
    let _data = require_data!();
    let (dir, res) = load();
    let intro_cmf = song(&dir, "intro.cmf");
    let menu_cmf = song(&dir, "menu.cmf");
    let mut ep = Episode1::new(&dir, &res, Box::new(PlaceholderLauncher::default())).unwrap();
    run(&mut ep, &dir, &res, None, 40);
    run(&mut ep, &dir, &res, Key::digit(1), 120);
    assert!(ep.menu().waiting());
    // 0 → 2 → 4 → 5 → 6 (items 1 and 3 are skipped with no game).
    for _ in 0..4 {
        run(&mut ep, &dir, &res, Some(Key::DOWN), 20);
    }
    assert_eq!(ep.menu().cursor(), 6);
    for (item, file) in [(6u8, "aboutus.scr"), (7, "order.scr")] {
        assert_eq!(ep.menu().cursor(), item);
        let audio = run(&mut ep, &dir, &res, Some(Key::ENTER), 200);
        assert!(plays(&audio, &intro_cmf), "item {item}: intro.cmf");
        // The picture, plotted one pixel to the right (OB-003, as E2's about/order).
        let scr = ScrImage::parse(&dir.read(file).unwrap()).unwrap();
        let mut fb = Framebuffer::new();
        ep.draw(&mut fb);
        for y in [0usize, 50, 199] {
            for x in [0usize, 100, 318] {
                assert_eq!(
                    fb.pixels[y * 320 + x + 1],
                    scr.pixels[y * 320 + x],
                    "item {item}: ({x}, {y})"
                );
            }
        }
        assert_eq!(fb.palette, scr.palette, "item {item}: faded in");
        // A key ends it; the menu comes back on the same item with the menu song.
        let audio = run(&mut ep, &dir, &res, Some(Key::SPACE), 200);
        assert!(plays(&audio, &menu_cmf), "item {item}: menu song again");
        assert!(ep.menu().waiting());
        assert_eq!(ep.menu().cursor(), item);
        assert_eq!(ep.game(), (0, 0), "game state unchanged");
        run(&mut ep, &dir, &res, Some(Key::DOWN), 20);
    }
}

#[test]
fn the_intro_hands_over_to_episode_1_and_esc_skips_it() {
    let _data = require_data!();
    let (dir, res) = load();
    let ep = Episode1::new(&dir, &res, Box::new(PlaceholderLauncher::default())).unwrap();
    let intro = Intro::load(&dir, &res, IntroOptions::default()).unwrap();
    let mut flow = Flow::new(Box::new(intro.then(Box::new(ep))));
    let settings = Settings::default();
    let mut audio = Vec::new();
    let mut ctx = FlowContext {
        data: &dir,
        resources: &res,
        settings: &settings,
        audio: &mut audio,
    };
    let mut fb = Framebuffer::new();
    // Esc during the date card: its hold ends, the fade-out, the exit fade (about 80 frames).
    for f in 0..160 {
        let pressed = if f == 30 {
            vec![Key::ESCAPE]
        } else {
            Vec::new()
        };
        flow.tick(
            &mut ctx,
            &TickInput {
                pressed,
                ..TickInput::default()
            },
        );
    }
    assert!(flow.running());
    // Episode 1's first screen (the script choice) is up and fading in.
    for _ in 0..60 {
        flow.tick(&mut ctx, &TickInput::default());
    }
    flow.draw(&mut fb);
    assert!(
        fb.palette.colors.iter().any(|c| c.r + c.g + c.b > 0),
        "script choice shown"
    );
    assert!(
        ctx.audio
            .iter()
            .any(|c| matches!(c, AudioCommand::RestartMusicDriver)),
        "Episode 1 started"
    );
}

#[test]
fn the_intro_timeline_with_the_original_files() {
    let _data = require_data!();
    let (dir, res) = load();
    let mut intro = Intro::load(&dir, &res, IntroOptions::default()).unwrap();
    let mut audio = Vec::new();
    while !intro.finished() {
        intro.step(&TickInput::default(), &mut audio);
    }
    // intro1 (looping), intro once, intro2 at the first step after intro has ended, stop.
    let songs: Vec<&Cmf> = audio
        .iter()
        .filter_map(|c| match c {
            AudioCommand::PlayMusic { song, .. } => Some(&**song),
            _ => None,
        })
        .collect();
    assert_eq!(songs.len(), 3);
    assert_eq!(*songs[0], song(&dir, "intro1.cmf"));
    assert_eq!(*songs[1], song(&dir, "intro.cmf"));
    assert_eq!(*songs[2], song(&dir, "intro2.cmf"));
    assert!(matches!(audio.last(), Some(AudioCommand::StopMusic)));
    // The five effects all play during the flyover.
    let effects: std::collections::BTreeSet<usize> = audio
        .iter()
        .filter_map(|c| match c {
            AudioCommand::PlaySfx { sound, .. } => Some(sound.samples.len()),
            _ => None,
        })
        .collect();
    assert_eq!(effects.len(), 5, "five distinct effects");
    let steps = intro
        .marks()
        .iter()
        .filter(|(_, m)| matches!(m, Mark::Step(_)))
        .count();
    assert_eq!(steps, 1618);
}

#[test]
fn credits_mode_a_and_b() {
    let _data = require_data!();
    let (dir, res) = load();
    let mut a = Credits::load(&dir, &res, CreditsMode::Flyover, IntroOptions::default()).unwrap();
    assert_eq!(a.mode(), CreditsMode::Flyover);
    // A key after a while ends mode A: fade out and pop.
    run(&mut a, &dir, &res, None, 1000);
    assert!(!a.finished());
    let audio = run(&mut a, &dir, &res, Some(Key::SPACE), 100);
    assert!(a.finished());
    assert!(matches!(audio.last(), Some(AudioCommand::StopMusic)));
    let mut b = Credits::load(&dir, &res, CreditsMode::Pages, IntroOptions::default()).unwrap();
    let audio = run(&mut b, &dir, &res, None, 100);
    assert!(plays(&audio, &song(&dir, "intro2.cmf")));
    let mut fb = Framebuffer::new();
    b.draw(&mut fb);
    // The title page: some caption ink between rows 84 and 110.
    assert!((84 * 320..110 * 320).any(|i| fb.pixels[i] == td_flow::credits::PAGE_INK));
    run(&mut b, &dir, &res, Some(Key::ESCAPE), 1);
    assert!(b.finished());
}
