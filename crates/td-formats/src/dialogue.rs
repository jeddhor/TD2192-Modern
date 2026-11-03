//! Dialogue scripts (`dialogue.dat`, `safedial.dat` and the episode 2/3 variants).
//!
//! Spec: `docs/re/FORMATS.md` §8 (CONFIRMED) and `docs/spec/narrative/dialogue-player.md`
//! (rev 1) §3.1, §4.2–§4.3. Fidelity: T0 for parsing.
//!
//! Two readers:
//! * [`read_scene`] reads one scene the way the original's player does (search by ID, then
//!   every line up to the one starting with `d`, in file order, ignoring unknown lines). The
//!   scene player uses it.
//! * [`DialogueScript::parse`] is a strict structural parser of a whole file, used to validate
//!   the shipped scripts and to explore them.
//!
//! The file is CR LF separated lines. Each line starts with a control byte:
//!
//! | Code | Line | Meaning |
//! |---|---|---|
//! | `06 n` | | start of scene `n` (a bare `06` is scene 13: its number byte 0x0D is the CR) |
//! | `01 slot face color name` | | speaker definition (in the header, or later to bring in a new speaker) |
//! | `01 slot face` | | portrait-only change for a slot (E3) |
//! | `03` | | start of a segment of lines |
//! | `02 p color text` | | a line spoken in `color` (`p` is always 1 in the shipped files) |
//! | `05` | | page break (wait for the player) |
//! | `04` | | end of a segment |
//! | `d` | | end of scene (follows the scene's last `04`) |
//!
//! A scene may hold several segments (`04` followed by `03`); later segments reuse the scene's
//! speakers unless `01` lines between the segments, or inside one, bring in new speakers (these
//! become [`Item::Speaker`] entries). Code-only lines may carry trailing spaces. Parsing is
//! otherwise strict: any other code is an error, so that unknown constructs surface in tests.

use crate::{FormatError, Result};

/// A parsed dialogue script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialogueScript {
    pub scenes: Vec<Scene>,
}

/// One scene: a speaker table and one or more segments of lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scene {
    /// Scene number from the `06` line.
    pub number: u8,
    pub speakers: Vec<Speaker>,
    pub segments: Vec<Vec<Item>>,
}

/// A speaker definition (`01` line).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Speaker {
    /// Speaker slot (1 or 2 in the shipped files).
    pub slot: u8,
    /// Portrait index into the episode's portrait file.
    pub face: u8,
    /// Text colour (palette index) used by this speaker's lines.
    pub color: u8,
    /// Display name.
    pub name: String,
}

/// One element of a segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    /// A text line (`02`). The speaker is the one whose colour matches.
    Line { param: u8, color: u8, text: String },
    /// A page break (`05`).
    PageBreak,
    /// A speaker (re)defined part-way through a scene (`01 slot face color name`), for example a
    /// new character taking slot 2.
    Speaker(Speaker),
    /// A portrait-only change for a slot (3-byte `01 slot face` line; E3 only). Face 200 appears
    /// when a character leaves, so it probably means "no portrait" (U-016).
    Face { slot: u8, face: u8 },
}

impl Scene {
    /// The header speaker whose colour matches `color`, if any. Mid-segment
    /// [`Item::Speaker`] changes are applied by the dialogue player as it plays.
    pub fn speaker_for_color(&self, color: u8) -> Option<&Speaker> {
        self.speakers.iter().find(|s| s.color == color)
    }
}

fn speaker(line: &[u8]) -> Speaker {
    Speaker {
        slot: line[1],
        face: line[2],
        color: line[3],
        name: text(&line[4..]),
    }
}

const FORMAT: &str = "dialogue";

/// Bytes are mapped one-to-one to `char`s so that no information is lost.
fn text(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| char::from(b)).collect()
}

impl DialogueScript {
    /// Parses a whole dialogue file.
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut scenes: Vec<Scene> = Vec::new();
        let mut offset = 0usize;
        // Parser state: are we inside a scene header, a segment, or between segments?
        #[derive(PartialEq)]
        enum State {
            Outside,
            Header,
            Segment,
            AfterSegment,
        }
        let mut state = State::Outside;
        // Speaker or face changes between two segments; they open the next segment.
        let mut pending: Vec<Item> = Vec::new();
        let lines = split_crlf(data);
        let n_lines = lines.len();
        for (i, line) in lines.into_iter().enumerate() {
            let at = offset;
            offset += line.len() + 2;
            let err = |detail: &str| FormatError::Invalid {
                format: FORMAT,
                offset: at,
                detail: format!("line {}: {detail}", i + 1),
            };
            let Some(&code) = line.first() else {
                if i + 1 == n_lines {
                    break; // the file ends with CR LF
                }
                return Err(err("empty line"));
            };
            // Code-only lines may carry trailing spaces (`05` + two spaces occurs in E2).
            let bare = line[1..].iter().all(|&b| b == b' ');
            match (code, &state) {
                (0x06, State::Outside) => {
                    let number = match *line {
                        [_, n] => n,
                        // Scene 13's number byte is 0x0D, which doubles as the CR of the line
                        // terminator in the shipped files (`06 0D 0A`).
                        [_] => 0x0D,
                        _ => return Err(err("scene line must be 1 or 2 bytes")),
                    };
                    scenes.push(Scene {
                        number,
                        speakers: Vec::new(),
                        segments: Vec::new(),
                    });
                    state = State::Header;
                }
                (0x01, State::Header) => {
                    if line.len() < 4 {
                        return Err(err("speaker line too short"));
                    }
                    let scene = scenes.last_mut().ok_or_else(|| err("no scene"))?;
                    scene.speakers.push(speaker(line));
                }
                (0x01, State::Segment | State::AfterSegment) => {
                    let item = match line.len() {
                        3 => Item::Face {
                            slot: line[1],
                            face: line[2],
                        },
                        4.. => Item::Speaker(speaker(line)),
                        _ => return Err(err("speaker line too short")),
                    };
                    if state == State::Segment {
                        let seg = current_segment(&mut scenes).ok_or_else(|| err("no segment"))?;
                        seg.push(item);
                    } else {
                        pending.push(item);
                    }
                }
                (0x03, State::Header | State::AfterSegment) if bare => {
                    let segment = std::mem::take(&mut pending);
                    scenes
                        .last_mut()
                        .ok_or_else(|| err("no scene"))?
                        .segments
                        .push(segment);
                    state = State::Segment;
                }
                (0x02, State::Segment) => {
                    if line.len() < 3 {
                        return Err(err("text line too short"));
                    }
                    let seg = current_segment(&mut scenes).ok_or_else(|| err("no segment"))?;
                    seg.push(Item::Line {
                        param: line[1],
                        color: line[2],
                        text: text(&line[3..]),
                    });
                }
                (0x05, State::Segment) if bare => {
                    let seg = current_segment(&mut scenes).ok_or_else(|| err("no segment"))?;
                    seg.push(Item::PageBreak);
                }
                (0x04, State::Segment) if bare => state = State::AfterSegment,
                (b'd', State::AfterSegment) if bare && pending.is_empty() => state = State::Outside,
                _ => return Err(err(&format!("unexpected code {code:#04x}"))),
            }
        }
        if state != State::Outside {
            return Err(FormatError::Truncated {
                format: FORMAT,
                offset: data.len(),
                needed: 1,
            });
        }
        Ok(Self { scenes })
    }

    /// The scene with number `n`, if present.
    pub fn scene(&self, n: u8) -> Option<&Scene> {
        self.scenes.iter().find(|s| s.number == n)
    }
}

/// One script line of a scene, as the original's player reads it (dialogue-player.md §3.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// `01 slot face colour name`: speaker definition (§4.4). Slot 1 is the left portrait and
    /// any other value the right one.
    Speaker {
        slot: u8,
        face: u8,
        color: u8,
        name: String,
    },
    /// `02 _ colour text`: a line, typed in the colour mapped to `color` (§4.5). The second
    /// byte is read and ignored.
    Line { color: u8, text: String },
    /// `03`: start of a segment, fade in (§4.6).
    SegmentStart,
    /// `04`: end of a segment, fade out (§4.6).
    SegmentEnd,
    /// `05`: page break, wait for a key (§4.7).
    PageBreak,
}

/// Value of a single-character field read past the end of a line: the line's CR. This is how
/// the bare `06` line of scene 13 gets its ID, and how the 3-byte `01 slot face` lines of
/// episode 3 get colour 13 (dialogue-player.md §4.2 step 4 and §5 item 3).
pub const END_OF_LINE_FIELD: u8 = 0x0D;

/// Reads scene `id` the way the original's player does (dialogue-player.md §4.2 step 4 and
/// §4.3): lines are read from the top until the `06` line whose ID byte equals `id`; then every
/// following line up to the first one starting with `d` (or the end of the file) is
/// interpreted by its first byte. `06` lines inside a scene and lines with other first bytes are
/// ignored.
///
/// Fields are single bytes; a field past the end of the line reads as
/// [`END_OF_LINE_FIELD`]; the rest of the line is the string. Port policy for U-063 (§5 item
/// 3): a short `01` line keeps colour 13, gets an empty name and never swallows the next line.
///
/// Returns `None` if no scene has that ID (the original then searches forever, §5 item 5).
pub fn read_scene(data: &[u8], id: u8) -> Option<Vec<Command>> {
    let mut lines = split_crlf(data).into_iter();
    let field = |line: &[u8], i: usize| line.get(i).copied().unwrap_or(END_OF_LINE_FIELD);
    let rest = |line: &[u8], from: usize| text(line.get(from..).unwrap_or_default());
    lines.find(|l| l.first() == Some(&0x06) && field(l, 1) == id)?;
    let mut out = Vec::new();
    for line in lines {
        let Some(&code) = line.first() else {
            continue;
        };
        out.push(match code {
            b'd' => break,
            0x01 => Command::Speaker {
                slot: field(line, 1),
                face: field(line, 2),
                color: field(line, 3),
                name: rest(line, 4),
            },
            0x02 => Command::Line {
                color: field(line, 2),
                text: rest(line, 3),
            },
            0x03 => Command::SegmentStart,
            0x04 => Command::SegmentEnd,
            0x05 => Command::PageBreak,
            _ => continue,
        });
    }
    Some(out)
}

fn current_segment(scenes: &mut [Scene]) -> Option<&mut Vec<Item>> {
    scenes.last_mut()?.segments.last_mut()
}

/// Splits on CR LF. A trailing CR LF yields a final empty element.
fn split_crlf(data: &[u8]) -> Vec<&[u8]> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i + 1 < data.len() {
        if data[i] == b'\r' && data[i + 1] == b'\n' {
            out.push(&data[start..i]);
            i += 2;
            start = i;
        } else {
            i += 1;
        }
    }
    out.push(&data[start..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn script(lines: &[&[u8]]) -> Vec<u8> {
        let mut v = Vec::new();
        for l in lines {
            v.extend_from_slice(l);
            v.extend_from_slice(b"\r\n");
        }
        v
    }

    #[test]
    fn parses_scenes_segments_and_speakers() {
        let data = script(&[
            b"\x06\x01",
            b"\x01\x01\x05\x2ealpha",
            b"\x01\x02\x07\x39beta",
            b"\x03",
            b"\x02\x01\x39 hello",
            b"\x05",
            b"\x02\x01\x2e hi",
            b"\x04",
            b"\x03",
            b"\x02\x01\x14 later",
            b"\x01\x01\xc8",
            b"\x04",
            b"\x01\x02\x09\x14gamma",
            b"\x03",
            b"\x05  ",
            b"\x04",
            b"d",
            b"\x06\x02",
            b"\x01\x01\x05\x2ealpha",
            b"\x03",
            b"\x04",
            b"d",
        ]);
        let s = DialogueScript::parse(&data).unwrap();
        assert_eq!(s.scenes.len(), 2);
        let first = &s.scenes[0];
        assert_eq!(first.speakers.len(), 2);
        assert_eq!(first.speaker_for_color(0x39).unwrap().name, "beta");
        assert_eq!(first.segments.len(), 3);
        assert_eq!(
            first.segments[2],
            vec![
                Item::Speaker(Speaker {
                    slot: 2,
                    face: 9,
                    color: 0x14,
                    name: "gamma".into()
                }),
                Item::PageBreak,
            ]
        );
        assert_eq!(
            first.segments[0],
            vec![
                Item::Line {
                    param: 1,
                    color: 0x39,
                    text: " hello".into()
                },
                Item::PageBreak,
                Item::Line {
                    param: 1,
                    color: 0x2e,
                    text: " hi".into()
                },
            ]
        );
        assert_eq!(s.scene(2).unwrap().segments, vec![Vec::<Item>::new()]);
    }

    #[test]
    fn reads_one_scene_like_the_player() {
        let data = script(&[
            b"\x06\x01",
            b"\x01\x01\x05\x2ealpha",
            b"\x03",
            b"\x02\x01\x2e hi",
            b"\x04",
            b"d",
            b"\x06", // scene 13: the ID is the line's CR
            b"\x01\x02\xc8\x39",
            b"\x03",
            b"\x06\x07", // ignored inside a scene
            b"\x07junk", // ignored
            b"",
            b"\x02\x01\x39",
            b"\x01\x01\x09", // short speaker line: colour = CR, no name
            b"\x05  ",
            b"\x04",
            b"\x03",
            b"\x04",
            b"d",
            b"\x02\x01\x39 after the end",
        ]);
        assert_eq!(
            read_scene(&data, 1).unwrap(),
            [
                Command::Speaker {
                    slot: 1,
                    face: 5,
                    color: 0x2e,
                    name: "alpha".into()
                },
                Command::SegmentStart,
                Command::Line {
                    color: 0x2e,
                    text: " hi".into()
                },
                Command::SegmentEnd,
            ]
        );
        assert_eq!(
            read_scene(&data, 13).unwrap(),
            [
                Command::Speaker {
                    slot: 2,
                    face: 200,
                    color: 0x39,
                    name: String::new()
                },
                Command::SegmentStart,
                Command::Line {
                    color: 0x39,
                    text: String::new()
                },
                Command::Speaker {
                    slot: 1,
                    face: 9,
                    color: END_OF_LINE_FIELD,
                    name: String::new()
                },
                Command::PageBreak,
                Command::SegmentEnd,
                Command::SegmentStart,
                Command::SegmentEnd,
            ]
        );
        assert_eq!(read_scene(&data, 2), None);
        // No `d`: the scene runs to the end of the file.
        assert_eq!(
            read_scene(&script(&[b"\x06\x03", b"\x03"]), 3).unwrap(),
            [Command::SegmentStart]
        );
    }

    #[test]
    fn rejects_malformed_scripts() {
        for bad in [
            script(&[b"\x02\x01\x2ex"]),              // text outside a segment
            script(&[b"\x06\x01", b"\x03", b"\x07"]), // unknown code
            script(&[b"\x06\x01", b"\x03"]),          // truncated
            script(&[b"\x06"]),                       // short scene line
            script(&[b"\x06\x01", b"", b"\x03"]),     // empty line inside
        ] {
            assert!(DialogueScript::parse(&bad).is_err(), "{bad:?}");
        }
        assert_eq!(DialogueScript::parse(b"").unwrap().scenes.len(), 0);
    }
}
