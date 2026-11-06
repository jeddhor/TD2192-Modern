//! Episode 1's menu items 6 (about) and 7 (ordering information), shown in-process.
//!
//! Spec: `docs/spec/flow/launcher.md` (rev 1) §4.4, §4.10. In the original, Episode 1 halts and
//! the launcher plays `intro.cmf` while it runs a TFP presentation (`aboutus.exe`, `order.exe`),
//! then starts Episode 1 again on the same menu item. The port does what Episodes 2 and 3 do for
//! the same items (`docs/re/episode-deltas.md` §8.3): it shows `aboutus.scr` or `order.scr` with
//! `intro.cmf`, then returns to the menu with the game unchanged. Running the TFP presentations
//! instead is decision D4. Fidelity: T3 (port shell).
//!
//! Layout and wait: Episode 2's (`crate::scenes_e2::Bg::Aboutus` / `Order`: the picture at x + 1,
//! 30 s or a key). The music follows the launcher: `intro.cmf` with the driver's default loop,
//! stopped when the menu comes back (its loader starts the menu song).

use td_audio::AudioCommand;

use crate::intro::load_song;
use crate::program::Program;
use crate::scenes_e2::Bg;
use crate::screen::FlowContext;
use crate::story::{StoryError, StoryScreen};

/// Menu item 6 of Episode 1.
pub const ABOUT: u8 = 6;
/// Menu item 7 of Episode 1.
pub const ORDER: u8 = 7;

/// The picture of menu item `item` (6 or 7), with `intro.cmf` started.
pub fn about_or_order(
    prog: &mut Program,
    ctx: &mut FlowContext<'_>,
    item: u8,
) -> Result<StoryScreen, StoryError> {
    let bg = if item == ORDER {
        Bg::Order
    } else {
        Bg::Aboutus
    };
    let screen =
        crate::scene_card::scene_card(&prog.data, &prog.assets, &bg.layout(), ["", "", ""])?;
    // The menu song stops; the launcher plays intro.cmf under the extra (launcher §4.1).
    prog.cues.stop(ctx.audio);
    if let Some(song) = load_song(&prog.data, "intro.cmf") {
        ctx.audio.push(AudioCommand::PlayMusic {
            song,
            looping: true,
        });
    }
    Ok(screen)
}
