# Parity: M3 audio (2026-11-02, in progress)

## Music: OPL register stream (L5, automated)

Method: the test `crates/td-audio/tests/oracle_parity.rs` (needs `TD2192_DATA` and `TD2192_ORACLE`) compares our OPL register writes with the DOSBox-X OPL logs (DRO) of oracle runs **C-0001** and **C-0002**, write for write. Both were captured with preset P1 at cycles 20000. A run's DRO (`capture/file0000_000.dro`) holds every OPL write from the first key-on to the end of the run, across programs:

- C-0001 (full intro, no input): `intro1.cmf` cut after 1937 ticks, `intro.cmf` to its end of track, `intro2.cmf` started 2 ticks later and stopped after 2618 ticks, all with looping off; then `td1` starts its driver and stops.
- C-0002 (power-on to the first flight): `intro1` cut after 394 ticks, `td1` as above, then `file0001` starts its own driver and plays `menu` (863 ticks), `hangar` (1866), `low` (840) and `td01` (3197) with looping on, each started by `Snd_PlayMusic` (stop, then start).

Song starts are the PIT reprogrammings to divisor 12428 in each run's `pit.tsv`; in the DRO each song change is a stop block followed by a song-start block. The cut points come from the capture: the written values allow a range of tick counts (between the last event that shows and the next one), and the timing check pins the count to ±1 tick. DOSBox records a DRO from the first key-on: it first lists the non-zero registers of its register map, then logs only writes that change a register. The test plays each schedule through the traced `CmfPlayer` and applies the same filter.

Player: `crates/td-audio/src/cmf_player.rs`, implemented from [cmf.md rev 2](../spec/audio/cmf.md).

| # | Behaviour | Tier | Result | Notes |
|---|---|---|---|---|
| 1 | Register writes, C-0001 (cmf §6.2 C1) | T1 | **PASS (exact)** | The whole DRO, 4,455 writes: the 85-register dump at the capture start, the 4,364 writes from the first key-on to the intro's final stop, and `td1`'s 6 (`A6..A8`, `B6..B8` = 0: a WORX stop with rhythm mode off). Same registers, values and order |
| 2 | Register writes, C-0002 | T1 | **PASS (exact)** | The whole DRO, 3,438 writes: 5 song starts, 3 driver start-ups (intro, `td1`, game), 7 stops |
| 3 | Write timing (cmf §6.2 C2) | T1 | **PASS** | After aligning each song's start, every write lands within −0.9..+1.4 ticks (−9.7..+14.4 ms) of our tick schedule (1193180 / 12428 Hz), stops included. The test allows ±1.5 ticks. The spread is the captures' whole-ms stamps plus the emulated CPU time of each write burst |
| 4 | Clock = header word 0x0C, first event at tick d0 + 1 (cmf §4.1, §4.4) | T1 | **PASS** | Rows 1–3; L0 test `player_ticks_at_the_pit_rate` |
| 5 | F-number table and −12 note offset (cmf §3.3, §4.5) | T1 | **PASS** | Rows 1–2; L0 vectors V1, V4, V7 |
| 6 | Voice allocation (own silent voice, never-used voice, any silent voice, else drop) | T1 | **PASS** | Rows 1–2 (was FAIL with first-use allocation); L0 vector V5 and the drop and take-over tests |
| 7 | Instrument writes only when the voice's program differs, E0 included | T1 | **PASS** | Rows 1–2. Every instrument of these songs has wave 0, so their E0 writes never change a register and the captures show none. That explains the earlier "no E0 writes" observation |
| 8 | Velocity ignored, carrier level from channel volume (U-034) | T1 | **PASS** | Rows 1–2 (was FAIL); L0 vectors V1–V3 |
| 9 | Rhythm mode: fixed drum kit, note-on and note-off toggle the BD bit | T1 | **PASS** | Rows 1–2; L0 vector V6 |
| 10 | Song start block (`00=00`, `BD=C0`, bank entry 0 of the previous song, or the built-in instrument after a driver start-up, to voices 0..8); stop; end of track without loop | T1 | **PASS** | Rows 1–2: 8 song starts, 5 driver start-ups, 11 stops |
| 11 | Cross-song state (bank leftovers, channel volumes, GM-drum flag) | T1 | **PASS** for what the songs exercise | Row 2. L0 tests `state_carries_over_between_songs`, `gm_drum_flag_suppresses_key_on_for_channels_9_and_up` |
| 12 | Loop restart in the same tick without key-offs | T1 | not captured | No captured song reaches its end with looping on. L0 vector V8 (from the spec) passes |
| 13 | Event counts per song (first 40 s), black-box | T1 | superseded | By rows 1–2 |

Not covered by a capture: loop restarts (row 12), controllers 07h and 68h, and the GM-drum channel (no shipped song uses the last three).

Port note: the original runs a new WORX driver in each program, so the game's first song start writes the built-in instrument rather than the intro's. A port that plays the intro and the game in one process gets the same writes by sending `AudioCommand::RestartMusicDriver` at the hand-over.

## Sound effects and mix

| # | Behaviour | Tier | Result | Notes |
|---|---|---|---|---|
| 14 | ADPCM sound effects | T2 | not yet compared | Needs WAV captures of individual effects (C-0002 has WAV of the E1 path) |
| 15 | Classic SFX policy: priority `Id div 20 − dist div 15`, pre-emption at ≥, idle priority 0, 15-tile cut-off, OB-009 as "too far" | T1 | L0 only | `crates/td-audio/src/sfx.rs` tests use sfx-policy §6 vectors 1–7. No capture yet |
| 16 | SB Pro left/right levels (sfx-policy §4.3) | T1 (levels) / T2 (gain) | L0 only | Mixer bytes of vectors 1–5 match. The SB Pro attenuation curve is not modelled: level n → linear gain n/11 |
| 17 | Music/SFX mix balance | T2 | not yet compared | WORX sets the FM volume to (10, 10) at start-up (`docs/re/worx.md`); not modelled |

Next: a capture where a looping song reaches its end (for example the menu left alone for 51 s) for row 12; VOC and balance comparisons against WAV captures.
