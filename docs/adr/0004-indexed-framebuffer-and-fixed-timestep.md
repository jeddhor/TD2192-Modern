# ADR-0004: 320×200 indexed framebuffer and deterministic fixed-timestep simulation

- **Status:** Accepted (plan approved by the user, 2026-11-02); tick rate pending U-001
- **Date:** 2026-11-02

## Context

The original renders VGA mode 13h (320×200, 256 colours, 6-bit DAC) and its game logic is integer-based on a 286-class machine. Fidelity (T0 presentation, T1 behaviour) and testability (hashable frames, replayable simulation) are top priorities.

## Decision

- All game rendering targets an **indexed 320×200 framebuffer plus a 6-bit palette**. Conversion to RGBA, scaling and aspect correction (default 4:3) happen only in the platform layer.
- The simulation runs at a **fixed tick rate equal to the original's** (to be established by M1-06). The render loop is decoupled through an accumulator. In the Classic profile, sim state is never interpolated.
- The simulation is deterministic: integer and fixed-point only, a bit-exact Borland Pascal RNG, no wall clock, slot-ordered iteration, and a per-tick `state_hash` for replay tests.

## Consequences

- Framebuffer hashes give exact render regression tests.
- Input recordings reproduce whole missions, which gives strong regression tests.
- Higher-resolution "Enhanced" visuals must be layered on top (shaders, extended viewport) rather than replacing the renderer.

## Alternatives considered

- **RGBA renderer at native resolution:** loses palette effects and T0 testability. Rejected for Classic.
- **Variable timestep with float physics:** breaks T1 parity and replays. Rejected.
