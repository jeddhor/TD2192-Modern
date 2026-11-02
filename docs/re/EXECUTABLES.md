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

**Ghidra image base segment** (fill in during M0-08):

| Exe | Ghidra base segment | Notes |
|---|---|---|
| `td1.exe` | TBD | |
| `file0000.exe` | TBD | |
| `file0001.exe` | TBD | |
| `td2.exe` | TBD | |
| `td3.exe` | TBD | |

## 2. Toolchain fingerprints

- "Portions Copyright (c) 1983,92 Borland" plus the "Runtime error ... at ..." text: **Borland Pascal 7.0** System unit (all five game exes).
- "WORX TOOLKIT VERSION 2.1 COPYRIGHT 1993 BY MYSTIC SOFTWARE": the sound library (td1, file0000, file0001; check td2/td3).
- A `WAVEfmt ` string in `td1.exe` (file offset 0x6AE1): WORX can apparently handle WAV headers as well.
- A lookup table of ascending ASCII-range bytes next to the WORX banner (`" %')+,-../0011..."`): WORX-internal curve.
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
