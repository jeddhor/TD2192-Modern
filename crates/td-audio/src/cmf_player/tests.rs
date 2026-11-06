//! L0 tests: the synthetic vectors of cmf.md §6.1 and the cross-song rules.

use super::*;

const I0: [u8; 11] = [
    0x01, 0x11, 0x4F, 0x00, 0xF1, 0xF2, 0x53, 0x74, 0x00, 0x00, 0x06,
];
const I1: [u8; 11] = [
    0x21, 0x31, 0x8A, 0x90, 0xE2, 0xD3, 0x15, 0x26, 0x01, 0x02, 0x0B,
];

/// A CMF v1.01 file with instruments `inst` and the event track `track` (deltas included).
fn cmf(inst: &[[u8; 11]], track: &[u8]) -> Arc<Cmf> {
    let mut v = vec![0u8; 0x28];
    v[..4].copy_from_slice(b"CTMF");
    v[4..6].copy_from_slice(&0x0101u16.to_le_bytes());
    v[6..8].copy_from_slice(&0x28u16.to_le_bytes());
    let music = 0x28 + 16 * inst.len() as u16;
    v[8..10].copy_from_slice(&music.to_le_bytes());
    v[0x0A..0x0C].copy_from_slice(&48u16.to_le_bytes());
    v[0x0C..0x0E].copy_from_slice(&96u16.to_le_bytes());
    v[0x24..0x26].copy_from_slice(&(inst.len() as u16).to_le_bytes());
    for i in inst {
        v.extend_from_slice(i);
        v.extend_from_slice(&[0; 5]);
    }
    v.extend_from_slice(track);
    Arc::new(Cmf::parse(&v).unwrap())
}

/// The two-instrument test song of cmf §6.1 with `events` followed by an end of track.
fn song(events: &[u8]) -> Arc<Cmf> {
    let mut track = events.to_vec();
    track.extend_from_slice(&[0x00, 0xFF, 0x2F, 0x00]);
    cmf(&[I0, I1], &track)
}

/// Writes of an instrument load into voice `v` (cmf §4.6).
fn inst_writes(v: usize, b: &[u8; 11]) -> Vec<(u8, u8)> {
    let mut w = Vec::new();
    for (i, base) in [0x20u8, 0x40, 0x60, 0x80, 0xE0].into_iter().enumerate() {
        w.push((base + MOD[v], b[2 * i]));
        w.push((base + CAR[v], b[2 * i + 1]));
    }
    w.push((0xC0 + v as u8, b[10]));
    w
}

fn regs(pairs: &[(u8, u8)]) -> Vec<(u8, u8)> {
    pairs.to_vec()
}

/// Runs a fresh driver on `song` for `ticks` ticks. Element 0 holds the start-up and song-start
/// writes, element k the writes of tick k.
fn run(song: Arc<Cmf>, looping: bool, ticks: usize) -> Vec<Vec<(u8, u8)>> {
    let mut w = Vec::new();
    let mut d = CmfDriver::new(&mut w);
    d.set_looping(looping);
    d.start(song, &mut w);
    let mut out = vec![w];
    for _ in 0..ticks {
        let mut w = Vec::new();
        d.tick(&mut w);
        out.push(w);
    }
    out
}

fn concat(parts: &[Vec<(u8, u8)>]) -> Vec<(u8, u8)> {
    parts.concat()
}

#[test]
fn song_start_writes_the_builtin_instrument_to_every_voice() {
    let t = run(song(&[]), false, 0);
    let mut expected = vec![(0x01, 0x20), (0x00, 0x00), (0xBD, 0xC0)];
    for v in 0..9 {
        expected.extend(inst_writes(v, &BUILTIN_INSTRUMENT));
    }
    assert_eq!(t[0], expected);
}

#[test]
fn v1_v2_velocity_is_ignored() {
    let expected_t1 = concat(&[
        regs(&[(0xBD, 0xC0)]),
        inst_writes(0, &I1),
        regs(&[(0xA0, 0x41), (0xB0, 0x32), (0x43, 0x11)]),
    ]);
    for vel in [0x7F, 0x14] {
        let t = run(
            song(&[
                0x00, 0xB0, 0x67, 0x00, 0x00, 0xC0, 0x01, 0x00, 0x90, 0x45, vel, 0x0A, 0x90, 0x45,
                0x00,
            ]),
            false,
            12,
        );
        assert_eq!(t[1], expected_t1, "velocity {vel:#x}");
        for (k, w) in t.iter().enumerate().take(11).skip(2) {
            assert!(w.is_empty(), "tick {k}: {w:x?}");
        }
        assert_eq!(t[11], [(0xA0, 0x41), (0xB0, 0x12), (0x43, 0x11)]);
        assert!(t[12].is_empty());
    }
}

#[test]
fn v3_channel_volume_scales_the_carrier_level() {
    let t = run(
        song(&[
            0x00, 0xB0, 0x67, 0x00, 0x00, 0xB0, 0x07, 0x40, 0x00, 0xC0, 0x01, 0x00, 0x90, 0x45,
            0x7F,
        ]),
        false,
        1,
    );
    assert_eq!(t[1].last(), Some(&(0x43, 0x29)));
}

#[test]
fn v4_transpose_69h_lowers_the_f_number() {
    let t = run(
        song(&[
            0x00, 0xB0, 0x67, 0x00, 0x00, 0xB0, 0x69, 0x40, 0x00, 0x90, 0x3C, 0x64,
        ]),
        false,
        1,
    );
    let expected = concat(&[
        regs(&[(0xBD, 0xC0)]),
        inst_writes(0, &I0),
        regs(&[(0xA0, 0xA3), (0xB0, 0x2E), (0x43, 0x01)]),
    ]);
    assert_eq!(t[1], expected);
}

#[test]
fn v5_voice_allocation() {
    let t = run(
        song(&[
            0x00, 0xB0, 0x67, 0x00, // rhythm off
            0x00, 0x90, 0x3C, 0x64, // t1 ch0 on
            0x01, 0x90, 0x40, 0x64, // t2 ch0 on
            0x01, 0xC1, 0x01, // t3 ch1 program 1
            0x00, 0x91, 0x43, 0x64, // t3 ch1 on
            0x01, 0x90, 0x3C, 0x00, // t4 ch0 off
            0x01, 0x92, 0x48, 0x64, // t5 ch2 on
            0x01, 0x90, 0x3E, 0x64, // t6 ch0 on
        ]),
        false,
        6,
    );
    let on = |v: u8, a: u8, b: u8, l: u8| regs(&[(0xA0 + v, a), (0xB0 + v, b), (l, 0x01)]);
    assert_eq!(
        t[1],
        concat(&[
            regs(&[(0xBD, 0xC0)]),
            inst_writes(0, &I0),
            on(0, 0xAE, 0x2E, 0x43)
        ])
    );
    assert_eq!(
        t[2],
        concat(&[inst_writes(1, &I0), on(1, 0xB0, 0x31, 0x44)])
    );
    assert_eq!(
        t[3],
        concat(&[
            inst_writes(2, &I1),
            regs(&[(0xA2, 0x02), (0xB2, 0x32), (0x45, 0x11)])
        ])
    );
    assert_eq!(t[4], on(0, 0xAE, 0x0E, 0x43));
    assert_eq!(
        t[5],
        concat(&[inst_writes(3, &I0), on(3, 0xAE, 0x32, 0x4B)])
    );
    assert_eq!(
        t[6],
        on(0, 0x81, 0x31, 0x43),
        "own voice, no instrument write"
    );
}

#[test]
fn v6_drums_toggle() {
    let t = run(
        song(&[
            0x00, 0xB0, 0x67, 0x01, 0x02, 0x9B, 0x24, 0x64, 0x02, 0x9B, 0x24, 0x00, 0x02, 0x9C,
            0x26, 0x64, 0x02, 0x9C, 0x26, 0x64, 0x02, 0x9F, 0x2A, 0x64,
        ]),
        false,
        11,
    );
    let mut expected = vec![(0xBD, 0xE0)];
    expected.extend(RHYTHM_INIT);
    expected.extend([(0xBD, 0xE0), (0x08, 0x00)]);
    assert_eq!(t[1], expected);
    assert_eq!(t[3], [(0xBD, 0xF0)]);
    assert_eq!(t[5], [(0xBD, 0xE0)], "note-off toggles too");
    assert_eq!(t[7], [(0xBD, 0xE8)]);
    assert_eq!(t[9], [(0xBD, 0xE0)], "a second note-on toggles off");
    assert_eq!(t[11], [(0xBD, 0xE1)]);
}

#[test]
fn v7_extreme_notes_corrupt_the_block() {
    let t = run(
        song(&[
            0x00, 0xB0, 0x67, 0x00, 0x00, 0x90, 0x01, 0x64, 0x00, 0x91, 0x0D, 0x64, 0x00, 0x92,
            0x6C, 0x64, 0x00, 0x93, 0x7F, 0x64,
        ]),
        false,
        1,
    );
    let on = |v: usize, a: u8, b: u8| {
        concat(&[
            inst_writes(v, &I0),
            regs(&[
                (0xA0 + v as u8, a),
                (0xB0 + v as u8, b),
                (0x40 + CAR[v], 0x01),
            ]),
        ])
    };
    let expected = concat(&[
        regs(&[(0xBD, 0xC0)]),
        on(0, 0xCA, 0x71),
        on(1, 0x6B, 0x21),
        on(2, 0xAE, 0x3E),
        on(3, 0x02, 0x26),
    ]);
    assert_eq!(t[1], expected);
}

#[test]
fn v8_loop_restarts_in_the_same_tick_without_key_off() {
    let track = [
        0x03, 0xB0, 0x67, 0x00, 0x00, 0x90, 0x3C, 0x64, 0x02, 0x90, 0x3C, 0x00, 0x00, 0xFF, 0x2F,
        0x00,
    ];
    let t = run(cmf(&[I0, I1], &track), true, 16);
    let mut restart = vec![(0x00, 0x00), (0xBD, 0xC0)];
    for v in 0..9 {
        restart.extend(inst_writes(v, &I0));
    }
    let on = concat(&[
        regs(&[(0xBD, 0xC0)]),
        inst_writes(0, &I0),
        regs(&[(0xA0, 0xAE), (0xB0, 0x2E), (0x43, 0x01)]),
    ]);
    let off = regs(&[(0xA0, 0xAE), (0xB0, 0x0E), (0x43, 0x01)]);
    let off_restart = concat(&[off, restart]);
    let mut expected = vec![Vec::new(); 17];
    expected[4] = on.clone();
    expected[6] = off_restart.clone();
    expected[9] = on.clone();
    expected[11] = off_restart.clone();
    expected[14] = on;
    expected[16] = off_restart;
    assert_eq!(&t[1..], &expected[1..]);
}

#[test]
fn end_of_track_without_loop_stops_silently() {
    let t = run(song(&[0x00, 0x90, 0x3C, 0x64]), false, 3);
    assert!(!t[1].is_empty());
    assert!(t[2].is_empty() && t[3].is_empty(), "no key-off at the end");
}

#[test]
fn notes_are_dropped_when_every_voice_sounds() {
    // Rhythm on: 6 melodic voices. Seven notes on channel 0; the seventh is lost.
    let mut ev = vec![0x00, 0xB0, 0x67, 0x01];
    for n in 0..7u8 {
        ev.extend([0x01, 0x90, 0x30 + n, 0x64]);
    }
    let t = run(song(&ev), false, 8);
    assert!(
        t[7].iter().any(|&(r, _)| r == 0xB5),
        "sixth note on voice 5"
    );
    assert!(t[8].is_empty(), "seventh note dropped, nothing stolen");
}

#[test]
fn a_silent_voice_of_another_channel_is_taken_over() {
    let mut ev = vec![0x00, 0xB0, 0x67, 0x01];
    // Channel 0 fills voices 0..5, then releases voice 2's note.
    for n in 0..6u8 {
        ev.extend([0x00, 0x90, 0x30 + n, 0x64]);
    }
    ev.extend([0x01, 0x90, 0x32, 0x00]); // t2: off on voice 2
    ev.extend([0x01, 0xC1, 0x01, 0x00, 0x91, 0x40, 0x64]); // t3: ch1 program 1, note on
    let t = run(song(&ev), false, 3);
    let expected = concat(&[
        inst_writes(2, &I1),
        regs(&[(0xA2, 0xB0), (0xB2, 0x31), (0x45, 0x11)]),
    ]);
    assert_eq!(t[3], expected);
}

#[test]
fn stop_writes_key_offs() {
    let mut w = Vec::new();
    let mut d = CmfDriver::new(&mut w);
    d.set_looping(false);
    d.start(song(&[0x00, 0xB0, 0x67, 0x00]), &mut w);
    d.tick(&mut w);
    w.clear();
    d.stop(&mut w);
    let mut expected: Vec<(u8, u8)> = (0..9).map(|v| (0xA0 + v, 0)).collect();
    expected.extend((0..9).map(|v| (0xB0 + v, 0)));
    assert_eq!(w, expected);
    assert!(!d.playing());

    d.start(song(&[0x00, 0xB0, 0x67, 0x01]), &mut w);
    d.tick(&mut w);
    w.clear();
    d.stop(&mut w);
    let mut expected = vec![(0xBD, 0xC0), (0x08, 0x00)];
    expected.extend((0..6).map(|v| (0xA0 + v, 0)));
    expected.extend((0..6).map(|v| (0xB0 + v, 0)));
    assert_eq!(w, expected, "rhythm mode: melodic voices only");
}

#[test]
fn state_carries_over_between_songs() {
    let mut w = Vec::new();
    let mut d = CmfDriver::new(&mut w);
    d.set_looping(false);
    // Song A: instruments I1, I0; rhythm on (GM-drum flag); channel 1 volume 64.
    let a = cmf(
        &[I1, I0],
        &[
            0x00, 0xB0, 0x67, 0x01, 0x00, 0xB1, 0x07, 0x40, 0x00, 0xFF, 0x2F, 0x00,
        ],
    );
    d.start(a, &mut w);
    d.tick(&mut w);
    d.stop(&mut w);
    // Song B: one instrument (I0), rhythm off; plays channel 1 with program 1.
    let b = cmf(
        &[I0],
        &[
            0x00, 0xB0, 0x67, 0x00, 0x00, 0xC1, 0x01, 0x00, 0x91, 0x45, 0x7F, 0x00, 0xFF, 0x2F,
            0x00,
        ],
    );
    w.clear();
    d.start(b, &mut w);
    // The start block uses song A's entry 0 (I1).
    let mut start = vec![(0x00, 0x00), (0xBD, 0xC0)];
    for v in 0..9 {
        start.extend(inst_writes(v, &I1));
    }
    assert_eq!(w, start);
    w.clear();
    d.tick(&mut w);
    // Program 1 is song A's leftover entry (I0); channel 1 keeps volume 64 (level 0x21).
    let mut expected = vec![(0xBD, 0xC0)];
    expected.extend(inst_writes(0, &I0));
    expected.extend([(0xA0, 0x41), (0xB0, 0x32), (0x43, 0x21)]);
    assert_eq!(w, expected);
}

#[test]
fn gm_drum_flag_suppresses_key_on_for_channels_9_and_up() {
    let mut w = Vec::new();
    let mut d = CmfDriver::new(&mut w);
    d.set_looping(false);
    // Song A turns rhythm mode (and the GM-drum flag) on; song B does not touch it.
    d.start(song(&[0x00, 0xB0, 0x67, 0x01]), &mut w);
    d.tick(&mut w);
    d.stop(&mut w);
    d.start(song(&[0x00, 0x9A, 0x45, 0x7F]), &mut w);
    w.clear();
    d.tick(&mut w);
    assert!(
        w.contains(&(0xB0, 0x12)),
        "channel 10, no key-on bit: {w:x?}"
    );
}

/// cmf rev 3 §4.4, §6.2 C2: tick 1 runs at the start (U-401), tick k (k − 1) periods later.
#[test]
fn player_ticks_at_the_pit_rate() {
    let s = song(&[0x00, 0x90, 0x3C, 0x64, 0x60, 0x90, 0x3C, 0x00]); // on at t1, off at t97
    let mut p = CmfPlayer::new_traced(s, false);
    let start = p.take_trace();
    assert!(start.iter().all(|w| w.sample == 0));
    // The latched interrupt: tick 1 is due at once.
    assert_eq!(p.samples_until_tick(), 0);
    let mut buf = vec![0i32; 518 * 100];
    p.render(&mut buf);
    let t = p.take_trace();
    let key_on = t
        .iter()
        .find(|w| w.reg == 0xB0 && w.value & 0x20 != 0)
        .unwrap();
    assert_eq!(key_on.sample, 0, "tick 1 runs before the first sample");
    let key_off = t
        .iter()
        .find(|w| w.reg == 0xB0 && w.value & 0x20 == 0)
        .unwrap();
    // 12428 / 1193180 s at 3579545 / 72 Hz: 517.834 samples per tick, so tick 97 comes
    // 96 × 517.834 = 49712.1 samples after the start, before sample 49713.
    assert_eq!(key_off.sample, 49_713);
    assert!(buf.iter().any(|&x| x.abs() > 500), "audible");
    assert!(p.finished());
}

#[test]
fn render_runs_ticks_due_at_the_end_of_the_buffer() {
    let s = song(&[0x01, 0x90, 0x3C, 0x64]); // note on at tick 2
    let mut p = CmfPlayer::new_traced(s, false);
    p.take_trace();
    p.render(&mut []);
    assert!(p.take_trace().is_empty(), "tick 1 ran (nothing to do)");
    let n = p.samples_until_tick() as usize;
    assert_eq!(n, 518);
    p.render(&mut vec![0; n]);
    assert!(!p.take_trace().is_empty(), "tick 2 ran");
    assert_eq!(p.position(), n as u64);
}

/// cmf rev 3 §4.4, §4.8 (C4): a looping song's first pass is as long as the later ones, L =
/// the sum of its delta times (here 5 ticks).
#[test]
fn every_pass_of_a_looping_song_lasts_l_ticks() {
    let s = song(&[0x03, 0x90, 0x3C, 0x64, 0x02, 0x90, 0x3C, 0x00]);
    let mut p = CmfPlayer::new_traced(s, true);
    p.take_trace();
    let mut restarts = Vec::new();
    for tick in 1..=16u32 {
        let n = p.samples_until_tick() as usize;
        p.render(&mut vec![0; n]);
        if p.take_trace()
            .iter()
            .any(|w| (w.reg, w.value) == (0xBD, 0xC0))
        {
            restarts.push(tick);
        }
    }
    // Restarts at ticks 6, 11, 16: the start is "tick 1", so each pass is 5 periods long.
    assert_eq!(restarts, [6, 11, 16]);
}
