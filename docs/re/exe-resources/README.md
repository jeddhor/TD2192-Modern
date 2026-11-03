# Executable Resource Maps

Per-executable TOML files that tell `td-assets` where to find text and tables **inside the user's original executables**, so that the port never needs to contain game text (ADR-0003, MDG §9.1.4).

Example shape (offsets illustrative; real values come from M1-11):

```toml
exe = "file0001.exe"
sha256 = "<from FILE_INVENTORY.md>"

[[resource]]
id = "e1.mission_title.01"
kind = "pstring"          # pstring | pstring_seq | fixed_width_text | u8_table | i16_table | u16_table
offset = 0x4F39           # file offset
note = "title card before E1M01"

[[resource]]
id = "e1.objective_msgs"
kind = "pstring_seq"
offset = 0x18D91
count = 20
```

Each map must also be listed in the `MAPS` table of `crates/td-assets/src/exe_res.rs` (it is compiled in with `include_str!`).

Rules: offsets are **file offsets** (of the ShortString length byte for `pstring` kinds); every entry needs a stable `id` (the code refers to resources only by `id`); `tdtool exe-res --verify` must pass for every map. Never put the text itself in these files.

## Maps and id conventions (M1-11, 2026-11-02)

| Map | Exe | Resources | Notes |
|---|---|---:|---|
| `file0001.toml` | Episode 1 | 83 | Compiled in |
| `file0000.toml` | Intro | 4 | Narration cards and credits. **Register in `MAPS`** |
| `td2.toml` | Episode 2 | 71 | **Register in `MAPS`** |
| `td3.toml` | Episode 3 | 62 | **Register in `MAPS`** |

- **One namespace.** `ExeResources` keeps every map's values in one table, so ids must be unique across all maps. Episode 1 owns the unprefixed shared names (`debrief.tiers`, `retry.taunts`, `script_choice.warning`, `ship_library`, `flight.prompts` ...); every Episode 2/3 id starts with `e2.`/`e3.`, and intro ids with `intro.`.
- `eN.mission.NN.title`: the title card of mission NN (from its Roman numeral). `eN.mission.NN.cards`: the strings that follow it up to the next title (locations, relative times, captions), in file order. Which of them are shown when is defined by the episode flow spec (M1-04), not by the map. `eN.ending`: lines after the last mission. `eN.story.preamble`: strings before the first title.
- Other groups are one `pstring_seq` per run of consecutive ShortStrings, named after the routine that uses them; `.01`, `.02` ... number several runs of one routine in file order. Notable groups: `e1.objectives` (padded ticker text per mission, `0E8B:60D8`), `e1.flight_messages.*` (mission event tickers), `e1.target_info` (30-character target lines), `e1.comms.NN` (the nine F6 transmissions, `2A15:*`), `debrief.labels`/`debrief.cheater`/`debrief.tiers`/`debrief.failure_note`/`debrief.note_label`, `e1.hints`, `e1.dossier.enemy`/`e1.dossier.player`, `e1.assignment_orders`, `e1.background_story`, `script_choice.missing_dialogue`, `jurisdiction.warnings`, `comm.prompts`, `flight.map_prompts`. Tables: `hud.radar_tile_colours` (`u8_table`, the radar colour of each map tile, DS:0264; RE-6).
- Not mapped on purpose: file names, command-line switches, the memory warning, debug strings, and the `label.exe` strings (never ported).
- Verification: `tdtool exe-res` passes for `file0001.toml` (83 resources, 2026-11-02 RE-6). The three new maps pass an equivalent offline check (SHA-256, every length byte valid, every character printable); run `tdtool exe-res` again once they are registered.
