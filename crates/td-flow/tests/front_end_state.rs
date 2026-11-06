//! The front-end state a campaign hands to its flights (weapons rev 4 §3.3, OB-017 preserved):
//! the 272 bytes that volleys on two-digit headings read as muzzle offsets, built from what the
//! port's screens left, against the original's DGROUP at the flight:
//!
//! - C-0002: `td1.exe`, script 1, new game, mission 1's title and hangar cards, dialogue 1 with
//!   Space, Space and Esc (at its third page break), the hint screen, flight 1 (DGROUP 4FEBh,
//!   all memory at frame 3543);
//! - C-0160: `FILE0001.EXE` started directly at mission 3, its title and hangar cards and
//!   dialogue 11 played to the end with Space, flight 3 (DGROUP 4467h, per-tick dumps).
//!
//! Data-gated (TD2192_DATA and TD2192_ORACLE). No captured bytes are stored.

use std::cell::RefCell;
use std::fs;
use std::path::Path;
use std::rc::Rc;

use td_assets::{DataDir, ExeResources};
use td_flow::campaign::Campaign;
use td_flow::program::Program;
use td_flow::settings::Settings;
use td_flow::{FlightLauncher, FlightScreen, FlowContext, PlaceholderLauncher};
use td_input::{Key, TickInput};
use td_sim::flight::front_end::{FrontEnd, TWO_DIGIT_HEADINGS};
use td_test_support::{require_data, require_oracle};

const DS_LEN: usize = 0x9DF0;

/// A launcher that keeps the front-end state handed over before the flight and flies
/// placeholder flights.
#[derive(Debug, Default)]
struct Recorder {
    state: Rc<RefCell<Option<FrontEnd>>>,
}

impl FlightLauncher for Recorder {
    fn set_front_end(&mut self, state: &FrontEnd) {
        *self.state.borrow_mut() = Some(state.clone());
    }

    fn launch(&mut self, ctx: &mut FlowContext<'_>, flight: u8) -> Box<dyn FlightScreen> {
        PlaceholderLauncher::default().launch(ctx, flight)
    }

    fn random(&mut self, _n: u16) -> u16 {
        0
    }
}

/// Plays mission `mission`'s block with `keys(frame since the dialogue started)` as the input
/// while the dialogue shows, `after(frame)` once it has ended, nothing before it, until the
/// flight is launched; returns the front-end state handed to the launcher.
fn front_end_at_flight(
    data: &Path,
    start: FrontEnd,
    mission: u8,
    keys: impl Fn(u32) -> Vec<Key>,
    after: impl Fn(u32) -> Vec<Key>,
) -> FrontEnd {
    let dir = DataDir::open(data).unwrap();
    let res = ExeResources::load(&dir).unwrap();
    let rec = Recorder::default();
    let state = rec.state.clone();
    let mut prog = Program::new(&dir, &res, Box::new(rec)).unwrap();
    prog.front_end = start;
    let mut c = Campaign::new(mission, 0);
    let settings = Settings::default();
    let mut audio = Vec::new();
    let (mut in_dlg, mut dlg_done) = (0u32, None::<u32>);
    for frame in 0..200_000u32 {
        let pressed = if c.in_dialogue() {
            in_dlg += 1;
            keys(in_dlg - 1)
        } else if in_dlg > 0 {
            let since = *dlg_done.get_or_insert(frame);
            after(frame - since)
        } else {
            Vec::new()
        };
        let mut ctx = FlowContext {
            data: &dir,
            resources: &res,
            settings: &settings,
            audio: &mut audio,
        };
        let input = TickInput {
            pressed,
            ..TickInput::default()
        };
        let _ = c.step(&mut prog, &mut ctx, &input);
        audio.clear();
        if let Some(s) = state.borrow().clone() {
            return s;
        }
    }
    panic!("mission {mission}: the flight never started");
}

/// The bytes the eight two-digit headings read, ours against the original's DGROUP `ds`, and
/// the two blocks they come from compared in full: the scene palette (DS:0D2E, 768 bytes) and
/// the lit flag, title font and dialogue reader (DS:102E..113D).
fn assert_matches_dgroup(fe: &FrontEnd, ds: &[u8], menu: &[u8], what: &str) {
    for (i, (&ours, &orig)) in fe.scene_palette.iter().zip(&ds[0x0D2E..0x102E]).enumerate() {
        assert_eq!(
            ours,
            orig,
            "{what}: scene palette byte {i} (entry {})",
            i / 3
        );
    }
    for addr in 0x102E..0x113E_u16 {
        let orig = ds[usize::from(addr)];
        assert_eq!(fe.ds_byte(addr, menu), orig, "{what}: DS:{addr:04X}");
    }
    for h in TWO_DIGIT_HEADINGS {
        let first = 0x035C + 34 * usize::from(h) + 2;
        assert_eq!(
            fe.heading_bytes(h, menu)[..],
            ds[first..first + 34],
            "{what}: heading {h:02X}h (DS:{first:04X})"
        );
    }
}

#[test]
fn campaign_start_through_the_launcher_matches_c0002() {
    let data = require_data!();
    let oracle = require_oracle!();
    // Dialogue 1: Space, Space, Esc, taken by its first three page breaks; the hint screen
    // flushes the buffer after its fade-in, then any key.
    let dialogue = |f: u32| match f {
        0 => vec![Key::SPACE, Key::SPACE, Key::ESCAPE],
        _ => Vec::new(),
    };
    let hint = |f: u32| {
        if f > 0 && f.is_multiple_of(40) {
            vec![Key::SPACE]
        } else {
            Vec::new()
        }
    };
    let fe = front_end_at_flight(&data, FrontEnd::launcher(), 1, dialogue, hint);
    assert_eq!(fe.reader.code, 0x05, "Esc at a page break");
    let mem = fs::read(
        oracle
            .join("C-0002")
            .join("mem")
            .join("f00003543_0000-0000_A0000_flight-1s.bin"),
    )
    .unwrap();
    let ds = &mem[0x4FEB0..0x4FEB0 + DS_LEN];
    let menu = DataDir::open(&data).unwrap().read("tdmen.pal").unwrap();
    assert_matches_dgroup(&fe, ds, &menu, "C-0002");
}

#[test]
fn mission_3_after_its_story_matches_c0160() {
    let data = require_data!();
    let oracle = require_oracle!();
    // Dialogue 11: Space every 40 frames to its end.
    let dialogue = |f: u32| {
        if f.is_multiple_of(40) {
            vec![Key::SPACE]
        } else {
            Vec::new()
        }
    };
    let fe = front_end_at_flight(&data, FrontEnd::direct_start(), 3, dialogue, |_| Vec::new());
    assert_eq!(fe.reader.code, b'd', "the scene's end");
    let b = fs::read(
        oracle
            .join("C-0160")
            .join("mem")
            .join("tick_4467-0000_9DF0_ds.bin"),
    )
    .unwrap();
    let menu = DataDir::open(&data).unwrap().read("tdmen.pal").unwrap();
    // The bytes are constant over the 80 dumped ticks (weapons §3.3 test vector 7).
    let mut ticks = 0;
    for rec in b.chunks(16 + DS_LEN) {
        assert_matches_dgroup(&fe, &rec[16..], &menu, "C-0160");
        ticks += 1;
    }
    assert!(ticks >= 80, "{ticks} records");
}

/// VE-5's whole flight 3 after its story (scenario `# ve5-e1f03-story`, C-0160's keys up to the
/// flight, then `e1f03.tdr`): the port's front-end bytes against every per-tick record of the
/// flight, which `td-sim/tests/oracle_e1_full.rs` flies with the capture's bytes.
#[test]
fn mission_3_whole_flight_after_its_story_matches_ve5_capture() {
    let data = require_data!();
    let oracle = require_oracle!();
    let Some(cap) = fs::read_dir(&oracle).ok().and_then(|d| {
        let mut v: Vec<_> = d.filter_map(|e| e.ok().map(|e| e.path())).collect();
        v.sort();
        v.into_iter().rev().find(|p| {
            fs::read_to_string(p.join("scenario.txt"))
                .is_ok_and(|s| s.starts_with("# ve5-e1f03-story "))
        })
    }) else {
        eprintln!("SKIPPED: no ve5-e1f03-story capture");
        return;
    };
    let dialogue = |f: u32| {
        if f.is_multiple_of(40) {
            vec![Key::SPACE]
        } else {
            Vec::new()
        }
    };
    let fe = front_end_at_flight(&data, FrontEnd::direct_start(), 3, dialogue, |_| Vec::new());
    let b = fs::read(cap.join("mem").join("tick_4467-0000_9DF0_ds.bin")).unwrap();
    let menu = DataDir::open(&data).unwrap().read("tdmen.pal").unwrap();
    let mut ticks = 0;
    for rec in b.chunks(16 + DS_LEN) {
        assert_matches_dgroup(&fe, &rec[16..], &menu, "ve5-e1f03-story");
        ticks += 1;
    }
    assert!(ticks > 1600, "{ticks} records");
}

#[test]
fn a_block_hands_its_screens_state_to_the_flight() {
    // Without data the screens are unavailable; with it, mission 1 with Space everywhere ends
    // on the hint screen (secret.pic's palette, entry 240 := (63, 10, 10)) and dialogue 1 read
    // to its end.
    let data = require_data!();
    let space = |f: u32| {
        if f.is_multiple_of(40) {
            vec![Key::SPACE]
        } else {
            Vec::new()
        }
    };
    let fe = front_end_at_flight(&data, FrontEnd::launcher(), 1, space, space);
    assert_eq!(fe.reader.code, b'd');
    assert!(!fe.lit);
    assert_eq!(fe.scene_palette[720..723], [63, 10, 10], "entry 240");
    let secret = DataDir::open(&data).unwrap().read("secret.pic").unwrap();
    assert_eq!(fe.scene_palette[..720], secret[..720]);
}
