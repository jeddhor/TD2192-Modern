# Oracle joystick injection proposal (RE-21)

Status: reviewable design, 2026-11-09. Tooling only; no DOSBox-X or game implementation belongs in the MIT port. This proposal has not been implemented or captured. It addresses the joystick part of U-024; keyboard behaviour is already confirmed.

## Injection boundary

Extend oracle 3 in a private DOSBox-X tree. The existing oracle scenario scheduler is the authoritative clock: frame anchors, file-open anchors, emulated milliseconds and tick-watcher generations already work. Add joystick actions alongside keyboard actions. Dispatch through the existing emulated-joystick API declared in DOSBox-X `include/joystick.h`, so the guest still reads its normal 201h gameport. Never patch the guest key table, movement state, poll result or calibration words to emulate a joystick. Never introduce a host thread or wall-clock sleep.

The required tool APIs are `JOYSTICK_Enable`, `JOYSTICK_Move_X`, `JOYSTICK_Move_Y` and `JOYSTICK_Button`. Their source remains outside this repository. A private oracle build must start from pinned oracle-3 source, retain its generation semantics and record a new binary hash/version before use. The shared checkout and pinned oracle-3 binary remain untouched during development.

## Proposed scenario contract

Use zero-based joystick and button numbers, explicitly documented; the first gameport stick is 0, its two buttons are 0 and 1. Require complete state on each state event:

```text
f1                  joy 0 1 0 0 0
open:m1.td+1         joy 0 1 -32768 0 0
tick:10             joy 0 1 0 -32768 1
tick:20             joy 0 1 0 0 0
```

Syntax: `<when> joy <stick> <enabled> <x> <y> <buttons>`. Stick 0..1; enabled 0..1; axes signed integers -32768..32768 (negative X left, negative Y up); button mask 0..3. The event changes all five values together at one scheduler boundary. Convert axes inside the emulator tool to its API domain by dividing by 32768. This power-of-two scale avoids decimal parse drift; endpoints and zero are exact. Reject malformed, out-of-range, non-integral or extra arguments before guest execution. Do not clamp silently. Repeated events are legal; state persists until replaced, including through modal screens and subsequent tickwatch generations. A disabled event must clear buttons and centre axes before disabling.

No implicit button pulses: scenarios state press and release explicitly. No per-frame reapplication: let the emulated gameport retain a held value, as real hardware does. For equal-time actions preserve existing source-line ordering, including keyboard actions and memory dumps. Tick-keyed events keep oracle 3's existing within-1-emulated-ms polling guarantee; they are not claimed to run before a particular instruction in the tick.

## Machine configuration and observation

Use a separate config overlay with `joysticktype=2axis`, deterministic fixed CPU cycles/core and an explicitly recorded `timed` setting. Reference keyboard-only captures currently use `joysticktype=none`; do not change their baseline preset. Disable autofire and host joystick/mapper input. Test the initial device-enable event under headless SDL before assuming the mapper exposes an absent physical device. The explicit enable call is necessary even if no host joystick exists.

Keep timed gameport emulation as the primary experiment and record its setting. An optional `timed=false` run isolates read-count dependence; it is a distinct configuration and must not be compared as the same oracle. Read-count mode and elapsed-emulated-time mode are different emulator behaviours, so calibrate independently. Start with axis positions held before calibration; capture the game's own calibration instead of supplying guessed counts. Preserve the same port state across its repeated measurements.

Add a `JOY` row to events.tsv containing frame, emulated ms, watcher generation/tick when applicable, source line and the complete integer state. Do not put float formatting into the scenario or evidence contract. A small optional 201h trace should record write start and sampled result with caller image-relative address; aggregate ordinary polls so output is bounded. DGROUP dumps at tick completion remain the gameplay evidence. Include config, scenario hash, build hash, fixed clock and calibration-file hash in capture provenance.

## Acceptance checks before adoption

1. Re-run oracle 3 backward-compatibility scenarios C-0058 and C-0156 with no `joy` actions and original configs: byte-identical frames, event logs and memory/framebuffer files, apart from version/build metadata. The parser must not enable a stick merely because the new binary contains the feature.
2. Run a tiny guest probe twice with synthetic centre/endpoints/buttons: verify button polarity, axes, exact event boundaries and identical outputs. Exercise frame/ms/open/tick/intick actions, same-time ordering, a modal stall and a second tickwatch generation. Validate bad scenarios fail before launch.
3. Run the game's calibration with centre and endpoints, then a fixed flight with keyboard released: neutral, each cardinal, diagonals, button 0, button 1 and both. Dump calibrated words and per-tick heading/speed/fire-request state. Repeat twice from identical private-copy state.
4. Hold a button across a dialogue page break, menu and flight transition, then release it. Test disconnected state separately. Compare gameport observations with the game-level gates (controller byte 2); keyboard-only config must ignore axis/button steering.
5. Register only validated captures in the external manifest and add clean numeric vectors to the appropriate input spec. U-024 remains partly open until those measurements exist. Do not claim this design itself resolves joystick behaviour.

## Provenance

Existing tool seams were inspected read-only: DOSBox-X `src/misc/oracle.cpp` scenario parse/scheduler and `include/joystick.h`; `src/hardware/joystick.cpp` exposes enabled state, two axes and two buttons per stick, with timed and read-count gameport handlers. Oracle 3 semantics/config/output are documented in external `re-work/dosbox/ORACLE.md` §§5, 5a, 5b, 6, 7. No tool code is copied here. These are tool design observations, not claims about the original game's steering algorithm.
