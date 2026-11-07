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

/// Two flights of one block in one program run (VE-5, oracle build 3): the first from its
/// direct start, the second with the session the first left (e3-program-flow §4.4.1, §4.4.2).
fn block_run(first: u8, second: u8, tag: &str) {
    use full_flight::{Leg, Muzzles};
    let (data, oracle) = (require_data!(), require_oracle!());
    full_flight::check_run(
        &full_flight::E3,
        &data,
        &oracle,
        tag,
        &[
            Leg {
                flight: first,
                suffix: "",
                muzzles: Muzzles::Capture,
                ends: true,
                front_end: &[],
            },
            Leg {
                flight: second,
                suffix: "",
                muzzles: Muzzles::Capture,
                ends: true,
                front_end: &[],
            },
        ],
    );
}

/// Block 6: flight 6, the debrief (15 s), Esc at the middle part's maraud3 card, flight 7
/// (M10-30).
#[test]
fn flights_06_and_07_in_one_run_match_oracle() {
    block_run(6, 7, "ve5-e3f06-f07");
}

/// Block 11: flight 11, no debrief, Esc ends dialogue 64, flight 21 (M10-43).
#[test]
fn flights_11_and_21_in_one_run_match_oracle() {
    block_run(11, 21, "ve5-e3f11-f21");
}

/// Flight 9 reached through its pre-flight story (VE-5): the keys of C-0180 (title card, kolorvci
/// card, dialogue 53, mining2 card, dialogue 54, Space every 120 frames), then `e3f09.tdr`. The
/// session comes from program start through the story; the muzzle bytes are the capture's
/// (U-441: the story changes them; this replay takes the same course with either).
#[test]
fn flight_09_after_its_story_matches_oracle() {
    use full_flight::{Leg, Muzzles};
    let (data, oracle) = (require_data!(), require_oracle!());
    full_flight::check_run(
        &full_flight::E3,
        &data,
        &oracle,
        "ve5-e3f09-story",
        &[Leg {
            flight: 9,
            suffix: "",
            muzzles: Muzzles::Capture,
            ends: true,
            front_end: &[],
        }],
    );
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
            front_end: &[],
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

/// A flight with no input from setup to its end (VE-9, M12 checklist; `tdtool plan --idle`,
/// replay `e3f<NN>-idle.tdr`), against the capture `ve9-e3f<NN>-idle` (C-0360..): every tick,
/// the outcome after teardown.
fn idle(flight: u8) {
    let (data, oracle) = (require_data!(), require_oracle!());
    let tag = format!("ve9-e3f{flight:02}-idle");
    full_flight::check_replay(&full_flight::E3, &data, &oracle, flight, "-idle", &tag);
}

/// e3-mission-logic §7 vector 17: the six trucks of flight 2 drive to row 199 and leave by the
/// exit rules; A completes the flight in the tick of the last exit and the outcome is 0
/// (752 ticks).
#[test]
fn flight_02_idle_trucks_escape_matches_oracle() {
    idle(2);
}

/// e3-mission-logic §7 vector 19 (second part): the trucks of flight 4 reach (199, 111), so the
/// flight completes with outcome 0 (761 ticks).
#[test]
fn flight_04_idle_trucks_escape_matches_oracle() {
    idle(4);
}

/// e3-mission-logic §7 vector 15 (second part): flight 8 without input ends with outcome 0
/// (1,312 ticks).
#[test]
fn flight_08_idle_matches_oracle() {
    idle(8);
}

/// e3-mission-logic §7 vector 16: the last truck reaches (168, 199) with the player unmoved;
/// the exit hides it, A sets phase 100, the base test succeeds two ticks later; outcome 0
/// (1,054 ticks).
#[test]
fn flight_09_idle_truck_exit_matches_oracle() {
    idle(9);
}

/// e3-mission-logic §7 vectors 6–8 without input: flight 10's counter phases and the truck's
/// end; outcome 0 (1,049 ticks).
#[test]
fn flight_10_idle_matches_oracle() {
    idle(10);
}

/// The player's death in E3 (VE-9): flight 8 on C-0041's keypad route into the hazard cells
/// (40, 131..133), tick-keyed (`e3f08-death.tdr`: keypad 2 from tick 22, 6 from 38, 8 from 109,
/// 6 from 252 to 280). The hazard damage (e3-mission-ships §7 vectors 7–9, OB-054) destroys the
/// ship; the end sequence and its hit flash 17, 15, …, 0 (sim/hit-flash §6 vector 1) are
/// compared every tick to the end of the flight (297 ticks) and the outcome after teardown.
#[test]
fn flight_08_hazard_death_matches_oracle() {
    let (data, oracle) = (require_data!(), require_oracle!());
    full_flight::check_replay(
        &full_flight::E3,
        &data,
        &oracle,
        8,
        "-death",
        "ve9-e3f08-death",
    );
}

/// The flight model's LIKELY parts in a capture (VE-9; flight-model §4.3–§4.6, collision rule
/// 1): flight 9 from the hangar, keypad 4 for ticks 1..149 (against the hangar wall), keypad 2
/// for 150..349 (vertical bounces off the hangar's south wall, ticks 180..198), keypad 8 and 6
/// together for 350..499 (the diagonal steering out of the hangar), keypad 8 for 500..649, then
/// nothing to tick 700. Every tick (the flight does not end).
#[test]
fn flight_09_movement_matches_oracle() {
    use full_flight::{Leg, Muzzles};
    let (data, oracle) = (require_data!(), require_oracle!());
    full_flight::check_run(
        &full_flight::E3,
        &data,
        &oracle,
        "ve9-e3f09-move",
        &[Leg {
            flight: 9,
            suffix: "-move",
            muzzles: Muzzles::Capture,
            ends: false,
            front_end: &[],
        }],
    );
}

/// BUG-18 (U-570; mission-ships rev 8 §4.2.3): flight 19 without input for 4,499 ticks
/// (RE-20's C-0340, `e3f19-idle.tdr`). Slot 3 (class 0) parks at (102, 199) with its goal on
/// row 200; its offset Y passes 32,767 in tick 4,235 and wraps to −32,766, then the ship climbs
/// one row a tick and the row-1 rule removes it in tick 4,433. Every tick against the capture
/// (the flight does not end).
#[test]
fn flight_19_idle_past_the_bottom_edge_wrap_matches_oracle() {
    use full_flight::{Leg, Muzzles};
    let (data, oracle) = (require_data!(), require_oracle!());
    full_flight::check_run(
        &full_flight::E3,
        &data,
        &oracle,
        "re20-e3f19-idle",
        &[Leg {
            flight: 19,
            suffix: "-idle",
            muzzles: Muzzles::Capture,
            ends: false,
            front_end: &[],
        }],
    );
}

/// e3-mission-logic §7 vector 11: flight 12 without input; truck 4 on (93, 76) at teardown,
/// outcome 0 (645 ticks).
#[test]
fn flight_12_idle_matches_oracle() {
    idle(12);
}
