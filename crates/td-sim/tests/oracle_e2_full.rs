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
