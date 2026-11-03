# Original Bugs and Quirks

The catalogue of bugs and quirks in the original game and the port's policy for each (MDG §6.3).

Policies: **preserve** (all profiles) · **fix-all** (crash, hang, save corruption, progression blocker) · **fix-enhanced** (preserved in Classic, fixed in Enhanced) · **not-ported** (hostile or obsolete behaviour).

| ID | Description | Where (spec / address) | Effect on play | Policy | Decided by / date |
|---|---|---|---|---|---|
| OB-001 | Apparent host-modifying path: `c:\dos\label.exe` + `PLAY TD2192` strings in `file0001.exe` near the in-flight UI (possibly tied to cheat detection). | `docs/re/EXECUTABLES.md` §5 | Would modify the user's disk label | **not-ported** (MDG NG4) | Design guide, 2026-11-02 |
| OB-002 | Memory checks, sound-card configuration, `nuffmem.exe`. | `td1.exe`, `file0001.exe` | Obsolete | **not-ported** (MDG NG3) | Design guide, 2026-11-02 |
| OB-003 | Sound-effect distance uses a 16-bit square: a source about 181–256 tiles from the view gives a negative argument to `Sqrt` and the original stops with RunError 207 (reachability unknown, U-082). | [sfx-policy §4.1](audio/sfx-policy.md#41-distance), `file0001:1E2A:006A` | Crash | **fix-all** (proposed: treat as "too far, do not play") | proposed RE-4, 2026-11-02 |
| OB-004 | Real48 subtraction is not correctly rounded (truncating alignment; up to 1 ulp high). | [real48 §4.1](core/real48.md#41-core-add-and-subtract-3a9a1006-subtract-3a9a1002) | Tiny numeric differences in gameplay maths | **preserve** (T1) | proposed RE-4, 2026-11-02 |
| OB-005 | PC-speaker effects call `Random`, so the gameplay RNG stream depends on the effects device. | [sfx-policy §4.6](audio/sfx-policy.md#46-rng-consequence-u-030) | Same inputs give different games with `/pc` or `/a` | **preserve** (record the device in replays; Classic default = SB) | proposed RE-4, 2026-11-02 |
| OB-006 | Every music cue restarts its song, even when that song is already playing. | [music-cues §4.1](audio/music-cues.md#41-snd_playmusicn) | Audible restarts between scenes | **fix-enhanced** (proposed) | proposed RE-4, 2026-11-02 |
