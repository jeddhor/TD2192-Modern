# Episode 3 program flow (`td3.exe`)

| | |
|---|---|
| **Spec ID** | `flow/e3-program-flow` |
| **Revision** | 2 |
| **Status** | Draft |
| **Fidelity tier** | As [e2-program-flow](e2-program-flow.md): T2 for the order of screens, scenes, dialogue, music and flights; T1 for the state that flows between missions (mission number, kills, outcome flags, the retry prompt's `Random` call, the `td3.sav` slot format); T0 for menu colours; T3 for the text-mode configuration screen, the `/?` usage text, the memory gate and the exit notice |
| **Confidence** | LIKELY overall. CONFIRMED (static) for the call order of all 18 mission blocks, every branch, every skip test (200) and every write of the campaign state: complete token walk of `Main_CampaignLoop` plus a byte scan of the whole image for writes of the flags (§10). CONFIRMED (static) for the music table, the flight song, start-up, menu bookkeeping and the save format. The start-up path to flight 1 and the `TD3.SAV` restore path to block 8 of §8 are CONFIRMED by C-0036 and C-0041 (the anchors and Esc windows below are the ones used there); the other Verifier timings are LIKELY (U-280) |
| **Tasks** | M10-02 |
| **Unknowns** | U-280 (start-up and restore paths confirmed), U-281, U-282 (new); U-270 (resolved, §4.4.1); U-250 (applies to E3 unchanged); U-011 (E3 part resolved here: the slot word is the kill total); U-206 (E3 menu fades and loader resolved here); U-005 (consistent) |

This is a **delta spec**. Implement [e2-program-flow](e2-program-flow.md) (itself a delta against [episode-program-flow](episode-program-flow.md), E1) and change what this document says. Screen layouts are in [screens](screens.md) (E1) and [episode-deltas §9.2](../../re/episode-deltas.md#92-scene-cards) (E3 scene cards). Addresses are `td3:<seg>:<off>` image-relative (ADG §7.8); the `td3:` prefix is dropped below. DGROUP is `32C5`, so `DS:x` means `32C5:x`. File offset = 14544 + seg × 16 + off. Text is named by its resource id in [td3.toml](../../re/exe-resources/td3.toml), never quoted.

## 1. Summary

`td3.exe` starts exactly like `td2.exe` (WORX, memory gate, `td.cfg`, one switch, configuration screen when needed, script choice at every start, joystick calibration, main menu), with E3 file names. *Start new game* and *Next mission* call `Main_CampaignLoop`, which plays one mission block for `g_MissionNumber` 1..6 or 8..19. There are **18 blocks and 20 flights**: block 6 flies F6 and F7 and then sets the mission to 7, block 11 flies F11 and F21, there is no block 7 and no block 20, and the episode is complete when the menu's mission counter reaches **20**. E3 has one ending (A, death) and an end card (`e3.ending`). Six blocks (2, 3, 4, 5, 9, 12) offer a replay after the debrief when the outcome is 0; block 8 offers a retry whose decline counts as a success; block 10 offers a retry whose decline leaves the mission unchanged. Block 19 has no debrief.

Three front-end changes against E2 matter for timing and testing: every timed wait (`Flow_WaitSecondsOrKey`) **flushes the BIOS keyboard buffer** on entry, the menu loader flushes it too, and `Main_CampaignLoop` **no longer clears the skip flag** while block 1 tests it before its first step (OB-050).

## 2. Definitions and units

E3 addresses of the campaign globals (CONFIRMED by references in the listings and the byte scan of §10; [td3.csv](../../re/symbols/td3.csv) marks some as HYPOTHESIS, they are confirmed here):

| Term | E3 global | Notes |
|---|---|---|
| Mission number | `g_MissionNumber` DS:0C49 (u8) | 0 = no game; 1..6, 8..19 = next block; 7 only transiently (block 6, `0000:171F`); 20 only transiently (menu) |
| Kills to date | `g_KillsToDate` DS:0C4A (i16) | |
| Mission kills | `g_MissionKills` DS:11EA (i16) | zeroed at every flight start (`0C5D:A9F7`) |
| Skip flag | `g_SkipRequested` DS:1E72 (u8) | cleared at the start of the 20 card routines and of the retry prompt, dossier, debrief and dialogue; set to 1 when a wait reads Esc. **Not** cleared by `Main_CampaignLoop` (E2 clears it) |
| Advance flag | `g_MissionAdvance` DS:0670 (u8) | |
| Game over | `g_GameOver` DS:066F (u8) | |
| Menu cursor | `g_MenuCursor` DS:07B4 (u8, 0..8) | `DS:07B3` previous cursor, `DS:07B5` "cursor changed", `DS:07B6` "quit confirmed"; `DS:066E` := cursor on quit (never read) |
| Menu palette | `g_MenuPalette` DS:0840 (u8[768]) | |
| Script choice | `g_ScriptChoice` DS:0C48 (u8) | 1 = `dialogu3.dat`, 2 = `safedia3.dat` |
| Devices | `g_MusicDevice` DS:11E7, `g_SfxDevice` DS:11E8, `g_CfgController` DS:11E9 | values as E1 §3.2; `g_NoWorx` DS:11E6; configuration-screen copies DS:066A..066D |
| Flight number | `g_FlightNumber` DS:31DD | the argument of `Sim_FlyMission`; equals the block number except F7 (block 6) and F21 (block 11) |
| Flight flags | `g_MissionLost` DS:11EC, `g_PlayerDied` DS:11ED, `g_MissionAborted` DS:11EE, `g_MissionOutcome` DS:11EF, `g_MissionSucceeded` DS:31C2, `g_EndTicks` DS:31DC | §3.4 |
| Resume status | `g_ResumeStatus` DS:07B2 | read at `0000:41EA` and `4301`, never written (byte scan): always 0 |

**Caption notation.** Scene-card captions are resource strings: `m.k` = `e3.mission.MM.cards[k]` (MM = m with two digits), `p.k` = `e3.story.preamble[k]`, `_` = `e3.story.preamble[2]` (a single space, a blank line). `Tnn` = title card `e3.mission.nn.title`; `T(x)` = a title-style card (`Scene_MissionTitle`) showing caption x; `T(end)` = `e3.ending`. The resource groups only reflect where the strings sit in the exe: E3 has no `cards` group for missions 3, 9, 10, 11, 12, 16 and no title for mission 3, and many captions are reused across blocks (for example `p.10` and `6.1`).

## 3. Data

### 3.1 Music index (`Snd_PlayMusic` `0C5D:07F3`)

Behaviour as E1 §3.1 / [music-cues](../audio/music-cues.md) §4.1 (nothing unless `g_MusicDevice` = 2; stop, free, load, play; every call restarts; the freed buffer pointer is kept and only the size is zeroed). The routine tests N in the order 1, 2, 44, 3..10, 13, 11, 12, then 45 or 69.

| N | File | Used by | N | File | Used by |
|---:|---|---|---:|---|---|
| 1 | `td01.CMF` | flight song | 10 | `mood2.CMF` | campaign, 16 cues |
| 2 | `td02.CMF` | flight song | 11 | `moodmaj.CMF` | campaign, 2 (blocks 18 and 19) |
| 3 | `hangar.CMF` | campaign, 22 | 12 | `death.CMF` | ending A only (1 site) |
| 4 | `satair.CMF` | campaign, 8 | 13 | `mood3.CMF` | campaign, 11 |
| 5 | `vulture.CMF` | campaign, 19 | 44 | `td03.CMF` | flight song |
| 6 | `mood.CMF` | campaign, 14 | 45 | `intro.cmf` | **never passed** |
| 7 | `menu.CMF` | menu only (2 sites) | 69 | `intro.cmf` | menu items 6 and 7 |
| 8 | `low.CMF` | before each of the 18 debriefs | other | nothing loaded; replays the freed buffer (as E1; no caller) | |
| 9 | `climax.CMF` | **never** | | | |

The campaign makes 111 music calls; the counts above add up to it (with M8 = 18 and M12 = 1). Every `Snd_PlayMusic` push in the image was counted: 111 campaign, 2 × song 7, 2 × song 69, 6 in the flight. The names in the exe spell the extension `CMF` in upper case except `intro.cmf`; DOS ignores case, so a port must look the files up case-insensitively.

**Flight song** (`Sim_FlyMission`, `0C5D:B853..B8E3`): flight mod 3 = 1 → 1, = 2 → 44, = 0 → 2. Then, only if `g_MusicBuffer` DS:11F2 is still nil (the load failed, or music is off so nothing was ever loaded), one fallback request: mod 3 = 2 → 2, = 1 → 44, = 0 → 1. Resulting songs: flights 1, 4, 7, 10, 13, 16, 19 `td01`; 2, 5, 8, 11, 14, 17 `td03`; 3, 6, 9, 12, 15, 18, 21 `td02`.

### 3.2 Start-up switches

As E2 §3.2 (first parameter only, exact compare, two spellings each). E3 addresses:

| Parameter | Effect | Where |
|---|---|---|
| `/nosound`, `/NOSOUND`, `/PC`, `/pc` | `g_NoWorx` := 1 | `0000:3ED2..3F3B` |
| `/m`, `/M` | skip the memory gate | `0000:3FAC..3FE1` |
| `/pc`, `/PC` | music 1, SFX 3 | `0000:4211..4248` |
| `/nosound`, `/NOSOUND` | music 1, SFX 1 | `0000:424D..4284` |
| `/sb`, `/SB` | music 2, SFX 2 | `0000:4289..42C0` |
| `/a`, `/A` | music 2, SFX 3 | `0000:42C5..42FC` |
| `/CONFIG`, `/config` | music := 0 (forces the configuration screen; guarded by `g_ResumeStatus` = 0, always true) | `0000:4301..433A` |
| `/?`, `?` | print the usage text and Halt(0) | `0000:433F..44A8` |

A switch that sets the music device skips the configuration screen whatever `td.cfg` says, and `td.cfg` is then not written (as E2).

### 3.3 Screens used by the program

| Routine | What | Wait | Notes |
|---|---|---|---|
| `UI_ScriptChoice` `04A0:4980` | script warning (`e3.script_choice.warning`), fonts `td.lt2`, `td-let.cin` | until `1` or `2` | as E2; **no** keyboard flush, so a key typed earlier counts |
| `UI_JoystickCalibration` `0C5D:A28E` | calibration (`td.joy`) | any key or a button | identical to E1 |
| `UI_LoadMainMenu` `0000:29E5` / `UI_MainMenuLoop` `0000:2E3F` | main menu | indefinitely | §4.2 |
| `Scene_MissionTitle` `04A0:4663(text, 3Fh, 0, 0)` | title card, font `td-let.cin` (loaded at every call) | 5 s or key | as E1; timed wait flushes (§3.5) |
| 17 scene-card routines `04A0:2013..449C` | one per background (§4.4 abbreviations) | 10 s; 15 s for `shuttle0`, `sel-pain`, `melting` | layouts: episode-deltas §9.2 |
| `Scene_Aboutus` `04A0:1D6B`, `Scene_Order` `04A0:1EBF` | full-screen `aboutus.scr` / `order.scr`, no text | 30 s / **45 s** or key | menu items 6, 7 |
| `Dlg_PlayScene` `04A0:7795(n)` | dialogue scene n (`dialogu3.dat` / `safedia3.dat`, `newfaces.td3`) | each page until a key or button, no timeout, **no flush**; Esc ends the scene and sets the skip flag | [dialogue-player](../narrative/dialogue-player.md), E3 deltas episode-deltas §9.3 |
| `UI_RetryPrompt` `04A0:4F47` | retry question (`e3.retry.taunts`) | until a key (keyboard only), no flush; calls `Random(10)` first | returns false for Esc only |
| `UI_Debrief` `04A0:5F7E(kills, kills to date, outcome)` | debrief (`e3.debrief`) | **15 s** or key | its heading shows `g_MissionNumber` (`04A0:6083`) |
| `UI_PlayerDossier` `04A0:5212(total kills)` | death dossier (`e3.dossier.player`) | 25 s or key | E1 layout; the last steps overwrite the status line with `e3.dossier.player[5]` |
| `Scene_BackgroundStory` `04A0:73DF` | story so far, 2 pages (`e3.background_story`) | 30 s / 45 s or key | does not clear the skip flag on entry (as E2), but an Esc in it sets it (OB-050) |
| `UI_MenuHelp` `0000:020B` | `td.hlp`, 2 pages | until key or button (`0000:01A5`) | identical to E1 |
| `Sav_RestoreScreen` `04A0:5450`, `Sav_SaveScreen` `04A0:6413` | restore / save | until a choice or Esc | §4.7 |

### 3.4 State handed to and from `Sim_FlyMission` (`0C5D:A9CC`)

| Flag / value | Set where | Meaning |
|---|---|---|
| `g_FlightNumber` DS:31DD | := argument at entry (`AA07`) | 1..19, 21 |
| `g_MissionKills` DS:11EA | := 0 at entry (`A9F7`) | kills of this attempt only |
| `g_MissionAborted` DS:11EE | := 0 at entry (`A9FF`); := 1 by `UI_FlightMenu` (`0C5D:7BE6`) and by the block-10 decline (`0000:1BE6`) | the player quit from the Esc menu |
| `g_MissionLost` DS:11EC | := 0 at entry (`AA2D`); := 1 only in flight 5 (`0C5D:99FD`, `Msn_E3TimedEvents`: escort gone before phase 100) | ends the flight; **never tested by the campaign** |
| `g_MissionSucceeded` DS:31C2 | := 0 at entry (`AA28`); set by mission logic | ends the flight normally |
| `g_PlayerDied` DS:11ED | := 0 at entry (`A9DA`, new in E3); := 1 in `Sim_EndSequenceStep` (`0C5D:59BD`, overwritten later); at the end (`EA3F..EA4D`): 1 iff `g_EndTicks` = 35 | the death sequence ran to its end |
| `g_MissionOutcome` DS:11EF | at the end (`EA52..EB94`): 1, then the per-flight tests of flights 2, 3, 4, 5, 8, 9, 10, 12 (episode-deltas §4.5; [e3-mission-logic](../sim/e3-mission-logic.md)) | flights 1, 6, 7, 11, 13..19, 21 always give 1 |

The flight loop ends after the tick in which aborted, end ticks = 35, lost or succeeded holds (`EA1D..EA39`). Teardown: flight fade-out, died, outcome := 1 and the per-flight tests, **then** INT 09h restored (`EBA5`), `NoSound`, wait for the playing VOC (SFX device 2), music stopped (unless `g_NoWorx`) and its buffer freed (pointer kept, size := 0), effects freed, heap released. **No text-mode switch** (E2 sets text mode 3 here). The flight also stops the current song at entry (`A9EB`, unless `g_NoWorx`) and flushes the BIOS keyboard buffer during setup (`B0BA..B0CD`), after it has hooked INT 09h (`ABE8..ABF7`). The byte scan of §10 found no other writer of DS:11EC..11EF.

In practice a flight-5 loss also gives outcome 0 (the escort is not at the south edge), so block 5's replay rule handles it (LIKELY; [e3-mission-logic](../sim/e3-mission-logic.md) §4.6.6, §4.8).

### 3.5 Keyboard flushes and the skip flag (E3)

| Routine | Flush on entry | Notes |
|---|---|---|
| `Flow_WaitSecondsOrKey` `04A0:0269` (title, scene cards, about/order, debrief, dossier, story so far) | **yes** (`04A0:0277..028A`: BIOS head copied to tail), and both joystick button flags := 0 | then as E1: ends on a key, a button or the timeout; reads one key if present; Esc sets the skip flag |
| `UI_LoadMainMenu` `0000:29E5` | **yes** (`29F4..2A07`, new in E3) | |
| Menu loop | reads one key, then discards every other buffered key (`0000:2EF3..2F04`, as E2) | |
| Menu item 0 after `Main_CampaignLoop` | yes (`0000:33FD..3410`) | item 3 has no flush of its own; its loader call flushes |
| `Sav_RestoreScreen`, `Sav_SaveScreen` | yes (`04A0:5474..5487`, `643F..644A`) | joystick Y and buttons := 0 before the selection loop |
| `Sim_FlyMission` setup | yes (`0C5D:B0BA..B0CD`) | |
| `Inp_WaitKeyOrButton` `1BB3:01ED` (dialogue pages), `UI_RetryPrompt`, `UI_ScriptChoice`, help-page wait `0000:01A5`, configuration screen | no | a buffered key is taken at once |

Skip-flag writers (byte scan, 26 in all): set by `04A0:03C6` and `1BB3:0239`; cleared at the entry of the 19 card routines (17 scene cards, about, order), the title card, the retry prompt (`04A0:4F60`), the dossier (`5221`), the debrief (`5F8D`) and the dialogue (`77A9`). Nothing in the main program writes it. The save and restore screens, the help pages, the story-so-far screen and the menu do not clear it, so an Esc that ends any timed screen outside a mission block leaves the flag at 1 until the next card, title, dialogue, retry, debrief or dossier routine runs (OB-050).

## 4. Behaviour

### 4.1 Program start (`Main_Entry` `0000:3E8A`, body `0000:3EAD`)

```text
1. unit initialisation chain (System, Crt, 2B72, 2215, 1BD8, 0C5D, 04A0); the mission-logic unit 1FEB has no init part
2. g_JoyCal DS:1E7A..1E81 := (0, FFFFh, 0, FFFFh)
   g_NoWorx := first parameter is /nosound, /NOSOUND, /PC or /pc; Crt CheckBreak := false
3. unless g_NoWorx: WORX_Init, WORX_SbInit -> g_SbIrq DS:0662 (else -1),
   WORX_SetTimerStreaming(1) if an SB was found, WORX_SetFmVolume(10, 10)
4. memory gate (0000:3F95..40EE): MemAvail < 2BF20h (180,000 bytes) and the first parameter is not /m or /M:
   memory warning (text mode, 7 unmapped strings at file 0x736C, names the E1 launcher) and Halt(0)   [T3, OB-002, U-250]
5. Crt.ClrScr (0000:40F8; new in E3)
6. td.cfg as E2: g_MusicDevice := 0; opened once as an existence test, then (if it opened) read 3 bytes
   (music, SFX, controller); detected values as E2; any difference -> g_MusicDevice := 0
7. apply the switch of §3.2; /? or ?: usage text (four unmapped strings at file 0x74E1, then e3.startup[0..3],
   with blank lines between some of them) and Halt(0)   [T3, U-250]
8. if g_MusicDevice = 0: text-mode configuration screen (0000:44AD..4917, as E2; lines e3.startup[5..12];
   less than 249F0h bytes free forces SFX off, shows e3.startup[4] with a beep and waits 5 s); writes td.cfg
9. if g_CfgController = 2: re-detect the joystick; none -> controller 1
10. if dialogu3.dat opens: script choice (04A0:4980); else g_ScriptChoice := 2 silently
11. if g_CfgController = 2: joystick calibration (0C5D:A28E)
12. joystick axes and buttons := 0; g_MissionNumber := 0; main menu (0000:2E3F)
13. exit: the menu loop has already set text mode 3 (0000:3A75, new in E3); unless g_NoWorx: WORX_SbShutdown,
    WORX_Shutdown; text mode 3 again; exit notice (§4.1.1); text colour 7; cursor to column 1, row 10; Halt(0)
```

Deltas to E2 §4.1: E3 file names; `Crt.ClrScr` after the gate; the extra text-mode call at the end of the menu loop. As in E2 there is no keyboard flush after the script choice, but in E3 the menu loader flushes on entry, so keys typed after `1`/`2` **do not** reach the menu.

#### 4.1.1 Exit notice

Identical to E2 §4.1.1: the 262 bytes at DS:0264 (`k_ExitScreenText`, file offset 0x36784; byte-identical to E2's at 0x3AF44) are uncrunched to B800:0000 by `3114:0000` (`0000:49BA..49D4`). Port policy: T3, optional (§9.3).

### 4.2 Main menu (`UI_MainMenuLoop` `0000:2E3F`)

The loop is E2's (E2 §4.2) with these differences:

1. **Entry:** mission := 0, cursor := 0, `UI_LoadMainMenu(1)`. E3 has **no** `Snd_PlayMusic(7)` at loop entry, so `menu.CMF` starts once (E2 starts it twice).
2. **Loader** `UI_LoadMainMenu(playMusic)` `0000:29E5`: **flushes the BIOS keyboard buffer** first (new); then exactly as E2: song 7 only when the argument is non-zero; `Gfx_InitUnchained`; `td.men` regions, `tdmen.pal`, `td.men` again; entries 224..233 := (16, 0, 0) including 227; the cursor's entry gets red 63 only; fade in (`0000:0000`); the cursor's red goes back to 16 in the buffer. OB-041 therefore applies to E3 as well.
3. **Fades** `0000:0000` (in) and `0000:00D2` (out): identical to E2 (17 levels, DAC 1..255 := Round(c × s / 16) from `g_MenuPalette`, one retrace-bit wait per level). U-206 resolved for E3.
4. **Highlight loop** and un-highlight: identical to E2 (`0000:2E58..2ECE`, `39EA..3A59`), including the ramp of the start cursor at loop entry and the 11 steps per frame. C-0125: fade-in f270..303 (2 frames per level), start ramp f303..307, fade-out on Enter f433..466.

Items (selection: Space, Enter, joystick button 1 or 2; Up/`8`/stick up and Down/`2`/stick down wrap 0..8 and skip items 1 and 3 while mission = 0; Esc = quit box; confirmation `UpCase(key) = 'Y'`):

| Item | E3 action | Bookkeeping |
|---:|---|---|
| 0 | New game (`0000:3203..3418`); box 2 only when mission ≠ 0. Mission := 1, kills := 0, fade out, `Main_CampaignLoop` (the skip flag is **not** cleared, OB-050) | advance = 1: mission += 1, kills += mission kills, cursor := 3 (with dead buffer stores to entries 224 and 227). Game over: mission := 0. Then `Snd_PlayMusic(7)`, flush BIOS keyboard, `UI_LoadMainMenu(0)` (which flushes again) |
| 1 | Save (only when mission ≠ 0, `3611..3639`): fade out, `Sav_SaveScreen` `04A0:6413(mission, kills, palette)`, `UI_LoadMainMenu(0)` | none |
| 2 | Restore (`341B..360E`); box 2 only when mission ≠ 0; fade out, `Sav_RestoreScreen` `04A0:5450(palette)`. If mission ≠ 0 afterwards: buffer entry 227 red and green := 63 (dead: the loader resets it), cursor := 3. `UI_LoadMainMenu(0)` | then cursor = 3: buffer entry 227 red, green := 0; else entry 226 red := 16 (as E2) |
| 3 | Next mission (`3672..36CC`): advance := 0, fade out, `Gfx_InitUnchained`, `Main_CampaignLoop` (no flush afterwards; the loader flushes) | advance = 1: mission += 1, kills += mission kills; **mission = 20** → cursor := 7, mission := 0 (episode complete, test at `0000:369E`). Game over: mission := 0, cursor := 0. Then `UI_LoadMainMenu(1)` |
| 4 | Help (`37A0..37EB`): fade out, `UI_MenuHelp`, re-fade-in of the menu (the routine assigns `tdmen.pal` but never opens it) | none |
| 5 | Story so far (`36CF..379B`): as E2 (fade out, `Scene_BackgroundStory`, reload `tdmen.pal`, entries 224..233 := (16, 0, 0), cursor red 63, fade in, red 16) | an Esc on a page leaves the skip flag at 1 (OB-050) |
| 6 | In-process: fade out, `Snd_PlayMusic(69)`, `Scene_Aboutus` (30 s or key), `UI_LoadMainMenu(1)` | an Esc leaves the skip flag at 1 (OB-050) |
| 7 | As 6 with `Scene_Order` (**45 s** or key) | as 6 |
| 8 | Quit: box 1; `Y` sets cursor 8 and the quit flag; the loop ends with a fade-out, DS:066E := cursor, `Crt.TextMode(3)` | return to `Main_Entry` step 13 |

### 4.3 One mission block (`Main_CampaignLoop` `0000:0BC5`)

```text
game over := 0; advance := 1                       (the skip flag is NOT cleared: E2 clears it)
the block for g_MissionNumber runs (tests 1..6, 8..19 at 0BD9, 0E00, ..., 2779; no test for 7 or 20;
  the end of each block falls into the next test, which fails, down to 2988 -> RETURN)

[block 1 only] if skip then go to FLY              (0BE3, before the first step; OB-050)
pre-flight steps; after a step with a skip test: if skip then go to FLY
FLY:
  Sim_FlyMission(flight)
  if died:    if RetryPrompt() then go to FLY
  if died:    ending A
  if aborted: go to RETURN
  Snd_PlayMusic(8); Debrief(mission kills, kills to date, outcome)       [not block 19; block 11: §4.4.2]
  [blocks 2, 3, 4, 5, 9, 12] if outcome = 0:
      if RetryPrompt() then go to FLY
      mission -= 1; go to RETURN                                         (advance stays 1)
  [blocks 8, 10] outcome rule (§4.4)
  post-flight steps; after a step with a skip test: if skip then go to RETURN
RETURN:
  if aborted: advance := 0
```

`RetryPrompt` returns true for every key but Esc and consumes one `Random(10)` value. A retry re-flies from `FLY`: the pre-flight story is not repeated; the flight handling (including another debrief) is. Mission 7 or 20 runs no block and returns with advance = 1 (OB-051).

### 4.4 Per-mission sequence

Notation as E2 §4.4 with §2's captions: `Mn` music n (§3.1); `Dn` dialogue scene n; `bg[a/b/c]` scene card; `B` = M8 + debrief; `R` retry prompt; a step marked `'` has **no** skip test after it. Every other step is followed by a test; the last step of each part has none or one that does not matter. "std" = died → R (accept → the same flight; decline → ending A), aborted → return. "fail → R/replay" = after B, outcome 0 → R (accept → the same flight; decline → mission −= 1, return with advance 1, so the menu offers the same block again).

| Abbr. | Routine `04A0:` | Abbr. | Routine `04A0:` | Abbr. | Routine `04A0:` |
|---|---|---|---|---|---|
| cmd | `3DB2` command | mar | `227C` marauder | mr3 | `27CC` maraud3 |
| cnv | `35A8` convers | mel | `3944` melting (15 s) | sel | `3775` sel-pain (15 s) |
| in2 | `2EE4` intro2 | mn1 | `3F7E` mining1 | sh0 | `30B1` shuttle0 (15 s) |
| ion | `420D` ionstat | mn2 | `449C` mining2 | vhg / vid / vlb | `2C86` vhang / `29AE` vidcom / `333D` vlab |
| kol | `3B14` kolorvci | mr2 | `24BB` maraud2 | lab | `2013` tdlab |

| Block | Pre-flight | Flight and outcome rule | Post-flight (after B) |
|---:|---|---|---|
| 1 | (skip test first, OB-050) M6 sh0[p.0/p.1/_] D1 T(p.3) D2 sh0[p.4/p.5/_] D3 sh0[p.6/p.7/_] D4 M5 mar[p.8/p.9/p.10] D5 T01 M10 mn1[p.10/1.0/_] D6 M3 mn1[1.1/1.2/_] D7 | F1 std | M6 mn1[1.1/1.3/_] D8 M4 cmd[1.4/1.5/_] D201 |
| 2 | M6 cmd[1.4/1.6/_] D202 T02 D11 M3 mn1[1.1/1.2/_] D12 | F2 std; fail → R/replay | M3 mn1[1.1/2.0/_] D203 M6 lab[2.1/2.2/_] D204 M5 mr2[2.3/2.4/p.10] D14 M10 sh0[p.7/p.6/_] D15 D16 |
| 3 | M13 cnv[2.5/2.6/_] D17 M4 cmd[1.4/p.10/_] D18 M10 cmd[2.7/2.8/_] D19 (no title card) | F3 std; fail → R/replay | M3 mn1[1.1/2.0/_] D20 M13 cmd[2.7/p.10/_] D21 D22 M6 lab[2.1/2.9/_] D23 |
| 4 | T04 M10 lab[2.1/p.10/_] D24 M6 cnv[4.0/p.10/_] D124 T(4.1) D25 | F4 std; fail → R/replay | M3 mn1[1.1/2.0/_] D26 M5 mr2[p.8/4.2/p.10] D27 |
| 5 | M6 cnv[4.3/p.10/_] D28 T05 M4 cmd[2.7/5.0/_] D29 M13 cnv[4.3/p.10/_] D30 M4 cmd[2.7/2.8/_] D31 M6 cmd[2.7/5.1/_] D32 M3 mn1[1.1/2.8/_] D33 | F5 std; fail → R/replay | M5 mr2[p.8/4.2/p.10] D34 |
| 6 | M5 mr3[5.2/_/_] D35 T06 M4 cmd[2.7/5.1/_] D234 M6 T(p.10) D36 M3 mn1[1.1/1.2/_] D37 | F6 std, B; M5 mr3[6.0/_/_] D38 kol[6.1/6.2/_] D39 T07 D40 M13 T(p.7) D41 (skip tests here go to F7); F7 std, B; **mission := 7** (§4.4.1) | M10 mn2[7.0/1.3/_] D42 M6 lab[2.1/1.5/_] D43 M5 kol[6.1/p.10/_] D44 sel[7.1/_/_] |
| 8 | M5 mr3[7.2/_/_] D46 T08 kol[6.1/p.7/_] sel[8.0/_/_] D48 M3 mn2[1.1/1.2/_] D47 | F8 std; after B, outcome 0: M3 mn2[1.1/1.3/_]' D49' R (accept → F8; **decline → continue as if outcome 1**) | M10 cmd[7.0/8.1/p.10] D50 M5 kol[6.1/8.2/_] D51 |
| 9 | T09 M5 kol[6.1/p.10/_] D53 M3 mn2[1.1/1.6/_] D54 | F9 std; fail → R/replay | M3 mn2[1.1/1.3/_] D55 M10 kol[6.1/p.10/_] D56 |
| 10 | T10 M10 lab[2.1/1.6/_] D57 M3 mn2[1.1/1.2/_] D58 | F10 std | M3 mn2[1.1/1.3/_]', then outcome 1: D60; outcome 0: D59' R (accept → F10; **decline → advance := 0, aborted := 1, return**) |
| 11 | T11 M5 kol[6.1/p.10/_] D61 M4 cmd[8.1/7.0/_] D62 M3 mn2[1.1/2.8/_] D63 | F11 std **without B**; M13 D64' F21; B; then died → R (accept → **F11**), A; aborted → return (§4.4.2) | M3 mn2[1.1/1.3/_] D65 |
| 12 | M5 kol[6.1/1.6/_] D66 T12 M3 mn2[1.1/2.8/_] D67 | F12 std; fail → R/replay | M3 mn2[1.1/1.3/_] D68 M10 cmd[7.0/8.1/1.2] D168 |
| 13 | T13 M6 lab[2.1/1.6/_] D69 M3 mn2[1.1/1.2/_] D70 | F13 std | M13 ion[13.0/1.3/_] D71 |
| 14 | T14 M10 kol[6.1/p.10/_] D72 T(1.5) D73 M13 vhg[14.0/2.2/_] D74 | F14 std | M3 mn2[1.1/1.3/_] D75 |
| 15 | M5 kol[6.1/_/_] D76 T15 M4 cmd[7.0/8.1/_] D77 M10 lab[2.1/15.0/_] D78 M4 cmd[7.0/8.1/p.10] D79 M6 vid[15.1/p.10/_] D80 M3 mn2[1.1/2.8/_] D81 | F15 std | M3 mn2[1.1/1.3/_] D82 |
| 16 | T16 M5 kol[6.1/_/_] D83 M10 vid[15.1/p.10/_] D84 M3 mn2[1.1/1.5/_] D85 | F16 std | M6 lab[2.1/1.3/_] D86 |
| 17 | T17 M5 kol[6.1/_/_] D87 lab[2.1/17.0/_] D88 M5 vlb[17.1/p.10/_] D90 M10 vlb[17.1/17.2/_] D91 | F17 std | M13 mn2[17.3/2.0/_] D92 M5 kol[6.1/p.10/_] D93 |
| 18 | T18 M10 vid[15.1/_/_] D94 M5 kol[6.1/p.10/_] D194 M13 vid[15.1/_/_] D95 | F18 std | M5 mr3[18.0/18.1/18.2] D96 mr3[18.3/2.2/_] D97 mr3[18.4/18.5/_] D197 mr3[18.6/18.7/_] D98 mr3[18.8/_/_] D99 in2[18.9/_/_] M11 mr2[18.4/18.10/18.11] D100 |
| 19 | M10 kol[6.1/_/_] D101 M13 sh0[18.12/_/_] in2[18.13/_/_] D102 T19 M6 kol[6.1/p.10/_] D103 mel[19.0/_/_] D105 M13 vhg[14.0/2.2/_] D106 | F19: died → R / A; aborted → return; **no B** | M10 mn2[19.1/19.2/_] D107 M11 kol[6.1/_/_] D108 kol[p.10/_/_] D109 T(end) (§4.5) |

Block addresses (start / flight call; the retry and skip target is the flight call − 2): 1 `0BE3`/`0D5B`, 2 `0E0A`/`0E94`, 3 `0FE7`/`107C`, 4 `1199`/`123D`, 5 `1313`/`1464`, 6 `1504`/`15E5` (F7 `16DC`), 8 `17E4`/`18AA`, 9 `198D`/`1A07`, 10 `1ADD`/`1B57`, 11 `1BFF`/`1CAE` (F21 `1CE9`), 12 `1D62`/`1DDC`, 13 `1EB2`/`1F2C`, 14 `1FAF`/`2053`, 15 `20CC`/`221D`, 16 `2296`/`2345`, 17 `23BE`/`249C`, 18 `254B`/`25FA`, 19 `2783`/`28B5`. Ending A `298A`; common return `29C6`.

Notes:

- Esc after a tested pre-flight step jumps to the flight call itself; music requested before it keeps playing until `Sim_FlyMission` stops it. Esc after a tested post-flight step returns with advance = 1: the mission counts as completed.
- The first screen of every block has a skip test after it, so an Esc there skips the rest of the pre-flight sequence (block 6: up to F6 only).
- Steps without a skip test: block 8's outcome-0 path (mn2 card, D49; the retry prompt clears the flag anyway), block 10's post-flight mn2 card and D59 (Esc there does not stop D59/D60 or the prompt), block 11's D64 (F21 follows directly).
- Dialogue: 111 scene ids are played, each once; they are exactly the 111 ids of `dialogu3.dat` and of `safedia3.dat`. Ids above 127 (168, 194, 197, 201..204, 234) are pushed as sign-extended bytes; the byte is the id.
- No block stops the music outright and none plays `low` before a flight. Blocks 4, 9, 10, 11, 13, 14, 16, 17 and 18 start with their title card before any music call, so the menu song continues under that title card.

#### 4.4.1 Block 6 (F6 and F7)

| Event | F6 (`15E5`) | F7 (`16DC`) |
|---|---|---|
| Esc before the flight | any tested pre-flight step → F6 | any tested step of the middle part (`mr3`, D38, `kol`, D39, T07, D40, T(p.7), D41) → F7 |
| Died | R: accept → F6 again; decline → ending A (dossier total = kills to date + F6 kills) | R: accept → **F7** again (F6 is not repeated); decline → ending A (kills to date + F7 kills) |
| Aborted | return, advance 0, mission 6 | return, advance 0, mission still 6 (the store of 7 comes later): Next mission replays block 6 **from its first step, F6 included** |
| Outcome | always 1 (no flight-6 test); not used | always 1; not used |
| Debrief | B with F6's kills; heading "mission 6" | B with F7's kills; heading also "mission 6" (the store of 7 follows it) |
| Kills | lost: F7's start zeroes `g_MissionKills` | added by the menu |
| After | the middle part, then F7 | mission := 7 (`0000:171F`), post-flight story; menu: 7 + 1 = 8 |
| Phase at F7 start | — | always 100: F7 is entered only after F6 ended by success (which needs phase 100) or as a retry of F7 itself (which never lowers the phase), and `Msn_Setup` case 7 has no phase store (U-270 resolved; [e3-mission-logic §5.7](../sim/e3-mission-logic.md), OB-043) |

#### 4.4.2 Block 11 (F11 and F21)

| Event | F11 (`1CAE`) | F21 (`1CE9`, `m21.td3`) |
|---|---|---|
| Esc before the flight | any tested pre-flight step → F11 | no skip test: after F11, M13 and D64 always play and F21 follows (an Esc in D64 only ends D64) |
| Died | R: accept → F11; decline → ending A (kills to date + F11 kills) | **B first**, then R: accept → **F11** (then M13, D64 and F21 again); decline → ending A (kills to date + F21 kills) |
| Aborted | return, advance 0, mission 11 (no debrief) | **B first**, then return, advance 0: Next mission starts at F11 again |
| Outcome | not used (no debrief after F11) | always 1 (no flight-21 test): the debrief always shows a rating |
| Debrief | none | one, with F21's kills; heading "mission 11" |
| Kills | lost: F21's start zeroes `g_MissionKills` | added by the menu |

### 4.5 Endings and the episode end

| Ending | Trigger | Steps (`0000:`) | Result |
|---|---|---|---|
| A | died and the retry was declined (any flight of any block) | `298A`: M12 (`298C`); unless `g_NoWorx`: `WORX_SetMusicLoop(0)`; `UI_PlayerDossier(kills to date + mission kills)` (25 s or key); unless `g_NoWorx`: loop on | game over := 1, advance := 0, return |
| Episode end | block 19's post-flight story completes | after D109: `T(end)` (title-style card `e3.ending`, 5 s or key) with `moodmaj` still playing; return with advance = 1 | menu (item 3 path): 19 + 1 = 20 → mission := 0, cursor := 7 ("ordering info"), `UI_LoadMainMenu(1)` restarts `menu.CMF` |

E3 has no ending B or C: the campaign never tests `g_MissionLost` and contains only one ending routine. An Esc anywhere in block 19's post-flight story also skips `T(end)`; the episode still completes (advance = 1). After ending A the menu applies "game over" (New Game path: mission := 0; Next Mission path: mission := 0, cursor := 0); kills are not added. The debrief is never shown on an ending (but block 11 shows F21's debrief before the retry prompt that may lead to it).

### 4.6 Bookkeeping on return to the menu

| Return | advance | Menu result (item 3 path; the item 0 path is the same except that it never reaches 20, sets cursor := 3 on success and leaves the cursor alone on game over) |
|---|---:|---|
| Completed (also Esc in the post-flight story) | 1 | mission += 1; kills += kills of the **last** flight flown; cursor stays 3 |
| Block 6 completed | 1 | mission 7 + 1 = 8; kills += F7 kills only |
| Block 11 completed | 1 | mission 12; kills += F21 kills only |
| Block 19 completed | 1 | mission 20 → 0; cursor := 7 |
| Aborted from the flight menu | 0 | nothing changes (block 11: after F21's debrief) |
| Ending A | 0 | mission := 0, cursor := 0 |
| Outcome replay declined, blocks 2, 3, 4, 5, 9, 12 | 1 | mission was decremented, so += 1 restores it: the same block is offered again; kills += the failed attempt's kills (OB-027, CONFIRMED for E3) |
| Block 8 retry declined | 1 | continues as a success: post-flight story, mission += 1, kills added |
| Block 10 retry declined | 0 | mission and kills unchanged; the same block is offered again |
| Mission 7 or 20 (edited save only) | 1 | no block runs: mission += 1 and kills += the stale `g_MissionKills` (OB-051) |

### 4.7 Save and restore (`td3.sav`, `td3.stt`, `td.sss`)

Same code as E1 `051A:76EA` / `7EEA` with E3 file names and E3's flush on entry ([FORMATS §13.2, §13.3](../../re/FORMATS.md); U-011 resolved for E3):

| File | Layout |
|---|---|
| `td3.sav` | 10 slots × 3 bytes; slot s (1..10) at offset (s − 1) × 3: **mission** (u8), then **kills to date** (i16, little-endian). A mission of 0 or > 20 means "empty slot" (`04A0:5631..5641`). Bytes 30..69 are never read or written. The shipped file holds `02 0E 00` in all ten slots (mission 2, 14 kills) and 40 × FFh |
| `td3.stt` | 10 text lines, one name per slot (260 bytes shipped); read line by line, rewritten whole on save |
| `td.sss` | the slot-bar picture (shared by all episodes) |

**Restore** (`04A0:5450`): flush the BIOS keyboard buffer; load `td.lt2`, `td3.stt`, `td3.sav` (empty slots are marked with `e3.saves.restore[0]` and their mission set to 0 in memory), `td.sss`; draw; fade in; cursor on **slot 1**; joystick Y and buttons := 0. Loop: Up/`8`/stick up and Down/`2`/stick down move the cursor (wrapping 1..10); Space, Enter or a button on a non-empty slot sets `g_MissionNumber` (`04A0:5B19`) and `g_KillsToDate` (`5B2A`) from the slot and leaves; on an empty slot nothing happens; Esc leaves without a change. Fade out. Back in the menu (§4.2 item 2): if the mission is not 0 the cursor goes to item 3 and the menu reloads without restarting `menu.CMF`.

**Save** (`04A0:6413`): flush, load the same files, choose a slot and edit its name (as E1), then write all ten slots back at their offsets (the chosen one with the current mission and kills; slots read as empty, mission 0 or > 20, are written with mission 0 and their old kills word, `04A0:65F4..660A`) and rewrite `td3.stt` (`04A0:6DF1..6EC7`; empty slots then probably get the empty-slot label `e3.saves.save[0]` as their name, LIKELY).

`td3.sav`, `td3.stt` and `td.sss` are opened with I/O checking on; if one is missing the original probably ends with a runtime error (U-281).

## 5. Edge cases and original bugs

| Quirk | Original | Port policy (proposal) |
|---|---|---|
| Block-1 skip shortcut (**OB-050**, new) | `Main_CampaignLoop` does not clear the skip flag and block 1 tests it before its first step (`0000:0BE3`). After an Esc that ended the about or order screen, a story-so-far page or the death dossier, or an Esc in a post-flight story (or in a pre-flight story followed by an abort), New game (or Next mission with mission 1) goes straight to flight 1: no cards, no dialogue, no M6, the menu song plays until the flight stops it. Blocks 2..19 are not affected (their first routine clears the flag) | Preserve (Classic); harmless |
| Missions 7 and 20 in a save (**OB-051**, new) | The restore screen accepts missions 1..20, but there is no block 7 or 20: Next mission plays nothing, the menu adds the stale `g_MissionKills` of the last flight flown (0 after start-up) and moves to 8 or 21; from 21 every Next mission adds 1 until the byte wraps to 0. Normal play never stores 7 or 20 (block 6 hands 7 + 1 to the menu; the episode end resets 20 to 0); only edited or foreign saves | Preserve (only edited saves) |
| Paired flights and replays (**OB-027**, extended) | Besides OB-027's items (F6/F11 kills lost, both block-6 debriefs say mission 6, F21 death retry restarts at F11, off-screen third caption of `sel-pain`/`melting`): declining a replay in blocks 2, 3, 4, 5, 9, 12 adds the failed attempt's kills (now CONFIRMED); block 10's decline adds nothing; block 8's decline counts as a success; aborting F7 or F21 forces F6 or F11 to be flown again; F21's debrief is shown even when the player died in or aborted F21; block 19 has no debrief | Preserve |
| Item 3 colours (**OB-041**, applies to E3) | Same loader as E2 (`0000:29E5`): "Next mission" dim red after every load; red instead of yellow with cursor 3; yellow flash on leaving it | Preserve (T0, Classic) |
| Timed waits discard typed-ahead keys | Every timed screen flushes the keyboard on entry; a key pressed during a card's loading or fade-in is lost | Preserve (T2; matters for input replays) |
| Dialogue pages and the retry prompt keep typed-ahead keys | no flush | Preserve |
| Menu music | `menu.CMF` started once at program start (E2: twice) | Preserve |
| `/?` and the memory gate halt after `WORX_Init` without `WORX_Shutdown` (U-250) | as E2 | Not ported (OB-002); Verifier: do not run `td3 /?` in an oracle session |
| 16-bit clock in timed waits (OB-034) | as E1 | As E1 |
| No text mode between flights | E3 drops E2's text-mode switch at the end of `Sim_FlyMission` | Preserve |

## 6. Test vectors (static unless marked; U-280)

| # | Setup / input | Expected | Evidence |
|---|---|---|---|
| 1 | `Snd_PlayMusic(44)`, `(45)`, `(69)`, `(13)`, `(14)` with music on | opens `td03.CMF`, `intro.cmf`, `intro.cmf`, `mood3.CMF`; 14 opens nothing (replays the freed buffer) | `0C5D:07F3` |
| 2 | Flight song for flights 1, 2, 3, 6, 7, 11, 21 | `td01`, `td03`, `td02`, `td02`, `td01`, `td03`, `td02` | `0C5D:B853` |
| 3 | `td3 /sb`, no joystick, keys `2`, Enter (after `tdmen.pal` opens), Esc 121 frames after `shuttle0.scr` opens | script 2; menu; `mood.CMF`, `td.lt2`, `shuttle0.scr`; then flight files (`td.pal` first, `m1.td3`, `td3.map`); no dialogue file opened after the menu | **C-0036**: `tdmen.pal` f253, `mood.CMF` f466, `shuttle0.scr` f470, `m1.td3` f627, `td3.map` f628, tick 1 f694 |
| 4 | As 3 but Esc sent during the menu fade-out | the Esc is discarded (flush); the `shuttle0` card stays 15 s, then D1 opens `safedia3.dat` | `04A0:0277` |
| 5 | Menu right after start-up | `menu.CMF` opened once; DAC 224 = (63, 0, 0), 225..233 = (16, 0, 0) | `0000:2E3F`, `29E5` |
| 6 | New game, complete block 1 | mission 2, kills = F1 kills, cursor 3 | `0000:33C1..33E7` |
| 7 | Block 2, outcome 0, Esc at the retry | menu: mission 2 again, kills += F2 kills | `0000:0ED7..0EFB` |
| 8 | Block 10, outcome 0, Esc at the retry | menu: mission 10, kills unchanged | `0000:1BCC..1BE6` |
| 9 | Block 8, outcome 0, Esc at the retry | `mood2` + `command` card, D50, `vulture` + `kolorvci` card, D51; menu: mission 9, kills += F8 kills | `0000:18ED..197E` |
| 10 | Block 6 completed | two debriefs headed "mission 6"; menu: mission 8, kills += F7 kills | `0000:171F`, `04A0:6083` |
| 11 | Block 11, die in F21, any key at the retry | debrief, then `m11.td3` opens (F11), then `mood3`, D64, `m21.td3` | `0000:1CF0..1D16` |
| 12 | Block 19 completed without Esc | no `low.CMF` after F19; `T(end)` 5 s; menu: mission 0, cursor 7, `menu.CMF` | `0000:28D6..2988`, `369E` |
| 13 | Fresh start, menu item 6, Esc on the about screen, then item 0 | after the fade-out no card and no `mood.CMF`: the flight files open at once (OB-050) | `0000:0BE3` |
| 14 | `TD3.SAV` slot 1 = `07 00 00`, restore, Next mission | no screen; menu: mission 8, kills 0 (stale mission kills after start-up) | OB-051 |
| 15 | Dialogue ids played | 111 distinct ids = all scenes of `dialogu3.dat` and `safedia3.dat` | file scan + walk |
| 16 | Menu items 6 and 7 | `intro.cmf`; `aboutus.scr` 30 s or `order.scr` 45 s or a key; menu reloaded with `menu.CMF`; nothing written | `0000:363C..366F` |

## 7. Presentation and audio hooks (program-level music)

| When | Song | Address |
|---|---|---|
| Menu loop entry | 7 (loader with argument 1), once | `0000:2E55` → `2A12` |
| Menu reload after items 3, 6, 7 | 7 (loader with argument 1) | `0000:2A12` |
| Return from a new game | 7 (explicit; loader with argument 0) | `0000:33F8` |
| Items 6, 7 | 69 `intro.cmf` | `0000:3648`, `3663` |
| Flight start / song / end | stop at entry; §3.1; stopped and freed at the end | `0C5D:A9EB`, `B853..B8E3`, `EBC6` |
| Debrief | 8 before each of the 18 debriefs | campaign |
| Ending A | 12 once (loop off, then on) | `0000:298C` |
| Episode end card | none (block 19's `moodmaj` continues) | `0000:2983` |

Save, restore, help and story-so-far keep the menu song playing.

## 8. Running TD3.EXE directly (for the Verifier)

Run on a **scratch copy** of the game directory: `td3.exe` writes `td.cfg` when the configuration screen is left and `td3.stt`/`td3.sav` when a game is saved. Nothing else is written. Keep `td3.sav`, `td3.stt` and `td.sss` in the copy (U-281).

**Files read at start:** `td.cfg` (opened twice when present: an existence test, then the 3-byte read; if it does not open, the configuration screen appears unless a switch sets the music device); `dialogu3.dat` (open/close only, existence test); `td.lt2` and `td-let.cin` (script choice); `td.joy` (calibration, only with the joystick controller); then the menu: `menu.CMF` (music on), `td.men`, `tdmen.pal`, `td.men`. Dialogue scenes later open `td.lt2`, `td.pal`, `dialogu3.dat` (script 1) or `safedia3.dat` (script 2) and `newfaces.td3`.

**Recommended DOSBox setup:** as E2 §8: Sound Blaster with OPL, `joysticktype=none` (the shipped `td.cfg` = 2, 2, 2 asks for a joystick; with none the controller becomes keyboard and no calibration screen appears), default conventional memory. Command: **`TD3.EXE /sb`** (one parameter only: music and SFX on the SB, no configuration screen, `td.cfg` read but not written). `/nosound` also skips the configuration screen but removes WORX (different timer setup); use it only for silent runs. Never run `td3 /?` in the shared rig (U-250).

**Start to the flight of mission 1:**

| Step | Screen | Ends on | Keys |
|---:|---|---|---|
| 1 | Nothing visible (WORX init, text screen cleared, `td.cfg`, `dialogu3.dat` test) | — | — |
| 2 | Script choice (mode X, fades in) | `1` or `2` only, no timeout, no flush | `2` (edited script), e.g. 140 frames after `td-let.cin` opens |
| 3 | Joystick calibration: skipped with `joysticktype=none` | — | — |
| 4 | Main menu: the loader **flushes the keyboard**, then `menu.CMF`, `td.men`, `tdmen.pal`, `td.men`, fade-in; cursor on item 0 | Space/Enter/button, no timeout | Enter **after `tdmen.pal` opens** (a key typed before the loader starts is discarded); no confirmation with no game in progress |
| 5 | Menu fades out; `mood.CMF`; `shuttle0` card (`td.lt2`, then `shuttle0.scr`) with `p.0`/`p.1` | 15 s or a key | Esc **during the card's 15 s wait** → straight to flight 1 |
| 6 | Flight 1 (`td01.CMF`; files `td.pal` first, then `td.sc3` ... `m1.td3`, `td3.map`, `td3.blk`) | — | — |

**The keyboard flush (E3 only).** `Flow_WaitSecondsOrKey` empties the BIOS keyboard buffer when it starts, and it starts only after the card has read its picture, drawn its captions and faded in. A key sent before that moment (during the menu fade-out, the loading or the fade-in) is **discarded**, and the card then runs its full time; the next step (here D1, which opens `dialogu3.dat`/`safedia3.dat`) shows that the Esc was too early. In E2 capture C-0016 (fixed 20,000 cycles) the scene card's fade-in ran from 20 to 53 frames after its `.scr` file opened, so in E3 send the Esc **at least ~60 frames after the `.scr` opens**; 120 frames is a safe choice, well inside the windows (15 s ≈ 1,050 frames for `shuttle0`, `sel-pain`, `melting`; 10 s ≈ 700 for other cards; 5 s ≈ 350 for title cards, which load only `td-let.cin` and fade in within ~35 frames, so +90 after `td-let.cin` opens is safe). The E2 rule "Esc may be pressed at any time after 0.5 s" does **not** hold for E3. Dialogue pages and the retry prompt do not flush: there a buffered key is used at once.

Oracle scenario (the key lines of C-0036; scenario syntax in `re-work\dosbox\ORACLE.md` §5, outside the repo, ADG §11):

```text
open:td-let.cin+140      press 2
open:tdmen.pal+180       press enter
open:shuttle0.scr+121    press esc
open:m1.td3+1            mark m1-open
```

**Confirmed (C-0036, C-0041).** With `TD3.EXE /sb` in the oracle rig (preset P1, private copy), `2` 140 frames after `td-let.cin` opens, Enter 180 frames after `tdmen.pal` opens and Esc **121** frames after `shuttle0.scr` opens reach flight 1: `m1.td3` opens at frame 627, the first flight tick is frame 694 (C-0036). With `TD3.SAV` bytes 0..2 := `08 00 00` and `-KeepState`: `2`; Down 180 and Enter 200 frames after `tdmen.pal` opens; Enter 91 frames after `td3.sav` opens; Enter 180 frames after the second `tdmen.pal` open; Esc 121 frames after `maraud3.scr` opens: flight 8 starts (`m8.td3` frame 1003, `td4.map` 1004, tick 1 at 1070) (C-0041). Nothing is written to the game directory on these paths.

**Without Esc** the block runs: `shuttle0` card 15 s → D1 (each page waits for a key, no timeout; Esc ends the scene and skips to the flight) → `T(p.3)` 5 s → D2 → `shuttle0` 15 s → D3 → `shuttle0` 15 s → D4 → `vulture` + `marauder` card 10 s → D5 → T01 5 s → `mood2` + `mining1` card 10 s → D6 → `hangar` + `mining1` → D7 → flight 1. Each of these steps is followed by a skip test. Shortcut: an Esc on the about screen (menu item 6) before New game skips the whole sequence (OB-050).

**After flight 1:** death → retry prompt (keyboard, no timeout): any key but Esc flies again; Esc → ending A (`death.CMF` once, dossier 25 s or key) → menu with no game. Abort from the flight's Esc menu → menu at once (`menu.CMF` restarts), mission still 1, cursor on item 0. Success → `low.CMF` debrief (15 s or key) → `mood` + `mining1` card → D8 → `satair` + `command` card → D201 → menu (mission 2, cursor 3). To quit: Esc in the menu, `Y`; text mode, WORX shutdown, the exit notice, DOS cursor on row 10.

**Any mission directly (via `TD3.SAV`).** Slot 1 is bytes 0..2 of `TD3.SAV`: mission, then kills to date (little-endian i16). In the scratch copy set them to `NN 00 00` (NN in 1..6 or 8..19; 7 and 20 run no block, OB-051). The shipped file already holds mission 2 with 14 kills in every slot, so block 2 needs no edit. At the menu press Down (cursor 0 → 2: items 1 and 3 are skipped with no game), Enter; the restore screen flushes the keyboard and loads, then Enter (cursor on slot 1; Space also selects); back at the menu (reloaded without restarting `menu.CMF`, keyboard flushed again) the cursor is on item 3: Enter. Block NN starts.

```text
open:td-let.cin+140      press 2
open:tdmen.pal+180       press down
open:tdmen.pal+200       press enter
open:td3.sav+90          press enter
open:tdmen.pal#2+180     press enter
<first-screen file of block NN>+120 (title card: +90)   press esc
```

| Block | First screen (and the files it opens) | Esc there |
|---:|---|---|
| 1 | `mood.CMF`, then the `shuttle0` card (`td.lt2`, `shuttle0.scr`), 15 s. If the skip flag is already 1 there is no card at all (OB-050) | skips the rest of the pre-flight sequence straight to F1 |
| 6 | `vulture.CMF`, then the `maraud3` card (`td.lt2`, `maraud3.scr`, first open), 10 s | straight to **F6**. After F6 and its debrief the middle part starts with another `maraud3` card (second open of `maraud3.scr`); an Esc there goes straight to F7. F7 cannot be reached without finishing F6 (an abort replays block 6) |
| 8 | `vulture.CMF`, then the `maraud3` card (`td.lt2`, `maraud3.scr`), 10 s | straight to F8 |
| 9 | the title card `T09` (`td-let.cin`, second open after the script choice), 5 s, with no music call before it (`menu.CMF` keeps playing) | straight to F9 (unlike E2 block 9, the title card is the first screen and is skippable) |

Every other block's first screen also has a skip test after it (blocks 2, 3, 5 `command`/`convers` cards; 12, 15, 19 `kolorvci`; 4, 10, 11, 13, 14, 16, 17, 18 a title card). F21 is reached only by finishing F11 (then `mood3.CMF`, D64, `m21.td3`).

Waits without a timeout: script choice, configuration screen, joystick calibration, main menu, confirmation boxes, dialogue pages, retry prompt, help pages, save/restore screens. Timed waits (all end on a key, and on a joystick button where a joystick is present; all flush the keyboard on entry): title cards 5 s, scene cards 10 s (15 s for `shuttle0`, `sel-pain`, `melting`), debrief 15 s, dossier 25 s, about 30 s, order 45 s, story pages 30 s and 45 s.

## 9. Open questions and corrections

### 9.1 Open questions

- [ ] **U-280** (new): E3 front end in captures. §8's start-up path and the restore path to block 8 are confirmed (C-0036, C-0041). Still to confirm: that a key sent before a card's wait starts is discarded, the block-1 shortcut of OB-050, `menu.CMF` opened once at start, the restore path, cursor 7 after block 19 and the OB-041 colours in E3.
- [ ] **U-281** (new): with `td3.sav`, `td3.stt` or `td.sss` missing, do the save/restore screens end the program with a runtime error (I/O checking is on after each `Reset`), leaving WORX installed as in U-250? The same code exists in E1 and E2. Do not test it in the shared rig without a hang plan.
- [ ] **U-282** (new): OB-051 in the rig: restoring a slot with mission 7 (or 20) and choosing Next mission shows nothing and moves the menu to 8 (or 21), adding the stale mission kills.
- U-250: applies to E3 unchanged (memory-gate Halt at `0000:40EE` and `/?` Halt at `44A8`, both after `WORX_Init`).
- U-011: resolved for E3 (§4.7): mission byte plus kills to date; the save writes all ten slots back.
- U-206: E3 menu fades and loader resolved (§4.2); the dialogue "02 line uses its own entry" rule stays open.

### 9.2 Corrections to [episode-deltas](../../re/episode-deltas.md) §7–§8 (rev 2)

The §8.2 E3 table is correct for the order of every call in all 18 blocks, and the §8.1 notes are correct (verified against the token walk). Corrections and additions:

1. §7 "Game over ... E3 has one (`0000:298A`)": that is the ending label; the music call is at `298C`. E3 also never passes index 45 (it maps to `intro.cmf`, but only 69 is used), and E3 starts the menu song only from the loader (once at program start).
2. §8.1 "play restarts at F11 (`0000:1CAC`)": `1CAC` is the retry label; the F11 call is at `1CAE`. The restart also replays M13 and D64 before F21. Add: an abort in F21 also shows the debrief first and then forces F11 to be flown again; an abort in F7 forces block 6 (F6 included) to be replayed.
3. §8.2 E3 table: block 1 begins with a skip test (OB-050); block 8's outcome-0 path (mn2 card, D49), block 10's post-flight mn2 card and D59, and block 11's D64 have no skip test. Block 10's post-flight mn2 card is shown for both outcomes before D59/D60.
4. §8.2 E3 notes, "Endings": `g_MissionLost` is set only in flight 5 (`0C5D:99FD`); it just ends the flight, and the flight-5 outcome test normally gives 0, so block 5's replay rule applies (LIKELY). It is not "folded into the outcome" by code.
5. §8.2 "Decline-and-replay paths ... (OB-027, LIKELY)": CONFIRMED for E3 (blocks 2, 3, 4, 5, 9, 12 jump to the common return `29C6`, which clears advance only when aborted). Block 10's decline is different (advance := 0) and block 8's decline continues as a success.
6. §8.3 E3 deltas not listed: no song-7 call at menu-loop entry; `UI_LoadMainMenu` flushes the keyboard on entry; item 3 has no flush after the campaign (the loader's flush replaces it); the menu loop sets text mode 3 on quit; `Crt.ClrScr` after the memory gate; `Sim_FlyMission` has no text-mode switch; `Main_CampaignLoop` does not clear the skip flag. "Menu items 0–5, 8: as E1" is not exact (E2's OB-041 loader changes apply to E3).
7. §8.3 "Save files": add the slot format and that the E3 restore accepts missions 1..20 although 7 and 20 have no block (OB-051).

### 9.3 Proposed resource additions ([td3.toml](../../re/exe-resources/td3.toml))

- `e3.startup.usage` (pstring_seq, offset 0x74E1, count 4): the first half of the `/?` text, only if the port prints a usage text; otherwise mark `e3.startup[0..3]` as not ported. `e3.startup[4]` is the low-memory note of the configuration screen and `[5..12]` its lines.
- `e3.exit_notice` (u8_table, offset 0x36784, count 262): the crunched exit notice (DS:0264), only if the port shows it (T3). Its bytes equal `e2.exit_notice` (proposed in e2-program-flow §9.3).
- `e3.saves.save[4]` (file 0xE6BA) is the file name `td.sss`, not display text: split the group into `e3.saves.save` (0xE68E, count 4) and `e3.saves.save_prompt` (0xE6C1, count 1), or mark index 4 as a file name.
- The memory warning (file 0x736C, 7 strings) stays unmapped on purpose.

### 9.4 Errors found in E1/E2 specs (not edited here)

- [e2-program-flow](e2-program-flow.md) §8 step 6 says the flight's first file is `td.map`; C-0015 shows `td.pal` first (frame 596), then `m1.td2` (598) and `td.map` (599).
- [FORMATS §13.2](../../re/FORMATS.md) leaves the slot word's meaning open (U-011) and says saving rewrites only those 3 bytes: the word is the kill total (`td3:04A0:5B2A`, and the save is called with the kill total at `td3:0000:362F`), and the save screen writes all ten 3-byte slots back (`td3:04A0:6E07..6E75`; td3.csv lists the routine as identical to E1's `051A:7EEA`).
- [episode-program-flow](episode-program-flow.md) §4.2: as noted in e2-program-flow §9.4, the E1 menu also flushes the keyboard after the campaign; nothing new found for E1.

## 10. Provenance

| Claim | Address / evidence | Method | Confidence |
|---|---|---|---|
| Start-up order, switches, gate, `td.cfg`, configuration screen, script choice, calibration, exit | `0000:3E8A..49EC`; compared with `td2:0000:4365..4EBA` | static | CONFIRMED |
| `g_ResumeStatus` never written | byte scan (0 writers of DS:07B2) | static | CONFIRMED |
| Exit notice bytes | file 0x36784, 262 bytes = `td2.exe` 0x3AF44 | byte compare | CONFIRMED |
| Menu items, bookkeeping, music, episode-complete test at 20 | `0000:2E3F..3A7B` (`369E`) | static | CONFIRMED |
| Loader (flush, colours, music argument); menu fades | `0000:29E5..2DD6`, `0000:0000..01A4`; compared with `td2:0000:2EC0` | static | CONFIRMED |
| Block order, captions, music, dialogue, skip tests (200), branches (§4.3, §4.4) | `0000:0BC5..29D3` | complete token walk | CONFIRMED |
| Skip flag not cleared by the campaign; block-1 pre-test | `0000:0BCF..0BE3`; byte scan (26 writers of DS:1E72, none in segment 0000); `td2:0000:0CCD` clears it | static | CONFIRMED |
| Writers of DS:0670, 066F, 0C49, 0C4A, 11EC..11EF | byte scan of the whole image for direct stores | static | CONFIRMED |
| Music table and call counts | `0C5D:07F3..09A4`; all `Snd_PlayMusic` pushes counted (121) | static | CONFIRMED |
| Flight song and fallback | `0C5D:B853..B8E3` | static | CONFIRMED |
| Flight hand-off and outcome | `0C5D:A9CC..AA71`, `EA1D..EC03`, `7BE6`, `99FD`, `B0BA` | static | CONFIRMED |
| Keyboard flushes | `04A0:0277`, `0000:29F4`, `33FD`, `04A0:5474`, `643F`, `0C5D:B0BA`; none in `1BB3:01ED`, `04A0:4F47`, `4980` | static | CONFIRMED |
| Waits of cards and screens | `04A0:1D6B..449C`, `4663`, `5212`, `5F7E`, `73DF` | static | CONFIRMED |
| Debrief heading uses the mission number | `04A0:6083` | static | CONFIRMED |
| Save format, restore effect | `04A0:55C4..5670`, `5AFE..5B2D`, `6E07..6E75`; shipped `td3.sav`; E2 C-0016 read pattern | static + file + capture (E2) | CONFIRMED |
| Dialogue coverage (111 of 111) | walk vs `dialogu3.dat` and `safedia3.dat` scene ids | static + decode | CONFIRMED |
| Flight-5 loss gives outcome 0 | [e3-mission-logic](../sim/e3-mission-logic.md) §4.6.6, §4.8 | static | LIKELY |
| Verifier timings (§8) | derived from the above; start-up path and restore path run in the oracle | static + C-0036, C-0041 | CONFIRMED (flight 1; restore to block 8 / flight 8); LIKELY (other blocks) |

## 11. Revision history

| Rev | Date | Author (session/agent) | Change |
|---|---|---|---|
| 1 | 2026-11-05 | RE-10c (Archaeologist + Spec author, draft for RE-10); merged and checked by RE-10 (C-0036, C-0041; U-270) | Initial: start-up, menu, all 18 blocks (blocks 6 and 11 in detail), ending A and the episode end, music, flight hand-off, keyboard flushes, save format, Verifier section; OB-050, OB-051; OB-027 and OB-041 extended; U-280..U-282; U-011 and U-206 resolved for E3 |
| 2 | 2026-11-06 | RE-16 (Archaeologist + Spec author) | §4.2 item 4: the menu fades and entry ramp measured as E2's (C-0125) |
