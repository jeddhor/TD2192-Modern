//! What the Episode 1 front end leaves in DGROUP for the flight: the bytes a volley on a
//! two-digit heading reads as its muzzle offsets (OB-017, preserved).
//!
//! Spec: `docs/spec/sim/weapons.md` rev 4 §3.3 (U-160) and `docs/spec/sim/mission-ships.md`
//! rev 5 §4.4.2. Fidelity: T1 (the offsets decide where a two-digit-heading shot makes its one
//! hit test).
//!
//! A volley on heading h reads muzzle m's offsets at DS:(035Ch + 34h + 2m) and the byte after
//! it. For h = 1..9 that is the muzzle table of the executable; the eight two-digit headings
//! read 272 bytes of other globals. None of them is written while a flight runs, so the port
//! builds them once per flight from three constants of the reference machine and the
//! front-end state the program run carries ([`FrontEnd`]), and hands td-sim the whole vector
//! as its muzzle world ([`super::World::muzzles`]; index = DS address − 035Ch).
//!
//! Episode 2 and 3 read their own DGROUP from DS:04BA, whose two-digit ranges are not mapped
//! yet (U-441): their worlds keep the plain table, i.e. offsets (0, 0) past it.

/// DS offset of the muzzle table (`file0001`): index 0 of the muzzle world.
pub const MUZZLE_BASE: u16 = 0x035C;
/// Bytes of the muzzle table in the executable (headings 1..9, muzzles 0..17).
pub const MUZZLE_TABLE_LEN: usize = 342;
/// Length of the muzzle world: heading 62h, muzzle 17, dy byte included (34 × 99 + 36).
pub const MUZZLE_WORLD_LEN: usize = 34 * 99 + 36;
/// The two-digit heading codes (keypad codes of the 22.5° headings).
pub const TWO_DIGIT_HEADINGS: [u8; 8] = [0x0C, 0x0E, 0x20, 0x24, 0x4A, 0x4E, 0x60, 0x62];

/// DS offsets of the globals the 272 bytes come from (E1 `file0001`, weapons §3.3).
const MENU_PALETTE: u16 = 0x07E0;
const SCENE_PALETTE: u16 = 0x0D2E;
const SCREEN_LIT: u16 = 0x102E;
const FONT_TITLE: u16 = 0x1032;
const READER_CODE: u16 = 0x1036;
const READER_FIELDS: u16 = 0x103A;
const READER_TEXT: u16 = 0x103E;

/// DS:04F6..0503: the runtime's `SegA000`, `SegB000`, `SegB800`, `Test8086` (2 on a 386),
/// `Test8087` (0), `FileMode` (2), a padding byte, `k_CrLf`, then `g_SbIrq` = 7, the IRQ of
/// the reference machine's Sound Blaster (preset P1; FFFFh without a card).
const RUNTIME_CONSTANTS: [u8; 14] = [
    0x00, 0xA0, 0x00, 0xB0, 0x00, 0xB8, 0x02, 0x00, 0x02, 0x00, 0x0D, 0x0A, 0x07, 0x00,
];
/// DS:0504: `g_MemAvailText` = `Str(MemAvail)` of the reference machine, `203352` (length
/// byte, digits; every later byte stays 0; U-440 asks whether it ever changes).
const MEM_AVAIL_TEXT: [u8; 7] = [6, b'2', b'0', b'3', b'3', b'5', b'2'];
/// Offset word of `g_FontTitle` (the title-font block starts 8 bytes into its allocation).
const FONT_TITLE_OFFSET: u16 = 0x0008;

/// How the program was started: decides where DOS loaded it, hence the heap segment of the
/// title-font block that `g_FontTitle` points to (weapons §3.3 item 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Launch {
    /// `FILE0001.EXE` started directly (the oracle captures: DGROUP 4467h).
    #[default]
    Direct,
    /// Started by the launcher `td1.exe`, as the game is played (C-0002: DGROUP 4FEBh).
    Launcher,
}

impl Launch {
    /// The segment word of `g_FontTitle` on the reference machine: DGROUP + 192Bh.
    #[must_use]
    pub fn title_font_segment(self) -> u16 {
        match self {
            Launch::Direct => 0x5D92,
            Launch::Launcher => 0x6916,
        }
    }
}

/// The dialogue reader's line state (`Dlg_PlayScene` `051A:92AF`, weapons §3.3 item 8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialogueReader {
    /// DS:1036: the code byte of the last line read (64h `d` at the end of a scene, 05h when
    /// Esc ended it at a page wait).
    pub code: u8,
    /// DS:103A..103C: the fields of the last `01`, `02` or `06` line, as stored.
    pub fields: [u8; 3],
    /// DS:103E `g_DlgText`: `string[255]`, length byte then characters; the tail of a longer
    /// earlier string survives a shorter one.
    pub text: [u8; 256],
}

impl Default for DialogueReader {
    fn default() -> Self {
        Self {
            code: 0,
            fields: [0; 3],
            text: [0; 256],
        }
    }
}

impl DialogueReader {
    /// Stores `s` the way BP7 `Read(s)` does into a `string[255]`: the length byte and the
    /// characters (at most 255); the bytes after them keep their old values.
    pub fn read_text(&mut self, s: &[u8]) {
        let n = s.len().min(255);
        self.text[0] = n as u8;
        self.text[1..=n].copy_from_slice(&s[..n]);
    }
}

/// The front-end globals a program run carries into its flights (weapons §3.3 items 5–8): all
/// 0 at program start, never reset, changed only by the front-end screens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontEnd {
    /// `g_Palette` (DS:0D2E): the scene palette, 768 bytes of 6-bit components, as the last
    /// front-end screen left it.
    pub scene_palette: [u8; 768],
    /// `g_ScreenLit` (DS:102E): 1 after a scene fade-in, 0 after a fade-out.
    pub lit: bool,
    /// The dialogue reader's last line.
    pub reader: DialogueReader,
    /// How the program was started (the title-font pointer).
    pub launch: Launch,
}

impl Default for FrontEnd {
    fn default() -> Self {
        Self::direct_start()
    }
}

impl FrontEnd {
    /// A direct start (`tdstatus.dat` resume, Esc at the mission card; how the oracle captures
    /// start a flight): no screen has touched the scene palette or played a dialogue
    /// (weapons §3.3 test vector (a)).
    #[must_use]
    pub fn direct_start() -> Self {
        Self {
            scene_palette: [0; 768],
            lit: false,
            reader: DialogueReader::default(),
            launch: Launch::Direct,
        }
    }

    /// Program start through the launcher (the game as played): everything 0 until the
    /// screens change it.
    #[must_use]
    pub fn launcher() -> Self {
        Self {
            launch: Launch::Launcher,
            ..Self::direct_start()
        }
    }

    /// Sets scene-palette entry `e` to 6-bit (r, g, b).
    pub fn set_entry(&mut self, e: u8, rgb: [u8; 3]) {
        let i = 3 * usize::from(e);
        self.scene_palette[i..i + 3].copy_from_slice(&rgb);
    }

    /// The byte at DS offset `addr` of the DGROUP the flight sees, for the addresses the
    /// two-digit headings read (weapons §3.3 construction table); 0 elsewhere. `menu_palette`
    /// is `tdmen.pal` (768 bytes; only bytes 70..103 are read, which the menu never changes).
    #[must_use]
    pub fn ds_byte(&self, addr: u16, menu_palette: &[u8]) -> u8 {
        let at = |base: u16, len: usize, bytes: &[u8]| {
            let i = usize::from(addr - base);
            (i < len).then(|| bytes.get(i).copied().unwrap_or(0))
        };
        let segment = self.launch.title_font_segment().to_le_bytes();
        let font = FONT_TITLE_OFFSET.to_le_bytes();
        match addr {
            0x04F6..=0x0503 => RUNTIME_CONSTANTS[usize::from(addr - 0x04F6)],
            0x0504..=0x050A => MEM_AVAIL_TEXT[usize::from(addr - 0x0504)],
            MENU_PALETTE..SCENE_PALETTE => at(MENU_PALETTE, 768, menu_palette).unwrap_or(0),
            SCENE_PALETTE..SCREEN_LIT => self.scene_palette[usize::from(addr - SCENE_PALETTE)],
            SCREEN_LIT => u8::from(self.lit),
            FONT_TITLE => font[0],
            0x1033 => font[1],
            0x1034 => segment[0],
            0x1035 => segment[1],
            READER_CODE => self.reader.code,
            READER_FIELDS..=0x103C => self.reader.fields[usize::from(addr - READER_FIELDS)],
            READER_TEXT..=0x113D => self.reader.text[usize::from(addr - READER_TEXT)],
            _ => 0,
        }
    }

    /// The 34 DS bytes heading `h` reads for muzzles 1..17 (dx, dy pairs), from
    /// DS:(035Ch + 34h + 2).
    #[must_use]
    pub fn heading_bytes(&self, h: u8, menu_palette: &[u8]) -> [u8; 34] {
        let first = MUZZLE_BASE + 34 * u16::from(h) + 2;
        std::array::from_fn(|k| self.ds_byte(first + k as u16, menu_palette))
    }
}

/// The E1 muzzle world (weapons §3.3 "Construction"): `table` (the executable's 342-byte
/// muzzle table) at indices 0..342, the 272 bytes of the eight two-digit headings built from
/// `front_end` and `menu_palette` (`tdmen.pal`), 0 elsewhere (never read).
#[must_use]
pub fn muzzle_world(table: &[u8], menu_palette: &[u8], front_end: &FrontEnd) -> Vec<u8> {
    let mut world = vec![0; MUZZLE_WORLD_LEN];
    let n = table.len().min(MUZZLE_TABLE_LEN);
    world[..n].copy_from_slice(&table[..n]);
    for h in TWO_DIGIT_HEADINGS {
        let at = 34 * usize::from(h) + 2;
        world[at..at + 34].copy_from_slice(&front_end.heading_bytes(h, menu_palette));
    }
    world
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A recognisable menu palette: byte i = i mod 251.
    fn menu() -> Vec<u8> {
        (0..768).map(|i| (i % 251) as u8).collect()
    }

    #[test]
    fn direct_start_bytes_follow_the_construction_table() {
        // weapons §3.3 test vector (a): 4Ah, 4Eh and 62h all 0; 60h 0 except DS:1032..1035 =
        // 08 00 92 5D; 0Ch the runtime constants, g_SbIrq and g_MemAvailText; 0Eh, 20h 0.
        let fe = FrontEnd::direct_start();
        let m = menu();
        let h0c = fe.heading_bytes(0x0C, &m);
        assert_eq!(
            h0c[..21],
            [
                0x00, 0xA0, 0x00, 0xB0, 0x00, 0xB8, 0x02, 0x00, 0x02, 0x00, 0x0D, 0x0A, 0x07, 0x00,
                0x06, 0x32, 0x30, 0x33, 0x33, 0x35, 0x32
            ]
        );
        assert_eq!(h0c[21..], [0; 13]);
        assert_eq!(fe.heading_bytes(0x0E, &m), [0; 34]);
        assert_eq!(fe.heading_bytes(0x20, &m), [0; 34]);
        // 24h: tdmen.pal bytes 70..103.
        let h24 = fe.heading_bytes(0x24, &m);
        assert_eq!(h24.to_vec(), (70..104).map(|i| i as u8).collect::<Vec<_>>());
        assert_eq!(fe.heading_bytes(0x4A, &m), [0; 34]);
        assert_eq!(fe.heading_bytes(0x4E, &m), [0; 34]);
        let mut h60 = [0u8; 34];
        h60[20..24].copy_from_slice(&[0x08, 0x00, 0x92, 0x5D]);
        assert_eq!(fe.heading_bytes(0x60, &m), h60);
        assert_eq!(fe.heading_bytes(0x62, &m), [0; 34]);
    }

    #[test]
    fn heading_ranges_are_the_spec_addresses() {
        // weapons §3.3 table: 0Ch DS:04F6.., 0Eh 053A, 20h 079E, 24h 0826, 4Ah 0D32, 4Eh 0DBA,
        // 60h 101E, 62h 1062.
        let first = |h: u16| MUZZLE_BASE + 34 * h + 2;
        assert_eq!(
            TWO_DIGIT_HEADINGS.map(|h| first(u16::from(h))),
            [
                0x04F6, 0x053A, 0x079E, 0x0826, 0x0D32, 0x0DBA, 0x101E, 0x1062
            ]
        );
        // The last byte read (heading 62h, muzzle 17, dy) lies inside the world.
        const { assert!(34 * 0x62 + 2 * 17 + 1 < MUZZLE_WORLD_LEN) };
    }

    #[test]
    fn campaign_state_reaches_the_northern_headings() {
        // 4Ah: scene bytes 4..37; 4Eh: 140..173; 60h: 752..767, lit, 0 × 3, the title font
        // (launcher segment 6916h), the reader's code byte, 0 × 3, fields, 0, text bytes 0, 1;
        // 62h: text bytes 36..69.
        let mut fe = FrontEnd::launcher();
        for (i, b) in fe.scene_palette.iter_mut().enumerate() {
            *b = (i % 64) as u8;
        }
        fe.lit = true;
        fe.reader.code = 0x64;
        fe.reader.fields = [1, 0x2E, 0xFB];
        for (i, b) in fe.reader.text.iter_mut().enumerate() {
            *b = i as u8;
        }
        let m = menu();
        let pal = |r: std::ops::Range<usize>| r.map(|i| (i % 64) as u8).collect::<Vec<_>>();
        assert_eq!(fe.heading_bytes(0x4A, &m).to_vec(), pal(4..38));
        assert_eq!(fe.heading_bytes(0x4E, &m).to_vec(), pal(140..174));
        let h60 = fe.heading_bytes(0x60, &m);
        assert_eq!(h60[..16].to_vec(), pal(752..768));
        assert_eq!(
            h60[16..],
            [
                1, 0, 0, 0, 0x08, 0x00, 0x16, 0x69, 0x64, 0, 0, 0, 1, 0x2E, 0xFB, 0, 0, 1
            ]
        );
        assert_eq!(
            fe.heading_bytes(0x62, &m).to_vec(),
            (36..70).map(|i| i as u8).collect::<Vec<_>>()
        );
        // The southern and western headings do not depend on the front end.
        let direct = FrontEnd::direct_start();
        for h in [0x0C, 0x0E, 0x20, 0x24] {
            assert_eq!(
                fe.heading_bytes(h, &m),
                direct.heading_bytes(h, &m),
                "{h:#X}"
            );
        }
    }

    #[test]
    fn muzzle_world_places_table_and_headings() {
        let table: Vec<u8> = (0..MUZZLE_TABLE_LEN).map(|i| (i % 200) as u8 + 1).collect();
        let m = menu();
        let fe = FrontEnd::launcher();
        let w = muzzle_world(&table, &m, &fe);
        assert_eq!(w.len(), MUZZLE_WORLD_LEN);
        assert_eq!(w[..MUZZLE_TABLE_LEN], table[..]);
        // Heading 0Ch muzzle 1 = DS:04F6 = SegA000 low byte 0; muzzle 1 dy = A0h.
        let i = 34 * 0x0C + 2;
        assert_eq!((w[i], w[i + 1]), (0x00, 0xA0));
        // Heading 60h muzzle 11 (dx, dy) = the title-font offset word 08h, 00h; muzzle 12 =
        // its segment.
        let i = 34 * 0x60 + 2 * 11;
        assert_eq!(w[i..i + 4], [0x08, 0x00, 0x16, 0x69]);
        // Bytes outside the table and the eight ranges are never read: 0.
        assert!(w[MUZZLE_TABLE_LEN..34 * 0x0C + 2].iter().all(|&b| b == 0));
    }

    #[test]
    fn read_text_keeps_the_tail_of_a_longer_string() {
        let mut r = DialogueReader::default();
        r.read_text(b"abcdef");
        r.read_text(b"xy");
        assert_eq!(r.text[..7], [2, b'x', b'y', b'c', b'd', b'e', b'f']);
    }
}
