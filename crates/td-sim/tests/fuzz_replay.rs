//! M12-06: "never panics" fuzz-style tests for the replay parser
//! (`td_sim::replay::Replay::from_bytes`; MDG §11, ADG §10.6).
//!
//! Inputs: random bytes; headers with any version and any declared tick and checkpoint counts
//! (up to `u32::MAX`) over bodies of any length; a synthetic replay truncated at every length;
//! the committed replays under `tests/replays/` (our data, not game data) truncated and with
//! seeded random mutations. After a successful parse the result is serialised and parsed
//! again, checked against the header, run through `verify`, and every input word is decoded
//! into the key table.
//!
//! The parser loops over its declared counts but stops at the first word or hash that is not
//! there, so a count can only cost time in proportion to the input. BUG-17: it does reserve
//! memory in proportion to the declared counts before checking the input length (replay.rs:89,
//! :97; capped at 2^24 input words = 256 MiB and 2^20 checkpoints = 8 MiB), so a 24-byte
//! header declaring `u32::MAX` ticks reserves 256 MiB. The tests here cannot observe
//! allocations (no allocator hooks without `unsafe`); `huge_declared_counts_fail_fast` checks
//! that such headers fail quickly and without a panic.
//!
//! `long_fuzz` (ignored) loops with fresh seeds for `TD2192_FUZZ_SECS` seconds (default 60)
//! from `TD2192_FUZZ_SEED` (default: from the clock) and prints the inputs tried:
//!
//! ```text
//! TD2192_FUZZ_SECS=600 cargo test -p td-sim --release --test fuzz_replay -- --ignored --nocapture
//! ```

use std::fs;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::time::{Duration, Instant};

use proptest::collection::vec;
use proptest::prelude::*;
use td_sim::replay::{Replay, ReplayError, verify};
use td_sim::{KeyTable, TickControls};

/// A parse slower than this fails as a suspected hang.
const SLOW_INPUT: Duration = Duration::from_secs(5);

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

/// Header offsets of the replay format: version, episode, mission, seed, ticks, interval,
/// checkpoint count.
const FIELDS: [(usize, usize); 7] = [(4, 2), (6, 1), (7, 1), (8, 4), (12, 4), (16, 4), (20, 4)];

/// Applies one random mutation (header fields favoured) and describes it.
fn mutate_once(data: &mut Vec<u8>, rng: &mut Rng) -> String {
    let len = data.len();
    match rng.below(7) {
        0 if len >= 24 => {
            let (at, width) = rng.pick(&FIELDS);
            let random = rng.next_u64();
            let v = rng.pick(&[0, 1, 2, 3, 0xFF, 0xFFFF, 0xFFFF_FFFF, 1 << 24, random]);
            data[at..at + width].copy_from_slice(&v.to_le_bytes()[..width]);
            format!("field[{at}]={v:#x}")
        }
        1 if len > 0 => {
            let o = rng.below(len);
            let bit = 1u8 << rng.below(8);
            data[o] ^= bit;
            format!("flip[{o}]^={bit:#04x}")
        }
        2 if len > 0 => {
            let o = rng.below(len);
            let v = rng.next_u64() as u8;
            data[o] = v;
            format!("set[{o}]={v:#04x}")
        }
        3 if len > 0 => {
            let o = rng.below(len);
            let n = rng.range(1, 16).min(len - o);
            data.drain(o..o + n);
            format!("delete[{o}..{}]", o + n)
        }
        4 if len > 0 => {
            let n = rng.below(len);
            data.truncate(n);
            format!("truncate({n})")
        }
        5 => {
            let o = rng.below(len + 1);
            let n = rng.range(1, 16);
            let ins = rng.bytes(n);
            let what = format!("insert[{o}]={ins:02x?}");
            data.splice(o..o, ins);
            what
        }
        _ => {
            let n = rng.range(1, 32);
            data.extend(rng.bytes(n));
            format!("append({n})")
        }
    }
}

fn mutate(data: &mut Vec<u8>, rng: &mut Rng, n: usize) -> String {
    let ops: Vec<String> = (0..n).map(|_| mutate_once(data, rng)).collect();
    ops.join(", ")
}

/// A replay header with the given fields.
fn header(version: u16, ticks: u32, interval: u32, checks: u32) -> Vec<u8> {
    let mut v = b"TDR1".to_vec();
    v.extend(version.to_le_bytes());
    v.extend([1, 3]);
    v.extend(0x1234_5678u32.to_le_bytes());
    v.extend(ticks.to_le_bytes());
    v.extend(interval.to_le_bytes());
    v.extend(checks.to_le_bytes());
    v
}

/// A well-formed replay with random content.
fn synth_replay(rng: &mut Rng) -> Replay {
    let ticks = rng.below(64);
    let interval = rng.pick(&[0, 1, 2, 7, 64]);
    let checks = if interval == 0 {
        rng.below(4)
    } else {
        ticks / interval as usize + rng.below(2)
    };
    Replay {
        episode: rng.range(1, 3) as u8,
        mission: rng.range(1, 21) as u8,
        seed: rng.next_u64() as u32,
        inputs: (0..ticks)
            .map(|_| u128::from(rng.next_u64()) | u128::from(rng.next_u64()) << 64)
            .collect(),
        interval,
        checkpoints: (0..checks).map(|_| rng.next_u64()).collect(),
    }
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_owned()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "non-string panic payload".to_owned()
    }
}

/// Parses `bytes`; on success checks the result against the header, round-trips it, runs it
/// through `verify` and decodes its input words. A panic or a slow parse fails with `origin`.
fn check(bytes: &[u8], origin: impl Fn() -> String) {
    let start = Instant::now();
    let outcome = catch_unwind(AssertUnwindSafe(|| match Replay::from_bytes(bytes) {
        Ok(r) => {
            let version = u16::from_le_bytes([bytes[4], bytes[5]]);
            let word = if version == 3 { 16 } else { 8 };
            assert!(version == 2 || version == 3);
            assert_eq!(r.inputs.len(), u32_at(bytes, 12) as usize);
            assert_eq!(r.interval, u32_at(bytes, 16));
            assert_eq!(r.checkpoints.len(), u32_at(bytes, 20) as usize);
            let used = 24 + r.inputs.len() * word + r.checkpoints.len() * 8;
            assert!(used <= bytes.len());
            let again = r.to_bytes();
            if version == 3 {
                assert_eq!(again, bytes[..used]);
            }
            assert_eq!(Replay::from_bytes(&again).as_ref(), Ok(&r));
            if version == 2 {
                assert!(r.inputs.iter().all(|&w| w >> 64 == 0));
            }
            // The recorded hashes verify against themselves; a different hash is reported.
            let interval = r.interval as usize;
            let mut tick = 0usize;
            let expected = |t: usize| {
                (interval > 0 && t.is_multiple_of(interval))
                    .then(|| r.checkpoints.get(t / interval - 1).copied())
                    .flatten()
                    .unwrap_or(0)
            };
            assert_eq!(
                verify(&r, |_| {
                    tick += 1;
                    expected(tick)
                }),
                Ok(())
            );
            let mut tick = 0usize;
            let _ = verify(&r, |w| {
                tick += 1;
                expected(tick) ^ w as u64 ^ 1
            });
            let mut keys = KeyTable::default();
            for &w in &r.inputs {
                keys.update(&TickControls::from_word(w));
            }
        }
        Err(ReplayError::BadMagic) => assert!(bytes.len() >= 4 && bytes[..4] != *b"TDR1"),
        Err(ReplayError::Version(v)) => assert!(v != 2 && v != 3),
        Err(ReplayError::Truncated) => {}
    }));
    let elapsed = start.elapsed();
    if let Err(payload) = outcome {
        panic!(
            "Replay::from_bytes panicked on {} ({} bytes): {}",
            origin(),
            bytes.len(),
            panic_message(payload.as_ref())
        );
    }
    assert!(
        elapsed < SLOW_INPUT,
        "Replay::from_bytes took {elapsed:?} on {} ({} bytes): suspected hang",
        origin(),
        bytes.len()
    );
}

/// The committed replays (`tests/replays/*.tdr`).
fn committed_replays() -> Vec<(String, Vec<u8>)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/replays");
    let mut out: Vec<(String, Vec<u8>)> = fs::read_dir(&dir)
        .map(|it| {
            it.filter_map(Result::ok)
                .filter(|e| e.path().extension().is_some_and(|x| x == "tdr"))
                .map(|e| {
                    let name = e.file_name().to_string_lossy().into_owned();
                    (name, fs::read(e.path()).expect("read replay"))
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

fn proptest_config(cases: u32) -> ProptestConfig {
    ProptestConfig {
        cases,
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

proptest! {
    #![proptest_config(proptest_config(1024))]

    /// Random bytes, with the magic in front half of the time.
    #[test]
    fn random_bytes_never_panic(mut data in vec(any::<u8>(), 0..512), magic in any::<bool>()) {
        if magic && data.len() >= 4 {
            data[..4].copy_from_slice(b"TDR1");
        }
        check(&data, || "random bytes".to_owned());
    }

    /// Any header (any version, any declared counts up to `u32::MAX`) over a body of any
    /// length: the parse fails or succeeds in time proportional to the body.
    #[test]
    fn any_header_and_counts_never_panic(
        version in prop_oneof![2u16..=3, any::<u16>()],
        ticks in prop_oneof![0u32..8, any::<u32>(), Just(u32::MAX), Just(1 << 24)],
        interval in prop_oneof![0u32..4, any::<u32>()],
        checks in prop_oneof![0u32..8, any::<u32>(), Just(u32::MAX), Just(1 << 20)],
        body in vec(any::<u8>(), 0..400),
    ) {
        let mut data = header(version, ticks, interval, checks);
        data.extend(body);
        check(&data, || format!("header v{version} ticks {ticks} interval {interval} checks {checks}"));
    }

    /// Well-formed replays (versions 3 and 2) with random mutations.
    #[test]
    fn synthetic_replays_mutated_never_panic(seed in any::<u64>(), mutations in 0usize..=4, v2 in any::<bool>()) {
        let mut rng = Rng::new(seed);
        let r = synth_replay(&mut rng);
        let mut data = if v2 { to_v2(&r) } else { r.to_bytes() };
        if mutations == 0 {
            prop_assert_eq!(Replay::from_bytes(&data), Ok(r_v(&r, v2)));
        }
        let ops = mutate(&mut data, &mut rng, mutations);
        check(&data, || format!("synthetic replay (seed {seed:#x}, v2 {v2}) + [{ops}]"));
    }
}

/// The replay `r` as written in version 2 (64-bit input words).
fn to_v2(r: &Replay) -> Vec<u8> {
    let mut v = header(
        2,
        r.inputs.len() as u32,
        r.interval,
        r.checkpoints.len() as u32,
    );
    v[6] = r.episode;
    v[7] = r.mission;
    v[8..12].copy_from_slice(&r.seed.to_le_bytes());
    for &w in &r.inputs {
        v.extend((w as u64).to_le_bytes());
    }
    for c in &r.checkpoints {
        v.extend(c.to_le_bytes());
    }
    v
}

/// What parsing `r` written in version 2 (`v2`) or 3 gives back.
fn r_v(r: &Replay, v2: bool) -> Replay {
    let mut out = r.clone();
    if v2 {
        out.inputs
            .iter_mut()
            .for_each(|w| *w &= u128::from(u64::MAX));
    }
    out
}

/// A synthetic replay of each version truncated at every length: every proper prefix is
/// `Truncated`, the whole file parses, and trailing bytes are ignored.
#[test]
fn synthetic_replays_truncated_at_every_length() {
    for seed in 0..8 {
        let mut rng = Rng::new(seed);
        let r = synth_replay(&mut rng);
        for v2 in [false, true] {
            let data = if v2 { to_v2(&r) } else { r.to_bytes() };
            for n in 0..data.len() {
                check(&data[..n], || {
                    format!("synthetic replay (seed {seed}, v2 {v2}) truncated to {n}")
                });
                assert_eq!(
                    Replay::from_bytes(&data[..n]),
                    Err(ReplayError::Truncated),
                    "seed {seed} v2 {v2} length {n}"
                );
            }
            let mut longer = data.clone();
            longer.extend([0xAA; 9]);
            assert_eq!(Replay::from_bytes(&longer), Ok(r_v(&r, v2)));
        }
    }
}

/// Header-only inputs that declare the largest counts fail quickly.
#[test]
fn huge_declared_counts_fail_fast() {
    for (ticks, checks) in [
        (u32::MAX, 0),
        (0, u32::MAX),
        (u32::MAX, u32::MAX),
        (1 << 24, 1 << 20),
        ((1 << 24) + 1, (1 << 20) + 1),
    ] {
        for version in [2, 3] {
            let data = header(version, ticks, 1, checks);
            let start = Instant::now();
            for _ in 0..16 {
                assert_eq!(Replay::from_bytes(&data), Err(ReplayError::Truncated));
            }
            assert!(
                start.elapsed() < SLOW_INPUT,
                "v{version} ticks {ticks} checks {checks}: {:?}",
                start.elapsed()
            );
        }
    }
}

/// The committed replays: unmodified, truncated and mutated.
#[test]
fn committed_replays_truncated_and_mutated_never_panic() {
    let replays = committed_replays();
    assert!(!replays.is_empty(), "tests/replays/*.tdr");
    for (name, bytes) in &replays {
        check(bytes, || name.clone());
        let len = bytes.len();
        let stride = (len / 64).max(1);
        for n in (0..64.min(len))
            .chain(len.saturating_sub(64)..len)
            .chain((0..len).step_by(stride))
        {
            check(&bytes[..n], || format!("{name} truncated to {n}"));
        }
        for round in 0..16 {
            let seed = mix(len as u64, round) ^ name.len() as u64;
            let mut rng = Rng::new(seed);
            let mut data = bytes.clone();
            let n = rng.range(1, 4);
            let ops = mutate(&mut data, &mut rng, n);
            check(&data, || format!("{name} + [{ops}] (seed {seed:#x})"));
        }
    }
}

fn env_u64(name: &str) -> Option<u64> {
    let v = std::env::var(name).ok()?;
    Some(
        v.trim()
            .parse()
            .unwrap_or_else(|e| panic!("{name}={v:?}: {e}")),
    )
}

/// Runs the replay parser on fresh random, synthetic and committed inputs for
/// `TD2192_FUZZ_SECS` seconds. Each parse is timed against [`SLOW_INPUT`]; the parser has no
/// unbounded loop, so no watchdog thread is needed.
#[test]
#[ignore = "long fuzz; run with --release -- --ignored"]
fn long_fuzz() {
    let secs = env_u64("TD2192_FUZZ_SECS").unwrap_or(60);
    let base = env_u64("TD2192_FUZZ_SEED").unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(1, |d| d.as_nanos() as u64)
    });
    println!("td-sim replay long fuzz: {secs} s, TD2192_FUZZ_SEED={base}");
    let replays = committed_replays();
    let deadline = Instant::now() + Duration::from_secs(secs);
    let mut counts = [0u64; 4];
    let mut round = 0u64;
    while Instant::now() < deadline {
        let seed = mix(base, round);
        let mut rng = Rng::new(seed);
        let origin = |what: &str| format!("{what} (TD2192_FUZZ_SEED={base}, round {round})");

        let n = rng.pick(&[8, 24, 64, 1024]);
        let n = rng.below(n);
        let mut data = rng.bytes(n);
        if data.len() >= 4 && rng.below(2) == 0 {
            data[..4].copy_from_slice(b"TDR1");
        }
        check(&data, || origin("random bytes"));

        let random = rng.next_u64() as u32;
        let ticks = rng.pick(&[0, 1, 7, 1 << 24, u32::MAX, random]);
        let checks = rng.pick(&[0, 1, 7, 1 << 20, u32::MAX, random]);
        let version = rng.pick(&[2, 3, 1, 4, 0xFFFF]);
        let mut data = header(version, ticks, rng.below(8) as u32, checks);
        let n = rng.below(512);
        data.extend(rng.bytes(n));
        check(&data, || {
            origin(&format!("header v{version} ticks {ticks} checks {checks}"))
        });

        let r = synth_replay(&mut rng);
        let mut data = if rng.below(2) == 0 {
            to_v2(&r)
        } else {
            r.to_bytes()
        };
        let n = rng.below(6);
        let ops = mutate(&mut data, &mut rng, n);
        check(&data, || origin(&format!("synthetic replay + [{ops}]")));
        counts[..3].iter_mut().for_each(|c| *c += 1);

        if !replays.is_empty() {
            let (name, bytes) = &replays[rng.below(replays.len())];
            let mut data = bytes.clone();
            let n = rng.range(1, 6);
            let ops = mutate(&mut data, &mut rng, n);
            check(&data, || origin(&format!("{name} + [{ops}]")));
            counts[3] += 1;
        }
        round += 1;
    }
    println!(
        "{round} rounds; inputs: random {}, header {}, synthetic {}, committed {}; total {}",
        counts[0],
        counts[1],
        counts[2],
        counts[3],
        counts.iter().sum::<u64>()
    );
}
