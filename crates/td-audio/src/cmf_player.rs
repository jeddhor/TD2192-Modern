//! Creative Music File sequencer driving an emulated OPL2.
//!
//! Spec: `docs/spec/audio/cmf.md` (rev 1). Fidelity: T2, pending WORX findings (U-007 tempo,
//! U-034 velocity, loop behaviour).

use std::sync::Arc;

use td_formats::cmf::{Cmf, EventKind, Instrument, OperatorRegs};
use td_opl::Opl2;

/// Operator register offsets of the two operators of each melodic voice.
const VOICE_OPS: [(u8, u8); 9] = [
    (0x00, 0x03),
    (0x01, 0x04),
    (0x02, 0x05),
    (0x08, 0x0B),
    (0x09, 0x0C),
    (0x0A, 0x0D),
    (0x10, 0x13),
    (0x11, 0x14),
    (0x12, 0x15),
];

/// Rhythm-mode percussion instruments, mapped from MIDI channels 11..=15.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Drum {
    Bass,
    Snare,
    Tom,
    Cymbal,
    HiHat,
}

impl Drum {
    fn from_channel(ch: u8) -> Option<Self> {
        Some(match ch {
            11 => Drum::Bass,
            12 => Drum::Snare,
            13 => Drum::Tom,
            14 => Drum::Cymbal,
            15 => Drum::HiHat,
            _ => return None,
        })
    }

    /// Bit in register 0xBD.
    fn bit(self) -> u8 {
        match self {
            Drum::Bass => 0x10,
            Drum::Snare => 0x08,
            Drum::Tom => 0x04,
            Drum::Cymbal => 0x02,
            Drum::HiHat => 0x01,
        }
    }

    /// The voice whose frequency registers set this drum's pitch.
    fn pitch_voice(self) -> u8 {
        match self {
            Drum::Bass => 6,
            Drum::Snare | Drum::HiHat => 7,
            Drum::Tom | Drum::Cymbal => 8,
        }
    }
}

/// Plays one CMF song on an OPL2, producing mono samples at the chip's native rate.
#[derive(Debug)]
pub struct CmfPlayer {
    song: Arc<Cmf>,
    opl: Opl2,
    looping: bool,
    finished: bool,
    next_event: usize,
    tick: u32,
    /// Native samples until the next sequencer tick, in 1/ticks_per_second units.
    tick_accum: u64,
    rhythm: bool,
    bd_bits: u8,
    transpose: [i32; 16],
    program: [u8; 16],
    /// Note currently sounding on each melodic voice.
    voice_note: [Option<u8>; 9],
    /// Last Bx value written per voice (to key off without changing the frequency).
    voice_bx: [u8; 9],
    /// OPL voice assigned to each MIDI channel on its first note (dynamic allocation).
    chan_voice: [Option<u8>; 16],
    /// Program currently loaded into each voice's operators.
    voice_program: [Option<u8>; 9],
    /// Native samples rendered so far (the time base of the trace).
    samples: u64,
    /// Register writes, when tracing (for comparison with oracle OPL captures).
    trace: Option<Vec<OplWrite>>,
}

/// One OPL register write, stamped with the native-sample time at which it happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OplWrite {
    pub sample: u64,
    pub reg: u8,
    pub value: u8,
}

impl CmfPlayer {
    /// Starts `song` from the beginning.
    pub fn new(song: Arc<Cmf>, looping: bool) -> Self {
        Self::build(song, looping, false)
    }

    /// Like [`Self::new`], but records every register write (see [`Self::take_trace`]).
    pub fn new_traced(song: Arc<Cmf>, looping: bool) -> Self {
        Self::build(song, looping, true)
    }

    /// Returns and clears the recorded register writes.
    pub fn take_trace(&mut self) -> Vec<OplWrite> {
        self.trace.as_mut().map(std::mem::take).unwrap_or_default()
    }

    fn build(song: Arc<Cmf>, looping: bool, traced: bool) -> Self {
        let mut p = Self {
            song,
            opl: Opl2::new(),
            looping,
            finished: false,
            next_event: 0,
            tick: 0,
            tick_accum: 0,
            rhythm: false,
            bd_bits: 0,
            transpose: [0; 16],
            program: [0; 16],
            voice_note: [None; 9],
            voice_bx: [0; 9],
            chan_voice: [None; 16],
            voice_program: [None; 9],
            samples: 0,
            trace: traced.then(Vec::new),
        };
        p.reset_chip();
        p.advance_tick(); // events at tick 0 take effect before the first sample
        p
    }

    /// Native output sample rate (about 49,716 Hz).
    pub fn sample_rate(&self) -> u32 {
        self.opl.sample_rate()
    }

    /// True once a non-looping song has played its end-of-track.
    pub fn finished(&self) -> bool {
        self.finished
    }

    /// Fills `out` with mono samples (signed 16-bit range) at [`Self::sample_rate`].
    pub fn render(&mut self, out: &mut [i32]) {
        let rate = u64::from(self.sample_rate());
        let tps = u64::from(self.song.ticks_per_second);
        let mut done = 0;
        while done < out.len() {
            // Samples until the next tick boundary.
            let until_tick = (rate.saturating_sub(self.tick_accum)).div_ceil(tps).max(1) as usize;
            let n = until_tick.min(out.len() - done);
            self.opl.generate(&mut out[done..done + n]);
            done += n;
            self.samples += n as u64;
            self.tick_accum += n as u64 * tps;
            while self.tick_accum >= rate {
                self.tick_accum -= rate;
                self.advance_tick();
            }
        }
    }

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

    fn reset_chip(&mut self) {
        self.opl.reset();
        self.write(0x01, 0x20);
        self.write(0xBD, BD_DEPTH);
        self.rhythm = false;
        self.bd_bits = 0;
        self.voice_note = [None; 9];
        self.voice_bx = [0; 9];
    }

    fn advance_tick(&mut self) {
        if self.finished {
            return;
        }
        let song = Arc::clone(&self.song);
        while let Some(ev) = song.events.get(self.next_event) {
            if ev.tick > self.tick {
                break;
            }
            self.handle(ev.kind);
            self.next_event += 1;
        }
        if self.next_event >= song.events.len() && self.tick >= song.end_tick {
            if self.looping {
                self.next_event = 0;
                self.tick = 0;
                self.transpose = [0; 16];
                return;
            }
            self.all_notes_off();
            self.finished = true;
            return;
        }
        self.tick += 1;
    }

    fn handle(&mut self, kind: EventKind) {
        match kind {
            EventKind::NoteOn { channel, note, .. } => self.note_on(channel, note),
            EventKind::NoteOff { channel, note } => self.note_off(channel, note),
            EventKind::Program { channel, program } => {
                self.program[channel as usize] = program;
                if let Some(Target::Drum(..)) = self.target(channel) {
                    self.apply_program(channel);
                }
            }
            EventKind::Controller {
                channel,
                controller,
                value,
            } => match controller {
                0x63 => {
                    let depth = ((value & 2) << 6) | ((value & 1) << 6);
                    self.write_bd(depth | (self.bd_bits & 0x3F));
                }
                0x67 => {
                    self.rhythm = value != 0;
                    let bits = if self.rhythm { 0x20 } else { 0 };
                    self.write_bd((self.bd_bits & 0xC0) | bits);
                    self.chan_voice = [None; 16];
                }
                0x68 => self.transpose[channel as usize] = i32::from(value),
                0x69 => self.transpose[channel as usize] = -i32::from(value),
                _ => {}
            },
            EventKind::PitchBend { .. } | EventKind::Pressure { .. } => {}
        }
    }

    fn write_bd(&mut self, value: u8) {
        self.bd_bits = value;
        self.write(0xBD, value);
    }

    fn instrument(&self, channel: u8) -> Instrument {
        let prog = self.program[channel as usize] as usize;
        self.song.instruments.get(prog).copied().unwrap_or_default()
    }

    fn write_operator(&mut self, slot: u8, op: OperatorRegs) {
        self.write(0x20 + slot, op.char_20);
        self.write(0x40 + slot, op.level_40);
        self.write(0x60 + slot, op.attack_decay_60);
        self.write(0x80 + slot, op.sustain_release_80);
        self.write(0xE0 + slot, op.wave_e0);
    }

    fn apply_program(&mut self, channel: u8) {
        let inst = self.instrument(channel);
        if let Some(Target::Voice(v)) = self.target(channel) {
            self.voice_program[v as usize] = Some(self.program[channel as usize]);
        }
        match self.target(channel) {
            Some(Target::Voice(v)) | Some(Target::Drum(Drum::Bass, v)) => {
                let (m, c) = VOICE_OPS[v as usize];
                self.write_operator(m, inst.modulator);
                self.write_operator(c, inst.carrier);
                self.write(0xC0 + v, inst.feedback_connection);
            }
            Some(Target::Drum(drum, _)) => {
                let (slot, regs) = match drum {
                    Drum::Snare => (0x14, inst.carrier),
                    Drum::Cymbal => (0x15, inst.carrier),
                    Drum::Tom => (0x12, inst.modulator),
                    Drum::HiHat => (0x11, inst.modulator),
                    Drum::Bass => unreachable!("handled above"),
                };
                self.write_operator(slot, regs);
            }
            None => {}
        }
    }

    /// Where a channel plays, without allocating (None for an unassigned melodic channel).
    fn target(&self, channel: u8) -> Option<Target> {
        if self.rhythm
            && let Some(d) = Drum::from_channel(channel)
        {
            return Some(Target::Drum(d, d.pitch_voice()));
        }
        self.chan_voice[channel as usize].map(Target::Voice)
    }

    /// Like [`Self::target`], assigning the lowest free voice to a melodic channel on its
    /// first note. Observed in oracle capture C-0001 for first notes; the original is not sticky
    /// per channel later (docs/parity/M3-audio.md row 4), exact rule pending M3-05.
    fn target_alloc(&mut self, channel: u8) -> Option<Target> {
        if let Some(t) = self.target(channel) {
            return Some(t);
        }
        let voices = if self.rhythm { 6 } else { 9 };
        let free = (0..voices).find(|v| !self.chan_voice.contains(&Some(*v)))?;
        self.chan_voice[channel as usize] = Some(free);
        Some(Target::Voice(free))
    }

    fn freq_regs(&self, channel: u8, note: u8) -> (u8, u8) {
        let (fnum, block) = note_to_fnum(note, self.transpose[channel as usize]);
        ((fnum & 0xFF) as u8, (block << 2) | ((fnum >> 8) as u8 & 3))
    }

    fn note_on(&mut self, channel: u8, note: u8) {
        match self.target_alloc(channel) {
            Some(Target::Voice(v)) => {
                let vi = v as usize;
                if self.voice_program[vi] != Some(self.program[channel as usize]) {
                    self.apply_program(channel);
                }
                let (a, b) = self.freq_regs(channel, note);
                if self.voice_note[vi].is_some() {
                    self.write(0xB0 + v, self.voice_bx[vi] & !0x20);
                }
                self.write(0xA0 + v, a);
                self.voice_bx[vi] = 0x20 | b;
                self.write(0xB0 + v, self.voice_bx[vi]);
                self.voice_note[vi] = Some(note);
            }
            Some(Target::Drum(drum, pv)) => {
                let (a, b) = self.freq_regs(channel, note);
                self.write(0xA0 + pv, a);
                self.write(0xB0 + pv, b);
                let bits = self.bd_bits & !drum.bit();
                self.write_bd(bits);
                self.write_bd(bits | drum.bit());
            }
            None => {}
        }
    }

    fn note_off(&mut self, channel: u8, note: u8) {
        match self.target(channel) {
            Some(Target::Voice(v)) => {
                let vi = v as usize;
                if self.voice_note[vi] == Some(note) {
                    self.write(0xB0 + v, self.voice_bx[vi] & !0x20);
                    self.voice_note[vi] = None;
                }
            }
            Some(Target::Drum(drum, _)) => {
                let bits = self.bd_bits & !drum.bit();
                self.write_bd(bits);
            }
            None => {}
        }
    }

    fn all_notes_off(&mut self) {
        for v in 0..9u8 {
            self.write(0xB0 + v, self.voice_bx[v as usize] & !0x20);
        }
        let bits = self.bd_bits & 0xE0;
        self.write_bd(bits);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    Voice(u8),
    Drum(Drum, u8),
}

/// AM and vibrato depth bits the original driver keeps set in register 0xBD.
const BD_DEPTH: u8 = 0xC0;

/// The classic AdLib F-number table, C-sharp to C (observed in the oracle OPL capture).
const FNUM_TABLE: [u16; 12] = [
    0x16B, 0x181, 0x198, 0x1B0, 0x1CA, 0x1E5, 0x202, 0x220, 0x241, 0x263, 0x287, 0x2AE,
];

/// F-number and block for a MIDI note: `fnum = TABLE[(note - 1) % 12]`,
/// `block = (note - 1) / 12 - 1`. A fine transpose (1/128 semitone) scales the F-number;
/// ASSUMPTION(U-034 family): how the original applies controllers 0x68/0x69 is unconfirmed.
pub fn note_to_fnum(note: u8, transpose_128ths: i32) -> (u16, u8) {
    let n = i32::from(note.max(1)) - 1;
    let base = FNUM_TABLE[(n % 12) as usize];
    let block = (n / 12 - 1).clamp(0, 7) as u8;
    if transpose_128ths == 0 {
        return (base, block);
    }
    let scaled = f64::from(base) * 2f64.powf(f64::from(transpose_128ths) / 1536.0);
    (scaled.round().clamp(0.0, 1023.0) as u16, block)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_frequencies_match_spec_vectors() {
        // Values seen in the oracle OPL capture of intro1.cmf (C-0001).
        assert_eq!(note_to_fnum(65, 0), (458, 4));
        assert_eq!(note_to_fnum(68, 0), (544, 4));
        assert_eq!(note_to_fnum(72, 0), (686, 4));
        assert_eq!(note_to_fnum(37, 0), (363, 2));
        assert_eq!(note_to_fnum(77, 0), (458, 5));
        let (f_down, _) = note_to_fnum(69, -64); // half a semitone down
        assert!(f_down < 0x241);
    }
}
