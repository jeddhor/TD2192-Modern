# ADR-0003: Bring-your-own-data and runtime extraction of exe-embedded content

- **Status:** Accepted (plan approved by the user, 2026-11-02)
- **Date:** 2026-11-02

## Context

All game content (graphics, music, dialogue, maps, missions) is copyrighted and lives in the original files. A lot of text (mission titles, story cards, objectives, debrief comments, credits, intro narration) and some tables exist **only inside the executables**.

## Decision

1. The port ships no original content. The user supplies an original installation, which is verified against a committed SHA-256 manifest (`docs/re/FILE_INVENTORY.md` → generated `manifest.rs`).
2. Text and tables embedded in the executables are **extracted at runtime** from the user's executables, using committed offset maps (`docs/re/exe-resources/<exe>.toml`) that are only valid for known exe hashes.
3. Numeric gameplay constants may be committed as code constants with provenance (they are facts), and should be cross-checked against runtime extraction in data-gated tests.
4. Tests use hashes of decoded output as goldens, never decoded assets.
5. The original directory is opened read-only.

## Consequences

- The repository stays free of game content.
- The port depends on known executable versions for text. Unknown versions fall back gracefully (D10).
- A small amount of extra work is needed to build the resource maps (M1-11, M2-14).

## Alternatives considered

- **Transcribe text into the repo:** simpler, but it redistributes copyrighted prose. Rejected.
- **Ship converted assets:** not permitted (NG1).
