//! The intro against oracle capture C-0047 (`docs/spec/flow/intro.md` rev 1 §6): the flyover
//! state per step (vector 4), the step cadence (vector 5), every captured frame of the cards
//! and the flyover windows (vectors 6 and 7) and the full fades (vector 3).
//!
//! Gated on TD2192_ORACLE (and TD2192_DATA for everything that draws). No captured pixels are
//! stored; the captures are read at test time.
//!
//! Frame convention: the capture's framebuffer dump of frame N shows the state after every
//! write of frames ≤ N − 1. Our tick k is the capture's frame k, so dump N is compared with our
//! state after tick N − 1, counted from anchors: the first write of each fade (from
//! `frames.tsv`) against our [`Mark`]s. Flyover frames are matched by the step value in the
//! capture's RAM instead (vector 6).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use td_assets::{DataDir, ExeResources};
use td_flow::intro::flyover::{END_STEP, FlyoverState};
use td_flow::intro::{Intro, IntroOptions, Mark};
use td_flow::screen::Screen;
use td_formats::palette::Palette;
use td_input::TickInput;
use td_render::Framebuffer;
use td_test_support::{require_data, require_oracle};

/// Capture frames of the first write of each fade, in the order of our [`Mark`]s (from
/// C-0047 `frames.tsv`, dac_3c8 column; intro §6 vectors 2 and 3).
const ANCHORS: [u64; 24] = [
    136,  // date: colour fade in (52)
    274,  // date: colour fade out
    281,  // intro0: full fade in
    346,  // card 1 in (52)
    719,  // card 1 out
    726,  // card 2 in (92)
    1099, // card 2 out
    1107, // card 3 in (195)
    1478, // card 3 out
    1485, // intro0: full fade out
    1552, // flyover: full fade in
    7520, // flyover: full fade out
    7614, // intro1: full fade in
    7679, // line 1 in (45)
    8020, // line 1 out
    8027, // line 2 in
    8366, // line 2 out
    8373, // intro1: full fade out
    8459, // intro2: full fade in
    8524, // line 1 down (140)
    8896, // line 1 up
    8904, // line 2 down (141)
    8904 + 375, // line 2 up (f9279)
    9286, // exit: full fade out
];
/// Full-fade anchors (their dumps can show a level half written, §6 vector 3).
const FULL_FADES: [usize; 8] = [2, 9, 10, 11, 12, 17, 18, 23];
/// The DAC entry of each colour-fade anchor (0 for the full fades).
const INKS: [u8; 24] = [
    52, 52, 0, 52, 52, 92, 92, 195, 195, 0, 0, 0, 0, 45, 45, 45, 45, 0, 0, 140, 140, 141, 141, 0,
];
/// Capture frames in which a retrace passed without the write the sequence expected (a dropped
/// fade step: frames.tsv shows no write at f1105 inside card 2's fade-out, and a level split
/// 64 + 191 around f7523 in the flyover's fade-out). Later frames of the same segment are one
/// frame late in the capture.
const DROPPED: [u64; 2] = [1105, 7523];
/// From this DAC entry on, a dump taken during a full fade may still hold the previous level:
/// the capture's frame boundary falls after about 190 of the 255 writes (frames.tsv: 190 or
/// 191 writes in a fade's first frame).
const TEAR: usize = 190;

fn capture(oracle: &Path) -> PathBuf {
    oracle.join("C-0047")
}

/// Framebuffer dumps by frame.
fn fb_frames(oracle: &Path) -> BTreeMap<u64, PathBuf> {
    let dir = capture(oracle).join("fb");
    let mut out = BTreeMap::new();
    for e in fs::read_dir(&dir).unwrap() {
        let p = e.unwrap().path();
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        if let Some(n) = name.get(1..9).and_then(|n| n.parse().ok()) {
            out.insert(n, p);
        }
    }
    out
}

fn read_fb(path: &Path) -> (Palette, Vec<u8>) {
    let raw = fs::read(path).unwrap();
    (Palette::parse_prefix(&raw).unwrap(), raw[768..].to_vec())
}

/// The flyover variables in one RAM dump (DGROUP 1EE5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Ram {
    s: u16,
    v194: i16,
    v196: i16,
    v198: i16,
    flag: u8,
    steer: i16,
    speed: i16,
    y: i16,
    page: u8,
}

fn word(b: &[u8], at: usize) -> i16 {
    i16::from_le_bytes([b[at], b[at + 1]])
}

/// RAM dumps by frame (C-0047 `mem/`: DS:0000 0x200, DS:AA3A 8, DS:B44B 1).
fn ram(oracle: &Path) -> BTreeMap<u64, Ram> {
    let dir = capture(oracle).join("mem");
    let mut frames = BTreeSet::new();
    for e in fs::read_dir(&dir).unwrap() {
        let name = e.unwrap().file_name().to_string_lossy().into_owned();
        if let Some(n) = name.get(1..9).and_then(|n| n.parse::<u64>().ok()) {
            frames.insert(n);
        }
    }
    let mut out = BTreeMap::new();
    for f in frames {
        let read = |tag: &str| fs::read(dir.join(format!("f{f:08}_{tag}.bin"))).ok();
        let (Some(ds), Some(skid), Some(page)) = (
            read("1EE5-0000_200_ds0"),
            read("1EE5-AA3A_8_skid"),
            read("1EE5-B44B_1_page"),
        ) else {
            continue;
        };
        out.insert(
            f,
            Ram {
                s: word(&ds, 0x190) as u16,
                v194: word(&ds, 0x194),
                v196: word(&ds, 0x196),
                v198: word(&ds, 0x198),
                flag: ds[0x69],
                steer: word(&skid, 2),
                speed: word(&skid, 4),
                y: word(&skid, 6),
                page: page[0],
            },
        );
    }
    out
}

/// Our states: index s = the state when the capture's RAM shows s (after step s − 1's motion,
/// with page = the page step s draws on).
fn model_states() -> Vec<FlyoverState> {
    let mut st = FlyoverState::new();
    let mut out = vec![st.clone()];
    while !st.ended() {
        st.begin();
        let mut shown = st.clone();
        // The RAM shows the motion of the step before.
        shown.s = st.s;
        out.push(shown);
        st.flip_and_move();
    }
    out.push(st);
    out
}

#[test]
fn vector_4_flyover_state_matches_every_ram_dump() {
    let oracle = require_oracle!();
    let dumps = ram(&oracle);
    assert!(!dumps.is_empty(), "no RAM dumps in C-0047");
    let states = model_states();
    let mut compared = 0;
    let mut mid_step = 0;
    let mut steps = BTreeSet::new();
    for (&f, r) in &dumps {
        assert_eq!(r.v196, 16, "f{f}: v196");
        let st = match r.s {
            0 => continue, // before the first step (the fade-in)
            END_STEP => states.last().unwrap(),
            // A dump between step s's motion and step s + 1's increment (the page already
            // flipped; f7439..7440, while intro2.cmf is started).
            s if r.page != ((s + 1) % 2) as u8 => {
                mid_step += 1;
                &states[usize::from(s) + 1]
            }
            s => &states[usize::from(s)],
        };
        let ours = (st.v194, st.v198, st.y, st.steer, st.speed);
        let theirs = (r.v194, r.v198, r.y, r.steer, r.speed);
        assert_eq!(ours, theirs, "f{f}: s = {}", r.s);
        if r.s != END_STEP {
            assert_eq!(st.page, r.page, "f{f}: page");
        }
        steps.insert(r.s);
        compared += 1;
    }
    eprintln!(
        "vector 4: {compared} RAM dumps ({mid_step} between two steps), {} distinct steps",
        steps.len()
    );
    assert!(compared > 800, "{compared}");
}

fn load_intro() -> Option<Intro> {
    let data = td_test_support::data_dir()?;
    let dir = DataDir::open(&data).unwrap();
    let res = ExeResources::load(&dir).unwrap();
    Some(Intro::load(&dir, &res, IntroOptions::default()).unwrap())
}

/// Runs the whole intro once without input; returns our marks.
fn marks() -> Vec<(u64, Mark)> {
    let mut intro = load_intro().unwrap();
    let mut audio = Vec::new();
    while !intro.finished() {
        intro.step(&TickInput::default(), &mut audio);
        audio.clear();
    }
    intro.marks().to_vec()
}

/// The capture's frame in which step s began (the dump of the next frame shows s).
fn capture_step_starts(dumps: &BTreeMap<u64, Ram>) -> BTreeMap<u16, u64> {
    let mut out = BTreeMap::new();
    let mut prev = None;
    for (&f, r) in dumps {
        if Some(r.s) != prev && prev.is_some() && (1..END_STEP).contains(&r.s) {
            out.entry(r.s).or_insert(f - 1);
        }
        prev = Some(r.s);
    }
    out
}

#[test]
fn vector_5_step_cadence() {
    let _data = require_data!();
    let oracle = require_oracle!();
    let dumps = ram(&oracle);
    let theirs = capture_step_starts(&dumps);
    let ours: BTreeMap<u16, u64> = marks()
        .iter()
        .filter_map(|&(f, m)| match m {
            Mark::Step(s) => Some((s, f)),
            _ => None,
        })
        .collect();
    assert_eq!(ours.len(), 1618);
    let (o1, t1) = (ours[&1], 1617);
    let mut worst = 0i64;
    for (&s, &t) in &theirs {
        // Only steps whose start the capture saw (a dump the frame before and after).
        if !dumps.contains_key(&t) {
            continue;
        }
        let d = (ours[&s] - o1) as i64 - (t - t1) as i64;
        worst = worst.max(d.abs());
    }
    let span = ours[&1618] - o1;
    eprintln!(
        "vector 5: {} captured step starts, worst offset {worst} frames, span {span} (capture 5899)",
        theirs.len()
    );
    // T2: the port's BIOS tick is the nominal 1,193,180 / 62,140 Hz; DOSBox's timer runs about
    // 0.025 % fast against its frames (2169 IRQ0 in f1618..3200), and the capture loses a
    // frame or two where intro2.cmf starts (steps 1595..1597).
    assert!(worst <= 3, "step cadence off by {worst} frames");
    assert!(span.abs_diff(7517 - 1618) <= 3, "span {span}");
    let starts: Vec<u64> = ours.values().copied().collect();
    assert!(starts.windows(2).all(|w| (3..=4).contains(&(w[1] - w[0]))));
}

/// What a dump is compared with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    /// Our state after tick `m`.
    Tick { m: u64, allow: Allow },
    /// Our state when step s began.
    Step(u16),
}

/// Allowed differences of one dump.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Allow {
    /// Inside a full fade: entries from [`TEAR`] on may hold the previous level.
    tear: bool,
    /// Inside a colour fade: this entry may be up to one frame's levels (10) away. The capture
    /// splits the 64 levels over its frames where the hold happened to end inside a retrace
    /// (f1478: 2 levels, then 10 per frame).
    colour: Option<u8>,
}

#[test]
fn vectors_3_6_7_every_captured_frame() {
    let _data = require_data!();
    let oracle = require_oracle!();
    let ours = marks();
    let mine: Vec<u64> = ours
        .iter()
        .filter(|(_, m)| !matches!(m, Mark::Step(_) | Mark::End))
        .map(|&(f, _)| f)
        .collect();
    assert_eq!(mine.len(), ANCHORS.len(), "fade count");
    let end = ours.last().unwrap().0;
    let dumps = ram(&oracle);
    let files = fb_frames(&oracle);
    let mut targets: BTreeMap<u64, Target> = BTreeMap::new();
    let mut skipped = 0;
    for &n in files.keys() {
        let w = n.saturating_sub(1);
        if let Some(r) = dumps.get(&n)
            && (1..END_STEP).contains(&r.s)
        {
            // A dump between two steps (page already flipped) shows step s's page: our
            // display when step s + 1 begins.
            let mid = r.page != ((r.s + 1) % 2) as u8;
            targets.insert(n, Target::Step(r.s + u16::from(mid)));
            continue;
        }
        let Some(i) = ANCHORS.iter().rposition(|&a| a <= w) else {
            skipped += 1; // before the date card: loading
            continue;
        };
        let dropped = DROPPED
            .iter()
            .filter(|&&d| ANCHORS[i] <= d && d <= w)
            .count() as u64;
        let off = w - ANCHORS[i] - dropped;
        let m = mine[i] + off;
        let seg_end = mine.get(i + 1).copied().unwrap_or(end + 1);
        // Flyover frames without RAM (every 5th frame between the windows) and the frames
        // after a segment's end on our side (holds differ by a frame or two, loading gaps).
        let flying = i == 10 && w >= 1617;
        if m >= seg_end || flying {
            skipped += 1;
            continue;
        }
        let allow = Allow {
            tear: FULL_FADES.contains(&i) && off <= 65,
            colour: (INKS[i] != 0 && off <= 7).then_some(INKS[i]),
        };
        targets.insert(n, Target::Tick { m, allow });
    }
    let mut by_tick: BTreeMap<u64, Vec<(u64, Allow)>> = BTreeMap::new();
    let mut by_step: BTreeMap<u16, Vec<u64>> = BTreeMap::new();
    for (&n, &t) in &targets {
        match t {
            Target::Tick { m, allow } => by_tick.entry(m).or_default().push((n, allow)),
            Target::Step(s) => by_step.entry(s).or_default().push(n),
        }
    }
    let mut intro = load_intro().unwrap();
    let mut audio = Vec::new();
    let mut fb = Framebuffer::new();
    let mut prev_dac = Palette::default();
    let mut failures = Vec::new();
    let (mut compared, mut black, mut raced) = (0, 0, Vec::new());
    let mut steps_seen = 0;
    let mut flags = BTreeMap::new();
    // Step frames that did not match step s, retried against step s + 1: the dump caught the
    // new start address before s was incremented (f1738, f7344).
    let mut retry: Vec<(u64, String)> = Vec::new();
    while !intro.finished() {
        prev_dac.clone_from(intro.dac());
        intro.step(&TickInput::default(), &mut audio);
        audio.clear();
        let m = intro.frame();
        let jobs: Vec<(u64, Allow)> = by_tick.remove(&m).unwrap_or_default();
        let mut step_jobs = Vec::new();
        let mut new_step = false;
        if let Some(&(f, Mark::Step(s))) = intro.marks().last()
            && f == m
        {
            flags.insert(s, intro.flyover().flag);
            step_jobs = by_step.remove(&s).unwrap_or_default();
            steps_seen += 1;
            new_step = true;
        }
        if step_jobs.is_empty() && jobs.is_empty() && !(new_step && !retry.is_empty()) {
            continue;
        }
        intro.draw(&mut fb);
        if new_step {
            for (n, first) in std::mem::take(&mut retry) {
                match compare(&fb, &prev_dac, &files[&n], Allow::default()) {
                    Ok(_) => raced.push(n),
                    Err(_) => failures.push(first),
                }
            }
        }
        for n in step_jobs {
            compared += 1;
            match compare(&fb, &prev_dac, &files[&n], Allow::default()) {
                Ok(Seen::Equal) => {}
                Ok(Seen::Black) => black += 1,
                Err(e) => retry.push((n, format!("f{n} (step, tick {m}): {e}"))),
            }
        }
        for (n, allow) in jobs {
            compared += 1;
            match compare(&fb, &prev_dac, &files[&n], allow) {
                Ok(Seen::Equal) => {}
                Ok(Seen::Black) => black += 1,
                Err(e) => failures.push(format!("f{n} (tick {m}): {e}")),
            }
        }
    }
    failures.extend(retry.into_iter().map(|(_, e)| e));
    eprintln!(
        "vectors 3/6/7: {compared} frames compared ({} targets, {skipped} skipped, {black} \
         black on both sides), {steps_seen} steps run, start address ahead of RAM in {raced:?}, \
         {} failures",
        targets.len(),
        failures.len()
    );
    assert!(by_tick.is_empty() && by_step.is_empty(), "unmatched targets");
    // The effect flag DS:0069 (intro §3.6) depends on the effect lengths: check it here,
    // where the effects play.
    for (&f, r) in &dumps {
        if r.page == ((r.s + 1) % 2) as u8
            && let Some(&flag) = flags.get(&r.s)
        {
            assert_eq!(u8::from(flag), r.flag, "f{f}: effect flag at step {}", r.s);
        }
    }
    assert!(
        failures.is_empty(),
        "{}",
        failures[..failures.len().min(20)].join("\n")
    );
    assert!(compared > 1500, "{compared}");
    assert!(raced.len() <= 4, "{raced:?}");
}

/// How a dump compared.
#[derive(Debug, PartialEq, Eq)]
enum Seen {
    Equal,
    /// Every colour the dump uses is black, and ours too: an invisible (loading) frame whose
    /// pixels are not compared (f1551: the capture still holds intro0 while `td.pal` loads).
    Black,
}

/// Pixels exactly; the DAC entries the frame uses exactly, except as [`Allow`] says.
fn compare(ours: &Framebuffer, prev: &Palette, file: &Path, allow: Allow) -> Result<Seen, String> {
    let (dac, pixels) = read_fb(file);
    let dark = |p: &Palette, px: &[u8]| {
        px.iter()
            .all(|&e| p.colors[usize::from(e)] == Default::default())
    };
    if dark(&dac, &pixels) && dark(&ours.palette, &ours.pixels) {
        return Ok(Seen::Black);
    }
    let diffs: Vec<String> = (0..pixels.len())
        .filter(|&i| ours.pixels[i] != pixels[i])
        .take(8)
        .map(|i| {
            format!(
                "({}, {}): {} vs {}",
                i % 320,
                i / 320,
                ours.pixels[i],
                pixels[i]
            )
        })
        .collect();
    if !diffs.is_empty() {
        let n = (0..pixels.len())
            .filter(|&i| ours.pixels[i] != pixels[i])
            .count();
        return Err(format!("{n} pixels differ: {diffs:?}"));
    }
    let used: BTreeSet<u8> = pixels.iter().copied().collect();
    let near = |a: u8, b: u8| a.abs_diff(b) <= 10;
    for e in used {
        let i = usize::from(e);
        let (c, o) = (dac.colors[i], ours.palette.colors[i]);
        if c == o
            || (allow.tear && i >= TEAR && c == prev.colors[i])
            || (allow.colour == Some(e) && near(c.r, o.r) && near(c.g, o.g) && near(c.b, o.b))
        {
            continue;
        }
        return Err(format!("DAC {e}: ours {o:?}, capture {c:?}"));
    }
    Ok(Seen::Equal)
}
