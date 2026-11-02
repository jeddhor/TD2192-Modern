# <Subsystem / topic>

| | |
|---|---|
| **Spec ID** | `<area>/<topic>` |
| **Revision** | 1 |
| **Status** | Draft · Reviewed · Implemented |
| **Fidelity tier** | T0 · T1 · T2 · T3 (with justification if below the MDG default) |
| **Confidence** | CONFIRMED · LIKELY · PARTIAL · HYPOTHESIS |
| **Tasks** | Mx-yy |
| **Unknowns** | U-0xx |

## 1. Summary

What this subsystem does in the original game, in 3–6 sentences.

## 2. Definitions and units

| Term / quantity | Type in original | Unit | Range | Notes |
|---|---|---|---|---|
| | | | | |

## 3. Data

Record layouts, tables and constants.

| Field | Offset | Size | Type | Meaning |
|---|---:|---:|---|---|
| | | | | |

| Constant | Value | Unit | Provenance |
|---|---|---|---|
| | | | |

## 4. Behaviour

State machines, ordered steps per tick, formulas. Preserve the order of operations and integer truncation where T1 depends on it, and say so explicitly. Pseudocode must be written fresh (ADG §5.4).

```text
each tick:
  1. ...
  2. ...
```

## 5. Edge cases and original bugs

List each quirk, how the original behaves, and the port policy (preserve, fix-all, or fix-enhanced; see MDG §6.3). Mirror the bugs in `original-bugs.md`.

## 6. Test vectors

Concrete values observed from the original (capture IDs or RAM dumps).

| # | Setup / input | Expected output | Evidence |
|---|---|---|---|
| 1 | | | C-xxxx |

## 7. Presentation and audio hooks

Events this subsystem emits (sounds, music cues, messages, palette changes) and when.

## 8. Open questions

- [ ] Q1 (U-0xx if cross-cutting): ...

## 9. Provenance

| Claim | Address / capture | Method (static / dynamic / capture / decode) | Confidence |
|---|---|---|---|
| | `file0001:XXXX:XXXX` | | |

## 10. Revision history

| Rev | Date | Author (session/agent) | Change |
|---|---|---|---|
| 1 | YYYY-MM-DD | | Initial |
