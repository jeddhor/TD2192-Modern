# Original Bugs and Quirks

The catalogue of bugs and quirks in the original game and the port's policy for each (MDG §6.3).

Policies: **preserve** (all profiles) · **fix-all** (crash, hang, save corruption, progression blocker) · **fix-enhanced** (preserved in Classic, fixed in Enhanced) · **not-ported** (hostile or obsolete behaviour).

| ID | Description | Where (spec / address) | Effect on play | Policy | Decided by / date |
|---|---|---|---|---|---|
| OB-001 | Apparent host-modifying path: `c:\dos\label.exe` + `PLAY TD2192` strings in `file0001.exe` near the in-flight UI (possibly tied to cheat detection). | `docs/re/EXECUTABLES.md` §5 | Would modify the user's disk label | **not-ported** (MDG NG4) | Design guide, 2026-11-02 |
| OB-002 | Memory checks, sound-card configuration, `nuffmem.exe`. | `td1.exe`, `file0001.exe` | Obsolete | **not-ported** (MDG NG3) | Design guide, 2026-11-02 |
