//! Creative Music File playback as the WORX Toolkit 2.1 driver does it, on an emulated OPL2.
//!
//! Spec: `docs/spec/audio/cmf.md` rev 3. Fidelity: T1 for the OPL register-write stream (the
//! port reproduces it write for write, tick for tick), T2 for the audio (ymfm, not real chips).
//!
//! Song start timing (cmf rev 3 §4.3, §4.4, U-401): the game's start call runs with interrupts
//! off for longer than a timer period, so a timer interrupt is latched and runs the song's
//! first tick as the call returns; [`CmfPlayer::start`] therefore runs tick 1 at the moment of
//! the start, and tick k comes (k − 1) periods later. A loop restart happens inside a tick and
//! adds no tick of its own, so every pass of a looping song, the first included, lasts L ticks.
//!
//! [`CmfDriver`] is the music driver's state machine: song start (§4.3), the per-tick event
//! interpreter (§4.4), notes and voice allocation (§4.5), instrument writes (§4.6), controllers
//! (§4.7), end of track and loop (§4.8) and stop (§4.9). It emits register writes to an
//! [`OplSink`] and keeps the cross-song state (instrument bank leftovers, channel volumes, the
//! GM-drum flag) for as long as it lives, as the original driver does for a whole session.
//! [`CmfPlayer`] runs a driver against a [`td_opl::Opl2`] at the song's PIT rate (§4.1) and
//! renders samples.
//!
//! The driver works on the parsed event list of [`td_formats::cmf::Cmf`] (absolute ticks), which
//! keeps every delta time. The parser drops a few things WORX would act on; none of them occurs in
//! the 17 shipped songs (checked: they hold only 9x, Bx 67h/69h, Cx and the end of track):
//! - `Ax` events, which WORX handles like `Bx` (§4.4.2), come out as `Pressure` and are ignored;
//! - meta event 51h (tempo, §4.8) is skipped, so the PIT rate never changes within a song;
//! - SysEx (`F0`) is skipped with its length, where WORX reads no data bytes (§4.4.1).

use std::sync::Arc;

use td_formats::cmf::{Cmf, EventKind, Instrument};
use td_opl::{OPL2_CLOCK, Opl2};

/// Modulator operator offset of each voice (cmf §2).
const MOD: [u8; 9] = [0x00, 0x01, 0x02, 0x08, 0x09, 0x0A, 0x10, 0x11, 0x12];
/// Carrier operator offset of each voice (cmf §2).
const CAR: [u8; 9] = [0x03, 0x04, 0x05, 0x0B, 0x0C, 0x0D, 0x13, 0x14, 0x15];

/// F-numbers by semitone, starting at C sharp (cmf §3.3, `file0001:2C37:2F56`).
const FNUM: [u16; 12] = [
    0x16B, 0x181, 0x198, 0x1B0, 0x1CA, 0x1E5, 0x202, 0x220, 0x241, 0x263, 0x287, 0x2AE,
];

/// Global note offset: −12 as a byte, added with 8-bit wrap (cmf §3.4, `2C37:31AB`).
const NOTE_OFFSET: u8 = 0xF4;
/// Initial volume of every channel; never reset by a song start (cmf §3.4, `2C37:2FBF`).
const DEFAULT_VOLUME: u8 = 127;
/// F-number units per 128/128 semitone of transpose (cmf §3.4, `2C37:2F80`).
const TRANSPOSE_STEP: u16 = 22;
/// PIT input clock constant WORX divides by the song clock (cmf §3.4, `2C37:47F2`). The tick
/// rate is `PIT_CLOCK / divisor` (cmf §4.10).
pub const PIT_CLOCK: u32 = 1_193_180;
/// PIT divisor WORX installs before any song plays (500 Hz; `docs/re/worx.md`, `WORX_Init`).
/// Ticks do nothing while no song plays, so it only sets the phase of idle ticks.
const IDLE_DIVISOR: u16 = 0x0952;

/// `voice_owner` / `voice_program` value of an unowned voice (cmf §4.2).
const FREE: u8 = 0xFF;
/// Owner of voices 6..8 while rhythm mode is on: matches no channel (cmf §4.7, controller 67h).
const RHYTHM_OWNER: u8 = 0x10;
/// Register BDh: AM and vibrato depth bits, always set (cmf §4.7).
const BD_DEPTH: u8 = 0xC0;
/// Register BDh: rhythm mode.
const BD_RHYTHM: u8 = 0x20;
/// The channel WORX's stop function passes as "all notes off" (its own function number; §4.9).
const STOP_CHANNEL: u8 = 0x0D;

/// Bank entry 0 before the first song: a WORX built-in instrument (cmf §3.2,
/// `file0001:2C37:291B`). Bytes in instrument-record order.
const BUILTIN_INSTRUMENT: [u8; 11] = [
    0x01, 0x01, 0x4F, 0x12, 0xF1, 0xD3, 0x50, 0x7C, 0x00, 0x00, 0x06,
];
/// Bank entries. Program numbers are bytes, so 256 entries cover every index the event stream
/// can produce. Entries 1.. hold zeros until a song loads them: WORX initialises them with a
/// default bank that the spec does not give (cmf §3.2); no shipped song reads past its own bank.
const BANK_SIZE: usize = 256;

/// Rhythm-mode initialisation writes, in order (cmf §3.5, `2C37:2EC3`).
#[rustfmt::skip]
const RHYTHM_INIT: [(u8, u8); 59] = [
    (0xBD, 0xE0), (0x08, 0x00), (0xA6, 0x38), (0xB6, 0x09), (0x50, 0x0B), (0xC6, 0x00),
    (0x70, 0xA8), (0x90, 0x4C), (0x30, 0x00), (0xF0, 0x00), (0x53, 0x04), (0x73, 0xD6),
    (0x93, 0x4F), (0x33, 0x00), (0xF3, 0x00),
    (0xBD, 0xE0), (0x08, 0x00), (0xBD, 0xE0), (0x08, 0x00), (0xA7, 0x03), (0xB7, 0x0A),
    (0x51, 0x0D), (0xC7, 0x01), (0x71, 0xFA), (0x91, 0x8C), (0x31, 0x01), (0xF1, 0x00),
    (0xBD, 0xE0), (0x08, 0x00),
    (0xBD, 0xE0), (0x08, 0x00), (0xA8, 0x57), (0xB8, 0x09), (0x52, 0x00), (0xC8, 0x01),
    (0x72, 0xF7), (0x92, 0xB5), (0x32, 0x04), (0xF2, 0x00), (0xBD, 0xE0), (0x08, 0x00),
    (0xBD, 0xE0), (0x08, 0x00), (0x54, 0x07), (0x74, 0xF8), (0x94, 0x47), (0x34, 0x0C),
    (0xF4, 0x00), (0xBD, 0xE0), (0x08, 0x00),
    (0xBD, 0xE0), (0x08, 0x00), (0x55, 0x0D), (0x75, 0xF5), (0x95, 0xA5), (0x35, 0x01),
    (0xF5, 0x00), (0xBD, 0xE0), (0x08, 0x00),
];

/// Receives OPL register writes (index port 388h, then data port 389h).
pub trait OplSink {
    fn write(&mut self, reg: u8, value: u8);
}

impl OplSink for Vec<(u8, u8)> {
    fn write(&mut self, reg: u8, value: u8) {
        self.push((reg, value));
    }
}

impl OplSink for Opl2 {
    fn write(&mut self, reg: u8, value: u8) {
        self.write_reg(reg, value);
    }
}

/// The 11 bytes of an instrument record that WORX keeps (cmf §3.2), in file order.
fn record(i: &Instrument) -> [u8; 11] {
    let (m, c) = (&i.modulator, &i.carrier);
    [
        m.char_20,
        c.char_20,
        m.level_40,
        c.level_40,
        m.attack_decay_60,
        c.attack_decay_60,
        m.sustain_release_80,
        c.sustain_release_80,
        m.wave_e0,
        c.wave_e0,
        i.feedback_connection,
    ]
}

/// The WORX music driver (cmf §4.2): one per session, songs come and go.
#[derive(Debug, Clone)]
pub struct CmfDriver {
    bank: [[u8; 11]; BANK_SIZE],
    /// Shadow of register BDh.
    bd: u8,
    gm_drums: bool,
    looping: bool,
    playing: bool,
    song: Option<Arc<Cmf>>,
    /// Index of the next event; `song.events.len()` stands for the end of track.
    next_event: usize,
    delay: u32,
    divisor: u16,
    chan_program: [u8; 16],
    chan_transpose: [u8; 16],
    chan_volume: [u8; 16],
    voice_owner: [u8; 9],
    voice_program: [u8; 9],
    voice_note: [u8; 9],
    voice_level: [u8; 9],
}

impl CmfDriver {
    /// Driver start-up: enables wave select (`01=20`, cmf §4.6). Looping is on (§3.4).
    pub fn new(out: &mut impl OplSink) -> Self {
        let mut bank = [[0; 11]; BANK_SIZE];
        bank[0] = BUILTIN_INSTRUMENT;
        out.write(0x01, 0x20);
        Self {
            bank,
            bd: BD_DEPTH,
            gm_drums: false,
            looping: true,
            playing: false,
            song: None,
            next_event: 0,
            delay: 0,
            divisor: IDLE_DIVISOR,
            chan_program: [0; 16],
            chan_transpose: [0; 16],
            chan_volume: [DEFAULT_VOLUME; 16],
            voice_owner: [FREE; 9],
            voice_program: [FREE; 9],
            voice_note: [0; 9],
            voice_level: [0; 9],
        }
    }

    /// True while a song plays (between [`Self::start`] and its end, or [`Self::stop`]).
    pub fn playing(&self) -> bool {
        self.playing
    }

    /// The loop flag (WORX function 1Dh, cmf §4.9): when set, the end of track restarts the
    /// song. Read at the end of track, so it may change while a song plays.
    pub fn set_looping(&mut self, looping: bool) {
        self.looping = looping;
    }

    /// Current loop flag.
    pub fn looping(&self) -> bool {
        self.looping
    }

    /// PIT divisor of the tick interrupt: `PIT_CLOCK div clock` once a song has started (§4.1).
    pub fn pit_divisor(&self) -> u16 {
        self.divisor
    }

    /// Song start (WORX function 1Eh, cmf §4.3). Does not stop the previous song first; the
    /// game always calls [`Self::stop`] before (music-cues §4.1).
    pub fn start(&mut self, song: Arc<Cmf>, out: &mut impl OplSink) {
        out.write(0x00, 0x00);
        self.bd = BD_DEPTH;
        out.write(0xBD, self.bd);
        // Entry 0 of the bank as it is now: the previous song's (or the built-in) instrument.
        let first = self.bank[0];
        for v in 0..9 {
            self.write_instrument(v, &first, out);
        }
        self.chan_program = [0; 16];
        self.chan_transpose = [0; 16];
        self.voice_note = [0; 9];
        self.voice_owner = [FREE; 9];
        self.voice_program = [FREE; 9];
        // The 8086 divide would fault for a clock below 19 Hz; the parser rejects 0. Saturate.
        let divisor = PIT_CLOCK / u32::from(song.ticks_per_second.max(1));
        self.divisor = u16::try_from(divisor).unwrap_or(u16::MAX);
        for (slot, inst) in self.bank.iter_mut().zip(&song.instruments) {
            *slot = record(inst);
        }
        self.next_event = 0;
        self.delay = song.events.first().map_or(song.end_tick, |e| e.tick);
        self.song = Some(song);
        self.playing = true;
    }

    /// Stop (WORX function 0Dh, cmf §4.9). `bd` keeps its value.
    pub fn stop(&mut self, out: &mut impl OplSink) {
        self.all_notes_off(STOP_CHANNEL, out);
        self.playing = false;
        let voices: u8 = if self.bd & 0xE0 == 0xE0 {
            out.write(0xBD, BD_DEPTH);
            out.write(0x08, 0x00);
            6
        } else {
            9
        };
        for v in 0..voices {
            out.write(0xA0 + v, 0x00);
        }
        for v in 0..voices {
            out.write(0xB0 + v, 0x00);
        }
    }

    /// One PIT interrupt (cmf §4.4). Events with delta `d` after an event at tick T run at tick
    /// T + d; the first event of a song runs at tick `d0 + 1`, counting from tick 1 = the tick
    /// [`CmfPlayer::start`] runs at the start itself.
    pub fn tick(&mut self, out: &mut impl OplSink) {
        if !self.playing {
            return;
        }
        let Some(song) = self.song.clone() else {
            return;
        };
        loop {
            if self.delay > 0 {
                self.delay -= 1;
                return;
            }
            let Some(ev) = song.events.get(self.next_event) else {
                // End of track (§4.8).
                if !self.looping {
                    self.playing = false;
                    return;
                }
                self.start(Arc::clone(&song), out);
                if song.end_tick == 0 {
                    // A zero-length song would restart forever within one tick (the original
                    // would hang in its interrupt handler); give the next tick a turn.
                    return;
                }
                continue;
            };
            self.dispatch(ev.kind, out);
            self.next_event += 1;
            let next = song
                .events
                .get(self.next_event)
                .map_or(song.end_tick, |e| e.tick);
            self.delay = next.saturating_sub(ev.tick);
        }
    }

    /// Event dispatch (cmf §4.4.2).
    fn dispatch(&mut self, kind: EventKind, out: &mut impl OplSink) {
        match kind {
            EventKind::NoteOn { channel, note, .. } => self.note_event(channel, note, true, out),
            EventKind::NoteOff { channel, note } => self.note_event(channel, note, false, out),
            EventKind::Controller {
                channel,
                controller,
                value,
            } => self.controller(channel, controller, value, out),
            EventKind::Program { channel, program } => {
                self.chan_program[usize::from(channel & 0x0F)] = program;
            }
            // No channel pressure, no pitch bend.
            EventKind::PitchBend { .. } | EventKind::Pressure { .. } => {}
        }
    }

    fn rhythm(&self) -> bool {
        self.bd & BD_RHYTHM != 0
    }

    /// Note on or off (cmf §4.5). `on` is false for status 8x and for 9x with velocity 0.
    /// Velocity is ignored (U-034).
    fn note_event(&mut self, ch: u8, raw_note: u8, on: bool, out: &mut impl OplSink) {
        let ch = ch & 0x0F;
        let rhythm = self.rhythm();
        if rhythm && ch >= 11 {
            // CMF percussion: 11 BD, 12 SD, 13 TT, 14 CY, 15 HH. Note-on and note-off both
            // toggle the bit.
            self.bd ^= 0x10 >> (ch - 11);
            out.write(0xBD, self.bd);
            return;
        }
        if rhythm && self.gm_drums && ch == 9 {
            // GM drum channel (§4.7): toggles a drum bit chosen through the note table at
            // `2C37:2E9B`, which the spec does not give. No shipped song uses channel 9.
            return;
        }
        let note = raw_note.wrapping_add(NOTE_OFFSET);
        let nv = if rhythm { 6 } else { 9 };
        if !on {
            let found = (0..nv).find(|&v| self.voice_owner[v] == ch && self.voice_note[v] == note);
            if let Some(v) = found {
                self.output(v, ch, note, false, out);
                self.voice_note[v] = 0;
            }
            return;
        }
        let Some(v) = self.allocate(ch, nv) else {
            return; // all voices sound: the note is lost (no stealing)
        };
        let program = self.chan_program[usize::from(ch)];
        if self.voice_program[v] != program {
            self.voice_owner[v] = ch;
            self.voice_program[v] = program;
            let inst = self.bank[usize::from(program)];
            self.write_instrument(v, &inst, out);
        }
        self.output(v, ch, note, true, out);
    }

    /// Voice allocation (cmf §4.5): own silent voice, then a never-used one, then any silent
    /// voice (released from its owner), else none.
    fn allocate(&mut self, ch: u8, nv: usize) -> Option<usize> {
        let silent = |d: &Self, v: usize| d.voice_note[v] == 0;
        if let Some(v) = (0..nv).find(|&v| self.voice_owner[v] == ch && silent(self, v)) {
            return Some(v);
        }
        if let Some(v) = (0..nv).find(|&v| self.voice_owner[v] == FREE && silent(self, v)) {
            return Some(v);
        }
        let v = (0..nv).find(|&v| silent(self, v))?;
        self.voice_owner[v] = FREE;
        self.voice_program[v] = FREE;
        Some(v)
    }

    /// Frequency, key and carrier level of voice `v` (cmf §4.5 `output`).
    fn output(&mut self, v: usize, ch: u8, note: u8, key_on: bool, out: &mut impl OplSink) {
        self.voice_note[v] = note;
        let k = note.wrapping_sub(1);
        let block = k / 12;
        let mut f = FNUM[usize::from(k % 12)];
        let t = self.chan_transpose[usize::from(ch)];
        if t != 0 {
            let a = u16::from(t) * TRANSPOSE_STEP / 128;
            f = if t == 0x80 {
                f.wrapping_add(a)
            } else {
                f.wrapping_sub(a)
            };
        }
        let vr = v as u8;
        out.write(0xA0 + vr, (f & 0xFF) as u8);
        let key = if key_on && !(self.gm_drums && ch >= 9) {
            0x20
        } else {
            0
        };
        out.write(
            0xB0 + vr,
            block.wrapping_mul(4) | key | ((f >> 8) & 3) as u8,
        );
        let loud = 63 - (self.voice_level[v] % 64);
        let scaled = u16::from(loud) * u16::from(self.chan_volume[usize::from(ch)]) / 256;
        let level = 63u8.wrapping_sub((scaled as u8).wrapping_mul(2));
        out.write(0x40 + CAR[v], level);
    }

    /// Instrument write (cmf §4.6).
    fn write_instrument(&mut self, v: usize, b: &[u8; 11], out: &mut impl OplSink) {
        let (m, c) = (MOD[v], CAR[v]);
        for (i, base) in [0x20u8, 0x40, 0x60, 0x80, 0xE0].into_iter().enumerate() {
            out.write(base + m, b[2 * i]);
            out.write(base + c, b[2 * i + 1]);
        }
        out.write(0xC0 + v as u8, b[10]);
        self.voice_level[v] = b[3];
    }

    /// Controllers (cmf §4.7).
    fn controller(&mut self, ch: u8, controller: u8, value: u8, out: &mut impl OplSink) {
        let ch = ch & 0x0F;
        let c = usize::from(ch);
        match controller {
            // Volume. (With the GM-drum flag on channel 9 WORX also rescales the drum operator
            // levels; not specified, and unused by the shipped songs.)
            0x07 => self.chan_volume[c] = value,
            0x67 => match value {
                0 => {
                    self.gm_drums = false;
                    self.bd = BD_DEPTH;
                    out.write(0xBD, self.bd);
                    self.voice_owner[6..].fill(FREE);
                    self.voice_program[6..].fill(FREE);
                }
                1 => {
                    self.gm_drums = true;
                    self.bd = BD_DEPTH | BD_RHYTHM;
                    out.write(0xBD, self.bd);
                    self.voice_owner[6..].fill(RHYTHM_OWNER);
                    self.voice_program[6..].fill(FREE);
                    for (reg, val) in RHYTHM_INIT {
                        out.write(reg, val);
                    }
                    out.write(0xBD, self.bd);
                    out.write(0x08, 0x00);
                }
                _ => {}
            },
            0x68 => self.chan_transpose[c] = value | 0x80,
            0x69 => self.chan_transpose[c] = value,
            0x7B => self.all_notes_off(ch, out),
            // Including 63h (depth: BDh keeps C0h) and 66h (marker).
            _ => {}
        }
    }

    /// Controller 7Bh: silence every voice owned by `ch`.
    fn all_notes_off(&mut self, ch: u8, out: &mut impl OplSink) {
        for v in 0..9 {
            if self.voice_owner[v] == ch {
                out.write(0xA0 + v as u8, 0x00);
                out.write(0xB0 + v as u8, 0x00);
                self.voice_note[v] = 0;
            }
        }
    }
}

/// One OPL register write, stamped with the native-sample time at which it happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OplWrite {
    pub sample: u64,
    pub reg: u8,
    pub value: u8,
}

/// The emulated chip, with the sample clock and the optional write trace.
#[derive(Debug)]
struct Chip {
    opl: Opl2,
    samples: u64,
    trace: Option<Vec<OplWrite>>,
}

impl OplSink for Chip {
    fn write(&mut self, reg: u8, value: u8) {
        if let Some(t) = self.trace.as_mut() {
            t.push(OplWrite {
                sample: self.samples,
                reg,
                value,
            });
        }
        self.opl.write_reg(reg, value);
    }
}

/// Duration of one OPL sample (72 chip clocks) in units of 1 / (OPL2_CLOCK × PIT_CLOCK) s.
const SAMPLE_LEN: u64 = 72 * PIT_CLOCK as u64;

/// A [`CmfDriver`] driving an OPL2 at the song's PIT rate, producing mono samples at the chip's
/// native rate. Keeps the driver (and its cross-song state) across songs.
#[derive(Debug)]
pub struct CmfPlayer {
    driver: CmfDriver,
    chip: Chip,
    /// Time since the PIT was last programmed, in units of 1 / (OPL2_CLOCK × PIT_CLOCK) s.
    phase: u64,
}

impl CmfPlayer {
    /// Starts the driver and plays `song` from the beginning.
    pub fn new(song: Arc<Cmf>, looping: bool) -> Self {
        let mut p = Self::idle();
        p.set_looping(looping);
        p.start(song);
        p
    }

    /// Like [`Self::new`], but records every register write (see [`Self::take_trace`]).
    pub fn new_traced(song: Arc<Cmf>, looping: bool) -> Self {
        let mut p = Self::build(true);
        p.set_looping(looping);
        p.start(song);
        p
    }

    /// A started driver with no song.
    pub fn idle() -> Self {
        Self::build(false)
    }

    fn build(traced: bool) -> Self {
        let mut chip = Chip {
            opl: Opl2::new(),
            samples: 0,
            trace: traced.then(Vec::new),
        };
        let driver = CmfDriver::new(&mut chip);
        Self {
            driver,
            chip,
            phase: 0,
        }
    }

    /// A new program starts its own WORX driver on the same card, as when the intro program
    /// hands over to the game: the driver returns to its start-up state (built-in bank entry 0,
    /// channel volumes 127, GM-drum flag off, looping on, no song), the chip keeps its registers.
    pub fn restart_driver(&mut self) {
        self.driver = CmfDriver::new(&mut self.chip);
    }

    /// Returns and clears the recorded register writes.
    pub fn take_trace(&mut self) -> Vec<OplWrite> {
        self.chip
            .trace
            .as_mut()
            .map(std::mem::take)
            .unwrap_or_default()
    }

    /// Native output sample rate (OPL2 clock / 72, about 49,716 Hz).
    pub fn sample_rate(&self) -> u32 {
        self.chip.opl.sample_rate()
    }

    /// Samples rendered so far (the time base of the trace).
    pub fn position(&self) -> u64 {
        self.chip.samples
    }

    /// True while a song plays.
    pub fn playing(&self) -> bool {
        self.driver.playing()
    }

    /// True when no song plays: a non-looping song has reached its end, or it was stopped.
    pub fn finished(&self) -> bool {
        !self.driver.playing()
    }

    /// Sets the loop flag (WORX function 1Dh).
    pub fn set_looping(&mut self, looping: bool) {
        self.driver.set_looping(looping);
    }

    /// What the game's `Snd_PlayMusic` does (music-cues §4.1): set the loop flag, stop the
    /// current song, start `song` from the top (even when it is the same song).
    pub fn play(&mut self, song: Arc<Cmf>, looping: bool) {
        self.set_looping(looping);
        self.stop();
        self.start(song);
    }

    /// Song start alone (WORX function 1Eh), as the game calls it (cmf rev 3 §4.3, U-401): the
    /// timer interrupt latched during the call runs tick 1 at once, before the next sample;
    /// the PIT write restarts the period, so tick 2 comes one full period later.
    pub fn start(&mut self, song: Arc<Cmf>) {
        self.driver.start(song, &mut self.chip);
        // A full period has "elapsed": the next render runs the latched tick before any sample.
        self.phase = self.tick_len();
    }

    /// Stops the song (WORX function 0Dh): key-offs, then silence on the next ticks. Notes
    /// release naturally.
    pub fn stop(&mut self) {
        self.driver.stop(&mut self.chip);
    }

    fn tick_len(&self) -> u64 {
        u64::from(self.driver.pit_divisor()) * u64::from(OPL2_CLOCK)
    }

    /// Samples to render before the next tick fires (0 if it is due now).
    pub fn samples_until_tick(&self) -> u64 {
        self.tick_len()
            .saturating_sub(self.phase)
            .div_ceil(SAMPLE_LEN)
    }

    fn run_due_ticks(&mut self) {
        loop {
            let len = self.tick_len();
            if self.phase < len {
                return;
            }
            self.phase -= len;
            self.driver.tick(&mut self.chip);
        }
    }

    /// Fills `out` with mono samples (signed 16-bit range) at [`Self::sample_rate`]. Ticks due
    /// at a sample boundary run before that sample, including at the end of `out`.
    pub fn render(&mut self, out: &mut [i32]) {
        let mut done = 0;
        loop {
            self.run_due_ticks();
            if done == out.len() {
                return;
            }
            let until = usize::try_from(self.samples_until_tick()).unwrap_or(usize::MAX);
            let n = until.clamp(1, out.len() - done);
            self.chip.opl.generate(&mut out[done..done + n]);
            done += n;
            self.chip.samples += n as u64;
            self.phase += n as u64 * SAMPLE_LEN;
        }
    }
}

#[cfg(test)]
mod tests;
