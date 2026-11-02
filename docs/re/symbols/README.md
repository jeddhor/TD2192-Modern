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

Area prefixes: `BP_` (Borland runtime), `WORX_`, `Main_`, `Flow_`, `Gfx_`, `Txt_`, `Snd_`, `Inp_`, `Sim_`, `AI_`, `Msn_`, `UI_`, `Sav_`, `Io_`.
