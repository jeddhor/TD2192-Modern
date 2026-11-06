use super::*;

/// A recognisable menu: palette byte i = i mod 251, image row byte i = 200 − i mod 199.
fn menu() -> MenuBytes {
    MenuBytes {
        palette: (0..768).map(|i| (i % 251) as u8).collect(),
        last_row: (0..320).map(|i| 200 - (i % 199) as u8).collect(),
    }
}

/// A recognisable scene palette: byte i = i mod 64.
fn patterned_scene(fe: &mut FrontEnd) {
    for (i, b) in fe.scene_palette.iter_mut().enumerate() {
        *b = (i % 64) as u8;
    }
}

fn pal(r: std::ops::Range<usize>) -> Vec<u8> {
    r.map(|i| (i % 64) as u8).collect()
}

const E1: Episode = Episode::E1;
const E2: Episode = Episode::E2;
const E3: Episode = Episode::E3;

#[test]
fn direct_start_bytes_follow_the_construction_table() {
    // weapons §3.3 test vector (a): 4Ah, 4Eh and 62h all 0; 60h 0 except DS:1032..1035 =
    // 08 00 92 5D; 0Ch the runtime constants, g_SbIrq and g_MemAvailText; 0Eh, 20h 0.
    let fe = FrontEnd::direct_start();
    let m = menu();
    let h0c = fe.heading_bytes(E1, 0x0C, &m);
    assert_eq!(
        h0c[..21],
        [
            0x00, 0xA0, 0x00, 0xB0, 0x00, 0xB8, 0x02, 0x00, 0x02, 0x00, 0x0D, 0x0A, 0x07, 0x00,
            0x06, 0x32, 0x30, 0x33, 0x33, 0x35, 0x32
        ]
    );
    assert_eq!(h0c[21..], [0; 13]);
    assert_eq!(fe.heading_bytes(E1, 0x0E, &m), [0; 34]);
    assert_eq!(fe.heading_bytes(E1, 0x20, &m), [0; 34]);
    // 24h: tdmen.pal bytes 70..103.
    let h24 = fe.heading_bytes(E1, 0x24, &m);
    assert_eq!(h24.to_vec(), (70..104).map(|i| i as u8).collect::<Vec<_>>());
    assert_eq!(fe.heading_bytes(E1, 0x4A, &m), [0; 34]);
    assert_eq!(fe.heading_bytes(E1, 0x4E, &m), [0; 34]);
    let mut h60 = [0u8; 34];
    h60[20..24].copy_from_slice(&[0x08, 0x00, 0x92, 0x5D]);
    assert_eq!(fe.heading_bytes(E1, 0x60, &m), h60);
    assert_eq!(fe.heading_bytes(E1, 0x62, &m), [0; 34]);
}

#[test]
fn heading_ranges_are_the_spec_addresses() {
    // weapons §3.3 table: 0Ch DS:04F6.., 0Eh 053A, 20h 079E, 24h 0826, 4Ah 0D32, 4Eh 0DBA,
    // 60h 101E, 62h 1062; §3.3.2 (E2 and E3, E1's + 15Eh): 0654, 0698, 08FC, 0984, 0E90,
    // 0F18, 117C, 11C0.
    let first = |ep: Episode, h: u8| muzzle_base(ep) + 34 * u16::from(h) + 2;
    assert_eq!(
        TWO_DIGIT_HEADINGS.map(|h| first(E1, h)),
        [
            0x04F6, 0x053A, 0x079E, 0x0826, 0x0D32, 0x0DBA, 0x101E, 0x1062
        ]
    );
    for ep in [E2, E3] {
        assert_eq!(
            TWO_DIGIT_HEADINGS.map(|h| first(ep, h)),
            [
                0x0654, 0x0698, 0x08FC, 0x0984, 0x0E90, 0x0F18, 0x117C, 0x11C0
            ]
        );
    }
    assert_eq!(MUZZLE_BASE_LATER - MUZZLE_BASE, 0x15E);
    // The last byte read (heading 62h, muzzle 17, dy) lies inside the world.
    const { assert!(34 * 0x62 + 2 * 17 + 1 < MUZZLE_WORLD_LEN) };
}

#[test]
fn campaign_state_reaches_the_northern_headings() {
    // 4Ah: scene bytes 4..37; 4Eh: 140..173; 60h: 752..767, lit, 0 × 3, the title font
    // (launcher segment 6916h), the reader's code byte, 0 × 3, fields, 0, text bytes 0, 1;
    // 62h: text bytes 36..69.
    let mut fe = FrontEnd::launcher();
    patterned_scene(&mut fe);
    fe.lit = true;
    fe.reader.code = 0x64;
    fe.reader.fields = [1, 0x2E, 0xFB];
    for (i, b) in fe.reader.text.iter_mut().enumerate() {
        *b = i as u8;
    }
    let m = menu();
    assert_eq!(fe.heading_bytes(E1, 0x4A, &m).to_vec(), pal(4..38));
    assert_eq!(fe.heading_bytes(E1, 0x4E, &m).to_vec(), pal(140..174));
    let h60 = fe.heading_bytes(E1, 0x60, &m);
    assert_eq!(h60[..16].to_vec(), pal(752..768));
    assert_eq!(
        h60[16..],
        [
            1, 0, 0, 0, 0x08, 0x00, 0x16, 0x69, 0x64, 0, 0, 0, 1, 0x2E, 0xFB, 0, 0, 1
        ]
    );
    assert_eq!(
        fe.heading_bytes(E1, 0x62, &m).to_vec(),
        (36..70).map(|i| i as u8).collect::<Vec<_>>()
    );
    // The southern and western headings do not depend on the front end; nor does E1 read the
    // E2/E3 state (scene file, drain key).
    let mut direct = FrontEnd::direct_start();
    direct.assign_scene_file("td2.sav");
    direct.drain_key = 0x48;
    for h in [0x0C, 0x0E, 0x20, 0x24] {
        assert_eq!(
            fe.heading_bytes(E1, h, &m),
            direct.heading_bytes(E1, h, &m),
            "{h:#X}"
        );
    }
    assert_eq!(
        muzzle_world(E1, &[], &m, &direct),
        muzzle_world(E1, &[], &m, &FrontEnd::direct_start())
    );
}

#[test]
fn later_episodes_follow_the_construction_table() {
    // weapons §3.3.2 "Construction" (E3: §3.3.3, the same table), field by field.
    let m = menu();
    let mut fe = FrontEnd::direct_start();
    patterned_scene(&mut fe);
    fe.assign_scene_file("td.lt2");
    fe.drain_key = 0x1B;
    // Set the E1-only state too: E2 and E3 do not read it.
    fe.lit = true;
    fe.reader.code = 0x64;
    fe.launch = Launch::Launcher;
    for ep in [E2, E3] {
        let h0c = fe.heading_bytes(ep, 0x0C, &m);
        let mut want = vec![
            0x00, 0xA0, 0x00, 0xB0, 0x00, 0xB8, 0x02, 0x00, 0x02, 0x00, 0x0D, 0x0A, // 0654
            0x1B, 0x00, // 0660: drain, 00
            0x07, 0x00, // 0662: g_SbIrq
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 0664..0669
            0x02, 0x02, 0x01, 0x00, // 066A: g_CfgWork, g_CfgCursor
            0x00, 0x00, 0x01, 0x00, // 066E: exit cursor, game over, advance, never written
        ];
        want.extend_from_slice(&m.last_row[..4]); // 0672: td.men 79,680..79,683
        assert_eq!(h0c.to_vec(), want, "{ep} 0Ch");
        // 0Eh: td.men bytes 79,718..79,751 (g_MenuRow 38..71).
        assert_eq!(
            fe.heading_bytes(ep, 0x0E, &m).to_vec(),
            m.last_row[38..72],
            "{ep} 0Eh"
        );
        // 20h, 24h: tdmen.pal bytes 188..221 and 324..357.
        assert_eq!(fe.heading_bytes(ep, 0x20, &m).to_vec(), m.palette[188..222]);
        assert_eq!(fe.heading_bytes(ep, 0x24, &m).to_vec(), m.palette[324..358]);
        // 4Ah: name bytes 2..35.
        let mut h4a = [0u8; 34];
        h4a[..4].copy_from_slice(b".lt2");
        assert_eq!(fe.heading_bytes(ep, 0x4A, &m), h4a, "{ep} 4Ah");
        // 4Eh, 60h: scene bytes 58..91 and 670..703.
        assert_eq!(fe.heading_bytes(ep, 0x4E, &m).to_vec(), pal(58..92));
        assert_eq!(fe.heading_bytes(ep, 0x60, &m).to_vec(), pal(670..704));
        // 62h: scene bytes 738..767, then g_FeLoopI/J = 00 00 FF 00.
        let h62 = fe.heading_bytes(ep, 0x62, &m);
        assert_eq!(h62[..30].to_vec(), pal(738..768));
        assert_eq!(h62[30..], [0x00, 0x00, 0xFF, 0x00]);
    }
}

#[test]
fn scene_file_names_on_heading_4a() {
    // weapons §3.3.2 item 8 and §3.3.3: heading 4Ah, muzzles 1..3 (dx, dy) per last name.
    let m = menu();
    let cases: [(&str, [(u8, u8); 3]); 5] = [
        ("td.lt2", [(46, 108), (116, 50), (0, 0)]),
        ("td.pal", [(46, 112), (97, 108), (0, 0)]),
        ("td.sss", [(46, 115), (115, 115), (0, 0)]),
        ("td2.sav", [(50, 46), (115, 97), (118, 0)]),
        ("td3.sav", [(51, 46), (115, 97), (118, 0)]),
    ];
    let mut fe = FrontEnd::direct_start();
    // Any order of assignments leaves the last name, NUL-padded (no name is longer than 7).
    for name in ["td2.sav", "td.lt2", "td3.sav", "td.pal", "td.sss"] {
        for (last, muzzles) in cases {
            fe.assign_scene_file(name);
            fe.assign_scene_file(last);
            assert_eq!(fe.scene_file_name(), last.as_bytes());
            let h = fe.heading_bytes(E2, 0x4A, &m);
            let got: Vec<(u8, u8)> = h.chunks(2).take(3).map(|p| (p[0], p[1])).collect();
            assert_eq!(got, muzzles, "{name} then {last}");
            assert!(h[6..].iter().all(|&b| b == 0), "{name} then {last}");
        }
    }
    // Program start: empty, all zero.
    assert_eq!(
        FrontEnd::direct_start().heading_bytes(E3, 0x4A, &m),
        [0; 34]
    );
}

#[test]
fn shooters_read_what_the_spec_lists() {
    // weapons §3.3.2 "Which shooters read what" and test vector (c), in muzzle-world terms
    // (index 34h + 2m).
    let m = menu();
    let mut fe = FrontEnd::direct_start();
    fe.drain_key = 0x50;
    let w = muzzle_world(E2, &[], &m, &fe);
    let at = |h: usize, mu: usize| (w[34 * h + 2 * mu] as i8, w[34 * h + 2 * mu + 1] as i8);
    // Class 2 on 0Ch: (0, −96), (0, −80), (2, 0), (2, 0).
    assert_eq!(
        [at(0x0C, 1), at(0x0C, 2), at(0x0C, 4), at(0x0C, 5)],
        [(0, -96), (0, -80), (2, 0), (2, 0)]
    );
    // Class 3 (8, 9): (7, 0) g_SbIrq, (0, 0). Class 4 (6, 7): (13, 10), (drain, 0).
    assert_eq!([at(0x0C, 8), at(0x0C, 9)], [(7, 0), (0, 0)]);
    assert_eq!([at(0x0C, 6), at(0x0C, 7)], [(13, 10), (0x50, 0)]);
    // Class 5 (3): (0, −72). Class 7 (12, 13): (2, 2), (1, 0).
    assert_eq!(at(0x0C, 3), (0, -72));
    assert_eq!([at(0x0C, 12), at(0x0C, 13)], [(2, 2), (1, 0)]);
    // Class 8 (14..17): (0, 0), (1, 0), then td.men bytes 79,680..79,683.
    let row = |i: usize| m.last_row[i] as i8;
    assert_eq!(
        [at(0x0C, 14), at(0x0C, 15), at(0x0C, 16), at(0x0C, 17)],
        [(0, 0), (1, 0), (row(0), row(1)), (row(2), row(3))]
    );
    // Test vector (c): class 8 on 62h, muzzle 16 (0, 0), muzzle 17 (−1, 0).
    assert_eq!([at(0x62, 16), at(0x62, 17)], [(0, 0), (-1, 0)]);
}

#[test]
fn muzzle_world_places_table_and_headings() {
    let table: Vec<u8> = (0..MUZZLE_TABLE_LEN).map(|i| (i % 200) as u8 + 1).collect();
    let m = menu();
    let fe = FrontEnd::launcher();
    for ep in [E1, E2, E3] {
        let w = muzzle_world(ep, &table, &m, &fe);
        assert_eq!(w.len(), MUZZLE_WORLD_LEN);
        assert_eq!(w[..MUZZLE_TABLE_LEN], table[..]);
        // Heading 0Ch muzzle 1 = SegA000 low byte 0; muzzle 1 dy = A0h.
        let i = 34 * 0x0C + 2;
        assert_eq!((w[i], w[i + 1]), (0x00, 0xA0));
        // Bytes outside the table and the eight ranges are never read: 0.
        assert!(w[MUZZLE_TABLE_LEN..34 * 0x0C + 2].iter().all(|&b| b == 0));
    }
    // E1 heading 60h muzzle 11 (dx, dy) = the title-font offset word 08h, 00h; muzzle 12 =
    // its segment.
    let w = muzzle_world(E1, &table, &m, &fe);
    let i = 34 * 0x60 + 2 * 11;
    assert_eq!(w[i..i + 4], [0x08, 0x00, 0x16, 0x69]);
}

#[test]
fn read_text_keeps_the_tail_of_a_longer_string() {
    let mut r = DialogueReader::default();
    r.read_text(b"abcdef");
    r.read_text(b"xy");
    assert_eq!(r.text[..7], [2, b'x', b'y', b'c', b'd', b'e', b'f']);
}

#[test]
fn assign_keeps_the_bytes_after_the_terminator() {
    let mut fe = FrontEnd::direct_start();
    fe.assign_scene_file("abcdefgh");
    fe.assign_scene_file("xy");
    assert_eq!(
        fe.scene_file[..9],
        [b'x', b'y', 0, b'd', b'e', b'f', b'g', b'h', 0]
    );
    assert_eq!(fe.scene_file_name(), b"xy");
}

#[test]
fn menu_bytes_take_the_last_image_row() {
    let men: Vec<u8> = (0..120_960u32).map(|i| (i % 253) as u8).collect();
    let pal: Vec<u8> = (0..768u32).map(|i| (i % 64) as u8).collect();
    let m = MenuBytes::new(&pal, &men);
    assert_eq!(m.palette, pal);
    assert_eq!(m.last_row, men[79_680..80_000]);
    // Short files read as zeros past their end.
    let short = MenuBytes::new(&pal[..10], &men[..79_700]);
    assert_eq!(short.last_row.len(), 20);
    let fe = FrontEnd::direct_start();
    assert_eq!(fe.heading_bytes(E2, 0x0E, &short), [0; 34]);
}

#[test]
fn direct_starts_follow_the_capture_method() {
    // M10-playthrough "Direct starts": the first scene card of each E2 block; E3 scene cards
    // or the restore screen and the title card.
    let file = |ep: Episode, n: u8| match direct_start(ep, n) {
        DirectStart::Card { file, .. } => file,
        DirectStart::RestoreThenTitle => "title",
        DirectStart::Nothing => "nothing",
    };
    let e2: Vec<&str> = (1..=20).map(|n| file(E2, n)).collect();
    assert_eq!(
        e2,
        [
            "marauder.scr",
            "velsroom.scr",
            "shooting.scr",
            "tdlab.scr",
            "bunker.scr",
            "sewer.scr",
            "bunker.scr",
            "tdlab.scr",
            "tdoffic2.scr",
            "shooting.scr",
            "sbar.scr",
            "sbar.scr",
            "tdoffic2.scr",
            "sbar.scr",
            "tdlab.scr",
            "marauder.scr",
            "hyde.scr",
            "tdoffic2.scr",
            "marauder.scr",
            "tdoffic2.scr"
        ]
    );
    let e3: Vec<&str> = [
        1, 2, 3, 4, 5, 6, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19,
    ]
    .into_iter()
    .map(|n| file(E3, n))
    .collect();
    assert_eq!(
        e3,
        [
            "shuttle0.scr",
            "command.scr",
            "convers.scr",
            "title",
            "convers.scr",
            "maraud3.scr",
            "maraud3.scr",
            "title",
            "title",
            "title",
            "kolorvci.scr",
            "title",
            "title",
            "kolorvci.scr",
            "title",
            "title",
            "title",
            "kolorvci.scr"
        ]
    );
    // ASSUMPTION(U-490): flights 7 and 21 take their block's start.
    assert_eq!(direct_start(E3, 7), direct_start(E3, 6));
    assert_eq!(direct_start(E3, 21), direct_start(E3, 11));
    assert!((1..=21).all(|n| direct_start(E1, n) == DirectStart::Nothing));
}

#[test]
fn direct_start_state() {
    let m = menu();
    let scr: Vec<u8> = (0..64_768u32).map(|i| (i % 61) as u8).collect();
    let read = |name: &str| -> Result<Vec<u8>, String> {
        if name.ends_with(".scr") {
            Ok(scr.clone())
        } else {
            Err(name.to_owned())
        }
    };
    // E1: nothing.
    assert_eq!(
        FrontEnd::after_direct_start(E1, 5, &m, read).unwrap(),
        FrontEnd::direct_start()
    );
    // E2 flight 2: velsroom's palette, ink 12 := (40, 40, 63), td.lt2.
    let fe = FrontEnd::after_direct_start(E2, 2, &m, read).unwrap();
    let mut want = scr[..768].to_vec();
    want[36..39].copy_from_slice(&[40, 40, 63]);
    assert_eq!(fe.scene_palette.to_vec(), want);
    assert_eq!(fe.scene_file_name(), b"td.lt2");
    assert_eq!(fe.drain_key, 0);
    // E3 flight 10: tdmen.pal with entries 224..233 := (16, 0, 0), entry 85 := (63, 0, 0),
    // td.sss.
    let fe = FrontEnd::after_direct_start(E3, 10, &m, read).unwrap();
    let mut want = m.palette.clone();
    for e in 224..=233 {
        want[3 * e..3 * e + 3].copy_from_slice(&[16, 0, 0]);
    }
    want[255..258].copy_from_slice(&[63, 0, 0]);
    assert_eq!(fe.scene_palette.to_vec(), want);
    assert_eq!(fe.scene_file_name(), b"td.sss");
    // A missing file is reported.
    let fail = |name: &str| -> Result<Vec<u8>, String> { Err(name.to_owned()) };
    assert_eq!(
        FrontEnd::after_direct_start(E3, 1, &m, fail),
        Err("shuttle0.scr".to_owned())
    );
    assert_eq!(
        direct_start_world(E2, 1, &[], fail),
        Err("tdmen.pal".to_owned())
    );
}
