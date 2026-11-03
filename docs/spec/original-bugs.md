# Original Bugs and Quirks

The catalogue of bugs and quirks in the original game and the port's policy for each (MDG §6.3).

Policies: **preserve** (all profiles) · **fix-all** (crash, hang, save corruption, progression blocker) · **fix-enhanced** (preserved in Classic, fixed in Enhanced) · **not-ported** (hostile or obsolete behaviour).

| ID | Description | Where (spec / address) | Effect on play | Policy | Decided by / date |
|---|---|---|---|---|---|
| OB-001 | Apparent host-modifying path: `c:\dos\label.exe` + `PLAY TD2192` strings in `file0001.exe` near the in-flight UI (possibly tied to cheat detection). | `docs/re/EXECUTABLES.md` §5 | Would modify the user's disk label | **not-ported** (MDG NG4) | Design guide, 2026-11-02 |
| OB-002 | Memory checks, sound-card configuration, `nuffmem.exe`. | `td1.exe`, `file0001.exe` | Obsolete | **not-ported** (MDG NG3) | Design guide, 2026-11-02 |
| OB-003 | Full-screen `.scr` scenes, `secret.pic` and `td1-14.pic` are plotted one pixel to the right: screen column 0 stays black and the image's last column is off screen. | [asset-load-sites.md](../re/asset-load-sites.md) §4; `file0001:051A:1E5A` and siblings; C-0002 f1630 | Cosmetic | **preserve** (proposed) | RE-3 proposal, 2026-11-02 (needs a decision) |
| OB-004 | Small-font characters outside the font (upper case, `:`, `'` ...) read glyph bytes from memory before the font block and advance 0 pixels; `!` does the same before its code-drawn shape. | [render/text.md](render/text.md) §5; `051A:088F` | Normally invisible (zero memory); unsupported characters overprint the next one | **preserve** the 0 advance, draw nothing for the stray lookup (proposed) | RE-3 proposal, 2026-11-02 |
| OB-005 | Title font: `:` maps to a glyph that is never loaded (39 of 40 glyphs read), and `.`/`,` also fall through to an out-of-range glyph lookup. | [render/text.md](render/text.md) §5; `051A:042A`, `051A:5852` | Possible stray pixels | **fix-all**: draw nothing for unloaded glyphs (proposed) | RE-3 proposal, 2026-11-02 |
| OB-006 | `td.blk` is loaded only up to 81,664 bytes: the bottom 8 rows of tile 79 are never loaded (unused by every E1 map). | [FORMATS §4](../re/FORMATS.md#4-tileset-blk); `0E8B:B880` | None with shipped maps | **preserve** behaviourally (loading the full tile is equivalent) | RE-3, 2026-11-02 |
| OB-007 | Dialogue: an `02` line whose colour matches neither speaker is drawn in slot 2's colour with slot 2 highlighted (E1; 4 lines). A scene ID that is not in the file makes the reader loop forever. | [narrative/dialogue-player.md](narrative/dialogue-player.md) §5; `051A:92AF` | Wrong colour on 4 lines; hang only with a damaged script | **preserve** the colour rule; **fix-all** the hang (proposed) | RE-3 proposal, 2026-11-02 |
| OB-008 | Mission path blocks: slot 20's path elements 482..500 lie past the file data and come from uninitialised memory (501-byte slot stride). | [FORMATS §7](../re/FORMATS.md#7-missions-mntd-mntd2-mntd3); `0E8B:B880`; U-060 | Unknown until U-060 is answered | open (U-060) | RE-3, 2026-11-02 |
