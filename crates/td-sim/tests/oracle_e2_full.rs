//! L5 parity: whole Episode 2 flights, every tick from setup to the end of the flight, against
//! the original flown with the same inputs (VE-4, M10-45; `docs/parity/M10-playthrough.md`).
//! Needs TD2192_DATA and TD2192_ORACLE. The harness (inputs, capture, what is compared) is in
//! [`full_flight`]; E2 captures: `TD2.EXE /sb` in a private rig copy: flight 1 by New game (C-0015's keys), the
//! others by slot 1 of `TD2.SAV` := `NN 00 00` and the restore path, Esc 90 frames after the
//! block's first scene card opens (e2-program-flow §8). DGROUP 3EDAh; the projectile arrays'
//! segment (per flight) is in the dump's file name. Scenario `# ve4-e2f<N>-plan`. A flight
//! without a replay or a capture is skipped.

mod full_flight;

use td_test_support::{require_data, require_oracle};

fn check(flight: u8) {
    let (data, oracle) = (require_data!(), require_oracle!());
    full_flight::check(&full_flight::E2, &data, &oracle, flight);
}

#[test]
fn flight_01_full_matches_oracle() {
    check(1);
}

#[test]
fn flight_02_full_matches_oracle() {
    check(2);
}

#[test]
fn flight_03_full_matches_oracle() {
    check(3);
}

#[test]
fn flight_04_full_matches_oracle() {
    check(4);
}

#[test]
fn flight_05_full_matches_oracle() {
    check(5);
}

#[test]
fn flight_06_full_matches_oracle() {
    check(6);
}

#[test]
fn flight_07_full_matches_oracle() {
    check(7);
}

#[test]
fn flight_08_full_matches_oracle() {
    check(8);
}

#[test]
fn flight_09_full_matches_oracle() {
    check(9);
}

#[test]
fn flight_10_full_matches_oracle() {
    check(10);
}

#[test]
fn flight_11_full_matches_oracle() {
    check(11);
}

#[test]
fn flight_12_full_matches_oracle() {
    check(12);
}

#[test]
fn flight_13_full_matches_oracle() {
    check(13);
}

#[test]
fn flight_14_full_matches_oracle() {
    check(14);
}

#[test]
fn flight_15_full_matches_oracle() {
    check(15);
}

#[test]
fn flight_16_full_matches_oracle() {
    check(16);
}

#[test]
fn flight_17_full_matches_oracle() {
    check(17);
}

#[test]
fn flight_18_full_matches_oracle() {
    check(18);
}

#[test]
fn flight_19_full_matches_oracle() {
    check(19);
}

#[test]
fn flight_20_full_matches_oracle() {
    check(20);
}

/// The homing missile (kind 6, player class 5), Tab and the afterburner (VE-5, `tdtool plan
/// --weapons`): one Tab to the missile weapon, all four homing missiles, three bursts. Each
/// missile moves along the heading it had at the start of the tick; its steering takes effect
/// the next tick (weapons rev 6 §4.3, U-502; BUG-11 fixed by IM-23). The replay holds the 220
/// planned ticks and the 13 ticks without input that C-0207 flew after them, to the success in
/// tick 233.
/// A flight with no input from setup to its end (VE-9, M12 checklist §0 item 6; `tdtool plan
/// --idle`, replay `e2f<NN>-idle.tdr`), against the capture `ve9-e2f<NN>-idle` (C-0360..):
/// every tick, the outcome after teardown.
fn idle(flight: u8) {
    let (data, oracle) = (require_data!(), require_oracle!());
    let tag = format!("ve9-e2f{flight:02}-idle");
    full_flight::check_replay(&full_flight::E2, &data, &oracle, flight, "-idle", &tag);
}

/// The player's death and the end sequence in E2: the enemies destroy the parked ship in
/// flight 7 (617 ticks).
#[test]
fn flight_07_idle_death_matches_oracle() {
    idle(7);
}

/// Outcome 0 in E2: flight 9 succeeds while a convoy truck has escaped east (e2-mission-logic
/// §4.8, vector 12's exit at (177, 169); 1,466 ticks).
#[test]
fn flight_09_idle_outcome_0_matches_oracle() {
    idle(9);
}

/// A lost E2 flight: flight 18's base is destroyed (e2-mission-logic §7 flights 10–20 vector
/// 11: the explosions, the ticker text, lost 148 ticks after the trigger; 968 ticks).
#[test]
fn flight_18_idle_lost_matches_oracle() {
    idle(18);
}

/// U-246, OB-039 (VE-9, C-0379): flight 13 with random key holds (a td-sim search) that destroy
/// the Areo-1 (slot 2) in tick 77 with heading 4, then 150 ticks without input. Flight 13's
/// rule keeps raising a fire request on slot 2's heading every tick, so the gone ship fires a
/// class-FFh volley every second tick from its last tile. Every tick, the projectiles included
/// (the flight goes on).
#[test]
fn flight_13_ghost_volleys_match_oracle() {
    use full_flight::{Leg, Muzzles};
    let (data, oracle) = (require_data!(), require_oracle!());
    full_flight::check_run(
        &full_flight::E2,
        &data,
        &oracle,
        "ve9-e2f13-ghost",
        &[Leg {
            flight: 13,
            suffix: "-ghost",
            muzzles: Muzzles::Capture,
            ends: false,
            front_end: &[],
        }],
    );
}

/// An aborted E2 flight (VE-9; flight-screens §4.1, e2-program-flow §4.3): flight 7 without
/// input, Esc in tick 300, Up (cursor 0 → 3, Quit) and Enter in the menu: `g_MissionAborted`
/// := 1 and the loop ends in tick 300. Every tick (the last against the stall record during
/// the menu) and the flags after teardown.
#[test]
fn flight_07_abort_matches_oracle() {
    use full_flight::{Leg, Muzzles};
    let (data, oracle) = (require_data!(), require_oracle!());
    let leg = Leg {
        flight: 7,
        suffix: "-abort",
        muzzles: Muzzles::Capture,
        ends: true,
        front_end: &[],
    };
    full_flight::check_abort(
        &full_flight::E2,
        (&data, &oracle),
        "ve9-e2f07-abort",
        &leg,
        300,
    );
}

#[test]
fn flight_20_weapons_matches_oracle() {
    let (data, oracle) = (require_data!(), require_oracle!());
    full_flight::check_replay(
        &full_flight::E2,
        &data,
        &oracle,
        20,
        "-weapons",
        "ve5-e2f20-weapons",
    );
}
