# Behaviour Specifications

Specs describe **how the original game behaves**, in our own words, precisely enough that an Implementer who never sees the original binary can reproduce it. They are the bridge between reverse engineering and implementation in the clean-room process (MDG §5, ADG §5 and §8).

## Layout

| Folder | Covers | MDG section |
|---|---|---|
| `assets/` | Locator, exe-resource extraction | §9.1 |
| `core/` | Timing, BP7 runtime semantics (Random, Real48), shared primitives | §8.4, §8.6 |
| `flow/` | Program flow of launcher and episodes | §8.5 |
| `render/` | Palette, screens, tiles, sprites, text, HUD, effects, presentation | §9.3 |
| `audio/` | VOC, ADPCM, CMF, sequencer, mixer, SFX policy, music cues | §9.4 |
| `input/` | Actions, bindings, original key semantics | §9.5 |
| `sim/` | World, units, entities, ship classes, flight, collision, weapons, damage, AI, mission runtime, guidance, comms, scoring, camera, RNG | §9.6 |
| `narrative/` | Story cards, dialogue player, portraits, dossiers, stills, intro, episode scripts | §9.7 |
| `frontend/` | Menus, settings, saves | §9.8, §9.9 |
| `missions/` | One file per mission: `e1m01.md` ... `e3m21.md` | §9.6.8 |
| `original-bugs.md` | Catalogue of original bugs and the port's policy for each | §6.3 |

## Rules

1. Start from [_TEMPLATE.md](_TEMPLATE.md). Every section is required, even when its content is "none".
2. **No decompiler output, no transliterated code, no bulk game text** (ADG §5.4).
3. Every constant has provenance (address in ADG §7.8 notation, or a capture ID).
4. Every non-trivial formula has test vectors taken from the original.
5. State the fidelity tier (T0–T3) and the confidence.
6. Bump `Revision` on every behavioural change. Code cites `path + revision`.
