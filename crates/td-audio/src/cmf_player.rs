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
}

impl CmfPlayer {
    /// Starts `song` from the beginning.
    pub fn new(song: Arc<Cmf>, looping: bool) -> Self {
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
            self.tick_accum += n as u64 * tps;
            while self.tick_accum >= rate {
                self.tick_accum -= rate;
                self.advance_tick();
            }
        }
    }

    fn reset_chip(&mut self) {
        self.opl.reset();
        self.opl.write_reg(0x01, 0x20);
        self.opl.write_reg(0xBD, 0x00);
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
                self.apply_program(channel);
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
        self.opl.write_reg(0xBD, value);
    }

    fn instrument(&self, channel: u8) -> Instrument {
        let prog = self.program[channel as usize] as usize;
        self.song.instruments.get(prog).copied().unwrap_or_default()
    }

    fn write_operator(&mut self, slot: u8, op: OperatorRegs) {
        self.opl.write_reg(0x20 + slot, op.char_20);
        self.opl.write_reg(0x40 + slot, op.level_40);
        self.opl.write_reg(0x60 + slot, op.attack_decay_60);
        self.opl.write_reg(0x80 + slot, op.sustain_release_80);
        self.opl.write_reg(0xE0 + slot, op.wave_e0);
    }

    fn apply_program(&mut self, channel: u8) {
        let inst = self.instrument(channel);
        match self.target(channel) {
            Some(Target::Voice(v)) | Some(Target::Drum(Drum::Bass, v)) => {
                let (m, c) = VOICE_OPS[v as usize];
                self.write_operator(m, inst.modulator);
                self.write_operator(c, inst.carrier);
                self.opl.write_reg(0xC0 + v, inst.feedback_connection);
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

    fn target(&self, channel: u8) -> Option<Target> {
        if self.rhythm {
            if let Some(d) = Drum::from_channel(channel) {
                return Some(Target::Drum(d, d.pitch_voice()));
            }
            (channel < 6).then_some(Target::Voice(channel))
        } else {
            (channel < 9).then_some(Target::Voice(channel))
        }
    }

    fn freq_regs(&self, channel: u8, note: u8) -> (u8, u8) {
        let (fnum, block) = note_to_fnum(note, self.transpose[channel as usize]);
        ((fnum & 0xFF) as u8, (block << 2) | ((fnum >> 8) as u8 & 3))
    }

    fn note_on(&mut self, channel: u8, note: u8) {
        match self.target(channel) {
            Some(Target::Voice(v)) => {
                let (a, b) = self.freq_regs(channel, note);
                let vi = v as usize;
                if self.voice_note[vi].is_some() {
                    self.opl.write_reg(0xB0 + v, self.voice_bx[vi] & !0x20);
                }
                self.opl.write_reg(0xA0 + v, a);
                self.voice_bx[vi] = 0x20 | b;
                self.opl.write_reg(0xB0 + v, self.voice_bx[vi]);
                self.voice_note[vi] = Some(note);
            }
            Some(Target::Drum(drum, pv)) => {
                let (a, b) = self.freq_regs(channel, note);
                self.opl.write_reg(0xA0 + pv, a);
                self.opl.write_reg(0xB0 + pv, b);
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
                    self.opl.write_reg(0xB0 + v, self.voice_bx[vi] & !0x20);
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
            self.opl
                .write_reg(0xB0 + v, self.voice_bx[v as usize] & !0x20);
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

/// OPL2 native rate for the standard clock, used for frequency numbers.
const OPL_RATE: f64 = 49_716.0;

/// F-number and block for a MIDI note with a fine transpose in 1/128 semitone.
pub fn note_to_fnum(note: u8, transpose_128ths: i32) -> (u16, u8) {
    let semis = f64::from(note) - 69.0 + f64::from(transpose_128ths) / 128.0;
    let freq = 440.0 * 2f64.powf(semis / 12.0);
    for block in 0..8u8 {
        let fnum = (freq * f64::from(1u32 << (20 - block)) / OPL_RATE).round();
        if fnum <= 1023.0 {
            return (fnum.max(0.0) as u16, block);
        }
    }
    (1023, 7)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_frequencies_match_spec_vectors() {
        assert_eq!(note_to_fnum(69, 0), (580, 4));
        let (f, b) = note_to_fnum(60, 0);
        assert_eq!(b, 3);
        assert!((689..=691).contains(&f));
        let (f_down, _) = note_to_fnum(69, -64); // half a semitone down
        assert!(f_down < 580);
    }
}
