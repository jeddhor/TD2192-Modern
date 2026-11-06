# Enhanced presentation: widescreen, upscaling and dynamic lighting

| | |
|---|---|
| **Spec ID** | `enhanced/presentation` |
| **Revision** | 4 |
| **Status** | Draft (design, not reverse engineering) |
| **Fidelity tier** | T3, intentional deviation, Enhanced profile only. The Classic profile is unaffected |
| **Decision** | [ADR-0006](../../adr/0006-enhanced-presentation-pipeline.md); requested by the user, 2026-11-05 |
| **Tasks** | M12-08 pipeline and settings, M12-09 widescreen, M12-10 upscaling, M12-11 lighting, M12-12 bloom and polish |

## 1. Summary

Three opt-in enhancements are layered after the game has composed its indexed frame:
- **widescreen** shows more of the city around the original playfield;
- **upscaling** refines the pixel art with pixel-art filters;
- **dynamic lighting** adds coloured light from explosions, shots and engines, soft shadows from buildings, an optional night look, and bloom.

None of them changes the simulation, the tick order, RNG use or any Classic output.

## 2. Settings

A new section in the settings file (`td_flow::Settings`), governed by the top-level profile key. Every key defaults to off, and the Classic profile forces every key off.

```toml
profile = "classic"       # classic | enhanced; default classic (MDG §6.2)

[enhanced]
widescreen = "off"        # off | "16:10" | "16:9" | "21:9" | "auto" (from the window's aspect)
upscaler   = "none"       # none | scale2x | scale3x | scale4x | xbr2 | xbr3 | xbr4
lighting   = "off"        # off | subtle | full
bloom      = false
```

- `profile = "enhanced"` is needed for any `[enhanced]` key to take effect. The profile governs only `[enhanced]` for now; `audio.sfx_mode` is separate.
- `auto` picks the widest preset whose display aspect does not exceed the window's (1 % tolerance, so 428 is picked on a 16:9 window).
- Settings apply live; no restart is needed.
- The options screen (M11) lists widescreen under "affects gameplay (visibility)".

## 3. Pipeline

```text
compose (indexed, W x 200)        <- flight screen / front-end screens, as today but W wide
  -> palette to RGBA (W x 200)
  -> light map L (W x 200, RGB, linear), flight playfield only   [lighting != off]
  -> upscale RGBA by k (k = 1..4)                                 [upscaler]
  -> out = upscaled * sample(L) + bloom                           [lighting / bloom]
  -> platform scaling (Aspect 4:3-equivalent / integer / stretch) of the (W*k) x (200*k) image
```

- Aspect correction uses the original pixel aspect 5:6, so a *W* × 200 frame displays at *W* : 240.
- The HUD rows (160..191) and every front-end screen are not lit, so their light map is 1.0.
- With every Enhanced key off, the output is identical to Classic. Test this as an invariant.
- **Marks** (rev 4): flight frames may carry fixed-colour pixels in base coordinates, drawn after "palette to RGBA" and again over the final image as *k* × *k* blocks. Today the only marks are the colour-blind guidance line (accessibility, M12-04, spec frontend/port-front-end rev 2 §3.3). That setting takes this pipeline even under the Classic profile, with *W* = 320 and every stage off, so its output is the Classic picture plus the marks.
- **Screenshots** (rev 4): F12 saves the indexed frame as before. When this pipeline presented the frame, F12 also saves its output, (*W·k*) × (200·*k*) RGBA, before the platform scaling (`<name>.enhanced.png`).

## 4. Widescreen (M12-09)

### 4.1 Widths

| Preset | *W* | Margin each side *m* = (*W* − 320) / 2 |
|---|---:|---:|
| off | 320 | 0 |
| 16:10 | 384 | 32 |
| 16:9 | 428 | 54 |
| 21:9 | 560 | 120 |

### 4.2 Flight playfield

- The playfield is *W* × 160 px.
- Map tiles are drawn from view tile X − ⌈*m* / 32⌉ to the right edge.
- Every screen position is the Classic position plus *m*. This applies to the player ship at (144 + *m*, 68), tiles, ships, shots, explosions, the flame and the guidance dots.
- The view limits (tiles X 6..183, Y 7..190) keep every drawn column inside the 201-wide map for all presets.
- Ships, shots and explosions are drawn whenever they fall inside the wider window. The Classic visibility tests are relaxed by *m* on each side. The overpass redraw (tick-order step 12) also covers the wider window.
- Invariant (test): columns *m* .. *m* + 319 of the widescreen playfield equal the Classic playfield pixel for pixel for the static playfield: the tile window, the player ship and the overpass redraw. Objects that the Classic tests leave out although they already overlap the 320-wide window (a mission ship or explosion one tile column outside the view) are drawn in a wide frame, so they do not pop in at the old window edge.

### 4.3 HUD and screens

- The 320 × 32 HUD bar is drawn at x = *m*.
- The side panels of the HUD band are filled with a mirrored repeat of the bar's outermost 16 columns (columns 0, 1, .., 15, 15, .., 0, 0, 1, .. outward), darkened by half at the RGBA stage, so the frame is continuous at every seam.
- All 320 × 200 front-end screens (menus, story, dialogue, comm screens, Esc menu, F5 map) are centred, with black side panels.
- Text drawn over the finished flight picture is an overlay, not a screen. That covers the infra-red notice and, from rev 4, the pause line. It is drawn over the wide frame at x = *m*, with the pixels it does not touch keeping the wide picture (`widescreen::overlay_centred`). While paused, the frame is the one the pause was drawn over (the tick is suspended).

## 5. Upscaling (M12-10)

Upscaling works on the composed RGBA image. The factor *k* is the scale in the filter name, and the output is (*W·k*) × (200·*k*).

- **scale2x** (EPX / AdvMAME2x), with E the pixel and B, D, F, H its up, left, right and down neighbours:
  - E0 = (D = B ∧ B ≠ F ∧ D ≠ H) ? D : E
  - E1 = (B = F ∧ B ≠ D ∧ F ≠ H) ? F : E
  - E2 = (D = H ∧ D ≠ B ∧ H ≠ F) ? D : E
  - E3 = (H = F ∧ D ≠ H ∧ B ≠ F) ? F : E
- **scale3x** is the AdvMAME3x rule set; **scale4x** is scale2x applied twice.
- **xbr2/3/4**: the public xBR "level 2" algorithm (Hyllian), implemented from its published description. It uses a YUV distance with the standard weights and edge blending at the 2×/3×/4× sub-pixel positions. Each output sub-pixel's blend weight is the exact area of the sub-pixel beyond the edge line, which reproduces the published 2× and 3× weights; for an edge that is both shallow and steep, a sub-pixel takes the larger of the two weights.
- **Edges:** pixels outside the image repeat the nearest edge pixel. HUD and playfield are filtered together; the boundary at row 160 is a hard edge (no blending across rows 159/160).

## 6. Dynamic lighting (M12-11)

### 6.1 Inputs, built each frame by the flight screen

- **Light sources**, each with screen position (base pixels), colour (linear RGB), radius *r* (px), intensity *I* and a `casts_shadows` flag:

| Source | Colour | *r* | *I* | Shadows |
|---|---|---:|---|---|
| Explosion, frames 1..6 | (1.0, 0.55, 0.15) | 56 | 1.6, 1.4, 1.1, 0.8, 0.5, 0.25 by frame | yes |
| Shot kind 0 / 1 / 2 | the kind's palette colour (3Dh / 33h / 35h) | 12 / 14 / 18 | 0.5 | no |
| Missile (kinds 4..6) | (1.0, 0.8, 0.5) | 20 | 1.0 | no |
| Muzzle flash (the tick a shot spawns) | (1.0, 0.9, 0.6) | 24 | 1.2 | no |
| Afterburner flame | (0.6, 0.8, 1.0) | 32 | 1.3 | yes |
| Player engine glow | (0.5, 0.7, 1.0) | 22 | 0.5 | no |
| Mission ship engine glow | (1.0, 0.6, 0.4) | 18 | 0.4 | no |

- **Where each source sits** (base pixels, screen position + *m*; implementation `td_render::enhanced::lights`):
  - explosions, missiles and the flame: the centre of their 32 × 32 frame (top-left + 16), from the same mapping as the sprite blit;
  - cannon shots: the pattern's (0, 0) pixel + 0.5;
  - muzzle flash: one per owner (player or mission slot), at the mean centre of the shots it fired on the tick they are first drawn. That is a cannon shot of age 2 or a missile (kinds 4, 5) of age 1, because spawning sets age 1 (0 for missiles) and the next step 6 ages the shot before drawing it. Homing missiles (kind 6) keep no age and get no flash, and kind 3 is never drawn and gets none;
  - engine glows: behind the ship's centre, against its heading, by 10 px (player) or 8 px (mission ship).
- **Which sources are added:**
  - explosions whenever their light circle reaches the playfield, drawn or not (OB-033 page rule, window edge), so that their light does not flicker;
  - shots and missiles only when they are drawn;
  - mission-ship glows only for drawn ships;
  - the player glow while the ship is drawn;
  - the flame while it is drawn.
- **Occluder grid**: the solid flag (`World::solid`) of every map cell in the drawn window, plus one cell on each side, widened by ⌈*m* / 32⌉ cells. Each cell is 32 × 32 px at its screen position. Cells outside the grid are open.
- **Emissive palette indices**: pixels in these colours glow at full brightness whatever the light map says, and feed bloom. They are:
  - the shot colours 3Ch, 3Dh, 32h, 33h, 35h and BEh;
  - the HUD lamp entries: compass lamps 31h, C1h, 86h, 85h, 2Fh, 2Eh, 30h and 24h, and the missile lamp 26h;
  - the explosion sprite colours, that is, the indices used by the `td.exp` frames (bank slots 20..26), collected when the sprite bank loads (no fixed list).

  Colour 0 is never emissive.
- **Ambient** comes from the mode:
  - `subtle`: ambient (1.0, 1.0, 1.0) and no shadows. Lights only add glow, as min(1 + Σ, 1.6).
  - `full`: "night" ambient (0.38, 0.42, 0.55) with shadows. Light can lift a pixel above its daytime colour up to 1.6×.

### 6.2 Light map

For each pixel *p* of the playfield (rows 0..159, columns 0..*W* − 1):

```text
L(p) = ambient + sum over lights l of  colour_l * I_l * falloff(|p - pos_l| / r_l) * vis_l(p)
falloff(d) = (1 - d^2)^2 for d < 1, else 0
vis_l(p)   = 1 if l does not cast shadows, else the fraction of the segment pos_l -> p
             that crosses no solid cell, softened: march in 4-px steps; each solid cell
             crossed multiplies by 0.25; the cells containing p and pos_l do not count
```

- **The march:**
  - it visits the interior points pos_l + (p − pos_l) · s / n for s = 1 .. n − 1, where n = ⌈|p − pos_l| / 4⌉;
  - a point that enters a new solid cell multiplies vis by 0.25;
  - the march stops once vis ≤ 1/256.

  The light's own cell is excluded so that a source standing on a solid cell (a flame over a building edge) does not darken its whole circle.
- Each channel of L is clamped to 1.6 in both modes.
- **Half resolution:** L is computed at half resolution, one sample per 2 × 2 block. Sample (*i*, *j*) sits at base point (2*i* + 1, 2*j* + 1). It is sampled bilinearly at output pixel centres, with edges clamped.
- Emissive pixels use max(L, 1). The emissive test reads the composed indexed frame, so it uses the base pixel under the output pixel.
- **Final colour** = clamp(srgb(linear(rgb) × L)), with the sRGB curve applied for the multiply.
  - Implementation precision: 8-bit sRGB ↔ 14-bit linear tables with an exact round trip, and L in 12-bit fixed point.
  - So L = 1 leaves a pixel bit-identical, and every step is integer or plain IEEE arithmetic, deterministic on every platform.

### 6.3 Bloom (M12-12, `bloom = true`)

1. Take the emissive pixels and the pixels whose lit luminance is above 0.9. Luminance is the Rec. 709 weighting (0.2126, 0.7152, 0.0722) of the linear colour.
2. Downsample ×4 with a box filter: the mean over each 4 × 4 block, where pixels that do not pass count as black.
3. Blur with two passes of a 9-tap Gaussian (σ = 2 at quarter resolution): horizontal, then vertical, with the edges repeated.
4. Upsample bilinearly to output pixel centres and add at strength 0.6 in linear light, clamped.

- Bloom runs after upscaling, at output resolution.
- It covers the whole flight frame, HUD rows included, so the lamps glow. It never runs on front-end screens or the flight modals.

### 6.4 Interaction with original palette effects

- The hit flash, white flash, infra-red modes and the flight-16 tint are palette changes that come first; lighting then works on the resulting colours.
- In infra-red modes, lighting is limited to `subtle`, which keeps the readability the original intended.

## 7. Performance

Budget: at most 6 ms per presented frame on a mid-range desktop CPU (release build) for the heaviest setting: 21:9, xbr4, `full` lighting and bloom. That is 560 × 200 → 2240 × 800.

- Work may be parallelised by rows with `rayon` (MIT/Apache-2.0). Add it to THIRD_PARTY.md if used.
- **Split of the budget** (rev 3):
  - xbr4 at 560 × 200 takes about 1.3 ms on 16 threads and 1.7 ms on 6;
  - lighting and bloom get at most 4 ms (`tests/enhanced_lighting_timing.rs`, a busy synthetic night).
- **Measured 2026-11-05** on an i7-10700F that other builds kept fully loaded, lighting and bloom together:
  - 3.7 ms on 16 threads (light map 0.6, modulation 1.0, bloom 2.2);
  - 5.1 ms on 6 threads.
- Per-frame buffers are reused, with no allocation in the steady state.
- A `tdtool bench-enhanced` command, or a criterion-free timing test behind `--ignored`, reports the time per stage.

## 8. Tests

- **Classic invariance:** with all keys off, the output equals the Classic RGBA for C-0002 f3543 and a front-end frame (hash).
- **Widescreen invariance:** the centre 320 columns of the playfield equal Classic, at the C-0002 f3500 and f3543 view.
- **Upscaler properties:**
  - a uniform image stays uniform;
  - scale2x on a single-pixel diagonal line produces the known EPX pattern;
  - xbr2/3/4 output hashes on a small synthetic sprite are stored as golden values.
- **Lighting:**
  - with no lights and ambient 1, the map is identity;
  - a single light gives radially symmetric falloff (away from occluders);
  - a solid cell between light and pixel darkens the far side;
  - emissive pixels are unchanged;
  - golden hashes on a synthetic scene.
- **Settings:** parsing round-trip; the Classic profile forces everything off.

## 9. Open questions

- [ ] Q1: tune the light table in §6.1 by eye in the arena. The values above are starting points, and changing them needs no spec revision beyond this table.
- [ ] Q2: should widescreen widen the radar or the F5 map? Proposal: no, they stay original.
- [ ] Q3: should a GPU backend (SDL3 GPU) follow once the CPU reference is done (ADR-0006 alternatives)?
- [x] Q4 (IM-12): relaxed visibility vs the centre invariant. Resolved in rev 2: the invariant covers the static playfield (§4.2).
- [x] Q5 (IM-12): side panels are a mirrored repeat (§4.3, rev 2).
- [x] Q6 (IM-12): xBR blend weights are exact sub-pixel coverage (§5, rev 2).
- [x] Q7 (IM-12): top-level `profile` key, default classic (§2, rev 2).

## 10. Revision history

| Rev | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-11-05 | main session | Initial design from the user's request (ADR-0006) |
| 2 | 2026-11-05 | main session | Q4-Q7 from IM-12 resolved: `profile` key, static-playfield invariant, mirrored-repeat panels, xBR coverage weights, auto tolerance |
| 3 | 2026-11-05 | IM-13 | §6 made precise from the implementation (M12-11, M12-12): source positions and which sources are added (muzzle flash by shot age, explosions drawn or not), occluder grid extent, the HUD lamp entries, the light's own cell excluded from the shadow march, the march points, the half-resolution sample grid, the clamp in both modes, fixed-point precision; §6.3 luminance weights, box mean, blur order, linear-light add, and that bloom covers the HUD rows. §6.1 table tuned by eye in the E1 flight-1 arena (Q1): shot intensity 0.9 → 0.5, because a volley's overlapping lights saturated into a flat tint; engine glows 20 px / 0.35 → 22 / 0.5 (player) and 16 / 0.25 → 18 / 0.4 (mission ships), because they were invisible at night. §7 budget split (lighting and bloom at most 4 ms) and measured timings |
| 4 | 2026-11-06 | IM-16 | §4.3: the pause line is an overlay drawn over the wide frame, like the infra-red notice. §3: fixed-colour marks (the colour-blind guidance line, which takes this pipeline at *W* = 320 under either profile) and the F12 output screenshot at the pipeline's size |
