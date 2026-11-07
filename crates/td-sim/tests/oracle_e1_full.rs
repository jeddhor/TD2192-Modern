//! L5 parity: whole Episode 1 flights, every tick from setup to the end of the flight, against
//! the original flown with the same inputs (VE-2, M9-21; `docs/parity/M9-playthrough.md`).
//! Needs TD2192_DATA and TD2192_ORACLE. The harness (inputs, capture, what is compared) is in
//! [`full_flight`]; E1 captures: `FILE0001.EXE` started directly in a private rig copy (VE-1's
//! method: `tdstatus.dat` = status 1, kills 0, mission N, cursor 3, script 1; DGROUP 4467h,
//! projectile arrays at 5C4E:0000), scenario `# ve2-e1f<N>-plan`.

mod full_flight;

use td_sim::flight::heap::{self, HeapUse, PortraitLoad};
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

/// Missiles, Tab and the afterburner (VE-5, `tdtool plan --weapons`): flight 8 (class 2) with
/// the volley weapon, one Tab to the missile weapon (kind 4), six missiles and three
/// afterburner bursts (the burst frame is compared every tick).
#[test]
fn flight_08_weapons_matches_oracle() {
    let (data, oracle) = (require_data!(), require_oracle!());
    full_flight::check_replay(
        &full_flight::E1,
        &data,
        &oracle,
        8,
        "-weapons",
        "ve5-e1f08-weapons",
    );
}

/// Flight 15 (class 3, the helicopter): one Tab to the missile weapon (kind 5) and all twenty
/// missiles (VE-5).
#[test]
fn flight_15_weapons_matches_oracle() {
    let (data, oracle) = (require_data!(), require_oracle!());
    full_flight::check_replay(
        &full_flight::E1,
        &data,
        &oracle,
        15,
        "-weapons",
        "ve5-e1f15-weapons",
    );
}

/// Flight 3 reached through the front end (VE-5): `FILE0001.EXE` at mission 3, Enter at the
/// menu, the pre-flight story played (title and hangar cards, dialogue 11 with Space, C-0160's
/// keys), then `e1f03.tdr`. The session comes from program start; the muzzle bytes are the
/// ones the screens left (OB-017), taken from the capture here and built by the port from its
/// own screens in `td-flow/tests/front_end_state.rs` against the same capture.
#[test]
fn flight_03_after_its_story_matches_oracle() {
    use full_flight::{Leg, Muzzles};
    let (data, oracle) = (require_data!(), require_oracle!());
    full_flight::check_run(
        &full_flight::E1,
        &data,
        &oracle,
        "ve5-e1f03-story",
        &[Leg {
            flight: 3,
            suffix: "",
            muzzles: Muzzles::Capture,
            ends: true,
            front_end: &[],
        }],
    );
}

/// Block 19 in one program run: flight 19 (`e1f19.tdr`, direct start), the front end's heap
/// uses, then flight 21 (`e1f21.tdr`: no input; it ends itself after 25 ticks through the end
/// sequence) with the session flight 19 left (episode-program-flow §4.4 row 19). Flight 21's
/// ships start from the path arrays' leftovers (mission-ships rev 6 §4.1.1, U-500).
fn block_19(tag: &str, front_end: &[HeapUse]) {
    block_19_with(tag, full_flight::Muzzles::DirectStart, front_end);
}

/// [`block_19`] with flight 19's direct-start muzzle world `start` (music on or off).
fn block_19_with(tag: &str, start: full_flight::Muzzles, front_end: &[HeapUse]) {
    use full_flight::{Leg, Muzzles};
    let (data, oracle) = (require_data!(), require_oracle!());
    full_flight::check_run(
        &full_flight::E1,
        &data,
        &oracle,
        tag,
        &[
            Leg {
                flight: 19,
                suffix: "",
                muzzles: start,
                ends: true,
                front_end: &[],
            },
            Leg {
                flight: 21,
                suffix: "",
                muzzles: Muzzles::Capture,
                ends: true,
                front_end,
            },
        ],
    );
}

/// C-0200 (VE-5, oracle build 3): the debrief times out, Esc at the voffice card jumps to
/// flight 21 (`M8`, the debrief, `M5`, the card; §4.1.1 vector 1).
#[test]
fn flights_19_and_21_in_one_run_match_oracle() {
    block_19("ve5-e1f19-f21", &heap::block_19_to_voffice(true));
}

/// C-0300 (RE-18): the whole post-flight story with no Esc (§4.1.1 vector 2): after the card,
/// dialogue 88 (faces 24 left, 11 right), `M3`, the hangar card and dialogue 89 (faces 1 left,
/// 2 right). `td-flow/tests/e1_block_flow.rs` checks that the campaign reports these uses.
#[test]
fn flights_19_and_21_after_the_whole_story_match_oracle() {
    let p = |face, right| PortraitLoad { face, right };
    let mut front_end = heap::block_19_to_voffice(true);
    front_end.extend([
        HeapUse::Screen(vec![p(24, false), p(11, true)]),
        HeapUse::Song("hangar.cmf".to_owned()),
        HeapUse::card(),
        HeapUse::Screen(vec![p(1, false), p(2, true)]),
    ]);
    block_19("re18-e1f19-f21-full", &front_end);
}

/// C-0323 (RE-19, U-540): C-0300's run with music off (`td.cfg` 01 02 02). No song is ever
/// loaded, so flight 19's muzzle bytes hold the music-off `g_MemAvailText` `209520` (weapons
/// rev 7 §3.3 item 2, U-560, BUG-22) and flight 21's path arrays come from the fonts and
/// portraits at the heap mark (mission-ships rev 7 §4.1.1 vector 3): the debrief, the card,
/// dialogue 88, the hangar card and dialogue 89, without `low`, `vulture` and `hangar`. Both
/// flights every tick, flight 21's 58 ticks included.
#[test]
fn flights_19_and_21_with_music_off_match_oracle() {
    let p = |face, right| PortraitLoad { face, right };
    let mut front_end = heap::block_19_to_voffice(false);
    front_end.extend([
        HeapUse::Screen(vec![p(24, false), p(11, true)]),
        HeapUse::card(),
        HeapUse::Screen(vec![p(1, false), p(2, true)]),
    ]);
    block_19_with(
        "re19-e1f19-f21-musicoff",
        full_flight::Muzzles::DirectStartMusicOff,
        &front_end,
    );
}

/// Ending B (RE-19, C-0324; episode-program-flow rev 7 §6 vector 5): flight 17 at a direct
/// start, flown with `e1f17.tdr` for ticks 1..150 (ship 1 killed, transmission 8 answered),
/// every key up from tick 151, then nothing: three ships reach the base and the flight ends
/// lost at tick 1075 (kills 1). Every tick against the capture; the last against the stall
/// record after it (no outcome dump: the capture goes on to the retry prompt, the debrief and
/// the ending-B screens, `td-flow/tests/screens_parity.rs`).
#[test]
fn flight_17_lost_matches_oracle() {
    use full_flight::{Leg, Muzzles};
    let (data, oracle) = (require_data!(), require_oracle!());
    let Some(mut r) = full_flight::replay(&full_flight::E1, 17, "") else {
        eprintln!("SKIPPED: no replay for flight 17");
        return;
    };
    r.inputs.truncate(150);
    r.inputs.resize(1075, 0);
    r.interval = 0;
    r.checkpoints.clear();
    let leg = Leg {
        flight: 17,
        suffix: "",
        muzzles: Muzzles::DirectStart,
        ends: true,
        front_end: &[],
    };
    full_flight::check_run_with(
        &full_flight::E1,
        (&data, &oracle),
        "re19-e1f17-lost",
        &[leg],
        &[r],
        false,
    );
}

/// The flight-8 soft lock (U-182, OB-022; RE-20's C-0342, `e1f08-softlock.tdr`): `e1f08.tdr`'s
/// keys for ticks 1..157 (slots 12 and 13 destroyed, phase 3, the Vulture IIs 8 and 9 spawned
/// in tick 157), then keypad 6 held: the player flies east to the map limit while slots 8 and
/// 9 leave by their exits (ticks 788 and 860) with the phase still 3, so the flight can no
/// longer be won. td-sim keeps the original by default (`Flight::enhanced_fixes` false, the
/// Classic profile): every tick 1..999 against the capture, the map edge included.
#[test]
fn flight_08_soft_lock_matches_oracle() {
    use full_flight::{Leg, Muzzles};
    let (data, oracle) = (require_data!(), require_oracle!());
    full_flight::check_run(
        &full_flight::E1,
        &data,
        &oracle,
        "re20-e1f08-softlock",
        &[Leg {
            flight: 8,
            suffix: "-softlock",
            muzzles: Muzzles::DirectStart,
            ends: false,
            front_end: &[],
        }],
    );
}

/// Vertical bounces and a map limit in a capture (VE-9; flight-model §4.3, §4.4, collision
/// rules 1 and 2): `e1f08.tdr`'s keys for ticks 1..157, then keypad 8 held to tick 1,099 (the
/// ship bounces off the walls north of it some 900 times), keypad 4 held from 1,100: the view
/// reaches the west limit (tile X 6), is pushed back a tile at a time with the edge alert, and
/// the enemies destroy the ship; the flight ends in tick 1,453. Every tick and the outcome.
#[test]
fn flight_08_bounces_and_west_limit_match_oracle() {
    let (data, oracle) = (require_data!(), require_oracle!());
    full_flight::check_replay(
        &full_flight::E1,
        &data,
        &oracle,
        8,
        "-move",
        "ve9-e1f08-move",
    );
}

/// A flight with no input from setup to its end (VE-9, M12 checklist §0 item 6; `tdtool plan
/// --idle`, replay `e1f<NN>-idle.tdr`), against the capture `ve9-e1f<NN>-idle` (C-0360..):
/// every tick, the outcome after teardown.
fn idle(flight: u8) {
    let (data, oracle) = (require_data!(), require_oracle!());
    let tag = format!("ve9-e1f{flight:02}-idle");
    full_flight::check_replay(&full_flight::E1, &data, &oracle, flight, "-idle", &tag);
}

/// Teardown outcome 0 (mission-logic §4.8): flight 12 succeeds without input while a convoy
/// ship stands on row 199 (697 ticks; VE-7's C-0274 showed the failure debrief).
#[test]
fn flight_12_idle_outcome_0_matches_oracle() {
    idle(12);
}

/// Flight 16 without input: the flight ends in tick 1,537 with the mission succeeded and the
/// player destroyed in the same run of the end sequence (both flags, as td-sim predicted).
#[test]
fn flight_16_idle_matches_oracle() {
    idle(16);
}

/// A lost flight: flight 20 without input, the base falls in tick 2,673 (VE-7's C-0279 showed
/// ending C after it).
#[test]
fn flight_20_idle_lost_matches_oracle() {
    idle(20);
}
