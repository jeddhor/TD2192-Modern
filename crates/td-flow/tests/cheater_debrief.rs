//! The debrief's "cheater" line (D9, user 2026-11-06: the original's text when its rule fires;
//! MDG §9.6.9, U-010; episode-program-flow rev 6 §4.5, test vector 3): T = kills to date +
//! mission kills below 0 or above 29,999 gives the cheater line in every episode, under either
//! profile; a failed flight shows the failure note instead. Needs TD2192_DATA (the program's
//! texts); skips otherwise.

use td_assets::{DataDir, Episode, ExeResources};
use td_flow::PlaceholderLauncher;
use td_flow::program::Program;
use td_test_support::require_data;

#[test]
fn impossible_totals_get_the_cheater_line() {
    let data = require_data!();
    let dir = DataDir::open(&data).unwrap();
    let res = ExeResources::load(&dir).unwrap();
    for ep in [Episode::E1, Episode::E2, Episode::E3] {
        let prog =
            Program::for_episode(&dir, &res, Box::new(PlaceholderLauncher::default()), ep).unwrap();
        let cheater = prog.debrief_verdict(0, -1, true);
        assert!(!cheater.trim().is_empty(), "{ep}: the cheater line");
        // T < 0 and T > 29,999 share the line (string 051A:6CE7 in E1).
        assert_eq!(prog.debrief_verdict(-3, 2, true), cheater, "{ep}");
        assert_eq!(prog.debrief_verdict(1, 29_999, true), cheater, "{ep}");
        // Ordinary totals get a tier, never the cheater line.
        for (kills, to_date) in [(0, 0), (5, 10), (1, 299), (0, 29_999)] {
            let v = prog.debrief_verdict(kills, to_date, true);
            assert_ne!(
                v,
                cheater,
                "{ep}: T = {}",
                i32::from(kills) + i32::from(to_date)
            );
        }
        // A failed flight shows the failure note, whatever T is.
        assert_ne!(prog.debrief_verdict(0, -1, false), cheater, "{ep}");
    }
}
