//! L5 parity: whole Episode 3 flights, every tick from setup to the end of the flight, against
//! the original flown with the same inputs (VE-4, M10-45; `docs/parity/M10-playthrough.md`).
//! Needs TD2192_DATA and TD2192_ORACLE. The harness (inputs, capture, what is compared) is in
//! [`full_flight`]; E3 captures: `TD3.EXE /sb` in a private rig copy: flight 1 by New game (C-0036's keys), the
//! others by slot 1 of `TD3.SAV` := `NN 00 00` and the restore path, Esc 121 frames after the
//! block's first scene card opens or 90 after its title card (e3-program-flow §8). Flights 7
//! and 21 are reached only through flights 6 and 11 (OB-051, e3-program-flow §4.4.1, §4.4.2). DGROUP 3AE9h; the projectile arrays'
//! segment (per flight) is in the dump's file name. Scenario `# ve4-e3f<N>-plan`. A flight
//! without a replay or a capture is skipped.

mod full_flight;

use td_test_support::{require_data, require_oracle};

fn check(flight: u8) {
    let (data, oracle) = (require_data!(), require_oracle!());
    full_flight::check(&full_flight::E3, &data, &oracle, flight);
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

/// U-430: the player leaves the home hangar of flight 10 and flies back in (RE-17's captures,
/// tick-keyed input from their scenarios: `tdtool schedule-replay`). Part of the flight, every
/// tick; the captures dump no projectiles (VE-5).
fn hangar(suffix: &str, tag: &str) {
    let (data, oracle) = (require_data!(), require_oracle!());
    full_flight::check_run(
        &full_flight::E3,
        &data,
        &oracle,
        tag,
        &[full_flight::Leg {
            flight: 10,
            suffix,
            muzzles: full_flight::Muzzles::Capture,
            ends: false,
        }],
    );
}

/// C-0182: a 12-tick dive south out of `home17`, back north into it at tick 71, two bounces off
/// the hangar top.
#[test]
fn flight_10_hangar_return_b_matches_oracle() {
    hangar("-hangar-b", "re17-e3f10-hangar-b");
}

/// C-0183: a 16-tick dive south to view (17, 166), back in at tick 70, sideways thrust that does
/// not move the ship, a bounce off the hangar top.
#[test]
fn flight_10_hangar_return_c_matches_oracle() {
    hangar("-hangar-c", "re17-e3f10-hangar-c");
}
