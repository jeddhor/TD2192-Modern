//! L5 parity: the save and restore screens (`docs/spec/flow/save-screens.md` rev 1 §6) against
//! oracle captures C-0042 (empty slot set, a save into slot 4, the restore screen) and C-0043
//! (the shipped files, restore slot 5, a save with a 25-letter name). The captures keep the
//! files before and after each run in `files\`; the test reads them as input and expected
//! output, and stores nothing. Needs TD2192_DATA and TD2192_ORACLE.

use std::fs;
use std::path::{Path, PathBuf};

use td_assets::{DataDir, ExeResources};
use td_flow::main_menu::{MainMenu, MenuGraphics, Reentry};
use td_flow::save_screens::{SaveKind, SaveOutcome, SaveScreen, SaveSet};
use td_flow::story::StoryAssets;
use td_formats::palette::Palette;
use td_formats::saves::{SaveNames, SaveSlots};
use td_input::{Key, TickInput};
use td_test_support::{require_data, require_oracle};

struct Rig {
    dir: DataDir,
    res: ExeResources,
    assets: StoryAssets,
    /// The menu's palette buffer as the menu passes it.
    pal: Palette,
}

fn rig(data: &Path) -> Rig {
    let dir = DataDir::open(data).unwrap();
    let res = ExeResources::load(&dir).unwrap();
    let assets = StoryAssets::load(&dir).unwrap();
    let mut menu = MainMenu::new(MenuGraphics::load(&dir));
    menu.enter(1, true, Reentry::Loader { first: false });
    for _ in 0..40 {
        menu.step(&TickInput::default());
    }
    let pal = menu.palette_buffer().clone();
    Rig {
        dir,
        res,
        assets,
        pal,
    }
}

fn files(capture: &Path, when: &str) -> SaveSet {
    let f = |name: &str| fs::read(capture.join("files").join(format!("{name}.{when}"))).unwrap();
    SaveSet {
        slots: SaveSlots::parse(&f("td.sav")),
        names: SaveNames::parse(&f("td.stt")),
    }
}

fn open(r: &Rig, kind: SaveKind, set: SaveSet) -> SaveScreen {
    let id = match kind {
        SaveKind::Save { .. } => "saves.save",
        SaveKind::Restore => "saves.restore",
    };
    let labels = r.res.texts(id).unwrap();
    let mut s = SaveScreen::open(kind, &r.dir, &r.assets.small, labels, &r.pal, set);
    for _ in 0..40 {
        s.step(&TickInput::default());
    }
    assert!(s.choosing());
    s
}

fn keys(s: &mut SaveScreen, keys: &[Key]) {
    for &k in keys {
        s.step(&TickInput {
            pressed: vec![k],
            ..TickInput::default()
        });
    }
}

fn shifted(s: &mut SaveScreen, k: Key) {
    s.step(&TickInput {
        pressed: vec![k],
        held: [Key::LSHIFT].into_iter().collect(),
        pad: None,
    });
}

/// Pixels must match the frame exactly; the DAC on the entries the frame uses, unless the
/// frame was taken during the fade-out (`dac` false).
fn compare(s: &SaveScreen, capture: &Path, name: &str, dac: bool) {
    let raw = fs::read(capture.join("fb").join(name)).unwrap();
    let (pal, frame) = (Palette::parse_prefix(&raw).unwrap(), &raw[768..]);
    let ours = s.screen();
    let d: Vec<(usize, usize)> = (0..64_000)
        .filter(|&i| ours.pixels[i] != frame[i])
        .map(|i| (i % 320, i / 320))
        .collect();
    assert!(
        d.is_empty(),
        "{name}: {} pixels differ, first {:?}",
        d.len(),
        &d[..d.len().min(8)]
    );
    if dac {
        let mut used = [false; 256];
        for &p in frame {
            used[usize::from(p)] = true;
        }
        for e in (0..256).filter(|&e| used[e]) {
            assert_eq!(ours.palette.colors[e], pal.colors[e], "{name}: DAC {e}");
        }
    }
}

fn capture(oracle: &Path, id: &str) -> PathBuf {
    oracle.join(id)
}

#[test]
fn save_into_an_empty_set_matches_c0042() {
    let data = require_data!();
    let oracle = require_oracle!();
    let r = rig(&data);
    let c = capture(&oracle, "C-0042");
    let save = SaveKind::Save {
        mission: 7,
        kills: 1234,
    };
    let mut s = open(&r, save, files(&c, "before"));
    // §6 vector 1: every slot empty, the highlight on slot 1.
    compare(&s, &c, "f00000410_320x200_save-empty.vga", true);
    keys(&mut s, &[Key::DOWN, Key::DOWN, Key::DOWN]);
    compare(&s, &c, "f00000450_320x200_save-slot4.vga", true);
    keys(&mut s, &[Key::ENTER]);
    assert!(s.entering_name());
    compare(&s, &c, "f00000475_320x200_save-entry-blank.vga", true);
    // §6 vector 2: r, Shift+e, '.', '-', 7, space, ',', '/', a, b, Backspace, Enter.
    let l = |c| Key::letter(c).unwrap();
    keys(&mut s, &[l('r')]);
    shifted(&mut s, l('e'));
    keys(
        &mut s,
        &[
            Key(55),
            Key(45),
            Key::digit(7).unwrap(),
            Key::SPACE,
            Key(54),
            Key(56),
            l('a'),
            l('b'),
        ],
    );
    compare(&s, &c, "f00000574_320x200_save-entry-typed-b.vga", true);
    keys(&mut s, &[Key::BACKSPACE]);
    compare(&s, &c, "f00000587_320x200_save-entry-typed.vga", true);
    keys(&mut s, &[Key::ENTER]);
    compare(&s, &c, "f00000605_320x200_save-after-enter.vga", false);
    for _ in 0..40 {
        s.step(&TickInput::default());
    }
    let Some(SaveOutcome::Saved(set)) = s.outcome() else {
        panic!("{:?}", s.outcome());
    };
    let after = |name: &str| fs::read(c.join("files").join(format!("{name}.after"))).unwrap();
    assert_eq!(set.slots.to_bytes(), after("td.sav"), "td.sav");
    assert_eq!(set.names.to_bytes(), after("td.stt"), "td.stt");

    // §6 vector 4: the restore screen on the result; Enter on the empty slot 1 does nothing,
    // Down × 3, Esc leaves without a change.
    let mut s = open(&r, SaveKind::Restore, files(&c, "after"));
    compare(&s, &c, "f00001044_320x200_restore-one-save.vga", true);
    keys(&mut s, &[Key::ENTER]);
    assert!(s.choosing());
    compare(&s, &c, "f00001069_320x200_restore-enter-on-empty.vga", true);
    keys(&mut s, &[Key::DOWN, Key::DOWN, Key::DOWN]);
    compare(&s, &c, "f00001104_320x200_restore-slot4.vga", true);
    keys(&mut s, &[Key::ESCAPE]);
    for _ in 0..40 {
        s.step(&TickInput::default());
    }
    assert_eq!(s.outcome(), Some(&SaveOutcome::Left));
}

#[test]
fn restore_and_save_with_the_shipped_files_match_c0043() {
    let data = require_data!();
    let oracle = require_oracle!();
    let r = rig(&data);
    let c = capture(&oracle, "C-0043");
    // §6 vector 5: Down × 4, Enter loads slot 5 (mission 2, 13 kills).
    let mut s = open(&r, SaveKind::Restore, files(&c, "before"));
    compare(&s, &c, "f00000477_320x200_restore-full.vga", true);
    keys(&mut s, &[Key::DOWN, Key::DOWN, Key::DOWN, Key::DOWN]);
    compare(&s, &c, "f00000527_320x200_restore-slot5.vga", true);
    keys(&mut s, &[Key::ENTER]);
    for _ in 0..40 {
        s.step(&TickInput::default());
    }
    assert_eq!(
        s.outcome(),
        Some(&SaveOutcome::Restored {
            mission: 2,
            kills: 13
        })
    );
    // §6 vector 6: slot 2, a..y (20 kept), Esc (ignored), Backspace, z, Enter.
    let save = SaveKind::Save {
        mission: 2,
        kills: 13,
    };
    let mut s = open(&r, save, files(&c, "before"));
    compare(&s, &c, "f00000962_320x200_save-full.vga", true);
    keys(&mut s, &[Key::DOWN, Key::ENTER]);
    compare(&s, &c, "f00000997_320x200_save-entry-blank.vga", true);
    let letters: Vec<Key> = ('a'..='y').map(|c| Key::letter(c).unwrap()).collect();
    keys(&mut s, &letters);
    compare(&s, &c, "f00001212_320x200_save-entry-25.vga", true);
    keys(&mut s, &[Key::ESCAPE]);
    assert!(s.entering_name(), "Esc does not cancel (OB-056)");
    compare(&s, &c, "f00001224_320x200_save-entry-after-esc.vga", true);
    keys(&mut s, &[Key::BACKSPACE, Key::letter('z').unwrap()]);
    compare(&s, &c, "f00001244_320x200_save-entry-final.vga", true);
    keys(&mut s, &[Key::ENTER]);
    for _ in 0..40 {
        s.step(&TickInput::default());
    }
    let Some(SaveOutcome::Saved(set)) = s.outcome() else {
        panic!("{:?}", s.outcome());
    };
    let after = |name: &str| fs::read(c.join("files").join(format!("{name}.after"))).unwrap();
    assert_eq!(set.slots.to_bytes(), after("td.sav"), "td.sav");
    assert_eq!(set.names.to_bytes(), after("td.stt"), "td.stt");
}

#[test]
fn the_menu_restores_from_and_saves_to_the_port_s_own_files() {
    // Menu items 2 and 1 through Episode1 (save-screens §4.3–§4.4): a restore sets mission
    // and kills and moves the cursor to 3; a save writes td.sav/td.stt in the save directory.
    let data = require_data!();
    let r = rig(&data);
    let dir = std::env::temp_dir().join(format!("td2192-save-parity-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let mut set = SaveSet::default();
    set.slots.slots[4] = td_formats::saves::Slot {
        mission: 2,
        kills: 13,
    };
    set.names.names[4] = " pilot".to_owned();
    set.store(&dir, td_flow::save_screens::E1_FILES).unwrap();
    let launcher = Box::new(td_flow::PlaceholderLauncher::default());
    let mut ep = td_flow::Episode1::new(&r.dir, &r.res, launcher)
        .unwrap()
        .with_save_dir(dir.clone());
    let settings = td_flow::Settings::default();
    let mut audio = Vec::new();
    let mut ctx = td_flow::FlowContext {
        data: &r.dir,
        resources: &r.res,
        settings: &settings,
        audio: &mut audio,
    };
    let mut run = |ep: &mut td_flow::Episode1, k: Option<Key>, frames: u32| {
        for f in 0..frames {
            let input = TickInput {
                pressed: if f == 0 {
                    k.into_iter().collect()
                } else {
                    Vec::new()
                },
                ..TickInput::default()
            };
            td_flow::Screen::tick(ep, &mut ctx, &input);
        }
    };
    run(&mut ep, None, 40);
    run(&mut ep, Key::digit(1), 120);
    assert!(ep.menu().waiting());
    run(&mut ep, Some(Key::DOWN), 20);
    assert_eq!(ep.menu().cursor(), 2);
    run(&mut ep, Some(Key::ENTER), 80);
    for _ in 0..4 {
        run(&mut ep, Some(Key::DOWN), 1);
    }
    run(&mut ep, Some(Key::ENTER), 120);
    assert_eq!(ep.game(), (2, 13));
    assert_eq!(ep.menu().cursor(), 3);
    // Up twice reaches item 1 (save) now that a game is in progress; save into slot 1.
    run(&mut ep, Some(Key::UP), 20);
    run(&mut ep, Some(Key::UP), 20);
    assert_eq!(ep.menu().cursor(), 1);
    run(&mut ep, Some(Key::ENTER), 80);
    run(&mut ep, Some(Key::ENTER), 1);
    run(&mut ep, Key::letter('x'), 1);
    run(&mut ep, Some(Key::ENTER), 80);
    let after = SaveSet::load(Some(&dir), td_flow::save_screens::E1_FILES);
    assert_eq!(
        after.slots.slots[0],
        td_formats::saves::Slot {
            mission: 2,
            kills: 13
        }
    );
    assert_eq!(after.names.names[0], format!("{:<24}", " x"));
    assert_eq!(after.names.names[4], " pilot");
    assert_eq!(ep.menu().cursor(), 1, "the cursor stays on save");
    fs::remove_dir_all(&dir).unwrap();
}
