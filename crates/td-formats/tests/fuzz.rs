//! M12-06: "never panics" fuzz-style tests for every `td-formats` parser (MDG §11, ADG §10.6).
//!
//! Every parser entry point gets three kinds of input; after a successful parse the accessors
//! that index into the result are walked too, since that is where out-of-bounds panics hide:
//!
//! 1. random bytes of random length (0..64 KiB), including lengths around each format's exact
//!    size (768 palette, 64,768 screen, 20,080 mission, 201x201 map, ...);
//! 2. synthetic inputs of the right shape (size, magic, structure) with random content, with and
//!    without a few random mutations;
//! 3. with `TD2192_DATA` set, every original file of each format, unmodified, truncated at many
//!    lengths (every length for files up to 4 KiB) and with seeded random mutations (bit flips,
//!    byte sets, inserts, deletes, length and count fields overwritten with 0, 0xFF, 0xFFFF,
//!    the input length, ...). Original bytes are only read at run time; nothing derived from
//!    them is stored.
//!
//! A panic is caught and reported with a reproducer (target, input origin, mutation list and
//! the accessor seed); an input that takes longer than [`SLOW_INPUT`] fails as a suspected hang.
//! No parser loops on a count without consuming input: `Cmf` (instrument count, event track),
//! `Voc` (blocks), `pstring_seq` (caller's count) and `DialogueScript` (lines) all read at
//! least one byte per iteration or stop with an error.
//!
//! Two tiers: the default tests are bounded (proptest with fixed case counts, bounded mutation
//! counts). `long_fuzz` is ignored by default; it loops with fresh seeds for `TD2192_FUZZ_SECS`
//! seconds (default 60) from the base seed `TD2192_FUZZ_SEED` (default: from the clock,
//! printed), under a watchdog that fails the test if one input runs longer than [`HANG_LIMIT`],
//! and prints the number of inputs tried per parser:
//!
//! ```text
//! TD2192_FUZZ_SECS=600 cargo test -p td-formats --release --test fuzz -- --ignored --nocapture
//! ```

use std::fmt::Write as _;
use std::fs;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use proptest::collection::vec;
use proptest::prelude::*;
use proptest::sample::Index;
use td_formats::ByteReader;
use td_formats::cmf::{Cmf, EventKind};
use td_formats::dialogue::{Command, DialogueScript, Item, read_scene};
use td_formats::font::{Font, FontKind};
use td_formats::map::TileMap;
use td_formats::mission::{self, Mission};
use td_formats::mz::{self, MzHeader};
use td_formats::palette::{self, PALETTE_LEN, Palette};
use td_formats::picture::{
    CELL_SHEETS, CellLayout, Picture, STILLS, StillLayout, cell_layout, parse_cells,
};
use td_formats::portraits::{self, Portraits};
use td_formats::saves::{self, SaveNames, SaveSlots};
use td_formats::screen::{self, PagedScreens, Screen};
use td_formats::sprites::{self, SpriteSheet};
use td_formats::tileset::{self, Tileset};
use td_formats::voc::{self, Block, Voc};
use td_test_support::{data_dir, require_data};

/// An input slower than this (in any build) fails the default tests as a suspected hang.
const SLOW_INPUT: Duration = Duration::from_secs(5);
/// The long test's watchdog limit for one input.
const HANG_LIMIT: Duration = Duration::from_secs(10);

// ---------------------------------------------------------------------------------------------
// Deterministic randomness and mutations
// ---------------------------------------------------------------------------------------------

/// splitmix64: small, fast and good enough to pick mutations; seeded so every run reproduces.
#[derive(Debug, Clone)]
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `0..n` (0 when `n` is 0).
    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }

    /// Uniform in `lo..=hi`.
    fn range(&mut self, lo: usize, hi: usize) -> usize {
        lo + self.below(hi - lo + 1)
    }

    /// True with probability `1 / n`.
    fn one_in(&mut self, n: usize) -> bool {
        self.below(n) == 0
    }

    fn byte(&mut self) -> u8 {
        self.next_u64() as u8
    }

    fn bytes(&mut self, n: usize) -> Vec<u8> {
        let mut v = Vec::with_capacity(n);
        while v.len() < n {
            let w = self.next_u64().to_le_bytes();
            let k = (n - v.len()).min(8);
            v.extend_from_slice(&w[..k]);
        }
        v
    }

    fn pick<T: Copy>(&mut self, xs: &[T]) -> T {
        xs[self.below(xs.len())]
    }
}

/// A seed derived from two values (rounds, files, steps).
fn mix(a: u64, b: u64) -> u64 {
    Rng::new(a ^ b.wrapping_mul(0xD1B5_4A32_D192_ED03)).next_u64()
}

/// FNV-1a of a name, to give every original file its own mutation stream.
fn name_seed(name: &str) -> u64 {
    name.bytes().fold(0xCBF2_9CE4_8422_2325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01B3)
    })
}

/// Byte values that tend to matter: zero, the control codes of the dialogue and text formats,
/// the 6-bit palette limit, sign bits and all-ones.
const INTERESTING_U8: &[u8] = &[
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x0A, 0x0D, 0x1A, 0x20, 0x2F, 0x3F, 0x40, 0x64, 0x7F,
    0x80, 0xC8, 0xFE, 0xFF,
];

/// An offset in `0..len`, biased towards the first 64 bytes where headers keep their sizes,
/// offsets and counts.
fn offset(rng: &mut Rng, len: usize) -> usize {
    if rng.one_in(2) {
        rng.below(len.min(64))
    } else {
        rng.below(len)
    }
}

/// Applies one random mutation and describes it (for the reproducer).
fn mutate_once(data: &mut Vec<u8>, rng: &mut Rng) -> String {
    let len = data.len();
    match rng.below(10) {
        0 if len > 0 => {
            let o = offset(rng, len);
            let bit = 1u8 << rng.below(8);
            data[o] ^= bit;
            format!("flip[{o}]^={bit:#04x}")
        }
        1 if len > 0 => {
            let o = offset(rng, len);
            let v = if rng.one_in(3) {
                rng.byte()
            } else {
                rng.pick(INTERESTING_U8)
            };
            data[o] = v;
            format!("set[{o}]={v:#04x}")
        }
        2 => {
            let o = rng.below(len + 1);
            let n = rng.range(1, 16);
            let ins = rng.bytes(n);
            let what = format!("insert[{o}]={ins:02x?}");
            data.splice(o..o, ins);
            what
        }
        3 if len > 0 => {
            let o = rng.below(len);
            let n = rng.range(1, 16).min(len - o);
            data.drain(o..o + n);
            format!("delete[{o}..{}]", o + n)
        }
        4 if len >= 2 => {
            let o = offset(rng, len - 1);
            let random = rng.next_u64() as u16;
            let v = rng.pick(&[
                0,
                1,
                0xFF,
                0x7FFF,
                0x8000,
                0xFFFF,
                len as u16,
                (len as u16).wrapping_sub(1),
                (len as u16).wrapping_add(1),
                random,
            ]);
            data[o..o + 2].copy_from_slice(&v.to_le_bytes());
            format!("u16[{o}]={v:#x}")
        }
        5 if len >= 3 => {
            let o = offset(rng, len - 2);
            let random = rng.next_u64() as u32 & 0xFF_FFFF;
            let v = rng.pick(&[
                0,
                0xFF_FFFF,
                0x80_0000,
                len as u32 & 0xFF_FFFF,
                (len as u32 + 1) & 0xFF_FFFF,
                random,
            ]);
            data[o..o + 3].copy_from_slice(&v.to_le_bytes()[..3]);
            format!("u24[{o}]={v:#x}")
        }
        6 if len >= 4 => {
            let o = offset(rng, len - 3);
            let random = rng.next_u64() as u32;
            let v = rng.pick(&[
                0,
                0xFFFF_FFFF,
                0x7FFF_FFFF,
                0x8000_0000,
                0x00FF_FFFF,
                len as u32,
                len as u32 + 1,
                random,
            ]);
            data[o..o + 4].copy_from_slice(&v.to_le_bytes());
            format!("u32[{o}]={v:#x}")
        }
        7 if len > 0 => {
            let n = rng.below(len);
            data.truncate(n);
            format!("truncate({n})")
        }
        8 if len > 1 => {
            let src = rng.below(len);
            let dst = rng.below(len);
            let n = rng.range(1, 64).min(len - src).min(len - dst);
            data.copy_within(src..src + n, dst);
            format!("copy[{src}..{}]->{dst}", src + n)
        }
        _ => {
            let n = rng.range(1, 16);
            let extra = rng.bytes(n);
            let what = format!("append={extra:02x?}");
            data.extend_from_slice(&extra);
            what
        }
    }
}

/// Applies `n` mutations and returns them as one description.
fn mutate(data: &mut Vec<u8>, rng: &mut Rng, n: usize) -> String {
    let ops: Vec<String> = (0..n).map(|_| mutate_once(data, rng)).collect();
    ops.join(", ")
}

/// Lengths at which to truncate a file of `len` bytes: every length up to 4 KiB; otherwise the
/// first and last 64, a stride of about `points` lengths and the format's exact sizes.
fn truncations(len: usize, sizes: &[usize], points: usize) -> Vec<usize> {
    if len <= 4096 {
        return (0..len).collect();
    }
    let stride = (len / points.max(1)).max(1);
    let mut v: Vec<usize> = (0..64)
        .chain(len - 64..len)
        .chain((0..len).step_by(stride))
        .chain(sizes.iter().flat_map(|&s| [s.saturating_sub(1), s, s + 1]))
        .filter(|&n| n < len)
        .collect();
    v.sort_unstable();
    v.dedup();
    v
}

// ---------------------------------------------------------------------------------------------
// Targets: one per parser module, each parsing and then walking the result
// ---------------------------------------------------------------------------------------------

/// One parser module under test.
#[derive(Debug, Clone, Copy)]
struct Target {
    name: &'static str,
    /// Parses `data` with every entry point of the module and walks the accessors of each
    /// result; `rng` picks accessor probes.
    run: fn(&[u8], &mut Rng),
    /// Exact sizes of the format (random bytes of these lengths, and a few bytes either side).
    sizes: fn() -> Vec<usize>,
    /// A valid-shaped input with random content.
    synth: fn(&mut Rng) -> Vec<u8>,
    /// The original files of this format.
    files: fn() -> Vec<String>,
}

const TARGETS: &[Target] = &[
    Target {
        name: "palette",
        run: run_palette,
        sizes: || vec![PALETTE_LEN],
        synth: synth_palette,
        files: || names(&["td.pal", "tdmen.pal"]),
    },
    Target {
        name: "screen",
        run: run_screen,
        sizes: || {
            let s = screen::SCREEN_FILE_LEN;
            vec![PALETTE_LEN, s, s + screen::PIXELS, s + 2 * screen::PIXELS]
        },
        synth: synth_screen,
        files: screen_files,
    },
    Target {
        name: "tileset",
        run: run_tileset,
        sizes: || {
            let n = tileset::TILE_COUNT * tileset::TILE_BYTES;
            vec![n, n + 864]
        },
        synth: synth_tileset,
        files: || names(&["td.blk", "td2.blk", "td3.blk"]),
    },
    Target {
        name: "map",
        run: run_map,
        sizes: || vec![0, 1, 4, 20 * 2000, 200 * 200, 201 * 201, 256 * 256],
        synth: synth_map,
        files: || names(&["td.map", "td2.map", "td3.map", "td4.map", "intro.map"]),
    },
    Target {
        name: "mission",
        run: run_mission,
        sizes: || vec![mission::MISSION_FILE_LEN],
        synth: synth_mission,
        files: mission_files,
    },
    Target {
        name: "mz",
        run: run_mz,
        sizes: || vec![0, 2, 27, 28, 64, 512],
        synth: synth_mz,
        files: exe_files,
    },
    Target {
        name: "pstring",
        run: run_pstring,
        sizes: || vec![0, 1, 2, 255, 256, 257],
        synth: synth_pstrings,
        files: exe_files,
    },
    Target {
        name: "byte_reader",
        run: run_reader,
        sizes: || vec![0, 1, 2, 3, 4, 5, 8],
        synth: |rng| {
            let n = rng.below(64);
            rng.bytes(n)
        },
        files: || names(&["td.cfg", "td.sav", "td.pal"]),
    },
    Target {
        name: "cmf",
        run: run_cmf,
        sizes: || vec![0x14, 0x24, 0x28, 0x38],
        synth: synth_cmf,
        files: cmf_files,
    },
    Target {
        name: "voc",
        run: run_voc,
        sizes: || vec![20, 24, 26, 27, 30],
        synth: synth_voc,
        files: voc_files,
    },
    Target {
        name: "dialogue",
        run: run_dialogue,
        sizes: || vec![0, 1, 2, 3, 4],
        synth: synth_dialogue,
        files: || {
            names(&[
                "dialogue.dat",
                "safedial.dat",
                "dialogu2.dat",
                "safedia2.dat",
                "dialogu3.dat",
                "safedia3.dat",
            ])
        },
    },
    Target {
        name: "font",
        run: run_font,
        sizes: || FONT_KINDS.iter().map(|k| k.file_len()).collect(),
        synth: synth_font,
        files: || names(&["td.lt2", "td-let.cin", "td.ltr"]),
    },
    Target {
        name: "picture",
        run: run_picture,
        sizes: picture_sizes,
        synth: synth_picture,
        files: || {
            STILLS
                .iter()
                .map(|s| s.name)
                .chain(CELL_SHEETS.iter().map(|c| c.name))
                .map(str::to_owned)
                .collect()
        },
    },
    Target {
        name: "portraits",
        run: run_portraits,
        sizes: || vec![portraits::RECORD_LEN, 2 * portraits::RECORD_LEN],
        synth: synth_portraits,
        files: || names(&["newfaces.td", "newfaces.td2", "newfaces.td3"]),
    },
    Target {
        name: "saves",
        run: run_saves,
        sizes: || vec![0, 29, saves::SLOT_BYTES, 31, saves::SAV_LEN, 260],
        synth: synth_saves,
        files: || {
            names(&[
                "td.sav", "td2.sav", "td3.sav", "td.stt", "td2.stt", "td3.stt", "td.cfg",
            ])
        },
    },
    Target {
        name: "sprites",
        run: run_sprites,
        sizes: || {
            vec![
                sprites::BASE_FRAMES * sprites::BASE_SIZE * sprites::BASE_SIZE,
                sprites::ROTATED_CELLS * sprites::ROTATED_SIZE * sprites::ROTATED_SIZE,
                1024,
                1600,
            ]
        },
        synth: synth_sprites,
        files: || {
            names(&[
                "td.shp",
                "td.sc3",
                "td.shr",
                "td.sc2",
                "mine.td",
                "lander.td",
                "td.exp",
            ])
        },
    },
];

const FONT_KINDS: [FontKind; 3] = [FontKind::Small, FontKind::Title, FontKind::Large];

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

fn screen_files() -> Vec<String> {
    let mut v = names(&[
        "aboutus.scr",
        "bb.scr",
        "briefing.scr",
        "bunker.scr",
        "command.scr",
        "convers.scr",
        "dieseche.scr",
        "eggbase.scr",
        "elevator.scr",
        "greenblo.scr",
        "hangar.scr",
        "hangar2.scr",
        "helihang.scr",
        "hyde.scr",
        "hydedead.scr",
        "i-port.scr",
        "intro0.scr",
        "intro1.scr",
        "intro2.scr",
        "ionstat.scr",
        "kolorvci.scr",
        "maraud2.scr",
        "maraud3.scr",
        "marauder.scr",
        "melting.scr",
        "mining1.scr",
        "mining2.scr",
        "order.scr",
        "sbar.scr",
        "sel-pain.scr",
        "sewer.scr",
        "shooting.scr",
        "shuttle.scr",
        "shuttle0.scr",
        "soffice.scr",
        "tdlab.scr",
        "tdoffic2.scr",
        "velsroom.scr",
        "velvswlf.scr",
        "vhang.scr",
        "vidcom.scr",
        "vlab.scr",
        "voffice.scr",
        "v-port.scr",
        "city.td",
        "javelin.td",
    ]);
    v.push("td.hlp".to_owned());
    v
}

fn mission_files() -> Vec<String> {
    let mut v = Vec::new();
    for (ext, last) in [("td", 20), ("td2", 20), ("td3", 21)] {
        for n in 1..=last {
            if !(ext == "td3" && n == 20) {
                v.push(format!("m{n}.{ext}"));
            }
        }
    }
    v
}

fn exe_files() -> Vec<String> {
    names(&[
        "td1.exe",
        "file0000.exe",
        "file0001.exe",
        "td2.exe",
        "td3.exe",
        "aboutus.exe",
        "catalog.exe",
        "helpme.exe",
        "nuffmem.exe",
        "order.exe",
        "order2.com",
    ])
}

fn cmf_files() -> Vec<String> {
    names(&[
        "climax.cmf",
        "death.cmf",
        "hangar.cmf",
        "intro.cmf",
        "intro1.cmf",
        "intro2.cmf",
        "low.cmf",
        "menu.cmf",
        "mood.cmf",
        "mood2.cmf",
        "mood3.cmf",
        "moodmaj.cmf",
        "satair.cmf",
        "td01.cmf",
        "td02.cmf",
        "td03.cmf",
        "vulture.cmf",
    ])
}

fn voc_files() -> Vec<String> {
    names(&[
        "afterb.voc",
        "boomboom.voc",
        "home.voc",
        "hornet.voc",
        "income.voc",
        "missile.voc",
        "poink.voc",
        "pulse.voc",
        "punyexp.voc",
        "ric2.voc",
        "ric3.voc",
        "scratch.voc",
        "stiletto.voc",
        "vii.voc",
        "vulturei.voc",
        "whoosh.voc",
        "zeus.voc",
    ])
}

fn picture_sizes() -> Vec<usize> {
    STILLS
        .iter()
        .map(|s| s.width * s.height + if s.has_palette { PALETTE_LEN } else { 0 })
        .chain(
            CELL_SHEETS
                .iter()
                .map(|c| c.cell_width * c.cell_height * c.count),
        )
        .collect()
}

/// Indices to probe a dimension of `n` with: both edges, just outside, far outside, random.
fn probes(rng: &mut Rng, n: usize) -> [usize; 8] {
    [
        0,
        1,
        n.saturating_sub(1),
        n,
        n + 1,
        usize::MAX,
        rng.below(n.max(1)),
        rng.below(2 * n + 2),
    ]
}

fn run_palette(d: &[u8], _: &mut Rng) {
    if let Ok(p) = Palette::parse(d) {
        assert_eq!(p.to_bytes(), d);
        let _ = p.to_rgb8();
    }
    if let Ok(p) = Palette::parse_prefix(d) {
        assert_eq!(p.to_bytes(), d[..PALETTE_LEN]);
        for (c, rgb) in p.colors.iter().zip(p.to_rgb8()) {
            assert_eq!(rgb, c.to_rgb8());
        }
    }
}

fn run_screen(d: &[u8], rng: &mut Rng) {
    if let Ok(s) = Screen::parse(d) {
        assert_eq!(s.pixels.len(), screen::PIXELS);
        for x in probes(rng, screen::WIDTH) {
            for y in probes(rng, screen::HEIGHT) {
                let inside = x < screen::WIDTH && y < screen::HEIGHT;
                assert_eq!(s.pixel(x, y).is_some(), inside);
            }
        }
    }
    if let Ok(p) = PagedScreens::parse(d) {
        assert_eq!(p.pages.len(), (d.len() - PALETTE_LEN) / screen::PIXELS);
        assert!(p.pages.iter().all(|page| page.len() == screen::PIXELS));
    }
}

fn run_tileset(d: &[u8], rng: &mut Rng) {
    if let Ok(t) = Tileset::parse(d) {
        assert_eq!(t.tiles.len(), tileset::TILE_COUNT);
        assert_eq!(
            t.trailing.len(),
            d.len() - tileset::TILE_COUNT * tileset::TILE_BYTES
        );
        for i in probes(rng, tileset::TILE_COUNT) {
            for x in probes(rng, tileset::TILE_SIZE) {
                for y in probes(rng, tileset::TILE_SIZE) {
                    let inside =
                        i < tileset::TILE_COUNT && x < tileset::TILE_SIZE && y < tileset::TILE_SIZE;
                    assert_eq!(t.pixel(i, x, y).is_some(), inside);
                }
            }
        }
    }
}

fn run_map(d: &[u8], rng: &mut Rng) {
    for m in [TileMap::parse_episode(d), TileMap::parse_any(d)]
        .into_iter()
        .flatten()
    {
        let n = m.size();
        assert_eq!(m.cells().len(), n * n);
        for x in probes(rng, n) {
            for y in probes(rng, n) {
                assert_eq!(m.get(x, y).is_some(), x < n && y < n);
            }
        }
    }
}

fn run_mission(d: &[u8], rng: &mut Rng) {
    let Ok(m) = Mission::parse(d) else { return };
    assert_eq!(m.to_bytes(), d);
    assert!(m.used_slots().all(|s| s < mission::SLOTS));
    let slots = (0..=mission::SLOTS + 1).chain([usize::MAX, rng.next_u64() as usize]);
    for slot in slots {
        match m.path(slot) {
            Some(p) => assert_eq!(p.count(), mission::PATH_LEN),
            None => assert!(slot >= mission::SLOTS),
        }
        for arr in [&m.path_x, &m.path_y] {
            assert_eq!(
                arr.steps(slot).map(<[u8]>::len),
                (slot < mission::SLOTS).then_some(mission::PATH_LEN)
            );
            for k in [0, 1, 481, 482, 500, 501, usize::MAX, rng.below(600)] {
                let inside = slot < mission::SLOTS && k < mission::PATH_STRIDE;
                assert_eq!(arr.get(slot, k).is_some(), inside);
            }
        }
    }
}

fn run_mz(d: &[u8], rng: &mut Rng) {
    let Ok(h) = MzHeader::parse(d) else { return };
    let addresses = [
        (0, 0),
        (h.initial_cs, h.initial_ip),
        (h.initial_ss, h.initial_sp),
        (0xFFFF, 0xFFFF),
        (rng.next_u64() as u16, rng.next_u64() as u16),
    ];
    for (seg, off) in addresses {
        let at = h.file_offset(seg, off);
        if let Ok(s) = mz::pstring(d, at) {
            assert_eq!(s.len(), usize::from(d[at]));
        }
        let count = rng.below(16);
        if let Ok(seq) = mz::pstring_seq(d, at, count) {
            assert_eq!(seq.len(), count);
        }
    }
}

fn run_pstring(d: &[u8], rng: &mut Rng) {
    let len = d.len();
    let offsets = [
        0,
        1,
        len.saturating_sub(1),
        len,
        len + 1,
        usize::MAX,
        rng.below(len + 2),
        rng.below(len + 2),
    ];
    for at in offsets {
        if let Ok(s) = mz::pstring(d, at) {
            assert_eq!(s.len(), usize::from(d[at]));
        }
        // BUG-16: `pstring_seq` reserves `count` slots before reading anything (mz.rs:77), so
        // a count above isize::MAX / 16 panics ("capacity overflow"; minimal case:
        // `pstring_seq(&[], 0, usize::MAX)`) and one in the terabytes aborts the process on
        // allocation failure. Counts are bounded here and in `pstrings_never_panic`.
        let random = rng.below(1024);
        let count = rng.pick(&[0, 1, 2, 255, 256, random]);
        if let Ok(seq) = mz::pstring_seq(d, at, count) {
            assert_eq!(seq.len(), count);
        }
    }
}

/// Drives a [`ByteReader`] over `d` with `(op, arg)` pairs and checks its invariants: a read
/// that succeeds advances by its size, one that fails leaves the position alone, and the
/// position never passes the end.
fn reader_ops(d: &[u8], ops: &[(u8, usize)]) {
    let mut r = ByteReader::new("fuzz", d);
    for &(op, arg) in ops {
        let before = r.pos();
        let advanced = |r: &ByteReader<'_>, ok: bool, n: usize| {
            assert_eq!(r.pos(), if ok { before + n } else { before });
        };
        match op % 9 {
            0 => {
                let got = r.bytes(arg);
                if let Ok(b) = got {
                    assert_eq!(b.len(), arg);
                }
                advanced(&r, got.is_ok(), arg);
            }
            1 => {
                let n = arg % (d.len() + 3);
                let ok = r.bytes(n).is_ok();
                advanced(&r, ok, n);
            }
            2 => {
                let ok = r.u8().is_ok();
                advanced(&r, ok, 1);
            }
            3 => {
                let ok = r.u16_le().is_ok();
                advanced(&r, ok, 2);
            }
            4 => {
                let ok = r.u24_le().is_ok();
                advanced(&r, ok, 3);
            }
            5 => {
                let ok = r.u32_le().is_ok();
                advanced(&r, ok, 4);
            }
            6 => {
                let to = arg % (d.len() + 3);
                assert_eq!(r.seek(to).is_ok(), to <= d.len());
                assert_eq!(r.pos(), if to <= d.len() { to } else { before });
            }
            7 => {
                assert_eq!(r.seek(arg).is_ok(), arg <= d.len());
            }
            _ => {
                let _ = r.invalid(format!("probe {arg}")).to_string();
                let copy = r.clone();
                assert_eq!(copy.pos(), r.pos());
            }
        }
        assert!(r.pos() <= d.len());
        assert_eq!(r.remaining(), d.len() - r.pos());
    }
}

fn run_reader(d: &[u8], rng: &mut Rng) {
    let ops: Vec<(u8, usize)> = (0..rng.below(64))
        .map(|_| {
            let (wide, near) = (rng.next_u64() as usize, rng.below(d.len() + 4));
            let arg = rng.pick(&[
                0,
                1,
                2,
                d.len(),
                d.len() + 1,
                usize::MAX,
                usize::MAX - 1,
                wide,
                near,
            ]);
            (rng.byte(), arg)
        })
        .collect();
    reader_ops(d, &ops);
}

fn run_cmf(d: &[u8], _: &mut Rng) {
    let Ok(c) = Cmf::parse(d) else { return };
    assert!(c.ticks_per_second > 0);
    let mut last = 0;
    for e in &c.events {
        assert!(e.tick >= last, "events are in time order");
        last = e.tick;
        let channel = match e.kind {
            EventKind::NoteOff { channel, .. }
            | EventKind::NoteOn { channel, .. }
            | EventKind::Controller { channel, .. }
            | EventKind::Program { channel, .. }
            | EventKind::PitchBend { channel, .. }
            | EventKind::Pressure { channel } => channel,
        };
        assert!(channel < 16);
        if let EventKind::NoteOn { velocity, .. } = e.kind {
            assert_ne!(velocity, 0);
        }
    }
    assert!(c.end_tick >= last);
    let regs: u32 = c
        .instruments
        .iter()
        .map(|i| u32::from(i.modulator.char_20) + u32::from(i.carrier.wave_e0))
        .sum();
    let _ = (regs, c.channel_in_use.iter().filter(|&&b| b).count());
}

fn run_voc(d: &[u8], _: &mut Rng) {
    let Ok(v) = Voc::parse(d) else { return };
    let mut samples = 0u64;
    for b in &v.blocks {
        match *b {
            Block::Sound {
                time_constant,
                data,
                ..
            } => {
                assert!(voc::sample_rate(time_constant) > 0);
                samples += data.iter().map(|&x| u64::from(x)).sum::<u64>();
            }
            Block::Continue { data } => samples += data.len() as u64,
            Block::Silence {
                samples: n,
                time_constant,
            } => {
                assert!((1..=0x1_0000).contains(&n));
                assert!(voc::sample_rate(time_constant) > 0);
            }
            Block::Ignored { kind } => assert!(kind == 4 || kind == 5),
        }
    }
    let _ = samples;
}

/// Scene IDs to read: all 256 for inputs up to 8 KiB; otherwise the IDs that some line starts
/// with (up to 8 of them, at random) plus 13 and two random ones, which keeps large files fast
/// while still covering both the found and the not-found search.
fn scene_ids(d: &[u8], rng: &mut Rng) -> Vec<u8> {
    if d.len() <= 8192 {
        return (0..=255).collect();
    }
    let mut present = Vec::new();
    let mut at_line_start = true;
    for (i, &b) in d.iter().enumerate() {
        if at_line_start && b == 0x06 {
            present.push(d.get(i + 1).copied().unwrap_or(0x0D));
        }
        at_line_start = b == b'\n';
    }
    let mut ids: Vec<u8> = (0..present.len().min(8))
        .map(|_| rng.pick(&present))
        .collect();
    ids.extend([0x0D, rng.byte(), rng.byte()]);
    ids
}

fn run_dialogue(d: &[u8], rng: &mut Rng) {
    if let Ok(s) = DialogueScript::parse(d) {
        for sc in &s.scenes {
            for sp in &sc.speakers {
                assert!(sc.speaker_for_color(sp.color).is_some());
            }
            let _ = sc.speaker_for_color(rng.byte());
            for item in sc.segments.iter().flatten() {
                match item {
                    Item::Line { text, .. } => assert!(text.chars().all(|c| (c as u32) < 256)),
                    Item::Speaker(sp) => assert!(sp.name.chars().all(|c| (c as u32) < 256)),
                    Item::PageBreak | Item::Face { .. } => {}
                }
            }
        }
        for n in 0..=255 {
            if let Some(sc) = s.scene(n) {
                assert_eq!(sc.number, n);
            }
        }
    }
    for id in scene_ids(d, rng) {
        let Some(cmds) = read_scene(d, id) else {
            continue;
        };
        for c in &cmds {
            match c {
                Command::Speaker { name, .. } | Command::Line { text: name, .. } => {
                    assert!(name.chars().all(|c| (c as u32) < 256));
                }
                Command::SegmentStart | Command::SegmentEnd | Command::PageBreak => {}
            }
        }
    }
}

fn run_font(d: &[u8], _: &mut Rng) {
    for kind in FONT_KINDS {
        let Ok(f) = Font::parse(d, kind) else {
            continue;
        };
        let (w, h) = kind.size();
        assert_eq!(f.glyphs.len(), kind.file_len() / (w * h));
        let chars = (0u8..=255).map(char::from).chain([
            '\u{100}',
            '\u{130}',
            '\u{212A}',
            '\u{FFFF}',
            char::MAX,
        ]);
        for c in chars {
            if let Some(g) = f.glyph(c) {
                assert_eq!(g.len(), w * h);
            }
            if let Some(i) = kind.glyph_index(c) {
                assert!(i < f.glyphs.len(), "{kind:?} {c:?} -> {i}");
            }
        }
    }
}

fn run_picture(d: &[u8], _: &mut Rng) {
    for layout in STILLS {
        if let Ok(p) = Picture::parse(d, layout) {
            assert_eq!(p.pixels.len(), layout.width * layout.height);
            assert_eq!(p.palette.is_some(), layout.has_palette);
        }
    }
    for layout in CELL_SHEETS {
        if let Ok(cells) = parse_cells(d, layout) {
            assert_eq!(cells.len(), layout.count);
            let n = layout.cell_width * layout.cell_height;
            assert!(cells.iter().all(|c| c.len() == n));
        }
    }
}

fn run_portraits(d: &[u8], _: &mut Rng) {
    let Ok(p) = Portraits::parse(d) else { return };
    assert_eq!(p.faces.len(), d.len() / portraits::RECORD_LEN);
    for face in 0..=255u8 {
        match p.by_face_byte(face) {
            Some(f) => {
                assert_eq!(
                    f.pixels.len(),
                    portraits::FACE_WIDTH * portraits::FACE_HEIGHT
                );
            }
            None => assert!(face == 0 || usize::from(face) > p.faces.len()),
        }
    }
}

fn run_saves(d: &[u8], _: &mut Rng) {
    let s = SaveSlots::parse(d);
    let b = s.to_bytes();
    if d.len() > saves::SLOT_BYTES {
        assert_eq!(b, d, "a file with a tail round-trips exactly");
    } else {
        assert_eq!(b.len(), saves::SAV_LEN);
        let whole = d.len() / 3 * 3;
        assert_eq!(b[..whole], d[..whole]);
    }
    assert_eq!(SaveSlots::parse(&b), s);
    let _ = s.slots.iter().filter(|slot| slot.used()).count();
    let n = SaveNames::parse(d);
    assert!(n.names.iter().all(|x| x.chars().count() <= saves::MAX_LINE));
    assert_eq!(SaveNames::parse(&n.to_bytes()), n);
}

fn run_sprites(d: &[u8], rng: &mut Rng) {
    let mut sheets = vec![
        SpriteSheet::parse_base(d),
        SpriteSheet::parse_rotation_set(d),
    ];
    let mut sizes = vec![0, 3, 32, 40, rng.range(4, 64)];
    if d.len() <= 65_536 {
        // One allocation per frame: keep the tiny frame sizes to inputs of moderate length.
        sizes.extend([1, 2]);
    }
    sheets.extend(sizes.into_iter().map(|size| SpriteSheet::parse(d, size)));
    for s in sheets.into_iter().flatten() {
        assert!(!s.is_empty());
        let frame = s.size() * s.size();
        assert_eq!(s.len() * frame, d.len());
        for i in probes(rng, s.len()) {
            match s.frame(i) {
                Some(f) => assert_eq!(f.len(), frame),
                None => assert!(i >= s.len()),
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Synthetic inputs of the right shape
// ---------------------------------------------------------------------------------------------

fn valid_palette(rng: &mut Rng) -> Vec<u8> {
    let mut v: Vec<u8> = rng.bytes(PALETTE_LEN).iter().map(|b| b & 0x3F).collect();
    if rng.one_in(4) {
        let o = rng.below(PALETTE_LEN);
        v[o] = rng.byte() | 0x40;
    }
    v
}

fn synth_palette(rng: &mut Rng) -> Vec<u8> {
    let mut v = valid_palette(rng);
    if rng.one_in(4) {
        let n = rng.below(100);
        v.extend(rng.bytes(n));
    }
    v
}

fn synth_screen(rng: &mut Rng) -> Vec<u8> {
    let mut v = valid_palette(rng);
    let pages = rng.pick(&[1, 1, 1, 2, 3, 0]);
    v.extend(rng.bytes(pages * screen::PIXELS));
    if rng.one_in(8) {
        let n = rng.range(1, 3);
        v.extend(rng.bytes(n));
    }
    v
}

fn synth_tileset(rng: &mut Rng) -> Vec<u8> {
    let random = rng.below(2000);
    let extra = rng.pick(&[0, 0, 864, random]);
    rng.bytes(tileset::TILE_COUNT * tileset::TILE_BYTES + extra)
}

fn synth_map(rng: &mut Rng) -> Vec<u8> {
    if rng.one_in(8) {
        return rng.bytes(20 * 2000);
    }
    let random = rng.range(1, 64);
    let size = rng.pick(&[201, 201, 200, 20, 1, 2, random]);
    let v: Vec<u8> = rng.bytes(size * size).iter().map(|b| b % 80).collect();
    v
}

fn synth_mission(rng: &mut Rng) -> Vec<u8> {
    let mut v = rng.bytes(mission::MISSION_FILE_LEN);
    for b in &mut v[2 * mission::SLOTS..3 * mission::SLOTS] {
        *b = rng.pick(&[0, 1, 2, 3, 4, 5, 6, 7, 8, mission::EMPTY_SLOT]);
    }
    v
}

fn put16(v: &mut [u8], at: usize, x: u16) {
    v[at..at + 2].copy_from_slice(&x.to_le_bytes());
}

fn synth_mz(rng: &mut Rng) -> Vec<u8> {
    let len = 28 + rng.below(1024);
    let mut v = rng.bytes(len);
    v[..2].copy_from_slice(b"MZ");
    if rng.one_in(2) {
        // A consistent header: small header, page counts that match the length.
        put16(&mut v, 2, (len % 512) as u16);
        put16(&mut v, 4, len.div_ceil(512) as u16);
        put16(&mut v, 8, rng.range(2, 8) as u16);
    }
    v
}

fn synth_pstrings(rng: &mut Rng) -> Vec<u8> {
    let n = rng.below(8);
    let mut v = rng.bytes(n);
    for _ in 0..rng.below(32) {
        let (short, long) = (rng.below(32), rng.below(256));
        let n = rng.pick(&[0, 1, short, long]);
        v.push(n as u8);
        let text: Vec<u8> = rng.bytes(n).iter().map(|b| 0x20 + b % 0x5F).collect();
        v.extend(text);
    }
    if rng.one_in(2) {
        v.push(rng.byte()); // a length byte with its text missing
    }
    v
}

/// Encodes a MIDI variable-length quantity (up to 28 bits).
fn vlq(value: u32, out: &mut Vec<u8>) {
    let value = value & 0x0FFF_FFFF;
    let mut groups = vec![(value & 0x7F) as u8];
    let mut rest = value >> 7;
    while rest > 0 {
        groups.push((rest & 0x7F) as u8 | 0x80);
        rest >>= 7;
    }
    out.extend(groups.iter().rev());
}

fn synth_cmf(rng: &mut Rng) -> Vec<u8> {
    let n_inst = rng.below(9);
    let mut v = vec![0u8; 0x28];
    v[..4].copy_from_slice(b"CTMF");
    put16(&mut v, 4, 0x0101);
    put16(&mut v, 6, 0x28);
    put16(&mut v, 8, (0x28 + 16 * n_inst) as u16);
    put16(&mut v, 0x0A, rng.below(200) as u16);
    let tps = if rng.one_in(16) {
        0
    } else {
        rng.range(1, 400) as u16
    };
    put16(&mut v, 0x0C, tps);
    for b in &mut v[0x14..0x24] {
        *b = rng.below(2) as u8;
    }
    put16(&mut v, 0x24, n_inst as u16);
    put16(&mut v, 0x26, rng.below(1000) as u16);
    v.extend(rng.bytes(16 * n_inst));
    let mut status: Option<u8> = None;
    for _ in 0..rng.below(80) {
        let delta = match rng.below(4) {
            0 => 0,
            1 => rng.below(128) as u32,
            2 => rng.below(1 << 14) as u32,
            _ => rng.next_u64() as u32,
        };
        vlq(delta, &mut v);
        match rng.below(12) {
            0 => {
                v.push(rng.pick(&[0xF0, 0xF7]));
                let n = rng.below(8);
                vlq(n as u32, &mut v);
                v.extend(rng.bytes(n));
            }
            1 => {
                v.push(0xFF);
                let random = rng.byte();
                v.push(rng.pick(&[0x01, 0x03, 0x51, 0x58, random]));
                let n = rng.below(8);
                vlq(n as u32, &mut v);
                v.extend(rng.bytes(n));
            }
            2 => v.push(rng.byte()), // anything, including a bad status
            _ => {
                let kind = rng.pick(&[0x80, 0x90, 0x90, 0xA0, 0xB0, 0xC0, 0xD0, 0xE0])
                    | rng.below(16) as u8;
                let n_data = if matches!(kind & 0xF0, 0xC0 | 0xD0) {
                    1
                } else {
                    2
                };
                if status != Some(kind) || rng.one_in(2) {
                    v.push(kind);
                }
                status = Some(kind);
                for _ in 0..n_data {
                    v.push(rng.byte() & 0x7F);
                }
            }
        }
    }
    if !rng.one_in(8) {
        v.extend([0x00, 0xFF, 0x2F, 0x00]);
    }
    let n = rng.below(8);
    v.extend(rng.bytes(n));
    v
}

fn synth_voc(rng: &mut Rng) -> Vec<u8> {
    let mut v = b"Creative Voice File\x1a".to_vec();
    let first: u16 = if rng.one_in(8) {
        rng.below(64) as u16
    } else {
        26
    };
    v.extend(first.to_le_bytes());
    v.extend(0x010Au16.to_le_bytes());
    v.extend(0x1129u16.to_le_bytes());
    for _ in 0..rng.below(10) {
        let random = rng.byte();
        let kind = rng.pick(&[1, 1, 1, 2, 2, 3, 4, 5, 6, 7, 8, 9, random]);
        let body: Vec<u8> = match kind {
            1 => {
                let (tc, random) = (rng.byte(), rng.byte());
                let mut b = vec![tc, rng.pick(&[0, 1, 2, 3, random])];
                let n = rng.below(600);
                b.extend(rng.bytes(n));
                b
            }
            2 => {
                let n = rng.below(600);
                rng.bytes(n)
            }
            3 => rng.bytes(3),
            4 => rng.bytes(2),
            5 => {
                let n = rng.below(20);
                let mut t: Vec<u8> = rng.bytes(n).iter().map(|b| 0x20 + b % 0x5F).collect();
                t.push(0);
                t
            }
            _ => {
                let n = rng.below(16);
                rng.bytes(n)
            }
        };
        let len = if rng.one_in(16) {
            rng.below(1 << 24)
        } else {
            body.len()
        };
        v.push(kind);
        v.extend(&(len as u32).to_le_bytes()[..3]);
        v.extend(body);
    }
    if !rng.one_in(4) {
        v.push(0);
    }
    v
}

fn dialogue_text(rng: &mut Rng) -> Vec<u8> {
    let n = rng.below(40);
    if rng.one_in(8) {
        rng.bytes(n)
    } else {
        rng.bytes(n).iter().map(|b| 0x20 + b % 0x5F).collect()
    }
}

fn speaker_line(rng: &mut Rng) -> Vec<u8> {
    let random = rng.byte();
    let slot = rng.pick(&[1, 2, random]);
    let mut l = vec![0x01, slot, rng.byte(), rng.byte()];
    l.extend(dialogue_text(rng));
    l
}

fn synth_dialogue(rng: &mut Rng) -> Vec<u8> {
    let mut lines: Vec<Vec<u8>> = Vec::new();
    for _ in 0..rng.range(1, 4) {
        let id = rng.byte();
        lines.push(if id == 0x0D && rng.one_in(2) {
            vec![0x06]
        } else {
            vec![0x06, id]
        });
        for _ in 0..rng.range(1, 3) {
            lines.push(speaker_line(rng));
        }
        for seg in 0..rng.range(1, 3) {
            if seg > 0 && rng.one_in(4) {
                lines.push(speaker_line(rng));
            }
            lines.push(vec![0x03]);
            for _ in 0..rng.below(6) {
                lines.push(match rng.below(5) {
                    0 => vec![0x05],
                    1 => {
                        let slot = rng.pick(&[1, 2]);
                        vec![0x01, slot, rng.byte()]
                    }
                    2 => speaker_line(rng),
                    _ => {
                        let mut l = vec![0x02, 1, rng.byte()];
                        l.extend(dialogue_text(rng));
                        l
                    }
                });
            }
            lines.push(vec![0x04]);
        }
        lines.push(b"d".to_vec());
        if rng.one_in(8) {
            lines.push(dialogue_text(rng));
        }
    }
    let mut v = Vec::new();
    for mut l in lines {
        if l.len() == 1 && rng.one_in(8) {
            l.extend(b"  ");
        }
        v.extend(l);
        v.extend_from_slice(match rng.below(32) {
            0 => b"\n",
            1 => b"\r",
            _ => b"\r\n",
        });
    }
    v
}

fn synth_font(rng: &mut Rng) -> Vec<u8> {
    let kind = rng.pick(&FONT_KINDS);
    let v: Vec<u8> = rng
        .bytes(kind.file_len())
        .iter()
        .map(|&b| if b < 128 { 0 } else { 0xF7 })
        .collect();
    v
}

fn synth_picture(rng: &mut Rng) -> Vec<u8> {
    let i = rng.below(STILLS.len() + CELL_SHEETS.len());
    match STILLS.get(i) {
        Some(s) => {
            let mut v = if s.has_palette {
                valid_palette(rng)
            } else {
                Vec::new()
            };
            v.extend(rng.bytes(s.width * s.height));
            v
        }
        None => {
            let c = &CELL_SHEETS[i - STILLS.len()];
            rng.bytes(c.cell_width * c.cell_height * c.count)
        }
    }
}

fn synth_portraits(rng: &mut Rng) -> Vec<u8> {
    let mut v = Vec::new();
    for _ in 0..rng.range(1, 3) {
        v.extend(valid_palette(rng));
        v.extend(rng.bytes(portraits::FACE_WIDTH * portraits::FACE_HEIGHT));
    }
    if rng.one_in(8) {
        v.push(rng.byte());
    }
    v
}

fn synth_saves(rng: &mut Rng) -> Vec<u8> {
    if rng.one_in(2) {
        let mut v = rng.bytes(saves::SAV_LEN);
        for slot in 0..saves::SLOTS {
            v[slot * 3] = rng.below(23) as u8;
        }
        return v;
    }
    let mut v = Vec::new();
    for _ in 0..rng.below(13) {
        let (short, long) = (rng.below(40), rng.below(300));
        let n = rng.pick(&[0, 19, short, long]);
        v.extend(rng.bytes(n).iter().map(|b| 0x20 + b % 0x5F));
        v.extend_from_slice(if rng.one_in(4) { b"\n" } else { b"\r\n" });
    }
    v
}

fn synth_sprites(rng: &mut Rng) -> Vec<u8> {
    let (small, large) = (1024 * rng.range(1, 8), 1600 * rng.range(1, 8));
    let len = rng.pick(&[
        sprites::BASE_FRAMES * sprites::BASE_SIZE * sprites::BASE_SIZE,
        sprites::ROTATED_CELLS * sprites::ROTATED_SIZE * sprites::ROTATED_SIZE,
        small,
        large,
    ]);
    rng.bytes(len)
}

// ---------------------------------------------------------------------------------------------
// Running one input
// ---------------------------------------------------------------------------------------------

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_owned()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "non-string panic payload".to_owned()
    }
}

/// Runs `t` on `data`; a panic or a suspiciously slow parse fails with a reproducer.
/// `origin` describes the input (file, truncation, mutations, seeds).
fn check(t: &Target, data: &[u8], probe_seed: u64, origin: impl Fn() -> String) {
    let start = Instant::now();
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        (t.run)(data, &mut Rng::new(probe_seed));
    }));
    let elapsed = start.elapsed();
    if let Err(payload) = outcome {
        panic!(
            "{} panicked on {} ({} bytes, probe seed {probe_seed:#x}): {}",
            t.name,
            origin(),
            data.len(),
            panic_message(payload.as_ref())
        );
    }
    assert!(
        elapsed < SLOW_INPUT,
        "{} took {elapsed:?} on {} ({} bytes): suspected hang",
        t.name,
        origin(),
        data.len()
    );
}

fn proptest_config(cases: u32) -> ProptestConfig {
    ProptestConfig {
        cases,
        // Failures print the minimal case; nothing is written next to the sources.
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

/// The original files of every target, read once: (target index, file name, bytes).
fn load_originals(dir: &Path) -> Vec<(usize, String, Vec<u8>)> {
    let mut out = Vec::new();
    for (ti, t) in TARGETS.iter().enumerate() {
        for name in (t.files)() {
            let bytes = fs::read(dir.join(&name)).unwrap_or_else(|e| panic!("{name}: {e}"));
            out.push((ti, name, bytes));
        }
    }
    out
}

/// Every regular file of the data directory (for cross-format runs).
fn load_all_files(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out: Vec<(String, Vec<u8>)> = fs::read_dir(dir)
        .expect("data directory")
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let bytes = fs::read(e.path()).unwrap_or_else(|err| panic!("{name}: {err}"));
            (name, bytes)
        })
        .collect();
    out.sort();
    out
}

// ---------------------------------------------------------------------------------------------
// Default tier: random and synthetic inputs (no data needed)
// ---------------------------------------------------------------------------------------------

proptest! {
    #![proptest_config(proptest_config(1024))]

    /// Short random inputs through every parser.
    #[test]
    fn short_random_bytes_never_panic(data in vec(any::<u8>(), 0..512), seed in any::<u64>()) {
        for t in TARGETS {
            check(t, &data, seed, || "random bytes".to_owned());
        }
    }

    /// Random bytes of each format's exact sizes (and a few bytes either side), with a few
    /// bytes set by proptest so that failures shrink to the bytes that matter.
    #[test]
    fn exact_size_random_bytes_never_panic(
        ti in 0..TARGETS.len(),
        pick in any::<Index>(),
        delta in -2isize..=2,
        seed in any::<u64>(),
        patches in vec((any::<Index>(), any::<u8>()), 0..8),
    ) {
        let t = &TARGETS[ti];
        let sizes = (t.sizes)();
        let len = pick.get(&sizes).saturating_add_signed(delta);
        let mut data = Rng::new(seed).bytes(len);
        for (i, v) in &patches {
            if !data.is_empty() {
                let at = i.index(data.len());
                data[at] = *v;
            }
        }
        check(t, &data, seed, || format!("random bytes of length {len} (seed {seed:#x})"));
    }

    /// Valid-shaped inputs with random content, then 0..=4 random mutations.
    #[test]
    fn synthetic_inputs_never_panic(
        ti in 0..TARGETS.len(),
        seed in any::<u64>(),
        mutations in 0usize..=4,
    ) {
        let t = &TARGETS[ti];
        let mut rng = Rng::new(seed);
        let mut data = (t.synth)(&mut rng);
        let ops = mutate(&mut data, &mut rng, mutations);
        check(t, &data, seed, || format!("synthetic {} (seed {seed:#x}) + [{ops}]", t.name));
    }

    /// A [`ByteReader`] driven by arbitrary operations keeps its invariants.
    #[test]
    fn byte_reader_ops_keep_invariants(
        data in vec(any::<u8>(), 0..64),
        ops in vec((any::<u8>(), prop_oneof![0usize..80, any::<usize>()]), 0..48),
    ) {
        reader_ops(&data, &ops);
    }

    /// Pascal strings at any offset, and sequences of them.
    #[test]
    fn pstrings_never_panic(
        data in vec(any::<u8>(), 0..600),
        at in prop_oneof![0usize..700, any::<usize>()],
        count in 0usize..300,
    ) {
        if let Ok(s) = mz::pstring(&data, at) {
            prop_assert_eq!(s.len(), usize::from(data[at]));
        }
        if let Ok(seq) = mz::pstring_seq(&data, at, count) {
            prop_assert_eq!(seq.len(), count);
            prop_assert!(seq.iter().map(|s| 1 + s.len()).sum::<usize>() <= data.len().saturating_sub(at));
        }
    }

    /// Lookups by name and by character never panic on arbitrary text.
    #[test]
    fn name_and_glyph_lookups_never_panic(name in "\\PC{0,16}", c in any::<char>()) {
        let _ = Picture::layout(&name);
        let _ = cell_layout(&name);
        for kind in FONT_KINDS {
            if let Some(i) = kind.glyph_index(c) {
                let (w, h) = kind.size();
                prop_assert!(i < kind.file_len() / (w * h));
            }
        }
        prop_assert_eq!(palette::expand6(c as u8) >> 2, (c as u8) & 0x3F);
        prop_assert!(voc::sample_rate(c as u8) >= 3906);
    }

    /// `SpriteSheet::parse` with any frame size the caller may pass.
    #[test]
    fn sprite_sheets_of_any_frame_size(
        len in 0usize..4096,
        size in prop_oneof![0usize..80, 0usize..=0xFFFF_FFFF, any::<usize>()],
        seed in any::<u64>(),
    ) {
        // BUG-15: `SpriteSheet::parse` squares the caller's frame size unchecked
        // (sprites.rs:30), so a size above u32::MAX panics with an overflow in debug builds
        // (and wraps in release). Minimal case: `SpriteSheet::parse(&[], 1 << 32)`.
        prop_assume!(size.checked_mul(size).is_some());
        let data = Rng::new(seed).bytes(len);
        if let Ok(s) = SpriteSheet::parse(&data, size) {
            prop_assert_eq!(s.len() * s.size() * s.size(), len);
            prop_assert!(s.frame(s.len()).is_none());
        }
    }

    /// Caller-made still and cell layouts (the shipped ones are covered by the targets).
    #[test]
    fn custom_layouts_never_panic(
        width in prop_oneof![0usize..48, any::<usize>()],
        height in prop_oneof![0usize..48, any::<usize>()],
        count in prop_oneof![0usize..6, any::<usize>()],
        has_palette in any::<bool>(),
        extra in 0usize..2,
        seed in any::<u64>(),
    ) {
        let pixels = width.checked_mul(height);
        let sheet = pixels.and_then(|p| p.checked_mul(count));
        // Inputs are at most a few KiB whatever the layout claims.
        let small = |n: Option<usize>| n.filter(|&n| n <= 4096);
        let mut rng = Rng::new(seed);

        // BUG-15: `Picture::parse` computes `width * height` and `+ 768` unchecked
        // (picture.rs:99, :101): an overflow panics in debug builds. Minimal case: a
        // `StillLayout` of width `usize::MAX / 4 + 1` and height 5, on empty data.
        if pixels.and_then(|p| p.checked_add(PALETTE_LEN)).is_some() {
            let still = StillLayout { name: "fuzz", width, height, has_palette };
            let mut data = if has_palette { valid_palette(&mut rng) } else { Vec::new() };
            let n = small(pixels).unwrap_or_else(|| rng.below(64)) + extra;
            data.extend(rng.bytes(n));
            if let Ok(p) = Picture::parse(&data, &still) {
                prop_assert_eq!(Some(p.pixels.len()), pixels);
            }
        }

        // BUG-14: `parse_cells` panics in every build when a layout's cell is empty
        // (`cell_width * cell_height == 0`): `chunks_exact(0)` (picture.rs:130). Minimal case:
        // `parse_cells(&[], &CellLayout { cell_width: 0, cell_height: 0, count: 0, .. })`.
        // BUG-15 again: `cell_width * cell_height * count` is unchecked (picture.rs:128, :129).
        if sheet.is_some() && pixels != Some(0) {
            let cells = CellLayout { name: "fuzz", cell_width: width, cell_height: height, count };
            let len = small(sheet).unwrap_or_else(|| rng.below(64)) + extra;
            let data = rng.bytes(len);
            if let Ok(v) = parse_cells(&data, &cells) {
                prop_assert_eq!(v.len(), count);
            }
        }
    }
}

proptest! {
    #![proptest_config(proptest_config(64))]

    /// Long random inputs (up to 64 KiB) through every parser.
    #[test]
    fn long_random_bytes_never_panic(len in 0usize..65_536, seed in any::<u64>()) {
        let data = Rng::new(seed).bytes(len);
        for t in TARGETS {
            check(t, &data, seed, || format!("random bytes of length {len} (seed {seed:#x})"));
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Default tier: original files (data-gated)
// ---------------------------------------------------------------------------------------------

/// Every parser on every original file, unmodified: each format's own files and, across
/// formats, every file of the installation.
#[test]
fn original_files_never_panic_any_parser() {
    let dir = require_data!();
    for (ti, name, bytes) in load_originals(&dir) {
        check(&TARGETS[ti], &bytes, name_seed(&name), || name.clone());
    }
    let all = load_all_files(&dir);
    assert!(all.len() >= 218, "{} files", all.len());
    for (name, bytes) in &all {
        for t in TARGETS {
            check(t, bytes, name_seed(name), || {
                format!("{name} (cross-format)")
            });
        }
    }
}

/// Every original file of each format truncated at many lengths.
#[test]
fn original_files_truncated_never_panic() {
    let dir = require_data!();
    for (ti, name, bytes) in load_originals(&dir) {
        let t = &TARGETS[ti];
        let points = if t.name == "dialogue" { 24 } else { 96 };
        for n in truncations(bytes.len(), &(t.sizes)(), points) {
            check(t, &bytes[..n], name_seed(&name) ^ n as u64, || {
                format!("{name} truncated to {n} bytes")
            });
        }
    }
}

/// Every original file of each format with seeded random mutations.
#[test]
fn original_files_mutated_never_panic() {
    let dir = require_data!();
    for (ti, name, bytes) in load_originals(&dir) {
        let t = &TARGETS[ti];
        let rounds = match bytes.len() {
            0..=8192 => 128,
            8193..=70_000 => 32,
            _ => 8,
        };
        for round in 0..rounds {
            let seed = mix(name_seed(&name), round);
            let mut rng = Rng::new(seed);
            let mut data = bytes.clone();
            let n = rng.range(1, 6);
            let ops = mutate(&mut data, &mut rng, n);
            check(t, &data, seed, || {
                format!("{name} + [{ops}] (seed {seed:#x})")
            });
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Long tier
// ---------------------------------------------------------------------------------------------

fn env_u64(name: &str) -> Option<u64> {
    let v = std::env::var(name).ok()?;
    Some(
        v.trim()
            .parse()
            .unwrap_or_else(|e| panic!("{name}={v:?}: {e}")),
    )
}

/// What the long test's worker is running, for the watchdog.
#[derive(Debug, Clone, Copy)]
struct Current {
    started: Instant,
    target: &'static str,
    kind: &'static str,
    round: u64,
}

/// One round of the long test: every target gets a random, an exact-size, a synthetic, an
/// original-file (truncated or mutated) and a cross-format input.
fn long_round(
    base: u64,
    round: u64,
    originals: &[(usize, String, Vec<u8>)],
    all: &[(String, Vec<u8>)],
    counts: &mut [u64],
    current: &Mutex<Option<Current>>,
) {
    let seed = mix(base, round);
    let mut rng = Rng::new(seed);
    let at = |ti: usize, kind: &'static str| {
        *current.lock().expect("watchdog lock") = Some(Current {
            started: Instant::now(),
            target: TARGETS[ti].name,
            kind,
            round,
        });
    };
    let origin =
        |kind: &str, what: &str| format!("{kind} {what} (TD2192_FUZZ_SEED={base}, round {round})");
    for (ti, t) in TARGETS.iter().enumerate() {
        // Random bytes, mostly short.
        let len = match rng.below(4) {
            0 => rng.below(64),
            1 => rng.below(1024),
            2 => rng.below(8192),
            _ => rng.below(65_536),
        };
        let data = rng.bytes(len);
        at(ti, "random");
        check(t, &data, seed, || origin("random", &format!("{len} bytes")));

        // Exact sizes, a few bytes either side.
        let sizes = (t.sizes)();
        let len = rng
            .pick(&sizes)
            .saturating_add_signed(rng.range(0, 4) as isize - 2);
        let mut data = rng.bytes(len);
        let n = rng.below(3);
        let ops = mutate(&mut data, &mut rng, n);
        at(ti, "exact-size");
        check(t, &data, seed, || {
            origin("exact-size", &format!("{len} [{ops}]"))
        });

        // Synthetic, mutated.
        let mut data = (t.synth)(&mut rng);
        let n = rng.below(5);
        let ops = mutate(&mut data, &mut rng, n);
        at(ti, "synthetic");
        check(t, &data, seed, || origin("synthetic", &format!("[{ops}]")));
        counts[ti] += 3;

        // An original file of this format, truncated or mutated.
        let own: Vec<&(usize, String, Vec<u8>)> =
            originals.iter().filter(|(i, ..)| *i == ti).collect();
        if !own.is_empty() {
            let (_, name, bytes) = own[rng.below(own.len())];
            let mut data = bytes.clone();
            let what = if rng.one_in(3) {
                let n = if rng.one_in(2) {
                    bytes.len() - rng.below(bytes.len().min(256) + 1)
                } else {
                    rng.below(bytes.len() + 1)
                };
                data.truncate(n);
                format!("{name} truncated to {n}")
            } else {
                let n = rng.range(1, 8);
                format!("{name} + [{}]", mutate(&mut data, &mut rng, n))
            };
            at(ti, "original");
            check(t, &data, seed, || origin("original", &what));
            counts[ti] += 1;
        }

        // Cross-format: another format's original file or synthetic input, lightly mutated.
        let (what, mut data) = if !all.is_empty() && rng.one_in(2) {
            let (name, bytes) = &all[rng.below(all.len())];
            (name.clone(), bytes.clone())
        } else {
            let other = &TARGETS[rng.below(TARGETS.len())];
            (format!("synthetic {}", other.name), (other.synth)(&mut rng))
        };
        let n = rng.below(3);
        let ops = mutate(&mut data, &mut rng, n);
        at(ti, "cross-format");
        check(t, &data, seed, || {
            origin("cross-format", &format!("{what} [{ops}]"))
        });
        counts[ti] += 1;
    }
}

/// Runs every property with fresh seeds for `TD2192_FUZZ_SECS` seconds.
#[test]
#[ignore = "long fuzz; run with --release -- --ignored"]
fn long_fuzz() {
    let secs = env_u64("TD2192_FUZZ_SECS").unwrap_or(60);
    let base = env_u64("TD2192_FUZZ_SEED").unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(1, |d| d.as_nanos() as u64)
    });
    println!("td-formats long fuzz: {secs} s, TD2192_FUZZ_SEED={base}");
    let (originals, all) = match data_dir() {
        Some(dir) => (load_originals(&dir), load_all_files(&dir)),
        None => {
            println!("TD2192_DATA not set: random and synthetic inputs only");
            (Vec::new(), Vec::new())
        }
    };
    let current: Arc<Mutex<Option<Current>>> = Arc::default();
    let deadline = Instant::now() + Duration::from_secs(secs);
    let worker = {
        let current = Arc::clone(&current);
        std::thread::spawn(move || {
            let mut counts = vec![0u64; TARGETS.len()];
            let mut round = 0;
            while Instant::now() < deadline {
                long_round(base, round, &originals, &all, &mut counts, &current);
                round += 1;
            }
            (round, counts)
        })
    };
    // Watchdog: a parser stuck on one input fails the test with that input's description.
    while !worker.is_finished() {
        std::thread::sleep(Duration::from_millis(100));
        if let Some(c) = *current.lock().expect("watchdog lock")
            && c.started.elapsed() > HANG_LIMIT
            && !worker.is_finished()
        {
            panic!(
                "{} stuck for over {HANG_LIMIT:?} on a {} input (TD2192_FUZZ_SEED={base}, \
                 round {})",
                c.target, c.kind, c.round
            );
        }
    }
    let (rounds, counts) = match worker.join() {
        Ok(v) => v,
        Err(payload) => std::panic::resume_unwind(payload),
    };
    let mut report = format!("{rounds} rounds; inputs per parser:");
    for (t, n) in TARGETS.iter().zip(&counts) {
        let _ = write!(report, "\n  {:<12} {n}", t.name);
    }
    println!("{report}");
    println!("total {}", counts.iter().sum::<u64>());
}
