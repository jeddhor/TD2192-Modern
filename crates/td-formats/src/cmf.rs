//! Creative Music File (CMF v1.01).
//!
//! Spec: `docs/spec/audio/cmf.md` §2 (rev 1). Fidelity: T0 for parsing.
//! The event track is decoded into a list of timed MIDI-style events; interpreting them is the
//! sequencer's job (`td-audio`).

use crate::{ByteReader, FormatError, Result};

/// One OPL2 instrument (16 bytes in the file; 11 used).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Instrument {
    /// Registers 0x20, 0x40, 0x60, 0x80, 0xE0 for the modulator.
    pub modulator: OperatorRegs,
    /// Registers 0x20, 0x40, 0x60, 0x80, 0xE0 for the carrier.
    pub carrier: OperatorRegs,
    /// Register 0xC0: feedback and connection.
    pub feedback_connection: u8,
}

/// The per-operator OPL register values of an instrument.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OperatorRegs {
    pub char_20: u8,
    pub level_40: u8,
    pub attack_decay_60: u8,
    pub sustain_release_80: u8,
    pub wave_e0: u8,
}

/// A decoded event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    NoteOff {
        channel: u8,
        note: u8,
    },
    NoteOn {
        channel: u8,
        note: u8,
        velocity: u8,
    },
    Controller {
        channel: u8,
        controller: u8,
        value: u8,
    },
    Program {
        channel: u8,
        program: u8,
    },
    PitchBend {
        channel: u8,
        value: u16,
    },
    /// Aftertouch and channel pressure (ignored by playback).
    Pressure {
        channel: u8,
    },
}

/// An event at an absolute tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Event {
    pub tick: u32,
    pub kind: EventKind,
}

/// A parsed music file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cmf {
    pub version: u16,
    pub ticks_per_quarter: u16,
    /// Playback clock in ticks per second (header field 0x0C).
    pub ticks_per_second: u16,
    pub channel_in_use: [bool; 16],
    pub basic_tempo: u16,
    pub instruments: Vec<Instrument>,
    pub events: Vec<Event>,
    /// Tick of the end-of-track event.
    pub end_tick: u32,
}

impl Cmf {
    /// Parses a CMF file.
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut r = ByteReader::new("cmf", data);
        if r.bytes(4)? != b"CTMF" {
            return Err(FormatError::BadMagic { format: "cmf" });
        }
        let version = r.u16_le()?;
        let inst_offset = r.u16_le()? as usize;
        let music_offset = r.u16_le()? as usize;
        let ticks_per_quarter = r.u16_le()?;
        let ticks_per_second = r.u16_le()?;
        r.seek(0x14)?;
        let mut channel_in_use = [false; 16];
        for c in &mut channel_in_use {
            *c = r.u8()? != 0;
        }
        let count = r.u16_le()? as usize;
        let basic_tempo = r.u16_le()?;
        if ticks_per_second == 0 {
            return Err(r.invalid("ticks per second is zero"));
        }

        r.seek(inst_offset)?;
        let mut instruments = Vec::with_capacity(count);
        for _ in 0..count {
            let b = r.bytes(16)?;
            let op = |i: usize| OperatorRegs {
                char_20: b[i],
                level_40: b[2 + i],
                attack_decay_60: b[4 + i],
                sustain_release_80: b[6 + i],
                wave_e0: b[8 + i],
            };
            instruments.push(Instrument {
                modulator: op(0),
                carrier: op(1),
                feedback_connection: b[10],
            });
        }

        r.seek(music_offset)?;
        let (events, end_tick) = parse_track(&mut r)?;
        Ok(Self {
            version,
            ticks_per_quarter,
            ticks_per_second,
            channel_in_use,
            basic_tempo,
            instruments,
            events,
            end_tick,
        })
    }
}

fn read_vlq(r: &mut ByteReader<'_>) -> Result<u32> {
    let mut v: u32 = 0;
    for _ in 0..4 {
        let b = r.u8()?;
        v = (v << 7) | u32::from(b & 0x7F);
        if b & 0x80 == 0 {
            return Ok(v);
        }
    }
    Err(r.invalid("variable-length quantity longer than 4 bytes"))
}

fn parse_track(r: &mut ByteReader<'_>) -> Result<(Vec<Event>, u32)> {
    let mut events = Vec::new();
    let mut tick: u32 = 0;
    let mut status: Option<u8> = None;
    loop {
        let delta = read_vlq(r)?;
        tick = tick
            .checked_add(delta)
            .ok_or_else(|| r.invalid("tick overflow"))?;
        let first = r.u8()?;
        let (st, data0) = if first & 0x80 != 0 {
            (first, None)
        } else {
            let st = status.ok_or_else(|| r.invalid("running status without a status byte"))?;
            (st, Some(first))
        };
        let next = |r: &mut ByteReader<'_>, pending: &mut Option<u8>| -> Result<u8> {
            match pending.take() {
                Some(b) => Ok(b),
                None => r.u8(),
            }
        };
        let mut pending = data0;
        match st {
            0xFF => {
                let meta = r.u8()?;
                let len = read_vlq(r)? as usize;
                r.bytes(len)?;
                if meta == 0x2F {
                    return Ok((events, tick));
                }
                continue;
            }
            0xF0 | 0xF7 => {
                let len = read_vlq(r)? as usize;
                r.bytes(len)?;
                continue;
            }
            0xF1..=0xFE => return Err(r.invalid(format!("unsupported status {st:#04x}"))),
            _ => {}
        }
        status = Some(st);
        let channel = st & 0x0F;
        let kind = match st & 0xF0 {
            0x80 => {
                let note = next(r, &mut pending)?;
                next(r, &mut pending)?;
                EventKind::NoteOff { channel, note }
            }
            0x90 => {
                let note = next(r, &mut pending)?;
                let velocity = next(r, &mut pending)?;
                if velocity == 0 {
                    EventKind::NoteOff { channel, note }
                } else {
                    EventKind::NoteOn {
                        channel,
                        note,
                        velocity,
                    }
                }
            }
            0xA0 | 0xD0 => {
                next(r, &mut pending)?;
                if st & 0xF0 == 0xA0 {
                    next(r, &mut pending)?;
                }
                EventKind::Pressure { channel }
            }
            0xB0 => EventKind::Controller {
                channel,
                controller: next(r, &mut pending)?,
                value: next(r, &mut pending)?,
            },
            0xC0 => EventKind::Program {
                channel,
                program: next(r, &mut pending)?,
            },
            0xE0 => {
                let lo = next(r, &mut pending)? as u16;
                let hi = next(r, &mut pending)? as u16;
                EventKind::PitchBend {
                    channel,
                    value: (hi << 7) | lo,
                }
            }
            _ => return Err(r.invalid(format!("bad status {st:#04x}"))),
        };
        events.push(Event { tick, kind });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(track: &[u8]) -> Vec<u8> {
        let mut v = vec![0u8; 0x28];
        v[..4].copy_from_slice(b"CTMF");
        v[4..6].copy_from_slice(&0x0101u16.to_le_bytes());
        v[6..8].copy_from_slice(&0x28u16.to_le_bytes()); // instruments at 0x28
        v[8..10].copy_from_slice(&0x38u16.to_le_bytes()); // music after one instrument
        v[0x0A..0x0C].copy_from_slice(&48u16.to_le_bytes());
        v[0x0C..0x0E].copy_from_slice(&96u16.to_le_bytes());
        v[0x14] = 1;
        v[0x24..0x26].copy_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&[0x21, 0x01, 0x10, 0x00, 0xF2, 0xF3, 0x54, 0x55, 0, 1, 0x0E]);
        v.extend_from_slice(&[0; 5]);
        v.extend_from_slice(track);
        v
    }

    #[test]
    fn parses_header_instruments_and_events() {
        let track = [
            0x00, 0xB0, 0x67, 0x01, // rhythm on
            0x00, 0xC0, 0x00, // program 0
            0x60, 0x90, 0x45, 0x7F, // +96: note on A4
            0x81, 0x00, 0x45, 0x00, // +128 (running status): note off via velocity 0
            0x00, 0xFF, 0x2F, 0x00,
        ];
        let c = Cmf::parse(&file(&track)).unwrap();
        assert_eq!(c.ticks_per_second, 96);
        assert!(c.channel_in_use[0] && !c.channel_in_use[1]);
        assert_eq!(c.instruments.len(), 1);
        assert_eq!(c.instruments[0].modulator.char_20, 0x21);
        assert_eq!(c.instruments[0].carrier.wave_e0, 1);
        assert_eq!(c.instruments[0].feedback_connection, 0x0E);
        assert_eq!(
            c.events,
            vec![
                Event {
                    tick: 0,
                    kind: EventKind::Controller {
                        channel: 0,
                        controller: 0x67,
                        value: 1
                    }
                },
                Event {
                    tick: 0,
                    kind: EventKind::Program {
                        channel: 0,
                        program: 0
                    }
                },
                Event {
                    tick: 96,
                    kind: EventKind::NoteOn {
                        channel: 0,
                        note: 0x45,
                        velocity: 127
                    }
                },
                Event {
                    tick: 224,
                    kind: EventKind::NoteOff {
                        channel: 0,
                        note: 0x45
                    }
                },
            ]
        );
        assert_eq!(c.end_tick, 224);
    }

    #[test]
    fn rejects_bad_input() {
        assert!(Cmf::parse(b"MThd").is_err());
        assert!(
            Cmf::parse(&file(&[0x00, 0x45])).is_err(),
            "running status first"
        );
        assert!(Cmf::parse(&file(&[0x00, 0x90, 0x45])).is_err(), "truncated");
    }
}
