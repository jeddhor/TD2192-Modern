//! The F6 transmission screen (`UI_CommScreen(n)`).
//!
//! Spec: `docs/spec/flow/flight-screens.md` rev 1 §3.3, §3.5, §4.2. Fidelity: T0 for the
//! picture (C-0013, transmission 1; the others LIKELY, U-221), T2 for the typing pace (one
//! character per retrace, as the front-end typing).
//!
//! Which transmission opens, and the mission-phase advance, belong to the mission logic
//! (`mission-logic.md` §4.6, `td_sim`): this screen only shows transmission `n`.

use td_input::{Key, TickInput};
use td_render::small_text::{self, Typewriter};
use td_render::{Framebuffer, Image, Rect, WIDTH};

use super::{FlightScreenAssets, KeyBytes, ModalStep};

/// Transmissions 1..=9 of Episode 1.
pub const TRANSMISSIONS: usize = 9;

/// Panel cells: `td.cmm` cell k at screen (32 + 32 (k mod 8), 16 + 32 (k div 8)) (§3.5).
pub const PANEL_ORIGIN: (i32, i32) = (32, 16);
/// Portrait positions: box A, box B (§3.5).
pub const PORTRAIT_POS: [(i32, i32); 2] = [(48, 32), (48, 96)];
/// Header pens (§3.5).
pub const HEADER_PEN: [(i32, i32); 2] = [(92, 35), (92, 87)];
/// Typed-line pens (§3.5).
pub const LINE_PEN: [(i32, i32); 2] = [(92, 67), (92, 119)];
/// Inks: headers, lines, markers (§3.5).
pub const HEADER_INK: u8 = 45;
pub const LINE_INK: u8 = 40;
pub const MARKER_INK: u8 = 43;
/// Scroll boxes: first and last screen row, and the column span (§3.5, §4.2).
pub const SCROLL_ROWS: [(usize, usize); 2] = [(42, 72), (94, 124)];
pub const SCROLL_X: (usize, usize) = (92, 271);
/// One-row scrolls after each typed line.
pub const SCROLLS_PER_LINE: usize = 6;

/// A transmission as the loaders fill it (§3.3), in the spec's notation: portraits (A, B)
/// as `td1.ccm` records; the lines as indices into `e1.comms.0n` with `P` and `E` for the
/// prompt and end markers; the speaker of each line (A = box A, B = box B).
struct Spec {
    portraits: (usize, usize),
    lines: &'static str,
    speakers: &'static str,
}

const SPECS: [Spec; TRANSMISSIONS] = [
    Spec {
        portraits: (1, 0),
        lines: "2 3 4 5 6 P 8 9 10 11 12 P 13 14 14 15 14 E",
        speakers: "A 1..15, B 16..18",
    },
    Spec {
        portraits: (2, 0),
        lines: "2 3 4 5 P 7 8 9 P 10 11 12 13 14 15 15 E",
        // Line 17 (the end marker) is not written; its speaker does not matter. U-230: §5
        // says line 16 is not written either.
        speakers: "A 1..6, B 7..10, A 11..13, B 14..15, A 16",
    },
    Spec {
        portraits: (3, 0),
        lines: "2 P 4 5 6 7 P 8 9 10 11 12 P 13 14 15 13 P 13 16 17 18 19 P 4 20 21 E",
        speakers: "A 1..3, B 4..8, A 9..19, B 20..25, A 26..28",
    },
    Spec {
        portraits: (3, 0),
        lines: "2 3 4 P 6 7 8 8 9 10 11 12 P 13 14 15 16 17 P 13 E",
        speakers: "A 1..5, B 6..8, A 9..14, B 15..21",
    },
    Spec {
        portraits: (4, 0),
        lines: "2 3 4 5 P 7 8 9 10 11 P 12 E",
        speakers: "A 1..6, B 7..13",
    },
    Spec {
        portraits: (5, 0),
        lines: "2 3 4 5 P 7 8 9 10 11 P 12 13 14 15 E",
        speakers: "A 1..6, B 7..12, A 13..16",
    },
    Spec {
        portraits: (6, 0),
        lines: "2 3 P 5 6 P 7 8 9 10 P 11 12 10 P 13 14 15 E",
        speakers: "A 1..3, B 4..6, A 7..11, B 12..15, A 16..19",
    },
    Spec {
        portraits: (7, 0),
        lines: "2 3 4 5 P 7 8 9 10 P 11 12 13 14 15 P 16 17 18 19 20 21 22 23 E",
        speakers: "A 1..21, B 21..25",
    },
    Spec {
        portraits: (0, 8),
        lines: "2 3 4 5 P 7 8 9 10 E",
        speakers: "B 1..6, A 7..10",
    },
];

/// One line slot of a transmission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Line {
    /// `e1.comms.0n[j]`.
    Text(usize),
    /// The prompt marker (`comm.prompts[0]`): wait for Space.
    Prompt,
    /// The end marker (`comm.prompts[1]`): typed in both boxes, then F6 or Esc closes.
    End,
}

/// A transmission's portraits and lines with their box (0 = A, 1 = B).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transmission {
    pub portraits: (usize, usize),
    pub lines: Vec<(Line, usize)>,
}

/// Transmission `n` (1..=9) from the §3.3 table.
pub fn transmission(n: usize) -> Option<Transmission> {
    let spec = SPECS.get(n.checked_sub(1)?)?;
    let lines: Vec<Line> = spec
        .lines
        .split_whitespace()
        .map(|t| match t {
            "P" => Line::Prompt,
            "E" => Line::End,
            j => Line::Text(j.parse().unwrap_or(0)),
        })
        .collect();
    // Speaker byte per line k = 1..: 1 = box A, anything else (2, or 0 if never written) = B.
    let mut side = vec![1usize; lines.len()];
    for part in spec.speakers.split(',') {
        let mut it = part.split_whitespace();
        let (Some(who), Some(range)) = (it.next(), it.next()) else {
            continue;
        };
        let (a, b) = range.split_once("..").unwrap_or((range, range));
        let (Ok(a), Ok(b)) = (a.parse::<usize>(), b.parse::<usize>()) else {
            continue;
        };
        for k in a..=b {
            if let Some(s) = side.get_mut(k - 1) {
                *s = usize::from(who == "B");
            }
        }
    }
    Some(Transmission {
        portraits: spec.portraits,
        lines: lines.into_iter().zip(side).collect(),
    })
}

/// What the screen does after the current typing finishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum After {
    /// Scroll the box six rows and go to the next line.
    Scroll(usize),
    /// Wait for Space, then the next line.
    WaitSpace,
    /// Type the end marker in box B.
    EndInB,
    /// Wait for F6 or Esc.
    WaitClose,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum State {
    Typing(Typewriter, After),
    WaitSpace,
    WaitClose,
}

/// The transmission screen (§4.2).
#[derive(Debug, Clone)]
pub struct CommScreen {
    canvas: Framebuffer,
    n: usize,
    tx: Transmission,
    /// Next line slot (0-based).
    k: usize,
    state: State,
    keys: KeyBytes,
    background: u8,
}

/// Scrolls box `side` up one row and fills its bottom row with `bg` (§4.2).
pub fn scroll_box(fb: &mut Framebuffer, side: usize, bg: u8) {
    let (top, bottom) = SCROLL_ROWS[side];
    let (x0, x1) = SCROLL_X;
    for y in top..bottom {
        let (dst, src) = (y * WIDTH, (y + 1) * WIDTH);
        fb.pixels.copy_within(src + x0..=src + x1, dst + x0);
    }
    fb.pixels[bottom * WIDTH + x0..=bottom * WIDTH + x1].fill(bg);
}

impl CommScreen {
    /// Opens transmission `n` (1..=9) over `shown`, the frame on screen (page 0 with this
    /// tick's world; the DAC stays the flight's). `None` if `n` is not a transmission.
    pub fn new(mut shown: Framebuffer, assets: &FlightScreenAssets, n: usize) -> Option<Self> {
        let tx = transmission(n)?;
        for (cell, pixels) in assets.panel.chunks_exact(1024).enumerate() {
            let (col, row) = ((cell % 8) as i32, (cell / 8) as i32);
            let pos = (PANEL_ORIGIN.0 + 32 * col, PANEL_ORIGIN.1 + 32 * row);
            shown.blit(
                Image::new(pixels, 32, 32),
                pos.0,
                pos.1,
                Rect::SCREEN,
                Some(0),
            );
        }
        for (record, pos) in [tx.portraits.0, tx.portraits.1]
            .into_iter()
            .zip(PORTRAIT_POS)
        {
            if let Some(face) = assets.portraits.get(record * 1024..(record + 1) * 1024) {
                shown.blit(
                    Image::new(face, 32, 32),
                    pos.0,
                    pos.1,
                    Rect::SCREEN,
                    Some(0),
                );
            }
        }
        let bg = assets.font_background;
        let strings = &assets.comms[n - 1];
        for (h, pen) in HEADER_PEN.into_iter().enumerate() {
            let text = FlightScreenAssets::text(strings, h);
            small_text::draw_opaque(&mut shown, &assets.font, text, pen, HEADER_INK, bg);
        }
        let mut screen = Self {
            canvas: shown,
            n,
            tx,
            k: 0,
            state: State::WaitClose,
            keys: KeyBytes::default(),
            background: bg,
        };
        screen.next_line(assets);
        Some(screen)
    }

    /// The transmission shown.
    pub fn number(&self) -> usize {
        self.n
    }

    /// The picture on screen.
    pub fn canvas(&self) -> &Framebuffer {
        &self.canvas
    }

    /// True while the screen waits for Space at a prompt marker.
    pub fn waiting_for_space(&self) -> bool {
        self.state == State::WaitSpace
    }

    /// True while the screen waits for F6 or Esc after the end marker.
    pub fn waiting_to_close(&self) -> bool {
        self.state == State::WaitClose
    }

    fn typewriter(&self, text: &str, side: usize, ink: u8) -> Typewriter {
        Typewriter::new(text, LINE_PEN[side], ink, self.background)
    }

    /// Starts line slot `k` (the loop head of §4.2).
    fn next_line(&mut self, assets: &FlightScreenAssets) {
        let Some(&(line, side)) = self.tx.lines.get(self.k) else {
            // A table without an end marker: wait to close.
            self.state = State::WaitClose;
            return;
        };
        let prompts = &assets.comm_prompts;
        self.state = match line {
            Line::Text(j) => {
                let text = FlightScreenAssets::text(&assets.comms[self.n - 1], j);
                State::Typing(self.typewriter(text, side, LINE_INK), After::Scroll(side))
            }
            Line::Prompt => {
                let text = FlightScreenAssets::text(prompts, 0);
                State::Typing(self.typewriter(text, side, MARKER_INK), After::WaitSpace)
            }
            Line::End => {
                let text = FlightScreenAssets::text(prompts, 1);
                State::Typing(self.typewriter(text, 0, MARKER_INK), After::EndInB)
            }
        };
    }

    /// Advances one retrace.
    pub fn tick(&mut self, assets: &FlightScreenAssets, input: &TickInput) -> ModalStep {
        self.keys.update(input);
        loop {
            match &mut self.state {
                State::Typing(tw, after) => {
                    let after = *after;
                    let typed = tw.step(&mut self.canvas, &assets.font).is_some();
                    if !tw.finished() {
                        return ModalStep::Continue;
                    }
                    match after {
                        After::Scroll(side) => {
                            for _ in 0..SCROLLS_PER_LINE {
                                scroll_box(&mut self.canvas, side, self.background);
                            }
                            self.k += 1;
                            self.next_line(assets);
                        }
                        After::WaitSpace => self.state = State::WaitSpace,
                        After::EndInB => {
                            let text = FlightScreenAssets::text(&assets.comm_prompts, 1);
                            self.state = State::Typing(
                                self.typewriter(text, 1, MARKER_INK),
                                After::WaitClose,
                            );
                        }
                        After::WaitClose => self.state = State::WaitClose,
                    }
                    if typed {
                        return ModalStep::Continue;
                    }
                }
                State::WaitSpace => {
                    if !self.keys.get(Key::SPACE) {
                        return ModalStep::Continue;
                    }
                    self.keys.clear(Key::SPACE);
                    self.k += 1;
                    self.next_line(assets);
                }
                State::WaitClose => {
                    if self.keys.get(Key::F6) || self.keys.get(Key::ESCAPE) {
                        return ModalStep::Close { abort: false };
                    }
                    return ModalStep::Continue;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_shapes() {
        // String counts of e1.comms.01..09 (resource map): every index fits, every
        // transmission ends with its end marker.
        let counts = [17, 17, 23, 19, 14, 17, 17, 25, 12];
        for (n, count) in (1..=TRANSMISSIONS).zip(counts) {
            let t = transmission(n).unwrap();
            assert_eq!(t.lines.last().map(|l| l.0), Some(Line::End), "n = {n}");
            for &(line, _) in &t.lines {
                if let Line::Text(j) = line {
                    assert!((2..count).contains(&j), "n = {n}, j = {j}");
                }
            }
        }
        assert_eq!(transmission(1).unwrap().lines.len(), 18);
        assert_eq!(transmission(3).unwrap().lines.len(), 28);
        assert_eq!(transmission(8).unwrap().lines.len(), 25);
        assert!(transmission(0).is_none() && transmission(10).is_none());
    }

    #[test]
    fn speakers() {
        let side = |n, k: usize| transmission(n).unwrap().lines[k - 1].1;
        assert_eq!((side(1, 15), side(1, 16)), (0, 1));
        // n = 8: B wins for line 21.
        assert_eq!((side(8, 20), side(8, 21)), (0, 1));
        assert_eq!((side(9, 6), side(9, 7)), (1, 0));
        assert_eq!(transmission(9).unwrap().portraits, (0, 8));
    }

    #[test]
    fn scroll_moves_one_row_and_fills() {
        let mut fb = Framebuffer::new();
        fb.put(100, 72, 9);
        fb.put(100, 42, 7);
        fb.put(91, 72, 5);
        scroll_box(&mut fb, 0, 178);
        assert_eq!(fb.get(100, 71), Some(9));
        assert_eq!(fb.get(100, 72), Some(178));
        assert_eq!(fb.get(100, 41), Some(0), "row 42 is the top: it drops out");
        assert_eq!(fb.get(91, 72), Some(5), "outside the box");
        assert_eq!(fb.get(272, 72), Some(0));
        assert_eq!(fb.get(271, 72), Some(178));
    }
}
