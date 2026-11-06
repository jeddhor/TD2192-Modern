# Episode 2 program flow (`td2.exe`)

| | |
|---|---|
| **Spec ID** | `flow/e2-program-flow` |
| **Revision** | 3 |
| **Status** | Draft |
| **Fidelity tier** | As [episode-program-flow](episode-program-flow.md): T2 for the order of screens, scenes, dialogue, music and flights; T1 for the state that flows between missions (mission number, kills, outcome flags, the retry prompt's `Random` call); T0 for menu colours; T3 for the text-mode configuration screen, the `/?` usage text, the memory gate and the exit notice (the port's own shell replaces them) |
| **Confidence** | LIKELY overall. CONFIRMED (static) for the call order of all 20 mission blocks, every branch and every write of the campaign state: complete token walk of `Main_CampaignLoop` plus a byte scan of the whole image for writes of the flags (§10). No E2 capture exists yet (U-249) |
| **Tasks** | M10-01 |
| **Unknowns** | U-206 (partly resolved here), U-249 (start-up path confirmed by C-0015/C-0016), U-250, U-251 (new); U-005 (consistent) |

This is a **delta spec**. Implement [episode-program-flow](episode-program-flow.md) (E1) and change what this document says. Screen layouts are in [screens](screens.md) (E1) and [episode-deltas §9](../../re/episode-deltas.md#9-scenes-text-and-dialogue-unit-051a--04ec--04a0) (E2 scene cards: anchors, inks, waits). Addresses are `td2:<seg>:<off>` image-relative (ADG §7.8); the `td2:` prefix is dropped below. DGROUP is `36B6`, so `DS:x` means `36B6:x`. Text is named by its resource id in [td2.toml](../../re/exe-resources/td2.toml), never quoted.

## 1. Summary

`td2.exe` is started directly; it reads no `tdstatus.dat` and needs no launcher. It initialises WORX, applies a memory gate, reads `td.cfg`, applies one command-line switch (now including `/?`), shows the configuration screen when needed, asks for the dialogue script at **every** start, calibrates the joystick when one is in use, and runs the main menu. *Start new game* and *Next mission* call `Main_CampaignLoop`, which plays exactly one mission block for `g_MissionNumber` 1..20: cards, music, dialogue, one flight with its retry loop, the debrief and the story after it. E2 has 20 blocks and 20 flights (no flight 21), no hint screens, no stills, no dossier screens other than the death dossier, and three game-over endings (A death; B mission 16, unreachable in practice; C missions 17/18 lost). Blocks 3, 8, 16 and 17 offer a second retry prompt **after the debrief** when the flight's outcome flag is 0. Menu items 6 and 7 show the `aboutus.scr` and `order.scr` pictures in-process with `intro.cmf`. Quitting shows a text-mode exit notice.

## 2. Definitions and units

E2 addresses of the E1 globals (all CONFIRMED by references in the listings; [td2.csv](../../re/symbols/td2.csv)):

| Term | E2 global | Notes |
|---|---|---|
| Mission number | `g_MissionNumber` DS:0C49 (u8) | 0 = no game; 1..20 = next block; 21 only transiently (menu) |
| Kills to date | `g_KillsToDate` DS:0C4A (i16) | |
| Mission kills | `g_MissionKills` DS:11EE (i16) | zeroed at every flight start |
| Skip flag | `g_SkipRequested` DS:1E76 (u8) | E1 `g_SkipStory`; cleared at the start of every scene, title, dialogue, retry, debrief, dossier, about and order routine and of `Main_CampaignLoop`; set to 1 when a wait reads Esc |
| Advance flag | `g_MissionAdvance` DS:0670 (u8) | |
| Game over | `g_GameOver` DS:066F (u8) | |
| Menu cursor | `g_MenuCursor` DS:07B4 (u8, 0..8) | `DS:07B3` previous cursor, `DS:07B5` "cursor changed", `DS:07B6` "quit confirmed" |
| Menu palette | `g_MenuPalette` DS:0840 (u8[768]) | |
| Script choice | `g_ScriptChoice` DS:0C48 (u8) | 1 = `dialogu2.dat`, 2 = `safedia2.dat` |
| Devices | `g_MusicDevice` DS:11EA, `g_SfxDevice` DS:11EB, `g_CfgController` DS:11EC | values as E1 §3.2; `g_NoWorx` DS:11E6 |
| Flight number | `g_FlightNumber` DS:31E1 | the argument of `Sim_FlyMission`; in E2 always equal to the block number |

**Caption notation.** Scene-card captions are resource strings: `m.k` = `e2.mission.MM.cards[k]` (MM = m with two digits), `p.k` = `e2.story.preamble[k]`, `_` = `e2.mission.01.cards[2]`, a single space (blank line). `Tnn` = title card `e2.mission.nn.title`; `T(m.k)` = a title-style card with `m.k`.

## 3. Data

### 3.1 Music index (`Snd_PlayMusic` `0E96:07FB`)

Behaviour as E1 §3.1 / [music-cues](../audio/music-cues.md) §4.1 (nothing unless `g_MusicDevice` = 2; stop, free, load, play; every call restarts). The routine tests N in the order 1, 2, 45, 3..13, 69.

| N | File | Used by | N | File | Used by |
|---:|---|---|---:|---|---|
| 1 | `td01.cmf` | flight song | 9 | `climax.cmf` | **never** |
| 2 | `td02.cmf` | flight song | 10 | `mood2.cmf` | campaign, 23 cues |
| 3 | `hangar.cmf` | campaign, 22 | 11 | `moodmaj.cmf` | campaign, 5 |
| 4 | `satair.cmf` | campaign, 19 | 12 | `death.cmf` | campaign, 4 (3 endings + block 20) |
| 5 | `vulture.cmf` | campaign, 19 | 13 | `mood3.cmf` | campaign, 12 |
| 6 | `mood.cmf` | campaign, 18 | 45 | `td03.cmf` | flight song |
| 7 | `menu.cmf` | menu only | 69 | `intro.cmf` | menu items 6 and 7 |
| 8 | `low.cmf` | before each of the 20 debriefs | other | nothing loaded; replays the freed buffer (as E1; no caller) | |

Index 14 loads nothing in E2. The campaign makes 142 music calls; the counts above add up to it.

**Flight song** (`Sim_FlyMission`, `0E96:B8CB..B959`): flight mod 3 = 1 → 1, = 2 → 45, = 0 → 2. Then, only if `g_MusicBuffer` DS:11F6 is still nil (the load failed, or music is off so nothing was ever loaded), one fallback request: mod 3 = 2 → 2, = 1 → 45, = 0 → 1. With music off both requests return at once.

### 3.2 Start-up switches

Only the **first** command-line parameter is examined, by exact string compare (two spellings each):

| Parameter | Effect | Where |
|---|---|---|
| `/nosound`, `/NOSOUND`, `/pc`, `/PC` | `g_NoWorx` := 1 (WORX never initialised) | `0000:43B7..441B` |
| `/m`, `/M` | skip the memory gate | `0000:447F..44B4` |
| `/pc`, `/PC` | music 1, SFX 3 | `0000:46DF..4716` |
| `/nosound`, `/NOSOUND` | music 1, SFX 1 | `0000:471B..4752` |
| `/sb`, `/SB` | music 2, SFX 2 | `0000:4757..478E` |
| `/a`, `/A` | music 2, SFX 3 | `0000:4793..47CA` |
| `/CONFIG`, `/config` | music := 0 (forces the configuration screen) | `0000:47CF..4808` |
| `/?`, `?` | print the usage text and Halt(0) | `0000:480D..4976` |

A switch that sets the music device (any of `/pc`, `/nosound`, `/sb`, `/a`) therefore **skips the configuration screen** whatever `td.cfg` says, and `td.cfg` is then not written.

### 3.3 Screens used by the program

| Routine | What | Wait | Notes |
|---|---|---|---|
| `UI_ScriptChoice` `04EC:689B` | script warning (`e2.script_choice.warning`) | until `1` or `2` | as E1 [screens](screens.md) §3.2 with one line fewer; no other key does anything |
| `UI_JoystickCalibration` `0E96:A3E1` | calibration (`td.joy`) | until any key or a button | identical to E1 |
| `UI_LoadMainMenu` `0000:2EC0` / `UI_MainMenuLoop` `0000:3304` | main menu | indefinitely | §4.2 |
| `Scene_MissionTitle` `04EC:5A89(text, 3Fh, 0, 0)` | title card, font `td-let.cin` | 5 s or key | as E1 `Flow_TextCard` |
| 25 scene-card routines `04EC:1D4A..6127` | one per background | 10 s, or 15 s for `dieseche`, `greenblo`, `hydedead`, `velvswlf` | layouts: episode-deltas §9.2 |
| `Scene_Aboutus` `04EC:50AD`, `Scene_Order` `04EC:5201` | full-screen `aboutus.scr` / `order.scr`, no text | 30 s or key | menu items 6, 7 |
| `Dlg_PlayScene` `04EC:96A4(n)` | dialogue scene n | each page until a key or button, no timeout; Esc ends the scene and sets the skip flag | [dialogue-player](../narrative/dialogue-player.md), E2 deltas episode-deltas §9.3 |
| `UI_RetryPrompt` `04EC:6E62` | retry question | until a key (keyboard only); calls `Random(10)` | returns false for Esc only |
| `UI_Debrief` `04EC:76AD(kills, kills to date, outcome)` | debrief | 10 s or key | identical to E1 |
| `UI_PlayerDossier` `04EC:712D(total kills)` | death dossier | 25 s or key | E1 layout; `e2.dossier.player` |
| `Scene_BackgroundStory` `04EC:92EE` | story so far, 2 pages | 30 s / 45 s or key | E1 [screens](screens.md) §3.8; `e2.background_story`. It no longer clears the skip flag on entry (no effect: nothing tests the flag afterwards) |
| `UI_MenuHelp` `0000:020B` | `td.hlp`, 2 pages | until key or button (`0000:01A5`) | identical to E1 |

### 3.4 State handed to and from `Sim_FlyMission` (`0E96:A76E`)

| Flag / value | Set where | Meaning |
|---|---|---|
| `g_FlightNumber` DS:31E1 | := argument at entry (`A7A9`) | 1..20 |
| `g_MissionKills` DS:11EE | := 0 at entry (`A799`); incremented by `Sim_ProjectilesAndHits` | kills of this attempt only (OB-026 can count a kill twice) |
| `g_MissionAborted` DS:11F2 | := 0 at entry (`A7A1`); := 1 by `UI_FlightMenu` (`0E96:7FC5`) | the player quit from the Esc menu |
| `g_MissionLost` DS:11F0 | := 0 at entry (`A7CF`); := 1 only by `Msn_E2FlightEvents` in flight 18 (`0E96:9B2E`) and flight 17 (`9BB9`) | the flight failed by objective |
| `g_MissionSucceeded` DS:31C6 | := 0 at entry (`A7CA`); set by mission logic | ends the flight normally |
| `g_PlayerDied` DS:11F1 | at the end (`EA54..EA62`): 1 iff `g_EndTicks` DS:31E0 = 35 | the death sequence ran to its end |
| `g_MissionOutcome` DS:11F3 | at the end: 1, then the per-flight tests of flights 3, 16, 8, 9, 10, 11, 17 (`EA67..EB47`) | [e2-mission-logic](../sim/e2-mission-logic.md) §4.8. Flight 16 always gives 1 (OB-025) |

The flight loop ends after the tick in which any of aborted, end ticks = 35, lost or succeeded holds. Teardown: flight fade-out, INT 09h restored, **text mode 3**, flags as above, `NoSound`, wait for the playing VOC (SFX device 2), music stopped and freed, effects freed. Music stays silent until the sequencer's next cue. The byte scan of §10 found no other writer of DS:11F0..11F3.

## 4. Behaviour

### 4.1 Program start (`Main_Entry` `0000:4365`, body `0000:438D`)

```text
1. unit initialisation chain (System, Crt, 2F63, 2555, 2236, 1E0B, 0E96, 04EC)
2. joystick calibration words: g_JoyCal DS:1E7E.. := (0, FFFFh, 0, FFFFh)
   g_NoWorx := first parameter is /nosound, /NOSOUND, /PC or /pc
3. unless g_NoWorx: WORX_Init (installs INT 08h/63h), WORX_SbInit -> g_SbIrq DS:0662,
   WORX_SetTimerStreaming(1) if an SB was found, WORX_SetFmVolume(10, 10)
4. memory gate: if MemAvail < 2BF20h (180,000 bytes) and the first parameter is not /m or /M:
   print the memory warning (text mode, 7 lines, names the E1 launcher) and Halt(0)   [T3, OB-002]
5. g_MusicDevice := 0; read td.cfg (3 bytes: music, SFX, controller) if it opens;
   detected values: SFX 2 if g_SbIrq <> -1 else 3; music 0 if g_NoWorx else WORX_DetectAdLib+1;
   controller 2 if a joystick answers else 1. If td.cfg was read and any byte differs from the
   detected value: g_MusicDevice := 0. (The test is skipped when g_ResumeStatus DS:07B2 <> 0,
   which never happens in E2: nothing writes it.)
6. apply the switch of §3.2 (music/SFX, /config); /? or ?: print the usage and Halt(0)
7. if g_MusicDevice = 0: text-mode configuration screen (0000:497B..4DE3, as E1:
   keys 8/Up, 2/Down/Enter move, Space toggles, Enter or Space on "start" or Esc leave;
   writes td.cfg). Less than 249F0h (150,000) bytes free forces SFX off with a beep.
8. if g_CfgController = 2: re-detect the joystick; none -> controller 1
9. if dialogu2.dat opens: script choice (04EC:689B); else g_ScriptChoice := 2 silently
10. if g_CfgController = 2: joystick calibration (0E96:A3E1)
11. joystick axes and buttons := 0; g_MissionNumber := 0; main menu (0000:3304)
12. exit: unless g_NoWorx: WORX_SbShutdown, WORX_Shutdown; text mode 3; draw the exit notice
    (§4.1.1); text colour 7; cursor to column 1, row 10; Halt(0)
```

Deltas to E1 §4.1: no `tdstatus.dat` (step 3 of E1 is gone, so script choice and calibration run at every start); memory gate on `MemAvail` with a new limit; `/?`; the missing-dialogue notice is gone; **no BIOS keyboard flush** after the script choice (E1 `file0001:0000:513A..514D` had one), so keys typed after `1`/`2` reach the menu; the exit notice.

#### 4.1.1 Exit notice

The 262 bytes at DS:0264 (`k_ExitScreenText`, file offset 3AF44h) are expanded into text-mode video memory at B800:0000 by the string-helper unit's routine `3505:0000(src, dst, length)` (identical to E1's, which expands the configuration screen). It is the TheDraw "crunched" text format: a running attribute starts at 0; a byte ≥ 20h is written as a character with the current attribute; 00h–0Fh set the foreground nibble; 10h–17h set the background bits 4–6 (blink bit kept); 18h moves to the start of the next 80-column row; 19h n writes n + 1 spaces; 1Ah n c writes n + 1 copies of c; 1Bh toggles the blink bit; 1Ch–1Fh are ignored; the length counts source bytes. The notice advertises `catalog.exe`. Port policy: T3, optional; if shown, take the bytes from the exe (§9.3).

### 4.2 Main menu (`UI_MainMenuLoop` `0000:3304`)

The loop is E1's (E1 §4.2, [screens](screens.md) §3.1) with these differences:

1. **Entry:** `Snd_PlayMusic(7)`, then mission := 0 and cursor := 0 unconditionally, then `UI_LoadMainMenu(1)`, which plays song 7 again (the song restarts at once).
2. **Loader** `UI_LoadMainMenu(playMusic)` `0000:2EC0`: plays song 7 only when its argument is non-zero; loads `td.men` regions and `tdmen.pal` exactly as E1; entries 224..233 := (16, 0, 0) **including 227**; the cursor's entry gets red component 63 **only** (cursor 3 too: (63, 0, 0), not yellow); fade in (`0000:0000`); the cursor's red component goes back to 16 in the buffer. E1's item-3 rules (entry 227 := black; yellow for cursor 3), DAC 255 := white and the `MemAvail` digits are **gone** (OB-041; U-229 does not apply to E2).
3. **Fades** `0000:0000` (in) and `0000:00D2` (out): same result as E1 (`0000:0000`, `0000:0124`): 17 levels, DAC 1..255 := Round(c × s / 16) from `g_MenuPalette`, one retrace-bit wait per level. E2 reads `g_MenuPalette` directly instead of a stack copy; nothing changes it during a fade, so the output is identical. (U-206, resolved for these routines.) At P1 a level takes 2 frames ([screens §2.1](screens.md#21-fades)): 34 frames per fade, as E1 (C-0120: in f272..305, out f435..468).
4. **Highlight loop:** unchanged (cursor ≠ 3: (k, 0, 0), k = 16..63; cursor 3: (k, k, 0), k = 0..63; un-highlight in reverse, from the *previous* cursor `DS:07B3`). One retrace-bit wait per step, which returns at once while the bit is set: **11 steps per frame** at P1, so a 48-step ramp takes 4–5 frames. The ramp of the current cursor runs **once when the loop is entered**, right after the loader's fade-in (`0000:332E..3398`; C-0120 f305..309), and again after every change of cursor (Up, Down, quit box: un-highlight the previous item, then back to `0000:3324` and ramp the new one, `3ECC..3F49`). After an action (new game, restore, next mission, help, story, about, order) the loop goes back to polling without a ramp (`3ED3`), so the DAC shows what the loader left: the cursor's entry still at red 63 from the fade-in. So a port must show, at every entry from the program start: menu fade-in (34 frames), then the start cursor's ramp; on Up/Down: the old item's ramp down, then the new item's ramp up; on an action: fade-out (34 frames).
5. **Key drain (U-441, U-480):** a poll that finds a key reads it (and the scan code after an extended key's 0), then reads every further waiting key from the BIOS buffer and discards it, storing each byte `ReadKey` returns in `g_MenuDrainKey` DS:0660 (`0000:33BF..33D0`). The last discarded byte stays there for the program run and becomes a muzzle offset of class-4 volleys on heading 0Ch ([weapons](../sim/weapons.md) rev 5 §3.3.2 item 2). A port keeps the BIOS-buffer behaviour: one key handled per poll, the rest discarded, the last discarded byte recorded.

Items (selection: Space, Enter, joystick button 1 or 2; Up/Down skip items 1 and 3 while mission = 0; Esc = quit box; confirmation `UpCase(key) = 'Y'`):

| Item | E2 action | Bookkeeping |
|---:|---|---|
| 0 | New game; box 2 only when mission ≠ 0. Mission := 1, kills := 0 (as E1), fade out, `Main_CampaignLoop`, flush BIOS keyboard | advance = 1: mission += 1, kills += mission kills, cursor := 3. Game over: mission := 0. Then `Snd_PlayMusic(7)`, `UI_LoadMainMenu(0)` |
| 1 | Save (only when mission ≠ 0): fade out, `Sav_SaveScreen` `04EC:8322(mission, kills, palette)` with **`td2.stt`/`td2.sav`** (and `td.sss` bars), `UI_LoadMainMenu(0)` | none |
| 2 | Restore; box 2 only when mission ≠ 0; fade out, `Sav_RestoreScreen` `04EC:7B20(palette)` (`td2.stt`/`td2.sav`; sets mission at `04EC:81DA` and kills), `UI_LoadMainMenu(0)` | mission ≠ 0 after it: cursor := 3. Then the buffer entry 227 := (0, 0, 0) if cursor = 3, else entry 226 red := 16 |
| 3 | Next mission: advance := 0, fade out, `Gfx_InitUnchained` (new in E2), `Main_CampaignLoop`, flush BIOS keyboard | advance = 1: mission += 1, kills += mission kills; mission = 21 → cursor := 7, mission := 0 (**episode complete**, `0000:3B80`). Game over: mission := 0, cursor := 0. Then `UI_LoadMainMenu(1)` |
| 4 | Help: fade out, `UI_MenuHelp`, re-fade-in of the menu (as E1) | none |
| 5 | Story so far: fade out, `Scene_BackgroundStory`, reload `tdmen.pal`, entries 224..233 := (16, 0, 0), cursor red := 63, fade in, red := 16 (as E1) | none |
| 6 | **In-process:** fade out, `Snd_PlayMusic(69)`, `Scene_Aboutus` (30 s or key), `UI_LoadMainMenu(1)` | none; no file written, no Halt |
| 7 | **In-process:** as 6 with `Scene_Order` | none |
| 8 | Quit: box 1; `Y` sets cursor 8 and the quit flag; the loop ends with a fade-out; DS:066E := cursor (written, never read) | return to `Main_Entry` step 12 |

The flags of the about/order screens: Esc there sets the skip flag, which nothing tests before `Main_CampaignLoop` clears it.

### 4.3 One mission block (`Main_CampaignLoop` `0000:0CC3`)

```text
skip := 0; game over := 0; advance := 1
the block for g_MissionNumber runs (tests 1..20 in turn at 0CDC, 0E79, ..., 2B8F;
  the end of each block falls into the next test, which fails, down to the return)

pre-flight steps; after a step with a skip test: if skip then go to FLY
FLY:
  Sim_FlyMission(block number)
  if died:            if RetryPrompt() then go to FLY
  [blocks 17, 18]     if lost: if RetryPrompt() then go to FLY
  [blocks 17, 18]     if lost: ending C
  if died:            ending A
  if aborted:         go to RETURN
  Snd_PlayMusic(8); Debrief(mission kills, kills to date, outcome)
  [blocks 3, 8, 16, 17] outcome rule (§4.4)
  post-flight steps; after a step with a skip test: if skip then go to RETURN
RETURN:
  if aborted: advance := 0
```

`RetryPrompt` returns true for every key but Esc and consumes one `Random(10)` value (E1 [screens](screens.md) §3.7). A retry re-flies from `FLY`: the pre-flight story is not repeated, the whole flight handling (including a second debrief) is.

### 4.4 Per-mission sequence

Notation as E1 §4.4 with §2's captions: `Mn` music n (§3.1); `Dn` dialogue scene n; `bg[a/b/c]` scene card (abbreviations below); `B` = M8 + debrief; `R` retry prompt; a step marked `'` has **no** skip test after it (Esc in it does not skip; the next routine clears the flag). Every other step is followed by a test; the last step of each part has none (it does not matter). "std" = died → R (decline → ending A), aborted → return.

| Abbr. | Routine `04EC:` | Abbr. | Routine `04EC:` | Abbr. | Routine `04EC:` |
|---|---|---|---|---|---|
| bar | `5644` sbar | grn | `3F44` greenblo | mar | `23AF` marauder |
| bb | `4507` bb | hdd | `4DBE` hydedead | sew | `33B1` sewer |
| brf | `25EF` briefing | heli | `2851` helihangar | sho | `2BF0` shooting |
| bun | `398E` bunker | hg | `1D4A` hangar | shu | `36B5` shuttle |
| die | `2DC0` dieseche | hg2 | `1F49` hangar2 | sof | `30AF` soffice |
| elv | `3C69` elevator | hyd | `5BA3` hyde | to2 | `5358` tdoffic2 |
| in2 | `2A23` intro2 | ipt / vpt | `4ACE` i-port / `47E0` v-port | vel / vid / vvw / lab | `583E` velsroom / `4232` vidcom / `6127` velvswlf / `2146` tdlab |

| Block | Pre-flight | Flight and outcome rule | Post-flight (after B) |
|---:|---|---|---|
| 1 | M5 mar[p.0/p.1/p.2] D1 T01 M10 lab[1.0/1.1/_] D2 M4 sof[1.3/1.4/1.5] D3 M3 hg[1.6/1.4/1.7] D4 | F1 std | M3 hg[1.6/1.4/1.8] D5 M13 bun[1.9/1.10/_] D6 |
| 2 | T02 M6 vel[2.0/2.1/_] D7 M10 brf[2.2/2.3/_] D8 M3 hg[1.6/1.4/2.4] D130 | F2 std | M3 hg[1.6/1.4/1.8] D131 M11 bar[2.5/2.6/2.3]' D11 |
| 3 | M6 sho[2.7/2.8/_] D12 T03 M10 brf[2.2/1.7/_] D233 | F3 std | M6, then outcome 1: D14; outcome 0: D247' R (accept → F3; **decline → ending A**). Then M13 bun[1.9/1.10/_] D15 |
| 4 | T04 M10 lab[1.0/4.0/_] D16 M4 sof[1.3/1.4/1.5] D17 M3 hg[1.6/1.4/4.1] D18 | F4 std | M3 hg[1.6/1.4/1.7] D19 M13 sew[4.2/4.3/_] D20 M4 sof[1.3/1.4/4.4] D21 M10 bar[2.5/2.6/2.3] D22 |
| 5 | M13 bun[1.9/4.5/_] D23 T05 M10 sew[4.2/4.3/_] D24 | F5 std | M13 sew[4.2/5.0/_] D25 M10 bun[1.9/5.1/_] D26 M6 sew[4.2/5.0/_] D27 |
| 6 | M13 sew[4.2/5.2/5.3] D28 T06 M5 mar[p.0/p.1/p.2] D29 M6 vid[6.0/1.1/_] D30 M10 sew[4.2/1.7/_] D31 | F6 std | M10 bun[1.9/5.0/_] D32 |
| 7 | T07 M13 bun[1.9/1.1/_] D34 vvw[_/_/_]' D35' M10 elv[7.0/_/_] D236 | F7 std | M3 hg[1.6/1.4/1.7] D36 M4 sof[1.3/1.4/2.4] D37 M5 mar[p.0/p.1/p.2] D38 |
| 8 | T08 M10 lab[1.0/1.1/_] D39 M5 mar[p.0/p.1/p.2] D40 M6 brf[2.2/2.1/_] D41 M11 bar[2.5/2.6/2.3] D42 M5 mar[8.0/p.1/p.2] D46 grn[8.1/8.2/8.3] M3 hg[1.6/1.4/_]' D47 | F8 std; after B, outcome 0: R (accept → F8; **decline → mission −= 1, return**) | M11 heli[8.4/8.5/1.7] D48 M6 D49 M10 to2[8.6/8.7/1.1] D50 |
| 9 | T09 M6 to2[8.6/8.7/2.1] D51 M10 sho[2.7/9.0/_] D52 M4 to2[8.6/8.7/4.1] D53 M3 hg2[1.6/8.5/1.7] D54 T(9.1) D55 | F9 std | M3 hg2[1.6/8.5/1.8] D56 M5 hyd[9.2/9.3/1.1] D57 |
| 10 | T10 M6 sho[2.7/5.3/_] D58 M5 hyd[9.2/9.3/1.1] D59 M4 to2[8.6/8.7/4.1] D60 M3 hg2[1.6/8.5/1.7] D61 | F10 std | M10 to2[8.6/8.7/4.1] D62 M13 hyd[9.2/9.3/1.1] D63 |
| 11 | T11 M11 bar[2.5/11.0/11.1] D64 M3 hg2[11.2/8.5/2.4] D65 vid[11.3/11.4/11.5]' D66 | F11 std | M3 hg2[1.6/8.5/1.8] D67 M5 hyd[9.2/9.3/1.1] D68 |
| 12 | T12 M6 bar[2.5/11.0/11.1] D69 M10 brf[12.0/2.1/_] D70 M3 hg2[1.6/8.5/2.4] D71 | F12 std | M4 to2[8.6/8.7/4.1] D72 |
| 13 | T13 M6 to2[8.6/8.7/4.1] D73 M11 bar[2.5/11.0/4.4] D74 M4 to2[8.6/8.7/2.1] D75 M3 hg2[1.6/8.5/2.4] D76 | F13 std | M4 to2[8.6/8.7/1.7] D77 M13 lab[1.0/1.1/_] D78 |
| 14 | T14 M6 bar[2.5/11.0/4.4] D79 M5 mar[p.0/p.1/p.2] D80 M10 bar[2.5/11.0/1.7] D81 M6 to2[8.6/8.7/2.1] D181 M3 hg2[1.6/8.5/2.4] D82 | F14 std | M4 to2[8.6/8.7/14.0] D83 M6 hyd[9.2/9.3/1.1] D84 |
| 15 | T15 M6 lab[1.0/1.1/_] D85 M10 sho[2.7/2.3/_] D86 T(9.1) D87 M13 lab[1.0/1.7/_] D187 M4 to2[8.6/8.7/15.0] D88 M3 hg2[1.6/8.5/2.4] D89 | F15 std | M3 hg2[1.6/8.5/14.0] D90 M4 to2[8.6/8.7/4.4] D91 |
| 16 | T16 M5 mar[p.0/p.1/16.0] D92 M10 lab[16.1/16.2/_] D93 M4 to2[8.6/8.7/16.3] D94 | F16 std; after B, outcome 0: R (accept → F16; **decline → ending B**). Outcome is always 1 in flight 16 (OB-025), so this rule never fires | M3 hg2[1.6/8.5/14.0] D95 M5 hyd[9.2/9.3/1.1] D96 |
| 17 | T17 M6 hyd[9.2/9.3/1.1] D97 M4 to2[8.6/8.7/17.0] D98 M10 vpt[_/_/_]' D99 M4 to2[8.6/8.7/11.4] D100 M13 brf[2.2/1.7/_] D101 | F17: died → R; lost → R; lost → **ending C**; died → A; aborted → return. After B, outcome 0: R (accept → F17; decline → mission −= 1, return) | M5 hyd[9.2/9.3/1.1] D102 M4 to2[8.6/8.7/2.4] D103 M5 mar[p.0/p.1/p.2] D104 |
| 18 | T18 M10 to2[18.0/8.7/4.0] D105 M6 vid[18.1/18.2/_] D106 M10 bar[2.5/11.0/1.7] D107 M3 hg2[1.6/8.5/18.3] D108 | F18: as 17 up to B; **no** outcome rule | M4 to2[18.0/8.7/9.0] D109 M10 vid[18.4/18.5/_] D110 |
| 19 | T19 M5 mar[p.0/p.1/p.2] D112 M4 to2[8.6/8.7/19.0] D113 M5 hyd[9.2/9.3/1.1] D114 M3 hg2[1.6/8.5/11.1] D115 | F19 std | M3 hg2[1.6/8.5/1.8] D116 M5 hyd[9.2/9.3/1.1] D117 bb[19.1/19.2/_] |
| 20 | T20 M4 to2[8.6/8.7/17.0] D119 ipt[_/_/_]' M5 mar[p.0/p.1/p.2] D120 M10 to2[8.6/8.7/2.4] D121 M6 bar[2.5/11.0/4.4] D122 | F20 std | M13 shu[20.0/20.1/14.0] D123 hdd[_/_/_] D124 M5 mar[8.0/p.1/p.2] D125 M12 die[20.2/20.3/20.4] M5 mar[8.0/p.1/p.2] D127 T(20.5) (§4.5) |

Block addresses (start / flight call): 1 `0CE6`/`0DD4`, 2 `0E83`/`0F32`, 3 `0FD7`/`105A`, 4 `1112`/`11C1`, 5 `12DC`/`1356`, 6 `143B`/`1520`, 7 `1599`/`162E`, 8 `1713`/`1879`, 9 `1967`/`1A77`, 10 `1B26`/`1C0B`, 11 `1CBA`/`1D59`, 12 `1E08`/`1EB7`, 13 `1F30`/`2015`, 14 `20C4`/`21DF`, 15 `228E`/`23D4`, 16 `2483`/`2532`, 17 `25FD`/`270F`, 18 `2837`/`291C`, 19 `29E7`/`2ACC`, 20 `2B99`/`2C92`.

Notes:

- Esc after a tested pre-flight step jumps to the flight call itself; music requested before it keeps playing until `Sim_FlyMission` stops it. Esc after a tested post-flight step returns with advance = 1: the mission counts as completed.
- Block 3's outcome rule: there is a skip test after D14 (at `10D2`), none after D247, whose retry prompt clears the flag anyway. The block-8/16/17 prompts follow the debrief directly.
- Dialogue: 124 scene ids are played, each once; scene 33 of `dialogu2.dat` (125 scenes) is never played. Ids above 127 (130, 131, 181, 187, 233, 236, 247) are pushed as sign-extended bytes; the byte is the id.
- `M12` in block 20 plays `death.cmf` **looping** (no loop-off call), under the 15 s `dieseche` card.
- No block stops the music outright (E1 `M0` is gone) and none plays `low` before a flight (no hint screens).

### 4.5 Endings and the episode end

| Ending | Trigger | Steps (`0000:`) | Result |
|---|---|---|---|
| A | died and the retry was declined (any block), or the block-3 outcome retry was declined | `2DDE`: M12; unless `g_NoWorx`: `WORX_SetMusicLoop(0)`; `UI_PlayerDossier(kills to date + mission kills)` (25 s or key; its last steps overwrite the status line with `e2.dossier.player[5]`); unless `g_NoWorx`: loop on | game over := 1, advance := 0, return |
| B | block 16, outcome 0, retry declined (unreachable, OB-025) | `2E1B`: M12; loop off; `in2[20.6/20.7/_]` (`intro2` card, 10 s or key); loop on | as A |
| C | blocks 17/18, lost and the lost retry declined | `2E5E`: M12; loop off; `in2[20.8/_/_]`; loop on | as A |
| Episode end | block 20's post-flight completes (or is skipped with Esc) | after D127: `T(20.5)` (title-style card, 5 s or key) with `vulture` still playing; return with advance = 1 | menu: mission 20 + 1 = 21 → mission := 0, cursor := 7 ("ordering info"), `UI_LoadMainMenu(1)` restarts `menu.cmf` |

With the loop flag off, `death.cmf` plays once and then stays silent until the next cue; switching the loop back on does not restart it. After every ending the menu path applies "game over": New Game path → mission := 0; Next Mission path → mission := 0, cursor := 0; kills are not added (advance = 0). The debrief is never shown on an ending.

### 4.6 Bookkeeping on return to the menu

| Return | advance | Menu result (item 3 path; item 0 path is the same except it never reaches 21 and leaves the cursor at 0 on game over) |
|---|---:|---|
| Completed (also Esc in the post-flight story) | 1 | mission += 1; kills += kills of the **last** attempt; cursor stays 3 |
| Block 20 completed | 1 | mission 21 → 0; cursor := 7 |
| Aborted from the flight menu | 0 | nothing changes; no debrief |
| Ending A/B/C | 0 | mission := 0, cursor := 0 |
| Block 8/17 outcome retry declined | 1 | mission was decremented, so mission += 1 restores it: the same block is offered again; kills += the failed attempt's kills (OB-027, now CONFIRMED for E2) |

## 5. Edge cases and original bugs

| Quirk | Original | Port policy (proposal) |
|---|---|---|
| Item 3 colours (OB-041) | "Next mission" is dim red (16, 0, 0) after every menu load until the cursor has passed over it; with cursor 3 after a mission or restore it is shown red (63, 0, 0) instead of yellow; when the cursor then leaves it, the un-highlight starts at (63, 63, 0), a yellow flash, and fades to black | Preserve (T0, Classic); fix-enhanced may use E1's look |
| Ending B unreachable (OB-042) | Block 16's outcome rule needs outcome 0, which flight 16 never produces (OB-025) | Preserve (dead path; implement it anyway) |
| Block-3 decline shows the death dossier (OB-042) | A pilot who survived but failed the objective gets ending A, whose dossier says dead | Preserve |
| Double prompt in blocks 17/18 (OB-042) | If died and lost are both 1, the player is asked twice; declining both gives ending C, not A. Needs lost to be set in the tick the death sequence reaches 35 (U-251) | Preserve |
| Retry after the debrief (blocks 3, 8, 16, 17) | The flight is replayed and the debrief (and the block-3 dialogue) is shown again; only the last attempt's kills count | Preserve |
| Decline-and-replay kills (OB-027) | Blocks 8/17: the failed attempt's kills are added | Preserve |
| Menu music restart | Song 7 is started twice at menu entry | Preserve (inaudible) |
| No keyboard flush after the script choice | Keys typed ahead reach the main menu | Preserve |
| `/?` and the memory gate halt after `WORX_Init` without `WORX_Shutdown` | INT 08h/63h stay hooked into freed memory, PIT at 500 Hz (U-250) | Not ported (OB-002); Verifier: do not run `td2 /?` in an oracle session |
| 16-bit clock in timed waits (OB-034) | A timed card can end at once | As E1 |
| Text mode 3 at the end of every flight | A blank text screen shows briefly before the next screen | Preserve (T2) or skip |

## 6. Test vectors (static; no E2 capture yet, U-249)

| # | Setup / input | Expected | Evidence |
|---|---|---|---|
| 1 | `Snd_PlayMusic(45)`, `(69)`, `(10)`, `(14)` with music on | opens `td03.cmf`, `intro.cmf`, `mood2.cmf`; 14 opens nothing | `0E96:07FB` |
| 2 | Flight song for flights 1, 2, 3, 20 | `td01`, `td03`, `td02`, `td03` | `0E96:B8CB` |
| 3 | `td2 /sb`, no joystick, keys `2`, Enter, (≥ 0.5 s) Esc | script 2; menu; `vulture.cmf`, `td.lt2`, `marauder.scr` opened; then the flight files (first `td.map`); no dialogue file opened after the menu | §8 |
| 4 | As 3 but no Esc | marauder card 10 s, D1 (waits per page), T01 5 s, `mood2` + `tdlab` card, ... D4, flight 1 | block 1 |
| 5 | Menu right after start-up | DAC 224 = (63,0,0), 225..233 = (16,0,0) (227 included); DAC 255 = `tdmen.pal` entry 255 | `0000:2EC0` |
| 6 | New game, complete block 1 | mission 2, kills = flight-1 kills, cursor 3, DAC 227 = (63,0,0) | `0000:3894..38E1` |
| 7 | Block 8 ends with outcome 0, Esc at the retry | after the menu: mission 8 again, kills += attempt kills | `0000:18C1..18E0` |
| 8 | Block 20 completed | mission 0, cursor 7, `menu.cmf` | `0000:3B80..3B91` |
| 9 | Block 3, outcome 0, Esc at the retry | `death` once, dossier with kills to date + mission kills, menu mission 0, cursor 0 | `0000:10BB..10CF`, `2DDE` |
| 10 | Block 17, lost, Esc at the second prompt | `death` once, `intro2` card with `20.8`, game over | `0000:2726..273F`, `2E5E` |
| 11 | Menu item 6 | fade out, `intro.cmf`, `aboutus.scr` for 30 s or a key, menu reloaded with `menu.cmf`; nothing written | `0000:3ADD..3AF5` |
| 12 | Dialogue ids played | 124 distinct ids = the 125 of `dialogu2.dat` minus 33 | file scan + walk |

## 7. Presentation and audio hooks (program-level music)

| When | Song | Address |
|---|---|---|
| Menu loop entry | 7, then 7 again from the loader | `0000:3310`, `2ED7` |
| Menu reload after items 3, 6, 7 | 7 (loader with argument 1) | `0000:2ED7` |
| Return from a new game | 7 (explicit; loader with argument 0) | `0000:38C4` |
| Items 6, 7 | 69 `intro.cmf` | `0000:3AE9`, `3B04` |
| Flight start / fallback / end | §3.1; music stopped at the end | `0E96:A78D`, `B8CB..B959`, `EB68` |
| Debrief | 8 before each debrief | campaign |
| Endings A, B, C | 12 once (loop off, then on) | `0000:2DE0`, `2E1D`, `2E60` |
| Block 20 end sequence | 12 looping | `0000:2D72` |

Save, restore, help and story-so-far keep the menu song playing.

## 8. Running TD2.EXE directly (for the Verifier)

Run on a **scratch copy** of the game directory: `td2.exe` writes `td.cfg` when the configuration screen is left and `td2.stt`/`td2.sav` when a game is saved. Nothing else is written (no `tdstatus.dat`).

**Files read at start:** `td.cfg` (3 bytes; if it does not open, the configuration screen appears unless a switch sets the music device); `dialogu2.dat` (open/close only, existence test); `td.lt2` and `td-let.cin` (script choice); `td.joy` (calibration, only with the joystick controller); `menu.cmf` (music on), `td.men` (twice per menu load) and `tdmen.pal`. Dialogue scenes later open `dialogu2.dat` (script 1) or `safedia2.dat` (script 2) and `newfaces.td2`.

**Recommended DOSBox setup:** Sound Blaster with OPL (default), `joysticktype=none` (the shipped `td.cfg` = 2,2,2 asks for the joystick; with none detected the controller becomes keyboard and no calibration screen appears), default conventional memory (the gate needs 180,000 bytes free). Command: `td2.exe /sb` (one parameter only: forces music and SFX on the SB and skips the configuration screen; `td.cfg` is read, not written). `/nosound` also skips it but removes WORX, which changes the timer setup ([timing](../core/timing.md)); use it only for silent runs. Without a switch and without a joystick, the configuration screen appears (because the stored controller 2 is not detected): Esc leaves it and writes `td.cfg`.

**Start to the flight of mission 1:**

| Step | Screen | Ends on | Keys |
|---:|---|---|---|
| 1 | Nothing visible (WORX init, `td.cfg`, `dialogu2.dat` test) | — | — |
| 2 | Script choice (mode X, fades in) | `1` or `2` only, no timeout | `2` (edited script) |
| 3 | Joystick calibration: skipped with `joysticktype=none` | any key or button | — |
| 4 | Main menu (`menu.cmf`, fade-in), cursor on item 0 | Space/Enter/button, no timeout | Enter (no confirmation: no game in progress) |
| 5 | Menu fades out; `vulture.cmf`; `marauder` card with the preamble | 10 s or a key | Esc → straight to flight 1 |
| 6 | Flight 1 (`td01.cmf`; first file `td.map`) | — | — |

**Confirmed (C-0015, C-0016):** with `TD2.EXE /sb` on a pristine copy in the oracle rig (preset P1), `2` 140 frames after `td-let.cin` opens, Enter 180 frames after `tdmen.pal` opens and Esc 90 frames after `marauder.scr` opens reach flight 1 at frame 598 (`m1.td2`), the first flight tick at frame 665. Nothing is written to the game directory on this path.

**Any mission directly (C-0016):** set slot 1 of `TD2.SAV` (bytes 0..2) to `NN 00 00` (mission NN, 0 kills) in the scratch copy. At the menu press Down (cursor 0 → 2: items 1 and 3 are skipped with no game), Enter; on the restore screen Enter (cursor on slot 1; Space also selects); back at the menu the cursor is on item 3, Enter. Block NN starts; Esc at its first scene card skips the rest of the pre-flight sequence (block 9: `tdoffic2`; the title card comes first and is not skipped by this Esc). With NN = 9 the flight starts at frame 1375.

Timing rules: the menu reads one key and **discards every other key already in the BIOS buffer**, so leave at least 0.5 s between Enter and Esc; after that, Esc may be pressed at any time (during the fade-out, loading or fade-in) because the scene card does not flush the buffer and its wait ends on a buffered key. Keys typed after `2` are not flushed (E2 change) and reach the menu. Without Esc the block runs: card 10 s → D1 (each page waits for a key, no timeout; Esc ends the scene and skips to the flight) → T01 5 s → `tdlab` card 10 s → D2 → `soffice` card → D3 → `hangar` card → D4 → flight 1. Each of these steps is followed by a skip test.

After flight 1: death → retry prompt (keyboard, no timeout): any key but Esc flies again; Esc → ending A (`death.cmf` once, dossier 25 s or key) → menu with no game. Abort from the flight's Esc menu → menu at once, cursor highlight on item 3, mission still 1. Success → `low.cmf` debrief (10 s or key) → `hangar` card → D5 → `bunker` card → D6 → menu (mission 2, cursor 3). To quit: Esc in the menu, `Y`; the program returns to text mode, draws the exit notice and leaves the DOS cursor on row 10.

Waits without a timeout: script choice, configuration screen, joystick calibration, main menu, confirmation boxes, dialogue pages, retry prompt, help pages, save/restore screens. Timed waits (all also end on a key; joystick buttons count where a joystick is present): title cards 5 s, scene cards 10 s (15 s for `dieseche`, `greenblo`, `hydedead`, `velvswlf`), debrief 10 s, dossier 25 s, about/order 30 s, story pages 30 s and 45 s.

## 9. Open questions and corrections

### 9.1 Open questions

- [ ] U-249: §8's start-up path is confirmed (C-0015, C-0016). Still to confirm: the menu colours of OB-041 (DAC 224..233 after start-up and after a mission), the music cues of §7 (69 for items 6/7) and cursor 7 after block 20.
- [ ] U-250: Do `td2 /?` and the memory-gate halt leave WORX's INT 08h/63h handlers installed (no `WORX_Shutdown` before `Halt`), so that DOS or DOSBox hangs afterwards? The same applies to E1's memory gate and to E3.
- [ ] U-251: Can `g_PlayerDied` and `g_MissionLost` both be 1 after flight 17 or 18 (lost set in the tick in which the end sequence reaches 35)? Decides whether the double retry prompt of OB-042 is reachable ([e2-mission-logic](../sim/e2-mission-logic.md), tick order).
- U-206: menu fades and loader highlight resolved (§4.2); `Scene_BackgroundStory` resolved (only the skip-flag clear was dropped; no effect). The dialogue rule "02 line uses its own entry" is not covered here and stays open.

### 9.2 Corrections to [episode-deltas](../../re/episode-deltas.md) §7–§8 (rev 1)

The §8.2 E2 table is correct for the order of every call in all 20 blocks (verified against the token walk). Corrections and additions:

1. Block 16: the outcome-0 retry and ending B can never happen, because flight 16's outcome is always 1 (OB-025).
2. Block 18: "Flight as 17" holds only up to the debrief; block 18 has **no** outcome rule after B.
3. Ending C also starts with M12 and loop off, like A and B. The §7 addresses `2DDE`, `2E1B`, `2E5E` are the ending labels; the music calls are at `2DE0`, `2E1D`, `2E60`.
4. §8.3 "E2 resets kills on New Game" is not a delta: E1 does the same (`file0001:0000:38FF`).
5. §8.3 menu items 0–5 are not entirely "as E1": the loader's item-3 and DAC-255 rules and the `MemAvail` print are gone (OB-041), the loop plays song 7 on entry, and item 3 calls `Gfx_InitUnchained` first.
6. §8.3 `/?`: the usage text is 8 lines after an empty line: four unmapped strings at file offset 826Ch (`0000:40EC..4184`), then `e2.startup[0..3]` (`0000:41B5..4225`). `e2.startup` therefore starts in the middle of the usage text.
7. §8.3 add: no BIOS keyboard flush after the script choice; the exit notice ends with text colour 7, the cursor at (1, 10) and Halt(0); `/m` and the other switches are mutually exclusive (first parameter only).
8. §8.2 "Decline-and-replay paths leave `g_MissionAdvance` at 1" is CONFIRMED for E2 (the decline path jumps to the common return, which clears advance only when aborted).

### 9.3 Proposed resource additions ([td2.toml](../../re/exe-resources/td2.toml))

- `e2.startup.usage` (pstring_seq, offset 0x826C, count 4): the first half of the `/?` text, only if the port prints a usage text; otherwise mark `e2.startup[0..3]` as not ported (the README excludes command-line switch text).
- `e2.exit_notice` (u8_table, offset 0x3AF44, count 262): the crunched exit notice (DS:0264), only if the port shows it (T3).
- The memory warning (0x80F7, 7 strings) stays unmapped on purpose.

### 9.4 Errors found in E1 specs (not edited here)

- [episode-program-flow](episode-program-flow.md) §4.2 omits the BIOS keyboard flush after `Main_CampaignLoop` returns in items 0 and 3 (`file0001:0000:390A..391D`, `3B97..3BAA`); E2 keeps it.

## 10. Provenance

| Claim | Address / evidence | Method | Confidence |
|---|---|---|---|
| Start-up order, switches, gate, `td.cfg`, config screen, script choice, calibration, exit | `0000:4365..4EBA` | static | CONFIRMED |
| No `tdstatus.dat`; `g_ResumeStatus` only read | `0000:46B8`, `47CF`; no writer of DS:07B2 | static | CONFIRMED |
| Exit-notice format | `3505:0000` (identical to E1); bytes at file 3AF44h | static | LIKELY (format), CONFIRMED (bytes, length) |
| Menu loop items, bookkeeping, music | `0000:3304..3F56` | static | CONFIRMED |
| Loader colours and music argument | `0000:2EC0..329B` vs `file0001:0000:2E88..32D7` | static comparison | CONFIRMED |
| Menu fades identical to E1 | `0000:0000..00D1`, `00D2..01A4` vs `file0001:0000:0000`, `0124` | static comparison | CONFIRMED |
| Scene_BackgroundStory delta | skeleton diff `04EC:92EE` vs `file0001:051A:8EAA` | static | CONFIRMED |
| Block order, captions, music, dialogue, skip tests, branches (§4.3, §4.4) | `0000:0CC3..2EAE` | complete token walk | CONFIRMED |
| Writers of DS:0670, 066F, 0C49, 11F0..11F3 | byte scan of the whole image for `MOV/INC/DEC [addr]` | static | CONFIRMED |
| Music table | `0E96:07FB..09A6` | static | CONFIRMED |
| Flight song and fallback | `0E96:B8CB..B959` | static | CONFIRMED |
| Flight hand-off and outcome | `0E96:A76E..A7DE`, `EA1C..EB9F`, `7FC5`, `9B2E`, `9BB9` | static | CONFIRMED |
| Dialogue coverage (124 of 125, not 33) | walk vs `dialogu2.dat` scene ids | static + decode | CONFIRMED |
| Waits of cards and screens | `04EC:5A89`, `23AF`, `50AD`, `5201`, `712D`, `76AD`; episode-deltas §9.2 | static | LIKELY |
| Verifier sequence (§8) | derived from the above; run in the oracle | static + C-0015, C-0016 | CONFIRMED (start to flight 1; restore to flight 9) |

## 11. Revision history

| Rev | Date | Author (session/agent) | Change |
|---|---|---|---|
| 1 | 2026-11-02 | RE-9d (Archaeologist + Spec author, for RE-9) | Initial: start-up, menu, all 20 blocks, endings, music, flight hand-off, Verifier section; U-206 (menu parts) resolved; OB-041, OB-042; U-249..U-251 |
| 2 | 2026-11-06 | RE-16 (Archaeologist + Spec author) | §4.2: menu fades 34 frames at P1; the highlight ramp also runs at loop entry, 11 steps per frame (C-0120); exact jump structure of the ramp (VE-3 M12-02 row 19) |
| 3 | 2026-11-06 | RE-17 (Archaeologist + Spec author) | §4.2 item 5: the menu poll discards waiting keys into `g_MenuDrainKey` DS:0660, a two-digit muzzle byte (U-441, weapons rev 5) |
