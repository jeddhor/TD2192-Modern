# Sound-effect policy (Episode 1)

| | |
|---|---|
| **Spec ID** | `audio/sfx-policy` |
| **Revision** | 4 |
| **Status** | Draft |
| **Fidelity tier** | T1 for the `Random` calls of the PC-speaker path (they advance the shared RNG, [bp7 §4.1](../core/bp7-runtime-semantics.md#41-random-number-generator)) and for the Real48 distance; T2 for what is heard |
| **Confidence** | Algorithm, tables, call sites: CONFIRMED (static, full read of `file0001:1E2A:006A..064F`). Which game event each call site stands for: HYPOTHESIS |
| **Tasks** | M1-02 (feeds M3-06, M8-03) |
| **Unknowns** | U-021 (resolved), U-030 (resolved), U-082, U-083, U-410 (resolved) |

## 1. Summary

All in-game sound effects go through one routine, `Snd_PlayEffect(Id, SrcX, SrcY, ViewX, ViewY, Device)` (`file0001:1E2A:006A`). It works out how far the sound source is from the centre of the view. With a **Sound Blaster** (device 2) it plays at most one digital effect at a time. A new effect cuts off the current one only if its priority is at least the current priority, and loudness and left/right balance come from the SB Pro mixer. With the **PC speaker** (device 3) it writes a short frequency sequence that a BIOS-tick handler plays, and some sequences are randomised with `Random`. **Only the PC-speaker path calls `Random`**, so the gameplay RNG stream depends on the effects setting (U-030). With no effects device (1) nothing is heard, but the distance is still computed.

## 2. Definitions and units

| Term | Type | Meaning |
|---|---|---|
| Device | byte, DS:12C7 | 1 = none, 2 = Sound Blaster digital, 3 = PC speaker. `/sb` → 2; `/a`, `/pc` → 3; `/nosound` → 1; otherwise `td.cfg` or `/config` auto-detection (2 if an SB was found). Forced to 1 when less than 150,000 bytes of heap are free (`0000:4CC0..4CE2`) |
| Id | byte | Effect number: 0..10, 100 (64h) or 200 (C8h) |
| SrcX, SrcY | byte | Tile position of the source; SrcX = FFh means "no position" (the player's own sounds) |
| ViewX, ViewY | byte | DS:3A36, DS:3A38: tile position of the view's top-left corner (all callers) |
| Distance `dist` | integer, tiles | §4.1 |
| Priority | signed 16-bit | §4.2 |
| Current priority | byte, DS:1F0A | Priority of the sound now playing; 0 = idle |
| Speaker queue | 100 × u16, DS:1E40 | Frequencies in Hz, one per BIOS tick; 0 ends the sequence |

## 3. Data

### 3.1 Effects

| Id | SB sample (loaded by `Snd_LoadEffects`, `1E2A:0724..0807`) | Base priority `Id div 20` | PC-speaker sequence (§4.5) | `Random` calls (device 3) |
|---:|---|---:|---|---|
| 0 | `afterb.voc` | 0 | 8 tones `Random(5) + 10 + 5i` | 8 × `Random(5)` |
| 1 | none (silent) | 0 | none | 0 |
| 2 | `stiletto.voc` | 0 | 3 tones `Random(10) + 240 + 20i` | 3 × `Random(10)` |
| 3 | `vii.voc` (same as 8) | 0 | as id 8 | 5 × `Random(10)` |
| 4 | `hornet.voc` | 0 | 4 tones `70 − 5i` | 0 |
| 5 | `hornet.voc` (same as 4) | 0 | as id 4 | 0 |
| 6 | `zeus.voc` | 0 | 5 tones `Random(10) + 38 − 5i` | 5 × `Random(10)` |
| 7 | `vulturei.voc` | 0 | 5 tones `170 − 9i + 2·(i mod 2)` | 0 |
| 8 | `vii.voc` | 0 | 5 tones `Random(10) + 180 + 10i`, but only the first 3 play (§5) | 5 × `Random(10)` |
| 9 | `punyexp.voc` | 0 | 1 tone `Random(20) + 20` | 1 × `Random(20)` |
| 10 | `missile.voc` | 0 | 40, then `70 + 3i` for i = 1..10 | 0 |
| 100 | `boomboom.voc` | 5 | 21 tones `Random(10) + 50 − 2i` | 21 × `Random(10)` |
| 200 | `income.voc` | 10 | 21 tones `(i mod 2)·1000 + (i mod 3)·100 + 100` | 0 |

i counts from 0 within a sequence. The VOC files not listed (`home`, `poink`, `pulse`, `ric2`, `ric3`, `scratch`, `whoosh`) are not referenced by `file0001`.

### 3.2 Call sites (all in unit `0E8B`; event meanings CONFIRMED static, U-083)

All 17 calls of `Snd_PlayEffect` in `file0001` are in unit `0E8B` (no other segment calls it). "Ship i" passes the slot's tile (DS:3B2D+i, DS:3B41+i), so the sound is attenuated and panned by distance (§4.1, §4.3); "none" passes FFh, FFh: full volume, centre.

| Site | Routine | Id | Position | Event |
|---|---|---|---|---|
| `0E8B:106B` | `Sim_ProjectilesAndHits` `0E8B:0984` | 9 | ship i | a shot (any owner other than ship i) hits live mission ship i: [weapons](../sim/weapons.md) §4.3 step 4, before the damage |
| `0E8B:11C4` | same | 100 | ship i | that hit brings ship i's armour below 1 (destroyed), before the distance flash |
| `0E8B:122D` | same | 100 | ship i | the hitting shot was a homing missile (kind 6): destroyed outright. **Unreachable** (U-421, [weapons](../sim/weapons.md) rev 5 §4.3): step 3 uses every homing missile that comes within 40 px of a ship before step 4's 20 px test, and step 3 requests no sound in E1 and E3. An E1 homing kill is silent; E2's step 3 requests 100 once (episode-deltas P4) |
| `0E8B:164E` | same | 9 | none | a mission ship's shot hits the player (step 5; never while `g_PlayerDrawLate` is set, so not in flights 12, 15, 21) |
| `0E8B:5719` | `Sim_EndSequenceStep` `0E8B:56ED` | 9 | none | player destruction sequence: small blast at the player whenever `g_EndTicks` (DS:32B8) is a multiple of 3 below 12 |
| `0E8B:578D` | same | 100 | none | player destruction sequence: the big explosion at `g_EndTicks` = 12 (after the distance flash) |
| `0E8B:596A`, `5908`, `59CF` | same | 9 | none | player destruction sequence: further blasts at `g_EndTicks` = 17, 19 and 25 |
| `0E8B:8073` | `UI_HudTickerAndIndicator` `0E8B:7FDF` | 200 | none | a transmission is pending and the F6 lamp has not started (comm pending ≠ 0, indicator phase 0): the "incoming transmission" chime ([render/hud](../render/hud.md) §4.8); not a reward |
| `0E8B:9F19` | `Msn_UpdateEvents` `0E8B:9860` | 9 | lander piece s | flight 20: slot 5's armour fell below 50; one blast per piece s = 1..5, each only if an explosion slot 1..20 is idle ([mission-logic](../sim/mission-logic.md) §5.20) |
| `0E8B:9F9B` | same | 100 | none | flight 20: the counter passed 149, the lander is destroyed (succeeded) |
| `0E8B:B54A` | `Sim_PlayerSpecials` `0E8B:B1EC` | 10 | none | player missile launch (weapons §4.2) |
| `0E8B:B6D1` | same | player class (DS:1A57) | none | player volley, once per shot spawned (restarts within the tick sound as one start); classes 2, 4, 8 give `stiletto`, `hornet`, `vii` |
| `0E8B:B6EA` | same | 2 | none | player volley while `g_PlayerDrawLate` is set (helicopter flights 12, 15, 21): `stiletto.voc` instead of the class-3 sound |
| `0E8B:CDE0` | `Sim_FlyMission` | 0 | none | afterburner burst starts (classes 2 and 5, principal heading, not over; flight-model §4.7) |
| `0E8B:F413` | `Sim_FlyMission` (fire requests, tick-order step 30) | class of ship i (DS:3A79+i) | ship i | mission ship i's volley, once per shot spawned |

There is no sound for a shot hitting a wall, for a ship leaving the map, or for the player's shot expiring.

## 4. Behaviour

### 4.1 Distance

```text
if SrcX = FFh: dist := 0
else:
  dx := ViewX + 5 − SrcX                     # view centre is 5 tiles right, 3 tiles down
  dy := ViewY + 3 − SrcY
  s  := (dy·dy + dx·dx) mod 65536, read as a signed 16-bit Integer
  dist := Round(Sqrt(Real(s)))               # Real48: real48.md §4.5, §4.9, §4.6
```

This runs for every device, including 1. If `s` is negative (true distance between about 181 and 256 tiles), the original stops with RunError 207 in `Sqrt` (§5).

### 4.2 Priority and preemption (device 2)

```text
p := (Id div 20) − (dist div 15)            # signed 16-bit, truncating division
if current_priority − 0 > p:                # DS:1F0B, always 0, is subtracted here
    do not play (go to the device-3 check only)
else if Device = 2:
    set volume and balance (§4.3)
    play the effect's VOC (WORX function 06h; it stops the sound that is playing first)
    current_priority := Id div 20            # distance not included
```

`current_priority` returns to 0 at the start of each mission and, during flight, on every tick at which WORX reports no digital sound playing (`0E8B:E526..E536`). WORX's "playing" status is not the end of the audio: it stays set for a fixed time after each effect starts, given exactly by [voc-and-adpcm §2.3](voc-and-adpcm.md#23-the-playing-status-worx-function-08h-u-410) (U-410). With E1 started from `td1` (r = 250) a `boomboom` (1.76 s of sound) holds priority 5 for 1.882 s and an `income` chime (1.39 s) holds priority 10 for 1.470 s; the reset happens on the first flight tick after that. So:

- **One digital voice** (U-021): a new effect always replaces the playing one when it is allowed to play. There is no mixing and no queue.
- Equal priority preempts. Explosions (100) cannot be cut by ids 0..10 until they end, and income (200) cannot be cut by anything below 200.
- An effect with `dist ≥ 15` tiles gets a negative priority and never plays (even when idle, the idle priority 0 is greater). For id 100 the limit is 90 tiles, and for 200 it is 165 tiles.
- At mission end, with device 2, the game waits until WORX's playing status clears (`0E8B:F990..F99E`), i.e. until the busy time of voc-and-adpcm §2.3 has passed since the last effect started, not merely until the sound ends.

### 4.3 Volume and panning (Sound Blaster)

Computed in signed 16-bit integers with truncating division:

```text
if SrcX = FFh: L := 15; R := 15
else:
  L := 18 if SrcX < ViewX + 10 else 0
  R := 18 if SrcX > ViewX else 0
  L := L − dist div 5 + (15 − R) div 5       # uses R before its update
  R := R − dist div 5 + (15 − L) div 5       # uses the updated L
  if L < 0: L := 0
  if R < 0: R := 0
  if L > 15: R := R + L − 15; L := 15
  if R > 15: L := L + R − 15; R := 15
  if L > 15: L := 15
mixer voice volume := ((12·L div 16) shl 4) or (12·R div 16)     # WORX function 21h, SB Pro register 04h
```

On a card without the SB Pro mixer, function 21h does nothing and every effect plays at the card's fixed level. The port maps L and R (0..11 after scaling) to a per-voice stereo gain.

### 4.4 Device 1 (none)

Only §4.1 runs. No sound, no `Random`.

### 4.5 PC speaker (device 3)

```text
if current_priority > Id div 20: return     # distance plays no part here
current_priority := Id div 20
if Id = 5: Id := 4
if Id = 3: Id := 8
write the sequence of §3.1 into queue[0..], followed by a 0 entry
```

Ids 1 and the other undefined ids write nothing, but they still set the priority. The sequences are built in index order: each entry calls `Random` (if it has one) and then adds the rest of the formula. The `Random` calls are part of the global RNG stream.

`Snd_SpeakerTick` (`1E2A:0000`, installed on INT 1Ch by `Snd_LoadEffects` for device 3) runs at the BIOS tick rate. If `queue[0] = 0`, it sets `current_priority := 0` and calls `NoSound`. Otherwise it calls `Sound(queue[0])` (PIT channel 2, `1234DDh div Hz`) and shifts the queue down by one entry. Then it chains to the previous INT 1Ch handler. A new effect overwrites the queue from entry 0 and so cuts off the old one.

### 4.6 RNG consequence (U-030)

The `Random` calls happen only on the device-3 path, and only when the speaker priority test passes. A run with PC-speaker effects (`/pc`, `/a`, or a `td.cfg` that selects them) therefore consumes extra random numbers, and every later `Random` result in gameplay differs from a run with Sound Blaster or no effects. For T1 replays the port must record the effects device with the replay. With device 3 it must make exactly these calls at exactly these points. The canonical Classic configuration should be device 2 (SB), which makes no `Random` calls here.

## 5. Edge cases and original bugs

| Quirk | Original | Port policy |
|---|---|---|
| 16-bit distance square | Sources 181–256 tiles away crash the original with RunError 207; beyond that the square wraps and the distance comes out wrong | Port: compute exactly as specified for the priority and volume, but treat a negative `s` as "too far: do not play" instead of failing (fix-all; record in original-bugs). Whether game code can reach it is U-082 |
| Id 1 | No SB sample, no speaker sequence | Preserve (silent) |
| Id 8 speaker sequence | The terminator is written at entry 3, so only 3 of the 5 tones play, but all 5 `Random` calls are made | Preserve |
| Speaker priority ignores distance | Far sounds can play on the speaker | Preserve |
| Priority-hysteresis byte DS:1F0B | Always 0 | Ignore |

## 6. Test vectors

| # | Setup / input | Expected | Evidence |
|---|---|---|---|
| 1 | Device 2, Id 9, SrcX = FFh, idle | plays `punyexp.voc`, mixer 04h = BBh (L = R = 15 → 11), current priority 0 | static |
| 2 | Device 2, Id 9, View (10, 10), Src (15, 13) = view centre | dist 0; L = R = 15; mixer `BBh` | static (§4.3 evaluated by hand) |
| 3 | Same view, Src (8, 13): 7 tiles left | dist 7; L 15, R 5; mixer `B3h`; plays (p = 0) | static |
| 4 | Same view, Src (22, 13): 7 tiles right | dist 7; L 5, R 15; mixer `3Bh` | static |
| 5 | Same view, Src (0, 0) | s = 394, dist 20 (Real48 `Round(Sqrt)`); p = −1, so not played (L 15, R 2 would apply) | static |
| 6 | Device 2, Id 9 at distance 14, then at 15 (idle each time) | plays; does not play | static |
| 7 | Device 2, Id 100 playing, then Id 9 with no position | Id 9 is dropped until the explosion ends | static |
| 8 | Device 3, RandSeed = 0 at program start, Id 9 then Id 2 (idle) | queue after Id 9: `20, 0`; after Id 2: `240, 268, 282, 0`; RandSeed afterwards `33DC589Ch` (4 calls) | static + [bp7 §4.1](../core/bp7-runtime-semantics.md#41-random-number-generator) |
| 9 | Device 3, Id 100 | exactly 21 `Random(10)` calls | static |
| 10 | Device 2, E1 started directly (r = 58), flight 1 at 18.52 ticks/s: an enemy `boomboom` (Id 100) | `current_priority` 5 from the tick of the explosion (C-0072 frame 1441) to the first tick after 1.903 s (observed: 0 in the dump of frame 1576, 1.926 s later) | C-0072; voc-and-adpcm §2.3 |


## 7. Presentation and audio hooks

The port's audio layer needs: one "digital effect" voice with stop-and-replace semantics, a per-voice left/right gain (from §4.3), a PC-speaker tone generator driven at the BIOS tick (Enhanced may synthesise it), and the music player ([cmf](cmf.md)).

## 8. Open questions

- [ ] Q1 (U-082): can a call site pass a source 181–256 tiles from the view (RunError 207 in the original)? Check the callers' range tests (M6/M8).
- [x] Q2 (U-083): resolved (§3.2, RE-14): every call site read in context; the hypotheses held except `0E8B:8073` (the incoming-transmission chime, not income) and the end-sequence sites (the player's destruction sequence, not player hits).
- [x] Q3: E2/E3 effect tables (same unit, `td2:1D51`, `td3:1B1E`). **Resolved (RE-7, M1-09):** the unit is identical (DGROUP offsets only), with the same files, ids, priorities and PC-speaker sequences ([episode-deltas §3.1, §7](../../re/episode-deltas.md#7-audio)).

## 9. Provenance

| Claim | Address | Method | Confidence |
|---|---|---|---|
| Distance, priority, panning, VOC dispatch | `file0001:1E2A:006A..0340` | static | CONFIRMED |
| Speaker sequences and `Random` calls | `1E2A:0341..064F` | static | CONFIRMED |
| Speaker tick handler | `1E2A:0000..0069` | static | CONFIRMED |
| VOC load list | `1E2A:0724..0807` (names at `1E2A:0655..06BA`) | static | CONFIRMED |
| Priority reset in flight; wait at mission end | `0E8B:B88E`, `0E8B:E526..E536`, `0E8B:F990..F99E` | static | CONFIRMED |
| Device setting from switches/config | `0000:4AA9..4C75`; WORX skipped for `/nosound`, `/pc`: `0000:4585..45DC` | static | CONFIRMED |
| Call-site events | §3.2: `0E8B:0F12..1250`, `15D4..1653`, `56ED..59D4`, `7FDF..8073`, `9EC2..9F9B`, `B538..B54A`, `B6B6..B6EA`, `CDA0..CDE0`, `F3C4..F413` | static (each site read in context) | CONFIRMED |

## 10. Revision history

| Rev | Date | Author (session/agent) | Change |
|---|---|---|---|
| 1 | 2026-11-02 | RE-4 (Archaeologist + Spec author) | Initial |
| 2 | 2026-11-06 | RE-14 (Archaeologist + Spec author) | §3.2 event of every call site confirmed or corrected (U-083): 200 is the incoming-transmission chime; the `56ED` sites are the player destruction sequence; the helicopter volley uses id 2; kind-6 kills request 100 twice |
| 3 | 2026-11-06 | RE-16 (Archaeologist + Spec author) | §4.2: the priority reset and the mission-end wait follow WORX's playing status, whose duration is now exact (voc-and-adpcm §2.3, U-410); vector 10 (C-0072) |
| 4 | 2026-11-06 | RE-17 (Archaeologist + Spec author) | §3.2 `0E8B:122D`: unreachable, so a homing kill requests no sound in E1/E3 and 100 once in E2 (U-421); rev 2's "kind-6 kills request 100 twice" is withdrawn |
