# Music cues (Episode 1)

| | |
|---|---|
| **Spec ID** | `audio/music-cues` |
| **Revision** | 1 |
| **Status** | Draft |
| **Fidelity tier** | T1 for which song plays when (it also sets the game tick rate, [timing](../core/timing.md)); T2 for the sound |
| **Confidence** | Song table, call sites and order: CONFIRMED (static, every call site). Scene labels: LIKELY (from the location strings each scene routine is given). Triggers of the game-over paths: HYPOTHESIS |
| **Tasks** | M1-02 (feeds M3-04, M5-xx) |
| **Unknowns** | U-026 (resolved by this spec), U-080, U-081 |

## 1. Summary

Episode 1 plays music through one routine, `Snd_PlayMusic(N)` (`file0001:0E8B:07ED`). It does nothing unless the music device setting is FM (DS:12C6 = 2). Otherwise it stops the current song, frees it, loads song file N and starts it. Every call restarts from the beginning, **even when N is the song already playing**. Songs loop by default. Only the death theme is played with looping switched off. The main menu plays `menu.cmf`. The story sequences between missions pick songs by scene (hangar, commander's office, the Vultures' base, tense or happy moods). Briefing and debrief screens use `low.cmf`. In flight the song depends on the mission number: `td01`, `td03` or `td02` in rotation. The intro program (`file0000.exe`) plays `intro1`, `intro` and `intro2`.

## 2. Definitions and units

| Term | Meaning |
|---|---|
| Song index N | Argument of `Snd_PlayMusic` (byte) |
| Music device | DS:12C6: 2 = FM (AdLib/SB OPL). Set by `/sb` or `/a`, or by `td.cfg`/`/config`. `/nosound` and `/pc` set 1 (no music) |
| Loop flag | WORX function 1Dh ([worx §2](../../re/worx.md#2-pascal-wrappers-2c3700000040f-and-game-use)), default on |
| Mission number | DS:32B9 during a flight; `Sim_FlyMission`'s argument (1..21) |

## 3. Data

### 3.1 Song index table (`file0001:0E8B:0825..0963`; names are CS strings at `0E8B:0761..07E4`)

| N | File | | N | File |
|---:|---|---|---:|---|
| 1 | `td01.cmf` | | 8 | `low.cmf` |
| 2 | `td02.cmf` | | 9 | `climax.cmf` |
| 3 | `hangar.cmf` | | 10 | `mood2.cmf` |
| 4 | `satair.cmf` | | 11 | `moodmaj.cmf` |
| 5 | `vulture.cmf` | | 12 | `death.cmf` |
| 6 | `mood.cmf` | | 13 | `mood3.cmf` |
| 7 | `menu.cmf` | | 14 | `td03.cmf` |

Note the order of 11–13: `moodmaj` = 11, `death` = 12, `mood3` = 13. Any other N loads nothing and replays the freed buffer of the previous song (an original bug; no call site passes another value). `mood2.cmf` (10) is never requested in E1. `intro.cmf`, `intro1.cmf` and `intro2.cmf` belong to `file0000.exe`.

### 3.2 Scene routines used as labels below (`file0001:051A`, our names)

| Code | Routine | Scene (LIKELY) |
|---|---|---|
| T | `051A:5852` | title card (mission title, "later" captions) |
| H | `051A:1E5A` | TD headquarters, upper hangar |
| C | `051A:4A66` | the commander's office |
| V | `051A:596F` | the Vultures' command |
| O | `051A:4F5A` | operations coordinators' office |
| A | `051A:35B0` | TD briefing auditorium |
| P | `051A:2B26(N)` | pre-flight screen (only before missions 1, 2 and 6) |
| D | `051A:6FF2` | post-mission debrief |
| R | `051A:6A33` | retry prompt after a failed mission |
| d*n* | `051A:92AF(n)` | dialogue sequence *n* |
| s*xxxx* | `051A:xxxx` | another scene routine, not yet named |

## 4. Behaviour

### 4.1 `Snd_PlayMusic(N)`

```text
if music_device ≠ 2: return
WORX stop music                      # writes key-offs, cmf §4.9
free the previous song buffer (if any)
load song file N into a new buffer   # WORX_LoadMusic
WORX play CMF(buffer)                # cmf §4.3; loops while the loop flag is set
```

The music has no fade or crossfade, and it is never resumed: every cue starts its song from the top.

### 4.2 Program-level cues

| When | Song | Provenance |
|---|---|---|
| Main menu screen drawn (start-up, and every return to the menu) | `menu` | `0000:2E9F` (menu screen routine `0000:2E88`), `0000:3957` (menu loop `0000:3381`) |
| Flight start (every mission, also after a retry) | mission number mod 3 = 1 → `td01`, = 2 → `td03`, = 0 → `td02` | `0E8B:C8EA..C92B` |
| Flight end | music stopped (no song until the next cue) | `0E8B:F9A7` |
| Game over (three paths) | `death`, played once (loop off), then the loop flag is restored | `0000:2DA8`, `2DE5`, `2E28` |
| Intro program | `intro1`, then `intro`, then `intro2` (loop off; each started when the previous one ends or is cut) | C-0001 capture |

At flight start `Sim_FlyMission` stops the music and frees the song before the mission loads, then starts the flight song. The mapping by mission number gives: missions 1, 4, 7, 10, 13, 16, 19 → `td01`; 2, 5, 8, 11, 14, 17, 20 → `td03`; 3, 6, 9, 12, 15, 18, 21 → `td02`.

### 4.3 Campaign sequence (`Main_CampaignLoop`, `file0001:0000:0F06`)

Songs in the order they are requested, with the scene that follows each request. Songs between two `→` markers play over every scene listed after them. "fly *k*" is `Sim_FlyMission(k)` with its flight song (§4.2). After every flight except the last: on failure, R (retry: fly again with the flight song restarted); on final failure, a game-over path (§4.2); on success, `low` and D. Then the next row starts. The campaign enters at the mission saved in DS:0D29 (scenes before the saved mission are skipped). When a scene sets DS:1F4E (the player aborted it), the campaign returns to the main menu (`0000:2E69`). After the very first title card it jumps to flight 1 instead. Music requested before the abort has already started.

| Before mission | Songs and scenes |
|---:|---|
| 1 | T → `hangar` H d1 → `low` P → fly 1 |
| 2 | `hangar` H d2 → `satair` C d3 T → `mood` O d4 s62FD d5 → `low` P → fly 2 |
| 3 | `hangar` H d6 → `satair` C d7 → `vulture` T V d8 T → `hangar` H d11 → fly 3 |
| 4 | `hangar` H d13 d12 → `vulture` T V d14 T → `mood` A d15 → `hangar` H d16 → fly 4 |
| 5 | `hangar` H d17 → `vulture` T V d18 T → `mood3` s3A6B s22BE O s6653 d19 → `satair` C d20 T d21 → `hangar` H d22 → fly 5 |
| 6 | `hangar` H d156 or d157 → `vulture` T V d23 T → `hangar` H d24 P → fly 6 |
| 7 | `hangar` H d25 T → `vulture` V d26 → `satair` C d27 → fly 7 |
| 8 | `hangar` H d28 → `vulture` T V d29 → `mood3` s2055 d100 T → `mood` A d30 → `hangar` H d31 → fly 8 |
| 9 | `hangar` H d32 → `satair` C d33 T → `satair` C d34 → `hangar` H d35 → fly 9 |
| 10 | `hangar` H d36 → `vulture` T V d37 T → `climax` s3E8E V d38 → `mood` A d39 → fly 10 |
| 11 | `hangar` H d40 T → `vulture` V d41 → `mood3` s248A d42 → fly 11 |
| 12 | `vulture` V d43 T → `satair` C d44 → `mood3` O d45 → `mood3` s3CC0 d46 → fly 12 |
| 13 | `mood` s3CC0 d47 → `mood` O d48 → `moodmaj` s4D66 d49 → *stop* T → `satair` C d50 → `climax` T s3A6B s4113 V d51 T → `vulture` V d52 → `mood3` s3109 d53 → fly 13 |
| 14 | `mood3` s3109 d54 → `vulture` V d55 T → `satair` C d56 → `mood` s2055 d57 → `low` s22BE d58 s22BE → `mood3` s2055 d59 → `climax` T V d60 → fly 14 |
| 15 | `satair` C d61 → `mood` s380C d108 T → `vulture` V d62 → `satair` C d63 → `mood` A d64 → `mood3` s3CC0 d109 → fly 15 |
| 16 | `mood3` s3CC0 d65 → `moodmaj` s4D66 d66 → `climax` T C d67 s4113 T → `mood3` s2055 d68 → `vulture` V d69 → `death` (loop off) s4113 (loop on) → `satair` C d70 → `hangar` H d71 → fly 16 |
| 17 | `vulture` V d72 d105 T → `satair` C d73 → `hangar` H d74 → fly 17 |
| 18 | `vulture` V d75 → `moodmaj` s4D66 d76 → `vulture` V d77 T → `vulture` V d78 → `hangar` H d80 → `satair` C d81 → fly 18 |
| 19 | `hangar` H d82 → `vulture` T V d83 s4979 d84 → `mood3` s2055 s471B T → `mood` A d85 → `satair` C d86 → `hangar` H d87 → fly 19 |
| 20 | `vulture` V d88 → `hangar` H d89 → fly 21 (no debrief) → `mood3` T s380C d91 → `vulture` T s3371 d90 T → `satair` C d92 → `moodmaj` s4D66 d93 → `satair` C d110 → `mood3` s380C d94 → `hangar` H d95 → fly 20 |
| end | `mood3` s380C d96 → `satair` C d97 → `mood` s380C d98 → `climax` s3A6B d99 s4113 T (closing card) |

- "→ `hangar` … → `hangar`": consecutive requests for the same song restart it.
- Before mission 6 the dialogue is 156 or 157 depending on DS:12CF (a result flag of the previous flight, HYPOTHESIS). Before mission 13 the music is stopped outright (`0000:1E3F`) for one title card.
- Flight 21 is a short extra flight between the story parts before mission 20. It uses the `td02` flight song and has no debrief (DS flags not traced, U-081).

## 5. Edge cases and original bugs

| Quirk | Original | Port policy |
|---|---|---|
| Same-song request restarts the song | Audible restart at many scene changes | Preserve (Classic). Enhanced may keep the song running |
| Invalid N plays freed memory | Unreachable from game code | Ignore |
| Music silent after each flight until the next cue | By design | Preserve |
| `death` never loops; every other song loops | Loop flag set around the death cues only | Preserve |

## 6. Test vectors

| # | Setup | Expected | Evidence |
|---|---|---|---|
| 1 | Start E1 with `/sb`, reach the main menu | `menu.cmf` starts | static `0000:2E9F`; C-0002 (to check) |
| 2 | New game, mission 1 | `hangar` (hangar scene), `low` (pre-flight screen), `td01` in flight, `low` at the debrief, then `hangar` | static |
| 3 | Fly mission 2, then 3 | `td03`, then `td02` | static |
| 4 | `/nosound` | no song requests reach WORX (WORX not even installed) | static `0000:47C2` |
| 5 | Fail mission 1 and decline the retry | `death` once, no repeat | static (path HYPOTHESIS) |

## 7. Presentation and audio hooks

`Snd_PlayMusic` is the only music entry point. A port should expose `play_song(N)` with the semantics of §4.1 and call it at the same points of the flow (M5 flow spec).

## 8. Open questions

- [ ] Q1 (U-080): which condition selects each of the three game-over paths (`0000:2DA6`, `2DE3`, `2E26`)? Mission failure without retry is the likely one for the first.
- [ ] Q2 (U-081): what flight 21 is (a scripted or bonus mission between the story parts before mission 20) and how its outcome is handled.
- [x] Q3 (M1-09): E2 and E3 cue tables (same routine in `td2`/`td3`, different song files). **Resolved (RE-7):** same behaviour; `td03` is index 45 in E2 and 44 in E3, `intro.cmf` is 69 (and 45 in E3); the flight song gains a fallback request; per-mission cues in [episode-deltas §7, §8.2](../../re/episode-deltas.md#7-audio).

## 9. Provenance

| Claim | Address | Method | Confidence |
|---|---|---|---|
| Song table and the music-device check | `0E8B:07ED..0981` | static | CONFIRMED |
| Every call site and its argument | 128 sites in `0000:0F06..2E28`, `0000:2E9F`, `0000:3957`, `0E8B:C8FE..C92B` | static (call-site scan) | CONFIRMED |
| Flight song from the mission number | `0E8B:C8EA..C92B` | static | CONFIRMED |
| Loop off/on around the death cues | `0000:24E7`/`250B`, `2DB6`/`2DD1`, `2DF3`/`2E15`, `2E36`/`2E58` | static | CONFIRMED |
| Scene labels | the location and caption strings passed to each scene routine | static | LIKELY |
| Intro order | C-0001 OPL capture | capture | CONFIRMED |

## 10. Revision history

| Rev | Date | Author (session/agent) | Change |
|---|---|---|---|
| 1 | 2026-11-02 | RE-4 (Archaeologist + Spec author) | Initial: song table, program-level cues, full E1 campaign sequence |
