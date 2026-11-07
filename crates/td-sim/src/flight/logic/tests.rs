//! Unit tests from `mission-logic.md` rev 1 (§7 vectors and the per-flight pseudocode).

use super::*;
use crate::flight::player::{Player, View};
use crate::flight::{START_VIEW, World};
use crate::flight::{TickResult, tables};
use crate::input::{FlightKey, TickControls};
use td_formats::map::TileMap;

fn open_world() -> World {
    let mut tile_attr = [tables::TILE_SOLID; 256];
    tile_attr[0] = tables::TILE_OPEN;
    World {
        map: TileMap::parse_episode(&vec![0u8; 201 * 201]).unwrap(),
        tile_attr,
        muzzles: crate::flight::shots::Muzzles(vec![0; 342]),
    }
}

/// A scripted flight with every slot empty, the player at the start view.
fn scripted(n: u8) -> Flight {
    let mut f = Flight::with_player(n, Player::new(tables::player_class(n), START_VIEW));
    f.mission.setup(n);
    f
}

/// Puts a live ship of `class` in slot `s` (1-based) at `tile`.
fn put(f: &mut Flight, s: usize, class: u8, tile: (u8, u8)) {
    let sh = &mut f.ships.0[s - 1];
    sh.class = class;
    (sh.tile_x, sh.tile_y) = tile;
    sh.armour = 100;
}

#[test]
fn vectors_2_to_4_near_live_uses_the_view_tile() {
    let mut f = scripted(1);
    put(&mut f, 8, 7, (121, 104));
    assert!(f.near_live(8, 0, 20));
    f.ships.0[7].tile_x = 122;
    assert!(!f.near_live(8, 0, 20));
    put(&mut f, 8, NO_SHIP, (110, 100));
    assert!(!f.near_live(8, 0, 20));
    // `Msn_Near` ignores the class.
    assert!(f.msn_near(8, 0, 20));
}

#[test]
fn setup_posts_the_objective_and_sets_the_home_marker() {
    let f = scripted(1);
    assert_eq!(f.mission.phase, 1);
    assert_eq!(f.mission.ticker.pos, 1);
    assert_eq!(f.mission.ticker.message, Some(Message::Objective(0)));
    assert_eq!(f.mission.ticker.ink, INK);
    assert_eq!(f.mission.home_marker, MARK_HOME);
    let f = scripted(8);
    assert_eq!(f.mission.ticker.message, Some(Message::Objective(3)));
    let f = scripted(19);
    assert_eq!(f.mission.home_marker, MARK_TARGET);
    assert_eq!(f.mission.ticker.ink, INK_ALERT);
    let mut f = scripted(20);
    assert_eq!(f.mission.ticker.message, None);
    assert_eq!(f.mission.ticker.pos, 0);
    f.mission.setup(21);
    assert_eq!(f.mission.ticker.message, Some(Message::Objective(18)));
    assert_eq!(
        Message::Flight2(16).resource(),
        ("e1.flight_messages.02", 16)
    );
}

#[test]
fn ticker_moves_on_page_0_ticks_and_ends_after_78_steps() {
    let mut f = scripted(1);
    // Rules off: flight 1 with no ships would post its next message at once.
    f.mission.scripted = false;
    f.draw_page = 0;
    let w = open_world();
    let mut ticks = 0;
    while f.mission.ticker.pos != 0 {
        f.tick(&w, &TickControls::default(), &mut vec![]);
        ticks += 1;
        assert!(ticks < 400);
    }
    // 78 page-0 ticks starting on a page-0 tick.
    assert_eq!(ticks, 155);
    assert_eq!(f.mission.ticker.window, 78);
    // M replays the idle ticker.
    let mut c = TickControls::default();
    c.set_down(FlightKey::M);
    c.set_made(FlightKey::M);
    f.tick(&w, &c, &mut vec![]);
    assert_eq!(f.mission.ticker.pos, 1);
    assert_eq!(
        f.frame.hud.ticker.pos, 0,
        "the HUD saw the ticker before step 33"
    );
}

/// Flight 1 phase script (§5.1), driven rule by rule.
#[test]
fn flight_1_phases() {
    let mut f = scripted(1);
    for s in [1, 2] {
        put(&mut f, s, 0, (60, 60));
    }
    for s in 3..=11 {
        put(&mut f, s, 0, (60, 60));
    }
    f.msn_update();
    // Tick 1: slots 5..11 hidden (3, 4 are hidden by setup in the real flight).
    assert!(f.dead_range(5, 11));
    assert!(f.live(3) && f.live(4));
    assert_eq!(f.mission.phase, 1);
    // Slots 1 and 2 leave through their exits.
    f.ships.0[0].tile_x = 50;
    f.ships.0[0].tile_y = 196;
    f.ships.0[1].tile_x = 51;
    f.ships.0[1].tile_y = 197;
    f.msn_update();
    assert_eq!(f.mission.phase, 2);
    assert_eq!(f.mission.ticker.message, Some(Message::Flight1(0)));
    assert_eq!(f.class_of(3), 0);
    // 3, 4 gone: phase 3, slots 5..8 spawned (8 as a Vulture), in the same pass.
    f.hide(&[3, 4]);
    f.msn_update();
    assert_eq!(f.mission.phase, 3);
    assert_eq!(
        [f.class_of(5), f.class_of(6), f.class_of(7), f.class_of(8)],
        [0, 0, 0, 7]
    );
    f.hide(&[5, 6, 7, 8]);
    f.msn_update();
    assert_eq!(f.mission.phase, 4);
    assert!(f.live(9) && f.live(10));
    // D waits for the ticker; slot 11 stays hidden meanwhile.
    f.hide(&[9, 10]);
    f.msn_update();
    assert_eq!(f.mission.phase, 4);
    assert!(!f.live(11));
    f.mission.ticker.pos = 0;
    f.msn_update();
    assert_eq!(f.mission.phase, 5);
    assert_eq!(f.class_of(11), 7);
    assert_eq!(f.mission.ticker.message, Some(Message::Flight1(3)));
    f.hide(&[11]);
    f.msn_update();
    assert_eq!(f.mission.phase, PHASE_DONE);
    // The base test runs at the top of the block: success shows one tick later.
    assert!(!f.succeeded);
    f.msn_update();
    assert!(f.succeeded, "the view is at the base tile");
    f.player.view.tile_x += 1;
    f.msn_update();
    assert!(!f.succeeded, "re-evaluated every tick");
}

#[test]
fn engage_orders_an_attack_within_range() {
    let mut f = scripted(1);
    put(&mut f, 8, 7, (110, 110));
    f.msn_update();
    let r = f.ships.0[7].record;
    assert_eq!((r.target_sel, r.attack_mode, r.direct_mode), (0, 1, 1));
    // Slot 14 engages within 10 only.
    put(&mut f, 14, 7, (113, 104));
    f.msn_update();
    assert_eq!(f.ships.0[13].record.attack_mode, 0);
}

/// Flight 2 (§5.2): the announcement repeats while idle; read or not, slots 1 and 2 gone.
#[test]
fn flight_2_transmission_and_convoy() {
    let mut f = scripted(2);
    put(&mut f, 1, 7, (10, 10));
    put(&mut f, 2, 0, (12, 12));
    for s in 3..=7 {
        put(&mut f, s, 0, (20, 20));
        f.ships.0[s - 1].record.path[1] = (30, s as u8);
    }
    f.mission.ticker.pos = 0;
    f.msn_update();
    assert!(f.mission.comm_pending);
    assert_eq!(f.mission.ticker.message, Some(Message::Flight1(5)));
    assert_eq!(f.mission.ticker.ink, INK_ALERT);
    assert!(f.dead_range(3, 7));
    // F6 in phase 1 shows transmission 1.
    assert_eq!(f.comm_viewer(), Some(1));
    assert_eq!(f.mission.phase, 2);
    assert!(!f.mission.comm_pending);
    assert_eq!(f.comm_viewer(), None);
    f.hide(&[1, 2]);
    f.msn_update();
    assert_eq!(f.mission.phase, 3);
    assert_eq!([f.class_of(3), f.class_of(4), f.class_of(5)], [0, 0, 7]);
    assert_eq!(f.ships.0[2].record.waypoint, (30, 3));
    f.hide(&[5]);
    f.msn_update();
    assert_eq!(f.mission.phase, 4);
    assert_eq!([f.class_of(6), f.class_of(7)], [7, 7]);
    f.hide(&[6, 7]);
    f.msn_update();
    assert_eq!(f.mission.phase, PHASE_DONE);
    // From the next tick C's else hides the convoy trucks.
    assert!(f.live(3));
    f.msn_update();
    assert!(f.dead(&[3, 4]));
}

/// Flight 3 (§5.3) and its outcome (§4.8, vector 11).
#[test]
fn flight_3_montgomery() {
    let mut f = scripted(3);
    put(&mut f, 4, 4, (100, 100));
    put(&mut f, 2, 7, (10, 150));
    put(&mut f, 3, 7, (10, 160));
    for s in [5, 6, 7, 8, 12] {
        put(&mut f, s, NO_SHIP, (90, 90));
    }
    f.msn_update();
    assert_eq!(f.mission.phase, 1, "the objective still scrolls");
    f.mission.ticker.pos = 0;
    f.msn_update();
    assert_eq!(f.mission.phase, 2);
    assert_eq!(f.class_of(5), 8);
    assert_eq!(f.ships.0[4].marker, MARK_TARGET);
    // Montgomery arrives.
    f.ships.0[3].tile_x = 107;
    f.ships.0[3].tile_y = 106;
    f.msn_update();
    assert!(!f.live(4));
    assert_eq!(f.mission.ticker.message, Some(Message::Flight1(8)));
    assert_eq!(f.ships.0[1].marker, MARK_TARGET);
    assert!(f.outcome());
    f.mission.ticker.pos = 0;
    f.msn_update();
    assert_eq!(f.mission.phase, 3);
    f.hide(&[2]);
    f.mission.ticker.pos = 0;
    f.msn_update();
    assert_eq!((f.mission.phase, f.class_of(8)), (4, 8));
    f.hide(&[8]);
    f.mission.ticker.pos = 0;
    f.msn_update();
    assert_eq!((f.mission.phase, f.class_of(12)), (5, 7));
    f.hide(&[12]);
    f.mission.ticker.pos = 0;
    f.msn_update();
    assert_eq!(f.mission.phase, PHASE_DONE);
    // Montgomery destroyed elsewhere: outcome 0.
    f.ships.0[3].tile_x = 90;
    assert!(!f.outcome());
}

#[test]
fn respawn_needs_distance_on_both_axes() {
    let mut f = scripted(1);
    put(&mut f, 19, NO_SHIP, (102 + 5 + 16, 104 + 2 + 3));
    f.msn_update_events(&mut vec![]);
    assert!(!f.live(19), "only 3 tiles away in y");
    f.ships.0[18].tile_y = 104 + 2 + 16;
    f.msn_update_events(&mut vec![]);
    assert_eq!((f.class_of(19), f.ships.0[18].armour), (7, 1));
}

#[test]
fn comm_viewer_table() {
    assert_eq!(transmission(14, 2), Some((7, PHASE_DONE)));
    assert_eq!(transmission(14, 1), None);
    let mut f = scripted(14);
    put(&mut f, 9, 7, (1, 1));
    f.ships.0[8].marker = MARK_TARGET;
    f.mission.phase = 2;
    f.mission.comm_pending = true;
    assert_eq!(f.comm_viewer(), Some(7));
    assert_eq!(f.ships.0[8].marker, 0);
    assert_eq!(f.ships.0[0].marker, MARK_TARGET);
    assert_eq!(f.mission.phase, PHASE_DONE);
    // The tick: F6 on page 0 opens the viewer only for a matching phase.
    let w = open_world();
    let mut f = scripted(2);
    put(&mut f, 1, 7, (10, 10));
    put(&mut f, 2, 0, (12, 12));
    f.comm_request = true;
    f.draw_page = 0;
    let r = f.tick(&w, &TickControls::default(), &mut vec![]);
    assert_eq!(r, TickResult::Modal(crate::flight::Modal::CommViewer));
    assert_eq!(f.mission.transmission, 1);
    let mut f = scripted(1);
    f.comm_request = true;
    f.draw_page = 0;
    assert_eq!(
        f.tick(&w, &TickControls::default(), &mut vec![]),
        TickResult::Done
    );
    assert!(!f.comm_request);
}

#[test]
fn indicator_cycles_and_sounds_once() {
    let mut f = scripted(1);
    f.mission.comm_pending = true;
    let mut frames = vec![];
    let mut sounds = 0;
    for _ in 0..9 {
        let mut ev = vec![];
        f.instruments(&mut ev);
        sounds += ev.len();
        frames.push(f.mission.comm_frame());
    }
    assert_eq!(frames, [1, 2, 3, 4, 3, 2, 1, 1, 2]);
    assert_eq!(sounds, 1);
    f.mission.comm_pending = false;
    f.instruments(&mut vec![]);
    assert_eq!(f.mission.comm_frame(), 0);
}

#[test]
fn edge_alert_posts_the_jurisdiction_warning() {
    let mut f = scripted(13);
    f.mission.ticker.pos = 0;
    f.edge_hit = true;
    f.instruments(&mut vec![]);
    assert_eq!(f.mission.ticker.message, Some(Message::Jurisdiction(1)));
    assert!(!f.edge_hit);
}

#[test]
fn flight_7_completion_by_the_guidance_rule() {
    let mut f = scripted(7);
    for s in 1..=12 {
        put(&mut f, s, NO_SHIP, (10 * s as u8, 0));
        f.ships.0[s - 1].marker = MARK_TARGET;
    }
    // The ticker is busy, so rule A cannot post; the instruments set phase 100 (U-180).
    f.msn_update();
    assert_eq!(f.mission.phase, 2);
    f.instruments(&mut vec![]);
    assert_eq!(f.mission.phase, PHASE_DONE);
}

/// Flight 8 (§5.8), OB-022: both Vulture IIs gone in phase 3 before the player came near
/// them. Classic keeps phase 3 for ever (the soft lock, preserved); the Enhanced fix moves on
/// to phase 5, and the flight then ends as usual (rule F).
#[test]
fn flight_8_ob022_soft_lock_only_in_classic() {
    for fix in [false, true] {
        let mut f = scripted(8);
        f.enhanced_fixes = fix;
        f.mission.phase = 3;
        // Slots 8 and 9 are gone (every slot of `scripted` is empty); slot 15 still flies.
        put(&mut f, 15, 7, (20, 20));
        for _ in 0..3 {
            f.msn_update();
        }
        let phase = f.mission.phase;
        assert_eq!(phase, if fix { 5 } else { 3 }, "fix {fix}");
        f.hide(&[15]);
        f.msn_update();
        let done = f.mission.phase == PHASE_DONE;
        assert_eq!(done, fix, "fix {fix}: the flight can be completed");
    }
    // With a live Vulture II the rules are the original's under either profile: no shortcut.
    let mut f = scripted(8);
    f.enhanced_fixes = true;
    f.mission.phase = 3;
    put(&mut f, 8, 8, (200, 10));
    f.msn_update();
    assert_eq!(f.mission.phase, 3);
}

#[test]
fn flight_7_mine_touch_destroys_the_player() {
    let mut f = scripted(7);
    // View (102, 104), offsets (32, 16): the ship is at ((4·8 + 128) / 32, (16 + 84) / 32).
    put(&mut f, 2, 3, (107, 107));
    f.ships.0[1].sub_x = 0;
    f.ships.0[1].sub_y = 4;
    f.msn_update();
    assert!(f.mission_over);
    assert_eq!(f.ships.0[1].armour, 0);
    let busy = f.explosions.0.iter().filter(|e| e.frame == 1).count();
    assert_eq!(busy, 5);
}

#[test]
fn flight_18_bomb_after_1000_ticks() {
    let mut f = scripted(18);
    put(&mut f, 3, 7, (105, 105));
    f.msn_update();
    // B resets the timer and D counts the same tick: 1 at the end of the finding tick, so the
    // bomb goes off 999 ticks later (the spec's vector 12 says t + 1000).
    assert_eq!((f.mission.phase, f.mission.ion_timer), (2, 1));
    for _ in 0..998 {
        f.msn_update();
    }
    assert!(!f.mission.bomb);
    f.msn_update();
    assert!(f.mission.bomb);
    f.msn_update_events(&mut vec![]);
    assert!(f.lost);
}

#[test]
fn flight_19_switch_and_pin() {
    let mut f = scripted(19);
    f.player.view = View {
        tile_x: 120,
        tile_y: 70,
        ..START_VIEW
    };
    f.msn_update_events(&mut vec![]);
    assert_eq!(f.mission.phase, 2);
    assert_eq!(f.player.class, 8);
    assert_eq!(f.player.view.tile_y, 71);
    assert_eq!(f.slot_tile(19), (125, 73));
    f.ships.0[18].tile_x = 0;
    f.player.shields = 40;
    f.flight_19_pin();
    assert_eq!((f.slot_tile(19), f.player.shields), ((125, 73), 10));
    f.msn_update();
    assert_eq!(f.mission.phase, PHASE_DONE);
    assert_eq!(f.mission.home_marker, MARK_HOME);
}

#[test]
fn flight_14_attacks_on_the_player_stop_at_phase_100() {
    // mission-logic rev 3 §5.14 (U-390): slots 8, 9, 10, 12, 13 are ordered to attack the
    // player only while phase < 100; the rules against slots 1..6 still run.
    let mut f = scripted(14);
    let v = f.player.view;
    let near = (v.tile_x as u8, v.tile_y as u8);
    put(&mut f, 13, 7, near);
    f.msn_update();
    assert_eq!(
        f.ships.0[12].record_bytes()[0x415],
        1,
        "attack ordered in phase 1"
    );
    let mut f = scripted(14);
    f.mission.phase = PHASE_DONE;
    put(&mut f, 13, 7, near);
    f.msn_update();
    assert_eq!(
        f.ships.0[12].record_bytes()[0x415],
        0,
        "no new order after the transmission"
    );
}

#[test]
fn scenes_stop_the_tick_only_for_a_host_that_shows_them() {
    // flight-screens rev 4 §4.10: the V.C.I. screen runs inside step 18; with
    // `stop_for_scenes` the tick returns `Modal::Scene` there and resumes at step 19 with the
    // key bytes kept (only the screen's own key changes apply); either way the hit flash ends.
    let world = open_world();
    let at_vci = |stop: bool| {
        let mut f = scripted(19);
        f.player.view = View {
            tile_x: 120,
            tile_y: 70,
            ..START_VIEW
        };
        f.stop_for_scenes = stop;
        f.hit_flash = 12;
        f
    };
    let mut held = TickControls::default();
    held.set_down(FlightKey::Kp8);
    held.set_made(FlightKey::Kp8);
    let mut plain = at_vci(false);
    assert_eq!(plain.tick(&world, &held, &mut vec![]), TickResult::Done);
    assert_eq!((plain.mission.scene, plain.hit_flash), (19, 0));
    let mut f = at_vci(true);
    assert_eq!(
        f.tick(&world, &held, &mut vec![]),
        TickResult::Modal(crate::flight::Modal::Scene)
    );
    assert_eq!((f.mission.scene, f.hit_flash, f.player.class), (19, 0, 8));
    // Nothing changed while it showed: the same key state goes on, as without the stop.
    let mut still = TickControls::default();
    still.set_down(FlightKey::Kp8);
    assert_eq!(f.resume(&world, &still, &mut vec![]), TickResult::Done);
    assert!(
        f.keys.get(FlightKey::Kp8),
        "the key byte survives the screen"
    );
    assert_eq!(crate::hash::state_hash(&f), crate::hash::state_hash(&plain));
}

#[test]
fn outcomes() {
    let mut f = scripted(12);
    assert!(!f.outcome());
    put(&mut f, 3, NO_SHIP, (51, 199));
    assert!(f.outcome());
    let mut f = scripted(17);
    assert!(f.outcome());
    f.lost = true;
    assert!(!f.outcome());
    assert!(scripted(1).outcome());
}

#[test]
fn unscripted_flights_run_no_rules() {
    let mut f = Flight::with_player(1, Player::new(4, START_VIEW));
    f.msn_update();
    assert_eq!(f.mission.phase, 0);
    assert!(f.dead_range(1, 20));
}
