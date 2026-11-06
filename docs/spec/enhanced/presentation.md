# Enhanced presentation: widescreen, upscaling and dynamic lighting

| | |
|---|---|
| **Spec ID** | `enhanced/presentation` |
| **Revision** | 2 |
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
| Shot kind 0 / 1 / 2 | the kind's palette colour (3Dh / 33h / 35h) | 12 / 14 / 18 | 0.9 | no |
| Missile (kinds 4..6) | (1.0, 0.8, 0.5) | 20 | 1.0 | no |
| Muzzle flash (the tick a shot spawns) | (1.0, 0.9, 0.6) | 24 | 1.2 | no |
| Afterburner flame | (0.6, 0.8, 1.0) | 32 | 1.3 | yes |
| Player engine glow | (0.5, 0.7, 1.0) | 20 | 0.35 | no |
| Mission ship engine glow | (1.0, 0.6, 0.4) | 16 | 0.25 | no |

- **Occluder grid**: the solid flag (`World::solid`) of every map cell in the drawn window. Each cell is 32 × 32 px at its screen position.
- **Emissive palette indices**: pixels in these colours glow at full brightness whatever the light map says, and feed bloom. They are the shot colours (3Ch, 3Dh, 32h, 33h, 35h, BEh), the HUD lamp entries, and the explosion sprite colours. That last set is the indices used by `td.exp` frames, collected when the sprite bank loads (no fixed list).
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
             crossed multiplies by 0.25; the cell containing p does not count
```

- Compute L at half resolution and sample it bilinearly.
- Emissive pixels use max(L, 1).
- Final colour = clamp(srgb(linear(rgb) × L)), with the sRGB curve applied for the multiply.

### 6.3 Bloom (M12-12, `bloom = true`)

1. Take the emissive pixels and pixels whose lit luminance is above 0.9.
2. Downsample ×4 (box filter).
3. Blur with two passes of a 9-tap Gaussian (σ = 2 at quarter resolution).
4. Upsample bilinearly and add at strength 0.6.

Bloom runs after upscaling, at output resolution.

### 6.4 Interaction with original palette effects

- The hit flash, white flash, infra-red modes and the flight-16 tint are palette changes that come first; lighting then works on the resulting colours.
- In infra-red modes, lighting is limited to `subtle`, which keeps the readability the original intended.

## 7. Performance

Budget: at most 6 ms per presented frame on a mid-range desktop CPU (release build) for the heaviest setting: 21:9, xbr4, `full` lighting and bloom. That is 560 × 200 → 2240 × 800.

- Work may be parallelised by rows with `rayon` (MIT/Apache-2.0). Add it to THIRD_PARTY.md if used.
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
