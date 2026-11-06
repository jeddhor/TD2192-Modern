# Parity: M12-01 master checklist and M12-03 bug-parity review (2026-11-06, VE-6)

Verifier: VE-6 (Verifier hat; no disassembly read). Tree under test: main `4ba7af9` (IM-19 merged) plus this branch's docs. Main's later `c2ab363` (the user's D3–D6 decisions) changes only docs and is merged here. Inputs: every spec in `docs/spec/`, `docs/re/FORMATS.md`, the parity reports [M3-audio](M3-audio.md), [M6-flight](M6-flight.md), [M8-missions](M8-missions.md), [M9-playthrough](M9-playthrough.md), [M10-playthrough](M10-playthrough.md), [M12-timing](M12-timing.md), the tests, and the oracle captures (`re-work\captures\oracle`, `MANIFEST.md`). No new oracle run was made.

## 0. What remains for Classic 1.0 (ordered by impact on play)

1. **Screens every player sees that were never compared with the original.** The debriefing (after every flight), the assignment orders, the enemy and pilot dossiers, the story-so-far pages, the stills (`td1-13..20.pic`, `secret.pic`, `td1-15`), the E1 episode end, and the endings A/B/C of E1 and E2 (and E3's ending A). They are specified statically (screens §3.4–§3.10, episode-program-flow §4.5, e2/e3-program-flow), drawn by the port, and checked only by flow tests. No capture holds them, except C-0170, a story-so-far run whose frames no test compares (only its title-card fade timing is used). They need captures and L2/L5 tests (§4 flow/screens rows 4, §3.4–3.5, §3.8–3.10).
2. ~~E2/E3 two-digit muzzle bytes~~ **Done after this review (IM-20, merged 90bd09f):** the E2/E3 worlds are built from front-end state, equal all 58 whole-flight captures and the C-0015/C-0036/C-0180/C-0121/C-0128 campaign states, every L5 flight uses the built bytes, and the dual replays were rewritten.
3. **Session state across flights is unverified.** Every L5 flight is a direct start, the first flight of a program run. The carry-over that a real campaign runs on has never been compared with a capture: the RNG stream after a previous flight, the class-table patches (OB-018, flights 12, 15, 20, 21; E2 flight 8), the mission globals between flights (U-144), `g_Weapon`, E2's radar colours (OB-024), and `g_MemAvailText` in a long campaign (U-440). This needs a capture of two consecutive flights, frame-keyed, or with a re-armable tick watcher (U-481).
4. **The flight-start fade-in is not ported** (M12-timing §7). Every flight starts with a cut where the original runs a 34-frame `FadeIn` before tick 1. BUG-10 is filed for it.
5. **Flights never compared:** E1 flight 21 (U-044), and E3 flights 7 and 21 (U-481). Only static tests cover them.
6. **Inputs and outcomes no capture exercised inside whole flights:**
   - missiles and Tab (C-0011 is 30 s long);
   - the afterburner burst;
   - vertical bounces and the map-edge push-back (flight-model, LIKELY parts);
   - the joystick (U-024);
   - E2/E3 deaths and end sequences, aborts and "lost" outcomes;
   - E1 failures (lost in flights 13, 17, 18 and 20; the flight-18 bomb).
7. **Original cheat chords are not ported** (T+I+M, M+K+A+R, tick-order step 34; `td-sim/src/flight/mod.rs:13`, "decision D5/D9"). Players who remember them will miss them, and the debrief's cheater text (D9) depends on them. This needs the user's decision.
8. **A Classic deviation found here (BUG-9, OB-023).** In E2, Alt from infra-red mode 2 back to mode 1 should leave the screen red-tinted: only DAC 255 and 37 are rewritten. The port restores the whole mode-1 palette in every episode and profile (`td_render::flight_palette::FlightDac::toggle`). The player guide therefore says the wrong thing about it.
9. **Missing Enhanced bug fixes.** The user's D5 decision (2026-11-06) puts every fix-enhanced fix in the Enhanced preset, so these are now 1.0 scope. OB-012, OB-024, OB-032, OB-056 and OB-057 are fix-enhanced and not implemented. OB-031 and OB-023 behave as fixed in both profiles, against their policy. The candidates OB-022, OB-036, OB-038, OB-041, OB-043, OB-046 and OB-102 have no decision (§7.2).
10. **55 OBs are still "proposed"** and OB-008 is still "open" (U-060 is now resolved). They need one batch decision (§7.1). OB-005 and OB-034 are labelled fix-all but are not crash, hang, save or progression bugs (§7.1).
11. **Timing residues:**
    - ADR-0007 (19.2015 Hz) was accepted by the user on 2026-11-06 (after this review), so U-001 can be closed;
    - U-451: `Crt.Delay` ends the flight-17 cut-scene about 12 retraces early;
    - U-461: the fade crossover at 380 is an estimate;
    - after a menu fade-out the original spends 1–2 frames more than the port (M12-timing §7);
    - nobody has counted ticks at runtime in the real binary (BUG-5 re-check);
    - the comm-screen typing pace was not measured (M12-timing row 13).
12. **Spec, test and documentation drift:**
    - mission-logic §7 vector 12 says the flight-18 bomb goes off at t + 1000, but the test pins t + 999 (§4);
    - four stale or closable ASSUMPTIONs: U-313, U-001, U-146 and the `fade.rs` header (§5);
    - text.md §5 item 3 and hud.md's "low-armour colour after recovery" are "preserve" quirks with no OB entry.
13. **Lower impact:**
    - the PC-speaker mode is not ported (M12-05, OB-011; sfx-policy vectors 8–9);
    - positioned effects (panning, the 15-tile cut-off) were never captured (M3 row 20);
    - parsers have no fuzz targets or property tests except `saves.rs` (M12-06; MDG §11 asks for 1 h of fuzzing).

**Coverage.** The checklist has 393 items (spec vectors and numbered behaviours):

| Status | Items | Share of the 375 Classic items |
|---|---:|---:|
| PASS | 327 | 87.2 % |
| PARTIAL | 33 | 8.8 % |
| NOT COVERED | 15 | 4.0 % |
| N/A (Enhanced, T3, not used by the game) | 18 | not counted |

**M12 exit criterion not met.** It asks for at least 95 % PASS with the rest documented. Every non-PASS item is documented below. Items 1–6 above account for most of the gap.

**L5 re-run.** All 789 tests pass with and without `TD2192_DATA` and `TD2192_ORACLE`: 0 failed, 4 ignored, the same in both runs. The ignored tests are timing benchmarks and developer tools. Counts per binary are in §3.

## 1. Method and status rules

- Each spec's test-vector section (§6 or §7, or §5 in older specs) was read row by row. Every row, or group of rows checked by the same test, became one item. Named behaviours outside the vector table were added where a parity report or STATUS tracks them, for example a spec section or a known gap. Code and test citations were searched with `grep` (spec name plus "vector", "§", C-, OB-, U- and BUG- ids), and every cited test was confirmed to exist.
- **PASS:** a test asserts the vector's values, or an L5 test compares the behaviour with a capture, and the test passed in this run.
- **PARTIAL:** the behaviour is implemented, but the test checks only part of it (the cell says what is missing), or the check covers only the path a capture happened to take.
- **NOT COVERED:** no test checks it, or the behaviour is not ported.
- **N/A:** Enhanced or T3 by design, or behaviour the game never uses.
- "Full flight" rows rely on the L5 comparisons of M9/M10. They compare every tick from setup to the outcome: the RNG seed, draw page, phase and mission flags, the player's state, the key table, every live slot's arrays and its 1,065-byte record, the projectiles and the hit flash. They cover only the paths the planned inputs took.

## 2. Summary per spec

| Spec | Rev | Tiers | Items | PASS | PARTIAL | NOT COV. | N/A | Main gaps |
|---|---|---|---:|---:|---:|---:|---:|---|
| re/FORMATS.md | — | T0 | 15 | 11 | 1 | 0 | 3 | stills never compared; `td.cfg`, `tdstatus.dat`, TFP are T3/D4 |
| core/bp7-runtime-semantics | 2 | T1, T3 | 11 | 9 | 1 | 0 | 1 | Real `Random` values 2–3 |
| core/real48 | 1 | T1 | 2 | 2 | 0 | 0 | 0 | — (1,705 file vectors) |
| core/timing | 5 | T1, T2 | 9 | 4 | 1 | 0 | 4 | U-451; ADR-0007 accepted 2026-11-06 |
| audio/cmf | 3 | T1, T2 | 12 | 11 | 1 | 0 | 0 | song lengths except `menu` |
| audio/music-cues | 2 | T1, T2 | 5 | 2 | 2 | 0 | 1 | E1 cue order not asserted end to end |
| audio/sfx-policy | 4 | T1, T2 | 11 | 8 | 0 | 3 | 0 | PC speaker (M12-05); positioned effects |
| audio/voc-and-adpcm | 3 | T0, T2 | 4 | 4 | 0 | 0 | 0 | — |
| render/hud | 3 | T0, T1 | 9 | 8 | 1 | 0 | 0 | E2/E3 home-point table |
| render/text | 3 | T0, T2 | 4 | 4 | 0 | 0 | 0 | — |
| narrative/dialogue-player | 2 | T0, T1, T2 | 7 | 7 | 0 | 0 | 0 | — |
| sim/camera-viewport | 1 | T1, T2 | 2 | 1 | 1 | 0 | 0 | C-0008 frame |
| sim/collision | 2 | T1 | 9 | 7 | 2 | 0 | 0 | vertical bounce, map edge |
| sim/flight-model | 2 | T1 | 8 | 7 | 1 | 0 | 0 | LIKELY parts not captured |
| sim/tick-order | 1 | T1 | 3 | 3 | 0 | 0 | 0 | — |
| sim/hit-flash | 4 | T0, T1 | 12 | 9 | 3 | 0 | 0 | C-0041 flash and DAC frames |
| sim/mission-setup | 2 | T1 | 3 | 2 | 0 | 1 | 0 | OB-018 persistence |
| sim/mission-ships | 5 | T1, T2 | 6 | 6 | 0 | 0 | 0 | — |
| sim/weapons | 5 | T1, T2 | 11 | 10 | 1 | 0 | 0 | missiles in whole flights (E2/E3 bytes done by IM-20 after this review) |
| sim/mission-logic (E1) | 3 | T1, T2 | 16 | 13 | 2 | 1 | 0 | vector 12; flight 21; failure paths |
| sim/e2-mission-setup | 1 | T1, T2 | 10 | 9 | 1 | 0 | 0 | home points |
| sim/e2-mission-ships | 1 | T1 | 5 | 5 | 0 | 0 | 0 | — |
| sim/e2-mission-logic | 1 | T1, T2 | 36 | 33 | 1 | 2 | 0 | vector 11; flight-16 transmission (OB-038) |
| sim/e3-mission-setup | 1 | T1, T2 | 14 | 13 | 1 | 0 | 0 | flight 7 |
| sim/e3-mission-ships | 1 | T1 | 9 | 9 | 0 | 0 | 0 | — |
| sim/e3-mission-logic | 1 | T1, T2 | 55 | 44 | 8 | 2 | 1 | flights 7, 21; U-430; branch vectors |
| flow/screens | 3 | T0, T1, T2 | 10 | 7 | 1 | 2 | 0 | debrief, orders, dossiers, story so far, stills |
| flow/episode-program-flow | 6 | T2, T1, T3 | 6 | 5 | 1 | 0 | 0 | endings: no capture |
| flow/launcher | 1 | T3, T1 | 4 | 3 | 0 | 0 | 1 | — |
| flow/intro | 3 | T0, T1, T2 | 10 | 10 | 0 | 0 | 0 | — |
| flow/save-screens | 1 | T0, T1, T2 | 7 | 7 | 0 | 0 | 0 | — |
| flow/flight-screens | 5 | T0, T1, T2 | 24 | 21 | 1 | 2 | 0 | flight-start fade-in; comm pace; C-0027 |
| flow/e2-flight-screens | 1 | T0, T1, T2 | 5 | 4 | 0 | 0 | 1 | — |
| flow/e2-program-flow | 3 | T2, T1, T0 | 15 | 13 | 1 | 1 | 0 | item 6; endings |
| flow/e3-program-flow | 3 | T2, T1, T0 | 18 | 17 | 1 | 0 | 0 | items 6/7 timing |
| frontend/credits | 1 | T3 | 2 | 0 | 0 | 0 | 2 | — |
| frontend/port-front-end | 2 | T3 | 2 | 0 | 0 | 0 | 2 | — |
| enhanced/presentation | 4 | T3 | 2 | 0 | 0 | 0 | 2 | Classic invariance passes |
| **Total** | | | **393** | **327** | **33** | **15** | **18** | |

`original-bugs.md` is reviewed in §7.

## 3. Test runs (M12-01 L5 re-run)

`cargo test --workspace --no-fail-fast` on this branch (main `4ba7af9` plus this branch's docs; debug profile), on 2026-11-06:

| Run | Passed | Failed | Ignored | Wall time |
|---|---:|---:|---:|---|
| Without `TD2192_DATA` and `TD2192_ORACLE` (data-gated tests return early) | 789 | 0 | 4 | 53 s (after a 5 min build) |
| With `TD2192_DATA=P:\msdos\TD2192\TD2192` and `TD2192_ORACLE=P:\msdos\TD2192\re-work\captures\oracle` | 789 | 0 | 4 | 22 min 37 s |

The run with the variables took 23 minutes on this machine, not an hour. `intro_parity` (280 s), `oracle_e1_flights` (376 s) and `hud_parity` (120 s) take most of it. A third run with `--nocapture` checked that no data-gated test skips when the variables are set (§3.1). `cargo fmt --check` passes. Clippy was not run: this branch changes only Markdown.

Counts per test binary. The two runs give the same counts; the time is from the run with the variables:

| Binary | Passed | Ignored | Time (s) |
|---|---:|---:|---:|
| `td-assets` unit tests (src/lib.rs) | 22 | 0 | 2.0 |
| `td-audio` unit tests (src/lib.rs) | 51 | 0 | 0.0 |
| `td-audio/tests/oracle_adpcm.rs` | 1 | 0 | 5.0 |
| `td-audio/tests/oracle_parity.rs` | 3 | 0 | 4.0 |
| `td-audio/tests/original_audio.rs` | 2 | 0 | 0.7 |
| `td-flow` unit tests (src/lib.rs) | 126 | 1 | 3.8 |
| `td-flow/tests/dialogue_parity.rs` | 2 | 0 | 2.7 |
| `td-flow/tests/e1_block_flow.rs` | 5 | 0 | 0.2 |
| `td-flow/tests/e2_flight_screens.rs` | 6 | 0 | 2.7 |
| `td-flow/tests/e2_front_end.rs` | 20 | 0 | 1.5 |
| `td-flow/tests/e3_flight_screens.rs` | 3 | 0 | 0.0 |
| `td-flow/tests/e3_front_end.rs` | 22 | 0 | 1.5 |
| `td-flow/tests/flight_screens_parity.rs` | 9 | 0 | 9.2 |
| `td-flow/tests/front_end_state.rs` | 3 | 0 | 0.3 |
| `td-flow/tests/interlude_parity.rs` | 2 | 0 | 3.9 |
| `td-flow/tests/intro_parity.rs` | 6 | 0 | 280.2 |
| `td-flow/tests/launcher_shell.rs` | 4 | 0 | 12.1 |
| `td-flow/tests/menu_pacing.rs` | 2 | 0 | 0.5 |
| `td-flow/tests/save_parity.rs` | 3 | 0 | 13.8 |
| `td-flow/tests/story_parity.rs` | 16 | 0 | 13.0 |
| `td-formats` unit tests (src/lib.rs) | 30 | 0 | 0.0 |
| `td-formats/tests/original_graphics.rs` | 9 | 0 | 5.5 |
| `td-formats/tests/original_text_and_missions.rs` | 5 | 0 | 14.4 |
| `td-input` unit tests (src/lib.rs) | 4 | 0 | 0.0 |
| `td-opl` unit tests (src/lib.rs) | 2 | 0 | 0.0 |
| `td-platform` unit tests (src/lib.rs) | 3 | 0 | 0.0 |
| `td-render` unit tests (src/lib.rs) | 101 | 0 | 0.2 |
| `td-render/tests/e3_flight_parity.rs` | 2 | 0 | 1.3 |
| `td-render/tests/enhanced_lighting_timing.rs` | 0 | 1 | 0.0 |
| `td-render/tests/enhanced_parity.rs` | 2 | 0 | 2.0 |
| `td-render/tests/enhanced_timing.rs` | 0 | 1 | 0.0 |
| `td-render/tests/flight_parity.rs` | 1 | 0 | 1.6 |
| `td-render/tests/heading_parity.rs` | 1 | 0 | 33.6 |
| `td-render/tests/hud_parity.rs` | 4 | 0 | 120.1 |
| `td-render/tests/oracle_parity.rs` | 4 | 0 | 0.4 |
| `td-sim` unit tests (src/lib.rs) | 124 | 0 | 0.1 |
| `td-sim/tests/flight_e1.rs` | 4 | 0 | 0.2 |
| `td-sim/tests/flight_e3.rs` | 8 | 0 | 0.3 |
| `td-sim/tests/oracle_e1_flights.rs` | 24 | 0 | 376.0 |
| `td-sim/tests/oracle_e1_full.rs` | 22 | 0 | 19.1 |
| `td-sim/tests/oracle_e2.rs` | 2 | 0 | 11.3 |
| `td-sim/tests/oracle_e2_full.rs` | 20 | 0 | 9.8 |
| `td-sim/tests/oracle_e3.rs` | 2 | 0 | 21.8 |
| `td-sim/tests/oracle_e3_full.rs` | 18 | 0 | 10.8 |
| `td-sim/tests/oracle_flight.rs` | 1 | 0 | 29.7 |
| `td-sim/tests/oracle_mission.rs` | 1 | 0 | 24.5 |
| `td-sim/tests/oracle_ships.rs` | 1 | 0 | 23.2 |
| `td-sim/tests/oracle_weapons.rs` | 1 | 0 | 9.1 |
| `td-sim/tests/replays_e1.rs` | 22 | 0 | 3.5 |
| `td-sim/tests/replays_e2.rs` | 20 | 0 | 3.3 |
| `td-sim/tests/replays_e3.rs` | 18 | 0 | 4.4 |
| `td-test-support` unit tests (src/lib.rs) | 1 | 0 | 0.0 |
| `td2192` unit tests (src/main.rs) | 24 | 1 | 7.0 |

### 3.1 Skip check

A data-gated test passes silently when it returns early, so a green run with the variables set does not prove that the L5 comparisons ran. A third run therefore used `cargo test --workspace --no-fail-fast -- --nocapture --test-threads 4` with both variables set (12:24–13:08):

- **Counts:** the same 789 passed, 0 failed, 4 ignored.
- **Skips:** no `SKIPPED` line. That message is what `require_data!`, `require_oracle!` and the capture lookups print, for example "no ve1-e1f… capture under the oracle root".
- **What the oracle tests compared:** their summary lines name 92 captures, from C-0001 to C-0159. Each E2 full flight, for example, prints its tick count, slot-ticks and event list (`cargo test -p td-sim --test oracle_e2_full -- --nocapture`).
- **Captures read without a printed ID:** C-0007/8, C-0011..C-0014, C-0042..C-0047, C-0071/72, C-0080, C-0081, C-0160 and C-0170 are read by tests that do not print the capture ID at the start of a line. Those tests assert pixel or state equality.

## 4. Per-spec detail

Test paths are `crate/tests/file.rs::name` for integration tests and `crate/src/file.rs::name` for unit tests (the `crates/` prefix is left out). "Full flight" means the L5 whole-flight comparisons of [M9-playthrough](M9-playthrough.md) and [M10-playthrough](M10-playthrough.md) (`td-sim/tests/oracle_e1_full.rs`, `oracle_e2_full.rs`, `oracle_e3_full.rs`: every tick, every slot, RNG seed, draw page, hit flash, key table, projectiles). Status is the last column of every row.

### re/FORMATS.md (formats live in `docs/re/FORMATS.md`, not `docs/spec/`; T0 decoding)

| # | Format | Test(s) | Evidence | Status |
|---|---|---|---|---|
| §1–§2 | Palettes, full-screen images | `td-formats/src/palette.rs` (2), `screen.rs` (2); `td-formats/tests/original_graphics.rs::palettes_parse`, `::all_full_screens_parse`; pixel-exact screens in `story_parity`, `save_parity` | every shipped file; C-0002 | PASS |
| §3 | Help screens | `original_graphics.rs::help_file_has_two_pages`; `flight_screens_parity.rs::computer_menu_library_and_help_match_c0012` | C-0012 | PASS |
| §4–§5 | Tilesets, maps (OB-006) | `original_graphics.rs::tilesets_parse`, `::maps_parse_and_reference_valid_tiles`, `::map_orientation_matches_oracle_flight_frame` | C-0002 f3500 | PASS |
| §6 | Sprites, rotation sets, other sprite files | `original_graphics.rs::sprite_sheets_parse`; `heading_parity.rs::vector_20_every_heading_matches_c0080` | C-0080 | PASS |
| §7 | Missions (U-060) | `td-formats/src/mission.rs` (3), `original_text_and_missions.rs::mission_files_parse`; every full flight's setup | C-0009.. | PASS |
| §8 | Dialogue scripts | `td-formats/src/dialogue.rs` (3), `original_text_and_missions.rs::dialogue_scripts_parse_strictly`, `::player_reader_agrees_with_the_strict_parser` | data | PASS |
| §9 | Fonts | `td-formats/src/font.rs` (2), `original_graphics.rs::fonts_parse_with_visible_glyphs`; text L5 tests | C-0002 | PASS |
| §10–§11 | Portraits, stills | `original_graphics.rs::portraits_stills_and_cell_sheets_parse`; portraits pixel-exact in `oracle_parity`; stills never compared with a capture | C-0002 (portraits) | PARTIAL (stills: no capture) |
| §12 | Raw UI graphics (`td.men`, `td.win`, `td.cmm`, `td*.ccm`, `td.sss`, `td.wea` …) | `td-formats/src/picture.rs` (2); used pixel-exact in `flight_screens_parity`, `hud_parity`, `save_parity` | C-0012, C-0013, C-0002, C-0042 | PASS |
| §13.1 | `td.cfg` | none (T3: the port keeps `settings.toml`; no configuration screen) | C-0048 | N/A |
| §13.2–§13.3 | Saves `td*.sav`, names `td*.stt` | `td-formats/src/saves.rs` (4, incl. arbitrary bytes), `save_parity.rs` | C-0042, C-0043 | PASS |
| §13.4 | `tdstatus.dat` | none (T3 hand-off, launcher §4.10) | C-0001, C-0049 | N/A |
| §14–§15 | CMF, VOC containers | `td-formats/src/cmf.rs` (2), `voc.rs` (2), `original_text_and_missions.rs::audio_containers_parse` | data | PASS |
| §16 | TFP presentation (`aboutus.tfp`, `order.tfp`) | none (D4 decided 2026-11-06: no TFP extras for 1.0; M11-07 won't do) | none | N/A |
| — | MZ headers, executable resources | `td-formats/src/mz.rs` (2), `original_text_and_missions.rs::executable_headers_match_executables_md`, `td-assets/src/exe_res.rs` (3) | data | PASS |

### core/bp7-runtime-semantics.md (rev 2; T1 Random and Real48, T3 the rest)

| # | Behaviour | Test(s) | Evidence | Status |
|---|---|---|---|---|
| 1 | Seed sequence from 0 | `td-sim/src/rng.rs::spec_vector_1_seed_sequence` | static; the seed is compared every tick in every full flight | PASS |
| 2–5 | `Random(100)`, `(320)`, `(2)`, `(65535)` from seed 0 | `td-sim/src/rng.rs::spec_vectors_2_to_5` | static | PASS |
| 6–7 | Mixed ranges; `Random(0)` | `td-sim/src/rng.rs::spec_vectors_6_and_7` | static | PASS |
| 8 | Real `Random` × 3 (Real48 bytes) | `td-sim/src/rng.rs::real_random_is_exact` asserts only the first value (2⁻³²) | static | PARTIAL (values 2 and 3 not asserted; no game path known to use Real `Random`) |
| 9 | `Randomize` (never called by the game) | none | static | N/A |
| 10 | LongInt → Real | `td-sim/src/real48.rs::matches_all_vectors_from_the_original` (the `from_long` rows 1, −1, 65536 are in `real48-vectors.csv`) | static | PASS |
| 11 | `Round` halves away from zero | `td-sim/src/real48.rs::rounding_is_half_away_from_zero` | static | PASS |

### core/real48.md (rev 1; T1)

| # | Behaviour | Test(s) | Evidence | Status |
|---|---|---|---|---|
| 6.1 | All 1,705 rows of `real48-vectors.csv` (generated by the original routines) | `td-sim/src/real48.rs::matches_all_vectors_from_the_original` | 247,705-input agreement (RE) | PASS |
| 6.2 | Selected vectors 1–38 (OB-010 rounding included) | `td-sim/src/real48.rs::spec_examples`, `::matches_all_vectors_from_the_original` | static | PASS |

### core/timing.md (rev 5; T1 one step per tick, T2 rates)

| # | Behaviour | Test(s) | Evidence | Status |
|---|---|---|---|---|
| 1 | WORX idle: 18.52 Hz | none (ADR-0007 keeps one canonical rate) | static | N/A |
| 2 | Song at 96 Hz: 19.20 Hz | `td-flow/tests/story_parity.rs::a_campaign_flight_runs_at_the_flight_rate` (BUG-5), `td-flow/src/timing.rs::no_drift_over_many_frames` | static | PASS |
| 3 | Measured 19.21 Hz (SB + music) | as row 2 (`FLIGHT_TICK_HZ` = 19.2015, ADR-0007 proposed) | C-0074, C-0010 | PASS |
| 4–6 | `/nosound` 18.21 Hz, no music 18.52 Hz, CPU-bound at 3,000 cycles | none (alternatives not ported, M12-timing §3) | C-0072, C-0074 | N/A |
| 7–8 | DOS-time waits of 5 s and 10 s: 91 and 182 BIOS ticks | `td-flow/src/timing.rs::dos_time_waits_count_bios_ticks`, `td-flow/src/story.rs::timed_hold_fades_in_waits_and_fades_out` | C-0002, C-0170 (port 332 / 664 frames, original 328 / 664) | PASS |
| 9 | `Crt.Delay` in the flight-17 cut-scene: 673 frames | `td-flow/tests/interlude_parity.rs::flight_17_cut_scene_matches_c0067` checks the pictures, not the length | C-0067 | PARTIAL (U-451: real milliseconds end the scene about 12 retraces early) |

### audio/cmf.md (rev 3; T1 register stream, T2 audio)

| # | Behaviour | Test(s) | Evidence | Status |
|---|---|---|---|---|
| V1–V2 | Note on/off; velocity ignored | `td-audio/src/cmf_player/tests.rs::v1_v2_velocity_is_ignored` | WORX on an interpreter | PASS |
| V3 | Channel volume scales the carrier | `…::v3_channel_volume_scales_the_carrier_level` | as above | PASS |
| V4 | Transpose 69h | `…::v4_transpose_69h_lowers_the_f_number` | as above | PASS |
| V5 | Voice allocation | `…::v5_voice_allocation`, `…::notes_are_dropped_when_every_voice_sounds`, `…::a_silent_voice_of_another_channel_is_taken_over` | as above | PASS |
| V6 | Rhythm toggle | `…::v6_drums_toggle` | as above | PASS |
| V7 | Extreme notes | `…::v7_extreme_notes_corrupt_the_block` | as above | PASS |
| V8 | Loop restart in the same tick | `…::v8_loop_restarts_in_the_same_tick_without_key_off`, `…::every_pass_of_a_looping_song_lasts_l_ticks` | as above | PASS |
| C1 | Intro songs, 4,364 writes | `td-audio/tests/oracle_parity.rs::intro_music_matches_capture_c0001` | C-0001 | PASS |
| C2 | Write timing | `td-audio/tests/oracle_parity.rs::game_music_matches_capture_c0002` (±1.5 ticks), `td-audio/src/cmf_player/tests.rs::player_ticks_at_the_pit_rate` | C-0001, C-0002 | PASS |
| C3 | Song durations (data survey) | `td-audio/tests/original_audio.rs::every_song_renders_audible_output` (no length asserted) | data survey | PARTIAL (only `menu`'s pass length is pinned, by C4) |
| C4 | Menu loop: 4,841 ticks per pass, first tick at the start | `td-audio/tests/oracle_parity.rs::looping_menu_music_matches_capture_c0073` | C-0073 | PASS |

### audio/music-cues.md (rev 2; T1 which song, T2 sound)

| # | Behaviour | Test(s) | Evidence | Status |
|---|---|---|---|---|
| 1 | `menu.cmf` at the E1 menu | `td-audio/src/music_cues.rs::cue_table`; the E1 flow's cue order is not asserted end to end (the E2/E3 menus are, in `e2_front_end`/`e3_front_end`) | C-0002 (register level only) | PARTIAL |
| 2 | Mission 1: hangar, low, td01, low, hangar | `td-flow/tests/story_parity.rs::every_block_plays_to_the_end` asserts that each mission plays music, not the order | static; C-0002 to the flight | PARTIAL |
| 3 | Missions 2, 3: td03, td02 | `td-audio/src/music_cues.rs::flight_song_rotates_by_mission` | static | PASS |
| 4 | `/nosound`: no song requests | none (the port has no `/nosound`; volume 0 instead) | static | N/A |
| 5 | Death once, no repeat | `td-flow/tests/story_parity.rs::death_without_retry_is_game_over` (looping off), `td-flow/tests/e2_front_end.rs::death_declined_is_ending_a` | static | PASS |

### audio/sfx-policy.md (rev 4; T1 RNG of the PC-speaker path, T2 sound)

| # | Behaviour | Test(s) | Evidence | Status |
|---|---|---|---|---|
| 1–5 | Distance, levels, mixer byte; Real48 `Round(Sqrt)` | `td-audio/src/sfx.rs::spec_vectors_volume_and_panning`, `::distance_rounds_like_real48` | static | PASS |
| 6 | 14 tiles plays, 15 does not | `td-audio/src/sfx.rs::effects_15_tiles_away_never_play` | static | PASS |
| 7 | Higher priority is not cut | `td-audio/src/sfx.rs::higher_priority_effects_are_not_cut` | static; C-0072 (pre-emption, M3 row 14d) | PASS |
| 8–9 | PC speaker (device 3): queue and `Random` calls | none: the PC-speaker path is not ported (M12-05, OB-011) | static | NOT COVERED |
| 10 | `boomboom` holds priority 5 for its busy time | `td-audio/src/sfx.rs::boomboom_holds_its_priority_for_its_busy_time` | C-0072 | PASS |
| — | Positioned effects: onset, panning, 15-tile cut-off in a capture | none | not captured (M3 row 20) | NOT COVERED |

### audio/voc-and-adpcm.md (rev 3; T0 decoding, T2 playback and status)

| # | Behaviour | Test(s) | Evidence | Status |
|---|---|---|---|---|
| 4.1 | 12 synthetic ADPCM vectors | `td-audio/src/adpcm.rs::spec_synthetic_vectors` | static | PASS |
| 4.2 | SHA-256 of all 17 decoded files, DSP rate clamp | `td-audio/tests/original_audio.rs::every_sound_effect_decodes_as_the_spec_says`, `td-audio/src/sound.rs::dsp_rates_clamp_per_codec` | data | PASS |
| 4.3 | Oracle correlation ≥ 0.98 for five effects | `td-audio/tests/oracle_adpcm.rs::adpcm_effects_correlate_with_the_oracle_wavs` | C-0071, C-0072 | PASS |
| §2.3 | WORX playing status (OB-100) | `td-audio/src/worx.rs::busy_times_match_the_spec_table`, `::intro_busy_times_match_c0081_to_the_frame` | C-0081 | PASS |

### render/hud.md (rev 3; T0 geometry and colours, T1 two rules)

| # | Behaviour | Test(s) | Evidence | Status |
|---|---|---|---|---|
| 1 | Bar = `td.win` except the drawn elements | `td-render/tests/hud_parity.rs::c0002_f3543_instruments_match` | C-0002 | PASS |
| 2 | Shields, armour, missiles, radar: 0 differing pixels | `td-render/tests/hud_parity.rs::c0002_f3543_instruments_match`, `::c0010_instruments_match_through_the_objective_sequence` | C-0002, C-0010 | PASS |
| 3 | Compass and missile lamps | as row 2 | C-0002 f3543 palette | PASS |
| 4 | Guidance dots (OB-020) | `td-render/src/hud.rs::guidance_dots_and_lamps_match_the_capture_vectors`, `::same_column_uses_the_plus_two_test` | C-0002 f3543 | PASS |
| 5 | Radar clipping, player dot | `td-render/tests/hud_parity.rs::c0002_f3543_instruments_match` | C-0002 | PASS |
| 6 | `k_TileRadarColour` from the executable | as row 2 (radar drawn from the table, 0 differences) | exe resource | PASS |
| §5 | Weapon-0 icon (OB-090) | `td-render/tests/hud_parity.rs::weapon_0_icon_matches_c0069` | C-0069 | PASS |
| §4.7 | Mission-ship brackets | `td-render/tests/hud_parity.rs::c0009_mission_ship_brackets_match` | C-0009 | PASS |
| — | E2/E3 home points of the guidance line (e2-mission-setup vector 8, e3 vector 12) | none asserts the table (`td_render::hud` holds it) | static | PARTIAL |

### render/text.md (rev 3; T0)

| # | Behaviour | Test(s) | Evidence | Status |
|---|---|---|---|---|
| 1 | Mission 1 title: width 185, x 67 | `td-render/tests/oracle_parity.rs::mission_title_card_matches_the_original` | C-0002 f1162 | PASS |
| 2 | Speaker names | `td-render/tests/oracle_parity.rs::portraits_names_and_palette_match_the_original`, `td-render/src/dialogue.rs::name_positions` | C-0002 f2433 | PASS |
| 3 | Dialogue prompt | `td-render/tests/oracle_parity.rs::dialogue_prompt_matches_the_original` | C-0002 f2433 | PASS |
| 4 | Typed and scrolled line | `td-render/tests/oracle_parity.rs::typed_dialogue_line_matches_the_original` | C-0002 f2433 | PASS |

### narrative/dialogue-player.md (rev 2; T0 layout, T1 order, T2 pacing)

| # | Behaviour | Test(s) | Evidence | Status |
|---|---|---|---|---|
| 1 | Palette after the first line | `td-render/tests/oracle_parity.rs::portraits_names_and_palette_match_the_original`, `td-flow/tests/dialogue_parity.rs::scene_1_frames_match_the_original` | C-0002 f2283 | PASS |
| 2 | Palette after the second line | `td-flow/tests/dialogue_parity.rs::scene_1_frames_match_the_original` | C-0002 f2433 | PASS |
| 3 | Portrait pixels | as row 1 | C-0002 f2283/f2433/f2573 | PASS |
| 4 | Names | as row 1 | C-0002 f2433 | PASS |
| 5 | Newest text line | `td-render/tests/oracle_parity.rs::typed_dialogue_line_matches_the_original` | C-0002 f2433 | PASS |
| 6 | Fade-in: 17 levels, 2 frames each | `td-flow/src/dialogue.rs::plays_a_scene_with_its_timing`, `td-render/src/fade.rs::fade_speed_follows_the_non_zero_components` | C-0002 frames.tsv 2186..2219 | PASS |
| — | Every scene of every script plays (E1, E2, E3) | `td-flow/tests/dialogue_parity.rs::every_scene_of_every_script_plays_to_the_end` | data | PASS |

### sim/camera-viewport.md (rev 1; T1 view state, T2 presentation)

| # | Behaviour | Test(s) | Evidence | Status |
|---|---|---|---|---|
| 1 | View (102, 104, 32, 16) → window and start address | `td-render/tests/flight_parity.rs::playfield_at_rest_matches_the_original` | C-0002 f3500, f3543 | PASS |
| 2 | View (109, 104, 28, 48) at C-0008 tick 70 | `td-render/src/flight.rs::horizontal_scroll_is_quantised_to_four_pixels` (the rule); the C-0008 frame is not rendered and compared | C-0008 (address computed) | PARTIAL |

### sim/collision.md (index, rev 2: the header said 1 until this branch corrected it; T1)

| # | Rule | Test(s) | Evidence | Status |
|---|---|---|---|---|
| 1 | Player ↔ map, bounce | `td-sim/tests/oracle_flight.rs::flight_matches_the_original_tick_by_tick`, `td-sim/tests/flight_e1.rs::vector_4_bounce_sequence` | C-0007, C-0008 (horizontal only) | PARTIAL (vertical bounces LIKELY, not captured) |
| 2 | Player ↔ map edge | `td-sim/src/flight/tests.rs` (unit only) | none | PARTIAL (not captured) |
| 3 | Projectile ↔ map | `td-sim/tests/oracle_weapons.rs::player_weapons_match_the_original` | C-0011 | PASS |
| 4 | Projectile ↔ map edge | full flights | C-0051..C-0159 | PASS |
| 5 | Projectile ↔ mission ship | full flights (kills, shields, armour) | C-0051..C-0159 | PASS |
| 6 | Projectile ↔ player | full flights (player armour and shields every tick) | C-0051..C-0159, C-0157 | PASS |
| 7 | Mission ship ↔ map (detours, OB-013) | `td-sim/tests/oracle_ships.rs::mission_ships_match_c0009_tick_by_tick` | C-0009 | PASS |
| 8 | Mission ship ↔ map edge | full flights (removals) | C-0017..C-0159 | PASS |
| 9 | No contact damage | full flights | C-0051..C-0159 | PASS |

### sim/flight-model.md (rev 2; T1)

| # | Behaviour | Test(s) | Evidence | Status |
|---|---|---|---|---|
| 1 | Class 4 thrust from rest, release | `td-sim/src/flight/tests.rs::vector_1_class_4_hold_up_then_release`, `td-sim/tests/oracle_flight.rs::flight_matches_the_original_tick_by_tick` | C-0008 | PASS |
| 2 | Class 0 thrust | `td-sim/src/flight/tests.rs::vector_2_class_0_hold_up`, `oracle_flight` | C-0008 | PASS |
| 3 | Start state | `td-sim/src/flight/tests.rs::vector_3_start_state` | C-0002 | PASS |
| 4 | Bounce at the start cell | `td-sim/tests/flight_e1.rs::vector_4_bounce_sequence`, `::holding_right_at_the_e1m1_start_bounces_in_place` | C-0007 | PASS |
| 5 | Opposite key ignored, steady bounce | `td-sim/tests/flight_e1.rs::vector_5_opposite_key_and_steady_bounce` | C-0007 | PASS |
| 6 | Turn ring | `td-sim/src/flight/tests.rs::steering_turns_one_step_per_tick`, `oracle_flight` | C-0007 | PASS |
| 7 | Down thrust without carry | `td-sim/tests/flight_e1.rs::vector_7_down_without_carry` | C-0008 | PASS |
| — | Vertical bounces, map limits, diagonal steering, joystick predicates, afterburner burst (LIKELY parts) | unit only (`afterburner_bursts_once_per_make_code`, `bounce_swaps_opposite_components`) | none | PARTIAL (no capture; the plans never press KP+/KP0 or use a joystick) |

### sim/tick-order.md (rev 1; T1)

| # | Behaviour | Test(s) | Evidence | Status |
|---|---|---|---|---|
| 1 | 19 ticks in 70 frames | `td-flow/tests/story_parity.rs::a_campaign_flight_runs_at_the_flight_rate` | C-0002 | PASS |
| 2 | Draw page flips every tick | `td-sim/src/flight/tests.rs::page_toggles_every_tick`; draw page compared every tick in the full flights | C-0002, C-0051.. | PASS |
| — | Whole step order | full flights (78 flights, every tick) | C-0051..C-0159 | PASS |

### sim/hit-flash.md (rev 4; T0 DAC, T1 timing)

| # | Behaviour | Test(s) | Evidence | Status |
|---|---|---|---|---|
| 1 | E3 end-sequence flash 17, 15, …, 0 | `oracle_e3.rs::e3_flight_8_matches_c0041` does not compare the hit flash; the E1 end sequence (same code) is compared every tick in `oracle_e1_full.rs::flight_01_idle_death_matches_oracle` | C-0041; C-0157 | PARTIAL |
| 2–3 | DAC during and after the flash | `td-render/src/flight_palette.rs::hit_flash_decays_and_restores_per_mode` (synthetic); no captured frame compared | C-0041 fb | PARTIAL |
| 4 | Kill at sq 25: 13, 11, …, 0 | `td-sim/tests/oracle_ships.rs::mission_ships_match_c0009_tick_by_tick` | C-0009 | PASS |
| 5–6 | Mode 1 and mode 0 kills | `td-sim/src/flight/hit_flash.rs::black_box_kills` | C-0023, C-0033 | PASS |
| 7–12 | Static decay and distance rules (wrapped square) | `td-sim/src/flight/hit_flash.rs::distance_table`, `::decay_per_mode`, `::wrapped_and_negative_squares`, `::integer_round_sqrt_equals_real48` | static | PASS |

### sim/mission-setup.md (rev 2; T1)

| # | Behaviour | Test(s) | Evidence | Status |
|---|---|---|---|---|
| 1 | Flight 1 setup | `td-sim/tests/oracle_ships.rs::mission_ships_match_c0009_tick_by_tick`, full flight 1 | C-0009, C-0051 | PASS |
| 2 | Flight 12 setup (class patch, late draw) | full flight 12, `oracle_e1_flights.rs` flight 12 | C-0027, C-0062 | PASS |
| §3.3 | Class patches persist for the run (OB-018) | none (every capture is a direct start) | none | NOT COVERED |

### sim/mission-ships.md (rev 5; T1)

| # | Behaviour | Test(s) | Evidence | Status |
|---|---|---|---|---|
| 1 | One-tick prediction of every slot | `td-sim/tests/oracle_ships.rs::mission_ships_match_c0009_tick_by_tick` | C-0009 | PASS |
| 2 | Slot 1 path following and speed | `td-sim/src/flight/ships/tests.rs::vector_2_path_following_and_speed`, `oracle_ships` | C-0009 | PASS |
| 3 | Attack order and detour | `td-sim/src/flight/ships/tests.rs::blocked_direct_step_starts_a_detour`, `oracle_ships` | C-0009 | PASS |
| 4 | In-line fire and return | `td-sim/src/flight/ships/tests.rs::attack_in_line_fires_and_then_returns`, `oracle_ships` | C-0009 | PASS |
| 5 | Enemy shot spawns | `oracle_ships`; full flights | C-0009 | PASS |
| 6 | Setup | `oracle_ships` | C-0009 | PASS |

### sim/weapons.md (rev 5; T1)

| # | Behaviour | Test(s) | Evidence | Status |
|---|---|---|---|---|
| 1 | Volleys every second tick | `td-sim/src/flight/tests.rs::weapons_vector_1_volleys_every_second_tick` | static | PASS |
| 2 | Shields absorb | `td-sim/src/flight/tests.rs::weapons_vector_2_shields_absorb` | static | PASS |
| 3 | Player hit | `td-sim/src/flight/tests.rs::weapons_vector_3_player_hit` | static | PASS |
| 4–6 | Volley offsets, missile, heading 6 | `td-sim/tests/oracle_weapons.rs::player_weapons_match_the_original` | C-0011 | PASS |
| 7 | Front-end bytes after a played story (OB-017, E1) | `td-flow/tests/front_end_state.rs::mission_3_after_its_story_matches_c0160`, `::campaign_start_through_the_launcher_matches_c0002`, `td-sim/src/flight/front_end.rs::direct_start_bytes_follow_the_construction_table` | C-0160, C-0002 | PASS |
| 8 | `g_Weapon` 0 in flight 19 (BUG-2) | `td-sim/src/flight/tests.rs::g_weapon_is_carried_from_flight_to_flight`, full flight 19 | C-0069 | PASS |
| 9 | E2/E3 two-digit muzzle bytes (§3.3.2, §3.3.3) | `td-sim/tests/oracle_front_end.rs` (all 58 whole-flight captures), `td-flow/tests/front_end_state.rs` (C-0015, C-0036, C-0180, C-0121, C-0128), the E2/E3 full-flight L5 tests with built bytes (IM-20, after this review) | C-0120..C-0159, C-0015, C-0036, C-0180 | PASS |
| 10 | Homing missile step 3 (U-421) | `td-sim/src/flight/shots/tests.rs::homing_kill_is_step_3_only` | static | PASS |
| — | Missiles and Tab inside whole flights | none (the plans never select missiles; C-0011 is short) | C-0011 only | PARTIAL |

### sim/mission-logic.md (rev 3; T1) — Episode 1

| # | Behaviour | Test(s) | Evidence | Status |
|---|---|---|---|---|
| 1 | Flight 1 first tick | `td-sim/tests/oracle_mission.rs::flight_1_mission_logic_matches_c0010`, `td-sim/src/flight/logic/tests.rs::setup_posts_the_objective_and_sets_the_home_marker` | C-0002, C-0010 | PASS |
| 2–4 | `Msn_NearLive` | `td-sim/src/flight/logic/tests.rs::vectors_2_to_4_near_live_uses_the_view_tile` | static | PASS |
| 5–10 | Flight 1 phases 1 → 5, ticker gate, respawn distance | `td-sim/tests/oracle_mission.rs::flight_1_mission_logic_matches_c0010`, `logic/tests.rs::flight_1_phases` | C-0010 | PASS |
| 11 | Flight 3 outcome | `td-sim/src/flight/logic/tests.rs::flight_3_montgomery`, full flight 3 | C-0053 | PASS |
| 12 | Flight 18 bomb at t + 1000 | `td-sim/src/flight/logic/tests.rs::flight_18_bomb_after_1000_ticks` pins **t + 999** and says the vector is off by one | static | PARTIAL (spec vector and test disagree; no capture of the bomb) |
| — | Flights 1–20 scripts to success, setup to teardown | `td-sim/tests/oracle_e1_full.rs` (20 flights), `oracle_e1_flights.rs` (first ~387 ticks, 4 comm runs), `replays_e1.rs` | C-0017..C-0040, C-0051..C-0069, C-0090 | PASS |
| — | Death, end sequence, in-flight pause/F5/Esc menu | `oracle_e1_full.rs::flight_01_idle_death_matches_oracle`, `::flight_01_modal_screens_match_oracle` | C-0157, C-0158 | PASS |
| — | Flight 21 (scripted, no mission file; U-044) | `td-flow/tests/e1_block_flow.rs` (flow only) | none | NOT COVERED |
| — | Teardown outcome 0 paths, failures (lost) in flights 13, 17, 18, 20 | unit tests only (`logic/tests.rs::outcomes`, `::flight_18_bomb_after_1000_ticks`) | none | PARTIAL |

### sim/e2-mission-setup.md (rev 1; T1)

| # | Behaviour | Test(s) | Evidence | Status |
|---|---|---|---|---|
| 1 | Flight 1 setup | `td-sim/tests/oracle_e2.rs::e2_flight_1_matches_c0015`, full flight E2 1 | C-0015, C-0120 | PASS |
| 2 | Flight 9 setup (map set 2) | `td-sim/tests/oracle_e2.rs::e2_flight_9_matches_c0016` | C-0016, C-0133 | PASS |
| 3 | Flight 17 setup | full flight E2 17 (setup compared) | C-0143 | PASS |
| 4–5 | Flights 8 and 7 | `td-sim/src/flight/logic/e2/tests.rs::setup_vectors_flights_7_and_8`, full flights | C-0131, C-0132 | PASS |
| 6 | Flight 2 slot 8 (OB-035) | full flight E2 2 (tick 1, every slot) | C-0121 | PASS |
| 7 | Tile attributes, both sets | `td-sim/src/flight/episode.rs::e2_tile_attribute_sets` | map files | PASS |
| 8 | Guidance home points | none (table in `td_render::hud`) | static | PARTIAL |
| 9 | Flight song | `td-audio/src/music_cues.rs::flight_song_rotates_by_mission`, `::episode_2_table` | static | PASS |
| 10 | F5 board, flight 1 | `td-flow/src/flight_screens/map.rs::e2_board` | static | PASS |

### sim/e2-mission-ships.md (rev 1; T1)

| # | Behaviour | Test(s) | Evidence | Status |
|---|---|---|---|---|
| 1–4 | C-0015 ticks 0–2: cursors, engagements | `td-sim/tests/oracle_e2.rs::e2_flight_1_matches_c0015`, `logic/e2/tests.rs::vector_c1_flight_1_tick_1_engagements` | C-0015 | PASS |
| 5 | C-0016 ticks 0–2 | `td-sim/tests/oracle_e2.rs::e2_flight_9_matches_c0016` | C-0016 | PASS |

### sim/e2-mission-logic.md (rev 1; T1)

| # | Behaviour | Test(s) | Evidence | Status |
|---|---|---|---|---|
| C1 | Flight 1 tick-1 engagements | `logic/e2/tests.rs::vector_c1_flight_1_tick_1_engagements`, `oracle_e2.rs::e2_flight_1_matches_c0015` | C-0015 | PASS |
| 1–3 | `Msn_NearLive` on the player tile | `logic/e2/tests.rs::vectors_1_to_3_near_live_uses_the_player_tile` | static | PASS |
| 4–9 | Base tests, flights 1, 5–9 | `logic/e2/tests.rs::base_tests` | static; full flights | PASS |
| 10 | Flight 3 arrival | `logic/e2/tests.rs::vector_10_flight_3_arrival` | static | PASS |
| 11 | Flight 3 truck shot down (step 6 vs 10 timing) | none | static | NOT COVERED |
| 12 | Flight 9 truck exit | `logic/e2/tests.rs::outcome_tests` (exit at (177, 169)) | static | PASS |
| 13 | Flight 8 wave 2 | `logic/e2/tests.rs::vector_13_flight_8_wave_2` | static | PASS |
| 14 | Flight 5 announcement after the ticker | full flight E2 5 (t157 phase 2) | C-0124 | PASS |
| 15 | Flight 7 comm viewer | `logic/e2/tests.rs::comm_viewer_table` | static | PASS |
| 16 | Flight 1 announcement waits for an idle ticker | full flight E2 1 (t821 phase 2) | C-0120 | PASS |
| 17 | Flight 2 slot 8 in tick 1 (U-243) | full flight E2 2 | C-0121 | PASS |
| 18–19 | Outcomes, flights 8 and 16 (OB-025) | `logic/e2/tests.rs::outcome_tests` | static | PASS |
| 10–20: 1–2 | Base tests, flights 10–19 | `logic/e2/tests.rs::base_tests` | static | PASS |
| 10–20: 3 | Flight 10 phase 18 | `logic/e2/tests.rs::flight_10_phase_18` | static; C-0134 | PASS |
| 10–20: 4–5 | Outcomes, flights 10, 11 (OB-040) | `logic/e2/tests.rs::outcome_tests` | static | PASS |
| 10–20: 6 | Flight 13 `Random(9)` and ghost fire (OB-039) | `logic/e2/tests.rs::flight_13_random_and_ghost_fire` | static (U-246 open) | PASS |
| 10–20: 7 | Flight 16 transmission only after M (OB-038, U-245) | none | static | NOT COVERED |
| 10–20: 8 | Flight 16 slot 2 exit | `logic/e2/tests.rs::outcome_tests` | static | PASS |
| 10–20: 9 | Flight 17 lost after M2[15] | `logic/e2/tests.rs::flight_17_lost_after_the_message` | static | PASS |
| 10–20: 10 | Flight 17 transports escape | full flight E2 17 | C-0143 | PASS |
| 10–20: 11 | Flight 18 base destruction timeline | `logic/e2/tests.rs::flight_18_base_destruction` | static | PASS |
| 10–20: 12 | Flight 18 second wave | full flight E2 18 (t477 phase 2) | C-0144 | PASS |
| 10–20: 13 | Flight 19 announcement range 45 | full flight E2 19 (t199 phase 2) | C-0145 | PASS |
| 10–20: 14 | Flight 20 base test | `logic/e2/tests.rs::base_tests` | static; C-0146 | PASS |
| — | All 20 E2 flights to success | `oracle_e2_full.rs` (20), `replays_e2.rs` (20) | C-0120..C-0146 | PASS |
| — | E2 deaths, aborts, outcome-0 and lost paths in a capture | unit and flow tests only | none | PARTIAL |

### sim/e3-mission-setup.md (rev 1; T1)

| # | Behaviour | Test(s) | Evidence | Status |
|---|---|---|---|---|
| 1 | Flight 1 setup | `td-sim/tests/oracle_e3.rs::e3_flight_1_matches_c0036` | C-0036, C-0125 | PASS |
| 2 | Flight 8 setup | `td-sim/tests/oracle_e3.rs::e3_flight_8_matches_c0041` | C-0041, C-0135 | PASS |
| 3 | Flight 9 | `td-sim/tests/flight_e3.rs::e3_flight_9_setup`, full flight E3 9 | C-0136 | PASS |
| 4 | Flight 12 | `td-sim/tests/flight_e3.rs::e3_flight_12_setup`, full flight | C-0148 | PASS |
| 5 | Flight 5 | `td-sim/tests/flight_e3.rs::e3_flight_5_setup`, full flight | C-0129 | PASS |
| 6 | Flight 7 (inherited phase, OB-043) | `td-sim/tests/flight_e3.rs::e3_flight_7_setup` | static (flight 7 not captured) | PARTIAL |
| 7 | Flight 17 after flight 16 | `td-sim/tests/flight_e3.rs::e3_flight_17_restores_the_class_tables` | static | PASS |
| 8 | Flight 18 markers (OB-052) | `td-sim/tests/flight_e3.rs::e3_flight_18_setup` | C-0154 | PASS |
| 9 | Flight 15 Kreel's shields (OB-048) | `td-sim/tests/flight_e3.rs::e3_flight_15_setup` | C-0151 | PASS |
| 10 | Tile attributes | `td-sim/src/flight/episode.rs::e3_tile_attributes` | map files | PASS |
| 11–12 | Start views, home points | `td-sim/src/flight/episode.rs::e3_files_classes_views_and_home_points` | static | PASS |
| 13 | Flight song | `td-audio/src/music_cues.rs::episode_3_table` | static | PASS |
| 14 | F5 board, flight 10 | `td-flow/src/flight_screens/map.rs::e3_board` | static | PASS |

### sim/e3-mission-ships.md (rev 1; T1)

| # | Behaviour | Test(s) | Evidence | Status |
|---|---|---|---|---|
| 1–4 | C-0036 ticks 0–58 | `td-sim/tests/oracle_e3.rs::e3_flight_1_matches_c0036` | C-0036 | PASS |
| 5–6 | C-0041 ticks 0–2, keypad route | `td-sim/tests/oracle_e3.rs::e3_flight_8_matches_c0041` | C-0041 | PASS |
| 7–9 | Hazard damage, i8 wrap (OB-054) | `oracle_e3.rs::e3_flight_8_matches_c0041`, `logic/e3/tests.rs::hazard_damage_wraps_through_the_end_sequence` | C-0041 | PASS |

### sim/e3-mission-logic.md (rev 1; T1)

| # | Behaviour | Test(s) | Evidence | Status |
|---|---|---|---|---|
| 1–9: C1 | Flight 1 tick 1 | `oracle_e3.rs::e3_flight_1_matches_c0036` | C-0036 | PASS |
| 1–9: C2 | Tick 1 of flights 1–9: no rule fires | full flights E3 1–6, 8, 9 | C-0125..C-0136, C-0156 | PASS |
| 1–9: 1 | `Msn_NearLive` (E3 numbers) | shared primitive tested for E1/E2 only | static | PARTIAL |
| 1–9: 2–4 | Base tests, flights 1, 6, 7, 9 | `logic/e3/tests.rs::base_tests` | static | PASS |
| 1–9: 5–7 | Flight 1 waves, completion set (OB-045) | `logic/e3/tests.rs::flight_1_waves` | static; C-0125 | PASS |
| 1–9: 8–9 | Flight 5 arrival, lost rule | `logic/e3/tests.rs::flight_5_arrival_and_outcome`, `::flight_5_lost_rule` | static; C-0129 | PASS |
| 1–9: 10–11 | Flight 7 inherited phase (OB-043) | `logic/e3/tests.rs::flight_7_phase` | static | PASS |
| 1–9: 12–15 | Flight 8 silent window, attack phase, outcome (OB-044) | `logic/e3/tests.rs::flight_8` | static; C-0135 | PASS |
| 1–9: 16 | Flight 9 truck exit | full flight E3 9 (only the captured path) | C-0136 | PARTIAL |
| 1–9: 17–20 | Flights 2, 3, 4, 6 completion and outcome branches | full flights E3 2, 3, 4, 6 (only the captured paths) | C-0126..C-0128, C-0156 | PARTIAL |
| 10–21: C1 | Tick 1 of flights 10–19, 21 | full flights E3 10–19 (not 21) | C-0147..C-0159 | PASS |
| 10–21: 1–5 | Base tests | `logic/e3/tests.rs::base_tests` | static | PASS |
| 10–21: 6–9 | Flight 10 counter phases, truck, outcome | `logic/e3/tests.rs::flight_10`, `::timers` | static; C-0159 | PASS |
| 10–21: 10–11 | Flight 12 | `logic/e3/tests.rs::flight_12` | static; C-0148 | PASS |
| 10–21: 12 | Flight 12 cells at tick 6480 | `logic/e3/tests.rs::flight_12_cells_return_at_tick_6480` | static | PASS |
| 10–21: 13 | Flight 14 timeline | `logic/e3/tests.rs::flight_14_timeline` | static; C-0150 | PASS |
| 10–21: 14–15 | Flight 15 Kreel | `logic/e3/tests.rs::flight_15` | static; C-0151 | PASS |
| 10–21: 16–17 | Flight 16 forced transmission (pages 0 and 1) | `logic/e3/tests.rs::flight_16_chain` (vector 16); vector 17 (draw page 1) not asserted | static; C-0152 | PARTIAL |
| 10–21: 18 | Flight 16 magenta fade values | `td-render/src/flight_palette.rs::magenta_fade_and_tint` | static | PASS |
| 10–21: 19 | Flight 16 freeze tables | `logic/e3/tests.rs::flight_16_freeze` | static | PASS |
| 10–21: 20 | Esc menu after phase 5: entry 0 tinted (OB-049) | `td-render/src/flight_palette.rs::fades_round_halves_up` | static | PASS |
| 10–21: 21 | White flash after phase 5 | `td-render/src/flight_palette.rs::white_flash_frames` | static | PASS |
| 10–21: 22 | F5/F6 blocked after phase 5 | `logic/e3/tests.rs::flight_16_chain` | static | PASS |
| 10–21: 23 | Flight 17 restores the tables | `td-sim/tests/flight_e3.rs::e3_flight_17_restores_the_class_tables` | static | PASS |
| 10–21: 24 | Flight 16 retry frozen from tick 1 | not reproduced on purpose: OB-028 fix-all, pinned by `td-sim/tests/flight_e3.rs::e3_ob028_freeze_never_leaks` | static | N/A |
| 10–21: 25 | Flight 18 timer and rule A | `logic/e3/tests.rs::flight_18`, `::timers` | static; C-0154 | PASS |
| 10–21: 26–27 | Flight 19 Osyluth | `logic/e3/tests.rs::flight_19` | static; C-0155 | PASS |
| 10–21: 28 | No outcome test in 11, 13–19, 21 | `logic/e3/tests.rs::comm_table_and_outcomes` | static | PASS |
| 10–21: 29 | Timed events inert elsewhere | `logic/e3/tests.rs::timed_events_are_inert_elsewhere` | static | PASS |
| — | 18 E3 flights to success | `oracle_e3_full.rs` (18), `replays_e3.rs` (18) | C-0125..C-0159 | PASS |
| — | Flights 7 and 21 in a capture (U-481) | static tests only | none | NOT COVERED |
| — | U-430 hangar re-entry, flight 10 | none (C-0183 schedule not replayed in td-sim) | C-0182, C-0183 | NOT COVERED |

### flow/screens.md (rev 3; T0 layouts, T1 retry `Random`, T2 timing)

| # | Behaviour | Test(s) | Evidence | Status |
|---|---|---|---|---|
| 1 | Main menu after start-up | `td-flow/tests/story_parity.rs::main_menu_matches_the_original`, `::main_menu_boxes_and_fades` | C-0002 f997 | PASS |
| 2 | Retry prompt: one `Random(10)` | `td-flow/tests/story_parity.rs::a_retry_draws_one_random_number`, `::retry_prompt_matches_the_original` | C-0002 f6300 | PASS |
| 3 | Esc on the title card | `td-flow/tests/story_parity.rs::esc_on_the_title_card_goes_straight_to_the_flight` | static | PASS |
| 4 | Debrief layout (lines at y 45..140) | `td-flow/src/campaign.rs::debrief_tiers` checks the tier only; no pixel test | static (no capture) | PARTIAL |
| §2.1 | Fade speed by palette (BUG-6, U-461) | `td-flow/tests/story_parity.rs::the_first_title_card_fades_at_one_frame_per_level`, `::a_title_card_after_the_story_so_far_fades_at_two_frames_per_level` | C-0002, C-0170 | PASS |
| §3.3 | Hint screens | `td-flow/tests/story_parity.rs::hint_1_matches_the_original` | C-0002 | PASS |
| §3.2 | Script choice | `td-flow/tests/story_parity.rs::script_choice_matches_the_original` | C-0002 f710 | PASS |
| §3.4–3.5 | Assignment orders, enemy and pilot dossiers | none | no capture | NOT COVERED |
| §3.8–3.10 | Story so far, `td1-15`, other stills (`td1-13..20.pic`, `secret.pic`) | none (C-0170 holds the story-so-far run but no frame test) | C-0170 (unused) | NOT COVERED |
| §3.12 | Esc in the pre-flight story | `td-flow/tests/story_parity.rs::keys_typed_ahead_carry_into_the_next_screen`, `td-flow/tests/e1_block_flow.rs::block_19_esc_before_flight_21_jumps_to_flight_21` | static | PASS |

### flow/episode-program-flow.md (rev 6; T2 order, T1 state, T3 hand-off)

| # | Behaviour | Test(s) | Evidence | Status |
|---|---|---|---|---|
| 1 | Mission-1 path to the flight | `td-flow/tests/story_parity.rs::title_and_hangar_cards_match_the_original`, `td-flow/tests/front_end_state.rs::campaign_start_through_the_launcher_matches_c0002` | C-0002 | PASS |
| 2 | Idle death, retry prompt | `oracle_e1_full.rs::flight_01_idle_death_matches_oracle`, `story_parity.rs::retry_prompt_matches_the_original` | C-0157, C-0002 | PASS |
| 3 | Debrief tiers | `td-flow/src/campaign.rs::debrief_tiers` | static | PASS |
| 4 | Music index 12, 13 | `td-audio/src/music_cues.rs::cue_table` | static | PASS |
| §4.4 | Blocks 19–21 (U-081, U-207, U-420) | `td-flow/tests/e1_block_flow.rs` (5 tests) | static | PASS |
| §4.5 | Endings A, B, C and the episode end | `td-flow/tests/story_parity.rs::death_without_retry_is_game_over`, `::every_block_plays_to_the_end` (flow only); no capture (U-080 open) | none | PARTIAL |

### flow/launcher.md (rev 1; T3 launcher, T1 order)

| # | Behaviour | Test(s) | Evidence | Status |
|---|---|---|---|---|
| 1 | Intro, then Episode 1 | `td-flow/tests/launcher_shell.rs::the_intro_timeline_with_the_original_files`, `::the_intro_hands_over_to_episode_1_and_esc_skips_it` | C-0001 | PASS |
| 2 | `td.cfg` configuration screen | none (T3: no configuration screen) | C-0048 | N/A |
| 3–4 | Menu items 6 and 7 (about, order) | `td-flow/tests/launcher_shell.rs::e1_items_6_and_7_show_the_pictures_with_intro_cmf` | C-0049, C-0050 | PASS |

### flow/intro.md (rev 3; T0 frames, T1 flyover state, T2 timing)

| # | Behaviour | Test(s) | Evidence | Status |
|---|---|---|---|---|
| 1–2 | File opens; date card | `td-flow/src/intro/tests.rs::timeline_follows_the_p1_frame_counts` | C-0047 | PASS |
| 3 | Full fades, 65 frames | `td-flow/tests/intro_parity.rs::vector_3_full_fades_are_65_frames` | C-0047 | PASS |
| 4 | Flyover state per step | `td-flow/tests/intro_parity.rs::vector_4_flyover_state_matches_every_ram_dump`, `td-flow/src/intro/flyover.rs::spec_vector_4_state_after_steps` | C-0047 | PASS |
| 5 | Step cadence | `td-flow/tests/intro_parity.rs::vector_5_step_cadence` | C-0047 | PASS |
| 6–7 | Every captured frame (OB-058, OB-059) | `td-flow/tests/intro_parity.rs::vectors_3_6_7_every_captured_frame`, `::ob058_fragment_in_classic_only` | C-0047 | PASS |
| 8 | Esc, Esc | `td-flow/src/intro/tests.rs::spec_vector_8_esc_esc` | C-0049, C-0050 | PASS |
| 9–10 | Effect and music bytes | `td-flow/tests/intro_parity.rs::vectors_9_10_effect_and_music_bytes_match_c0081` | C-0081 | PASS |

### flow/save-screens.md (rev 1; T0 image, T1 file bytes, T2 timing)

| # | Behaviour | Test(s) | Evidence | Status |
|---|---|---|---|---|
| 1–4 | Save into an empty set; restore | `td-flow/tests/save_parity.rs::save_into_an_empty_set_matches_c0042` | C-0042 | PASS |
| 5–6 | Shipped files: restore, save, Esc ignored (OB-056) | `td-flow/tests/save_parity.rs::restore_and_save_with_the_shipped_files_match_c0043` | C-0043 | PASS |
| §4.1 | Missing or short files (OB-055) | `td-formats/src/saves.rs::short_or_missing_files_are_empty_slots`, `td-flow/tests/save_parity.rs::the_menu_restores_from_and_saves_to_the_port_s_own_files` | static | PASS |

### flow/flight-screens.md (rev 5; T0 images, T1 state, T2 timing)

| # | Behaviour | Test(s) | Evidence | Status |
|---|---|---|---|---|
| 1–3 | Computer menu, ship library, help | `td-flow/tests/flight_screens_parity.rs::computer_menu_library_and_help_match_c0012` | C-0012 | PASS |
| 4 | Pause line | `…::pause_line_and_ir_notice_match_c0012_c0014` | C-0012 f5724 | PASS |
| 5–6 | F5 tactical map | `…::tactical_map_matches_c0012`, `td-flow/src/flight_screens/map.rs::parses_ranges_and_lists` | C-0012 | PASS |
| 7–8 | IR palette formulas | `td-render/src/flight_palette.rs::mode_1_start_formula_and_entry_37`, `::mode_2_formula` | static | PASS |
| 9–11 | Transmission screen | `…::transmission_screen_matches_c0013`, `::transmission_markers_are_the_prompt_strings` | C-0013 | PASS |
| 12–16 | IR palettes, IR notice, pause in IR, menu in IR (OB-029) | `…::ir_palettes_match_c0014`, `::pause_line_and_ir_notice_match_c0012_c0014`, `::ir_menu_shows_the_normal_palette_c0014` | C-0014 | PASS |
| 17 | Helicopter sprite by page | `td2192/src/flight.rs::player_sprite_follows_the_page_only_when_drawn_late` (rule); no C-0027 frame compared | C-0027 | PARTIAL |
| 18–19 | Mission-specific sprite loads; library reload (OB-057) | `td2192/src/flight.rs::mission_sprites_land_where_flight_screens_says` | C-0022, C-0027, C-0035, C-0045 (values) | PASS |
| 20 | All 16 heading blits | `td-render/tests/heading_parity.rs::vector_20_every_heading_matches_c0080` | C-0080 | PASS |
| §4.1 | Menu fades 2 frames per level, ramp 11 (BUG-8) | `…::menu_fades_take_two_frames_per_level_and_the_ramp_eleven_steps_per_frame` | C-0012 | PASS |
| §4.10 | Flight-17 cut-scene, flight-19 V.C.I. screen (BUG-3) | `td-flow/tests/interlude_parity.rs::flight_17_cut_scene_matches_c0067`, `::flight_19_vci_screen_matches_c0069` | C-0067, C-0069 | PASS |
| §4.5.3 | Flight-start fade-in (34 frames before tick 1) | none: not ported (M12-timing §7) | C-0002 | NOT COVERED |
| — | Comm typing pace (M12-timing row 13) | none | not measured | NOT COVERED |

### flow/e2-flight-screens.md (rev 1; as flight-screens)

| # | Behaviour | Test(s) | Evidence | Status |
|---|---|---|---|---|
| 1 | Extraction method reproduces E1's tables | none (RE method check) | static | N/A |
| 2 | String counts per run | `td-flow/src/flight_screens/comm.rs::e2_table_shapes`, `::e3_table_shapes`, `::table_shapes` | static | PASS |
| 3 | E2 transmission 4 | `td-flow/tests/e2_flight_screens.rs::transmission_4_table_matches_c0044_ram`, `::transmission_4_screen_matches_c0044` | C-0044 | PASS |
| 4 | E2 ship library, flight 11 (U-313) | `td-flow/tests/e2_flight_screens.rs::ship_library_matches_c0044` | C-0044 | PASS |
| 5 | E3 library: nine blocks | `td-flow/tests/e3_flight_screens.rs::e3_ship_library_shows_all_nine_blocks` | static | PASS |

### flow/e2-program-flow.md (rev 3; T2 order, T1 state, T0 menu colours)

| # | Behaviour | Test(s) | Evidence | Status |
|---|---|---|---|---|
| 1–2 | Music indices, flight songs | `td-audio/src/music_cues.rs::episode_2_table`, `::flight_song_rotates_by_mission` | static | PASS |
| 3 | Esc at the first card goes to the flight | `td-flow/tests/e2_front_end.rs::esc_at_the_first_card_goes_straight_to_the_flight` | C-0015 | PASS |
| 4 | Block 1 without Esc | `td-flow/tests/e2_front_end.rs::block_1_follows_the_spec` | static | PASS |
| 5–6 | Menu colours (OB-041), block 1 completed | `…::start_up_to_the_menu_and_a_completed_block`, `::main_menu_matches_c0015_and_c0016`, `::menu_after_a_restore_matches_c0016` | C-0015, C-0016 | PASS |
| 7 | Block 8 outcome 0, Esc at the retry | `…::blocks_8_and_17_replay_after_a_declined_outcome_retry` | static | PASS |
| 8 | Block 20 completed | `…::episode_complete_and_game_over_bookkeeping`, `::block_20_end_sequence_plays_death_looping` | static | PASS |
| 9 | Block 3 death declined → ending A | `…::death_declined_is_ending_a` | static | PASS |
| 10 | Block 17 lost → ending C | `…::lost_flights_17_and_18` | static | PASS |
| 11 | Menu item 6: about screen 30 s | none for E2 (E1 items in `launcher_shell`) | static | NOT COVERED |
| 12 | 124 dialogue ids | `td-flow/src/campaign_e2.rs::dialogue_coverage` | file scan | PASS |
| §4.2 | Menu fades and ramp | `td-flow/tests/menu_pacing.rs::e2_menu_fades_and_ramp_match_c0120` | C-0120 | PASS |
| — | Scene cards (marauder, tdoffic2, hyde) | `…::marauder_card_matches_c0015`, `::tdoffic2_card_matches_c0016`, `::hyde_card_matches_c0046` | C-0015, C-0016, C-0046 | PASS |
| — | Endings, dossier, end card pixels | flow tests only | no capture (U-249) | PARTIAL |

### flow/e3-program-flow.md (rev 3; as e2-program-flow)

| # | Behaviour | Test(s) | Evidence | Status |
|---|---|---|---|---|
| 1–2 | Music indices, flight songs | `td-audio/src/music_cues.rs::episode_3_table` | static | PASS |
| 3 | New game with Esc at `shuttle0` | `td-flow/tests/e3_front_end.rs::esc_at_the_first_card_goes_straight_to_the_flight`, `::shuttle0_card_matches_c0036` | C-0036 | PASS |
| 4 | Esc in the menu fade-out is flushed | `…::esc_at_the_first_card_goes_straight_to_the_flight` | static | PASS |
| 5–6 | Menu start-up, block 1 completed | `…::start_up_to_the_menu_and_a_completed_block`, `::main_menu_matches_c0036` | C-0036 | PASS |
| 7 | Block 2 outcome 0, Esc (OB-027) | `…::replay_blocks_offer_the_block_again` | static | PASS |
| 8 | Block 10 decline | `…::block_10_decline_keeps_the_mission`, `::block_10_decline_and_game_over_bookkeeping` | static | PASS |
| 9 | Block 8 decline counts as success | `…::block_8_decline_counts_as_a_success` | static | PASS |
| 10 | Block 6: F6 then F7 | `…::block_6_flies_f6_then_f7`, `::block_6_and_episode_end_bookkeeping` | static | PASS |
| 11 | Block 11: F21 death restarts F11 | `…::block_11_flies_f11_then_f21` | static | PASS |
| 12 | Block 19 end card | `…::block_19_ends_with_the_end_card_and_no_debrief` | static | PASS |
| 13 | OB-050 shortcut | `…::esc_on_the_about_screen_skips_block_1`, `::block_1_skip_shortcut` | static | PASS |
| 14 | OB-051 restore of mission 7 | `…::missions_without_a_block_add_the_stale_kills` | static | PASS |
| 15 | 111 dialogue ids | `td-flow/src/campaign_e3.rs::dialogue_coverage` | file scan | PASS |
| 16 | Menu items 6 and 7 (30 s / 45 s) | `td-flow/src/scenes_e3.rs::waits` (wait lengths only) | static | PARTIAL |
| §4.2 | Menu fades and ramp | `td-flow/tests/menu_pacing.rs::e3_menu_fades_and_ramp_match_c0125` | C-0125 | PASS |
| — | Cards, restore screen | `…::maraud3_card_matches_c0041`, `::restore_screen_and_menu_match_c0041` | C-0036, C-0041 | PASS |

### frontend/credits.md (rev 1; T3)

| # | Behaviour | Test(s) | Evidence | Status |
|---|---|---|---|---|
| 1 | Mode A = intro flyover | `td-flow/tests/launcher_shell.rs::credits_mode_a_and_b`, `td-flow/src/credits.rs::pages_fade_hold_and_end` | C-0047 (through intro) | N/A |
| 2 | Mode B page layout | `td-flow/src/credits.rs::spec_vector_2_page_layout` | this spec | N/A |

### frontend/port-front-end.md (rev 2; T3)

| # | Behaviour | Test(s) | Evidence | Status |
|---|---|---|---|---|
| §2–§5 | Start screen, options, game data (folder or ZIP), controls, controller navigation, colour-blind guidance | `td-flow/src/front/start.rs` (10), `front/options.rs` (12 + 1 ignored), `front/data.rs` (2), `front/ui.rs` (6), `settings.rs` (6), `controls.rs` (5), `td-assets/src/archive.rs` (5), `import.rs` (3), `td2192/src/data.rs` (6), `td-render/src/enhanced/guidance.rs` (3) | design | N/A |
| §5 | Original screens unchanged except the listed controller answers | `td-flow/src/main_menu.rs::a_controller_answers_the_boxes` | design | N/A |

### enhanced/presentation.md (rev 4; T3, Enhanced only)

| # | Behaviour | Test(s) | Evidence | Status |
|---|---|---|---|---|
| §8 | Classic invariance with every key off | `td-render/tests/enhanced_parity.rs` (2), `td2192/src/enhanced.rs` (5) | C-0002 f3543 | N/A (passes; it is what keeps Classic exact) |
| §4–§6 | Widescreen, upscalers, lighting, bloom | `td-render/src/enhanced/*` (37), `td-render/src/flight.rs::widescreen_*`; timing tests ignored by design | design | N/A |

## 5. Ignored tests and code ASSUMPTIONs

### 5.1 `#[ignore]` tests (4; none refers to a U-id)

| Test | Reason | Note |
|---|---|---|
| `td-flow/src/front/options.rs::dump_pages` | writes images for a visual check (`TD2192_UI_DUMP`) | developer tool |
| `td-render/tests/enhanced_lighting_timing.rs::lighting_and_bloom_fit_the_budget` | timing, release build on an idle machine | Enhanced performance (M12-12) |
| `td-render/tests/enhanced_timing.rs::stage_timings` | timing | Enhanced performance |
| `td2192/src/flight/lights.rs::render_lit_frames` | renders lit frames for tuning | developer tool |

`td-sim/tests/oracle_e1_flights.rs::class_ffh_slots_setup_shields_and_armour` (BUG-1) is no longer ignored and passes. [M8-missions](M8-missions.md) D1 now says so (§8).

### 5.2 ASSUMPTIONs and TODOs in the code

There is no `TODO` or `FIXME` in `crates/` or `tools/`. Every ASSUMPTION:

| Place | U-id | U-id status | Finding |
|---|---|---|---|
| `td-flow/src/flight_screens/mod.rs:158` (E2 ship library uses E1's line table) | U-313 | RESOLVED 2026-11-05 (e2-flight-screens §3.3) | **Stale.** The behaviour is verified by `e2_flight_screens.rs::ship_library_matches_c0044` (C-0044, 36 of 36 lines). Replace the comment with the spec citation |
| `td-flow/src/timing.rs:45` (`DEFAULT_TICK_HZ` = 70 Hz) | U-001 | OPEN, but measured (19.2015 Hz; ADR-0007 proposed) | **Stale.** The constant is exported and never used (M12-timing §6). Remove it once ADR-0007 is accepted |
| `td-sim/src/flight/logic.rs:1541` (flight 19: "armour := 1" is the player's) | U-146 | OPEN | **Closable.** C-0069 (full flight 19) compares the player's armour and weapon every tick across the switch at tick 205 and passes. Close U-146 and drop the ASSUMPTION |
| `td-render/src/fade.rs:3` (linear fade; "`fades.md`, to be written") | none (M5-02) | — | **Stale header.** Fades are specified in flow/screens rev 3 §2.1 and dialogue-player §3.4, and implemented (`frames_per_level`) |
| `td-sim/src/flight/logic.rs:561` (mission explosions use slots 1..20, no `Random` when none is idle) | U-145 | OPEN | Not stale. Full flights 7, 17 and 20 (C-0057, C-0067, C-0090) pass with this choice; the no-idle-slot case is not exercised |
| `td-flow/src/small_text.rs:32` (one frame per character) | U-062 | OPEN | Not stale. One frame per character at P1 is confirmed (C-0002, `dialogue_parity`); only real hardware is open |
| `td-flow/src/main_menu.rs:24, 558, 575`, `program.rs:680`, `campaign.rs:954` | U-302 | OPEN | Not stale (five E1 front-end details) |
| `td-sim/src/flight/mod.rs:536` (flight-16 afterburner test placement) | U-330 | OPEN | Not stale |
| `td-sim/src/flight/shots.rs:90` (palette at the ghost volleys) | U-246 | OPEN | Not stale. C-0139 (E2 flight 13) never removes the Areo-1, so the ghost volleys are not exercised |
| `td-flow/src/scenes_e3.rs:20` (`maraud3` separators) | U-350 | OPEN | Not stale |
| `td-render/src/fade.rs:38` (crossover 380) | U-461 | OPEN | Not stale |
| `td-audio/src/worx.rs:15` (X / R) | U-470 | OPEN | Not stale |
| `td-audio/src/sfx.rs:186` (effect id 1 sets the priority, plays nothing) | none (U-141) | OPEN | Not stale; cite U-141 |
| `td-flow/src/flight_screens/menu.rs:42` (seven whole retraces per poll) | none | — | T2 |
| `td-flow/src/intro/flyover.rs:366` (BIOS phase chosen to fit C-0047) | none | — | T2, fitted to the capture |
| `td-sim/src/flight/shots.rs:264` (missile writes life 200 into slot 40 before storing) | none | — | C-0011 (`oracle_weapons`) passes |

The first four are the stale or closable ones.

## 6. Open U-ids that affect Classic behaviour

`docs/re/UNKNOWNS.md` has 165 rows. 80 are not resolved. That count leaves out U-270 and U-273, whose status cells start with a bold "RESOLVED", and U-275, closed by the user's OB-028 decision. 13 of the 80 have no known effect on Classic play:

- U-014: unexplained byte in `m18.td3`;
- U-028: `sound_override` string;
- U-031: unit roles;
- U-082: OB-009 is fixed either way;
- U-142: no shipped song uses that WORX data;
- U-161: Real48 `Sqrt`;
- U-186: dead code;
- U-223: DS:16E6;
- U-225: redundant exclusions;
- U-250: WORX left installed after `Halt` (T3);
- U-281: the port fixes OB-055;
- U-460: real Sound Blaster hardware;
- U-481: oracle tooling.

The other 67 are listed here. "Closable" means existing captures or tests already answer it, so a Verifier or Archaeologist can close it without a new run.

| U-id | Pri | Topic | Port today / evidence |
|---|---|---|---|
| U-001 | P1 | Canonical tick rate | 19.2015 Hz; measured (C-0074, C-0010). **Closable** once ADR-0007 is accepted |
| U-009 | P1 | Map collision (ships, shots) | Player, ship and shot collisions are reproduced by C-0007/8, C-0009, C-0011 and 78 full flights. **Likely closable** for E1–E3; vertical bounces are uncaptured |
| U-024 | P1 | Key bindings, joystick mapping | Keys confirmed by the full flights; the joystick was never captured |
| U-044 | P3 | E1 flight 21 | Static only; no capture |
| U-061 | P3 | Small-font `!` lookup | The port draws nothing (OB-004) |
| U-062 | P2 | Typing speed | One character per frame at P1 is confirmed (C-0002); real hardware open |
| U-063 | P3 | `Read(ch)` then `Read(s)` on 3-byte speaker lines (E3) | Port per spec |
| U-080 | P3 | Which condition leads to each E1 game-over path | Flow tests only |
| U-126 | P3 | Byte 1Ah in the dialogue scripts | Port reads a plain byte |
| U-140 | P2 | Does `death.cmf` loop? | Port: looping off |
| U-141 | P3 | Effect id 1 | Port keeps the playing sound |
| U-144 | P2 | Mission globals reset between flights | Port per mission-logic §3.3; **not verifiable with direct starts** (§0 item 3) |
| U-145 | P3 | Mission-explosion slots and `Random` calls | Port: slots 1..20; C-0057, C-0067, C-0090 pass |
| U-146 | P2 | Flight-19 armour := 1 | **Closable** (C-0069, §5.2) |
| U-147 | P3 | Keys during the cut-scene and the V.C.I. screen | See U-450 |
| U-162 | P3 | Detour step with a facing not in {2, 4, 6, 8} loops forever | Would be a hang (fix-all) if reachable |
| U-180 | P3 | Flight-7 guidance rule pre-empts the message | C-0057 passes (phase 100 at t648) |
| U-182 | P2 | Flight-8 soft lock reachable? (OB-022) | Classic preserved |
| U-184 | P3 | Flight-20 lander success rule | C-0090 passes (success at t424). **Likely closable** |
| U-185 | P3 | Ticker text past x = 351 | Port clips |
| U-204 | P2 | E2/E3 double kill (OB-026), palette-step order | Implemented; no capture of a double kill |
| U-206 | P3 | E2/E3 menu details | Partly resolved; C-0120 and C-0125 match the menus |
| U-220 | P3 | F5 closed by typematic repeat | Port ignores the repeat |
| U-221 | P3 | E1 transmissions 2..9 | State confirmed for all nine in full flights (C-0052..C-0068); screen pixels only for transmission 1 (C-0013) |
| U-222 | P3 | Afterburner flame, explosion and shot drawing | Static only |
| U-224 | P3 | Comm inks follow the IR palette | Port follows the code |
| U-226 | P3 | Flight-21 F5 shows stale data | Static |
| U-227 | P3 | Byte before the row buffer | Port 0 (observed) |
| U-228 | P3 | Hint screens flush after the fade-in | Port per spec |
| U-229 | P3 | Main-menu debug leftovers | Port per spec |
| U-230 | P3 | Transmission 2, line 16 box | Port: box A |
| U-231 | P3 | Does the comm viewer fade? | Port: no fade. **Closable:** C-0058 keeps the hit flash (10) across the screen, so it does not fade (M9-playthrough, harness note) |
| U-240 | P3 | E2 empty slots given a class later | 20 E2 full flights pass |
| U-241 | P2 | E2 setup vectors (flights 2, 8, 17) | **Closable:** setups compared in C-0121, C-0132, C-0143 |
| U-242 | P2 | E2 flight-8 slot 11 loops (OB-036) | C-0132: slot 11 gone at t1860, phase 100 at t1861, port equal; check whether that was a kill or a removal |
| U-243 | P3 | E2 flight-2 slot 8 (OB-035) | **Closable:** C-0121 tick 1 shows "slot 8 class FF" and matches |
| U-244 | P3 | Base test during the end sequence | Static |
| U-245 | P3 | E2 flight-16 transmission (OB-038) | Not tested (§4) |
| U-246 | P2 | E2 flight-13 ghost volleys (OB-039) | ASSUMPTION in `shots.rs:90`; not exercised |
| U-247 | P3 | E2 flight-18 race | Static |
| U-248 | P3 | E2 flight-11 slot-12 exit | C-0137 passes along its path |
| U-249 | P3 | E2 front end in captures | Partly resolved; C-0120, C-0015 and C-0016 match |
| U-251 | P3 | E2 17/18: died and lost both set | Flow test `lost_flights_17_and_18` |
| U-271 | P3 | E3 flight-8 F6 lamp | Static (`logic/e3/tests.rs::flight_8`) |
| U-272 | P2 | E3 flight-5 escort on (80, 199) | **Closable:** C-0129 passes ("slot 2 hidden at (80, 199)", phase 100 at t853) |
| U-274 | P2 | E3 flight 16: ships after the freeze | C-0152 ends one tick after the freeze; not exercised |
| U-276 | P2 | E3 OB-046 convoy turns back | Static |
| U-277 | P3 | E3 flight-10 medical truck | C-0159 (idle run, truck destroyed at t1048) partly |
| U-278 | P3 | E3 flight 14: slots vanish | **Closable:** C-0150 compares every slot every tick through ticks 76 and 176 |
| U-279 | P3 | E3 F5 entry 21 presentation | Static |
| U-280 | P3 | E3 front end in captures | Partly confirmed (C-0036, C-0041, C-0125) |
| U-282 | P3 | OB-051 in the rig | Flow test only |
| U-283 | P3 | E3 F5 fixed points | Static (`map.rs::e3_board`) |
| U-284 | P2 | E3 setup vectors 3–9 | **Closable for 3, 4, 5, 8, 9:** setups compared in C-0136, C-0148, C-0129, C-0154, C-0151. Vectors 6 (flight 7) and 7 (after flight 16) stay open |
| U-285 | P3 | E3: anything visible before the flight fade-in | Ties in with BUG-10 |
| U-286 | P3 | E3 hazard: armour gauge during the wrap | Static |
| U-302 | P3 | E1 front-end details (a–e) | ASSUMPTIONs (§5.2) |
| U-312 | P3 | E2 abort cursor in block 1 | Port follows §4.2 |
| U-330 | P3 | E3 flight-16 afterburner test placement | ASSUMPTION |
| U-350 | P3 | `maraud3` separators | ASSUMPTION |
| U-430 | P3 | E3 flight-10 hangar re-entry | Original: yes (C-0182, C-0183); td-sim not compared (§4) |
| U-440 | P3 | Long E1 campaign: `g_MemAvailText`, font segment | Not verifiable with direct starts (OB-017) |
| U-450 | P3 | Keys during the real-time screens | ASSUMPTION in `td2192` `held_controls` |
| U-451 | P3 | `Crt.Delay` at P1 | Port uses real ms; cut-scene about 12 retraces short |
| U-461 | P3 | Fade crossover | Port 380 |
| U-470 | P3 | WORX status phase | Port X / R |
| U-480 | P3 | E2/E3 menu drain key | Feeds M10-46 |

By priority: three P1 (U-001, U-009, U-024); thirteen P2 (U-062, U-140, U-144, U-146, U-182, U-204, U-241, U-242, U-246, U-272, U-274, U-276, U-284); the rest P3. Ten rows are closable or likely closable from existing evidence: U-001 (after the ADR), U-009 (likely), U-146, U-184 (likely), U-231, U-241, U-243, U-272, U-278, and U-284 in part.

## 7. M12-03 bug-parity review (`original-bugs.md`)

One row per OB. "Decided" is "no" where the catalogue's last column still says "proposed" or "open". The policy is quoted from the catalogue. "Pinned" names the test that would fail if the port changed the behaviour.

| OB | Policy | Decided | Port | Pinned by | Finding |
|---|---|---|---|---|---|
| 001 | not-ported | yes | no disk-label path anywhere | (absence) | OK |
| 002 | not-ported | yes | no memory check or sound-card screen (launcher T3) | (absence) | OK |
| 003 | preserve | no | one-pixel shift (`scene_card.rs`, `story.rs`) | `story_parity.rs::title_and_hangar_cards_match_the_original`, `launcher_shell.rs::e1_items_6_and_7_show_the_pictures_with_intro_cmf` | OK |
| 004 | preserve 0 advance, draw nothing | no | `small_text.rs` | `small_text.rs::unsupported_characters_draw_nothing_and_do_not_advance` | OK |
| 005 | fix-all: draw nothing for unloaded glyphs | no | `title_text.rs` | `title_text.rs::colon_is_not_loaded_and_space_draws_nothing`, `::dot_and_comma_shapes` | OK, but mislabelled (§7.1) |
| 006 | preserve behaviourally | yes | full tile loaded | `original_graphics.rs::tilesets_parse` | OK |
| 007 | preserve colour; fix-all hang | no | `dialogue.rs` | `dialogue.rs::text_entries_rotate_and_unmatched_lines_use_slot_2`, `::missing_scene_plays_nothing` | OK |
| 008 | open (U-060) | no | not reachable | `mission.rs::edges_of_the_file_image` | U-060 is resolved; needs a decision (§7.1) |
| 009 | fix-all: too far | no | `sfx.rs`, `mixer.rs`, `hit_flash.rs` | `sfx.rs::distance_overflow_is_too_far`, `mixer.rs::far_and_overflowing_effects_never_play`, `hit_flash.rs::wrapped_and_negative_squares` | OK |
| 010 | preserve (T1) | no | bit-exact Real48 | `real48.rs::matches_all_vectors_from_the_original` (vector 8) | OK |
| 011 | preserve; record the device | no | PC-speaker path not ported; replays have no device field | none | Moot until M12-05; M12-05 must draw the `Random` calls and record the device |
| 012 | fix-enhanced | no | Classic restarts every cue; `CueRestart::IfDifferent` exists in td-audio, but `td-flow/src/program.rs:107` always passes `CueRestart::Always` | `music_cues.rs::classic_restarts_on_every_cue`; the mechanism by `::enhanced_keeps_a_looping_song` | **Enhanced fix missing** (not wired to the profile) |
| 013 | preserve as no-op | no | `ships.rs` | `oracle_ships.rs` (C-0009), full flights | OK |
| 014 | preserve | yes | `ships.rs` | `oracle_ships.rs`, full flights | OK |
| 015 | preserve | no | `ships.rs` | `oracle_ships.rs` (slot 19 detour) | OK |
| 016 | preserve | no | scan toward +x / +y (`ships.rs:635`, uncited) | full flights (enemy fire) | OK; cite OB-016 in the code |
| 017 | preserve | yes (main session) | E1 and E2/E3: `td_sim::flight::front_end` (IM-18, IM-20) | `front_end_state.rs`, `oracle_front_end.rs`, `front_end/tests.rs`, full flights | implemented in all episodes (after this review) |
| 018 | preserve (session state) | no | `ClassTables` in the `Session` | none | **Not pinned** (direct starts only) |
| 019 | preserve | no | `hud.rs` | `hud_parity.rs::c0002_f3543_instruments_match` | OK |
| 020 | preserve | no | `hud.rs` | `hud.rs::same_column_uses_the_plus_two_test` | OK |
| 021 | preserve | no | flight-7 re-arm rule (`logic.rs:934`, `msn_near` without a class test, uncited) | full flight 7 (C-0057) only lays mines | **Not pinned** for re-arming a destroyed mine |
| 022 | fix-enhanced candidate | no | Classic preserved | none | **Not pinned**; needs a decision (§7.2) |
| 023 | fix-enhanced | no | **Classic not preserved**: `FlightDac::toggle` writes the whole new-mode palette in every episode | none | **BUG-9** (Classic deviation); the player guide is wrong here |
| 024 | preserve Classic, fix-enhanced | no | Classic: `td2192/src/flight.rs:471` and the session | none | **Enhanced fix missing**; Classic not pinned |
| 025 | preserve | no | `logic/e2.rs` | `logic/e2/tests.rs::outcome_tests`, `e2_front_end.rs::block_16_ending_b` | OK |
| 026 | preserve (T1) | no | `shots.rs:461` | none | **Not pinned** (U-204) |
| 027 | preserve | no | `campaign_e2.rs`, `campaign_e3.rs`, `scenes_e3.rs` | `e2_front_end.rs::death_accepted_flies_again`, `e3_front_end.rs::replay_blocks_offer_the_block_again`, `scenes_e3.rs::sel_pain_third_caption_is_off_screen` | OK |
| 028 | fix-all | yes (user) | `setup/e3.rs::restore_class_tables` | `setup/e3.rs::every_flight_setup_restores_the_class_tables`, `flight_e3.rs::e3_ob028_freeze_never_leaks` | OK |
| 029 | preserve | no | `flight_palette.rs` | `flight_screens_parity.rs::ir_palettes_match_c0014` (vector 16) | OK |
| 030 | preserve | no | `flight_screens/menu.rs:555` | none | **Not pinned** |
| 031 | fix-enhanced | no | the comm screen waits for Space; port-front-end §5 binds controller South to Space in **both** profiles | none | **Fixed in Classic too**, by the binding layer: policy and port disagree (§7.2) |
| 032 | fix-enhanced | no | Classic preserved (`flight_palette.rs`) | `flight_palette.rs::white_flash_frames` | **Enhanced fix missing** |
| 033 | preserve | no | `flight.rs` | `flight.rs::explosions_follow_the_window_and_page_rules` | OK |
| 034 | fix-all (port counts frames) | no | frame counts | `timing.rs::dos_time_waits_count_bios_ticks` | OK, but mislabelled (§7.1) |
| 035 | preserve | no | `setup/e2.rs` | full flight E2 2 (C-0121) | OK |
| 036 | preserve Classic; fix-enhanced candidate | no | `logic/e2.rs:296` | full flight E2 8 (C-0132) | Candidate (§7.2) |
| 037 | preserve | no | `logic/e2.rs` (uncited) | full flight E2 5 along its path only | **Not pinned** (the quirk paths) |
| 038 | preserve Classic; fix-enhanced candidate | no | `logic/e2.rs:472` | none | **Not pinned**; candidate (§7.2) |
| 039 | preserve, pending U-246 | no | `logic/e2.rs`, `shots.rs` | `logic/e2/tests.rs::flight_13_random_and_ghost_fire` | OK (the volleys' effects are ASSUMPTIONs) |
| 040 | preserve | no | `logic/e2.rs` (uncited) | `logic/e2/tests.rs::outcome_tests` | OK |
| 041 | preserve Classic; Enhanced may use E1's look | no | `MenuStyle::Later` | `e2_front_end.rs::main_menu_matches_c0015_and_c0016`, `e3_front_end.rs::main_menu_matches_c0036` | Optional fix not implemented (§7.2) |
| 042 | preserve | no | `campaign_e2.rs` | `e2_front_end.rs::lost_flights_17_and_18`, `::block_16_ending_b` | OK |
| 043 | preserve Classic; fix-enhanced candidate | no | `logic/e3.rs`, `setup/e3.rs` | `logic/e3/tests.rs::flight_7_phase`, `flight_e3.rs::e3_flight_7_setup` | Candidate (§7.2) |
| 044 | preserve | no | `logic/e3.rs` | `logic/e3/tests.rs::flight_8` | OK |
| 045 | preserve | no | `logic/e3.rs` | `logic/e3/tests.rs::flight_1_waves` | OK |
| 046 | preserve Classic; a fix needs a decision | no | `logic/e3.rs` (inert exits) | none | **Not pinned**; decision (§7.2) |
| 047 | preserve | no | `logic/e3.rs` | full flight E3 16 (C-0152) | OK |
| 048 | preserve | no | `setup/e3.rs` and the shields reset | `flight_e3.rs::e3_flight_15_setup` | OK |
| 049 | preserve | no | `flight_palette.rs` | `flight_palette.rs::fades_round_halves_up`, `::magenta_fade_and_tint`, `::white_flash_frames` | OK |
| 050 | preserve | no | `campaign_e3.rs` | `e3_front_end.rs::block_1_skip_shortcut`, `::esc_on_the_about_screen_skips_block_1` | OK |
| 051 | preserve | no | `campaign_e3.rs` | `e3_front_end.rs::missions_without_a_block_add_the_stale_kills` | OK |
| 052 | preserve | no | `setup/e3.rs` | `flight_e3.rs::e3_flight_18_setup`, `setup/e3.rs::marker_fills` | OK |
| 053 | preserve | no | `map.rs` | `map.rs::e3_board` | OK |
| 054 | preserve | yes (RE-10, C-0041) | hazard test | `oracle_e3.rs::e3_flight_8_matches_c0041`, `logic/e3/tests.rs::hazard_damage_wraps_through_the_end_sequence` | OK |
| 055 | fix-all | no | missing slots are empty; first save creates the files | `saves.rs::short_or_missing_files_are_empty_slots`, `save_parity.rs::the_menu_restores_from_and_saves_to_the_port_s_own_files` | OK |
| 056 | preserve Classic; fix-enhanced | no | Esc ignored | `save_parity.rs::restore_and_save_with_the_shipped_files_match_c0043` | **Enhanced fix missing** |
| 057 | preserve Classic; fix-enhanced | no | slot-0 reload | `td2192/src/flight.rs::mission_sprites_land_where_flight_screens_says` | **Enhanced fix missing** |
| 058 | preserve Classic; fix-enhanced | no | `IntroOptions::fix_ob058` under Enhanced | `intro_parity.rs::ob058_fragment_in_classic_only` | OK, both profiles |
| 059 | preserve | no | `td-render/src/intro.rs` | `intro.rs::hoverskid_is_flipped_shifted_and_wraps_on_the_low_byte`, `intro_parity.rs::vectors_3_6_7_every_captured_frame` | OK |
| 090 | preserve as observed | no (implemented) | `hud.rs` | `hud_parity.rs::weapon_0_icon_matches_c0069` | OK |
| 100 | preserve with the oracle's offsets | no | `td_audio::worx` | `worx.rs::intro_busy_times_match_c0081_to_the_frame`, `sfx.rs::boomboom_holds_its_priority_for_its_busy_time` | OK |
| 101 | preserve "first tick at the start" | no | `CmfPlayer::start` | `oracle_parity.rs::looping_menu_music_matches_capture_c0073` | OK |
| 102 | preserve Classic; Enhanced may use one rate | no | `fade::frames_per_level` | `story_parity.rs::the_first_title_card_fades_at_one_frame_per_level`, `::a_title_card_after_the_story_so_far_fades_at_two_frames_per_level` | Optional fix not implemented |

**Totals.**

| Group | Count | OBs |
|---|---:|---|
| Total | 63 | |
| Policy implemented and pinned | 45 | |
| Classic side implemented but not pinned | 8 | OB-018, 021, 022, 026, 030, 037, 038, 046 |
| Missing or deviating | 8 | OB-012, 024, 032, 056, 057 (Enhanced fix missing); OB-017 (E2/E3 missing); OB-023 (Classic deviation); OB-031 (Classic deviates from the policy) |
| Moot | 2 | OB-008, 011 |

### 7.1 OBs still "proposed" or "open": the decision each needs

55 OBs say "proposed" in their policy or "decided by" cell, and OB-008 says "open". Only seven are settled: OB-001, 002, 006, 014, 017, 028 and 054.

**Recommendation:** accept as proposed, in one batch, every OB whose policy follows the MDG §6.3 rule. The port already implements each of them, as the table shows. They are:

- **Preserve** (affects play or feel, or is cosmetic and observable): OB-003, 004, 010, 013, 015, 016, 018, 019, 020, 021, 025, 026, 027, 029, 030, 033, 035, 037, 039, 040, 042, 044, 045, 047, 048, 049, 050, 051, 052, 053, 059, 090, 100, 101.
- **Fix-all** (crash or hang): OB-007 (the hang), OB-009 (RunError 207), OB-055 (I/O runtime error).

The ones that need more than a rubber stamp:

| OB | Decision needed | Recommendation (MDG §6.3) |
|---|---|---|
| OB-005 | Labelled fix-all, but it is cosmetic (title-font `:` reads uninitialised heap; `.` and `,` look the same as observed) | Relabel as **preserve as observed**, with a T3 note for `:`: uninitialised memory cannot be reproduced, so the port draws nothing. Fix-all is reserved for crashes, hangs, save corruption and blockers |
| OB-008 | "open (U-060)"; U-060 is now RESOLVED: not reachable with shipped data | **Preserve (moot)**: the port loads exactly what the original loads; no shipped path reaches the unloaded steps |
| OB-011 | Preserve, but the PC-speaker mode does not exist yet | **Preserve**, deferred to M12-05. If that mode is built, its `Random` calls and a device field in replays are required |
| OB-034 | Labelled fix-all; a vanishing card is neither a crash nor a blocker | Relabel as **T3 deviation** (the DOS centisecond clock is not modelled; the port counts frames). Keep the behaviour |
| OB-012, 024, 032, 056, 057 | fix-enhanced, proposed | **Accept fix-enhanced**: each is a pure annoyance. Implement the Enhanced fixes (§7.2) |
| OB-023 | fix-enhanced, proposed | **Accept** (cosmetic annoyance), but the Classic side must first be restored (BUG-9) |
| OB-031 | fix-enhanced, proposed; the port already accepts a bound controller button in both profiles | **Record as T3 port behaviour** (port-front-end §5: "hit any key" screens take bound buttons). The original's joystick port is not emulated separately, so there is no Classic way to be keyboard-only. Change the policy cell and the player guide |
| OB-058 | fix-enhanced, implemented | Accept |
| OB-102 | preserve in Classic; "fix-enhanced may use one rate" | **Preserve; no Enhanced fix** (the speed difference is barely noticeable; not worth an option) |

### 7.2 Fix-enhanced OBs whose Enhanced fix is not implemented

The player guide (`docs/player/README.md` §8) says the planned Enhanced fixes are not selectable yet. Since D5 (user, 2026-11-06) the Enhanced preset includes every fix-enhanced fix, so these are 1.0 work for M12-04. Exactly these are missing:

| OB | What the Enhanced fix would be | State in the port |
|---|---|---|
| OB-012 | A cue for the song already playing (and looping) does not restart it | Mechanism done (`CueRestart::IfDifferent`, tested); `td-flow/src/program.rs:107` always passes `Always`. Wire it to `Profile::Enhanced` |
| OB-023 | E2: leaving infra-red mode 2 restores the full mode-1 palette | The port does this in **every** profile (BUG-9). The Classic side is wrong, not the Enhanced one |
| OB-024 | E2: restore `k_TileRadarColour` after flights 9–20 | Not implemented |
| OB-031 | A joystick or controller button advances the transmission screen | Effectively done in both profiles by the binding layer (§7.1) |
| OB-032 | The white flash in palette mode 2 restores the palette at its end | Not implemented |
| OB-056 | Esc cancels the save-name entry | Not implemented |
| OB-057 | The ship library restores the flight's own sprite slot 0 (E1 flight 20) | Not implemented |

Candidates and options with no decision yet:

| OB | Candidate | Recommendation |
|---|---|---|
| OB-022 | E1 flight 8 soft lock: also advance when both Vulture IIs are gone | Borderline "blocks progress": abort and retry recover from it, and reachability is unknown (U-182). **User decision.** Recommend Classic preserve plus an Enhanced fix; escalate to fix-all if U-182 shows it happens in ordinary play (the OB-028 precedent) |
| OB-036 | E2 flight 8: exit at (1, 123) | **Preserve, no Enhanced fix.** It changes how the flight is won (gameplay); §6.3 keeps those in every profile |
| OB-038 | E2 flight 16: test an idle ticker | Pure annoyance (a missable transmission). **Fix-enhanced acceptable**, low value; defer past 1.0 |
| OB-041 | E2/E3 menu with E1's item-3 colours | **Preserve; no Enhanced fix** (cosmetic, faithful) |
| OB-043 | E3 flight 7: phase := 1 at setup | **Preserve, no Enhanced fix** (it changes the mission) |
| OB-046 | E3 flights 15/16: exit at (168, 199) | **Preserve, no Enhanced fix** (it changes flight 16 into "destroy or let escape"; U-276 unconfirmed) |
| OB-102 | One fade rate | Preserve; no fix (§7.1) |

Also found while reviewing: text.md §5 items 1–3 asks for "mirror in original-bugs.md". Items 1 and 2 are OB-004 and OB-005; item 3 (pen past x = 351 is clipped, T2) has no OB. hud.md §5 "low-armour colour after recovery: preserve" has no OB either. A Spec author should add both and sweep every spec's "Edge cases and original bugs" section for other unmirrored "preserve" items.

## 8. Doc corrections made by this branch

1. `docs/STATUS.md`:
   - M12-01 and M12-03 claimed, then set to REVIEW;
   - **BUG-1** gets its missing status line (fixed by M8-12, test un-ignored);
   - **BUG-9** (OB-023 Classic deviation) and **BUG-10** (flight-start fade-in not ported) filed;
   - M4-02's "tick rate placeholder 70 Hz (U-001)" annotated as superseded.
2. `docs/parity/M8-missions.md` D1: the BUG-1 test is no longer ignored.
3. `docs/parity/M6-flight.md`: a pointer to the later reports. Its weapons and HUD rows predate C-0011, `hud_parity` and the full flights.
4. `docs/spec/sim/collision.md`: header revision 1 → 2, to match its own revision history (rev 2, RE-5). No behaviour change.
5. `docs/spec/original-bugs.md` OB-008: "Effect on play" no longer waits on U-060 (resolved by RE-14). The policy is left for the decision in §7.1.
6. `docs/parity/README.md`: lists this checklist.
7. Main merged (`c2ab363`, the user's D3–D6 decisions). D4 (no TFP) and D5 (all fix-enhanced fixes in the Enhanced preset) are reflected above.

Not changed, for the coordinator:

- STATUS "Current state" still says "M1–M8 in parallel" (last updated 2026-11-02).
- M9-21 is IN PROGRESS although the evidence for flights 1–20 is complete (M9-playthrough; flight 21 is not captured).
- M9-20 (endings) is TODO although `td_flow::campaign` implements endings A/B/C.
- M10-30 and M10-43 are REVIEW with no owner.
- UNKNOWNS rows closable from existing evidence are listed in §6.
- The player guide §8 is wrong about OB-023 and OB-031.

## 9. Reproducing

- Test index: every `#[test]` with the spec citations next to it, from a scratch script (`ve6/testindex.py`, outside the repo). The tallies of §2 come from the status column of §4.
- Runs, from the repo root:
  - `cargo test --workspace --no-fail-fast`, with and without `TD2192_DATA`/`TD2192_ORACLE`;
  - `cargo test -p td-sim --test oracle_e2_full -- --nocapture` prints one PASS line per flight with its events;
  - `cargo test --workspace -- --nocapture` (with the variables) shows any `SKIPPED` line (§3.1).
