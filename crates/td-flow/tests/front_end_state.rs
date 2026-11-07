//! The front-end state a campaign hands to its flights (weapons rev 5 §3.3, §3.3.2, §3.3.3;
//! OB-017 preserved): the 272 bytes that volleys on two-digit headings read as muzzle offsets,
//! built from what the port's screens left, against the original's DGROUP at the flight:
//!
//! - C-0002: `td1.exe`, script 1, new game, mission 1's title and hangar cards, dialogue 1 with
//!   Space, Space and Esc (at its third page break), the hint screen, flight 1 (DGROUP 4FEBh,
//!   all memory at frame 3543);
//! - C-0160: `FILE0001.EXE` started directly at mission 3, its title and hangar cards and
//!   dialogue 11 played to the end with Space, flight 3 (DGROUP 4467h, per-tick dumps);
//! - C-0015: `TD2.EXE /sb`, script 2, New game, Esc at the `marauder` card, E2 flight 1
//!   (DGROUP 3EDAh, per-frame dumps);
//! - C-0036: `TD3.EXE /sb`, script 2, New game, Esc at the `shuttle0` card, E3 flight 1
//!   (DGROUP 3AE9h, per-frame dumps);
//! - C-0180: `TD3.EXE /sb`, script 2, restore of mission 9, next mission, block 9's story played
//!   without Esc (title card, `kolorvci` card, dialogue 53, `mining2` card, dialogue 54), E3
//!   flight 9 (DGROUP 3AE9h, per-tick dumps).
//!
//! The E2/E3 runs drive the whole program (`Episode2`, `Episode3`): script choice, menu,
//! restore screen and campaign. Data-gated (TD2192_DATA and TD2192_ORACLE). No captured bytes
//! are stored.

use std::cell::RefCell;
use std::fs;
use std::path::Path;
use std::rc::Rc;

use td_assets::{DataDir, Episode, ExeResources};
use td_flow::campaign::Campaign;
use td_flow::program::Program;
use td_flow::settings::Settings;
use td_flow::{
    Episode2, Episode3, FlightLauncher, FlightScreen, FlowContext, PlaceholderLauncher, Screen,
};
use td_input::{Key, TickInput};
use td_sim::flight::front_end::{
    DirectStart, FrontEnd, MenuBytes, TWO_DIGIT_HEADINGS, direct_start, muzzle_base,
};
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
fn assert_matches_dgroup(fe: &FrontEnd, ds: &[u8], menu: &MenuBytes, what: &str) {
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
        assert_eq!(
            fe.ds_byte(Episode::E1, addr, menu),
            orig,
            "{what}: DS:{addr:04X}"
        );
    }
    for h in TWO_DIGIT_HEADINGS {
        let first = 0x035C + 34 * usize::from(h) + 2;
        assert_eq!(
            fe.heading_bytes(Episode::E1, h, menu)[..],
            ds[first..first + 34],
            "{what}: heading {h:02X}h (DS:{first:04X})"
        );
    }
}

/// `tdmen.pal` and `td.men` of the installation.
fn menu_bytes(data: &Path) -> MenuBytes {
    let dir = DataDir::open(data).unwrap();
    MenuBytes::load(|f| dir.read(f)).unwrap()
}

/// The E2/E3 state against the original's DGROUP `ds`: the scene palette (DS:0EDE, 768 bytes),
/// `g_SceneFile`'s name field (DS:0E8E, 80 bytes) and the drain key (DS:0660) in full, then the
/// bytes the eight two-digit headings read (weapons §3.3.2 "Construction").
fn assert_matches_later(ep: Episode, fe: &FrontEnd, ds: &[u8], menu: &MenuBytes, what: &str) {
    for (i, (&ours, &orig)) in fe.scene_palette.iter().zip(&ds[0x0EDE..0x11DE]).enumerate() {
        assert_eq!(
            ours,
            orig,
            "{what}: scene palette byte {i} (entry {})",
            i / 3
        );
    }
    assert_eq!(
        fe.scene_file[..],
        ds[0x0E8E..0x0EDE],
        "{what}: g_SceneFile name ({:?})",
        String::from_utf8_lossy(fe.scene_file_name())
    );
    assert_eq!(fe.drain_key, ds[0x0660], "{what}: g_MenuDrainKey");
    for h in TWO_DIGIT_HEADINGS {
        let first = usize::from(muzzle_base(ep)) + 34 * usize::from(h) + 2;
        assert_eq!(
            fe.heading_bytes(ep, h, menu)[..],
            ds[first..first + 34],
            "{what}: heading {h:02X}h (DS:{first:04X})"
        );
    }
}

/// Runs a later episode's program with `keys(frame)` until the launcher receives the
/// front-end state of the first flight; returns it.
fn later_front_end_at_flight(
    data: &Path,
    ep: Episode,
    saves: Option<&Path>,
    keys: impl Fn(u32) -> Vec<Key>,
) -> FrontEnd {
    let dir = DataDir::open(data).unwrap();
    let res = ExeResources::load(&dir).unwrap();
    let rec = Recorder::default();
    let state = rec.state.clone();
    let mut program: Box<dyn Screen> = match ep {
        Episode::E2 => {
            let p = Episode2::new(&dir, &res, Box::new(rec)).unwrap();
            Box::new(match saves {
                Some(s) => p.with_save_dir(s.to_path_buf()),
                None => p,
            })
        }
        _ => {
            let p = Episode3::new(&dir, &res, Box::new(rec)).unwrap();
            Box::new(match saves {
                Some(s) => p.with_save_dir(s.to_path_buf()),
                None => p,
            })
        }
    };
    let settings = Settings::default();
    let mut audio = Vec::new();
    for frame in 0..400_000u32 {
        let mut ctx = FlowContext {
            data: &dir,
            resources: &res,
            settings: &settings,
            audio: &mut audio,
        };
        let input = TickInput {
            pressed: keys(frame),
            ..TickInput::default()
        };
        let _ = program.tick(&mut ctx, &input);
        audio.clear();
        if let Some(s) = state.borrow().clone() {
            return s;
        }
    }
    panic!("{ep:?}: the flight never started");
}

/// The keys of a New game: script 2, Enter at the menu, then `key` every 20 frames.
fn new_game(key: Key) -> impl Fn(u32) -> Vec<Key> {
    move |f| match f {
        10 => vec![Key::digit(2).unwrap()],
        100 => vec![Key::ENTER],
        f if f >= 120 && f.is_multiple_of(20) => vec![key],
        _ => Vec::new(),
    }
}

#[test]
fn e2_new_game_matches_c0015() {
    let data = require_data!();
    let oracle = require_oracle!();
    let fe = later_front_end_at_flight(&data, Episode::E2, None, new_game(Key::ESCAPE));
    assert_eq!(fe.scene_file_name(), b"td.lt2");
    // The last per-frame dump, inside the flight (tick 1 at frame 664).
    let ds = fs::read(
        oracle
            .join("C-0015")
            .join("mem")
            .join("f00000880_3EDA-0000_9800_dg.bin"),
    )
    .unwrap();
    assert_matches_later(Episode::E2, &fe, &ds, &menu_bytes(&data), "C-0015");
}

#[test]
fn e3_new_game_matches_c0036() {
    let data = require_data!();
    let oracle = require_oracle!();
    let fe = later_front_end_at_flight(&data, Episode::E3, None, new_game(Key::ESCAPE));
    assert_eq!(fe.scene_file_name(), b"td.lt2");
    // The last per-frame dump, inside the flight (tick 1 at frame 694).
    let ds = fs::read(
        oracle
            .join("C-0036")
            .join("mem")
            .join("f00000900_3AE9-0000_9800_dg.bin"),
    )
    .unwrap();
    assert_matches_later(Episode::E3, &fe, &ds, &menu_bytes(&data), "C-0036");
}

#[test]
fn e3_flight_9_after_its_story_matches_c0180() {
    let data = require_data!();
    let oracle = require_oracle!();
    // The shipped td3.sav with slot 1 := 09 00 00, in a scratch save directory.
    let saves = std::env::temp_dir().join(format!("td2192-c0180-{}", std::process::id()));
    fs::create_dir_all(&saves).unwrap();
    let mut sav = fs::read(data.join("td3.sav")).unwrap();
    sav[..3].copy_from_slice(&[9, 0, 0]);
    fs::write(saves.join("td3.sav"), sav).unwrap();
    fs::copy(data.join("td3.stt"), saves.join("td3.stt")).unwrap();
    // Script 2; Down, Enter (restore); Enter (slot 1); Enter (next mission); then Space every
    // 40 frames through the story, never Esc.
    let keys = |f: u32| match f {
        10 => vec![Key::digit(2).unwrap()],
        100 => vec![Key::DOWN],
        101 => vec![Key::ENTER],
        160 => vec![Key::ENTER],
        240 => vec![Key::ENTER],
        f if f >= 280 && f.is_multiple_of(40) => vec![Key::SPACE],
        _ => Vec::new(),
    };
    let fe = later_front_end_at_flight(&data, Episode::E3, Some(&saves), keys);
    fs::remove_dir_all(&saves).unwrap();
    assert_eq!(fe.scene_file_name(), b"td.pal", "dialogue 54 last");
    let b = fs::read(
        oracle
            .join("C-0180")
            .join("mem")
            .join("tick_3AE9-0000_9800_ds.bin"),
    )
    .unwrap();
    let menu = menu_bytes(&data);
    // The bytes are constant over the dumped ticks 1..3 (weapons §3.3.3 test vector (b)).
    let mut ticks = 0;
    for rec in b.chunks(16 + 0x9800) {
        assert_matches_later(Episode::E3, &fe, &rec[16..], &menu, "C-0180");
        ticks += 1;
    }
    assert_eq!(ticks, 3);
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
    let menu = menu_bytes(&data);
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
    let menu = menu_bytes(&data);
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
    let menu = menu_bytes(&data);
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

/// `td_sim::flight::front_end::direct_start` repeats, for the tools, the first screen of each
/// E2/E3 block and its card's picture and ink; they must be the campaigns' and the card
/// tables' (no data needed).
#[test]
fn direct_start_table_agrees_with_the_blocks() {
    use td_flow::campaign_e2::{E2_BLOCKS, Step as S2};
    use td_flow::campaign_e3::{Step as S3, block_of};
    use td_flow::scene_card::Layout;
    let card = |l: Layout| DirectStart::Card {
        file: l.file,
        ink: l.ink.1.map(|c| (l.ink.0, [c.r, c.g, c.b])),
    };
    for (i, block) in E2_BLOCKS.iter().enumerate() {
        let n = i as u8 + 1;
        // E2 cards do not flush: the Esc at the first scene card skips from there.
        let first = block.pre.iter().find_map(|s| match s {
            S2::S(bg, _) => Some(bg.layout()),
            _ => None,
        });
        assert_eq!(
            direct_start(Episode::E2, n),
            card(first.expect("a scene card")),
            "E2 block {n}"
        );
    }
    for n in (1..=19).filter(|&n| n != 7) {
        // E3 screens flush: the Esc at the block's first screen skips from there.
        let steps = block_of(n).expect("a block").steps;
        let first = steps
            .iter()
            .find(|s| matches!(s, S3::S(..) | S3::T(_) | S3::Tc(_)))
            .expect("a screen");
        let want = match first {
            S3::S(bg, _) => card(bg.layout()),
            _ => DirectStart::RestoreThenTitle,
        };
        assert_eq!(direct_start(Episode::E3, n), want, "E3 block {n}");
    }
}

/// A scratch save directory holding the shipped `tdN.sav` with slot 1 := `mission` 00 00 and
/// `tdN.stt`.
fn saves_with(data: &Path, ep: Episode, mission: u8) -> std::path::PathBuf {
    let n = if ep == Episode::E2 { 2 } else { 3 };
    let dir = std::env::temp_dir().join(format!("td2192-fe-e{n}m{mission}-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let mut sav = fs::read(data.join(format!("td{n}.sav"))).unwrap();
    sav[..3].copy_from_slice(&[mission, 0, 0]);
    fs::write(dir.join(format!("td{n}.sav")), sav).unwrap();
    fs::copy(
        data.join(format!("td{n}.stt")),
        dir.join(format!("td{n}.stt")),
    )
    .unwrap();
    dir
}

/// The tick-1 DGROUP of a whole-flight capture (`tick_<DGROUP>-0000_9800_ds.bin`).
fn tick_1(oracle: &Path, capture: &str, dgroup: &str) -> Vec<u8> {
    let b = fs::read(
        oracle
            .join(capture)
            .join("mem")
            .join(format!("tick_{dgroup}-0000_9800_ds.bin")),
    )
    .unwrap();
    b.chunks(16 + 0x9800)
        .find(|r| u32::from_le_bytes(r[..4].try_into().unwrap()) == 1)
        .expect("a record of tick 1")[16..]
        .to_vec()
}

/// VE-4's restore path (M10-playthrough "Direct starts"): script 2; Down, Enter (restore);
/// Enter (slot 1); Enter (next mission); then `after(frames since 300)`.
fn restore_then(after: impl Fn(u32) -> Vec<Key>) -> impl Fn(u32) -> Vec<Key> {
    move |f| match f {
        10 => vec![Key::digit(2).unwrap()],
        100 => vec![Key::DOWN],
        101 => vec![Key::ENTER],
        160 => vec![Key::ENTER],
        260 => vec![Key::ENTER],
        f if f >= 300 => after(f - 300),
        _ => Vec::new(),
    }
}

#[test]
fn e2_restored_flight_2_matches_c0121() {
    // Block 2: the title card's 5 s, then Esc at the velsroom card (E2 cards do not flush).
    let data = require_data!();
    let oracle = require_oracle!();
    let saves = saves_with(&data, Episode::E2, 2);
    let esc = |f: u32| {
        if f >= 600 && f.is_multiple_of(100) {
            vec![Key::ESCAPE]
        } else {
            Vec::new()
        }
    };
    let fe = later_front_end_at_flight(&data, Episode::E2, Some(&saves), restore_then(esc));
    fs::remove_dir_all(&saves).unwrap();
    assert_eq!(fe.scene_file_name(), b"td.lt2");
    let ds = tick_1(&oracle, "C-0121", "3EDA");
    assert_matches_later(Episode::E2, &fe, &ds, &menu_bytes(&data), "C-0121");
}

#[test]
fn e3_restored_flight_4_matches_c0128() {
    // Block 4: Esc at the title card, so g_Palette is the menu's palette buffer that the
    // restore screen copied, with the title card's entry 85 (weapons §3.3.3 test vector (a)).
    let data = require_data!();
    let oracle = require_oracle!();
    let saves = saves_with(&data, Episode::E3, 4);
    let esc = |f: u32| {
        if f >= 40 && f.is_multiple_of(20) {
            vec![Key::ESCAPE]
        } else {
            Vec::new()
        }
    };
    let fe = later_front_end_at_flight(&data, Episode::E3, Some(&saves), restore_then(esc));
    fs::remove_dir_all(&saves).unwrap();
    assert_eq!(fe.scene_file_name(), b"td.sss");
    let ds = tick_1(&oracle, "C-0128", "3AE9");
    assert_matches_later(Episode::E3, &fe, &ds, &menu_bytes(&data), "C-0128");
}
