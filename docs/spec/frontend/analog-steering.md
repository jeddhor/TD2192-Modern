# Optional analog steering

| | |
|---|---|
| Spec ID | `frontend/analog-steering` |
| Revision | 2 |
| Status | Port implementation design, IM-28 |
| Fidelity | T3; Enhanced only, explicit opt-in, outside the D5 preset |

## Summary and units

The profile page offers `analog steering`, off by default. `analog_steering = true` at the top level of settings format 2 stores the choice. Classic forces it off; switching profiles keeps the choice. Restoring the Enhanced preset leaves this separate option alone. Old settings files that lack the key load with it off.

With it enabled, the left stick controls the frequency of the original directional thrust steps. Heading still uses the original steering rules and sixteen sprite headings; acceleration, drag, braking, collisions, afterburner and simulation tick rate keep their original rules. This deliberately changes flight handling. Menus keep the original digital stick handling.

## Conversion

`Pad.stick_x` and `stick_y` carry SDL signed 16-bit axes: positive is right/down. The existing `Pad.x/y` remain the original signs and digital threshold for Classic and menus.

Each axis has an integer remainder and previous sign, sampled once per simulated tick, not per rendered frame:

1. Clamp the absolute raw axis to 32767 (so -32768 is safe).
2. An absolute value at or below 8000 is centred. Clear the remainder and facing direction.
3. Otherwise the original-sign direction is the negative raw sign. On a sign change clear the remainder.
4. Add `abs(raw) - 8000` to the remainder. If it reaches 24767, emit that tick's full original thrust direction and subtract 24767; otherwise emit zero thrust.
5. Independently, send the steady direction as the facing direction every tick. Facing does not alternate when only one thrust axis pulses.

8000 and 24767 are port tuning constants, not original-game measurements. Small deflections produce intermittent original acceleration steps; full deflection thrusts every tick. Because drag and braking are unchanged, this is proportional thrust frequency rather than a promised linear speed curve.

## Precedence and lifetime

- Any bound movement key held takes precedence over the stick on both axes, at original strength. Controller buttons mapped to those keys have the same precedence. Fire and afterburner buttons remain available.
- Any D-pad direction held takes precedence over the stick on both axes, at original strength. Opposite D-pad directions cancel on their axis.
- Disable, disconnect, centre, direction reversal and keyboard/D-pad takeover reset the relevant fractional state. Reconnect starts with no stored thrust.
- The converter belongs to the host flight, and is sampled only for a tick or a resumed tick. Modal screens never generate thrust while they are open. The host reads one centred joystick sample after disconnect to release the last digital direction.

## Simulation and replays

The converter resolves analog input before the deterministic simulation boundary. `TickControls` contains the original digital thrust plus an optional facing override. No raw axis, floating point, device state, settings lookup, wall time or extra RNG enters the simulation.

The override uses previously unused `made` bits: 30 marks its presence, 24/25 are left/right, 26/27 up/down. These metadata bits are excluded from `any_made` and key-table updates. Step 30 applies ordinary thrust with the sampled pulse axes, then ordinary steering with the steady facing axes. At modal resume, T3 thrust uses the current input rather than the earlier step-17 joystick sample. Without the marker the original path is unchanged.

Replay serialization writes version 4 whenever any input has the marker. Its layout is the same 128-bit words as version 3. Older players reject version 4 instead of silently dropping the facing override. Recordings without it still write version 3, and versions 2 and 3 still load. Parsing rejects version-2/3 payloads containing the analog presence marker, rather than reinterpreting formerly reserved bits as a new control mode. Canonical version-4 files must contain at least one marked input; digital-only payloads with a version-4 header are rejected. Validation runs after the complete declared payload has been read, preserving truncation errors for incomplete files. Every accepted version-3/4 payload serializes back to its exact consumed bytes. Unmarked reserved direction bits stay opaque and do not enable analog steering. The resolved words reproduce analog flight without a controller, converter state or profile setting.

## Test vectors and verification

- Raw 0 and +/-8000: no facing or thrust.
- Raw -32768: full positive original-sign thrust each tick.
- Raw +20384: steady negative facing; 500 negative thrust pulses over 1000 ticks.
- A direction reversal starts a fresh remainder; D-pad left overrides an opposing right stick immediately.
- Compare full-diagonal flight with a half-strength horizontal pulse sequence: facing matches each tick, movement differs, and recording/decoding every resolved word reproduces state hashes each tick.
- Settings default/migration, round trip, live profile gate and preset restoration keep the option isolated.

A physical controller acceptance check remains separate from these synthetic tests.

## Open questions

None needed for this implementation. Dead zone and thrust feel remain player-tunable design follow-up candidates; no original-game parity is claimed for this option.

## Provenance

Port design implementing MDG sections 6.2 and 9.5 and the user's D5 decision. Original movement and heading remain `sim/flight-model.md`; only the input conversion and separation of facing are additions.

## Revision history

- Rev 1 (2026-11-09, IM-28): explicit Enhanced-only proportional thrust, steady facing, precedence and replay version 4.

- Rev 2 (2026-11-09, IM-28): enforce the replay version/metadata contract, strict byte round trips and malformed version-4 rejection.
