//! The hit flash (`g_HitFlash` DS:12C4): the distance flash that kills near the player start,
//! and its decay per tick.
//!
//! Spec: `docs/spec/sim/hit-flash.md` (rev 1) §4.1 (`Sim_DistanceFlash` `0E8B:06C5`), §4.3
//! (decay at tick-order step 14). Fidelity: T1 for when the flash starts and how it decays.
//! The value feeds only the palette (`td_render::flight_palette::FlightDac` draws it); no game
//! rule reads it, so it is not part of the state hash.

use super::player::View;

/// The strongest flash (`0E8B:0731`).
pub const MAX: i16 = 20;
/// Values above this after the decay end the flash (u16 compare, so a wrap below 0 also does).
const RESET_ABOVE: u16 = 30;

/// `Round(Sqrt(sq))` for 0 ≤ sq ≤ 32767: the k with k² − k + 1 ≤ sq ≤ k² + k (spec §4.1; the
/// integer form equals Real48 `Round(Sqrt)` on this range).
#[must_use]
pub fn round_sqrt(sq: u16) -> u16 {
    let k = sq.isqrt();
    if u32::from(sq) > u32::from(k) * u32::from(k) + u32::from(k) {
        k + 1
    } else {
        k
    }
}

/// `Sim_DistanceFlash(TileX, TileY)`: raises `level` to 20 − the distance in whole tiles from
/// the player's tile (view tile + (5, 3)) to `tile`, if that is larger.
///
/// The square is 16 bits wide: a sum of 65,536 or more wraps (and may flash, preserved); a
/// sum of 32,768..65,535 reads as negative, where the original stops with RunError 207 in
/// `Sqrt`; the port gives no flash (spec §5, with OB-009).
pub fn distance_flash(level: &mut u16, view: View, tile: (u8, u8)) {
    let d = if tile.0 == 0xFF {
        0
    } else {
        let dx = i32::from(view.tile_x.wrapping_add(5)) - i32::from(tile.0);
        let dy = i32::from(view.tile_y.wrapping_add(3)) - i32::from(tile.1);
        let sq = (dy * dy + dx * dx) as u16;
        if sq as i16 >= 0 {
            round_sqrt(sq) as i16
        } else {
            return;
        }
    };
    let v = MAX - d;
    if v > 0 && *level < v as u16 {
        *level = v as u16;
    }
}

/// Tick-order step 14's decay in palette `mode`: −2 (−3 in mode 1), and a value above 30
/// (after a wrap below zero) becomes 0. Modes other than 0..=2 leave the value alone.
pub fn decay(level: &mut u16, mode: u8) {
    if *level == 0 || mode > 2 {
        return;
    }
    *level = level.wrapping_sub(if mode == 1 { 3 } else { 2 });
    if *level > RESET_ABOVE {
        *level = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::real48::Real48;

    fn view() -> View {
        View {
            tile_x: 102,
            tile_y: 104,
            sub_x: 32,
            sub_y: 16,
        }
    }

    fn after_ticks(v: u16, mode: u8) -> Vec<u16> {
        let mut level = v;
        let mut out = Vec::new();
        loop {
            decay(&mut level, mode);
            out.push(level);
            if level == 0 {
                return out;
            }
        }
    }

    #[test]
    fn integer_round_sqrt_equals_real48() {
        for sq in 0..=32_767u16 {
            let r = Real48::from_i32(i32::from(sq))
                .sqrt()
                .unwrap()
                .round()
                .unwrap();
            assert_eq!(i32::from(round_sqrt(sq)), r, "sq {sq}");
        }
    }

    #[test]
    fn distance_table() {
        // Spec §4.1 table and §6 vectors 4–7, 9, 10 (player tile (107, 107)).
        let flash = |dx: i16, dy: i16, start: u16| {
            let mut level = start;
            let t = ((107 - dx) as u8, (107 - dy) as u8);
            distance_flash(&mut level, view(), t);
            level
        };
        assert_eq!(flash(0, 0, 0), 20);
        assert_eq!(flash(3, 4, 0), 15);
        assert_eq!(flash(13, 14, 0), 1, "sq 365");
        assert_eq!(flash(14, 14, 0), 0, "sq 392: none");
        assert_eq!(flash(4, 4, 10), 14, "sq 32: v 14 beats 10");
        assert_eq!(flash(10, 11, 10), 10, "v 6 does not lower 10");
        let mut level = 0;
        distance_flash(&mut level, view(), (0xFF, 0));
        assert_eq!(level, 20, "TileX FFh: distance 0");
    }

    #[test]
    fn wrapped_and_negative_squares() {
        // §6 vector 10: dx 188, dy 174 → true sq 65,620 wraps to 84 → v 11.
        let far = View {
            tile_x: 188 - 5,
            tile_y: 174 + 10 - 3,
            ..view()
        };
        let mut level = 0;
        distance_flash(&mut level, far, (0, 10));
        assert_eq!(level, 11);
        // A negative 16-bit square (dx 150, dy 120: 36,900; OB-009): no flash instead of
        // RunError 207.
        let neg = View {
            tile_x: 155,
            tile_y: 127,
            ..view()
        };
        let mut level = 0;
        distance_flash(&mut level, neg, (10, 10));
        assert_eq!(level, 0);
    }

    #[test]
    fn black_box_kills() {
        // C-0009 f4396, C-0023 f1747/f1772, C-0033 f1839 (view (102, 104)): the ship killed
        // in the tick and g_HitFlash after step 14 (modes 0, 1, 1, 0).
        for (tile, mode, shown) in [
            ((103, 110), 0, 13),
            ((110, 122), 1, 2),
            ((113, 122), 1, 1),
            ((107, 91), 0, 2),
        ] {
            let mut level = 0;
            distance_flash(&mut level, view(), tile);
            decay(&mut level, mode);
            assert_eq!(level, shown, "{tile:?}");
        }
    }

    #[test]
    fn decay_per_mode() {
        // §4.3 sequences.
        assert_eq!(after_ticks(20, 0), [18, 16, 14, 12, 10, 8, 6, 4, 2, 0]);
        assert_eq!(after_ticks(19, 2), [17, 15, 13, 11, 9, 7, 5, 3, 1, 0]);
        assert_eq!(after_ticks(20, 1), [17, 14, 11, 8, 5, 2, 0]);
        assert_eq!(after_ticks(1, 0), [0]);
        assert_eq!(after_ticks(3, 1), [0]);
        let mut level = 5;
        decay(&mut level, 3);
        assert_eq!(level, 5, "no block for other modes");
    }
}
