# Parity: M3 audio (2026-11-02, in progress)

Method: black-box comparison with oracle capture **C-0001**. Its DOSBox-X OPL register log (DRO) of the intro holds `intro1.cmf`, `intro.cmf` and `intro2.cmf`, captured with preset P1 at cycles 20000. Our register writes come from `tdtool cmf-trace`. The comparison extracts note events (melodic key-on with voice, F-number and block; percussion triggers in register 0xBD) and aligns them per song. Scripts are in `re-work/scratch/audio-compare/` (outside the repo).

| # | Behaviour | Tier | Result | Notes |
|---|---|---|---|---|
| 1 | Sequencer clock = 96 ticks/s (header field 0x0C) | T1 | **PASS** | Tick 147 at 1523 ms in the capture; event slots agree within ±1 tick (ms jitter in the capture). Settles U-007 for these songs |
| 2 | F-number and block per note | T1 | **PASS** (after fix) | Classic AdLib table C♯..C (0x16B..0x2AE), `block = (note − 1)/12 − 1`. Our first version used equal temperament, about 9 cents sharp |
| 3 | Register 0xBD depth bits | T1 | **PASS** (after fix) | Depth bits 0xC0 always set; rhythm bit from controller 103; stop writes 0xC0 |
| 4 | Voice allocation | T1 | **FAIL** | Dynamic. First notes match (intro1: the first 4 events; intro2: the first 4), but the original is not sticky per channel. Exact rule pending M3-05 (RE-4) |
| 5 | Instrument writes | T2 | PARTIAL | Written at note-on when the voice's program differs (implemented). The original writes no E0 (wave select) registers; we still do |
| 6 | Velocity → carrier level | T2 | **FAIL** | The original rewrites the carrier level just after key-on (velocity 49: 0x80 and 0x00 both become 0x01). Formula pending (U-034) |
| 7 | Event counts per song (first 40 s) | T1 | close | intro1 102/102, intro 637 vs 632 ours, intro2 153/153 |
| 8 | ADPCM sound effects | T2 | not yet compared | Needs WAV captures of individual effects (C-0002 has WAV of the E1 path) |
| 9 | Music/SFX mix balance | T2 | not yet compared | |

Next: implement the allocation and velocity rules once the M3-05 spec lands, then re-run until rows 4–7 pass. Then compare decoded VOCs with the WAV captures.
