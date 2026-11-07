//! The ship path arrays as memory on the flight heap, and what the front end leaves in them
//! between two flights: E1 flight 21 loads no mission file and flies those leftovers.
//!
//! Spec: `docs/spec/sim/mission-ships.md` rev 6 §4.1 step 2 and §4.1.1 (U-500; OB-140,
//! preserved), `docs/re/structures.md` §2. Fidelity: T1 (where flight 21's ships start and
//! fly).
//!
//! `Sim_FlyMission` marks the heap top M at entry and allocates its buffers from there in a
//! fixed order, so the two path arrays (`array[1..20, 0..500] of byte`, 10,020 bytes each) sit at
//! M + 9,328 (X) and M + 19,352 (Y) in every flight of a program run. Between two flights the
//! front end reuses that memory from M ([`HeapUse`]): the current song (the whole CMF file at
//! M), the small font's 975-byte block at the song's size rounded up to 8, 16,000-byte portrait
//! blocks after it, and BP7's free-list size words in bytes 4..7 of every freed block.
//!
//! [`PathMemory`] holds the two arrays. The session carries it from flight to flight
//! (`Session::paths`): a flight with a mission file loads the file's arrays into it, every
//! flight's setup copies its ships' paths from it (mission-ships §4.1 steps 3 and 4), and
//! [`PathMemory::apply`] replays the front end's heap uses between two flights (td-flow reports
//! block 19's, between flights 19 and 21).
//!
//! Bytes no writer touched since the program started (element (1, 0), slot 20's steps 482..500
//! of a mission file) are 0, as in a direct start under DOSBox; a mission load writes them as 0
//! too, which is the spec's port value for heap contents from before flight 19 (§4.1.1).

use std::fmt;

use td_formats::mission::{FIXUP_SLOT, Mission, PATH_LEN, PATH_STRIDE, SLOTS};
use td_formats::portraits::RECORD_LEN as PORTRAIT_RECORD_LEN;

/// Offset of path X from the flight's heap mark M (mission-ships §4.1.1).
pub const PATH_X_AT: usize = 9_328;
/// Offset of path Y from M.
pub const PATH_Y_AT: usize = 19_352;
/// Bytes of one path array in memory (20 slots × steps 0..=500).
pub const PATH_ARRAY_LEN: usize = SLOTS * PATH_STRIDE;
/// BP7 allocates in units of 8 bytes.
pub const HEAP_UNIT: usize = 8;
/// The small font's block, `GetMem(975)` rounded up to [`HEAP_UNIT`].
pub const FONT_BLOCK_LEN: usize = 976;
/// The small font file every scene card, dialogue scene and the debrief load into its block.
pub const FONT_FILE: &str = "td.lt2";
/// Bytes of [`FONT_FILE`] read into the block (the file; the block's last 25 bytes stay).
pub const FONT_READ_LEN: usize = 950;
/// A portrait block: the 160 × 100 pixels of one face.
pub const PORTRAIT_LEN: usize = 16_000;
/// The E1 portrait file (`Dlg_LoadPortrait`, dialogue-player §4.4).
pub const PORTRAIT_FILE: &str = "newfaces.td";
/// Portrait pixels at or above this become 0; a right-side portrait is moved up by it
/// (palette entries 112..223, dialogue-player §4.4 step 5).
pub const FACE_COLOURS: u8 = 112;
/// Where a freed block's size word goes: bytes 4..7 of the block (`BP_HeapFree`).
const SIZE_FIELD_AT: usize = 4;

/// The size word BP7 writes into a freed block: the size as a normalised pointer, offset
/// (size mod 16) then paragraphs, both little-endian (976 bytes: `00 00 3D 00`; 16,000 bytes:
/// `00 00 E8 03`).
#[must_use]
pub fn free_size_word(len: usize) -> [u8; 4] {
    let off = (len % 16) as u16;
    let para = (len / 16) as u16;
    let [a, b] = off.to_le_bytes();
    let [c, d] = para.to_le_bytes();
    [a, b, c, d]
}

/// `n` rounded up to a multiple of [`HEAP_UNIT`].
#[must_use]
pub fn heap_round(n: usize) -> usize {
    n.div_ceil(HEAP_UNIT) * HEAP_UNIT
}

/// One portrait a screen loads (dialogue-player §4.4 step 5; the debrief's face 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortraitLoad {
    /// The face record, 1-based (record face − 1 of [`PORTRAIT_FILE`]), after the face-code
    /// remap.
    pub face: u8,
    /// Right side (dialogue slot 2): every pixel is moved up by [`FACE_COLOURS`].
    pub right: bool,
}

/// One use of the flight heap by the front end between two flights, in program order
/// (mission-ships §4.1.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeapUse {
    /// `Snd_PlayMusic` with music on (`g_CfgMusic` = 2): the old song's buffer is freed and the
    /// named CMF file is read whole at M. With music off `Snd_PlayMusic` loads nothing, so the
    /// front end reports no such use (U-540: specified statically only).
    Song(String),
    /// A screen that loads the small font into its block (a scene card, a dialogue scene, the
    /// debrief), loads these portraits one after the other while it runs (each into its own
    /// block, freed at once), and frees the font when it ends.
    Screen(Vec<PortraitLoad>),
}

impl HeapUse {
    /// The debrief (`Flow_Debrief`): the small font and face 1 on the right side
    /// (`051A:70E6`).
    #[must_use]
    pub fn debrief() -> Self {
        HeapUse::Screen(vec![PortraitLoad {
            face: 1,
            right: true,
        }])
    }

    /// A scene card: the small font only.
    #[must_use]
    pub fn card() -> Self {
        HeapUse::Screen(Vec::new())
    }
}

/// Block 19 between flights 19 and 21 when the skip flag is set at the `C:voffice` card
/// (episode-program-flow §4.4 row 19; VE-5's capture C-0200, mission-ships §4.1.1 vector 1):
/// `M8` (`low.cmf`), the debrief, `M5` (`vulture.cmf`), the card. With `music` off the songs
/// load nothing.
#[must_use]
pub fn block_19_to_voffice(music: bool) -> Vec<HeapUse> {
    let mut uses = Vec::new();
    if music {
        uses.push(HeapUse::Song("low.cmf".to_owned()));
    }
    uses.push(HeapUse::debrief());
    if music {
        uses.push(HeapUse::Song("vulture.cmf".to_owned()));
    }
    uses.push(HeapUse::card());
    uses
}

/// The two ship path arrays as they sit on the flight heap (mission-ships §4.1.1): slot `s`
/// (0-based), step `k` (0..=500) at `s × 501 + k`.
#[derive(Clone, PartialEq, Eq)]
pub struct PathMemory {
    x: Box<[u8]>,
    y: Box<[u8]>,
}

impl fmt::Debug for PathMemory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PathMemory")
            .field("slot1_step1", &self.step(0, 1))
            .finish_non_exhaustive()
    }
}

impl Default for PathMemory {
    fn default() -> Self {
        Self::new()
    }
}

impl PathMemory {
    /// The arrays at program start: every byte 0 (fresh memory, as DOSBox gives it).
    #[must_use]
    pub fn new() -> Self {
        Self {
            x: vec![0; PATH_ARRAY_LEN].into_boxed_slice(),
            y: vec![0; PATH_ARRAY_LEN].into_boxed_slice(),
        }
    }

    /// Setup step 2 of a flight with a mission file (mission-ships §4.1, structures §2): the
    /// file's X and Y blocks from element (1, 1) on (bytes the file does not hold are 0), then
    /// every FFh in steps 1..=500 of slot 2 becomes 0.
    pub fn load_mission(&mut self, m: &Mission) {
        for s in 0..SLOTS {
            for k in 0..PATH_STRIDE {
                let i = s * PATH_STRIDE + k;
                self.x[i] = m.path_x.get(s, k).unwrap_or(0);
                self.y[i] = m.path_y.get(s, k).unwrap_or(0);
            }
        }
        for k in 1..=PATH_LEN {
            let i = FIXUP_SLOT * PATH_STRIDE + k;
            for a in [&mut self.x, &mut self.y] {
                if a[i] == 0xFF {
                    a[i] = 0;
                }
            }
        }
    }

    /// Step `k` (0..=500) of slot `slot` (0-based): (tile X, tile Y).
    ///
    /// # Panics
    /// If `slot` ≥ 20 or `k` > 500.
    #[must_use]
    pub fn step(&self, slot: usize, k: usize) -> (u8, u8) {
        assert!(slot < SLOTS && k < PATH_STRIDE, "slot {slot} step {k}");
        let i = slot * PATH_STRIDE + k;
        (self.x[i], self.y[i])
    }

    /// The X array (memory layout, 10,020 bytes).
    #[must_use]
    pub fn x(&self) -> &[u8] {
        &self.x
    }

    /// The Y array (memory layout, 10,020 bytes).
    #[must_use]
    pub fn y(&self) -> &[u8] {
        &self.y
    }

    /// Writes `bytes` at heap offset `at` from M; only the parts that land in the two arrays
    /// are kept.
    pub fn write(&mut self, at: usize, bytes: &[u8]) {
        let end = at + bytes.len();
        for (base, a) in [(PATH_X_AT, &mut self.x), (PATH_Y_AT, &mut self.y)] {
            let (lo, hi) = (at.max(base), end.min(base + PATH_ARRAY_LEN));
            if lo < hi {
                a[lo - base..hi - base].copy_from_slice(&bytes[lo - at..hi - at]);
            }
        }
    }

    /// Replays the front end's heap uses between two flights, in order (mission-ships §4.1.1
    /// writer table). The heap holds no song when a flight has ended (the flight's own song
    /// lies above its buffers and is released with them), so the first screen's font goes to
    /// M unless a song comes first. `read` gives an installation file's bytes: the songs,
    /// [`FONT_FILE`] and [`PORTRAIT_FILE`]. A face beyond the end of the portrait file loads
    /// nothing (none in the shipped scripts).
    ///
    /// # Errors
    /// What `read` returns.
    pub fn apply<E>(
        &mut self,
        uses: &[HeapUse],
        mut read: impl FnMut(&str) -> Result<Vec<u8>, E>,
    ) -> Result<(), E> {
        // m: the size of the song in the buffer at M (0: none).
        let mut song = 0;
        let mut font: Option<Vec<u8>> = None;
        let mut faces: Option<Vec<u8>> = None;
        for u in uses {
            match u {
                HeapUse::Song(name) => {
                    let cmf = read(name)?;
                    self.write(0, &cmf);
                    song = cmf.len();
                }
                HeapUse::Screen(portraits) => {
                    let at = heap_round(song);
                    if font.is_none() {
                        font = Some(read(FONT_FILE)?);
                    }
                    let lt2 = font.as_deref().unwrap_or_default();
                    self.write(at, &lt2[..lt2.len().min(FONT_READ_LEN)]);
                    for p in portraits {
                        if faces.is_none() {
                            faces = Some(read(PORTRAIT_FILE)?);
                        }
                        let file = faces.as_deref().unwrap_or_default();
                        let Some(pixels) = portrait_pixels(file, *p) else {
                            continue;
                        };
                        let block = at + FONT_BLOCK_LEN;
                        self.write(block, &pixels);
                        self.write(block + SIZE_FIELD_AT, &free_size_word(PORTRAIT_LEN));
                    }
                    self.write(at + SIZE_FIELD_AT, &free_size_word(FONT_BLOCK_LEN));
                }
            }
        }
        Ok(())
    }
}

/// A portrait block as `Dlg_LoadPortrait` leaves it (dialogue-player §4.4 step 5): the pixels
/// of record face − 1, each p ≥ 112 replaced by 0, then moved up by 112 on the right side.
fn portrait_pixels(file: &[u8], p: PortraitLoad) -> Option<Vec<u8>> {
    let start = usize::from(p.face.checked_sub(1)?) * PORTRAIT_RECORD_LEN
        + (PORTRAIT_RECORD_LEN - PORTRAIT_LEN);
    let raw = file.get(start..start + PORTRAIT_LEN)?;
    let lift = if p.right { FACE_COLOURS } else { 0 };
    Some(
        raw.iter()
            .map(|&v| if v >= FACE_COLOURS { 0 } else { v } + lift)
            .collect(),
    )
}

#[cfg(test)]
mod tests;
