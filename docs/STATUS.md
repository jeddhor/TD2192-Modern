# Project Status Board

The single source of truth for **who is doing what**. Protocol: [AGENT_DEVELOPMENT_GUIDE.md §6](AGENT_DEVELOPMENT_GUIDE.md#6-task-lifecycle) and [§14](AGENT_DEVELOPMENT_GUIDE.md#14-multi-agent-coordination). Task definitions, deliverables and exit criteria are in [MASTER_DESIGN_GUIDE.md §12](MASTER_DESIGN_GUIDE.md#12-milestones).

**Status values:** `TODO` · `IN PROGRESS` · `PAUSED` (link a handoff) · `BLOCKED` (say on what) · `REVIEW` · `DONE` (date + commit/PR)
**Hats:** RE Archaeologist · SP Spec author · IM Implementer · VE Verifier · CO Coordinator. A task may need several sessions (for example RE/SP, then IM, then VE), each in a separate session.

---

## Current state

| | |
|---|---|
| **Current milestone** | M0 (in progress); M2 started in parallel |
| **Last updated** | 2026-11-02 |
| **Ghidra lock** | free (RE-2 applied the merged file0001.csv, 448 rows incl. RE-3 and RE-4, to the canonical project on 2026-11-02: 269 functions, 170 data, 9 labels, 0 skipped) |
| **Active handoffs** | [2026-11-02-M1-06](handoffs/2026-11-02-M1-06.md) (timing: VE measurements) · [2026-11-02-M6-01](handoffs/2026-11-02-M6-01.md) (flight-core specs: flight model drafted) |

### Blocking user decisions (MDG §14)

| ID | Decision | Blocks | Status |
|---|---|---|---|
| D1 | Language and stack | M0-01 and everything after it | **DECIDED 2026-11-02: Rust + SDL3** |
| D2 | Project licence | M0-03, M0-12 | **DECIDED 2026-11-02: MIT** |
| D3 | Import original saves? | M11-06 | OPEN |
| D4 | TFP extras? | M11-07 | OPEN |
| D5 | Enhanced features in 1.0 | M12-04, M12-05 | OPEN |
| D6 | Platforms and signing | M13-01 | OPEN |
| D7 | Repo hosting and visibility | Running CI remotely (the workflow file is written regardless) | OPEN |
| D8 | Default script variant | M5-07 | OPEN (recommend: ask, as the original does) |
| D9 | Cheater judgement text | M8-09 | OPEN (recommend: text only) |
| D10 | Unknown exe versions | M2-14 | OPEN |

### Done so far (pre-M0)

- 2026-11-02: repository created; MDG, ADG, AGENTS.md, RE knowledge base (inventory with SHA-256 for 218 files, formats, executables, unknowns), spec template and draft ADRs written from an initial survey of the original files.

---

## M0: Foundation

| ID | Task | Hat | Depends on | Status | Owner | Notes |
|---|---|---|---|---|---|---|
| M0-01 | Cargo workspace + crate stubs, toolchain pin, lints, profiles | IM | D1 | DONE | main-session | 2026-11-02; Rust 1.95.0 pinned; SDL3 built from source via VS-bundled CMake (machine-local P:\msdos\TD2192\.cargo\config.toml) |
| M0-02 | CI workflow (fmt, clippy, test; win/linux/mac) | IM | M0-01 | REVIEW | main-session | Workflow written; cannot run until a remote exists (D7) |
| M0-03 | THIRD_PARTY.md, CONTRIBUTING.md, repo hygiene files | IM | M0-01 | DONE | main-session | 2026-11-02 |
| M0-04 | td-assets locator (precedence, 8.3 + case-insensitive) | IM | M0-01 | DONE | main-session | 2026-11-02; unit-tested incl. helihangar.scr |
| M0-05 | tdtool gen-manifest + inventory | IM | M0-04 | DONE | main-session | 2026-11-02; inventory reports 218/218, E1-E3 exact |
| M0-06 | td-test-support (require_data!, assert_sha256!) | IM | M0-01 | DONE | main-session | 2026-11-02 |
| M0-07 | RE work area + captures manifest template | RE | - | DONE | main-session | 2026-11-02 |
| M0-08 | Ghidra project: import + analyse 5 exes; record image bases | RE | M0-07 | DONE | RE-1 | 2026-11-02; base seg 1000 for all; DGROUP + segment maps + coverage in docs/re/EXECUTABLES.md §1.1; loader defects fixed by tools/ghidra/SetupBp7Program.java; U-027 resolved |
| M0-09 | Ghidra scripts (index, strings, symbols in/out, decompile export, port IO) + headless wrapper | RE | M0-08 | DONE | RE-1 | 2026-11-02; tools/ghidra/*.java + tools/scripts/ghidra-headless.ps1; usage in tools/ghidra/README.md; all tested headless |
| M0-10 | DOSBox reference setup on a writable copy | VE | M0-07 | DONE | VE-1 | 2026-11-02; oracle rig deterministic (C-0002=C-0003); patch committed on dosbox-x branch td2192-oracle; re-work/dosbox/ORACLE.md |
| M0-11 | Capture tooling and procedure | VE | M0-10 | DONE | VE-1 | 2026-11-02; oracle-run.ps1, capture-add.ps1, oracle-fb2png.py, oracle-summary.py; ADG 11; captures C-0001..C-0006; docs/re/dynamic-observations.md |
| M0-12 | STATUS populated; ADRs accepted by user | CO | D1, D2 | DONE | main-session | 2026-11-02; D1 Rust+SDL3, D2 MIT; ADRs accepted |

## M1: Executable archaeology

| ID | Task | Hat | Depends on | Status | Owner | Notes |
|---|---|---|---|---|---|---|
| M1-01 | BP7 runtime identification; Random/Real48 semantics spec | RE | M0-08 | DONE | RE-1 | 2026-11-02; docs/spec/core/bp7-runtime-semantics.md rev 1 + docs/re/symbols/file0001.csv (162 rows); RNG exact; Real48 used by gameplay (U-022, U-023 resolved); Real48 algorithms split to M1-01.b |
| M1-01.b | Real48 bit-exact algorithms spec (add/mul/div rounding, Sqrt/Sin/Cos/ArcTan) + vectors from the original routines (U-029) | RE/SP | M1-01 | DONE | RE-4 | 2026-11-02; docs/spec/core/real48.md rev 1 + real48-vectors.csv (1,705 rows from the original); model = original on 247,705 inputs; U-029 resolved |
| M1-02 | WORX API + call-site catalogue (music cues, SFX policy draft) | RE | M1-01 | DONE | RE-4 | 2026-11-02; docs/re/worx.md, docs/spec/audio/music-cues.md rev 1, sfx-policy.md rev 1; U-021, U-026, U-030, U-036 resolved; U-080..U-083 added |
| M1-03 | Hardware access catalogue | RE | M0-09 | DONE | RE-1 | 2026-11-02; docs/re/hardware-access.md (466 sites, every one listed by device); vectors hooked: INT 08h/63h (WORX), 09h (game, per mission), 1Ch (SFX unit) |
| M1-04 | Episode program top-level flow spec | RE/SP | M1-01 | REVIEW | RE-2 | docs/spec/flow/episode-program-flow.md rev 1 (start-up, menu, all 20 E1 mission blocks + flight 21, endings, debrief tiers, tdstatus.dat). Static; mission-1 path matches C-0002 |
| M1-05 | Asset load-site table | RE | M1-01 | DONE | RE-3 | 2026-11-02: docs/re/asset-load-sites.md (all E1 load sites, VGA layout); FORMATS 3/4/6.3/7/9-13 updated (path stride 501, td.men regions, stills confirmed); E2/E3 map/tileset loaders |
| M1-06 | Timing model spec (U-001) | RE/SP | M1-03, M0-10 | PAUSED | RE-1 | Static draft done 2026-11-02: docs/spec/core/timing.md rev 1 (tick = BIOS tick via DOS-time busy-wait at 0E8B:E05C; 18.2/18.5/19.2 Hz by WORX PIT state). Needs VE measurements: docs/handoffs/2026-11-02-M1-06.md |
| M1-07 | Global data map + symbols | RE | M1-01 | REVIEW | RE-2 | docs/re/globals.md (front end, input + flight keys, cheat chords, player/view state, slot/projectile/explosion arrays, buffers) + 158 symbol rows (functions and data; ApplySymbols-checked on a private copy) |
| M1-08 | Core record layouts (ship, projectile, mission, class table) | RE | M1-07 | REVIEW | RE-2 | docs/re/structures.md: units (tile+pixel), mission file load map with 501-byte path stride (FORMATS 7 corrected; M2-11 parser needs the fix), ship record 1065 B, slot/projectile/explosion arrays, class table (U-008), heading codes (U-003), tile attributes (U-009) |
| M1-09 | Cross-episode Version Tracking + deltas | RE | M1-07 | TODO | | |
| M1-10 | Launcher + intro flow specs | RE/SP | M1-01 | TODO | | |
| M1-11 | Exe resource maps (all text/tables) | RE | M1-04 | REVIEW | RE-3 | 2026-11-02: file0001.toml 82 resources (all mission titles/cards, ending, objectives, flight messages, target info, 9 comms, debrief labels/tiers, retry, dossiers, hints, background story, script-choice notices, ship library, prompts); new file0000/td2/td3 maps (4/71/62) to register in td-assets MAPS; tdtool exe-res green for file0001 |
| M1-12 | Unknowns triage | RE | M1-01..M1-11 | TODO | | |

## M2: Static assets

| ID | Task | Hat | Depends on | Status | Owner | Notes |
|---|---|---|---|---|---|---|
| M2-01 | palette parser + tests | IM | M0-06 | DONE | main-session | 2026-11-02 |
| M2-02 | screen parser (46 files) + tests | IM | M2-01 | DONE | main-session | 2026-11-02; 46 screens + td.hlp (2 pages) parse |
| M2-03 | tileset parser (+ E2/E3 trailing bytes) | IM | M2-01 | IN PROGRESS | main-session | Parser done; 864 trailing bytes still unknown (U-013) |
| M2-04 | map parser (201/200) + tdtool map | IM | M2-03 | IN PROGRESS | main-session | Parser done; tdtool map pending |
| M2-05 | sprite sheet + rotation sets + 16-heading assembly | RE/IM | M2-01 | DONE | main-session | 2026-11-02; rotation sets CCW (measured); 16-heading assembly in td-render; tdtool export sprites |
| M2-06 | .pic decoder | RE/IM | M1-05 | DONE | main-session | 2026-11-02; data analysis: raw sub-rectangles, 8 with palette, 2 without (td1-17/19); sizes LIKELY pending M1-05 load sites |
| M2-07 | portraits geometry + parser | RE/IM | M1-05 | DONE | main-session | 2026-11-02; 26/27 x (768 pal + 160x100); per-face palettes; special face codes U-019 |
| M2-08 | fonts x3 | RE/IM | M1-05 | REVIEW | RE-3 (RE part) | 2026-11-02 RE done: spec/render/text.md rev 1 (td.lt2 is the dialogue font, advance table, code-drawn . , " ? - !, title font map, typed text; 3 oracle frames 0-diff). IM follow-up: align td-formats char maps + td-render with the spec |
| M2-09 | raw UI graphics dimensions + parsers | RE/IM | M1-05 | REVIEW | RE-3 (RE part) | 2026-11-02 RE done: FORMATS 12/12.1 (td.men 5 regions + blit sites, td.cmm 256x128 panel, td.wea 6x36x18, td.win rows 160-191, td.sss, td.h2; td.sel unused), spec/render/hud.md draft (display geometry, radar, weapon icon, F6 indicator, ticker). IM follow-up: parsers/CELL_SHEETS; gauges still RE (M6-07) |
| M2-10 | dialogue parser (full grammar) | RE/IM | M2-01 | IN PROGRESS | main-session | Strict parser handles all 6 files (scene IDs, scene-13 CR quirk, mid-scene speakers, portrait-only changes, trailing spaces); field meanings open (U-016) |
| M2-11 | mission parser + dump + path overlay | RE/IM | M1-08 | DONE | main-session | Parser per FORMATS §7 rev 2 / structures §2: paths stored as the in-memory 20x501 array (stride 501, file image from element (1,1)), slot-2 FFh->0 loader fixup in Mission::path; header bytes A/B/D kept (U-038). L1 on 60 files + m1 slot 1/2 start check; tdtool mission-map paths now follow roads |
| M2-12 | cfg / sav / stt parsers | RE/IM | M0-10 | TODO | | |
| M2-13 | mz + pstring utilities | IM | M0-01 | DONE | main-session | 2026-11-02; MzHeader, file_offset, pstring/pstring_seq; headers checked vs EXECUTABLES.md |
| M2-14 | exe-resource extraction + tdtool exe-res | IM | M2-13, M1-11 | IN PROGRESS | main-session | Mechanism done (td-assets::exe_res, compiled-in TOML maps, hash-gated; tdtool exe-res); maps to be filled by M1-11 |
| M2-15 | tdtool export (all formats, outside repo) | IM | M2-02..M2-12 | IN PROGRESS | main-session | tdtool export screens/tiles/sprites/maps done (refuses repo paths) |
| M2-16 | Visual verification; FORMATS to CONFIRMED | VE | M2-15 | TODO | | |

## M3: Audio

| ID | Task | Hat | Depends on | Status | Owner | Notes |
|---|---|---|---|---|---|---|
| M3-01 | VOC parser | IM | M0-06 | DONE | main-session | 2026-11-02; td-formats::voc; all 17 files parse |
| M3-02 | Creative ADPCM decoders (4/2.6/2-bit) | SP/IM | M3-01 | DONE | main-session | 2026-11-02; spec audio/voc-and-adpcm.md; decoder + vectors; decoded signals sane (lag-1 autocorr 0.79-1.00); A/B pending M3-08 |
| M3-03 | td-opl (ymfm wrapper) | IM | M0-01 | DONE | main-session | 2026-11-02; td-opl over ymfm-sys (YM3812) |
| M3-04 | CMF parser + sequencer | IM | M3-03 | DONE | main-session | 2026-11-02; spec audio/cmf.md; fixed channel map, rhythm mode, fine transpose; velocity ignored (U-034); 96 ticks/s (U-007) |
| M3-05 | CMF tempo semantics in WORX (U-007) | RE | M1-02 | DONE | RE-4 | 2026-11-02; docs/spec/audio/cmf.md rev 2: WORX player exact (emulated driver = C-0001 capture 4,364/4,364 writes; model = driver on 17 songs, 146,595 writes); U-007, U-034, U-035 resolved; timing.md rev 2 |
| M3-06 | Mixer + SFX policy | SP/IM | M1-02, M3-02 | IN PROGRESS | main-session | Mixer + Classic/Polyphonic SFX policies done; Classic rule provisional (U-021), balance untuned |
| M3-07 | tdtool play / export audio | IM | M3-04, M3-06 | IN PROGRESS | main-session | tdtool export audio (WAV) done; live play arrives with M4 platform |
| M3-08 | Audio parity vs DOSBox | VE | M3-05, M3-07 | IN PROGRESS | main-session | docs/parity/M3-audio.md: tempo, pitch table, 0xBD PASS vs DRO C-0001; voice allocation + velocity FAIL pending M3-05 |

## M4: Platform shell

| ID | Task | Hat | Depends on | Status | Owner | Notes |
|---|---|---|---|---|---|---|
| M4-01 | Window, texture, palette LUT, scaling modes, F11, high-DPI | IM | M2-02 | DONE | main-session | 2026-11-02; aspect 4:3 sharp-bilinear / integer / stretch; F10 cycles, F11 fullscreen; verified by window read-back (1280x960) |
| M4-02 | Fixed-timestep driver | IM | M1-06 | DONE | main-session | 2026-11-02; td-flow::FixedStep, no drift over 600 s; tick rate placeholder 70 Hz (U-001) |
| M4-03 | Input layer (scancodes, gamepad, actions, text) | IM | M4-01 | IN PROGRESS | main-session | Keyboard (HID scancodes, edges, repeat, text) done; gamepad pending (fold into M11-04 action layer) |
| M4-04 | Audio device + ring buffer | IM | M3-06 | DONE | main-session | 2026-11-02; main-thread render + push to SDL stream (~43 ms queue); OPL chip is not Send, so no audio thread |
| M4-05 | Settings + first-run data picker + validation UI | IM | M0-05 | REVIEW | main-session | settings.toml + first-run folder picker (SDL dialog, validates E1, saves data_dir); picker not yet exercised interactively - needs a manual check |
| M4-06 | Dev map viewer | IM | M2-04, M2-05 | DONE | main-session | 2026-11-02; td2192 --dev-mapview e1/e2a/e2b/e3a/e3b/intro; sprites at 16 headings; music/SFX audition |
| M4-07 | Screenshot, logging, fatal-error dialog | IM | M4-01 | DONE | main-session | 2026-11-02; F12 framebuffer PNG, --dev-exit-after window capture, tracing (TD2192_LOG), fatal error box |

## M5: Narrative & screens

| ID | Task | Hat | Depends on | Status | Owner | Notes |
|---|---|---|---|---|---|---|
| M5-01 | Text renderer per font | SP/IM | M2-08 | IN PROGRESS | main-session | td-render::small_text: proportional td.lt2 + observed punctuation, PIXEL-EXACT vs oracle dialogue frame (L5 test); other fonts' layout pending |
| M5-02 | Palette fade engine | SP/IM | M1-03 | IN PROGRESS | main-session | td-render::Fade (6-bit linear, provisional) pending DAC-routine RE |
| M5-03 | Story card player | SP/IM | M1-04, M1-11 | TODO | | |
| M5-04 | Dialogue scene player | SP/IM | M2-07, M2-10 | IN PROGRESS | main-session | td-render::dialogue: portraits (+112 offset slot 2), palette ranges, listener dimming v>>1, name labels with shadow; PIXEL-EXACT vs oracle f2283 (L5). Line reveal/scroll/paging pending RE-3 spec |
| M5-05 | Dossier screens | SP/IM | M1-11 | TODO | | |
| M5-06 | Help screens | SP/IM | M2-09 | TODO | | |
| M5-07 | Script-choice + missing-dialogue screens | SP/IM | M5-01 | TODO | | |
| M5-08 | Intro sequence | SP/IM | M1-10 | TODO | | |
| M5-09 | Narrative parity | VE | M5-01..M5-08 | TODO | | |

## M6: Flight core

| ID | Task | Hat | Depends on | Status | Owner | Notes |
|---|---|---|---|---|---|---|
| M6-01 | Specs: world, units, flight model, map collision, camera, viewport | SP | M1-06, M1-08 | IN PROGRESS | RE-2 | flight-model rev 2 CONFIRMED by C-0007/C-0008 (215 ticks); weapons.md (U-041/U-043), mission-ships.md (partial, U-039/U-045), camera-viewport.md, collision.md (index) drafted. Handoff: docs/handoffs/2026-11-02-M6-01.md |
| M6-02 | td-sim skeleton (tick, input, BpRandom, state_hash, record/replay) | IM | M6-01 | IN PROGRESS | main-session | BpRandom + BIT-EXACT Real48 (all ops incl. Sqrt/Sin/Cos/ArcTan; 1705/1705 vectors from the original) + state_hash + replay; tick/world skeleton waits for M1-08/M6-01 |
| M6-03 | Player flight model | IM | M6-02 | TODO | | |
| M6-04 | Map collision | IM | M6-02 | TODO | | |
| M6-05 | Camera, viewport, heading sprites | IM | M6-02, M4-06 | TODO | | |
| M6-06 | Jurisdiction, IR toggle, pause | IM | M6-03 | TODO | | |
| M6-07 | HUD frame + instruments | IM | M6-05 | TODO | | |
| M6-08 | Dev arena | IM | M6-03 | TODO | | |
| M6-09 | Flight parity + feel sign-off | VE | M6-03..M6-07 | TODO | | |

## M7: Combat & entities

| ID | Task | Hat | Depends on | Status | Owner | Notes |
|---|---|---|---|---|---|---|
| M7-01 | Ship-class table spec (all episodes) | RE/SP | M1-08 | TODO | | |
| M7-02 | Weapons spec | RE/SP | M1-08 | TODO | | |
| M7-03 | Damage model spec | RE/SP | M1-08 | TODO | | |
| M7-04 | Entity arenas | IM | M6-02 | TODO | | |
| M7-05 | Weapons + projectiles | IM | M7-02, M7-04 | TODO | | |
| M7-06 | Damage, destruction, effects, SFX events | IM | M7-03, M7-05 | TODO | | |
| M7-07 | Arena hostiles | IM | M7-05 | TODO | | |
| M7-08 | Combat parity | VE | M7-06 | TODO | | |

## M8: AI & mission runtime

| ID | Task | Hat | Depends on | Status | Owner | Notes |
|---|---|---|---|---|---|---|
| M8-01 | Mission file semantics spec | RE/SP | M2-11 | TODO | | |
| M8-02 | Mission logic architecture + primitives spec | RE/SP | M1-04 | TODO | | |
| M8-03 | AI behaviours spec | RE/SP | M1-08 | TODO | | |
| M8-04 | Path playback + AI | IM | M8-01, M8-03, M7-06 | TODO | | |
| M8-05 | Mission runtime + MissionScript API | IM | M8-02 | TODO | | |
| M8-06 | Guidance line | IM | M8-05 | TODO | | |
| M8-07 | Objective messages, comms, jurisdiction warnings | IM | M8-05, M2-14 | TODO | | |
| M8-08 | Ship switching, wingmen, protected targets | IM | M8-05 | TODO | | |
| M8-09 | Debrief computation + retry flow | IM | M8-05 | TODO | | |
| M8-10 | E1M01-E1M03 specs + scripts | SP/IM | M8-04..M8-09 | TODO | | |
| M8-11 | E1M01-E1M03 parity | VE | M8-10 | TODO | | |

## M9: Episode 1 campaign

| ID | Task | Hat | Depends on | Status | Owner | Notes |
|---|---|---|---|---|---|---|
| M9-01 | E1M04: spec + script + success replay | RE/SP, IM, VE | M8-11 | TODO | | |
| M9-02 | E1M05: spec + script + success replay | RE/SP, IM, VE | M8-11 | TODO | | |
| M9-03 | E1M06: spec + script + success replay | RE/SP, IM, VE | M8-11 | TODO | | |
| M9-04 | E1M07: spec + script + success replay | RE/SP, IM, VE | M8-11 | TODO | | |
| M9-05 | E1M08: spec + script + success replay | RE/SP, IM, VE | M8-11 | TODO | | |
| M9-06 | E1M09: spec + script + success replay | RE/SP, IM, VE | M8-11 | TODO | | |
| M9-07 | E1M10: spec + script + success replay | RE/SP, IM, VE | M8-11 | TODO | | |
| M9-08 | E1M11: spec + script + success replay | RE/SP, IM, VE | M8-11 | TODO | | |
| M9-09 | E1M12: spec + script + success replay | RE/SP, IM, VE | M8-11 | TODO | | |
| M9-10 | E1M13: spec + script + success replay | RE/SP, IM, VE | M8-11 | TODO | | |
| M9-11 | E1M14: spec + script + success replay | RE/SP, IM, VE | M8-11 | TODO | | |
| M9-12 | E1M15: spec + script + success replay | RE/SP, IM, VE | M8-11 | TODO | | |
| M9-13 | E1M16: spec + script + success replay | RE/SP, IM, VE | M8-11 | TODO | | |
| M9-14 | E1M17: spec + script + success replay | RE/SP, IM, VE | M8-11 | TODO | | |
| M9-15 | E1M18: spec + script + success replay | RE/SP, IM, VE | M8-11 | TODO | | |
| M9-16 | E1M19: spec + script + success replay | RE/SP, IM, VE | M8-11 | TODO | | |
| M9-17 | E1M20: spec + script + success replay | RE/SP, IM, VE | M8-11 | TODO | | |
| M9-18 | E1 narrative script (cards, dialogue, stills, dossiers, music between missions) | RE/SP/IM | M5-09, M1-04 | TODO | | |
| M9-19 | Main menu, new game, save/load, kills-to-date | IM | M8-09 | TODO | | |
| M9-20 | Episode endings (success, continuation, failure variants) | SP/IM | M9-18 | TODO | | |
| M9-21 | Full E1 playthrough + parity report | VE | M9-01..M9-20 | TODO | | |

## M10: Episodes 2 and 3

| ID | Task | Hat | Depends on | Status | Owner | Notes |
|---|---|---|---|---|---|---|
| M10-01 | E2 engine deltas + data set | RE/SP | M1-09 | TODO | | |
| M10-02 | E3 engine deltas + data set + mission numbering (U-012) | RE/SP | M1-09 | TODO | | |
| M10-03 | Implement deltas as episode configuration | IM | M10-01, M10-02 | TODO | | |
| M10-04 | E2M01: spec + script + success replay | RE/SP, IM, VE | M10-03 | TODO | | |
| M10-05 | E2M02: spec + script + success replay | RE/SP, IM, VE | M10-03 | TODO | | |
| M10-06 | E2M03: spec + script + success replay | RE/SP, IM, VE | M10-03 | TODO | | |
| M10-07 | E2M04: spec + script + success replay | RE/SP, IM, VE | M10-03 | TODO | | |
| M10-08 | E2M05: spec + script + success replay | RE/SP, IM, VE | M10-03 | TODO | | |
| M10-09 | E2M06: spec + script + success replay | RE/SP, IM, VE | M10-03 | TODO | | |
| M10-10 | E2M07: spec + script + success replay | RE/SP, IM, VE | M10-03 | TODO | | |
| M10-11 | E2M08: spec + script + success replay | RE/SP, IM, VE | M10-03 | TODO | | |
| M10-12 | E2M09: spec + script + success replay | RE/SP, IM, VE | M10-03 | TODO | | |
| M10-13 | E2M10: spec + script + success replay | RE/SP, IM, VE | M10-03 | TODO | | |
| M10-14 | E2M11: spec + script + success replay | RE/SP, IM, VE | M10-03 | TODO | | |
| M10-15 | E2M12: spec + script + success replay | RE/SP, IM, VE | M10-03 | TODO | | |
| M10-16 | E2M13: spec + script + success replay | RE/SP, IM, VE | M10-03 | TODO | | |
| M10-17 | E2M14: spec + script + success replay | RE/SP, IM, VE | M10-03 | TODO | | |
| M10-18 | E2M15: spec + script + success replay | RE/SP, IM, VE | M10-03 | TODO | | |
| M10-19 | E2M16: spec + script + success replay | RE/SP, IM, VE | M10-03 | TODO | | |
| M10-20 | E2M17: spec + script + success replay | RE/SP, IM, VE | M10-03 | TODO | | |
| M10-21 | E2M18: spec + script + success replay | RE/SP, IM, VE | M10-03 | TODO | | |
| M10-22 | E2M19: spec + script + success replay | RE/SP, IM, VE | M10-03 | TODO | | |
| M10-23 | E2M20: spec + script + success replay | RE/SP, IM, VE | M10-03 | TODO | | |
| M10-24 | E3 mission 01: spec + script + success replay | RE/SP, IM, VE | M10-03 | TODO | | |
| M10-25 | E3 mission 02: spec + script + success replay | RE/SP, IM, VE | M10-03 | TODO | | |
| M10-26 | E3 mission 03: spec + script + success replay (numbering per U-012) | RE/SP, IM, VE | M10-03 | TODO | | |
| M10-27 | E3 mission 04: spec + script + success replay | RE/SP, IM, VE | M10-03 | TODO | | |
| M10-28 | E3 mission 05: spec + script + success replay | RE/SP, IM, VE | M10-03 | TODO | | |
| M10-29 | E3 mission 06: spec + script + success replay | RE/SP, IM, VE | M10-03 | TODO | | |
| M10-30 | E3 mission 07: spec + script + success replay | RE/SP, IM, VE | M10-03 | TODO | | |
| M10-31 | E3 mission 08: spec + script + success replay | RE/SP, IM, VE | M10-03 | TODO | | |
| M10-32 | E3 mission 09: spec + script + success replay | RE/SP, IM, VE | M10-03 | TODO | | |
| M10-33 | E3 mission 10: spec + script + success replay | RE/SP, IM, VE | M10-03 | TODO | | |
| M10-34 | E3 mission 11: spec + script + success replay | RE/SP, IM, VE | M10-03 | TODO | | |
| M10-35 | E3 mission 12: spec + script + success replay | RE/SP, IM, VE | M10-03 | TODO | | |
| M10-36 | E3 mission 13: spec + script + success replay | RE/SP, IM, VE | M10-03 | TODO | | |
| M10-37 | E3 mission 14: spec + script + success replay | RE/SP, IM, VE | M10-03 | TODO | | |
| M10-38 | E3 mission 15: spec + script + success replay | RE/SP, IM, VE | M10-03 | TODO | | |
| M10-39 | E3 mission 16: spec + script + success replay | RE/SP, IM, VE | M10-03 | TODO | | |
| M10-40 | E3 mission 17: spec + script + success replay | RE/SP, IM, VE | M10-03 | TODO | | |
| M10-41 | E3 mission 18: spec + script + success replay | RE/SP, IM, VE | M10-03 | TODO | | |
| M10-42 | E3 mission 19: spec + script + success replay | RE/SP, IM, VE | M10-03 | TODO | | |
| M10-43 | E3 mission 20: spec + script + success replay (numbering per U-012) | RE/SP, IM, VE | M10-03 | TODO | | |
| M10-44 | E2/E3 narrative scripts + endings | RE/SP/IM | M10-01, M10-02 | TODO | | |
| M10-45 | Full E2 + E3 playthroughs + parity reports | VE | M10-04..M10-44 | TODO | | |

## M11: Front end, options & meta

| ID | Task | Hat | Depends on | Status | Owner | Notes |
|---|---|---|---|---|---|---|
| M11-01 | Original main menu + episode select | SP/IM | M5-09 | TODO | | |
| M11-02 | In-flight options menu (Esc) | SP/IM | M6-06 | TODO | | |
| M11-03 | Modern settings screens (video, audio, controls, profile, accessibility, data dir) | IM | M4-05 | TODO | | |
| M11-04 | Gamepad UI navigation | IM | M11-03 | TODO | | |
| M11-05 | Credits screen (exe resources) | IM | M2-14 | TODO | | |
| M11-06 | Original save import (D3) | IM | M2-12, D3 | TODO | | |
| M11-07 | TFP extras (D4, optional) | RE/IM | D4 | TODO | | |

## M12: Fidelity verification & polish

| ID | Task | Hat | Depends on | Status | Owner | Notes |
|---|---|---|---|---|---|---|
| M12-01 | Master parity checklist + full L5 re-run | VE | M10, M11 | TODO | | |
| M12-02 | Timing audit | VE | M12-01 | TODO | | |
| M12-03 | Bug-parity review (original-bugs.md) | VE/SP | M12-01 | TODO | | |
| M12-04 | Enhanced features for 1.0 (D5) | IM | D5 | TODO | | |
| M12-05 | PC-speaker SFX mode (optional) | RE/IM | D5 | TODO | | |
| M12-06 | Replay soak, fuzzing, profiling | VE | M12-01 | TODO | | |
| M12-07 | Accessibility review | VE | M12-04 | TODO | | |

## M13: Release engineering

| ID | Task | Hat | Depends on | Status | Owner | Notes |
|---|---|---|---|---|---|---|
| M13-01 | Packaging per OS (D6) | IM | M12 | TODO | | |
| M13-02 | First-run import from folder or original ZIP | IM | M4-05 | TODO | | |
| M13-03 | Versioning, changelog, notices | IM | M12 | TODO | | |
| M13-04 | Player documentation | IM | M12 | TODO | | |
| M13-05 | Fresh-machine install tests + release checklist | VE | M13-01..M13-04 | TODO | | |

## M14: Post-1.0 enhancements

Not yet broken into tasks. See MDG §12 M14.

## Bugs

None yet. Use the bug template in ADG §17.5.
