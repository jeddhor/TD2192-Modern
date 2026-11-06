//! What the front end leaves in DGROUP for the flight: the bytes a volley on a two-digit
//! heading reads as its muzzle offsets (OB-017, preserved), in all three episodes.
//!
//! Spec: `docs/spec/sim/weapons.md` rev 5 §3.3 (E1, U-160), §3.3.2 (E2) and §3.3.3 (E3)
//! (U-441), and `docs/spec/sim/mission-ships.md` rev 5 §4.4.2. Fidelity: T1 (the offsets decide
//! where a two-digit-heading shot makes its one hit test).
//!
//! A volley on heading h reads muzzle m's offsets at DS:(base + 34h + 2m) and the byte after
//! it, base = DS:035C in E1 and DS:04BA in E2 and E3 ([`muzzle_base`]). For h = 1..9 that is
//! the muzzle table of the executable; the eight two-digit headings read 272 bytes of other
//! globals. None of them is written while a flight runs, so the port builds them once per
//! flight from constants of the reference machine and start, two installation files
//! ([`MenuBytes`]: `tdmen.pal`, and E2/E3 the last row of `td.men`) and the front-end state the
//! program run carries ([`FrontEnd`]), and hands td-sim the whole vector as its muzzle world
//! ([`super::World::muzzles`]; index = DS address − base, so the index of heading h, muzzle m
//! is 34h + 2m in every episode).
//!
//! A flight started directly, the way the oracle captures start one, ends a front-end sequence
//! of its own ([`DirectStart`], [`direct_start_world`]): nothing in E1, the block's first scene
//! card (or E3's restore screen and title card) in E2 and E3.

use crate::flight::episode::Episode;

/// DS offset of the E1 muzzle table (`file0001`): index 0 of the E1 muzzle world.
pub const MUZZLE_BASE: u16 = 0x035C;
/// DS offset of the E2 and E3 muzzle tables (`td2`, `td3`; weapons §3.3.2): index 0 of their
/// muzzle worlds.
pub const MUZZLE_BASE_LATER: u16 = 0x04BA;
/// Bytes of the muzzle table in the executable (headings 1..9, muzzles 0..17).
pub const MUZZLE_TABLE_LEN: usize = 342;
/// Length of the muzzle world: heading 62h, muzzle 17, dy byte included (34 × 99 + 36).
pub const MUZZLE_WORLD_LEN: usize = 34 * 99 + 36;
/// The two-digit heading codes (keypad codes of the 22.5° headings).
pub const TWO_DIGIT_HEADINGS: [u8; 8] = [0x0C, 0x0E, 0x20, 0x24, 0x4A, 0x4E, 0x60, 0x62];

/// The DS offset of `episode`'s muzzle table, index 0 of its muzzle world.
#[must_use]
pub fn muzzle_base(episode: Episode) -> u16 {
    match episode {
        Episode::E1 => MUZZLE_BASE,
        Episode::E2 | Episode::E3 => MUZZLE_BASE_LATER,
    }
}

/// The System unit's typed constants `SegA000`, `SegB000`, `SegB800`, `Test8086` (2 on a 386),
/// `Test8087` (0), `FileMode` (2), a padding byte and `k_CrLf`: E1 DS:04F6..0501, E2/E3
/// DS:0654..065F (weapons §3.3 item 1, §3.3.2 item 1).
const RUNTIME_CONSTANTS: [u8; 12] = [
    0x00, 0xA0, 0x00, 0xB0, 0x00, 0xB8, 0x02, 0x00, 0x02, 0x00, 0x0D, 0x0A,
];
/// `g_SbIrq`: the IRQ of the reference machine's Sound Blaster (preset P1, SB16 at IRQ 7;
/// FFFFh without a card, `/nosound` or `/pc`). E1 DS:0502, E2/E3 DS:0662.
const SB_IRQ_REFERENCE: u16 = 7;

/// DS offsets of the globals the E1 bytes come from (`file0001`, weapons §3.3).
mod e1 {
    pub const RUNTIME: u16 = 0x04F6;
    pub const SB_IRQ: u16 = 0x0502;
    pub const MEM_AVAIL_TEXT: u16 = 0x0504;
    pub const MENU_PALETTE: u16 = 0x07E0;
    pub const SCENE_PALETTE: u16 = 0x0D2E;
    pub const SCREEN_LIT: u16 = 0x102E;
    pub const FONT_TITLE: u16 = 0x1032;
    pub const READER_CODE: u16 = 0x1036;
    pub const READER_FIELDS: u16 = 0x103A;
    pub const READER_TEXT: u16 = 0x103E;
}

/// DS offsets of the globals the E2 and E3 bytes come from (`td2`, `td3`: the same addresses,
/// weapons §3.3.2, §3.3.3).
mod later {
    pub const RUNTIME: u16 = 0x0654;
    /// `g_MenuDrainKey`.
    pub const DRAIN: u16 = 0x0660;
    pub const SB_IRQ: u16 = 0x0662;
    /// `g_CfgWork` (music, SFX, controller) and `g_CfgCursor`.
    pub const CFG: u16 = 0x066A;
    /// `g_MenuExitCursor`, `g_GameOver`, `g_MissionAdvance` and a byte never written.
    pub const MENU_FLAGS: u16 = 0x066E;
    /// `g_MenuRow` (`u8[320]`).
    pub const MENU_ROW: u16 = 0x0672;
    pub const MENU_PALETTE: u16 = 0x0840;
    /// `g_SceneFile` (FileRec at DS:0E5E): its name field.
    pub const SCENE_FILE_NAME: u16 = 0x0E8E;
    pub const SCENE_PALETTE: u16 = 0x0EDE;
    /// `g_FeLoopI`, `g_FeLoopJ` (two u16).
    pub const FE_LOOPS: u16 = 0x11DE;
}

/// E1 DS:0504: `g_MemAvailText` = `Str(MemAvail)` of the reference machine, `203352` (length
/// byte, digits; every later byte stays 0; U-440 asks whether it ever changes).
const MEM_AVAIL_DIGITS: [u8; 7] = [6, b'2', b'0', b'3', b'3', b'5', b'2'];
/// Offset word of E1 `g_FontTitle` (the title-font block starts 8 bytes into its allocation).
const FONT_TITLE_OFFSET: u16 = 0x0008;
/// E2/E3 DS:066A..066D: `g_CfgWork` and `g_CfgCursor` of the reference start `TD2.EXE /sb` /
/// `TD3.EXE /sb` with the shipped `td.cfg` and no joystick: music device 2, SFX device 2
/// (Sound Blaster), controller 1 (keyboard), cursor 0 (weapons §3.3.2 item 4).
const CFG_REFERENCE: [u8; 4] = [0x02, 0x02, 0x01, 0x00];
/// E2/E3 DS:066E..0671 at every flight: `g_MenuExitCursor` 0, `g_GameOver` 0,
/// `g_MissionAdvance` 1 (set at the start of every block), a byte never written (item 5).
const MENU_FLAGS_AT_FLIGHT: [u8; 4] = [0x00, 0x00, 0x01, 0x00];
/// E2/E3 DS:11DE..11E1 at every flight: `g_FeLoopI` 0, `g_FeLoopJ` FFh, as the scene fade-out
/// that ends every front-end screen leaves them (item 10).
const FE_LOOPS_AT_FLIGHT: [u8; 4] = [0x00, 0x00, 0xFF, 0x00];

/// Length of a FileRec's name field (`array[0..79] of Char`).
pub const SCENE_FILE_NAME_LEN: usize = 80;
/// The last name the script choice, the scene cards (about and order included) and the story
/// so far assign to E2/E3 `g_SceneFile` (weapons §3.3.2 item 8).
pub const SCENE_FILE_CARDS: &str = "td.lt2";
/// The last name the dialogue scenes, the retry prompt, the debrief and the death dossier
/// assign (item 8).
pub const SCENE_FILE_PALETTE: &str = "td.pal";
/// The last name the restore screen, and the save screen when nothing was saved, assign
/// (item 8; a save assigns `td2.sav` / `td3.sav`).
pub const SCENE_FILE_SAVES: &str = "td.sss";

/// The installation bytes the two-digit headings read: `tdmen.pal` (E1 bytes 70..103, E2/E3
/// bytes 188..221 and 324..357; the menu never changes them) and, for E2/E3, the last row of
/// the main-menu image, which the menu loader's row buffer `g_MenuRow` keeps (weapons §3.3.2
/// item 6).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MenuBytes {
    /// `tdmen.pal` (768 bytes).
    pub palette: Vec<u8>,
    /// `td.men` bytes 79,680..79,999 (320 bytes).
    pub last_row: Vec<u8>,
}

impl MenuBytes {
    /// The menu palette file.
    pub const PALETTE_FILE: &'static str = "tdmen.pal";
    /// The menu image file.
    pub const IMAGE_FILE: &'static str = "td.men";
    /// Where the menu image's last row starts in `td.men`: the image is bytes 16,000..79,999,
    /// 200 rows of 320 (FORMATS §12.1).
    pub const LAST_ROW: usize = 79_680;

    /// From the contents of `tdmen.pal` and `td.men` (missing bytes read as 0).
    #[must_use]
    pub fn new(tdmen_pal: &[u8], td_men: &[u8]) -> Self {
        let row = td_men.get(Self::LAST_ROW..).unwrap_or_default();
        Self {
            palette: tdmen_pal.iter().copied().take(768).collect(),
            last_row: row.iter().copied().take(320).collect(),
        }
    }

    /// Reads the two files with `read` (an installation file's bytes by name).
    ///
    /// # Errors
    /// What `read` returns.
    pub fn load<E>(mut read: impl FnMut(&str) -> Result<Vec<u8>, E>) -> Result<Self, E> {
        let palette = read(Self::PALETTE_FILE)?;
        let image = read(Self::IMAGE_FILE)?;
        Ok(Self::new(&palette, &image))
    }
}

/// How the program was started: decides where DOS loaded it, hence the heap segment of the
/// title-font block that E1's `g_FontTitle` points to (weapons §3.3 item 7). E2 and E3 are
/// always started directly and read no heap pointer.
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

/// The dialogue reader's line state (E1 `Dlg_PlayScene` `051A:92AF`, weapons §3.3 item 8).
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

/// The front-end globals a program run carries into its flights (weapons §3.3 items 5–8,
/// §3.3.2 items 2, 8, 9): all 0 at program start, never reset, changed only by the front end.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontEnd {
    /// `g_Palette` (E1 DS:0D2E, E2/E3 DS:0EDE): the scene palette, 768 bytes of 6-bit
    /// components, as the last front-end screen left it.
    pub scene_palette: [u8; 768],
    /// E1 `g_ScreenLit` (DS:102E): 1 after a scene fade-in, 0 after a fade-out.
    pub lit: bool,
    /// E1: the dialogue reader's last line.
    pub reader: DialogueReader,
    /// E1: how the program was started (the title-font pointer).
    pub launch: Launch,
    /// E2/E3 `g_SceneFile`'s name field (DS:0E8E): the last name a front-end screen assigned,
    /// as `Assign` stores it (the characters, then a NUL; the bytes after the NUL keep their
    /// old values). Empty at program start.
    pub scene_file: [u8; SCENE_FILE_NAME_LEN],
    /// E2/E3 `g_MenuDrainKey` (DS:0660): the last byte the main menu read and discarded from
    /// the BIOS keyboard buffer in this program run, 0 if none (weapons §3.3.2 item 2).
    pub drain_key: u8,
}

impl Default for FrontEnd {
    fn default() -> Self {
        Self::direct_start()
    }
}

impl FrontEnd {
    /// A program start, or an E1 direct start (`tdstatus.dat` resume, Esc at the mission card;
    /// how the oracle captures start an E1 flight): no screen has touched the scene palette,
    /// played a dialogue or assigned a scene file, and the menu has discarded no key
    /// (weapons §3.3 test vector (a)).
    #[must_use]
    pub fn direct_start() -> Self {
        Self {
            scene_palette: [0; 768],
            lit: false,
            reader: DialogueReader::default(),
            launch: Launch::Direct,
            scene_file: [0; SCENE_FILE_NAME_LEN],
            drain_key: 0,
        }
    }

    /// E1 program start through the launcher (the game as played): everything 0 until the
    /// screens change it.
    #[must_use]
    pub fn launcher() -> Self {
        Self {
            launch: Launch::Launcher,
            ..Self::direct_start()
        }
    }

    /// The state a direct start of flight `flight` leaves ([`direct_start`]): E1
    /// [`FrontEnd::direct_start`]; E2/E3 the scene card's palette with its ink and `td.lt2`, or
    /// (E3) the menu's palette buffer that the restore screen copied with the title card's
    /// entry 85 and `td.sss`. `read` gives an installation file's bytes (the card's picture).
    ///
    /// # Errors
    /// What `read` returns.
    pub fn after_direct_start<E>(
        episode: Episode,
        flight: u8,
        menu: &MenuBytes,
        mut read: impl FnMut(&str) -> Result<Vec<u8>, E>,
    ) -> Result<Self, E> {
        let mut fe = Self::direct_start();
        if episode == Episode::E1 {
            return Ok(fe);
        }
        // Every E2/E3 start runs the script choice (dialogu2.dat / dialogu3.dat are installed):
        // g_Palette cleared but for five entries, g_SceneFile := td.lt2 (§3.3.2 items 8, 9).
        fe.assign_scene_file(SCENE_FILE_CARDS);
        match direct_start(episode, flight) {
            DirectStart::Nothing => {}
            DirectStart::Card { file, ink } => {
                let scr = read(file)?;
                fe.load_scene_palette(&scr);
                if let Some((e, rgb)) = ink {
                    fe.set_entry(e, rgb);
                }
                fe.assign_scene_file(SCENE_FILE_CARDS);
            }
            DirectStart::RestoreThenTitle => {
                fe.load_scene_palette(&menu.palette);
                for e in 224..=233 {
                    fe.set_entry(e, [16, 0, 0]);
                }
                fe.assign_scene_file(SCENE_FILE_SAVES);
                fe.set_entry(TITLE_ENTRY, TITLE_RGB);
            }
        }
        Ok(fe)
    }

    /// Sets scene-palette entry `e` to 6-bit (r, g, b).
    pub fn set_entry(&mut self, e: u8, rgb: [u8; 3]) {
        let i = 3 * usize::from(e);
        self.scene_palette[i..i + 3].copy_from_slice(&rgb);
    }

    /// Loads the first 768 bytes of `bytes` (a `.scr` picture or a palette) into the scene
    /// palette, as `BlockRead` does (missing bytes keep their old values).
    pub fn load_scene_palette(&mut self, bytes: &[u8]) {
        let n = bytes.len().min(768);
        self.scene_palette[..n].copy_from_slice(&bytes[..n]);
    }

    /// `Assign(g_SceneFile, name)`: the name's characters and a NUL into the name field
    /// (at most 79 characters); the bytes after the NUL keep their old values.
    pub fn assign_scene_file(&mut self, name: &str) {
        let n = name.len().min(SCENE_FILE_NAME_LEN - 1);
        self.scene_file[..n].copy_from_slice(&name.as_bytes()[..n]);
        self.scene_file[n] = 0;
    }

    /// The name `g_SceneFile` holds (up to its NUL).
    #[must_use]
    pub fn scene_file_name(&self) -> &[u8] {
        let n = self.scene_file.iter().position(|&b| b == 0).unwrap_or(0);
        &self.scene_file[..n]
    }

    /// The byte at DS offset `addr` of the DGROUP an `episode` flight sees, for the addresses
    /// the two-digit headings read (the construction tables of weapons §3.3 and §3.3.2); 0
    /// elsewhere.
    #[must_use]
    pub fn ds_byte(&self, episode: Episode, addr: u16, menu: &MenuBytes) -> u8 {
        match episode {
            Episode::E1 => self.e1_byte(addr, menu),
            Episode::E2 | Episode::E3 => self.later_byte(addr, menu),
        }
    }

    /// The E1 construction table (weapons §3.3). Only `tdmen.pal` bytes 70..103 are read,
    /// which the menu never changes.
    fn e1_byte(&self, addr: u16, menu: &MenuBytes) -> u8 {
        let rel = |base: u16| usize::from(addr - base);
        let segment = self.launch.title_font_segment().to_le_bytes();
        let font = FONT_TITLE_OFFSET.to_le_bytes();
        let irq = SB_IRQ_REFERENCE.to_le_bytes();
        match addr {
            e1::RUNTIME..e1::SB_IRQ => RUNTIME_CONSTANTS[rel(e1::RUNTIME)],
            e1::SB_IRQ..e1::MEM_AVAIL_TEXT => irq[rel(e1::SB_IRQ)],
            e1::MEM_AVAIL_TEXT..=0x050A => MEM_AVAIL_DIGITS[rel(e1::MEM_AVAIL_TEXT)],
            e1::MENU_PALETTE..e1::SCENE_PALETTE => byte(&menu.palette, rel(e1::MENU_PALETTE)),
            e1::SCENE_PALETTE..e1::SCREEN_LIT => self.scene_palette[rel(e1::SCENE_PALETTE)],
            e1::SCREEN_LIT => u8::from(self.lit),
            e1::FONT_TITLE => font[0],
            0x1033 => font[1],
            0x1034 => segment[0],
            0x1035 => segment[1],
            e1::READER_CODE => self.reader.code,
            e1::READER_FIELDS..=0x103C => self.reader.fields[rel(e1::READER_FIELDS)],
            e1::READER_TEXT..=0x113D => self.reader.text[rel(e1::READER_TEXT)],
            _ => 0,
        }
    }

    /// The E2/E3 construction table (weapons §3.3.2, §3.3.3: the same addresses).
    fn later_byte(&self, addr: u16, menu: &MenuBytes) -> u8 {
        use later as l;
        let rel = |base: u16| usize::from(addr - base);
        let irq = SB_IRQ_REFERENCE.to_le_bytes();
        match addr {
            l::RUNTIME..l::DRAIN => RUNTIME_CONSTANTS[rel(l::RUNTIME)],
            l::DRAIN => self.drain_key,
            l::SB_IRQ..0x0664 => irq[rel(l::SB_IRQ)],
            l::CFG..l::MENU_FLAGS => CFG_REFERENCE[rel(l::CFG)],
            l::MENU_FLAGS..l::MENU_ROW => MENU_FLAGS_AT_FLIGHT[rel(l::MENU_FLAGS)],
            l::MENU_ROW..0x07B2 => byte(&menu.last_row, rel(l::MENU_ROW)),
            l::MENU_PALETTE..0x0B40 => byte(&menu.palette, rel(l::MENU_PALETTE)),
            l::SCENE_FILE_NAME..l::SCENE_PALETTE => self.scene_file[rel(l::SCENE_FILE_NAME)],
            l::SCENE_PALETTE..l::FE_LOOPS => self.scene_palette[rel(l::SCENE_PALETTE)],
            l::FE_LOOPS..0x11E2 => FE_LOOPS_AT_FLIGHT[rel(l::FE_LOOPS)],
            _ => 0,
        }
    }

    /// The 34 DS bytes heading `h` reads for muzzles 1..17 (dx, dy pairs), from
    /// DS:(base + 34h + 2).
    #[must_use]
    pub fn heading_bytes(&self, episode: Episode, h: u8, menu: &MenuBytes) -> [u8; 34] {
        let first = muzzle_base(episode) + 34 * u16::from(h) + 2;
        std::array::from_fn(|k| self.ds_byte(episode, first + k as u16, menu))
    }
}

/// Byte `i` of `bytes`, 0 past its end.
fn byte(bytes: &[u8], i: usize) -> u8 {
    bytes.get(i).copied().unwrap_or(0)
}

/// The title card's entry and colour (`Scene_MissionTitle(text, 3Fh, 0, 0)`).
const TITLE_ENTRY: u8 = 85;
const TITLE_RGB: [u8; 3] = [63, 0, 0];

/// The muzzle world of an `episode` flight (weapons §3.3, §3.3.2 "Construction"): `table`
/// (the executable's 342-byte muzzle table) at indices 0..342, the 272 bytes of the eight
/// two-digit headings built from `front_end` and `menu`, 0 elsewhere (never read).
#[must_use]
pub fn muzzle_world(
    episode: Episode,
    table: &[u8],
    menu: &MenuBytes,
    front_end: &FrontEnd,
) -> Vec<u8> {
    let mut world = vec![0; MUZZLE_WORLD_LEN];
    let n = table.len().min(MUZZLE_TABLE_LEN);
    world[..n].copy_from_slice(&table[..n]);
    for h in TWO_DIGIT_HEADINGS {
        let at = 34 * usize::from(h) + 2;
        world[at..at + 34].copy_from_slice(&front_end.heading_bytes(episode, h, menu));
    }
    world
}

/// The last front-end screen before the flight when a flight is started directly, the way the
/// oracle captures start one (E1: `tdstatus.dat` resume with Esc at the mission card, VE-1; E2
/// and E3: `TD2.EXE /sb` / `TD3.EXE /sb`, New game for flight 1, else a restored slot and
/// "next mission", then Esc at the block's first screen; e2-program-flow §8, e3-program-flow
/// §8, M10-playthrough "Direct starts").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectStart {
    /// E1: no front-end screen changes the state.
    Nothing,
    /// A scene card: its picture, whose palette it loads, and the ink entry and 6-bit colour
    /// it sets (episode-deltas §9.2). E2 cards do not flush the keyboard, so the Esc sent at
    /// the block's first scene card skips from there even after a title card.
    Card {
        file: &'static str,
        ink: Option<(u8, [u8; 3])>,
    },
    /// E3: the restore screen (a copy of the menu's palette buffer: `tdmen.pal` with entries
    /// 224..233 at (16, 0, 0)), then Esc at the block's title card (entry 85 := (63, 0, 0)).
    RestoreThenTitle,
}

/// The E2 scene cards that start a block (episode-deltas §9.2: picture, ink).
const E2_MAR: DirectStart = card("marauder.scr", 255, [63, 5, 5]);
const E2_VEL: DirectStart = card("velsroom.scr", 12, [40, 40, 63]);
const E2_SHO: DirectStart = card("shooting.scr", 28, [14, 14, 63]);
const E2_LAB: DirectStart = card("tdlab.scr", 109, [10, 10, 63]);
const E2_BUN: DirectStart = card("bunker.scr", 197, [40, 40, 63]);
const E2_SEW: DirectStart = card("sewer.scr", 194, [14, 50, 63]);
const E2_TO2: DirectStart = card("tdoffic2.scr", 4, [14, 50, 63]);
const E2_BAR: DirectStart = card("sbar.scr", 90, [48, 38, 63]);
const E2_HYD: DirectStart = card("hyde.scr", 3, [63, 63, 63]);
/// The E3 scene cards that start a block (episode-deltas §9.2 E3 table).
const E3_SH0: DirectStart = card("shuttle0.scr", 255, [63, 63, 14]);
const E3_CMD: DirectStart = card("command.scr", 255, [63, 63, 63]);
const E3_CNV: DirectStart = card("convers.scr", 255, [63, 14, 14]);
const E3_MR3: DirectStart = card("maraud3.scr", 83, [63, 15, 63]);
const E3_KOL: DirectStart = card("kolorvci.scr", 255, [63, 63, 14]);

const fn card(file: &'static str, e: u8, rgb: [u8; 3]) -> DirectStart {
    DirectStart::Card {
        file,
        ink: Some((e, rgb)),
    }
}

/// The last screen of a direct start of flight `flight` of `episode` (weapons §3.3.2 and
/// §3.3.3 test vectors (a); the first screen of each block, M10-playthrough "Direct starts").
///
/// E3 flights 7 and 21 have no block of their own and cannot be started directly (OB-051):
/// ASSUMPTION(U-490): they take the direct start of the block they belong to (flight 7 block
/// 6, flight 21 block 11), as if the block's first screen had been skipped to that flight; no
/// capture exists.
#[must_use]
pub fn direct_start(episode: Episode, flight: u8) -> DirectStart {
    use DirectStart::RestoreThenTitle as Title;
    match episode {
        Episode::E1 => DirectStart::Nothing,
        Episode::E2 => match flight {
            2 => E2_VEL,
            3 | 10 => E2_SHO,
            4 | 8 | 15 => E2_LAB,
            5 | 7 => E2_BUN,
            6 => E2_SEW,
            9 | 13 | 18 | 20 => E2_TO2,
            11 | 12 | 14 => E2_BAR,
            17 => E2_HYD,
            _ => E2_MAR,
        },
        Episode::E3 => match flight {
            1 => E3_SH0,
            2 => E3_CMD,
            3 | 5 => E3_CNV,
            6..=8 => E3_MR3,
            12 | 15 | 19 => E3_KOL,
            _ => Title,
        },
    }
}

/// The muzzle world of a direct start of flight `flight` of `episode`
/// ([`FrontEnd::after_direct_start`], [`muzzle_world`]): `table` is the executable's muzzle
/// table, `read` gives an installation file's bytes (`tdmen.pal`, `td.men`, the card).
///
/// # Errors
/// What `read` returns.
pub fn direct_start_world<E>(
    episode: Episode,
    flight: u8,
    table: &[u8],
    mut read: impl FnMut(&str) -> Result<Vec<u8>, E>,
) -> Result<Vec<u8>, E> {
    let menu = MenuBytes::load(&mut read)?;
    let fe = FrontEnd::after_direct_start(episode, flight, &menu, read)?;
    Ok(muzzle_world(episode, table, &menu, &fe))
}

#[cfg(test)]
mod tests;
