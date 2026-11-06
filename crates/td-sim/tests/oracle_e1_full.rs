//! L5 parity: whole Episode 1 flights, every tick from setup to the end of the flight, against
//! the original flown with the same inputs (VE-2, M9-21; `docs/parity/M9-playthrough.md`).
//! Needs TD2192_DATA and TD2192_ORACLE. The harness (inputs, capture, what is compared) is in
//! [`full_flight`]; E1 captures: `FILE0001.EXE` started directly in a private rig copy (VE-1's
//! method: `tdstatus.dat` = status 1, kills 0, mission N, cursor 3, script 1; DGROUP 4467h,
//! projectile arrays at 5C4E:0000), scenario `# ve2-e1f<N>-plan`.

mod full_flight;

use td_test_support::{require_data, require_oracle};

fn check(flight: u8) {
    let (data, oracle) = (require_data!(), require_oracle!());
    full_flight::check(&full_flight::E1, &data, &oracle, flight);
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

/// U-390 (docs/parity/M9-playthrough.md D2): the attacks on the player run only while phase
/// < 100 (mission-logic rev 3 §5.14), so after the transmission slot 13 gets no new order.
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

/// The player's death and the end sequence: no input, the enemies destroy the parked ship
/// (VE-4, M10-45; `docs/parity/M10-playthrough.md`).
#[test]
fn flight_01_idle_death_matches_oracle() {
    let (data, oracle) = (require_data!(), require_oracle!());
    full_flight::check_replay(
        &full_flight::E1,
        &data,
        &oracle,
        1,
        "-idle",
        "ve4-e1f1-idle",
    );
}

/// The idle death with the pause (tick 100), the tactical map (200) and the flight menu (300)
/// opened and closed during the flight (VE-4, M10-45).
#[test]
fn flight_01_modal_screens_match_oracle() {
    let (data, oracle) = (require_data!(), require_oracle!());
    full_flight::check_replay(
        &full_flight::E1,
        &data,
        &oracle,
        1,
        "-modals",
        "ve4-e1f1-modals",
    );
}
