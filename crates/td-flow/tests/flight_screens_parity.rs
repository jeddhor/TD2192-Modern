//! In-flight modal screens and IR palettes against the original data and oracle captures
//! C-0012 (E1 flight 1: computer menu, ship library, help, pause, F5 map), C-0013 (flight 2,
//! transmission 1) and C-0014 (flight 8: IR modes, notice, menu in IR).
//! Spec: `docs/spec/flow/flight-screens.md` rev 1 §6. Data-gated (TD2192_DATA; the frame
//! comparisons also TD2192_ORACLE). No captured pixels are stored.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use td_assets::{DataDir, ExeResources};
use td_flow::flight_screens::comm::{CommScreen, SCROLL_ROWS, SCROLL_X};
use td_flow::flight_screens::map::{ENTRIES, ShipDot, TacticalMap, TargetBoard};
use td_flow::flight_screens::menu::ComputerMenu;
use td_flow::flight_screens::{FlightScreenAssets, ModalStep, draw_ir_notice, draw_pause_line};
use td_formats::map::TileMap;
use td_formats::palette::{Palette, Rgb6};
use td_input::{Key, TickInput};
use td_render::Framebuffer;
use td_render::flight_palette::{fade_target, ir_palette};
use td_test_support::{require_data, require_oracle};

const W: usize = 320;

fn load(data: &Path) -> (DataDir, FlightScreenAssets, Palette) {
    let dir = DataDir::open(data).unwrap();
    let res = ExeResources::load(&dir).unwrap();
    let assets = FlightScreenAssets::load(&dir, &res).unwrap();
    let pal = Palette::parse(&dir.read("td.pal").unwrap()).unwrap();
    (dir, assets, pal)
}

/// DAC and pixels of an oracle frame (320 × 192 or 320 × 200).
fn frame(oracle: &Path, capture: &str, name: &str) -> (Palette, Vec<u8>) {
    let raw = fs::read(oracle.join(capture).join("fb").join(name)).unwrap();
    (Palette::parse_prefix(&raw).unwrap(), raw[768..].to_vec())
}

/// Pixel differences between `ours` and an oracle frame over its rows.
fn pixel_diffs(ours: &Framebuffer, theirs: &[u8]) -> Vec<(usize, usize, u8, u8)> {
    (0..theirs.len())
        .filter(|&i| ours.pixels[i] != theirs[i])
        .map(|i| (i % W, i / W, ours.pixels[i], theirs[i]))
        .collect()
}

fn assert_same_pixels(ours: &Framebuffer, theirs: &[u8], what: &str) {
    let d = pixel_diffs(ours, theirs);
    assert!(
        d.is_empty(),
        "{what}: {} pixels differ, first {:?}",
        d.len(),
        &d[..d.len().min(8)]
    );
}

fn assert_dac_of_used(ours: &Palette, dac: &Palette, pixels: &[u8], what: &str) {
    let used: BTreeSet<u8> = pixels.iter().copied().collect();
    for e in used.into_iter().map(usize::from) {
        assert_eq!(ours.colors[e], dac.colors[e], "{what}: DAC entry {e}");
    }
}

fn idle() -> TickInput {
    TickInput::default()
}

/// A key held for `frames` retraces (pressed on the first), then released.
fn hold(key: Key, frames: usize) -> Vec<TickInput> {
    let mut v = Vec::new();
    for f in 0..frames {
        let mut i = TickInput::default();
        i.held.insert(key);
        if f == 0 {
            i.pressed.push(key);
        }
        v.push(i);
    }
    v.push(TickInput::default());
    v
}

fn run_menu_until(
    m: &mut ComputerMenu,
    a: &FlightScreenAssets,
    inputs: &[TickInput],
    until: impl Fn(&ComputerMenu) -> bool,
) -> ModalStep {
    for i in inputs {
        let step = m.tick(a, i);
        if step != ModalStep::Continue {
            return step;
        }
    }
    for _ in 0..2_000 {
        if until(m) {
            return ModalStep::Continue;
        }
        let step = m.tick(a, &idle());
        if step != ModalStep::Continue {
            return step;
        }
    }
    panic!("menu did not reach the expected state");
}

#[test]
fn computer_menu_library_and_help_match_c0012() {
    let data = require_data!();
    let oracle = require_oracle!();
    let (_dir, a, pal) = load(&data);
    let mut shown = Framebuffer::new();
    shown.palette = pal.clone();
    shown.pixels.fill(7);
    let mut m = ComputerMenu::new(shown.clone(), &pal, 0, 1);
    run_menu_until(&mut m, &a, &[], ComputerMenu::polling);
    assert_eq!(m.cursor(), 0);
    // Test vector 1 (f3844): Down; the menu on black, DAC 107..110 with item 1 lit.
    run_menu_until(&mut m, &a, &hold(Key::DOWN, 8), |m| {
        m.polling() && m.cursor() == 1
    });
    let (dac, f) = frame(&oracle, "C-0012", "f00003844_320x192_menu1.vga");
    assert_same_pixels(m.canvas(), &f, "menu");
    let red = |r| Rgb6 { r, g: 0, b: 0 };
    assert_eq!(
        &m.canvas().palette.colors[107..111],
        &[red(32), red(63), red(32), red(32)]
    );
    assert_dac_of_used(&m.canvas().palette, &dac, &f, "menu");
    // Test vector 2 (f4184): Enter opens the ship library (flight 1: sprites 5, 6, 8 hatched).
    run_menu_until(
        &mut m,
        &a,
        &hold(Key::ENTER, 8),
        ComputerMenu::waiting_for_key,
    );
    let (_, f) = frame(&oracle, "C-0012", "f00004184_320x192_menu1b.vga");
    assert_same_pixels(m.canvas(), &f, "ship library");
    assert_dac_of_used(&m.canvas().palette, &pal, &f[..W * 120], "ship library");
    // Back to the menu, Down to help, Enter: help pages 1 and 2 (test vector 3).
    run_menu_until(&mut m, &a, &hold(Key::SPACE, 4), ComputerMenu::polling);
    assert_eq!(m.cursor(), 1);
    run_menu_until(&mut m, &a, &hold(Key::DOWN, 8), |m| {
        m.polling() && m.cursor() == 2
    });
    let help = Palette::parse_prefix(&a.help).unwrap();
    for (page, name) in [
        (0, "f00004824_320x192_help2.vga"),
        (1, "f00005124_320x192_menu2b.vga"),
    ] {
        let keys = if page == 0 {
            hold(Key::ENTER, 8)
        } else {
            hold(Key::SPACE, 4)
        };
        run_menu_until(&mut m, &a, &keys, ComputerMenu::waiting_for_key);
        let (_, f) = frame(&oracle, "C-0012", name);
        assert_same_pixels(m.canvas(), &f, name);
        assert_dac_of_used(&m.canvas().palette, &help, &f, name);
    }
    run_menu_until(&mut m, &a, &hold(Key::SPACE, 4), ComputerMenu::polling);
    assert_eq!(m.cursor(), 2);
    // Up, Up (f5514): item 0 lit.
    run_menu_until(&mut m, &a, &hold(Key::UP, 8), |m| {
        m.polling() && m.cursor() == 1
    });
    run_menu_until(&mut m, &a, &hold(Key::UP, 8), |m| {
        m.polling() && m.cursor() == 0
    });
    let (dac, f) = frame(&oracle, "C-0012", "f00005514_320x192_menu0b.vga");
    assert_same_pixels(m.canvas(), &f, "menu, item 0");
    assert_dac_of_used(&m.canvas().palette, &dac, &f, "menu, item 0");
    assert_eq!(m.canvas().palette.colors[107], red(63));
    // Return to game: the flight picture comes back in the flight palette.
    let step = run_menu_until(&mut m, &a, &hold(Key::ENTER, 8), |_| false);
    assert_eq!(step, ModalStep::Close { abort: false });
    assert_eq!(m.canvas().pixels, shown.pixels);
    assert_eq!(&m.canvas().palette.colors[1..], &pal.colors[1..]);
}

#[test]
fn menu_fades_take_two_frames_per_level_and_the_ramp_eleven_steps_per_frame() {
    // BUG-8, flight-screens rev 5 §4.5.3 and §3.5 (C-0012), M12-14: 17 levels two frames
    // apart, level 0 one, each shown from the frame after its last write. The fade-out's
    // levels are complete in the frames 3, 5, …, 33, 34 after the Esc tick (C-0012 writes
    // f3555.92..3588.1: f3558, 3560, …, 3588, 3589), so black in the 34th; the fade-in's
    // levels follow 1, 3, …, 33 frames later (C-0012 f3595.92..3628.44, 7 frames later than
    // here: the original reads and draws the menu page in f3589..3594, instant here); item 0's
    // ramp 32..63 starts in the frame of the last level and runs 11, 11 and 10 steps
    // (f3628..3630).
    let data = require_data!();
    let (_dir, a, pal) = load(&data);
    let mut shown = Framebuffer::new();
    shown.palette = pal.clone();
    let mut m = ComputerMenu::new(shown, &pal, 0, 1);
    let black = |p: &Palette| p.colors.iter().all(|c| *c == Rgb6::default());
    let mut frames = Vec::new();
    while !m.polling() {
        assert_eq!(m.tick(&a, &idle()), ModalStep::Continue);
        frames.push(m.canvas().palette.clone());
        assert!(frames.len() < 200);
    }
    let out = frames.iter().position(black).unwrap() + 1;
    assert_eq!(out, 34, "fade-out: level 0 in the 34th frame");
    let first_change = frames.iter().position(|p| *p != pal).unwrap() + 1;
    assert_eq!(
        first_change, 5,
        "level 16 (frame 3) leaves the DAC as it was"
    );
    let menu_pal = {
        let mut p = pal.clone();
        for e in 107..111 {
            p.colors[e] = Rgb6 { r: 32, g: 0, b: 0 };
        }
        p
    };
    let full = frames[out..]
        .iter()
        .position(|p| p.colors[1..107] == menu_pal.colors[1..107])
        .unwrap()
        + out
        + 1;
    assert_eq!(
        full - out,
        33,
        "fade-in: level 0 one frame after the fade-out, 16 levels on"
    );
    let ramp: Vec<u8> = frames[full - 1..].iter().map(|p| p.colors[107].r).collect();
    assert_eq!(ramp, [42, 53, 63], "11, 11 and 10 ramp steps");
}

#[test]
fn quit_aborts_without_fading_back() {
    let data = require_data!();
    let (_dir, a, pal) = load(&data);
    let mut m = ComputerMenu::new(Framebuffer::new(), &pal, 0, 1);
    run_menu_until(&mut m, &a, &[], ComputerMenu::polling);
    run_menu_until(&mut m, &a, &hold(Key::UP, 8), |m| {
        m.polling() && m.cursor() == 3
    });
    let step = run_menu_until(&mut m, &a, &hold(Key::SPACE, 8), |_| false);
    assert_eq!(step, ModalStep::Close { abort: true });
    // The menu stays on screen (no fade back to the flight).
    assert_eq!(m.canvas().pixels[50 * W + 80], a.menu[0]);
}

/// Ink-37 text over a sentinel canvas: every pixel we draw matches the oracle, and every
/// ink-37 pixel of the oracle inside the text's box is ours.
fn check_notice(ours: &Framebuffer, theirs: &[u8], what: &str) {
    const SENTINEL: u8 = 255;
    let mut box_x = (W, 0);
    for (i, &p) in ours.pixels.iter().enumerate() {
        if p != SENTINEL {
            assert_eq!(theirs[i], p, "{what}: ({}, {})", i % W, i / W);
            if p == 37 {
                box_x = (box_x.0.min(i % W), box_x.1.max(i % W));
            }
        }
    }
    for y in 77..=81 {
        for x in box_x.0..=box_x.1 {
            if theirs[y * W + x] == 37 {
                assert_eq!(ours.pixels[y * W + x], 37, "{what}: ({x}, {y}) missing");
            }
        }
    }
}

#[test]
fn pause_line_and_ir_notice_match_c0012_c0014() {
    let data = require_data!();
    let oracle = require_oracle!();
    let (_dir, a, _pal) = load(&data);
    // Test vector 4 (f5724): ink 37 at x 102..242, rows 77..81, black outline.
    let mut fb = Framebuffer::new();
    fb.pixels.fill(255);
    draw_pause_line(&mut fb, &a);
    let (_, f) = frame(&oracle, "C-0012", "f00005724_320x192_pause.vga");
    check_notice(&fb, &f, "pause");
    let xs: Vec<usize> = (0..W * 192)
        .filter(|&i| fb.pixels[i] == 37)
        .map(|i| i % W)
        .collect();
    assert_eq!((xs.iter().min(), xs.iter().max()), (Some(&102), Some(&242)));
    // Test vector 15 (C-0014 f1522): the same line in mode 1.
    let (_, f) = frame(&oracle, "C-0014", "f00001522_320x192_pause.vga");
    check_notice(&fb, &f, "pause in IR");
    // Test vector 13 (f522): the IR notice, x 106..232.
    let mut fb = Framebuffer::new();
    fb.pixels.fill(255);
    draw_ir_notice(&mut fb, &a);
    let (_, f) = frame(&oracle, "C-0014", "f00000522_320x192_ir.vga");
    check_notice(&fb, &f, "IR notice");
    let xs: Vec<usize> = (0..W * 192)
        .filter(|&i| fb.pixels[i] == 37)
        .map(|i| i % W)
        .collect();
    assert_eq!((xs.iter().min(), xs.iter().max()), (Some(&106), Some(&232)));
}

/// DAC entries that differ from `want`.
fn dac_diffs(dac: &Palette, want: &Palette) -> Vec<usize> {
    (0..256)
        .filter(|&i| dac.colors[i] != want.colors[i])
        .collect()
}

#[test]
fn ir_palettes_match_c0014() {
    let data = require_data!();
    let oracle = require_oracle!();
    let (_dir, _a, pal) = load(&data);
    // The flight patches some entries of its palette (HUD lamps and indicators, hud.md);
    // those are the only differences allowed (test vector 12; 85h, 86h, 90h, C1h are the
    // HUD entries of U-225).
    let patched = |d: &[usize]| {
        d.iter()
            .all(|i| matches!(i, 36 | 38 | 46..=49 | 0x85 | 0x86 | 0x90 | 0xC1))
    };
    for (name, want, what) in [
        (
            "f00000522_320x192_ir.vga",
            ir_palette(&pal, 1),
            "start, mode 1",
        ),
        (
            "f00001322_320x192_m2.vga",
            ir_palette(&pal, 2),
            "Alt, mode 2",
        ),
        (
            "f00001422_320x192_m1.vga",
            ir_palette(&pal, 1),
            "Alt, mode 1",
        ),
        (
            "f00001982_320x192_back.vga",
            fade_target(&pal, 1),
            "after the menu",
        ),
    ] {
        let (dac, _) = frame(&oracle, "C-0014", name);
        let d = dac_diffs(&dac, &want);
        assert!(patched(&d), "{what}: entries {d:?} differ");
    }
    // Test vector 16: entry 37 after the menu is the dim (8, 6, 8), not P[255] (OB-029).
    let (dac, _) = frame(&oracle, "C-0014", "f00001982_320x192_back.vga");
    assert_eq!(dac.colors[37], Rgb6 { r: 8, g: 6, b: 8 });
    assert_eq!(ir_palette(&pal, 1).colors[37], pal.colors[255]);
}

#[test]
fn ir_menu_shows_the_normal_palette_c0014() {
    let data = require_data!();
    let oracle = require_oracle!();
    let (_dir, a, pal) = load(&data);
    let mut shown = Framebuffer::new();
    shown.palette = ir_palette(&pal, 1);
    let mut m = ComputerMenu::new(shown, &pal, 1, 8);
    run_menu_until(&mut m, &a, &[], ComputerMenu::polling);
    let (dac, f) = frame(&oracle, "C-0014", "f00001822_320x192_menu.vga");
    assert_same_pixels(m.canvas(), &f, "menu in IR");
    assert_dac_of_used(&m.canvas().palette, &dac, &f, "menu in IR");
}

/// u8 / i16 reads from a DGROUP dump.
fn ds_u8(ram: &[u8], off: usize) -> u8 {
    ram[off]
}

fn ds_i16(ram: &[u8], off: usize) -> i32 {
    i32::from(i16::from_le_bytes([ram[off], ram[off + 1]]))
}

#[test]
fn tactical_map_matches_c0012() {
    let data = require_data!();
    let oracle = require_oracle!();
    let (dir, a, _pal) = load(&data);
    let map = TileMap::parse_episode(&dir.read("td.map").unwrap()).unwrap();
    let ram = fs::read(
        oracle
            .join("C-0012")
            .join("mem")
            .join("f00006004_4FEB-0000_9DF0_map.bin"),
    )
    .unwrap();
    // Entries as the map set them (DS:1F74 flags, DS:1F94 X, DS:1FB4 Y), the slot classes
    // (DS:3A7A) and the view tile (DS:3A36, DS:3A38).
    let flag = |e: usize| ds_u8(&ram, 0x1F74 + e) != 0;
    let pos = |e: usize| (ds_u8(&ram, 0x1F94 + e), ds_u8(&ram, 0x1FB4 + e));
    let view = (ds_i16(&ram, 0x3A36), ds_i16(&ram, 0x3A38));
    assert_eq!(pos(0), ((view.0 + 5) as u8, (view.1 + 3) as u8));
    let ships: Vec<ShipDot> = (1..=20)
        .map(|s| ShipDot {
            class: ds_u8(&ram, 0x3A7A + s - 1),
            tile: pos(s),
            marked: flag(s),
        })
        .collect();
    // Test vector 5: the flight-1 entries; the lines in RAM are e1.target_info strings.
    let mut board = TargetBoard::default();
    board.load(1);
    assert!(flag(1) && flag(2) && flag(21) && pos(21) == (107, 104));
    for e in 0..ENTRIES {
        for l in 0..5 {
            let at = 0x1FD4 + 155 * e + 31 * l;
            let len = usize::from(ram[at]);
            let theirs: String = ram[at + 1..at + 1 + len]
                .iter()
                .map(|&b| char::from(b))
                .collect();
            let ours: String = board.lines[e].map_or(String::new(), |idx| {
                a.target_info[usize::from(idx[l])]
                    .chars()
                    .take(30)
                    .collect()
            });
            assert_eq!(ours, theirs, "entry {e} line {l}");
        }
    }
    let (dac, f0) = frame(&oracle, "C-0012", "f00005914_320x200_map0.vga");
    let mut m = TacticalMap::new(&dac, &a, &map, view, &ships, &board);
    // f5914 caught the map while it was drawn: rows 0..91 are done (no dots yet), the rest
    // is still black. Tile (x, y) is at (x, y - 1) in its radar colour.
    for y in 0..92 {
        for x in 0..200 {
            let want = map
                .get(x, y + 1)
                .filter(|_| x > 0)
                .map_or(0, |t| a.radar_colours[usize::from(t)]);
            assert_eq!(f0[y * W + x], want, "map ({x}, {y})");
        }
    }
    let tab = TickInput {
        pressed: vec![Key::TAB],
        ..TickInput::default()
    };
    for name in ["f00006024_320x200_map1.vga", "f00006104_320x200_map2.vga"] {
        assert_eq!(m.tick(&a, &tab), ModalStep::Continue);
        let (_, f) = frame(&oracle, "C-0012", name);
        assert_same_pixels(m.canvas(), &f, name);
    }
    let esc = TickInput {
        pressed: vec![Key::ESCAPE],
        ..TickInput::default()
    };
    assert_eq!(m.tick(&a, &esc), ModalStep::Close { abort: false });
}

/// The panel's text area: headers and both boxes, x 92..271.
fn text_area(fb: &[u8]) -> Vec<u8> {
    let rows = (35..=40).chain(SCROLL_ROWS[0].0..=SCROLL_ROWS[0].1);
    let rows = rows
        .chain(87..=92)
        .chain(SCROLL_ROWS[1].0..=SCROLL_ROWS[1].1);
    rows.flat_map(|y| fb[y * W + SCROLL_X.0..=y * W + SCROLL_X.1].to_vec())
        .collect()
}

#[test]
fn transmission_screen_matches_c0013() {
    let data = require_data!();
    let oracle = require_oracle!();
    let (_dir, a, pal) = load(&data);
    let mut shown = Framebuffer::new();
    shown.palette = pal.clone();
    let mut c = CommScreen::new(shown, &a, 1).unwrap();
    // Test vector 9: panel cells and portraits (records 1 and 0) over the playfield.
    let (dac, f) = frame(&oracle, "C-0013", "f00000584_320x192_c.vga");
    let mut panel = 0;
    for y in 16..144 {
        for x in 32..288 {
            let p = c.canvas().pixels[y * W + x];
            let in_text = (92..=271).contains(&x) && (35..=124).contains(&y);
            if p != 0 && !in_text {
                assert_eq!(f[y * W + x], p, "panel ({x}, {y})");
                panel += 1;
            }
        }
    }
    assert!(panel > 10_000);
    for e in [40, 43, 45, 178] {
        assert_eq!(dac.colors[e], pal.colors[e], "DAC {e} = td.pal");
    }
    // Test vectors 10, 11: at each wait (two prompts, then the end marker) the text area
    // equals a captured frame.
    let frames: Vec<Vec<u8>> = {
        let mut names: Vec<_> = fs::read_dir(oracle.join("C-0013").join("fb"))
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.file_name().into_string().unwrap()))
            .filter(|n| n.ends_with(".vga"))
            .collect();
        names.sort();
        names
            .iter()
            .map(|n| text_area(&frame(&oracle, "C-0013", n).1))
            .collect()
    };
    let space = TickInput {
        pressed: vec![Key::SPACE],
        ..TickInput::default()
    };
    let mut waits = 0;
    for _ in 0..5_000 {
        if c.waiting_for_space() || c.waiting_to_close() {
            let ours = text_area(&c.canvas().pixels);
            assert!(
                frames.contains(&ours),
                "wait {waits}: no captured frame shows this text"
            );
            waits += 1;
            if c.waiting_to_close() {
                break;
            }
            c.tick(&a, &space);
        } else {
            c.tick(&a, &idle());
        }
    }
    assert_eq!(waits, 3);
    let f6 = TickInput {
        pressed: vec![Key::F6],
        ..TickInput::default()
    };
    assert_eq!(c.tick(&a, &f6), ModalStep::Close { abort: false });
}

#[test]
fn transmission_markers_are_the_prompt_strings() {
    // §3.3: the marker slots of each transmission hold the two comm.prompts strings.
    let data = require_data!();
    let (_dir, a, _pal) = load(&data);
    assert_eq!(a.comms[0][7], a.comm_prompts[0]);
    assert_eq!(a.comms[0][16], a.comm_prompts[1]);
    for (n, strings) in a.comms.iter().enumerate() {
        assert_eq!(
            strings.last(),
            a.comm_prompts.get(1),
            "transmission {} ends with the closing line",
            n + 1
        );
    }
}
