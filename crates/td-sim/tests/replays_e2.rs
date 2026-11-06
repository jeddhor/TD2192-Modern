//! L4 success replays of Episode 2 (MDG §10.2, ADG §10.5; VE-4, M10-45). The harness is in
//! [`success_replay`]. A flight without a replay is skipped.

mod success_replay;

use td_sim::flight::Episode;
use td_test_support::require_data;

fn check(flight: u8) {
    let data = require_data!();
    success_replay::check(&data, Episode::E2, flight);
}

#[test]
fn flight_01_success_replay() {
    check(1);
}

#[test]
fn flight_02_success_replay() {
    check(2);
}

#[test]
fn flight_03_success_replay() {
    check(3);
}

#[test]
fn flight_04_success_replay() {
    check(4);
}

#[test]
fn flight_05_success_replay() {
    check(5);
}

#[test]
fn flight_06_success_replay() {
    check(6);
}

#[test]
fn flight_07_success_replay() {
    check(7);
}

#[test]
fn flight_08_success_replay() {
    check(8);
}

#[test]
fn flight_09_success_replay() {
    check(9);
}

#[test]
fn flight_10_success_replay() {
    check(10);
}

#[test]
fn flight_11_success_replay() {
    check(11);
}

#[test]
fn flight_12_success_replay() {
    check(12);
}

#[test]
fn flight_13_success_replay() {
    check(13);
}

#[test]
fn flight_14_success_replay() {
    check(14);
}

#[test]
fn flight_15_success_replay() {
    check(15);
}

#[test]
fn flight_16_success_replay() {
    check(16);
}

#[test]
fn flight_17_success_replay() {
    check(17);
}

#[test]
fn flight_18_success_replay() {
    check(18);
}

#[test]
fn flight_19_success_replay() {
    check(19);
}

#[test]
fn flight_20_success_replay() {
    check(20);
}

/// The homing missile, Tab and the afterburner (VE-5, `tdtool plan --weapons`). Planned with the
/// homing order of BUG-9; re-plan it (and capture it again) when BUG-9 is fixed.
#[test]
fn flight_20_weapons_success_replay() {
    let data = require_data!();
    success_replay::check_ending(&data, Episode::E2, 20, "-weapons", false);
}
