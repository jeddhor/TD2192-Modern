//! L5 parity: the in-flight instruments rebuilt from the RAM state of captures of the original
//! and compared with the frames captured at the same moment (`docs/spec/render/hud.md` §6):
//! the HUD bar (rows 160..191), the palette entries the instruments animate, and the guidance
//! dots in the playfield. C-0002 f3543 (E1 flight 1 at rest, full memory dump) and every
//! 35th frame of C-0010 (flight 1 objective sequence, DGROUP DS:1700..3CAF dumps).
//! Needs TD2192_DATA and TD2192_ORACLE; skips otherwise. No captured pixels are stored here.

use std::fs;
use std::path::Path;

use td_assets::{DataDir, ExeResources, ResourceValue};
use td_formats::map::TileMap;
use td_formats::sprites::SpriteSheet;
use td_formats::tileset::Tileset;
use td_render::Framebuffer;
use td_render::flight::{self, PLAYFIELD, ViewPos};
use td_render::hud::{Hud, HudAssets, HudState, INK_MESSAGE, SlotMark, TickerFont};
use td_test_support::{require_data, require_oracle};

/// Runtime DGROUP segment of `file0001.exe` in the captures (C-0009, C-0010).
const DGROUP: usize = 0x4FEB0;

/// A view of DGROUP: `base` is the DS offset of `bytes[0]`.
struct Ds<'a> {
    bytes: &'a [u8],
    base: usize,
}

impl Ds<'_> {
    fn b(&self, off: usize) -> u8 {
        self.bytes[off - self.base]
    }
    fn w(&self, off: usize) -> u16 {
        u16::from_le_bytes([self.b(off), self.b(off + 1)])
    }
}

/// The step-19 state from the globals (hud.md §3.3, globals.md §6). Projectile positions live
/// on the heap and are passed in.
fn state(ds: &Ds<'_>, shots: [(u8, u8); 51]) -> HudState {
    let mut slots = [SlotMark::EMPTY; 20];
    for (i, m) in slots.iter_mut().enumerate() {
        *m = SlotMark {
            class: ds.b(0x3A7A + i),
            tile_x: ds.b(0x3B2E + i),
            tile_y: ds.b(0x3B42 + i),
            marker: ds.b(0x3958 + i),
        };
    }
    let ticker = ds.w(0x32BC);
    HudState {
        episode: td_sim::flight::episode::Episode::E1,
        flight: 1,
        // Mission ticks only drive the low-armour pulse; armour stays above 6 in these runs.
        ticks: 0,
        view: ViewPos {
            tile_x: i32::from(ds.w(0x3A36) as i16),
            tile_y: i32::from(ds.w(0x3A38) as i16),
            sub_x: i32::from(ds.w(0x3A3A) as i16),
            sub_y: i32::from(ds.w(0x3A3C) as i16),
        },
        armour: ds.b(0x398E) as i8,
        shields: ds.b(0x398F) as i8,
        missiles: ds.b(0x1CB9),
        missile_cooldown: ds.b(0x1CBA),
        weapon: ds.b(0x1CB8),
        palette_mode: ds.b(0x1704),
        slots,
        shots,
        home_marker: ds.b(0x396C),
        phase: ds.b(0x32BA),
        // The window shows the last blit: the ticker is incremented after it, and returns to 0
        // after blitting position 78. At 1 the message was posted but not blitted yet.
        ticker_blit: match ticker {
            0 => Some(78),
            1 => None,
            t => Some(t - 1),
        },
        comm_phase: ds.b(0x1705),
    }
}

struct Setup {
    assets: HudAssets,
    map: TileMap,
    objective: String,
    messages: Vec<String>,
}

fn setup(data: &Path) -> Setup {
    let dir = DataDir::open(data).unwrap();
    let res = ExeResources::load(&dir).unwrap();
    let Some(ResourceValue::Bytes(radar)) = res.get("hud.radar_tile_colours").cloned() else {
        panic!("radar colour table missing");
    };
    let read = |n: &str| dir.read(n).unwrap();
    Setup {
        assets: HudAssets::new(
            read("td.win"),
            read("td.wea"),
            read("td.fac"),
            &read("td.lt2"),
            radar,
        )
        .unwrap(),
        map: TileMap::parse_episode(&read("td.map")).unwrap(),
        objective: res.texts("e1.objectives").unwrap()[0].clone(),
        messages: res.texts("e1.flight_messages.01").unwrap().to_vec(),
    }
}

/// A frame file: 768 DAC bytes, then 320 × 192 pixels.
fn frame(path: &Path) -> (Vec<u8>, Vec<u8>) {
    let bytes = fs::read(path).unwrap();
    (bytes[..768].to_vec(), bytes[768..].to_vec())
}

/// Differing pixels of rows 160..191, by element, skipping pixels `skip` accepts.
fn hud_diff(fb: &Framebuffer, px: &[u8], skip: impl Fn(usize, usize, u8) -> bool) -> usize {
    let mut n = 0;
    for y in 0..32 {
        for x in 0..320 {
            let i = (160 + y) * 320 + x;
            if fb.pixels[i] != px[i] && !skip(x, y, px[i]) {
                n += 1;
            }
        }
    }
    n
}

/// Palette entries set by the instruments that differ from the captured DAC.
fn dac_diff(hud: &Hud, dac: &[u8]) -> Vec<(u8, [u8; 3], [u8; 3])> {
    let mut entries: Vec<(u8, [u8; 3])> = Vec::new();
    for &(e, rgb) in hud.entries() {
        // Later writes to the same entry win.
        entries.retain(|x| x.0 != e);
        entries.push((e, rgb.map(|v| v & 63)));
    }
    entries
        .into_iter()
        .filter_map(|(e, rgb)| {
            let i = 3 * usize::from(e);
            let got = [dac[i], dac[i + 1], dac[i + 2]];
            (got != rgb).then_some((e, rgb, got))
        })
        .collect()
}

/// Guidance pixels whose final colour differs from the captured playfield.
fn dots_diff(hud: &Hud, px: &[u8]) -> usize {
    let mut layer = std::collections::BTreeMap::new();
    for &(x, y, c) in hud.dots() {
        layer.insert((x, y), c);
    }
    layer
        .iter()
        .filter(|&(&(x, y), &c)| px[(y * 320 + x) as usize] != c)
        .count()
}

#[test]
fn c0002_f3543_instruments_match() {
    let data = require_data!();
    let oracle = require_oracle!();
    let s = setup(&data);
    let cap = oracle.join("C-0002");
    let mem = fs::read(
        cap.join("mem")
            .join("f00003543_0000-0000_A0000_flight-1s.bin"),
    )
    .unwrap();
    let ds = Ds {
        bytes: &mem[DGROUP..DGROUP + 0x10000],
        base: 0,
    };
    // Projectile tile X / Y arrays: far pointers at DS:3986 and DS:398A (structures.md §5).
    let array = |ptr: usize| {
        let lin = usize::from(ds.w(ptr + 2)) * 16 + usize::from(ds.w(ptr));
        &mem[lin..lin + 51]
    };
    let mut shots = [(0xFF, 0); 51];
    for (i, sh) in shots.iter_mut().enumerate() {
        *sh = (array(0x3986)[i], array(0x398A)[i]);
    }
    let st = state(&ds, shots);
    assert_eq!((st.armour, st.shields, st.missiles), (25, 25, 4));

    let mut hud = Hud::new(&s.assets);
    hud.post(&s.assets, &s.objective, INK_MESSAGE, TickerFont::Flight);
    hud.step(&s.assets, &s.map, &st);

    // The playfield as flight_parity.rs draws it, then the instruments.
    let tiles = Tileset::parse(&fs::read(data.join("td.blk")).unwrap()).unwrap();
    let shp = SpriteSheet::parse_base(&fs::read(data.join("td.shp")).unwrap()).unwrap();
    let mut fb = Framebuffer::new();
    flight::draw_tile_window(&mut fb, &s.map, &tiles, st.view);
    flight::draw_player_ship(&mut fb, shp.frame(4).unwrap(), 32);
    flight::redraw_overpasses(&mut fb, &s.map, &tiles, st.view);
    hud.draw(&mut fb);

    let (dac, px) = frame(&cap.join("fb").join("f00003543_320x192_flight.vga"));
    assert_eq!(hud_diff(&fb, &px, |_, _, _| false), 0, "HUD rows 160..191");
    assert_eq!(dac_diff(&hud, &dac), vec![], "animated palette entries");
    let playfield = (0..PLAYFIELD.h as usize * 320)
        .filter(|&i| fb.pixels[i] != px[i])
        .count();
    assert_eq!(playfield, 0, "playfield with the guidance dots");
}

/// HUD elements (x range, y range) checked one by one in C-0010.
/// Name, x range and y range (half open).
type Element = (&'static str, (usize, usize), (usize, usize));

const ELEMENTS: [Element; 7] = [
    ("shields", (6, 64), (11, 15)),
    ("armour", (6, 64), (23, 27)),
    ("missiles", (100, 104), (2, 30)),
    ("radar", (148, 168), (6, 26)),
    ("weapon", (179, 215), (7, 25)),
    ("f6", (220, 248), (2, 30)),
    ("ticker", (256, 312), (13, 18)),
];

/// Pixels of one HUD element that differ: (x, y, model, capture).
fn element_diff(
    fb: &Framebuffer,
    px: &[u8],
    (x0, x1): (usize, usize),
    (y0, y1): (usize, usize),
) -> Vec<(usize, usize, u8, u8)> {
    let mut out = Vec::new();
    for y in y0..y1 {
        for x in x0..x1 {
            let i = (160 + y) * 320 + x;
            if fb.pixels[i] != px[i] {
                out.push((x, y, fb.pixels[i], px[i]));
            }
        }
    }
    out
}

/// The capture's timing: the HUD rows are captured at a moment within the frame, the DGROUP
/// dump at another, so a tick can fall between them (C-0010 dumps only every 35th frame).
/// The instruments are drawn straight into visible video memory, so a capture can also catch
/// one half drawn. An element that differs from the dump's state must then show a state one
/// tick away: the bars and the missile column another valid value (or half of each), the radar
/// ships (55h) or shots (22h) elsewhere, the ticker window the page-0 position before or after.
/// Everything else (the rest of the bar, the weapon icon, the F6 lamp, the palette entries)
/// must match every frame.
#[test]
fn c0010_instruments_match_through_the_objective_sequence() {
    let data = require_data!();
    let oracle = require_oracle!();
    let s = setup(&data);
    let cap = oracle.join("C-0010");
    let mut frames = 0;
    let mut exact = [0usize; ELEMENTS.len()];
    let mut timing = [0usize; ELEMENTS.len()];
    let (mut other_px, mut dac_bad, mut dot_frames) = (0, 0, 0);
    let mut failures = Vec::new();
    for f in (3475..=30050).step_by(35) {
        let Ok(mem) = fs::read(
            cap.join("mem")
                .join(format!("f{f:08}_4FEB-1700_25B0_ds.bin")),
        ) else {
            continue;
        };
        let ds = Ds {
            bytes: &mem,
            base: 0x1700,
        };
        let st = state(&ds, [(0xFF, 0); 51]);
        // The strip holds every text posted so far, each drawn over the last: the objective,
        // then one message per phase step (mission-logic §5.1).
        let mut hud = Hud::new(&s.assets);
        hud.post(&s.assets, &s.objective, INK_MESSAGE, TickerFont::Flight);
        let posted = match st.phase {
            100 => 5,
            p => usize::from(p.saturating_sub(1)),
        };
        for m in &s.messages[..posted] {
            hud.post(&s.assets, m, INK_MESSAGE, TickerFont::Mission);
        }
        let model = |hud: &Hud, st: &HudState| {
            let mut hud = hud.clone();
            hud.step(&s.assets, &s.map, st);
            let mut fb = Framebuffer::new();
            hud.draw(&mut fb);
            (hud, fb)
        };
        let (stepped, fb) = model(&hud, &st);
        let (dac, px) = frame(&cap.join("fb").join(format!("f{f:08}_320x192_hud.vga")));
        frames += 1;
        other_px += hud_diff(&fb, &px, |x, y, _| {
            ELEMENTS
                .iter()
                .any(|(_, xs, ys)| (xs.0..xs.1).contains(&x) && (ys.0..ys.1).contains(&y))
        });
        dac_bad += dac_diff(&stepped, &dac).len();
        dot_frames += usize::from(dots_diff(&stepped, &px) > 0);
        for (k, &(name, xs, ys)) in ELEMENTS.iter().enumerate() {
            let d = element_diff(&fb, &px, xs, ys);
            if d.is_empty() {
                exact[k] += 1;
                continue;
            }
            // The element as drawn for another value `v` (a tick before or after the dump), or
            // caught while being redrawn: every pixel as drawn for the dump's value or for `v`
            // (C-0010 f22620 shows the shield bar going from 15 to 25 row by row).
            let other_state_matches =
                |alt: &mut dyn FnMut(&mut HudState, i32), range: std::ops::RangeInclusive<i32>| {
                    range.into_iter().any(|v| {
                        let mut st2 = st.clone();
                        alt(&mut st2, v);
                        let old = model(&hud, &st2).1;
                        (ys.0..ys.1).all(|y| {
                            (xs.0..xs.1).all(|x| {
                                let i = (160 + y) * 320 + x;
                                px[i] == fb.pixels[i] || px[i] == old.pixels[i]
                            })
                        })
                    })
                };
            let ticker = ds.w(0x32BC);
            // The page-0 blit before the dump's last one, and the one after it.
            let around = if ticker == 0 {
                77..=77
            } else {
                i32::from(ticker.max(2) - 2)..=i32::from(ticker.min(78))
            };
            let ok = match name {
                "shields" => other_state_matches(&mut |st, v| st.shields = v as i8, -1..=28),
                "armour" => other_state_matches(&mut |st, v| st.armour = v as i8, -1..=28),
                "missiles" => {
                    other_state_matches(&mut |st, v| st.missiles = v as u8, 0..=27)
                        // Caught between two overlapping `Fill4x2` blocks: one row shows the
                        // other column colour.
                        || (d.len() <= 4
                            && d.iter().all(|&(_, y, m, c)| {
                                y == d[0].1 && [m, c].iter().all(|v| [0x26, 0x27].contains(v))
                            }))
                }
                "radar" => d
                    .iter()
                    .all(|&(_, _, m, c)| [m, c].iter().any(|v| [0x55, 0x22].contains(v))),
                // Posted this tick, not blitted yet: the window shows the previous text.
                "ticker" if ticker == 1 => true,
                "ticker" => {
                    other_state_matches(&mut |st, v| st.ticker_blit = Some(v as u16), around)
                }
                _ => false,
            };
            if ok {
                timing[k] += 1;
            } else {
                failures.push((f, name, d.len()));
            }
        }
    }
    for (k, (name, _, _)) in ELEMENTS.iter().enumerate() {
        eprintln!(
            "C-0010 {name}: exact in {} of {frames} frames, a tick off or half drawn in {}",
            exact[k], timing[k]
        );
    }
    eprintln!(
        "C-0010: other HUD pixels differing {other_px}; palette entries differing {dac_bad}; \
         frames whose guidance dots differ (the playfield shows the previous tick's page) \
         {dot_frames}"
    );
    assert!(frames > 700);
    assert_eq!(failures, vec![], "HUD elements (frame, element, pixels)");
    assert_eq!(other_px, 0, "HUD pixels outside the elements");
    assert_eq!(dac_bad, 0, "animated palette entries");
}

/// Bracket pixels (screen x, y, colour) of the marked ships a C-0009 dump would draw.
fn brackets_from_dump(mem: &[u8]) -> Vec<(i32, i32, u8)> {
    let ds = Ds {
        bytes: mem,
        base: 0,
    };
    let view = ViewPos {
        tile_x: i32::from(ds.w(0x3A36) as i16),
        tile_y: i32::from(ds.w(0x3A38) as i16),
        sub_x: i32::from(ds.w(0x3A3A) as i16),
        sub_y: i32::from(ds.w(0x3A3C) as i16),
    };
    // The dump is taken during the tick wait, before the page toggle (tick-order step 24).
    let page = i32::from(ds.b(0x3A30));
    let mut out = Vec::new();
    for i in 0..20 {
        let marker = ds.b(0x3958 + i);
        if ds.b(0x3A7A + i) == 0xFF || marker == 0 {
            continue;
        }
        let mark = flight::ShipMark {
            tile_x: ds.b(0x3B2E + i),
            tile_y: ds.b(0x3B42 + i),
            sub_x: ds.w(0x3ACA + 2 * i) as i16,
            sub_y: ds.w(0x3AF2 + 2 * i) as i16,
            marker,
        };
        let Some(pos) = flight::mission_ship_pos(view, page, &mark) else {
            continue;
        };
        let mut fb = Framebuffer::new();
        fb.clear(0xFF);
        flight::draw_mission_ship(&mut fb, None, pos, marker);
        for y in 0..PLAYFIELD.h {
            for x in 0..320 {
                if let Some(c) = fb.get(x, y).filter(|&c| c != 0xFF) {
                    out.push((x, y, c));
                }
            }
        }
    }
    out
}

/// Mission-ship placement and target brackets (`mission-ships.md` §4.7 with the row reading of
/// U-127) against C-0009 (E1 flight 1, no input; DGROUP dumped every frame, the screen every
/// 35th). A frame shows the page drawn by a tick whose state is in one of the dumps of the few
/// frames before it (a tick lasts about 3.4 frames): for one of those dumps, every bracket
/// pixel of every marked ship inside the drawing window must be in the capture.
#[test]
fn c0009_mission_ship_brackets_match() {
    let oracle = require_oracle!();
    let cap = oracle.join("C-0009");
    let (mut frames, mut with_brackets) = (0, 0);
    for f in (3480..=4600).step_by(35) {
        let Ok(bytes) = fs::read(cap.join("fb").join(format!("f{f:08}_320x192_ms.vga"))) else {
            continue;
        };
        let px = &bytes[768..];
        frames += 1;
        let found = (f - 6..=f)
            .filter_map(|g| {
                let name = format!("f{g:08}_4FEB-0000_9DF0_ds.bin");
                fs::read(cap.join("mem").join(name)).ok()
            })
            .map(|mem| brackets_from_dump(&mem))
            .find(|p| p.iter().all(|&(x, y, c)| px[(y * 320 + x) as usize] == c));
        let Some(found) = found else {
            panic!("C-0009 f{f}: no dump of the frames before puts the brackets where they are");
        };
        with_brackets += usize::from(!found.is_empty());
    }
    eprintln!("C-0009: {frames} frames consistent, {with_brackets} with bracket pixels");
    assert!(frames >= 30 && with_brackets >= 5);
}
