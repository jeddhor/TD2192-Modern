# ADR-0007: Canonical Classic tick rate

- **Status:** Accepted (the user, 2026-11-06; from VE-3's M1-06 measurements)
- **Date:** 2026-11-06

## Context

The original paces its flight with the BIOS timer tick, and the WORX sound driver reprograms the timer, so the tick rate depends on the sound configuration ([core/timing.md](../spec/core/timing.md) §4.2). VE-3 measured it in the oracle at preset P1 ([docs/parity/M12-timing.md](../parity/M12-timing.md), capture C-0074, nine runs from 8,000 to 60,000 cycles):

| Original configuration | Ticks per second |
|---|---:|
| Sound Blaster with music playing (the shipped `td.cfg` and the oracle reference) | 19.21 (19.208 over 378 s) |
| Sound Blaster, music off | 18.52 |
| `/nosound` | 18.21 |

Everything paced in DOS time scales the same way. Examples are the timed story holds (BUG-7) and the flight's real-time delays. With music playing, DOS time runs 5.47 % faster than wall time.

The port uses one fixed rate today, `FLIGHT_TICK_HZ` = 19.202 Hz (`td_flow::screen`). It also has a music volume rather than a music on/off switch in `td.cfg`, so a player who turns the music down still has the "music playing" timer state.

## Decision

1. **Classic runs at 19.2015 Hz in every configuration.** This is the reference configuration, Sound Blaster with music. `FLIGHT_TICK_HZ` becomes 1,193,182 / 62,140 = 19.2015 Hz, the exact divisor product for that WORX state (timing §4.2); the measured 19.208 is within the oracle's clock error. The port's current 19.202 is 0.003 % away, so play does not change.
2. **DOS-time waits use the same clock.** DOS time counts BIOS ticks at a nominal 18.2065 per second, so a wait of *s* DOS seconds lasts *s* × 18.2065 / 19.2015 ≈ 0.948 *s* wall seconds (10 s = 664 retrace frames, as VE-3 measured). This is the fix for BUG-7.
3. The music-off (18.52 Hz) and `/nosound` (18.21 Hz) rates are not reproduced: the port has no equivalent of those driver states. This is recorded as a T2 deviation in timing.md.

## Consequences

- One rate keeps replays and parity tests independent of audio settings.
- timing.md is revised to rev 3, with the measured rates CONFIRMED and this canonical choice, and M1-06 can close.
- An implementer applies the BUG-7 scaling to every DOS-time wait (story holds, flight real-time delays, intro holds where the spec uses DOS time).

## Alternatives considered

- **Follow the original's configuration-dependent rate** (19.21 / 18.52 / 18.21). This would be the most faithful, but the port's audio settings do not map onto the original's driver states, and replays would depend on audio settings. Rejected unless the user wants it.
- **A round 19.2 Hz or 70.086 / 3.65:** no advantage over the exact divisor product.
