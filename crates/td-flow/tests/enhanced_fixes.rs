//! The fix-enhanced original-bug fixes of the front end follow the D5 preset
//! (`docs/spec/original-bugs.md`; spec frontend/port-front-end rev 3 §3.4): preserved under the
//! Classic profile, fixed under Enhanced (its preset turns the fixes on), and preserved again
//! when the player turns the fixes off. OB-012 (music cues) needs TD2192_DATA for the
//! program's story assets and skips otherwise; OB-058's options need nothing.

use td_assets::{DataDir, Episode, ExeResources};
use td_audio::{AudioCommand, CueRestart};
use td_flow::intro::IntroOptions;
use td_flow::program::{Program, cue_restart};
use td_flow::settings::{Profile, Settings};
use td_flow::{FlowContext, PlaceholderLauncher};
use td_test_support::require_data;

fn classic() -> Settings {
    Settings::default()
}

fn enhanced() -> Settings {
    Settings {
        profile: Profile::Enhanced,
        ..Settings::default()
    }
}

fn enhanced_without_fixes() -> Settings {
    let mut s = enhanced();
    s.enhanced.fixes = Some(false);
    s
}

#[test]
fn the_fixes_follow_the_profile_and_the_choice() {
    assert!(!classic().enhanced_fixes());
    assert!(enhanced().enhanced_fixes(), "the preset turns them on");
    assert!(!enhanced_without_fixes().enhanced_fixes());
    // OB-012.
    assert_eq!(cue_restart(&classic()), CueRestart::Always);
    assert_eq!(cue_restart(&enhanced()), CueRestart::IfDifferent);
    assert_eq!(cue_restart(&enhanced_without_fixes()), CueRestart::Always);
    // OB-058 (intro flyover).
    assert!(!IntroOptions::for_settings(&classic()).fix_ob058);
    assert!(IntroOptions::for_settings(&enhanced()).fix_ob058);
    assert!(!IntroOptions::for_settings(&enhanced_without_fixes()).fix_ob058);
    // Classic forces the fixes off even when the choice says on.
    let mut c = classic();
    c.enhanced.fixes = Some(true);
    assert!(!c.enhanced_fixes());
    assert!(!IntroOptions::for_settings(&c).fix_ob058);
}

/// The `PlayMusic` commands of the menu cue requested twice in a row.
fn menu_cue_twice(episode: Episode, settings: &Settings) -> usize {
    let data = td_test_support::data_dir().expect("TD2192_DATA");
    let dir = DataDir::open(&data).unwrap();
    let res = ExeResources::load(&dir).unwrap();
    let mut prog = Program::for_episode(
        &dir,
        &res,
        Box::new(PlaceholderLauncher::default()),
        episode,
    )
    .unwrap();
    let mut audio: Vec<AudioCommand> = Vec::new();
    for _ in 0..2 {
        let mut ctx = FlowContext {
            data: &dir,
            resources: &res,
            settings,
            audio: &mut audio,
        };
        prog.play_cue(&mut ctx, td_audio::music_cues::MENU);
    }
    audio
        .iter()
        .filter(|c| matches!(c, AudioCommand::PlayMusic { .. }))
        .count()
}

/// OB-012 (music-cues §4.1): every cue restarts its song in Classic, even the song already
/// playing; the Enhanced fix keeps a looping song.
#[test]
fn ob012_a_repeated_cue_restarts_the_song_in_classic_only() {
    let _ = require_data!();
    for ep in [Episode::E1, Episode::E2, Episode::E3] {
        assert_eq!(menu_cue_twice(ep, &classic()), 2, "{ep} Classic restarts");
        assert_eq!(menu_cue_twice(ep, &enhanced()), 1, "{ep} Enhanced keeps it");
        assert_eq!(
            menu_cue_twice(ep, &enhanced_without_fixes()),
            2,
            "{ep} Enhanced without the fixes restarts"
        );
    }
}
