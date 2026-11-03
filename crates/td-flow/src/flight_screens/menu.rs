//! The Esc computer menu (`UI_FlightMenu`), the ship library and the in-flight help.
//!
//! Spec: `docs/spec/flow/flight-screens.md` rev 1 §3.1, §3.2, §3.5, §4.1. Fidelity: T0 for
//! the pictures and DAC values (C-0012), T1 for the abort flag and the keys (OB-030 kept),
//! T2 for the pacing.
//!
//! The routine is a straight-line program with fades, highlight pulses and key waits. The port
//! runs it as a queue of steps, one retrace per waiting step; a key poll appends the steps of
//! its outcome.

use std::collections::VecDeque;

use td_formats::palette::{Palette, Rgb6};
use td_input::{Key, TickInput};
use td_render::flight_palette::{FADE_STEPS, fade_target, write_fade_step};
use td_render::small_text;
use td_render::{Framebuffer, Image, Rect};

use super::{FlightScreenAssets, KEY_KP2, KEY_KP8, KeyBytes, ModalStep, blank_below_display};

/// Where the 160 × 100 menu image goes (§3.5).
pub const MENU_POS: (i32, i32) = (80, 50);
/// Menu image size.
pub const MENU_W: usize = 160;
pub const MENU_H: usize = 100;
/// Palette entry of item 0's letters; items 1..3 follow (§3.1).
pub const FIRST_ITEM_ENTRY: usize = 107;
/// Number of items: return, ship library, help, quit.
pub const ITEMS: u8 = 4;
/// The item colour while the menu is open, and the low end of the highlight pulse (§3.5).
pub const ITEM_RED_LOW: u8 = 32;
/// The high end of the highlight pulse.
pub const ITEM_RED_HIGH: u8 = 63;

/// Retraces per key poll: `Crt.Delay(100)` is 100 ms, 7.0 retraces at 70.086 Hz.
/// ASSUMPTION (T2): seven whole retraces.
pub const POLL_FRAMES: u8 = 7;

/// Flights from which the library shows the fourth player ship (sprite 8) unhatched (§3.2).
pub const LIBRARY_SHIP_8_FROM: u8 = 4;

/// One ship of the library (§3.2): `td.shp` record, text pen x and the lines
/// (pen y, `ship_library` index, ink).
struct LibraryShip {
    sprite: usize,
    pen_x: i32,
    lines: [(i32, usize, u8); 4],
}

const LIBRARY: [LibraryShip; 7] = [
    LibraryShip {
        sprite: 0,
        pen_x: 40,
        lines: [(12, 0, 96), (18, 1, 92), (24, 2, 92), (32, 3, 94)],
    },
    LibraryShip {
        sprite: 1,
        pen_x: 40,
        lines: [(49, 4, 96), (55, 1, 92), (61, 5, 92), (69, 6, 94)],
    },
    LibraryShip {
        sprite: 7,
        pen_x: 40,
        lines: [(92, 7, 96), (98, 8, 92), (104, 9, 92), (112, 10, 94)],
    },
    LibraryShip {
        sprite: 8,
        pen_x: 40,
        lines: [(123, 11, 96), (129, 12, 92), (135, 13, 92), (143, 14, 94)],
    },
    LibraryShip {
        sprite: 2,
        pen_x: 200,
        lines: [(28, 15, 96), (34, 12, 92), (40, 16, 92), (48, 17, 94)],
    },
    LibraryShip {
        sprite: 3,
        pen_x: 200,
        lines: [(71, 18, 96), (77, 19, 92), (83, 16, 92), (91, 20, 94)],
    },
    LibraryShip {
        sprite: 4,
        pen_x: 200,
        lines: [(109, 21, 96), (115, 22, 92), (121, 16, 92), (129, 23, 94)],
    },
];

/// Library positions of all nine records (§3.2), in record order.
const LIBRARY_POS: [(i32, i32); 9] = [
    (4, 12),
    (4, 49),
    (164, 28),
    (164, 71),
    (164, 109),
    (164, 150),
    (4, 152),
    (4, 92),
    (4, 123),
];

/// Draws the ship library (`UI_ShipLibrary`, §4.1.2) for flight `flight`.
pub fn draw_ship_library(fb: &mut Framebuffer, assets: &FlightScreenAssets, flight: u8) {
    fb.clear(0);
    for (rec, &(x, y)) in assets.library.iter().zip(&LIBRARY_POS) {
        fb.blit(Image::new(rec, 32, 32), x, y, Rect::SCREEN, Some(0));
    }
    // The hatch: colour 1 where row + column is even (a one-pixel checkerboard).
    let mut hatched = vec![6, 5];
    if flight < LIBRARY_SHIP_8_FROM {
        hatched.push(8);
    }
    for r in hatched {
        let (x, y) = LIBRARY_POS[r];
        for row in 0..32 {
            for col in (0..32).filter(|c| (row + c) % 2 == 0) {
                fb.put(x + col, y + row, 1);
            }
        }
    }
    for ship in &LIBRARY {
        if ship.sprite == 8 && flight < LIBRARY_SHIP_8_FROM {
            continue;
        }
        for &(y, i, ink) in &ship.lines {
            let text = FlightScreenAssets::text(&assets.ship_library, i);
            small_text::draw_opaque(fb, &assets.font, text, (ship.pen_x, y), ink, 0);
        }
    }
}

/// Draws the menu page: black, the menu image at [`MENU_POS`].
pub fn draw_menu_page(fb: &mut Framebuffer, assets: &FlightScreenAssets) {
    fb.clear(0);
    fb.blit(
        Image::new(&assets.menu, MENU_W, MENU_H),
        MENU_POS.0,
        MENU_POS.1,
        Rect::SCREEN,
        None,
    );
}

/// A picture the menu puts in its page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Menu,
    Library,
    Help(usize),
    /// The flight frame shown before Esc (the other page).
    Flight,
}

/// One step of the menu program.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Op {
    /// A fade step per retrace: DAC entries `first..` := `target` · s / 16, s moving from
    /// `s` to `end`.
    Fade {
        target: Box<Palette>,
        s: u8,
        end: u8,
        first: usize,
    },
    /// Instant: draw a page.
    Show(Page),
    /// Instant: clear key bytes 1..600.
    ClearKeys,
    /// Highlight pulse of one item, one DAC value per retrace from `k` to `end`.
    Pulse { entry: usize, k: u8, end: u8 },
    /// `Crt.Delay(100)`, then read the keys.
    Poll { frames: u8 },
    /// Wait until any key byte is set, then clear them all.
    WaitAnyKey,
    /// The outer loop's test (`until done`).
    OuterTest,
    /// Leave the menu.
    Finish,
}

/// The Esc computer menu (`UI_FlightMenu`, tick-order step 25).
#[derive(Debug, Clone)]
pub struct ComputerMenu {
    canvas: Framebuffer,
    flight_picture: Framebuffer,
    /// `g_FlightPalette` with entries 107..110 at the menu colour (32, 0, 0).
    menu_palette: Palette,
    /// `g_FlightPalette` as the flight had it.
    flight_palette: Palette,
    saved_mode: u8,
    flight: u8,
    cursor: u8,
    shown: u8,
    done: bool,
    aborted: bool,
    keys: KeyBytes,
    ops: VecDeque<Op>,
}

fn fade_out(target: Palette) -> Op {
    Op::Fade {
        target: Box::new(target),
        s: FADE_STEPS,
        end: 0,
        first: 0,
    }
}

fn fade_in(target: Palette) -> Op {
    Op::Fade {
        target: Box::new(target),
        s: 0,
        end: FADE_STEPS,
        first: 1,
    }
}

impl ComputerMenu {
    /// Opens the menu over `shown` (the frame on screen, whose DAC is the flight's), with the
    /// flight palette `flight_palette` (P), the palette mode and the flight number.
    #[must_use]
    pub fn new(shown: Framebuffer, flight_palette: &Palette, mode: u8, flight: u8) -> Self {
        let mut menu_palette = flight_palette.clone();
        for e in FIRST_ITEM_ENTRY..FIRST_ITEM_ENTRY + usize::from(ITEMS) {
            menu_palette.colors[e] = Rgb6 {
                r: ITEM_RED_LOW,
                g: 0,
                b: 0,
            };
        }
        let mut ops = VecDeque::new();
        // FadeOut in the mode as it is; then the menu page in mode 0.
        ops.push_back(fade_out(fade_target(flight_palette, mode)));
        ops.push_back(Op::Show(Page::Menu));
        ops.push_back(fade_in(menu_palette.clone()));
        ops.push_back(Op::ClearKeys);
        let mut menu = Self {
            canvas: shown.clone(),
            flight_picture: shown,
            menu_palette,
            flight_palette: flight_palette.clone(),
            saved_mode: mode,
            flight,
            cursor: 0,
            shown: 0,
            done: false,
            aborted: false,
            keys: KeyBytes::default(),
            ops,
        };
        menu.start_outer_iteration();
        menu
    }

    /// The item under the cursor (0 return, 1 ship library, 2 help, 3 quit).
    pub fn cursor(&self) -> u8 {
        self.cursor
    }

    /// The picture and DAC on screen.
    pub fn canvas(&self) -> &Framebuffer {
        &self.canvas
    }

    /// True while the menu waits in its key-poll loop (no fade or pulse running).
    pub fn polling(&self) -> bool {
        matches!(self.ops.front(), Some(Op::Poll { .. }))
    }

    /// Outer loop head: remember the item shown, pulse it up, then poll.
    fn start_outer_iteration(&mut self) {
        self.shown = self.cursor;
        self.ops.push_back(Op::Pulse {
            entry: FIRST_ITEM_ENTRY + usize::from(self.cursor),
            k: ITEM_RED_LOW,
            end: ITEM_RED_HIGH,
        });
        self.ops.push_back(Op::Poll {
            frames: POLL_FRAMES,
        });
    }

    /// Advances one retrace.
    pub fn tick(&mut self, assets: &FlightScreenAssets, input: &TickInput) -> ModalStep {
        self.keys.update(input);
        while let Some(op) = self.ops.pop_front() {
            match op {
                Op::Fade {
                    target,
                    s,
                    end,
                    first,
                } => {
                    write_fade_step(&mut self.canvas.palette, &target, s, first);
                    if s != end {
                        let s = if s < end { s + 1 } else { s - 1 };
                        self.ops.push_front(Op::Fade {
                            target,
                            s,
                            end,
                            first,
                        });
                    }
                    return ModalStep::Continue;
                }
                Op::Show(page) => self.show(assets, page),
                Op::ClearKeys => self.keys.clear_all(),
                Op::Pulse { entry, k, end } => {
                    self.canvas.palette.colors[entry] = Rgb6 { r: k, g: 0, b: 0 };
                    if k != end {
                        let k = if k < end { k + 1 } else { k - 1 };
                        self.ops.push_front(Op::Pulse { entry, k, end });
                    }
                    return ModalStep::Continue;
                }
                Op::Poll { frames } => {
                    if frames > 1 {
                        self.ops.push_front(Op::Poll { frames: frames - 1 });
                    } else {
                        self.poll(assets, input);
                    }
                    return ModalStep::Continue;
                }
                Op::WaitAnyKey => {
                    if self.keys.any() {
                        self.keys.clear_all();
                    } else {
                        self.ops.push_front(Op::WaitAnyKey);
                        return ModalStep::Continue;
                    }
                }
                Op::OuterTest => {
                    if !self.done {
                        self.start_outer_iteration();
                    } else if self.aborted {
                        // Quit: no fade back; the flight's teardown follows.
                        self.ops.push_back(Op::Finish);
                    } else {
                        self.ops.push_back(fade_out(self.menu_palette.clone()));
                        self.ops.push_back(Op::Show(Page::Flight));
                        self.ops
                            .push_back(fade_in(fade_target(&self.flight_palette, self.saved_mode)));
                        self.ops.push_back(Op::Finish);
                    }
                }
                Op::Finish => {
                    return ModalStep::Close {
                        abort: self.aborted,
                    };
                }
            }
        }
        ModalStep::Close {
            abort: self.aborted,
        }
    }

    fn show(&mut self, assets: &FlightScreenAssets, page: Page) {
        match page {
            Page::Menu => draw_menu_page(&mut self.canvas, assets),
            Page::Library => draw_ship_library(&mut self.canvas, assets, self.flight),
            Page::Help(n) => self.canvas.copy_full(assets.help_page(n)),
            Page::Flight => self.canvas.pixels.clone_from(&self.flight_picture.pixels),
        }
        if page != Page::Flight {
            blank_below_display(&mut self.canvas);
        }
    }

    /// One poll of the inner loop (§4.1), at the end of its `Crt.Delay(100)`.
    fn poll(&mut self, assets: &FlightScreenAssets, input: &TickInput) {
        let pad = input.pad.unwrap_or_default();
        let k = &mut self.keys;
        let mut changed = false;
        let mut sub = Vec::new();
        if k.get(Key::UP) || k.get(KEY_KP8) || pad.y > 0 {
            self.cursor = (self.cursor + ITEMS - 1) % ITEMS;
            // OB-030: the code clears 30h and E0 48h, not 48h, so keypad 8 keeps repeating.
            k.clear(Key::letter('b').unwrap_or(Key::UP));
            k.clear(Key::UP);
            changed = true;
        }
        if k.get(Key::DOWN) || k.get(KEY_KP2) || pad.y < 0 {
            self.cursor = (self.cursor + 1) % ITEMS;
            // OB-030: 32h and E0 50h.
            k.clear(Key::letter('m').unwrap_or(Key::DOWN));
            k.clear(Key::DOWN);
            changed = true;
        }
        if k.get(Key::ESCAPE) {
            self.done = true;
            changed = true;
            self.cursor = 1;
        }
        let select = k.get(Key::SPACE)
            || k.get(Key::ENTER)
            || k.get(Key::KP_ENTER)
            || pad.button1
            || pad.button2;
        if select {
            changed = true;
            match self.cursor {
                3 => {
                    self.aborted = true;
                    self.done = true;
                }
                2 => {
                    let help = assets.help_palette();
                    sub.extend([
                        fade_out(self.menu_palette.clone()),
                        Op::Show(Page::Help(0)),
                        fade_in(help.clone()),
                        Op::WaitAnyKey,
                        fade_out(help.clone()),
                        Op::Show(Page::Help(1)),
                        fade_in(help.clone()),
                        Op::WaitAnyKey,
                        fade_out(help),
                        Op::Show(Page::Menu),
                        fade_in(self.menu_palette.clone()),
                    ]);
                }
                1 => sub.extend([
                    fade_out(self.menu_palette.clone()),
                    Op::Show(Page::Library),
                    fade_in(self.menu_palette.clone()),
                    Op::WaitAnyKey,
                    fade_out(self.menu_palette.clone()),
                    Op::Show(Page::Menu),
                    fade_in(self.menu_palette.clone()),
                ]),
                _ => self.done = true,
            }
        }
        if !changed {
            self.ops.push_back(Op::Poll {
                frames: POLL_FRAMES,
            });
            return;
        }
        self.ops.extend(sub);
        self.ops.push_back(Op::Pulse {
            entry: FIRST_ITEM_ENTRY + usize::from(self.shown),
            k: ITEM_RED_HIGH,
            end: ITEM_RED_LOW,
        });
        self.ops.push_back(Op::OuterTest);
    }
}
