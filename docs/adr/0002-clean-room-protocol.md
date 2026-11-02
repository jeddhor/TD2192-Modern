# ADR-0002: Two-hat clean-room protocol

- **Status:** Accepted (plan approved by the user, 2026-11-02)
- **Date:** 2026-11-02

## Context

No original source code is available. Behaviour must be recovered from the binaries with Ghidra and DOSBox. We want the port to be our own expression of the game's behaviour, not a translation of its machine code, both for legal hygiene and for code quality.

## Decision

- Separate roles ("hats"): Archaeologist (RE), Spec author, Implementer, Verifier (MDG §5.1, ADG §5).
- Knowledge crosses from RE to implementation **only through written specs** in `docs/spec/`, in our own words, with provenance and test vectors.
- Implementers do not open decompiler output, disassembly or Ghidra. They file questions instead.
- One hat per task. RE and IM on the same subsystem are never done in the same session (context).
- Decompiler output and Ghidra databases stay outside the repository (`re-work/`).
- PRs carry a clean-room attestation.

## Consequences

- Slower than direct translation, but it produces documented, testable behaviour and a codebase that reads as modern Rust.
- Specs become a lasting asset (they are also the documentation).
- Discipline must be enforced through task briefs and review.

## Alternatives considered

- **Direct transliteration of decompiled code:** faster at first, legally weaker, and it produces unidiomatic code that is hard to maintain. Rejected.
- **Strict two-team clean room with separate people:** strongest, but impractical here. The session-separated hat model is the pragmatic equivalent.
