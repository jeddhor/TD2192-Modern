//! The real-time screens of the E1 mission events against the original (BUG-3): flight 17's
//! cut-scene (C-0067, tick 107, frames 877..1548) and flight 19's V.C.I. screen (C-0069, tick
//! 205, frames 1232..1510). Spec: `docs/spec/flow/flight-screens.md` rev 4 §4.10.
//! Data-gated (TD2192_DATA and TD2192_ORACLE). No captured pixels are stored.
//!
//! The captures dump a framebuffer every 70 frames, so each screen is seen in a handful of
//! frames. The pacing is T2 (`Crt.Delay` real time; the reference machine's `Crt.Delay(1000)`
//! lasts about 71.5 frames, the port's 70), so each dumped frame is matched against the port's
//! screen within a window of retraces around the same offset from the first fade level. In
//! the window the port must show the same pixels, every one, and the same DAC: exactly for a
//! frame taken between two DAC changes, or, for a frame taken while a fade level was being
//! written (the original spreads a level over two frames), each entry as one of two
//! consecutive port states.
//!
//! The flight's palette `g_FlightPalette` (with the HUD's lamp entries) is the capture's DGROUP
//! at the tick; the frame on screen when the screen opens is the first dumped frame of the
//! opening fade-out (the last flight frame, HUD band included).

use std::fs;
use std::path::Path;

use td_assets::{DataDir, ExeResources};
use td_flow::flight_screens::interlude::{
    CUT_SCENE_LINES, Interlude, PICTURE_ROWS, PICTURE_TOP, SCENE_CUT_SCENE, SCENE_VCI, VCI_LINE,
};
use td_flow::flight_screens::{FLIGHT_ROWS, FlightScreenAssets, ModalStep};
use td_formats::palette::Palette;
use td_render::Framebuffer;
use td_test_support::{require_data, require_oracle};

const W: usize = 320;
const DS_LEN: usize = 0x9DF0;
/// `g_FlightPalette`.
const FLIGHT_PALETTE: usize = 0x13E6;
/// Retraces of slack around the expected offset (T2 pacing, see the module docs).
const WINDOW: i64 = 24;

/// DAC and pixels of an oracle frame (320 × 192).
fn frame(oracle: &Path, capture: &str, f: u32) -> (Palette, Vec<u8>) {
    let name = format!("f{f:08}_320x192.vga");
    let raw = fs::read(oracle.join(capture).join("fb").join(name)).unwrap();
    (Palette::parse_prefix(&raw).unwrap(), raw[768..].to_vec())
}

/// `g_FlightPalette` in the per-tick DGROUP record of counter value `tick`.
fn flight_palette(oracle: &Path, capture: &str, tick: u32) -> Palette {
    let path = oracle
        .join(capture)
        .join("mem")
        .join("tick_4467-0000_9DF0_ds.bin");
    let b = fs::read(path).unwrap();
    let rec = b
        .chunks(16 + DS_LEN)
        .find(|c| u32::from_le_bytes(c[0..4].try_into().unwrap()) == tick)
        .expect("the tick's record");
    let ds = &rec[16..];
    Palette::parse(&ds[FLIGHT_PALETTE..FLIGHT_PALETTE + 768]).unwrap()
}

/// Every state the screen shows, one per retrace, until it closes.
fn run(mut s: Interlude, assets: &FlightScreenAssets) -> Vec<Framebuffer> {
    let mut out = Vec::new();
    loop {
        let step = s.tick(assets);
        out.push(s.canvas().clone());
        if step != ModalStep::Continue {
            return out;
        }
        assert!(out.len() < 2000, "the screen never closes");
    }
}

/// How the port state `k` (and `k + 1`) compares with a captured frame.
fn matches(states: &[Framebuffer], k: usize, dac: &Palette, px: &[u8]) -> Option<bool> {
    let a = states.get(k)?;
    let b = states.get(k + 1).unwrap_or(a);
    if a.pixels[..px.len()] != *px {
        return None;
    }
    if a.palette == *dac {
        return Some(true);
    }
    let torn = (0..256)
        .all(|e| dac.colors[e] == a.palette.colors[e] || dac.colors[e] == b.palette.colors[e]);
    torn.then_some(false)
}

/// Checks each captured frame `f` of `frames` against the port's states around offset
/// `f - first`; returns (frame, port offset − expected offset, exact DAC) per frame.
fn compare(
    states: &[Framebuffer],
    oracle: &Path,
    capture: &str,
    first: u32,
    frames: &[u32],
) -> Vec<(u32, i64, bool)> {
    let mut out = Vec::new();
    for &f in frames {
        let (dac, px) = frame(oracle, capture, f);
        let expected = i64::from(f - first);
        let mut best: Option<(i64, bool)> = None;
        for d in -WINDOW..=WINDOW {
            let Ok(k) = usize::try_from(expected + d) else {
                continue;
            };
            if let Some(exact) = matches(states, k, &dac, &px) {
                let better = match best {
                    None => true,
                    Some((bd, be)) => (exact && !be) || (exact == be && d.abs() < bd.abs()),
                };
                if better {
                    best = Some((d, exact));
                }
            }
        }
        let Some((d, exact)) = best else {
            // Report the closest pixel picture for the failure message.
            let k = usize::try_from(expected).unwrap_or(0).min(states.len() - 1);
            let diff = (0..px.len())
                .filter(|&i| states[k].pixels[i] != px[i])
                .count();
            let dac_diff: Vec<usize> = (0..256)
                .filter(|&e| states[k].palette.colors[e] != dac.colors[e])
                .take(8)
                .collect();
            panic!(
                "{capture} f{f}: no port state within {WINDOW} retraces of offset {expected} \
                 matches (state {k}: {diff} pixels differ, DAC entries {dac_diff:?}..)"
            );
        };
        out.push((f, d, exact));
    }
    out
}

fn shown(px: &[u8]) -> Framebuffer {
    let mut fb = Framebuffer::new();
    fb.pixels[..px.len()].copy_from_slice(px);
    fb
}

#[test]
fn flight_17_cut_scene_matches_c0067() {
    let data = require_data!();
    let oracle = require_oracle!();
    let dir = DataDir::open(&data).unwrap();
    let res = ExeResources::load(&dir).unwrap();
    let assets = FlightScreenAssets::load(&dir, &res).unwrap();
    let p = flight_palette(&oracle, "C-0067", 107);
    // f890: the opening fade-out over the last flight frame (frames 877..910).
    let (_, open) = frame(&oracle, "C-0067", 890);
    let s = Interlude::new(SCENE_CUT_SCENE, shown(&open), &p, 0, &assets).unwrap();
    let states = run(s, &assets);
    // 17-level fades at two retraces each, the picture read, 7,300 ms of delays: C-0067 tick
    // 107 lasts 673 frames (875..1548).
    assert!(
        (640..700).contains(&states.len()),
        "{} retraces",
        states.len()
    );
    let frames = [890, 960, 1030, 1100, 1170, 1240, 1310, 1380, 1450, 1520];
    let got = compare(&states, &oracle, "C-0067", 877, &frames);
    eprintln!("C-0067 (frame, offset, exact DAC): {got:?}");
    // The steady frames (picture and text on screen, DAC between changes) match exactly.
    for &(f, _, exact) in &got {
        if (960..=1450).contains(&f) {
            assert!(exact, "C-0067 f{f}: DAC not exact");
        }
    }
    // Spot checks of the layout (§4.10.1): picture rows 35..124, line pens and inks.
    let last_text = &states[states.len() - 120];
    let pic = &assets.scene_picture;
    for r in 0..PICTURE_ROWS {
        let row = &last_text.pixels[(PICTURE_TOP + r) * W..(PICTURE_TOP + r + 1) * W];
        assert_eq!(row, &pic[r * W..(r + 1) * W], "picture row {r}");
    }
    for &((x, y), ink) in &CUT_SCENE_LINES {
        let rows = (y + 1) as usize..=(y + 5) as usize;
        let inked = rows
            .flat_map(|r| (x as usize + 2..W).map(move |c| (c, r)))
            .filter(|&(c, r)| last_text.pixels[r * W + c] == ink)
            .count();
        assert!(
            inked > 20,
            "line at ({x}, {y}) in ink {ink}: {inked} pixels"
        );
    }
    // The end: the playfield black, the HUD band as it was, the flight palette.
    let end = states.last().unwrap();
    assert!(end.pixels[..160 * W].iter().all(|&c| c == 0));
    assert_eq!(end.pixels[160 * W..FLIGHT_ROWS * W], open[160 * W..]);
    assert_eq!(end.palette.colors[1..], p.colors[1..]);
}

#[test]
fn flight_19_vci_screen_matches_c0069() {
    let data = require_data!();
    let oracle = require_oracle!();
    let dir = DataDir::open(&data).unwrap();
    let res = ExeResources::load(&dir).unwrap();
    let assets = FlightScreenAssets::load(&dir, &res).unwrap();
    let p = flight_palette(&oracle, "C-0069", 205);
    // f1240: the opening fade-out (frames 1232..1265).
    let (_, open) = frame(&oracle, "C-0069", 1240);
    let s = Interlude::new(SCENE_VCI, shown(&open), &p, 0, &assets).unwrap();
    let states = run(s, &assets);
    // Four fades of 34 retraces and 2,000 ms: C-0069 tick 205 lasts 279 frames (1231..1510).
    assert!(
        (260..300).contains(&states.len()),
        "{} retraces",
        states.len()
    );
    let got = compare(&states, &oracle, "C-0069", 1232, &[1240, 1310, 1380, 1450]);
    eprintln!("C-0069 (frame, offset, exact DAC): {got:?}");
    for &(f, _, exact) in &got {
        if (1310..=1380).contains(&f) {
            assert!(exact, "C-0069 f{f}: DAC not exact");
        }
    }
    // Only the line, in ink 131, on rows 49..53 (C-0069 f1310: columns 42..262).
    let (_, f1310) = frame(&oracle, "C-0069", 1310);
    let ((x, y), ink) = VCI_LINE;
    for (i, &c) in f1310.iter().enumerate() {
        let (col, row) = ((i % W) as i32, (i / W) as i32);
        if c != 0 {
            assert_eq!(c, ink);
            assert!(
                (y + 1..=y + 5).contains(&row) && col >= x + 2,
                "({col}, {row})"
            );
        }
    }
    let end = states.last().unwrap();
    assert!(end.pixels[..160 * W].iter().all(|&c| c == 0));
    assert_eq!(end.palette.colors[1..], p.colors[1..]);
}

/// Relative luminance (Rec. 709, linear sRGB) of a 6-bit colour.
fn luminance(c: td_formats::palette::Rgb6) -> f64 {
    let lin = |v: u8| {
        let s = f64::from(td_formats::palette::expand6(v)) / 255.0;
        if s <= 0.04045 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b)
}

/// M12-18, reduce flashing (port-front-end rev 5 §3.5): the cut-scene runs the same steps in
/// the same retraces and draws the same pixels; only its flash steps differ, and each entry
/// then stays within a small distance of its steady colour (no WCAG flash: less than 0.1 in
/// luminance). Needs the game data only (the flight palette is `td.pal`).
#[test]
fn reduce_flashing_caps_the_cut_scene_flash() {
    let data = require_data!();
    let dir = DataDir::open(&data).unwrap();
    let res = ExeResources::load(&dir).unwrap();
    let assets = FlightScreenAssets::load(&dir, &res).unwrap();
    let p = Palette::parse(&dir.read("td.pal").unwrap()).unwrap();
    let open = vec![7u8; 192 * W];
    let plain = run(
        Interlude::new(SCENE_CUT_SCENE, shown(&open), &p, 0, &assets).unwrap(),
        &assets,
    );
    let reduced = run(
        Interlude::new(SCENE_CUT_SCENE, shown(&open), &p, 0, &assets)
            .unwrap()
            .reducing_flashes(true),
        &assets,
    );
    assert_eq!(plain.len(), reduced.len(), "the same retraces");
    let mut flash_states = 0;
    for (a, b) in plain.iter().zip(&reduced) {
        assert_eq!(a.pixels, b.pixels);
        if a.palette == b.palette {
            continue;
        }
        flash_states += 1;
        let mut strongest = 0.0f64;
        for e in 0..256 {
            let (steady, shown) = (luminance(p.colors[e]), luminance(b.palette.colors[e]));
            assert!((shown - steady).abs() < 0.1, "entry {e}");
            strongest = strongest.max((luminance(a.palette.colors[e]) - steady).abs());
        }
        assert!(strongest >= 0.1, "the original's step is a flash");
    }
    // Black, yellow-white, black: 100 ms each.
    assert!(
        (15..30).contains(&flash_states),
        "{flash_states} retraces of flash"
    );
}
