# Symbol Tables

`<exe>.csv` (for example `file0001.csv`) holds **our names** for addresses in the original executables. These CSVs, not the Ghidra project, are the source of truth for naming: they merge in git, Ghidra databases do not. See ADG §7.3 and §14.4.

Columns:

```csv
address,kind,name,signature,comment,confidence,source_task
```

- `address`: image-relative `SSSS:OOOO` (ADG §7.8).
- `kind`: `func` · `data` · `label` · `struct` · `enum`.
- `name`: `Area_Thing` for functions (`Sim_UpdatePlayerShip`, `Gfx_FadeOut`, `WORX_PlayCMF`, `BP_Move`), `g_` prefix for globals, `k_` prefix for constant tables.
- `confidence`: CONFIRMED · LIKELY · HYPOTHESIS.
- Keep rows **sorted by address** so that concurrent additions merge cleanly.
- Comments are short and written by us. No decompiler output.

`td2.csv` and `td3.csv` were generated from `file0001.csv` by `tools/scripts/match-episodes.py --symbols` (M1-09; method in [episode-deltas.md §2](../episode-deltas.md#2-method-and-match-statistics)) and then corrected by hand. Each transferred row cites its E1 address and match class (identical, operands differ, changed + similarity) in the comment. To carry new E1 names across, re-run the generator into a scratch directory and merge the new rows; do not overwrite the hand corrections (rows whose comment does not start with `= E1`).

Area prefixes: `BP_` (Borland runtime), `WORX_`, `Main_`, `Flow_`, `Gfx_`, `Txt_`, `Snd_`, `Inp_`, `Sim_`, `AI_`, `Msn_`, `UI_`, `Sav_`, `Io_`.
