# ADR-0006: Enhanced presentation pipeline (widescreen, upscaling, dynamic lighting)

- **Status:** Proposed (the user asked for upscaling, widescreen and a modern lighting engine as enhancements, 2026-11-05; this records how)
- **Date:** 2026-11-05

## Context

ADR-0004 makes the indexed 320×200 framebuffer the source of truth for presentation, which keeps the Classic profile bit-exact (T0) and testable against the oracle. The user wants three enhancements on top of the faithful game: **upscaling** of the pixel art, a **widescreen** view, and a **modern lighting engine**. All three must leave the Classic profile and the simulation untouched: every L5 parity test and every replay must keep passing unchanged.

## Decision

1. **Two presentation paths.** Classic is unchanged: indexed 320×200 → RGBA → platform scaling (ADR-0004, MDG §9.3.8). Enhanced adds stages *after* composition, configured per feature in a new `[enhanced]` settings section that defaults to off. The Classic profile forces every Enhanced feature off.
2. **Pipeline order** (spec: [spec/enhanced/presentation.md](../spec/enhanced/presentation.md)):
   1. compose the indexed frame at base resolution, *W* × 200, where *W* = 320 or a widescreen width;
   2. indexed → RGBA through the palette;
   3. build the light map at base resolution from a light list and an occluder grid supplied by the flight screen;
   4. upscale the unlit RGBA image by an integer factor with a pixel-art filter;
   5. modulate the upscaled image by the bilinearly sampled light map, then add bloom;
   6. platform scaling to the window, as today.
3. **Widescreen** widens only what the game composes: the flight playfield shows more map columns around the original window, and the 320-wide HUD bar and all 320×200 screens are centred with side panels. Widths keep the original pixel aspect (5:6): 16:10 → 384, 16:9 → 428, 21:9 → 560. The simulation's view state, tick order and RNG are unchanged; only more of the world is drawn. Because the player sees further, the setting is marked "affects gameplay (visibility)" in the UI (MDG §6.2).
4. **Upscaling** runs on the composed RGBA image (CPU, deterministic): Scale2x/3x/4x (EPX family) and xBR (2×/3×/4×), plus "none". The existing scale modes then fit the result to the window.
5. **Lighting** is presentation only and deterministic. It is computed from sim state through the screen mapping and **never feeds back** into the simulation. Inputs: light sources (explosions, shots, muzzle flashes, afterburner flame, engine glow), an occluder grid from the map's tile attributes, emissive palette indices, and an ambient preset. Output: a light map, with soft shadows from solid tiles, plus bloom. It applies to the flight playfield only; the HUD and front-end screens stay unlit.
6. **Crate placement.** The pipeline and its stages live in `td-render` (`enhanced` module); the platform layer gains a path that presents an RGBA frame of any size. The light list is built in the flight screen (`crates/td2192/src/flight.rs`) from `td_sim::Flight`.

## Consequences

- Classic presentation, the L5 parity tests and replays are untouched by construction. Enhanced stages get their own tests: golden hashes on synthetic input, plus invariants such as "the centre 320 columns of a widescreen playfield equal the Classic playfield" and "a uniform image upscales to a uniform image".
- Widescreen needs the flight renderer to take a viewport width. Classic is the case *W* = 320.
- The CPU cost of xBR 4× at 428×200 plus lighting must fit a frame budget (spec §6). Row-parallelism with `rayon` (MIT/Apache-2.0) is allowed if needed; THIRD_PARTY.md must be updated in that case.
- This answers part of decision D5 (which Enhanced features exist); whether they ship in 1.0 stays D5.

## Alternatives considered

- **GPU shaders (SDL3 GPU API)** for upscaling and lighting: faster and higher quality at 4K, but they add a shader toolchain and per-backend testing. Deferred. The CPU pipeline defines the reference behaviour, and a GPU backend can follow behind the same interface.
- **Lighting in indexed colour** (palette tricks, as the original's infra-red mode does): cheap, but it cannot express coloured, smooth or shadowed light. Rejected.
- **Widescreen by stretching:** distorts the art. Rejected; Stretch remains a separate scale mode.
