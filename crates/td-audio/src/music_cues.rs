//! Which song plays when, Episode 1: the cue table of `Snd_PlayMusic(N)` and a helper that turns
//! a cue into [`AudioCommand`]s.
//!
//! Spec: `docs/spec/audio/music-cues.md` rev 1 (§3.1 table, §4.1 `Snd_PlayMusic`, §4.2 flight and
//! death cues). Fidelity: T1 for which song plays when. The campaign sequence (§4.3) is the flow's
//! business; this module only knows the cue numbers.

use std::sync::Arc;

use td_formats::cmf::Cmf;

use crate::mixer::AudioCommand;

/// Song file of each cue number N = 1..=14 (music-cues §3.1, `file0001:0E8B:0825..0963`).
pub const SONGS: [&str; 14] = [
    "td01.cmf",
    "td02.cmf",
    "hangar.cmf",
    "satair.cmf",
    "vulture.cmf",
    "mood.cmf",
    "menu.cmf",
    "low.cmf",
    "climax.cmf",
    "mood2.cmf",
    "moodmaj.cmf",
    "death.cmf",
    "mood3.cmf",
    "td03.cmf",
];

pub const TD01: u8 = 1;
pub const TD02: u8 = 2;
pub const HANGAR: u8 = 3;
pub const SATAIR: u8 = 4;
pub const VULTURE: u8 = 5;
pub const MOOD: u8 = 6;
pub const MENU: u8 = 7;
pub const LOW: u8 = 8;
pub const CLIMAX: u8 = 9;
/// Never requested in Episode 1.
pub const MOOD2: u8 = 10;
pub const MOODMAJ: u8 = 11;
/// The only song played with looping off.
pub const DEATH: u8 = 12;
pub const MOOD3: u8 = 13;
pub const TD03: u8 = 14;

/// The intro program's songs (`file0000.exe`), in play order, each with looping off
/// (music-cues §4.2; capture C-0001).
pub const INTRO_SONGS: [&str; 3] = ["intro1.cmf", "intro.cmf", "intro2.cmf"];

/// The song file of cue `n`, if `n` is in the table.
pub fn song_file(n: u8) -> Option<&'static str> {
    SONGS.get(usize::from(n).checked_sub(1)?).copied()
}

/// The flight song of mission `mission` (music-cues §4.2, `0E8B:C8EA..C92B`): mission number
/// mod 3 = 1 → `td01`, 2 → `td03`, 0 → `td02`.
pub fn flight_cue(mission: u8) -> u8 {
    match mission % 3 {
        1 => TD01,
        2 => TD03,
        _ => TD02,
    }
}

/// Whether cue `n` is played with the loop flag set (all but `death`).
pub fn cue_loops(n: u8) -> bool {
    n != DEATH
}

/// When a cue for the song that is already playing restarts it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CueRestart {
    /// Every cue starts its song from the top, as the original does (Classic; OB-012 preserved).
    Always,
    /// A looping song keeps playing when it is requested again (Enhanced; OB-012 fixed).
    IfDifferent,
}

/// Turns music cues into audio commands, remembering the current song.
#[derive(Debug, Clone)]
pub struct MusicCues {
    restart: CueRestart,
    current: Option<u8>,
}

impl MusicCues {
    pub fn new(restart: CueRestart) -> Self {
        Self {
            restart,
            current: None,
        }
    }

    /// The cue last started, until [`Self::stop`].
    pub fn current(&self) -> Option<u8> {
        self.current
    }

    /// `Snd_PlayMusic(n)` (music-cues §4.1): stop the current song and start song `n` from the
    /// top, with looping on except for [`DEATH`]. `load` reads a song file by name.
    ///
    /// Returns false and pushes nothing when `n` is not in the table or the song cannot be
    /// loaded (the original would replay the freed previous song; no call site passes such an
    /// `n`), or when [`CueRestart::IfDifferent`] keeps the current song.
    ///
    /// The game switches looping off only around the death cue and back on afterwards, at points
    /// the flow knows (music-cues §4.2); use [`Self::set_loop`] there. The flag is read when a
    /// song reaches its end (cmf §4.8).
    pub fn play_cue(
        &mut self,
        n: u8,
        load: impl FnOnce(&'static str) -> Option<Arc<Cmf>>,
        out: &mut Vec<AudioCommand>,
    ) -> bool {
        if self.restart == CueRestart::IfDifferent && self.current == Some(n) && cue_loops(n) {
            return false;
        }
        let Some(song) = song_file(n).and_then(load) else {
            return false;
        };
        out.push(AudioCommand::PlayMusic {
            song,
            looping: cue_loops(n),
        });
        self.current = Some(n);
        true
    }

    /// The flight song of mission `mission` (every flight start, also after a retry).
    pub fn play_flight(
        &mut self,
        mission: u8,
        load: impl FnOnce(&'static str) -> Option<Arc<Cmf>>,
        out: &mut Vec<AudioCommand>,
    ) -> bool {
        self.play_cue(flight_cue(mission), load, out)
    }

    /// Music stopped with no new song (flight start and end, the title card before mission 13).
    pub fn stop(&mut self, out: &mut Vec<AudioCommand>) {
        out.push(AudioCommand::StopMusic);
        self.current = None;
    }

    /// Sets the loop flag without touching the song (WORX function 1Dh).
    pub fn set_loop(&mut self, looping: bool, out: &mut Vec<AudioCommand>) {
        out.push(AudioCommand::SetMusicLoop(looping));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy() -> Arc<Cmf> {
        Arc::new(Cmf {
            version: 0x0101,
            ticks_per_quarter: 48,
            ticks_per_second: 96,
            channel_in_use: [false; 16],
            basic_tempo: 0,
            instruments: Vec::new(),
            events: Vec::new(),
            end_tick: 0,
        })
    }

    fn requested(out: &[AudioCommand]) -> Vec<bool> {
        out.iter()
            .map(|c| match c {
                AudioCommand::PlayMusic { looping, .. } => *looping,
                other => panic!("unexpected {other:?}"),
            })
            .collect()
    }

    #[test]
    fn cue_table() {
        assert_eq!(song_file(MENU), Some("menu.cmf"));
        assert_eq!(song_file(MOODMAJ), Some("moodmaj.cmf"));
        assert_eq!(song_file(DEATH), Some("death.cmf"));
        assert_eq!(song_file(MOOD3), Some("mood3.cmf"));
        assert_eq!(song_file(TD03), Some("td03.cmf"));
        assert_eq!(song_file(0), None);
        assert_eq!(song_file(15), None);
    }

    #[test]
    fn flight_song_rotates_by_mission() {
        let songs: Vec<_> = (1..=6).map(|m| song_file(flight_cue(m)).unwrap()).collect();
        assert_eq!(
            songs,
            [
                "td01.cmf", "td03.cmf", "td02.cmf", "td01.cmf", "td03.cmf", "td02.cmf"
            ]
        );
        assert_eq!(flight_cue(21), TD02);
    }

    #[test]
    fn classic_restarts_on_every_cue() {
        let mut c = MusicCues::new(CueRestart::Always);
        let mut out = Vec::new();
        let mut names = Vec::new();
        for n in [HANGAR, HANGAR, DEATH] {
            assert!(c.play_cue(
                n,
                |f| {
                    names.push(f);
                    Some(dummy())
                },
                &mut out
            ));
        }
        assert_eq!(names, ["hangar.cmf", "hangar.cmf", "death.cmf"]);
        assert_eq!(requested(&out), [true, true, false], "death plays once");
        assert_eq!(c.current(), Some(DEATH));
    }

    #[test]
    fn enhanced_keeps_a_looping_song() {
        let mut c = MusicCues::new(CueRestart::IfDifferent);
        let mut out = Vec::new();
        assert!(c.play_cue(HANGAR, |_| Some(dummy()), &mut out));
        assert!(!c.play_cue(HANGAR, |_| Some(dummy()), &mut out));
        assert!(c.play_cue(DEATH, |_| Some(dummy()), &mut out));
        assert!(
            c.play_cue(DEATH, |_| Some(dummy()), &mut out),
            "death has ended"
        );
        assert_eq!(out.len(), 3);
        c.stop(&mut out);
        assert!(c.play_cue(DEATH, |_| Some(dummy()), &mut out));
        assert!(matches!(out[3], AudioCommand::StopMusic));
    }

    #[test]
    fn unknown_cues_and_missing_files_do_nothing() {
        let mut c = MusicCues::new(CueRestart::Always);
        let mut out = Vec::new();
        assert!(!c.play_cue(0, |_| Some(dummy()), &mut out));
        assert!(!c.play_cue(MENU, |_| None, &mut out));
        assert!(out.is_empty());
        assert_eq!(c.current(), None);
    }
}
