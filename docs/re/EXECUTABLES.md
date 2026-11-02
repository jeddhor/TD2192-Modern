# Original Executables

Facts about the DOS executables, plus starting anchors for reverse engineering. The address notation is explained in [AGENT_DEVELOPMENT_GUIDE.md §7.8](../AGENT_DEVELOPMENT_GUIDE.md#78-address-notation). Offsets in this file are **file offsets** unless they are marked otherwise.

## 1. MZ header facts

| Exe | File size | MZ image | Header bytes | Relocs | Entry CS:IP | SS:SP | Min alloc | Packed | Overlay |
|---|---:|---:|---:|---:|---|---|---:|---|---|
| `td1.exe` | 33,776 | 33,776 | 544 | 126 | 0000:01FE | 0865:0898 | 3,360 | No | No |
| `file0000.exe` | 47,728 | 47,728 | 1,600 | 391 | 0000:0FC0 | 16A9:4000 | 63,072 | No | No |
| `file0001.exe` | 267,264 | 267,264 | 19,136 | 4,777 | 0000:4526 | 4623:3E80 | 55,152 | No | No |
| `td2.exe` | 242,496 | 242,496 | 16,768 | 4,185 | 0000:4365 | 4088:FA00 | 102,592 | No | No |
| `td3.exe` | 224,128 | 224,128 | 14,544 | 3,629 | 0000:3E8A | 3C96:FA00 | 102,576 | No | No |
| `helpme.exe` | 13,936 | 13,936 | 704 | 167 | 0000:0A89 | 0B4D:1000 | 37,152 | No | No |
| `nuffmem.exe` | 3,216 | 3,216 | 144 | 28 | 0000:00DB | 00E4:0FA0 | 4,576 | No | No |
| `aboutus.exe` | 20,503 | 20,503 | 512 | 10 | 0040:0006 | 0000:0400 | 32 | ? (unusual header) | No |
| `order.exe` | 20,501 | 20,501 | 512 | 10 | 0040:0006 | 0000:0400 | 32 | ? (unusual header) | No |
| `catalog.exe` | 53,026 | 7,605 | 32 | 0 | 01B7:000E | 02B2:0080 | 25,728 | **LZEXE (`LZ91`)** | data appended |
| `order2.com` | 1,718 | — | — | — | — | — | — | .COM | — |

Entry CS values are image-relative (0000 = first paragraph of the load image).

### 1.1 Ghidra image base, segments and DGROUP (M0-08, 2026-11-02)

Ghidra 12.2's MZ loader loads every program at **image base segment `0x1000`**, so **Ghidra address = `(0x1000 + rel_seg):off`** for all five exes (for example `file0001:3A9A:0000` is Ghidra `4A9A:0000`). It creates one block per segment that a relocation targets. For these BP7 binaries those are exactly the unit code segments plus DGROUP (verified against our own relocation-table scan), and every block starts at offset 0. Ghidra sometimes *displays* an address in a 64 KB-normalised form (`1000:77E5` is the same byte as `151A:2645`). Our scripts always convert through the containing block, so their output is image-relative.

`tools/ghidra/SetupBp7Program.java` (M0-09) repeats all the fixes below (see [tools/ghidra/README.md](../../tools/ghidra/README.md)). It has been applied to all five programs in the canonical project.

| Exe | Image base | DGROUP (rel) | DGROUP initialised bytes | SS (rel) = DGROUP + | SP | DGROUP found at |
|---|---|---|---:|---|---|---|
| `file0001.exe` | `1000` | `3C43` | 0x510 | `4623` = DS:9E00 | 3E80 | `file0001:3A9A:0000` (`mov dx,3C43h; mov ds,dx`) |
| `td2.exe` | `1000` | `36B6` | 0x660 | `4088` = DS:9D20 | FA00 | `td2:350D:0000` |
| `td3.exe` | `1000` | `32C5` | 0x660 | `3C96` = DS:9D10 | FA00 | `td3:311C:0000` |
| `file0000.exe` | `1000` | `0B3D` | 0x60 | `16A9` = DS:B6C0 | 4000 | `file0000:09FD:0000` |
| `td1.exe` | `1000` | `0817` | 0x60 | `0865` = DS:04E0 | 0898 | `td1:0717:0000` |

In every exe the entry point is the BP7 main block. It starts with a run of `call far` to each unit's initialisation part (System first), then the main body's own `push bp; mov bp,sp` frame. That run gives the unit order (M1-01):

| Exe | Init chain (image-relative, in call order) | Main body |
|---|---|---|
| `file0001.exe` | `3A9A:0000` (System) · `3A1C:000D` (Crt) · `34E6:3836` · `2A15:220F` · `2404:60FD` · `1EDE:5247` · `0E8B:F9E7` · `051A:96F9` | `0000:454E` |
| `td2.exe` | `350D:0000` · `3499:000D` · `2F63:3836` · `2555:174D` · `2236:31DE` · `1E0B:429F` · `0E96:EBA8` · `04EC:9A93` | `0000:438D` |
| `td3.exe` | `311C:0000` · `30A8:000D` · `2B72:3836` · `2215:0C35` · `1BD8:4122` · `0C5D:EC06` · `04A0:7BC0` | `0000:3EAD` |
| `file0000.exe` | `09FD:0000` · `0993:000D` | `0000:0FCA` |
| `td1.exe` | `0717:0000` · `006B:000D` | `0000:0208` |

Code segments (image-relative start, size in bytes). The unit names are ours (M1-01, [bp7-runtime-semantics](../spec/core/bp7-runtime-semantics.md)):

| `file0001` | `td2` | `td3` | Size (E1/E2/E3) | Unit |
|---|---|---|---|---|
| `0000` | `0000` | `0000` | 51A0 / 4EC0 / 4A00 | Main program |
| `051A` | `04EC` | `04A0` | 9710 / 9AA0 / 7BD0 | Game unit (init) |
| `0E8B` | `0E96` | `0C5D` | F9F0 / EBB0 / EC10 | Game unit (init) |
| `1E2A` | `1D51` | `1B1E` | 950 / 950 / 950 | Unit, no init |
| `1EBF` | `1DE6` | `1BB3` | 1F0 / 250 / 250 | Unit, no init |
| `1EDE` | `1E0B` | `1BD8` | 5260 / 42B0 / 4130 | Game unit (init) |
| `2404` | `2236` | `1FEB` | 6110 / 31F0 / 22A0 | Game unit (init in E1/E2) |
| `2A15` | `2555` | `2215` | 2220 / 1760 / C50 | Unit (init) |
| `2C37` | `26CB` | `22DA` | 65B0 (all) | **WORX Toolkit** (no init; contains `WAVEfmt `, ISR code, tables) |
| `3292` | `2D26` | `2935` | 2540 / 23D0 / 23D0 | Unit, no init |
| `34E6` | `2F63` | `2B72` | 3880 (all) | Engine unit (init at `:3836` in all three) |
| `386E` | `32EB` | `2EFA` | 1AE0 (all) | Engine unit, no init |
| `3A1C` | `3499` | `30A8` | 620 (all) | **Crt** |
| `3A7E` | `34FB` | `310A` | 140 / A0 / A0 | Unit, no init |
| `3A92` | `3505` | `3114` | 80 (all) | Unit, no init |
| `3A9A` | `350D` | `311C` | 1A90 (all) | **System** |

`file0000.exe` segments: `0000` (2FA0), `02FA` (65D0, WORX), `0957` (3C0), `0993` (620, Crt), `09F5` (80), `09FD` (1400, System). `td1.exe`: `0000` (6B0), `006B` (620, Crt), `00CD` (6400, WORX), `070D` (A0), `0717` (1000, System).

**Loader defects found and fixed by `SetupBp7Program` (U-027):**

1. **Truncated last page.** The loader computes the image end as `(e_cp − 1) × 512 + e_cblp`, which is 512 bytes short when `e_cblp = 0`. That is the case only for `file0001.exe` (`e_cp = 522`, `e_cblp = 0`). The last 0x200 bytes of DGROUP's initialised data (`DS:0310..050F`, which holds System's typed constants, including `PrefixSeg` and `RandSeed`) were left uninitialised. The script re-creates them from the file bytes.
2. **DS not set.** The loader sets DS only when the entry instruction is `mov dx,imm`. A BP7 entry is `call far System:init`, so DS was unset. The script sets DS = DGROUP over all code blocks.
3. **DGROUP marked executable** (named `CODE_n`). The script renames it `DGROUP`/`DGROUP_BSS` and clears the execute flag.
4. **False "non-returning" functions.** The analyzer "Non-Returning Functions – Discovered" flagged System's string-assign helper (`td2:350D:0D1E`, `td3:311C:0D1E`) as non-returning and dropped the code after every call to it (about 16 KB of code per exe). The script disables that analyzer, clears the flag on any function that contains a return instruction, and re-disassembles the dropped fall-throughs.
5. **Near branches 64 KB off (Ghidra x86 SLEIGH bug).** The 16-bit `rel16` operand is computed as `((inst_next >> 16) << 16) | ((inst_next + rel) & 0xFFFF)` on the *linear* address, so it wraps at a 64 KB linear boundary instead of at the segment base. In a segment that straddles a 64 KB linear boundary, every near `CALL`/`JMP rel16` that crosses that line resolves 64 KB away from the real `CS:((IP+rel) & 0xFFFF)` target, and Ghidra then disassembles and creates "functions" in whatever lies there (usually string literals). Affected segments: `file0001` `0E8B`, `1EDE`, `2C37`; equivalent segments in `td2`/`td3`. The script replaces each wrong flow reference with a `CALL_OVERRIDE_UNCONDITIONAL`/`JUMP_OVERRIDE_UNCONDITIONAL` reference to the correct target (78 in `file0001`, 115 in `td2`, 54 in `td3`; flow following and the decompiler both honour these). It then removes the bogus functions and clears any orphan run of "instructions" that is at least 70% printable ASCII. **Any new disassembly in those segments needs the same fix, so re-run the script after manual disassembly.**
6. The loader's "adjust segment starts" heuristic (it moves bytes up to a `RETF` in the first 16 bytes of a block into the previous block) did **not** fire for any of the five exes: every block starts at offset 0. Re-check this if an exe is ever re-imported.

**Coverage after setup** (instructions as a share of all code-segment bytes; the rest is classified by the script):

| Exe | Functions | Instruction bytes | Code-segment bytes | % instructions | Pascal-string bytes | Zero fill | Other (tables, unreached) |
|---|---:|---:|---:|---:|---:|---:|---:|
| `file0001.exe` | 391 | 199,947 | 246,832 | 81.0% | 25,682 | 5,024 | 15,930 (6.5%) |
| `td2.exe` | 400 | 178,810 | 224,096 | 79.8% | 19,594 | 5,191 | 20,141 (9.0%) |
| `td3.exe` | 387 | 168,751 | 207,952 | 81.1% | 16,782 | 5,127 | 16,941 (8.1%) |
| `file0000.exe` | 175 | 23,248 | 46,032 | 50.5% | 1,375 | 5,022 | 16,127 (35.0%) |
| `td1.exe` | 141 | 11,479 | 33,136 | 34.6% | 577 | 4,939 | 15,801 (47.7%) |

Before setup, `file0001` had 340 functions and 196,388 instruction bytes, but some of those were string literals decoded as code (defect 5).

In `file0001` the remaining "other" bytes are almost all in WORX (`2C37`: an 11 KB run of tables and zeroed buffers starting at `2C37:0410`, plus some handler code that is still unreached) and System (`3A9A:14C3..`: Real48/8087-emulator constant tables).

Function counts are low for the code size because much of the game code is written as a few very large procedures. In `file0001` the largest are `0E8B:B880` (16.7 KB, 72 distinct callees), `1EDE:0E6A` (16.0 KB), `0E8B:0984` (14.1 KB), `2404:3521` (11.2 KB) and `3292:0011` (9.5 KB). `file0000`/`td1` were not studied further: their unexplained bytes are mostly WORX tables.

## 2. Toolchain fingerprints

- "Portions Copyright (c) 1983,92 Borland" plus the "Runtime error ... at ..." text: **Borland Pascal 7.0** System unit (all five game exes).
- "WORX TOOLKIT VERSION 2.1 COPYRIGHT 1993 BY MYSTIC SOFTWARE": the sound library (td1, file0000, file0001; check td2/td3).
- A `WAVEfmt ` string in `td1.exe` (file offset 0x6AE1): WORX can apparently handle WAV headers as well.
- A lookup table of ascending ASCII-range bytes next to the WORX banner (`" %')+,-../0011..."`): WORX-internal curve.
- **Prologue encodings** (`file0001`): Pascal-compiled procedures use `55 89 E5` (`push bp; mov bp,sp` with the 89 encoding; 159 sites) or, in `{$G+}` units, `C8 nn nn 00` (`enter`; 54 sites, mostly in `0E8B` and `386E`). The MASM/TASM-style `55 8B EC` appears only in hand-written assembler (24 in System, 7 in `3A7E`, 1 in Crt). The game was therefore compiled at least partly with `{$G+}` (286 instructions).
- Pascal ShortStrings: game text is length-prefixed. When strings are dumped naively, the length byte shows up as a leading printable character (for example `4just seconds away...`, where `4` = 0x34 = 52 characters).

## 3. Launcher `td1.exe`

String anchors:

| Offset | Content |
|---|---|
| 0x224 | Command-line help (`/pc`, `/a`, `/sb`, `/nosound`, `/config`) |
| 0x39F | `nuffmem.exe` |
| 0x3AC | `/nosound`, `/NOSOUND` |
| 0x3C6 | `file0000.exe` |
| 0x3D3 | `intro.cmf` |
| 0x3DD | `tdstatus.dat` |
| 0x3EA | `file0001.exe` |
| 0x3F7 | `td.cfg` |
| 0x3FE | `order.exe`, `aboutus.exe`, `order2.com` |

Questions: does it chain to `td2.exe`/`td3.exe` (U-005)? What does `tdstatus.dat` carry (U-004)? Note that `file0001.exe` also contains the config menu and the `/m` memory-override message ("td1.exe /m"), so the switches may be forwarded.

## 4. Intro `file0000.exe`

| Offset | Content |
|---|---|
| 0x11A3 | `sound_override == True` (debug/assert-style string) |
| 0x11DD | `intro.map` |
| 0x11F5 | `whoosh.voc`, `pulse.voc`, `scratch.voc` |
| 0x1228 | `intro.CMF`, `intro1.CMF`, `intro2.CMF` |
| 0x1248 | `intro0.scr` |
| 0x1253 | Opening narration (date, planet, the pilot flying home) |
| 0x135E | "safari software presents", production and title lines |
| 0x13B6 | Credits block (programming, graphics, music, storyline, special thanks, directed by) |
| 0x1506 | `intro1.scr` + narration |
| 0x1579 | `intro2.scr` + narration |

## 5. Episode 1 `file0001.exe` (primary target)

| Offset | Content | Relevance |
|---|---|---|
| 0x4F39–0x59B0 | Mission titles interleaved with story cards (locations, relative times), episode-end lines | Narrative sequencing (M1-04, M9-18) |
| 0x793F, 0x7E38 | `tdmen.pal` | Menu palette loads |
| 0x7E2B, 0x8D27 | `tdstatus.dat` | Inter-process state |
| 0x8D0D | `/nosound`, `/NOSOUND` | Switch parsing |
| 0x8D37 | Memory warning text, `/m` hint | Not ported |
| 0x8EDF | `/config`; "NOT ENOUGH MEMORY FOR SOUND EFFECTS" | |
| 0x8F0B–0x8FBF | Config menu labels (music, SFX, controller, "FIRE UP THE GAME!") | `td.cfg` semantics |
| 0x8FDA | `dialogue.dat` | Script choice |
| 0xB26D | `newfaces.td` | Portraits |
| 0xBAB0… | `hangar.scr`, `vlab.scr`, `intro2.scr`, `shooting.scr`, `secret.pic` | Scene loads (one routine per scene?) |
| 0xC2AD | First-mission tutorial text (Stiletto, afterburner keys, guidance-line colours, Esc help, F6 comms) | Controls and HUD semantics |
| 0xCD60–0xE9BE | Scene file names (`vhang`, `marauder`, `briefing`, `tdlab`, `velsroom`, `helihangar`, `eggbase`, `td1-1x.pic`, `soffice`, `sbar`) | Scene catalogue |
| 0xEBB3 | `pic4.td` | |
| 0xED83 | `td-let.cin` + script-choice warning | M5-07 |
| 0xF1C2 | `td.lt2` + missing-dialogue fallback text | M5-07 |
| 0xFB3F | `javelin.td` | |
| 0xFC20 | `city.td` | |
| 0xFDD2 | Assignment printout (orders format) | Dossier/briefing screen |
| 0x101A1–0x102A3 | Dossier fields for an enemy ace | Dossier screen |
| 0x1047E | `td.pal` + retry taunts | Retry prompt |
| 0x108D7–0x10C3E | Debriefing labels, 12 rating tiers + cheater tier, "computer file's note" | Scoring (M8-09) |
| 0x1108C | Player dossier fields | |
| 0x11318, 0x11AF6 | Save/load labels; `td.sss` | Saves |
| 0x126E8 | `td.lt2` + background story | Story intro screen |
| 0x12EAC | `dialogue.dat`, `safedial.dat`, dialogue paging prompt | Dialogue player |
| 0x13AD2–0x13B55 | Music list: `td01`, `td02`, `hangar`, `satair`, `vulture`, `mood`, `menu`, `low`, `climax`, `mood2`, `mood3`, `moodmaj`, `death`, `td03` (`.CMF`) | **Music index table** (M1-02) |
| 0x18560, 0x1866D | `td1.ccm` | Comm graphics |
| 0x18781 | "hit space", "end transmission... hit f6" | Comm viewer |
| 0x18D91 | Fixed-width in-flight objective messages | Mission logic (M8) |
| 0x1A4D9–0x1A6D0 | Ship class stat strings (class, armament, armour/shields, top speed) | Ship classes (M7-01) |
| 0x1B29F | Jurisdiction warnings | M6-06 |
| 0x1C9BE | Performance warning; ship status panel (class, registry, pilot) | HUD, ship switching |
| 0x1CACC | `td1-17.pic` + mission-specific alerts (hangar doors sealed, ion defence screen down, ...) | Late-E1 missions |
| 0x1DA59 | "hit tab to change ships", "hit -esc- to return" | Ship switching UI |
| 0x1E262 | `td.lt2` + joystick calibration prompt | Not ported (replaced) |
| 0x1EB45 | `mine.td`, `lander.td` | Sprites |
| 0x1EB71 | `td.win` + IR prompt + pause prompt | HUD |
| 0x1EBC0 | `c:\dos\label.exe`, `velasquez`, `label.exe`, `PLAY TD2192` | **Host-modifying path: document, never port** |

## 6. Episode 2 `td2.exe`

| Offset | Content |
|---|---|
| 0x4385 | `td.hlp` |
| 0x4590–0x4D00 | E2 mission titles + story cards |
| 0x7030 | `td.men`, `tdmen.pal` |
| 0x84D9 | `dialogu2.dat` |
| 0xA7A1 | `newfaces.td2` |
| 0xAD79–0xF154 | Scene list (each preceded by `td.lt2`): `hangar`, `hangar2`, `tdlab`, `marauder`, `briefing`, `helihangar`, `intro2`, `shooting`, `dieseche`, `soffice`, `sewer`, `shuttle`, `bunker`, `elevator`, `greenblo`, `vidcom`, `bb`, `v-port`, `i-port`, `hydedead`, `aboutus`, `order`, `tdoffic2`, `sbar`, `velsroom`, `td-let.cin`, `hyde` |

## 7. Episode 3 `td3.exe`

| Offset | Content |
|---|---|
| 0x3AD5 | `td.hlp` |
| 0x3D75–0x4410 | E3 mission titles + story cards (no "mission iii" or "mission xx" title found) |
| 0x62A5 | `td.men`, `tdmen.pal` |
| 0x774E | `dialogu3.dat` |
| 0x9A51 | `newfaces.td3` |
| 0xA029–0xC761 | Scene list: `aboutus`, `order`, `tdlab`, `marauder`, `maraud2`, `maraud3`, `vidcom`, `vhang`, `intro2`, `shuttle0`, `vlab`, `convers`, `sel-pain`, `melting`, `kolorvci`, `command`, `mining1`, `ionstat`, `mining2` |
| 0xC929–0xCF25 | `td-let.cin`, `javelin.td`, `city.td` |
| 0xD004 | `td.pal` + retry taunts |
| 0xD6DE | `td3.stt`, `td3.sav`, `td.sss` |
| 0xF281 | Background story (dated 2193) |

## 8. Cross-episode notes

- E2 and E3 reference `td.men`/`tdmen.pal` and `td.hlp` near the start of their string areas, and E1 references `tdmen.pal`. Menu structure may differ between E1 and E2/E3 (U-005).
- E2 and E3 include `aboutus.scr`/`order.scr` in their scene lists: the in-game "about"/"order" screens are probably shown from the menu.
- Every scene name in E2/E3 is preceded by `td.lt2`, which suggests a common "show scene with text" routine that takes (font, background) pairs.
