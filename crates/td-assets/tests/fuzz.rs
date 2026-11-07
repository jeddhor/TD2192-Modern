//! M12-06: "never panics" fuzz-style tests for the `td-assets` input paths (MDG §11,
//! ADG §10.6): the ZIP import (`archive::read_zip_from`, and `DataDir::open` on an archive), the
//! executable-resource extraction (`ExeResources::load`), the DOS 8.3 name rule (`dos83`,
//! `manifest::entry`) and `validate`.
//!
//! Inputs:
//!
//! * random bytes of random length, also with a ZIP end-of-central-directory record appended
//!   so that parsing gets past the signature search;
//! * synthetic ZIPs built with the `zip` writer (stored and deflated entries, folders, odd
//!   names), truncated at every length and with seeded random mutations;
//! * hand-built ZIPs whose entries use every compression method number the reader may meet
//!   (stored, the PKZIP 1.x methods Shrink, Reduce and Implode, Deflate, unsupported ones), with
//!   random compressed data, random flags, random declared sizes up to and past `MAX_ENTRY`;
//! * with `TD2192_DATA` set: an in-memory ZIP of a few original files, truncated and mutated;
//!   and copies of the four executables (in a temporary folder that is removed afterwards),
//!   truncated and mutated, through `ExeResources::load` and `validate`.
//!
//! `ExeResources::load` only extracts from an executable whose SHA-256 matches its map, so a
//! changed executable can only take the "version mismatch" path; the extraction itself
//! (`exe_res::extract`, private) only ever sees the compiled-in maps and the exact original
//! files, and its byte readers (`td_formats::mz::pstring`, `pstring_seq`) are fuzzed in
//! `td-formats`.
//!
//! The default tests are bounded. `long_fuzz` (ignored) loops with fresh seeds for
//! `TD2192_FUZZ_SECS` seconds (default 60) from `TD2192_FUZZ_SEED` (default: from the clock),
//! under a watchdog, and prints the inputs tried per path:
//!
//! ```text
//! TD2192_FUZZ_SECS=600 cargo test -p td-assets --release --test fuzz -- --ignored --nocapture
//! ```

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::io::{Cursor, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use proptest::collection::vec;
use proptest::prelude::*;
use td_assets::archive::{ArchiveFile, MAX_ENTRIES, MAX_ENTRY, read_zip_from};
use td_assets::{DataDir, ExeResources, MapStatus, dos83, manifest, validate};
use td_test_support::{data_dir, require_data};
use zip::CompressionMethod;
use zip::write::SimpleFileOptions;

/// An input slower than this fails the default tests as a suspected hang.
const SLOW_INPUT: Duration = Duration::from_secs(10);
/// The long test's watchdog limit for one input.
const HANG_LIMIT: Duration = Duration::from_secs(20);

/// The executables with compiled-in resource maps.
const EXES: [&str; 4] = ["file0000.exe", "file0001.exe", "td2.exe", "td3.exe"];

/// Small original files of several formats for the in-memory archive.
const ZIP_FILES: &[&str] = &[
    "td.cfg",
    "td.pal",
    "td.sav",
    "td.stt",
    "mood.cmf",
    "scratch.voc",
    "td.lt2",
    "td.hel",
    "m1.td",
    "td1.exe",
];

// ---------------------------------------------------------------------------------------------
// Deterministic randomness and mutations
// ---------------------------------------------------------------------------------------------

/// splitmix64, seeded so every run reproduces.
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

    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }

    fn range(&mut self, lo: usize, hi: usize) -> usize {
        lo + self.below(hi - lo + 1)
    }

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

fn mix(a: u64, b: u64) -> u64 {
    Rng::new(a ^ b.wrapping_mul(0xD1B5_4A32_D192_ED03)).next_u64()
}

/// Applies one random mutation and describes it. Offsets favour the start and the end of the
/// data, where ZIP headers and the central directory are.
fn mutate_once(data: &mut Vec<u8>, rng: &mut Rng) -> String {
    let len = data.len();
    let at = |rng: &mut Rng, len: usize, width: usize| {
        let span = len + 1 - width;
        match rng.below(3) {
            0 => rng.below(span.min(64)),
            1 => span - 1 - rng.below(span.min(128)),
            _ => rng.below(span),
        }
    };
    match rng.below(8) {
        0 if len > 0 => {
            let o = at(rng, len, 1);
            let bit = 1u8 << rng.below(8);
            data[o] ^= bit;
            format!("flip[{o}]^={bit:#04x}")
        }
        1 if len > 0 => {
            let o = at(rng, len, 1);
            let random = rng.byte();
            let v = rng.pick(&[0, 1, 0x7F, 0x80, 0xFF, random]);
            data[o] = v;
            format!("set[{o}]={v:#04x}")
        }
        2 if len >= 2 => {
            let o = at(rng, len, 2);
            let random = rng.next_u64() as u16;
            let v = rng.pick(&[0, 1, 0xFF, 0x7FFF, 0xFFFF, len as u16, random]);
            data[o..o + 2].copy_from_slice(&v.to_le_bytes());
            format!("u16[{o}]={v:#x}")
        }
        3 if len >= 4 => {
            let o = at(rng, len, 4);
            let random = rng.next_u64() as u32;
            let v = rng.pick(&[
                0,
                0xFFFF_FFFF,
                0x7FFF_FFFF,
                len as u32,
                len as u32 + 1,
                random,
            ]);
            data[o..o + 4].copy_from_slice(&v.to_le_bytes());
            format!("u32[{o}]={v:#x}")
        }
        4 if len > 0 => {
            let o = rng.below(len);
            let n = rng.range(1, 16).min(len - o);
            data.drain(o..o + n);
            format!("delete[{o}..{}]", o + n)
        }
        5 if len > 0 => {
            let n = rng.below(len);
            data.truncate(n);
            format!("truncate({n})")
        }
        6 => {
            let o = rng.below(len + 1);
            let n = rng.range(1, 16);
            let ins = rng.bytes(n);
            let what = format!("insert[{o}]={ins:02x?}");
            data.splice(o..o, ins);
            what
        }
        _ if len > 1 => {
            let src = rng.below(len);
            let dst = rng.below(len);
            let n = rng.range(1, 64).min(len - src).min(len - dst);
            data.copy_within(src..src + n, dst);
            format!("copy[{src}..{}]->{dst}", src + n)
        }
        _ => {
            data.push(rng.byte());
            "append".to_owned()
        }
    }
}

fn mutate(data: &mut Vec<u8>, rng: &mut Rng, n: usize) -> String {
    let ops: Vec<String> = (0..n).map(|_| mutate_once(data, rng)).collect();
    ops.join(", ")
}

// ---------------------------------------------------------------------------------------------
// ZIP inputs
// ---------------------------------------------------------------------------------------------

/// A ZIP built with the `zip` writer: stored and deflated entries, folders, odd names.
fn synth_zip(rng: &mut Rng) -> Vec<u8> {
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for i in 0..rng.range(1, 6) {
        let stem = rng.pick(&[
            "td.pal",
            "TD2192/HELIHANGAR.SCR",
            "a/b/c.txt",
            "dir/",
            "x",
            "\u{e9}t\u{e9}.voc",
            "..\\evil.cfg",
            "../../escape.cfg",
            "name.with.many.dots.",
            "",
        ]);
        let name = format!("{i}{stem}");
        let options = SimpleFileOptions::default();
        if name.ends_with('/') {
            let _ = w.add_directory(name, options);
            continue;
        }
        let method = rng.pick(&[CompressionMethod::Stored, CompressionMethod::Deflated]);
        let (short, long) = (rng.below(64), rng.below(4096));
        let len = rng.pick(&[0, 1, short, long]);
        let content = if rng.one_in(2) {
            rng.bytes(len)
        } else {
            vec![rng.byte(); len]
        };
        if w.start_file(name, options.compression_method(method))
            .is_ok()
        {
            w.write_all(&content).expect("write to memory");
        }
    }
    w.finish().expect("finish in memory").into_inner()
}

/// One entry of a hand-built ZIP.
#[derive(Debug, Clone)]
struct RawEntry {
    name: Vec<u8>,
    method: u16,
    flags: u16,
    crc: u32,
    data: Vec<u8>,
    /// Declared uncompressed size.
    size: u32,
    extra: Vec<u8>,
}

/// CRC-32 (IEEE, as ZIP uses it), bit by bit.
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn put_u16(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_le_bytes());
}

fn put_u32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}

/// Lays out local headers, data, the central directory and the end record.
fn raw_zip(entries: &[RawEntry], comment: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for e in entries {
        let offset = out.len() as u32;
        put_u32(&mut out, 0x0403_4B50);
        put_u16(&mut out, 20);
        put_u16(&mut out, e.flags);
        put_u16(&mut out, e.method);
        put_u16(&mut out, 0); // time
        put_u16(&mut out, 0x21); // date
        put_u32(&mut out, e.crc);
        put_u32(&mut out, e.data.len() as u32);
        put_u32(&mut out, e.size);
        put_u16(&mut out, e.name.len() as u16);
        put_u16(&mut out, e.extra.len() as u16);
        out.extend_from_slice(&e.name);
        out.extend_from_slice(&e.extra);
        out.extend_from_slice(&e.data);

        put_u32(&mut central, 0x0201_4B50);
        put_u16(&mut central, 20); // made by
        put_u16(&mut central, 20); // needed
        put_u16(&mut central, e.flags);
        put_u16(&mut central, e.method);
        put_u16(&mut central, 0);
        put_u16(&mut central, 0x21);
        put_u32(&mut central, e.crc);
        put_u32(&mut central, e.data.len() as u32);
        put_u32(&mut central, e.size);
        put_u16(&mut central, e.name.len() as u16);
        put_u16(&mut central, e.extra.len() as u16);
        put_u16(&mut central, 0); // comment
        put_u16(&mut central, 0); // disk
        put_u16(&mut central, 0); // internal attributes
        put_u32(&mut central, 0); // external attributes
        put_u32(&mut central, offset);
        central.extend_from_slice(&e.name);
        central.extend_from_slice(&e.extra);
    }
    let cd_offset = out.len() as u32;
    out.extend_from_slice(&central);
    eocd(
        &mut out,
        entries.len() as u16,
        central.len() as u32,
        cd_offset,
        comment,
    );
    out
}

/// Appends an end-of-central-directory record.
fn eocd(out: &mut Vec<u8>, entries: u16, cd_size: u32, cd_offset: u32, comment: &[u8]) {
    put_u32(out, 0x0605_4B50);
    put_u16(out, 0);
    put_u16(out, 0);
    put_u16(out, entries);
    put_u16(out, entries);
    put_u32(out, cd_size);
    put_u32(out, cd_offset);
    put_u16(out, comment.len() as u16);
    out.extend_from_slice(comment);
}

/// A hand-built ZIP whose entries use any compression method number with random data, flags
/// and declared sizes. `max_size` bounds the declared sizes that are not edge values.
fn synth_raw_zip(rng: &mut Rng, max_size: usize) -> Vec<u8> {
    let entries: Vec<RawEntry> = (0..rng.range(1, 4))
        .map(|i| {
            let random_method = rng.next_u64() as u16;
            // 0 stored, 1 shrink, 2..=5 reduce, 6 implode, 8 deflate, the rest unsupported.
            let method = rng.pick(&[
                0,
                1,
                1,
                2,
                3,
                4,
                5,
                6,
                6,
                8,
                8,
                9,
                12,
                14,
                93,
                95,
                98,
                99,
                random_method,
            ]);
            let random_flags = rng.next_u64() as u16;
            // Implode reads bits 1 and 2 (dictionary size, literal tree); bit 0 = encrypted,
            // bit 3 = data descriptor, bit 11 = UTF-8 names.
            let flags = rng.pick(&[0, 0, 2, 4, 6, 1, 8, 0x800, random_flags]);
            let (small, medium) = (rng.below(256), rng.below(4096));
            let n = rng.pick(&[0, 1, 16, small, medium]);
            let data = rng.bytes(n);
            let declared = rng.below(max_size + 1) as u32;
            let size = rng.pick(&[
                0,
                1,
                data.len() as u32,
                data.len() as u32 * 4,
                declared,
                MAX_ENTRY as u32,
                MAX_ENTRY as u32 + 1,
                u32::MAX,
            ]);
            let extra = match rng.below(6) {
                0 => {
                    // A ZIP64 extended-information field of some length.
                    let n = rng.pick(&[0, 8, 16, 24, 28, 3]);
                    let mut e = Vec::new();
                    put_u16(&mut e, 0x0001);
                    put_u16(&mut e, n as u16);
                    e.extend(rng.bytes(n));
                    e
                }
                1 => {
                    let n = rng.below(12);
                    rng.bytes(n)
                }
                _ => Vec::new(),
            };
            let ext = rng.pick(&["pal", "scr", "exe", "", "voc"]);
            // A stored entry often carries its true checksum and size, so that it reads.
            let stored_ok = method == 0 && rng.one_in(2);
            RawEntry {
                name: format!("dir/f{i}.{ext}").into_bytes(),
                method,
                flags: if stored_ok { 0 } else { flags },
                crc: if stored_ok {
                    crc32(&data)
                } else {
                    rng.next_u64() as u32
                },
                size: if stored_ok { data.len() as u32 } else { size },
                data,
                extra,
            }
        })
        .collect();
    let n = rng.pick(&[0, 0, 0, 5, 300]);
    let comment = rng.bytes(n);
    raw_zip(&entries, &comment)
}

/// Random bytes followed by an end-of-central-directory record with random fields.
fn random_with_eocd(rng: &mut Rng) -> Vec<u8> {
    let n = rng.below(512);
    let mut v = rng.bytes(n);
    let (r1, r2, r3) = (
        rng.next_u64() as u16,
        rng.next_u64() as u32,
        rng.next_u64() as u32,
    );
    let entries = rng.pick(&[0, 1, 2, 0xFFFF, r1]);
    let cd_size = rng.pick(&[0, 46, n as u32, 0xFFFF_FFFF, r2]);
    let cd_offset = rng.pick(&[0, n as u32 / 2, n as u32, 0xFFFF_FFFF, r3]);
    let m = rng.pick(&[0, 3]);
    let comment = rng.bytes(m);
    eocd(&mut v, entries, cd_size, cd_offset, &comment);
    v
}

/// An in-memory ZIP of a few original files (stored and deflated, in a top folder).
fn original_zip(dir: &Path) -> Vec<u8> {
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (i, name) in ZIP_FILES.iter().enumerate() {
        let bytes = fs::read(dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        let method = if i % 2 == 0 {
            CompressionMethod::Deflated
        } else {
            CompressionMethod::Stored
        };
        let options = SimpleFileOptions::default().compression_method(method);
        w.start_file(format!("TD2192/{}", name.to_uppercase()), options)
            .expect("start entry");
        w.write_all(&bytes).expect("write to memory");
    }
    w.finish().expect("finish in memory").into_inner()
}

// ---------------------------------------------------------------------------------------------
// Checks
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

/// Runs `f`; a panic or a suspiciously slow run fails with a reproducer.
fn guarded(what: &str, origin: impl Fn() -> String, f: impl FnOnce()) {
    let start = Instant::now();
    if let Err(payload) = catch_unwind(AssertUnwindSafe(f)) {
        panic!(
            "{what} panicked on {}: {}",
            origin(),
            panic_message(payload.as_ref())
        );
    }
    let elapsed = start.elapsed();
    assert!(
        elapsed < SLOW_INPUT,
        "{what} took {elapsed:?} on {}: suspected hang",
        origin()
    );
}

/// What a successful archive read must look like.
fn check_files(files: &BTreeMap<String, ArchiveFile>) {
    assert!(files.len() <= MAX_ENTRIES);
    for (key, f) in files {
        let base = f.path.rsplit(['/', '\\']).next().unwrap_or(&f.path);
        assert!(!base.is_empty(), "{f:?}");
        assert_eq!(*key, dos83(base), "{f:?}");
        if let Ok(data) = &f.data {
            assert!(data.len() as u64 <= MAX_ENTRY, "{f:?}");
        }
        let _ = format!("{f:?}");
    }
}

/// Reads `bytes` as a ZIP archive.
fn check_zip(bytes: &[u8], origin: impl Fn() -> String) {
    guarded(
        "read_zip_from",
        || format!("{} ({} bytes)", origin(), bytes.len()),
        || {
            if let Ok(files) = read_zip_from(Cursor::new(bytes), Path::new("fuzz.zip")) {
                check_files(&files);
            }
        },
    );
}

/// A fresh, empty temporary folder, removed when dropped.
#[derive(Debug)]
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("td-assets-fuzz-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temporary folder");
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Loads the executable resources of the folder `dir` and walks the result.
fn check_exe_res(dir: &Path, expect: &[(&str, bool)], origin: impl Fn() -> String) {
    guarded("ExeResources::load", &origin, || {
        let data = DataDir::open(dir).expect("open temporary folder");
        let res = ExeResources::load(&data).expect("compiled-in maps load");
        assert_eq!(res.status.len(), EXES.len());
        for &(exe, loaded) in expect {
            let status = res.status.iter().find(|(e, _)| e == exe).map(|(_, s)| s);
            assert_eq!(
                matches!(status, Some(MapStatus::Loaded { .. })),
                loaded,
                "{exe}: {status:?}"
            );
        }
        for id in res.ids() {
            assert!(res.get(id).is_some());
            let _ = (res.text(id), res.texts(id));
        }
        for exe in EXES {
            for at in [0, 1, 0x100, usize::MAX] {
                let _ = res.text_at(exe, at);
            }
        }
        let _ = res.text_at("", 0);
    });
}

/// Validates the folder `dir` and checks the report adds up.
fn check_validate(dir: &Path, origin: impl Fn() -> String) {
    guarded("validate", &origin, || {
        let data = DataDir::open(dir).expect("open temporary folder");
        let report = validate(&data);
        let c = report.counts();
        assert_eq!(
            c.ok + c.modified + c.missing + c.unreadable,
            manifest::entries().len()
        );
        let _ = (report.playable_episodes(), report.intro_available());
        for name in data.names() {
            let _ = data.read(name);
        }
    });
}

fn proptest_config(cases: u32) -> ProptestConfig {
    ProptestConfig {
        cases,
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

// ---------------------------------------------------------------------------------------------
// Default tier (no data needed)
// ---------------------------------------------------------------------------------------------

proptest! {
    #![proptest_config(proptest_config(1024))]

    /// Random bytes as an archive.
    #[test]
    fn random_bytes_as_zip_never_panic(data in vec(any::<u8>(), 0..2048)) {
        check_zip(&data, || "random bytes".to_owned());
    }

    /// Random bytes ending in an end-of-central-directory record with random fields.
    #[test]
    fn random_bytes_with_end_record_never_panic(seed in any::<u64>()) {
        let data = random_with_eocd(&mut Rng::new(seed));
        check_zip(&data, || format!("random bytes + end record (seed {seed:#x})"));
    }

    /// Writer-made archives with random mutations.
    #[test]
    fn synthetic_zips_mutated_never_panic(seed in any::<u64>(), mutations in 0usize..=6) {
        let mut rng = Rng::new(seed);
        let mut data = synth_zip(&mut rng);
        let ops = mutate(&mut data, &mut rng, mutations);
        check_zip(&data, || format!("synthetic zip (seed {seed:#x}) + [{ops}]"));
    }

    /// Hand-built archives using every compression method number, with random contents.
    #[test]
    fn raw_zips_with_any_method_never_panic(seed in any::<u64>(), mutations in 0usize..=3) {
        let mut rng = Rng::new(seed);
        let mut data = synth_raw_zip(&mut rng, 1 << 16);
        let ops = mutate(&mut data, &mut rng, mutations);
        check_zip(&data, || format!("raw zip (seed {seed:#x}) + [{ops}]"));
    }

    /// The 8.3 rule on any text: never panics, gives at most 8 + 1 + 3 characters with no
    /// ASCII capitals, and is idempotent except for names ending in `.` (whose truncated base
    /// may itself end in, or contain, a dot: `dos83("abcdefg.xyz.")` is `"abcdefg."`, and
    /// `dos83("abcdefg.")` is `"abcdefg"`).
    #[test]
    fn dos83_never_panics(name in prop_oneof!["\\PC{0,24}", "[a-zA-Z0-9. _-]{0,16}", any::<String>()]) {
        let r = dos83(&name);
        prop_assert!(r.chars().count() <= 12, "{r:?}");
        prop_assert!(!r.chars().any(|c| c.is_ascii_uppercase()), "{r:?}");
        if !name.ends_with('.') {
            prop_assert_eq!(dos83(&r), r.clone());
        }
        if let Some(e) = manifest::entry(&name) {
            prop_assert_eq!(e.name, r);
        }
    }
}

/// A small writer-made archive and a hand-built one truncated at every length.
#[test]
fn synthetic_zips_truncated_never_panic() {
    for seed in 0..4 {
        let mut rng = Rng::new(seed);
        for (kind, data) in [
            ("synthetic", synth_zip(&mut rng)),
            ("raw", synth_raw_zip(&mut rng, 1 << 16)),
        ] {
            for n in 0..data.len() {
                check_zip(&data[..n], || {
                    format!("{kind} zip (seed {seed}) truncated to {n}")
                });
            }
        }
    }
}

/// Executables that are missing or are not the known versions.
#[test]
fn exe_resources_of_unknown_executables() {
    let dir = TempDir::new("exe-random");
    check_exe_res(&dir.0, &EXES.map(|e| (e, false)), || {
        "an empty folder".to_owned()
    });
    for seed in 0..4u64 {
        let mut rng = Rng::new(seed);
        for exe in EXES {
            let (short, long) = (rng.below(64), rng.below(300_000));
            let n = rng.pick(&[0, 2, 28, short, long]);
            let mut bytes = rng.bytes(n);
            if bytes.len() >= 2 {
                bytes[..2].copy_from_slice(b"MZ");
            }
            fs::write(dir.0.join(exe), bytes).expect("write temporary file");
        }
        check_exe_res(&dir.0, &EXES.map(|e| (e, false)), || {
            format!("random executables (seed {seed})")
        });
    }
}

/// `validate` and `DataDir::open` on folders and archives of random files.
#[test]
fn validate_random_folders_and_archives() {
    let dir = TempDir::new("validate-random");
    let entries = manifest::entries();
    for seed in 0..4u64 {
        let mut rng = Rng::new(seed);
        for _ in 0..8 {
            let e = &entries[rng.below(entries.len())];
            let size = e.size as usize;
            let n = rng.pick(&[0, size, size.saturating_sub(1), size + 1]);
            fs::write(dir.0.join(e.name.to_uppercase()), rng.bytes(n)).expect("write file");
        }
        check_validate(&dir.0, || format!("random manifest files (seed {seed})"));
        let mut zip = synth_zip(&mut rng);
        let n = rng.below(3);
        let ops = mutate(&mut zip, &mut rng, n);
        let path = dir.0.join("archive.zip");
        fs::write(&path, &zip).expect("write archive");
        guarded(
            "DataDir::open(archive)",
            || format!("synthetic zip (seed {seed}) + [{ops}]"),
            || {
                if let Ok(data) = DataDir::open(&path) {
                    let report = validate(&data);
                    assert_eq!(report.checks.len(), entries.len());
                    for name in data.names() {
                        let _ = data.read(name);
                    }
                }
            },
        );
        fs::remove_file(&path).expect("remove archive");
    }
}

// ---------------------------------------------------------------------------------------------
// Default tier: original files (data-gated)
// ---------------------------------------------------------------------------------------------

/// An archive of original files, truncated and mutated.
#[test]
fn original_zip_truncated_and_mutated_never_panic() {
    let dir = require_data!();
    let zip = original_zip(&dir);
    check_zip(&zip, || "original files".to_owned());
    let len = zip.len();
    let stride = (len / 128).max(1);
    let lengths = (0..64)
        .chain(len - 256..len)
        .chain((0..len).step_by(stride));
    for n in lengths {
        check_zip(&zip[..n], || format!("original-file zip truncated to {n}"));
    }
    for seed in 0..256 {
        let mut rng = Rng::new(seed);
        let mut data = zip.clone();
        let n = rng.range(1, 6);
        let ops = mutate(&mut data, &mut rng, n);
        check_zip(&data, || {
            format!("original-file zip (seed {seed}) + [{ops}]")
        });
    }
}

/// Copies of the executables, unmodified, truncated and mutated.
#[test]
fn exe_resources_of_changed_executables() {
    let data = require_data!();
    let originals: Vec<Vec<u8>> = EXES
        .iter()
        .map(|exe| fs::read(data.join(exe)).unwrap_or_else(|e| panic!("{exe}: {e}")))
        .collect();
    let dir = TempDir::new("exe-mutated");
    let write_all = || {
        for (exe, bytes) in EXES.iter().zip(&originals) {
            fs::write(dir.0.join(exe), bytes).expect("write temporary file");
        }
    };
    write_all();
    check_exe_res(&dir.0, &EXES.map(|e| (e, true)), || {
        "the original executables".to_owned()
    });
    for (i, exe) in EXES.iter().enumerate() {
        let len = originals[i].len();
        let mut rng = Rng::new(i as u64);
        for round in 0..6 {
            let mut bytes = originals[i].clone();
            let what = match round {
                0 => {
                    bytes.truncate(len - 1);
                    "truncated by one byte".to_owned()
                }
                1 => {
                    bytes.truncate(28);
                    "truncated to its header".to_owned()
                }
                _ => {
                    let n = rng.range(1, 4);
                    mutate(&mut bytes, &mut rng, n)
                }
            };
            fs::write(dir.0.join(exe), &bytes).expect("write temporary file");
            // A mutation can leave the file as it was (a byte set to its own value, a run
            // copied onto an identical run); then the map still applies.
            let unchanged = bytes == originals[i];
            let expect: Vec<(&str, bool)> =
                EXES.iter().map(|&e| (e, e != *exe || unchanged)).collect();
            check_exe_res(&dir.0, &expect, || format!("{exe} {what}"));
            fs::write(dir.0.join(exe), &originals[i]).expect("restore temporary file");
        }
    }
}

/// `validate` on a folder of original files, some of them truncated or mutated.
#[test]
fn validate_changed_original_files() {
    let data = require_data!();
    let dir = TempDir::new("validate-mutated");
    for seed in 0..4u64 {
        let mut rng = Rng::new(seed);
        for name in ZIP_FILES {
            let mut bytes = fs::read(data.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
            if rng.one_in(2) {
                let n = rng.range(1, 3);
                mutate(&mut bytes, &mut rng, n);
            }
            fs::write(dir.0.join(name), &bytes).expect("write temporary file");
        }
        check_validate(&dir.0, || format!("changed original files (seed {seed})"));
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

const LONG_PATHS: [&str; 7] = [
    "zip random",
    "zip end record",
    "zip synthetic",
    "zip raw methods",
    "zip originals",
    "dos83",
    "exe_res+validate",
];

/// One round of the long test; `slot` reports the path being run to the watchdog.
fn long_round(
    base: u64,
    round: u64,
    original: Option<&[u8]>,
    exes: &[Vec<u8>],
    tmp: &Path,
    counts: &mut [u64; 7],
    slot: &Mutex<Option<(Instant, usize, u64)>>,
) {
    let seed = mix(base, round);
    let mut rng = Rng::new(seed);
    let at = |path: usize| {
        *slot.lock().expect("watchdog lock") = Some((Instant::now(), path, round));
    };
    let origin = |what: String| format!("{what} (TD2192_FUZZ_SEED={base}, round {round})");

    at(0);
    let len = rng.pick(&[64, 1024, 8192]);
    let n = rng.below(len);
    let data = rng.bytes(n);
    check_zip(&data, || origin("random bytes".to_owned()));

    at(1);
    let data = random_with_eocd(&mut rng);
    check_zip(&data, || origin("random bytes + end record".to_owned()));

    at(2);
    let mut data = synth_zip(&mut rng);
    let n = rng.below(7);
    let ops = mutate(&mut data, &mut rng, n);
    check_zip(&data, || origin(format!("synthetic zip + [{ops}]")));

    at(3);
    let mut data = synth_raw_zip(&mut rng, MAX_ENTRY as usize + 1);
    let n = rng.below(4);
    let ops = mutate(&mut data, &mut rng, n);
    check_zip(&data, || origin(format!("raw zip + [{ops}]")));
    counts[..4].iter_mut().for_each(|c| *c += 1);

    if let Some(zip) = original {
        at(4);
        let mut data = zip.to_vec();
        let what = if rng.one_in(4) {
            let n = rng.below(data.len());
            data.truncate(n);
            format!("original-file zip truncated to {n}")
        } else {
            let n = rng.range(1, 8);
            format!("original-file zip + [{}]", mutate(&mut data, &mut rng, n))
        };
        check_zip(&data, || origin(what.clone()));
        counts[4] += 1;
    }

    at(5);
    for _ in 0..16 {
        let n = rng.below(32);
        let chars: String = (0..n)
            .map(|_| {
                let c = rng.pick(&['.', 'a', 'Z', '\u{e9}', '\u{130}', ' ', '/']);
                if rng.one_in(3) {
                    char::from_u32(rng.below(0x11_0000) as u32).unwrap_or(c)
                } else {
                    c
                }
            })
            .collect();
        let r = dos83(&chars);
        assert!(r.chars().count() <= 12, "{chars:?} -> {r:?}");
        if !chars.ends_with('.') {
            assert_eq!(dos83(&r), r, "{chars:?}");
        }
        let _ = manifest::entry(&chars);
        counts[5] += 1;
    }

    // Executable resources and validation write files, so they run every 64th round.
    if round.is_multiple_of(64) {
        at(6);
        let i = rng.below(EXES.len());
        let mut bytes = match exes.get(i) {
            Some(b) => b.clone(),
            None => {
                let n = rng.below(4096);
                rng.bytes(n)
            }
        };
        let n = rng.range(1, 6);
        let ops = mutate(&mut bytes, &mut rng, n);
        fs::write(tmp.join(EXES[i]), &bytes).expect("write temporary file");
        // A mutation can leave an original executable as it was; then its map still applies.
        let unchanged = exes.get(i) == Some(&bytes);
        let expect: Vec<(&str, bool)> = EXES
            .iter()
            .map(|&e| (e, !exes.is_empty() && (e != EXES[i] || unchanged)))
            .collect();
        check_exe_res(tmp, &expect, || origin(format!("{} + [{ops}]", EXES[i])));
        check_validate(tmp, || origin(format!("{} + [{ops}]", EXES[i])));
        let restore = exes.get(i).map_or(&[][..], Vec::as_slice);
        fs::write(tmp.join(EXES[i]), restore).expect("restore temporary file");
        counts[6] += 1;
    }
}

/// Runs every input path with fresh seeds for `TD2192_FUZZ_SECS` seconds.
#[test]
#[ignore = "long fuzz; run with --release -- --ignored"]
fn long_fuzz() {
    let secs = env_u64("TD2192_FUZZ_SECS").unwrap_or(60);
    let base = env_u64("TD2192_FUZZ_SEED").unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(1, |d| d.as_nanos() as u64)
    });
    println!("td-assets long fuzz: {secs} s, TD2192_FUZZ_SEED={base}");
    let data = data_dir();
    if data.is_none() {
        println!("TD2192_DATA not set: random and synthetic inputs only");
    }
    let original = data.as_deref().map(original_zip);
    let exes: Vec<Vec<u8>> = data.as_deref().map_or_else(Vec::new, |d| {
        EXES.iter()
            .map(|exe| fs::read(d.join(exe)).unwrap_or_else(|e| panic!("{exe}: {e}")))
            .collect()
    });
    let tmp = TempDir::new("long");
    for (i, exe) in EXES.iter().enumerate() {
        let bytes = exes.get(i).map_or(&[][..], Vec::as_slice);
        fs::write(tmp.0.join(exe), bytes).expect("write temporary file");
    }
    let tmp_path = tmp.0.clone();
    let slot: Arc<Mutex<Option<(Instant, usize, u64)>>> = Arc::default();
    let deadline = Instant::now() + Duration::from_secs(secs);
    let worker = {
        let slot = Arc::clone(&slot);
        std::thread::spawn(move || {
            let mut counts = [0u64; 7];
            let mut round = 0;
            while Instant::now() < deadline {
                long_round(
                    base,
                    round,
                    original.as_deref(),
                    &exes,
                    &tmp_path,
                    &mut counts,
                    &slot,
                );
                round += 1;
            }
            (round, counts)
        })
    };
    while !worker.is_finished() {
        std::thread::sleep(Duration::from_millis(100));
        if let Some((started, path, round)) = *slot.lock().expect("watchdog lock")
            && started.elapsed() > HANG_LIMIT
            && !worker.is_finished()
        {
            panic!(
                "{} stuck for over {HANG_LIMIT:?} (TD2192_FUZZ_SEED={base}, round {round})",
                LONG_PATHS[path]
            );
        }
    }
    let (rounds, counts) = match worker.join() {
        Ok(v) => v,
        Err(payload) => std::panic::resume_unwind(payload),
    };
    let mut report = format!("{rounds} rounds; inputs per path:");
    for (name, n) in LONG_PATHS.iter().zip(counts) {
        let _ = write!(report, "\n  {name:<17} {n}");
    }
    println!("{report}");
    drop(tmp);
}
