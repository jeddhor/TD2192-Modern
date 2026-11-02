# Unknowns Register

Every open question about the original game that affects the port. Add new ones at the end with the next ID. When one is resolved, set Status to `RESOLVED`, write the answer (or link to the doc that holds it) and keep the row. IDs are never reused.

Priority: **P1** blocks a milestone's core work · **P2** blocks a specific task · **P3** nice to know.

| ID | Question | Pri | Evidence so far | How to resolve | Milestone / task | Status |
|---|---|---|---|---|---|---|
| U-001 | **What regulates game speed?** VGA retrace (70.086 Hz), a PIT-driven tick, or CPU-bound? What is the canonical tick rate, and what runs per tick vs per frame? | P1 | 286 target; WORX hooks the timer for music | Find `3DAh` polling and PIT programming (ADG §7.6); DOSBox: count main-loop iterations per second at several `cycles` settings | M1-06 | OPEN |
| U-002 | Internal position units: pixels, or sub-pixel fixed point? Width and signedness? | P1 | World is 6,432 px; fits in 16 bits | RAM dumps while moving (ADG §7.9); structure analysis | M1-08, M6-01 | OPEN |
| U-003 | Internal angle representation and resolution (16 sprite headings, but physics may be finer); sine/cosine tables? | P1 | 4 rotation bases × 4 quadrants = 16 frames | Find trig tables (look for monotonic word tables); dumps while turning | M1-08, M6-01 | OPEN |
| U-004 | Contents and lifecycle of `tdstatus.dat` (not shipped) | P3 | Referenced by `td1.exe` and `file0001.exe` | Run in DOSBox; inspect the file; xrefs | M1-10 | OPEN |
| U-005 | How Episodes 2 and 3 are launched and whether anything carries over (kills to date?) | P2 | `td1.exe` names only `file0000`/`file0001`; `td2`/`td3` have their own menus and saves; **no exe references `td2.exe` or `td3.exe`**, so the player probably runs them directly | `td1.exe` and `file0001.exe` analysis; DOSBox experiments | M1-10, M10 | OPEN |
| U-006 | Which map each mission uses: E2 references **both `td.map` and `td2.map`** (and `td.blk` + `td2.blk`); E3 references `td3.map` and `td4.map` | P2 | Filename scan; the two E3 maps start almost identically; E3 visits a mining station and Kolor | Load-site analysis in `td3.exe` | M1-05, M10-02 | OPEN |
| U-007 | **CMF tempo:** which header field WORX honours (0x0A varies 23..192; 0x0C is always 96), or does it use its own rate? | P1 | All 17 headers parsed | WORX analysis + compare DOSBox capture durations with both interpretations | M3-05 | OPEN |
| U-008 | Meaning of mission slot class codes 0..8 (and 0xFF = empty?) and their mapping to `td.shp` sprite slots and ship classes | P1 | Histogram over 60 files: 7 and 8 most common; 1 in truck missions | Mission loader + spawn code; DOSBox observation of m1 | M1-08, M8-01 | OPEN |
| U-009 | Map collision: which tile indices are solid; per-tile or per-pixel; response (stop, slide, bounce, damage) | P1 | 80 tiles; buildings vs roads visually distinct | Collision routine; experiments in DOSBox | M6-01 | OPEN |
| U-010 | What triggers the "cheater" judgement in the debrief, and does anything else happen (see `label.exe`)? | P2 | Cheater rating string; `c:\dos\label.exe` + `PLAY TD2192` strings nearby | Static analysis only. **Do not trigger it in DOSBox with a mounted real drive**; use an isolated mount | M8-09 | OPEN |
| U-011 | What the original saves per slot (`td*.sav` 70 bytes, `td*.stt` names) | P2 | Shipped files are templates | Create saves in DOSBox (on the copy) and diff | M2-12 | OPEN |
| U-012 | Episode 3 mission numbering: files `m1..m19` + `m21`; no "mission iii" or "mission xx" title strings | P2 | File list; string scan | `td3.exe` mission sequencing | M10-02 | OPEN |
| U-013 | Purpose of the 864 trailing bytes in `td2.blk`/`td3.blk` | P3 | Size difference vs `td.blk` | Load-site analysis; render the bytes | M2-03 | OPEN |
| U-014 | Byte at offset 10 = `0x4C` in `m18.td3`'s otherwise-zero reserved header | P3 | Only anomaly in 60 files | Mission loader; does E3M18 behave specially? | M8-01 | OPEN |
| U-015 | First byte of some slots' path arrays (`0xFF`, `0x80`, `0x11`, `0x64`, `0x02`, `0x50`) differs from the path run: flag, delay or start marker? | P2 | Mission file scan | Path playback code | M8-01 | OPEN |
| U-016 | Dialogue format details: the second byte of `02` lines (always `01` so far), the literal `d` line after `04`, any other control codes | P2 | First scenes of `dialogue.dat` | Full-file scan for control bytes + dialogue routine | M2-10 | OPEN |
| U-017 | `td.ltr` font geometry (14,976 bytes) and which screens use it | P3 | Not 8 px wide | Text-drawing routines | M2-08 | OPEN |
| U-018 | `.pic` encoding (compressed or sub-rectangle) | P2 | Data after the palette doesn't divide by 320 | Loader near file offset 0xDD48 in `file0001.exe` | M2-06 | OPEN |
| U-019 | Portrait cell geometry and animation frames in `newfaces.*` | P2 | Render at 320 px is close but misaligned | Portrait blit routine | M2-07 | OPEN |
| U-020 | Dimensions and usage of the raw UI files (`td.men`, `td.win`, `td.cmm`, `td.sel`, `td.sss`, `td.wea`, `td.fac`, `td.dun`, `td.joy`, `td*.ccm`) | P2 | Sizes only | Load sites + blit calls | M1-05, M2-09 | OPEN |
| U-021 | SFX policy: does a new effect preempt the current one (single DSP stream)? Priorities? | P2 | SB 1.x/2.0-class hardware plays one stream | WORX call sites | M1-02, M3-06 | OPEN |
| U-022 | Does gameplay use Real48 (software floating point)? | P1 | Unknown | Look for System Real48 helper calls from game code | M1-01 | OPEN |
| U-023 | Exact Borland Pascal `Random` / `Randomize` implementation and every gameplay call site | P1 | BP7 LCG believed to be ×0x08088405 + 1 | System unit analysis; RandSeed watch in DOSBox | M1-01, M8-03 | OPEN |
| U-024 | Full original key bindings (thrust, brake, fire, missiles, pause) and joystick mapping | P1 | Only Ins/keypad +, Alt, F6, Tab, Esc confirmed from text | INT 9 handler + key-state table; DOSBox experiments | M1-03, M6-01 | OPEN |
| U-025 | Viewport geometry and HUD layout in flight | P1 | Unknown | Captures + drawing code | M6-07 | OPEN |
| U-026 | Music cue mapping (which CMF plays when) | P2 | Music list in `file0001` at 0x13AD2 suggests an index table | WORX call sites | M1-02 | OPEN |
| U-027 | Whether Ghidra's MZ loader splits segments correctly for these binaries (BP7 code/data segments) | P2 | Not yet imported | Inspect the memory map after import; fix by hand | M0-08 | OPEN |
| U-028 | Meaning of the `sound_override == True` string in `file0000.exe` | P3 | String only | xref | M1-10 | OPEN |
