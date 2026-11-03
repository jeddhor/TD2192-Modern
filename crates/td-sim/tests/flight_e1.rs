//! Flight checks on the original Episode 1 map (data-gated).
//!
//! RE-2 observation (flight-model rev 2, captures C-0007/C-0008): at the E1M1 start the cells
//! beside the ship are solid and the view's Y offset is 16 (< 40), so holding right never moves
//! the view; the left and right speed components swap on every bounce.

use td_assets::DataDir;
use td_formats::map::TileMap;
use td_sim::flight::tables::episode1_tile_attributes;
use td_sim::flight::{START_VIEW, World};
use td_sim::{Flight, FlightKey, TickControls};
use td_test_support::require_data;

fn e1_world(dir: &std::path::Path) -> World {
    let data = DataDir::open(dir).expect("data dir");
    World {
        map: TileMap::parse_episode(&data.read("td.map").expect("td.map")).expect("map"),
        tile_attr: episode1_tile_attributes(),
    }
}

#[test]
fn holding_right_at_the_e1m1_start_bounces_in_place() {
    let dir = require_data!();
    let w = e1_world(&dir);
    assert!(w.solid(START_VIEW.tile_x + 6, START_VIEW.tile_y + 2));
    let mut f = Flight::new(1);
    let mut ev = vec![];
    let mut c = TickControls::default();
    c.set_down(FlightKey::Kp6);
    c.set_made(FlightKey::Kp6);
    let mut swaps = 0;
    for _ in 0..40 {
        let before = f.player.speed;
        f.tick(&w, &c, &mut ev);
        assert_eq!(f.player.view, START_VIEW);
        if before.right > 16 && f.player.speed.left > 16 {
            swaps += 1;
        }
    }
    assert!(swaps > 5, "bounces observed: {swaps}");
}
