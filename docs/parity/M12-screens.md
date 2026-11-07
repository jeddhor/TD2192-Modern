# Parity: M12 front-end screens against the original (2026-11-06, VE-7)

Verifier: VE-7 (Verifier hat; no disassembly read). This report covers the screens that M12-checklist §0 item 1 lists as "never compared with a capture": the debriefs, the assignment orders, the enemy and pilot dossiers, the story so far, the stills, the endings and the episode end cards of all three episodes. Tree under test: main `0efb4c8` plus this branch. Tests: `crates/td-flow/tests/screens_parity.rs` (data- and oracle-gated; every test reads the capture's frame dumps, no pixels are stored).

## 1. Method

- **Rig.** A private copy of the game in `re-work\ve7\dosbox` (reset from the original before every run, the original only read), oracle build 3 (pinned `re-work\dosbox\bin\oracle-3`), preset P1. The run scripts are in the session scratchpad; the scenarios are kept with each capture (`scenario.txt`).
- **Reaching the screens.** Direct starts (E1 `tdstatus.dat`; E2/E3 a prepared `TD2.SAV`/`TD3.SAV` slot 1 and the menu's restore, as M9/M10). The flight is either **idle** or flown with a success replay's tick-keyed schedule (`tdtool replay --oracle-schedule`). For the idle flights, td-sim predicted the outcome first (`tdtool plan --idle`): E1 flight 1 death (617 ticks), flight 12 success with outcome 0 (697), flight 20 lost (2673); E2 flight 7 death (617), 9 success with outcome 0 (1466), 13 success (158), 18 lost (968); E3 flight 2 success with outcome 0 (752), 5 success (854). **The original ended every one of these flights the same way** (the screens that followed are the ones the flow spec gives for that outcome; the debriefs show the failure note exactly for the outcome-0 flights).
- **Dialogue scenes** get exactly one Space per page break (`05` line, dialogue-player §4.7), counted from the script file, so no key is left over for the screen after them (E1 and E2 timed waits do not flush the keyboard). All 16 scenes ended exactly on their last page.
- **Dumps.** Every screen is dumped at every frame from its first file open for 90 to 120 frames (fade-in, the start of typing), then every 4 to 6 frames through the typing, then every 10 to 30 frames to the end of the hold.
- **Comparison.** `sequence()` aligns the port's first frame with a lit DAC with the capture's, then compares every dump with the port's frame at the same distance: all 64,000 pixels, and all 256 DAC entries wherever the capture's DAC is complete (equal to the next frame's DAC when that frame is dumped; a fade level is written over about 1.5 frames, screens §2.1). Text cards are compared in their hold, pixels and the entries they show (their other entries are whatever the screen before left in `g_Palette`).

## 2. Captures

| Capture | Run | Screens |
|---|---|---|
| C-0270 | E1 flight 1 idle (death), Esc at the retry prompt; 7 kills to date | ending A: pilot dossier |
| C-0271 | E1 block 5 pre-flight, no keys | enemy dossier |
| C-0272 | E1 menu item 5, no keys | story so far, pages 1 and 2 |
| C-0273 | E1 block 2 pre-flight (dialogues 4, 5) | assignment orders, hint 2 |
| C-0274 | E1 flight 12 idle (success, outcome 0); 45 kills to date; post-flight story (dialogues 47–50) | failure debrief, title cards T[0938], T[05FC], `td1-14` still set (P3) |
| C-0275 | E1 flight 18 with `e1f18.tdr`; 150 kills to date; post-flight story (dialogues 82–84) | success debrief, T[0560], `td1-15`, `td1-18` card |
| C-0276 | E1 flight 20 with `e1f20.tdr`; post-flight story (dialogues 96–99) | `td1-20` still set (P2), episode end card T[0E51] |
| C-0277 | E2 flight 13 idle (success); 300 kills to date | success debrief |
| C-0278 | E3 flight 5 idle (success); 75 kills to date | success debrief (15 s) |
| C-0279 | E1 flight 20 idle (lost), Esc at the retry prompt | ending C card |
| C-0280 | E1 block 6 pre-flight, Esc in dialogue 24 (no skip test) | hint 6 |
| C-0281 | E1 block 16 pre-flight (dialogues 68, 69) | `td1-16` still set (P1) |
| C-0282 | E1 flight 15 with `e1f15.tdr`; post-flight story (dialogues 65–67) | success debrief, `td1-13` still set (P4) |
| C-0283 | E2 flight 9 idle (success, outcome 0); 20 kills to date | failure debrief |
| C-0284 | E2 flight 7 idle (death), Esc at the retry prompt; 33 kills to date | ending A: pilot dossier |
| C-0285 | E2 flight 18 idle (lost), Esc at the retry prompt | ending C card |
| C-0286 | E2 flight 20 with `e2f20.tdr`; post-flight story (dialogues 123–127) | success debrief, end card T(20.5) |
| C-0287 | E3 flight 2 idle (success, outcome 0); 12 kills to date; Esc at the replay prompt | failure debrief |
| C-0288 | E3 flight 19 with `e3f19.tdr`; post-flight story (dialogues 107–109) | end card T(end) |
| C-0289 | E3 flight 8 on C-0041's keypad route into the hazard cells (death), Esc at the retry prompt; 21 kills to date | ending A: pilot dossier |

C-0200 (VE-5, E1 block 19) also holds an E1 success debrief (flight 19), sampled every 70 frames; one hold frame is compared.

| C-0290 | E2 menu item 6, no game in progress, no keys | about screen (30 s) |
| C-0291 | E3 menu items 6 and 7, no keys | about screen (30 s), order screen (45 s) |

## 3. Results

"Exact" = every dump from the screen's first lit frame to the last dump but one of its hold equals the port's frame at the same distance: 64,000 pixels and 256 DAC entries (the DAC only where complete, §1). "D1" = exact apart from typing up to two frames early (§4). Frames are capture frames.

| Screen | Spec | Capture, frames | Test (`screens_parity.rs`) | Result |
|---|---|---|---|---|
| E1 success debrief, mission 19 | screens §3.6, episode-program-flow §4.5 | C-0200 f2500 (hold) | `e1_success_debrief_matches_c0200` | exact, one kill count fits |
| E1 failure debrief, mission 12 (labels[4], failure note), 45 kills to date | screens §3.6 | C-0274 f3073..4073 | `e1_failure_debrief_matches_c0274` | D1 |
| E1 success debriefs, mission 18 (150 kills to date) and 15 | screens §3.6, tiers §4.5 | C-0275 f3810..4816, C-0282 f3082..4094 | `e1_success_debriefs_match_c0275_and_c0282` | D1 |
| E1 pilot dossier (ending A), total 7 | screens §3.5, §4.5 | C-0270 f2915..5047 | `e1_pilot_dossier_matches_c0270` | D1 (the line-5 overwrite after `Crt.Delay(2000)` included) |
| E1 enemy dossier | screens §3.5 | C-0271 f2945..3206; f3501 | `e1_enemy_dossier_matches_c0271` | D1 to line 8; then **BUG-12** (pinned) |
| E1 assignment orders | screens §3.4 | C-0273 f2258..2749 | `e1_assignment_orders_match_c0273` | D1 (two frames from the 120th character) |
| E1 hints 2 and 6 (`secret.pic` banner, typed lines) | screens §3.3, text §4.5 | C-0273 f3452..3747, C-0280 f1678..2069 | `e1_hint_2_matches_c0273`, `e1_hint_6_matches_c0280` | D1 |
| E1 story so far, page 1 (`javelin.td`) and page 2 (`city.td`) | screens §3.8 | C-0272 f406..2420 (every dump), f3006 | `e1_story_so_far_matches_c0272` | exact (fade-in included) |
| E1 still sets P1 `td1-16`, P3 `td1-14`, P4 `td1-13` with captions | asset-load-sites §4, text §4.5 | C-0281 f4059..5719, C-0274 f12629..14285, C-0282 f10109..11771 | `e1_stills_match_…` | exact |
| E1 still set P2 `td1-20` (episode end) | asset-load-sites §4, FORMATS §11 | C-0276 f8698 (hold) | `e1_stills_match_…` | **BUG-13**: the original's picture is one pixel to the right (pinned) |
| E1 `td1-15` still | screens §3.9 | C-0275 f7497..8176 | `e1_stills_match_…` | exact (column 32 black, U-227) |
| E1 `td1-18` card | text §4.5 | C-0275 f9079..10091 | `e1_stills_match_…` | exact |
| E1 ending C card (intro2) | episode-program-flow §4.5 | C-0279 f10415..11094 | `ending_c_cards_match_c0279_and_c0285` | exact |
| E1 episode end card T[0E51] | §4.5 | C-0276 f10304 (hold) | `episode_end_cards_match_…` | exact (pixels, shown entries) |
| E1 title cards T[0938], T[05FC], T[0560] after a dialogue | text §4.5.2 | C-0274 f9752, f11659; C-0275 f6220 | `e1_title_cards_after_a_dialogue_…` | exact (pixels, shown entries) |
| E2 success debrief (300 kills to date, tier 11), failure debrief, debrief after flight 20 | e2-program-flow §3.3 | C-0277 f2059..3041, C-0283 f6830..7824, C-0286 f2272..3278 | `e2_debriefs_match_…` | D1 |
| E2 pilot dossier (ending A), total 33 | e2-program-flow §4.5 | C-0284 f3870..5982 | `e2_and_e3_pilot_dossiers_…` | D1 |
| E2 ending C card | e2-program-flow §4.5 | C-0285 f5236..5916 | `ending_c_cards_…` | exact |
| E2 end card T(20.5) | e2-program-flow §4.5 | C-0286 f9809 (hold) | `episode_end_cards_…` | exact (pixels, shown entries) |
| E2 about screen | e2-program-flow §3.3 | C-0290 f572..2562 | `about_and_order_screens_…` | exact |
| E3 success debrief (15 s), failure debrief (E3's own note) | e3-program-flow §3.3 | C-0278 f4224..5560, C-0287 f3852..5176 | `e3_debriefs_match_…` | D1 |
| E3 pilot dossier (ending A), total 21 | e3-program-flow §4.5 | C-0289 f2328..4440 | `e2_and_e3_pilot_dossiers_…` | D1 |
| E3 end card T(end) | e3-program-flow §4.5 | C-0288 f7417 (hold) | `episode_end_cards_…` | exact (pixels, shown entries) |
| E3 about and order screens | e3-program-flow §3.3 | C-0291 f570..2560, f2903..5883 | `about_and_order_screens_…` | exact |

**Flow observations confirmed by the captures** (file opens in `int21.tsv`): the E2 block-20 post-flight order (shuttle, D123, hydedead 15 s, D124, marauder, D125, `death.cmf`, dieseche, marauder, D127, T(20.5), menu; e2-program-flow §4.4, C-0286); E2 and E3 offer one retry prompt for a lost flight and go straight to ending C on Esc (C-0285; E1 C-0279); the E3 failure debrief is followed by the replay prompt (C-0287); the E1 block-5 dossier comes before any dialogue (C-0271); the debrief is the screen after `low.cmf` in every block (M8).

## 4. Divergences

| # | Screen, frames | What differs | Spec | Disposition |
|---|---|---|---|---|
| D1 | Every typed screen (orders, dossiers, debriefs, hints), e.g. C-0273 f2288..2339 (one frame), f2409..2700 (two), C-0270 f2945..2960, f2996, f3323 | The port shows each typed line one frame early: its first character appears in the frame after the last fade level (or after the `Crt.Delay`), the original's a frame later. In a long run of typing (the orders, nine lines back to back) the original falls one more frame behind around the 120th character. Pixels and DAC are otherwise identical; every final image is exact | text §4.3 (one character per retrace-bit wait, U-062) | T2, at most 2 frames. Open question **U-521** (the exact cadence); no bug filed until the spec states it |
| D2 | End of every timed hold, e.g. C-0277 f3047, C-0281 f5759, C-0274 f4085 | The port starts the fade-out up to 6 frames (one level at 2 frames per level, the dump spacing in the hold) before the original | screens §2.2 (DOS-time wait: ceil(n × 18.2065) BIOS ticks or one less, 3.65 frames per tick) | Within the spec's own one-tick spread plus D1; no action |
| D3 | E1 enemy dossier, C-0271 f3211 onwards | The original never types line 9 (`e1.dossier.enemy[9]`, ink 39 over line 4); it types line 10 right after line 8. The port types line 9, so the hold shows line 4 overwritten in red (rows 110..114) | screens §3.5 | **BUG-12**; spec question **U-520** for an Archaeologist. The test pins the difference |
| D4 | E1 still set P2 `td1-20`, C-0276 f8412..10058 | Rows 50..149 of the original are the file's rows one pixel to the right of the port's: screen x = 1 + c for c = 0..318, column 0 black, the last byte of each row not shown. The captions and the DAC are identical | asset-load-sites §4, FORMATS §11 ((0, 50)) | **BUG-13**; spec question **U-522**. The test pins the difference (the shifted port frame equals the capture) |

No port bug that deviates from its spec was found, so this branch changes no port code: both differences come from the specs.

## 5. Not covered

| Screen | Why | What would cover it |
|---|---|---|
| E1 ending B (mission 17 failed: voffice card, D75, `intro2` card) | Flight 17 never ends without input (td-sim: still flying after 8,000 idle ticks); "lost" needs three ships at the base (mission-logic §5.17), which no planner mode targets | A planned flight 17 that lets the base fall (`tdtool plan` has no "lost" goal), then the retry declined; the debrief would also show the E1 failure note for flight 17 |
| E2 ending B | Unreachable: flight 16's outcome is always 1 (OB-025, e2-program-flow §5) | — |
| E2 end card after an Esc in block 20's post-flight story | e2-program-flow §4.5 says the end card follows "(or is skipped with Esc)": whether T(20.5) is shown after an Esc was not captured (C-0286 plays the story to the end) | One run like C-0286 with Esc at the shuttle card |
| Fade-outs | Compared only to the last dump but one of the hold (D2); the dumps in the hold are 6 to 30 frames apart | Dense dumps around the end of a hold |
| E1 enemy dossier after line 8 | BUG-12 | The fix, then the same test to f3886 |
| `td1-20` fade-in | BUG-13 (only the hold frame is compared, shifted) | The fix, then `assert_sequence` like the other stills |
