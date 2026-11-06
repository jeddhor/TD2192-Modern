//! Intro logic on synthetic assets (no game data): the frame timeline of intro §4.3 and §6
//! vector 8, keys, music and effects, page flips. The parity with C-0047 is in
//! `tests/intro_parity.rs`.

use super::*;

fn song(end_tick: u32) -> Arc<Cmf> {
    Arc::new(Cmf {
        version: 0x0101,
        ticks_per_quarter: 48,
        ticks_per_second: 96,
        channel_in_use: [false; 16],
        basic_tempo: 0,
        instruments: Vec::new(),
        events: Vec::new(),
        end_tick,
    })
}

fn scr(fill: u8) -> ScrImage {
    let mut palette = Palette::default();
    for (i, c) in palette.colors.iter_mut().enumerate() {
        *c = Rgb6 {
            r: (i % 64) as u8,
            g: 63,
            b: 32,
        };
    }
    ScrImage {
        palette,
        pixels: vec![fill; 64_000].into_boxed_slice(),
    }
}

fn fly_assets() -> FlyoverAssets {
    let mut font = vec![0u8; FontKind::Small.file_len()];
    for g in font.chunks_exact_mut(25) {
        g[0] = 1;
    }
    let mut blk = vec![0u8; 80 * 1024];
    for (t, tile) in blk.chunks_exact_mut(1024).enumerate() {
        tile.fill(t as u8);
    }
    let mut sprites = vec![0u8; 20 * 1024];
    sprites[4 * 1024..5 * 1024].fill(200);
    let mut td_pal = Palette::default();
    td_pal.colors[200] = Rgb6 { r: 1, g: 2, b: 3 };
    let pcm = |n: usize| Some(Arc::new(Pcm8 {
        rate: 11_025,
        samples: vec![128; n],
    }));
    FlyoverAssets {
        font: Font::parse(&font, FontKind::Small).unwrap(),
        sprites,
        // Column i (1-based) holds tile 10 + i everywhere.
        map: (0..20 * 2000).map(|k| 11 + (k / 2000) as u8).collect(),
        blk,
        dun: vec![71; 3 * 1024],
        td_pal,
        credits: (0..25).map(|k| format!("credit {k}")).collect(),
        intro_song: Some(song(8000)),
        intro2_song: Some(song(4000)),
        // whoosh, pulse, ric3, scratch, home: about 1, 0.5, 2, 1, 1 s.
        effects: [pcm(11_025), pcm(5_512), pcm(22_050), pcm(11_025), pcm(11_025)],
    }
}

fn assets() -> IntroAssets {
    IntroAssets {
        fly: fly_assets(),
        intro0: scr(1),
        intro1: scr(2),
        intro2: scr(3),
        narration: [
            (0..5).map(|k| format!("line {k}")).collect(),
            vec!["one".into(), "two".into()],
            vec!["three".into(), "four".into()],
        ],
        intro1_song: Some(song(500)),
    }
}

/// Runs until the end (or `limit` frames), pressing `keys` at the given frames.
fn run(
    intro: &mut Intro,
    keys: &[(u64, Key)],
    limit: u64,
) -> Vec<(u64, AudioCommand)> {
    let mut out = Vec::new();
    while !intro.finished() && intro.frame() < limit {
        let next = intro.frame() + 1;
        let pressed = keys
            .iter()
            .filter(|(f, _)| *f == next)
            .map(|&(_, k)| k)
            .collect();
        let mut audio = Vec::new();
        intro.step(
            &TickInput {
                pressed,
                ..TickInput::default()
            },
            &mut audio,
        );
        out.extend(audio.into_iter().map(|c| (next, c)));
    }
    out
}

fn fades(intro: &Intro) -> Vec<(u64, Mark)> {
    intro
        .marks()
        .iter()
        .copied()
        .filter(|(_, m)| !matches!(m, Mark::Step(_)))
        .collect()
}

#[test]
fn timeline_follows_the_p1_frame_counts() {
    let mut intro = Intro::new(assets(), IntroOptions::default());
    run(&mut intro, &[], 20_000);
    assert!(intro.finished());
    let f = fades(&intro);
    let at = |i: usize| f[i].0;
    use Mark::{ColourFade as C, FullFade as F};
    let kinds: Vec<Mark> = f.iter().map(|&(_, m)| m).collect();
    assert_eq!(
        kinds[..10],
        [
            C { entry: 52, up: true },
            C { entry: 52, up: false },
            F { up: true },
            C { entry: 52, up: true },
            C { entry: 52, up: false },
            C { entry: 92, up: true },
            C { entry: 92, up: false },
            C { entry: 195, up: true },
            C { entry: 195, up: false },
            F { up: false },
        ]
    );
    // C-0047: date fade in f136, out f274 (6 + 132 frames later), full fade f281, card 1
    // f346, its fade-out 6 + 366 frames after it, card 2 7 frames after that.
    assert_eq!(at(1) - at(0), 6 + HOLD_DATE as u64);
    assert_eq!(at(2) - at(1), 7);
    assert_eq!(at(3) - at(2), 65);
    assert_eq!(at(4) - at(3), 6 + HOLD_11 as u64);
    assert_eq!(at(5) - at(4), 7);
    assert_eq!(at(9) - at(8), 7);
    // The flyover: full fade in, 1618 steps, full fade out, Crt.Delay, intro1, intro2, exit.
    let steps: Vec<u64> = intro
        .marks()
        .iter()
        .filter_map(|&(fr, m)| matches!(m, Mark::Step(_)).then_some(fr))
        .collect();
    assert_eq!(steps.len(), 1618);
    assert_eq!(kinds[10], F { up: true });
    // Step 1 begins in the frame after the fade-in's last level (C-0047: f1552 -> f1617).
    assert_eq!(steps[0] - at(10), 65);
    assert!(steps.windows(2).all(|w| (3..=4).contains(&(w[1] - w[0]))));
    assert_eq!(kinds[11], F { up: false });
    let tail = &kinds[12..];
    assert_eq!(
        tail,
        [
            F { up: true },
            C { entry: 45, up: true },
            C { entry: 45, up: false },
            C { entry: 45, up: true },
            C { entry: 45, up: false },
            F { up: false },
            F { up: true },
            C { entry: 140, up: false },
            C { entry: 140, up: true },
            C { entry: 141, up: false },
            C { entry: 141, up: true },
            F { up: false },
            Mark::End,
        ]
    );
    assert_eq!(intro.flyover().s, flyover::END_STEP);
}

#[test]
fn spec_vector_8_esc_esc() {
    // C-0049/C-0050: Esc at file0000 +300 (f308, in the intro0 fade-in) and +400 (f408). The
    // first Esc ends card 1's hold at once; the exit fade follows its fade-out (f360..424);
    // the second Esc is swallowed by the exit's key read; text mode at f426.
    let mut intro = Intro::new(assets(), IntroOptions::default());
    let mut probe = Intro::new(assets(), IntroOptions::default());
    run(&mut probe, &[], 140);
    let date = probe.marks()[0].0;
    // Our frame for capture frame f: f − 136 + date.
    let ours = |f: u64| f - 136 + date;
    let audio = run(
        &mut intro,
        &[(ours(308), Key::ESCAPE), (ours(408), Key::ESCAPE)],
        5_000,
    );
    let f = fades(&intro);
    assert_eq!(f[3], (ours(346), Mark::ColourFade { entry: 52, up: true }));
    assert_eq!(f[4], (ours(353), Mark::ColourFade { entry: 52, up: false }));
    assert_eq!(f[5], (ours(360), Mark::FullFade { up: false }));
    assert_eq!(f[6], (ours(425), Mark::End));
    assert!(intro.keys.is_empty());
    assert!(matches!(audio.last(), Some((_, AudioCommand::StopMusic))));
    assert!(intro.marks().iter().all(|(_, m)| !matches!(m, Mark::Step(_))));
}

#[test]
fn other_keys_skip_cards_and_the_flyover() {
    let mut intro = Intro::new(assets(), IntroOptions::default());
    let space = Key::SPACE;
    // One key per card hold: each ends the hold, the key check consumes it.
    let mut probe = Intro::new(assets(), IntroOptions::default());
    run(&mut probe, &[], 140);
    let d = probe.marks()[0].0;
    run(&mut intro, &[(d + 20, space)], 1_000);
    let f = fades(&intro);
    // The date hold ended 20 − 6 frames into it: fade-out right away.
    assert_eq!(f[1], (d + 20, Mark::ColourFade { entry: 52, up: false }));
    // The next hold runs its full length (the key was consumed).
    let card1 = f[3].0;
    run(&mut intro, &[], card1 + 400);
    let f = fades(&intro);
    assert_eq!(f[4].0 - f[3].0, 6 + HOLD_11 as u64);
    // A key during the flyover ends the loop after the current step and goes on to intro1.
    run(&mut intro, &[], 2_400);
    let s = intro.flyover().s;
    assert!(s > 100, "flyover running: {s}");
    let at = intro.frame() + 1;
    run(&mut intro, &[(at, space)], at + 300);
    let f = fades(&intro);
    assert!(intro.flyover().s <= s + 2, "loop left at once");
    assert!(
        f.iter().any(|&(_, m)| m == Mark::ColourFade { entry: 45, up: true }),
        "intro1 follows"
    );
}

#[test]
fn esc_in_the_flyover_exits() {
    let mut intro = Intro::new(assets(), IntroOptions::default());
    run(&mut intro, &[], 2_400);
    let at = intro.frame() + 1;
    run(&mut intro, &[(at, Key::ESCAPE)], 20_000);
    let kinds: Vec<Mark> = fades(&intro).iter().map(|&(_, m)| m).collect();
    assert_eq!(
        kinds[kinds.len() - 3..],
        [Mark::FullFade { up: true }, Mark::FullFade { up: false }, Mark::End]
    );
}

#[test]
fn music_cues_and_the_intro2_restart() {
    let mut intro = Intro::new(assets(), IntroOptions::default());
    let audio = run(&mut intro, &[], 20_000);
    let songs: Vec<(u64, u32, bool)> = audio
        .iter()
        .filter_map(|(f, c)| match c {
            AudioCommand::PlayMusic { song, looping } => Some((*f, song.end_tick, *looping)),
            _ => None,
        })
        .collect();
    // intro1 (looping) at the date card, intro once, then intro2 when intro has ended.
    assert_eq!(songs[0].1, 500);
    assert!(songs[0].2);
    assert_eq!((songs[1].1, songs[1].2), (8000, false));
    assert_eq!((songs[2].1, songs[2].2), (4000, false));
    assert_eq!(songs.len(), 3, "intro2 is not restarted after the flyover");
    // intro (8001 song ticks from the flyover setup) ends during the flyover: intro2 starts at
    // the first step after it (about 8001 / 5 steps).
    let step_of = |frame: u64| {
        intro
            .marks()
            .iter()
            .filter(|&&(f, m)| f <= frame && matches!(m, Mark::Step(_)))
            .count()
    };
    let s = step_of(songs[2].0);
    assert!((1580..=1600).contains(&s), "intro2 at step {s}");
    let loop_off = audio
        .iter()
        .position(|(_, c)| matches!(c, AudioCommand::SetMusicLoop(false)))
        .unwrap();
    assert!(matches!(audio[loop_off + 1].1, AudioCommand::PlayMusic { .. }));
    assert!(matches!(audio.last(), Some((_, AudioCommand::StopMusic))));
}

#[test]
fn effects_follow_the_schedule() {
    let mut intro = Intro::new(assets(), IntroOptions::default());
    let audio = run(&mut intro, &[], 20_000);
    let lens: Vec<(usize, usize)> = audio
        .iter()
        .filter_map(|(f, c)| match c {
            AudioCommand::PlaySfx { sound } => {
                let s = intro
                    .marks()
                    .iter()
                    .filter(|&&(fr, m)| fr <= *f && matches!(m, Mark::Step(_)))
                    .count();
                Some((s, sound.samples.len()))
            }
            _ => None,
        })
        .collect();
    // whoosh at step 1, then pulse when it has ended (1 s = 20 steps).
    assert_eq!(lens[0], (1, 11_025));
    assert_eq!(lens[1].1, 5_512);
    assert!((20..=21).contains(&lens[1].0), "{:?}", lens[1]);
    for (step, len) in [(150, 11_025), (155, 22_050), (240, 11_025), (245, 11_025)] {
        assert!(lens.contains(&(step, len)), "step {step}");
    }
    // Pulse keeps filling the silences to the end of the flyover.
    assert!(lens.iter().any(|&(s, l)| s > 1500 && l == 5_512));
}

#[test]
fn pages_flip_and_the_screen_shows_the_previous_page() {
    let mut intro = Intro::new(assets(), IntroOptions::default());
    // To step 60: the scroll has started, the hoverskid is on screen.
    let mut before = intro.flyover().clone();
    while intro.flyover().s < 60 {
        before = intro.flyover().clone();
        let next = intro.frame() + 1;
        run(&mut intro, &[], next);
    }
    assert_eq!(before.s, 59);
    let st = intro.flyover().clone();
    assert_eq!(st.page, (60 + 1) % 2);
    let mut fb = Framebuffer::new();
    intro.draw(&mut fb);
    // The screen shows the other page (drawn at step 59): column x shows map column
    // x div 32 + 5 (1-based), tile 15 + x div 32 in the synthetic data.
    let shown = 1 - st.page;
    // Step 59's set start used its v198, before its motion.
    assert_eq!(before.page, shown);
    assert_eq!(
        intro.vga().start(),
        draw::page_start(shown, before.v198),
        "start of page {shown}"
    );
    for x in [0, 33, 100, 319] {
        let c = fb.get(x, 0).unwrap();
        assert_eq!(c, 15 + (x / 32) as u8, "x {x}");
    }
    // The hoverskid of step 59 at screen (140.., y − 16), y as after step 58's motion.
    assert!((0..200).any(|y| fb.get(150, y) == Some(200)));
    assert_eq!(fb.palette.colors[46], Rgb6 { r: 0, g: 63, b: 63 });
}

#[test]
fn flyover_only_ends_with_a_fade_and_stops_the_music() {
    let mut s = Intro::flyover_only(fly_assets(), IntroOptions::default());
    let audio = run(&mut s, &[], 3_000);
    assert!(!s.finished());
    let at = s.frame() + 1;
    let more = run(&mut s, &[(at, Key::ESCAPE)], 3_200);
    assert!(s.finished(), "Esc ends mode A too");
    let kinds: Vec<Mark> = fades(&s).iter().map(|&(_, m)| m).collect();
    assert_eq!(
        kinds,
        [Mark::FullFade { up: true }, Mark::FullFade { up: false }, Mark::End]
    );
    assert!(matches!(audio[0].1, AudioCommand::StopMusic));
    assert!(matches!(more.last(), Some((_, AudioCommand::StopMusic))));
}

#[test]
fn intro_policy() {
    let s = |play, seen| IntroSettings { play, seen };
    // Classic always plays it.
    assert!(s(IntroPolicy::Never, true).plays(false));
    assert!(s(IntroPolicy::Always, true).plays(true));
    assert!(s(IntroPolicy::FirstLaunch, false).plays(true));
    assert!(!s(IntroPolicy::FirstLaunch, true).plays(true));
    assert!(!s(IntroPolicy::Never, false).plays(true));
    let parsed: crate::Settings = toml::from_str("[intro]\nplay = \"first-launch\"\n").unwrap();
    assert_eq!(parsed.intro.play, IntroPolicy::FirstLaunch);
    assert_eq!(crate::Settings::default().intro, IntroSettings::default());
}
