//! L5 parity: the CMF player's OPL register writes against the DOSBox-X OPL captures (DRO) of
//! oracle runs C-0001 and C-0002 (cmf.md rev 2 §6.2 C1, C2). Needs `TD2192_DATA` (the songs)
//! and `TD2192_ORACLE` (the captures).
//!
//! Each DRO holds every OPL write from the first key-on to the end of the run, across programs:
//! the intro (`file0000`), the launcher (`td1`) and, in C-0002, the game (`file0001`). Song starts
//! are the PIT reprogrammings to divisor 12428 in each run's `pit.tsv`; in the DRO each song
//! change shows as a stop block followed by a song-start block. The test replays each run's music
//! schedule through the traced `CmfPlayer`. The cut points (ticks a song plays before the program
//! stops it) come from the capture: any count between the last event that shows in the DRO and
//! the next event gives the same writes, and the timing check pins the count to the capture's
//! timestamps.
//!
//! DOSBox's DRO capture starts at the first key-on, first lists every non-zero register of the
//! capture's register map in register order, and then logs a write only when it changes the
//! register (cmf §6.2 C1). The test applies the same filter to our trace and then requires the
//! whole DRO to match: same registers, values and order.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use td_audio::cmf_player::PIT_CLOCK;
use td_audio::{CmfPlayer, OplWrite};
use td_formats::cmf::Cmf;
use td_opl::OPL2_CLOCK;

/// Largest timing difference allowed between a capture and our tick schedule, in ticks, after
/// aligning each song's start. The captures' timestamps are whole ms and include the emulated
/// CPU time of each write burst (cmf §6.2 C2 gives −18..+11 ms). Measured: −0.6..+1.4 ticks.
const TIMING_TOLERANCE_TICKS: f64 = 1.5;

/// The same for a run with loop restarts (C-0073). A restart writes instrument entry 0 to all
/// nine voices, the rhythm table and the new pass's first notes in one interrupt; WORX's OPL
/// write routine spreads that burst over about 26 ms of emulated time (DRO 50,413..50,439 ms),
/// while our trace stamps every write of a tick with the tick's start. Measured: −1.1..+2.5
/// ticks; the pass length itself is checked exactly from the PIT writes.
const LOOP_TIMING_TOLERANCE_TICKS: f64 = 3.0;

/// One step of a run's music schedule.
#[derive(Debug, Clone, Copy)]
enum Step {
    /// WORX song start (function 1Eh) with this file, looping off for the intro and on in the
    /// game. The first one also starts the driver.
    Start(&'static str, bool),
    /// This many PIT ticks.
    Ticks(u32),
    /// Ticks until a non-looping song reaches its end of track.
    UntilEnd,
    /// WORX stop (function 0Dh).
    Stop,
    /// The next program starts its own driver (`CmfPlayer::restart_driver`).
    NewProgram,
}

use Step::*;

/// C-0001: the full intro with no input, to `file0001`'s configuration screen.
///
/// `intro1` is cut after 1937 ticks; `intro` plays to its end, and `intro2` starts 2 ticks
/// later. `intro2` stops after 2618 ticks: the written values allow 2612..=2630 (events at ticks
/// 2612 and 2631), the timestamps 2618. (cmf §6.2 C1 says "stop at tick 12634"; read as
/// 1937 + 8065 + 2 + 2630 that is in the range but 12 ticks later than the capture.) Then `td1`
/// starts its driver and writes `A0..A8 = 0`, `B0..B8 = 0`, which is a WORX stop with rhythm
/// mode off (only `A6..A8`, `B6..B8` still change).
const C0001: &[Step] = &[
    Start("intro1.cmf", false),
    Ticks(1937),
    Stop,
    Start("intro.cmf", false),
    UntilEnd,
    Ticks(2),
    Stop,
    Start("intro2.cmf", false),
    Ticks(2618),
    Stop,
    NewProgram,
    Stop,
];

/// C-0002: power-on to the first flight. The intro is skipped during `intro1`; `td1` stops as in
/// C-0001; `file0001` starts its own driver, and `Snd_PlayMusic` (stop, then start; music-cues
/// §4.1) plays `menu`, `hangar`, `low` and `td01` with looping on, each cut by the next cue and
/// `td01` by the end of the flight. The tick counts are the timestamps' (±1 tick; the written
/// values allow wider ranges). This run checks the driver restart between programs, the
/// built-in bank entry 0 in the game's first song start and the instrument leftovers between
/// songs.
const C0002: &[Step] = &[
    Start("intro1.cmf", false),
    Ticks(394),
    Stop,
    NewProgram,
    Stop,
    NewProgram,
    Stop,
    Start("menu.cmf", true),
    Ticks(863),
    Stop,
    Start("hangar.cmf", true),
    Ticks(1866),
    Stop,
    Start("low.cmf", true),
    Ticks(840),
    Stop,
    Start("td01.cmf", true),
    Ticks(3197),
    Stop,
];

/// C-0073: `file0001` started directly (no intro, no launcher), the main menu left alone.
/// `menu.cmf` plays with looping on and reaches its end of track twice: the PIT song divisor is
/// written again at both loop restarts (f3611, f7146). The run ends 11,949 ticks after the song
/// start (exit at 125,550 ms, start at 1,092 ms), with no stop.
const C0073: &[Step] = &[Start("menu.cmf", true), Ticks(11_949)];

/// One OPL write from a DRO v2 file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DroWrite {
    ms: u64,
    reg: u8,
    value: u8,
}

/// A DOSBox Raw OPL v2 capture.
struct Dro {
    /// Registers the capture can record (the file's code map).
    codemap: Vec<u8>,
    writes: Vec<DroWrite>,
}

fn parse_dro(d: &[u8]) -> Dro {
    assert_eq!(&d[..8], b"DBRAWOPL", "DRO signature");
    let u16_at = |i: usize| u16::from_le_bytes([d[i], d[i + 1]]);
    let u32_at = |i: usize| u32::from_le_bytes([d[i], d[i + 1], d[i + 2], d[i + 3]]);
    assert_eq!((u16_at(8), u16_at(10)), (2, 0), "DRO version 2.0");
    let pairs = u32_at(12) as usize;
    let (hw, short_delay, long_delay, cmlen) = (d[20], d[23], d[24], usize::from(d[25]));
    assert_eq!(hw, 0, "OPL2 capture");
    let codemap = d[26..26 + cmlen].to_vec();
    let mut writes = Vec::new();
    let mut ms = 0u64;
    for pair in d[26 + cmlen..26 + cmlen + 2 * pairs].chunks_exact(2) {
        let (code, val) = (pair[0], pair[1]);
        if code == short_delay {
            ms += u64::from(val) + 1;
        } else if code == long_delay {
            ms += (u64::from(val) + 1) << 8;
        } else {
            assert_eq!(code & 0x80, 0, "second register bank in an OPL2 capture");
            writes.push(DroWrite {
                ms,
                reg: codemap[usize::from(code)],
                value: val,
            });
        }
    }
    Dro { codemap, writes }
}

/// Where one of our writes came from.
#[derive(Debug, Clone, Copy)]
struct Stamp {
    /// Song start counter (a new program also counts, so its writes form their own group).
    song: usize,
    /// Tick of that song (0 = written by a start call, or before any tick of the song).
    tick: u32,
    sample: u64,
}

fn load(dir: &Path, name: &str) -> Arc<Cmf> {
    Arc::new(Cmf::parse(&fs::read(dir.join(name)).unwrap()).unwrap())
}

/// Plays `steps` through the traced player. Stop writes keep the stamp of the song they end and
/// the tick after which they happen, so the timing check covers the cut points too.
fn play(data: &Path, steps: &[Step]) -> Vec<(OplWrite, Stamp)> {
    let mut out = Vec::new();
    let mut p: Option<CmfPlayer> = None;
    let (mut song, mut tick) = (0usize, 0u32);
    let record = |p: &mut CmfPlayer, out: &mut Vec<_>, song, tick| {
        for w in p.take_trace() {
            let sample = w.sample;
            out.push((w, Stamp { song, tick, sample }));
        }
    };
    for &step in steps {
        if let Start(name, looping) = step {
            song += 1;
            tick = 0;
            let s = load(data, name);
            match p.as_mut() {
                None => p = Some(CmfPlayer::new_traced(s, looping)),
                Some(p) => {
                    p.set_looping(looping);
                    p.start(s);
                }
            }
            record(p.as_mut().unwrap(), &mut out, song, tick);
            continue;
        }
        let p = p.as_mut().expect("schedule starts with a song");
        let run_tick = |p: &mut CmfPlayer, out: &mut Vec<_>, tick: u32| {
            let n = p.samples_until_tick() as usize;
            p.render(&mut vec![0; n]);
            record(p, out, song, tick);
        };
        match step {
            Ticks(n) => {
                for _ in 0..n {
                    tick += 1;
                    run_tick(p, &mut out, tick);
                }
            }
            UntilEnd => {
                while p.playing() {
                    tick += 1;
                    run_tick(p, &mut out, tick);
                }
            }
            Stop => {
                p.stop();
                record(p, &mut out, song, tick);
            }
            NewProgram => {
                song += 1;
                tick = 0;
                p.restart_driver();
                record(p, &mut out, song, tick);
            }
            Start(..) => unreachable!(),
        }
    }
    out
}

/// What DOSBox's DRO capture would have recorded of `writes` (see the module comment).
fn capture_filter(writes: &[(OplWrite, Stamp)], codemap: &[u8]) -> Vec<(u8, u8, Option<Stamp>)> {
    let mut mapped = [false; 256];
    for &r in codemap {
        mapped[usize::from(r)] = true;
    }
    let mut regs = [0u8; 256];
    let mut out = Vec::new();
    let mut capturing = false;
    for &(w, stamp) in writes {
        let r = usize::from(w.reg);
        if !capturing {
            let key_on = (0xB0..=0xB8).contains(&w.reg) && w.value & 0x20 != 0;
            let drum_on = w.reg == 0xBD && (w.value & 0x3F) > 0x20;
            if key_on || drum_on {
                capturing = true;
                for (reg, &val) in regs.iter().enumerate() {
                    if mapped[reg] && val != 0 {
                        out.push((reg as u8, val, None));
                    }
                }
                out.push((w.reg, w.value, Some(stamp)));
            }
        } else if mapped[r] && regs[r] != w.value {
            out.push((w.reg, w.value, Some(stamp)));
        }
        regs[r] = w.value;
    }
    out
}

fn sample_ms(sample: u64) -> f64 {
    sample as f64 * 72_000.0 / f64::from(OPL2_CLOCK)
}

/// What [`check`] compared: our filtered stream (register, value, stamp; no stamp for the
/// capture's initial register dump) and the capture's writes, index for index.
type Compared = (Vec<(u8, u8, Option<Stamp>)>, Vec<DroWrite>);

/// Compares the schedule `steps` with capture `id`; `span` is the number of writes from the
/// first key-on to the end of the DRO.
fn check(id: &str, steps: &[Step], span: usize) {
    check_dro(
        id,
        "capture/file0000_000.dro",
        steps,
        span,
        TIMING_TOLERANCE_TICKS,
    );
}

/// [`check`] for a capture whose DRO is `dro_file` (DOSBox names it after the program that ran
/// when the recording started). Returns what was compared, or `None` when skipped.
fn check_dro(
    id: &str,
    dro_file: &str,
    steps: &[Step],
    span: usize,
    tolerance_ticks: f64,
) -> Option<Compared> {
    let (Some(oracle), Some(data)) = (td_test_support::oracle_dir(), td_test_support::data_dir())
    else {
        eprintln!("SKIPPED: TD2192_ORACLE or TD2192_DATA not set (oracle parity test {id})");
        return None;
    };
    let dro = parse_dro(&fs::read(oracle.join(id).join(dro_file)).unwrap());
    let ours = capture_filter(&play(&data, steps), &dro.codemap);
    let theirs = &dro.writes;

    // Register-write stream: exact, the whole capture.
    let n = ours.len().min(theirs.len());
    let differs = |i: usize| (ours[i].0, ours[i].1) != (theirs[i].reg, theirs[i].value);
    if let Some(i) = (0..n)
        .find(|&i| differs(i))
        .or((ours.len() != theirs.len()).then_some(n))
    {
        let ctx: Vec<String> = (i.saturating_sub(4)..(i + 8).min(n))
            .map(|j| {
                format!(
                    "{j:5}: ours {:02X}={:02X} {:?} | capture {:02X}={:02X} at {} ms",
                    ours[j].0, ours[j].1, ours[j].2, theirs[j].reg, theirs[j].value, theirs[j].ms
                )
            })
            .collect();
        panic!(
            "{id}: first difference at write {i} (ours {}, capture {}):\n{}",
            ours.len(),
            theirs.len(),
            ctx.join("\n")
        );
    }
    let dump = ours.iter().take_while(|w| w.2.is_none()).count();
    assert_eq!(
        ours.len() - dump,
        span,
        "{id}: writes from the first key-on"
    );

    // Timing: per song, the capture time minus our time is a constant (the song's start) plus
    // jitter.
    let tick_ms = 1000.0 * 12428.0 / f64::from(PIT_CLOCK);
    let songs = ours
        .iter()
        .filter_map(|w| w.2)
        .map(|s| s.song)
        .max()
        .unwrap();
    for song in 1..=songs {
        let mut diffs: Vec<f64> = ours
            .iter()
            .zip(theirs)
            .filter_map(|(o, t)| {
                let s = o.2.filter(|s| s.song == song && s.tick > 0)?;
                Some(t.ms as f64 - sample_ms(s.sample))
            })
            .collect();
        if diffs.is_empty() {
            continue;
        }
        diffs.sort_by(f64::total_cmp);
        let offset = diffs[diffs.len() / 2];
        let (lo, hi) = (diffs[0] - offset, diffs[diffs.len() - 1] - offset);
        eprintln!(
            "{id} song {song}: {} tick writes, capture − ours = {offset:.1} ms, \
             {lo:+.1}/{hi:+.1} ms ({:+.2}/{:+.2} ticks)",
            diffs.len(),
            lo / tick_ms,
            hi / tick_ms
        );
        let limit = tolerance_ticks * tick_ms;
        assert!(
            lo >= -limit && hi <= limit,
            "{id} song {song}: timing off by {lo:.1}..{hi:.1} ms"
        );
    }
    Some((ours, dro.writes))
}

/// cmf §6.2 C1: 4,364 writes from the first key-on to the intro's final stop, plus `td1`'s 6.
#[test]
fn intro_music_matches_capture_c0001() {
    check("C-0001", C0001, 4364 + 6);
}

/// The intro cut short, then the game's menu, hangar, briefing and flight songs.
#[test]
fn game_music_matches_capture_c0002() {
    check("C-0002", C0002, 3438 - 85);
}

/// Song starts in a run's `pit.tsv`: the emulated ms of every PIT write of divisor 12428 by
/// `prog` (a WORX song start, also at a loop restart).
fn song_start_ms(oracle: &Path, id: &str, prog: &str) -> Vec<f64> {
    fs::read_to_string(oracle.join(id).join("pit.tsv"))
        .unwrap()
        .lines()
        .skip(1)
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            (f[2] == prog && f[7].contains("count=12428")).then(|| f[1].parse().unwrap())
        })
        .collect()
}

/// cmf §4.8, M3 parity row 12: a looping song that reaches its end of track. The whole DRO
/// matches write for write, including the two restart blocks (`BD=C0` and instrument entry 0 to
/// voices 0..8) and the new pass's first events in the same burst. The pass after a restart
/// lasts as many ticks as ours (4,841 for `menu.cmf`, the sum of its delta times): the capture's
/// restart-to-restart interval is that many PIT periods plus the time WORX takes to reach the
/// PIT write inside the restart (under one tick).
#[test]
fn looping_menu_music_matches_capture_c0073() {
    let Some((ours, _)) = check_dro(
        "C-0073",
        "capture/file0001_000.dro",
        C0073,
        6540 - 86,
        LOOP_TIMING_TOLERANCE_TICKS,
    ) else {
        return;
    };
    let oracle = td_test_support::oracle_dir().expect("checked above");
    let restarts: Vec<u32> = ours
        .iter()
        .filter_map(|&(r, v, s)| ((r, v) == (0xBD, 0xC0)).then_some(s?.tick))
        .collect();
    assert_eq!(restarts.len(), 2, "two loop restarts in C-0073");
    let starts = song_start_ms(&oracle, "C-0073", "FILE0001");
    assert_eq!(starts.len(), 3, "song start + two restarts in pit.tsv");
    let period_ms = 1000.0 * 12428.0 / f64::from(PIT_CLOCK);
    let capture_pass = (starts[2] - starts[1]) / period_ms;
    let ours_pass = restarts[1] - restarts[0];
    eprintln!(
        "C-0073: restarts at our ticks {restarts:?}; PIT song starts {starts:?} ms; \
         restart-to-restart {capture_pass:.3} periods, ours {ours_pass} ticks"
    );
    assert_eq!(ours_pass, 4841, "menu.cmf pass after a restart");
    assert_eq!(
        capture_pass.floor() as u32,
        ours_pass,
        "pass after a restart"
    );
    // Finding (docs/parity/M3-audio.md row 12a, U-401): in the capture the first pass is as long
    // as the later ones (start-to-restart equals restart-to-restart to the microsecond), so the
    // song's first event runs in the start call, not on tick d0 + 1 as cmf §4.4 and our player
    // have it (our first restart is on tick 4842). Reported, not asserted, until the spec decides.
    let capture_first = (starts[1] - starts[0]) / period_ms;
    eprintln!(
        "C-0073: start-to-restart {capture_first:.3} periods in the capture; our first restart \
         on tick {}",
        restarts[0]
    );
}
