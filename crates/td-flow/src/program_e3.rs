//! The Episode 3 program (`td3.exe` main body): script choice, main menu and the campaign.
//!
//! Spec: `docs/spec/flow/e3-program-flow.md` rev 1 §4.1 (start-up), §4.2 (menu), §4.6
//! (bookkeeping), §7 (music). E3's program is E2's with the deltas below, so it is
//! [`LaterEpisode`] (`crate::program_e2`) with the E3 campaign ([`CampaignE3`]) and these
//! [`FrontEnd`] constants:
//!
//! - files and texts: `dialogu3.dat`, `e3.script_choice.warning`, `e3.background_story`;
//! - the menu song starts only from the loader (once at program start; E2 twice);
//! - `UI_LoadMainMenu` flushes the keyboard, so keys typed after the script choice do not
//!   reach the menu;
//! - the episode is complete when the menu's mission counter reaches **20** (block 19 + 1);
//! - `Main_CampaignLoop` does not clear the skip flag, which the menu keeps between blocks
//!   (OB-050), and missions without a block (7, 20, above 20) add the stale mission kills
//!   (OB-051);
//! - every timed wait flushes the keyboard on entry (§3.5; `StoryScreen::flushing_waits`);
//! - `order.scr` waits 45 s (`crate::scenes_e3`).
//!
//! Not ported, as in E2: the configuration screen, switches, memory gate, joystick
//! calibration and exit notice. Save and restore are E1's screens on the port's own
//! `td3.sav`/`td3.stt` (save-screens rev 1 §4.5, e3-program-flow §4.7; the slot format is E1's:
//! mission, then kills to date; missions 1..20 count as used, so 7 and 20 restore, OB-051).

use td_assets::Episode;
use td_input::TickInput;
use td_render::Framebuffer;

use crate::campaign_e2::BlockResult;
use crate::campaign_e3::CampaignE3;
use crate::program::Program;
use crate::program_e2::{BlockStart, EpisodeCampaign, FrontEnd, LaterEpisode};
use crate::save_screens::SaveFiles;
use crate::scenes_e3::{self, Bg};
use crate::screen::FlowContext;
use crate::story::{StoryError, StoryScreen};

/// The Episode 3 program.
pub type Episode3 = LaterEpisode<CampaignE3>;

/// The Episode 3 front end (e3-program-flow rev 1 §4.1, §4.2).
pub const E3_FRONT: FrontEnd = FrontEnd {
    episode: Episode::E3,
    script_choice: "e3.script_choice.warning",
    dialogue_file: "dialogu3.dat",
    background_story: "e3.background_story",
    intro_cue: td_audio::music_cues::E3_INTRO,
    complete_at: 20,
    menu_song_at_entry: false,
    loader_flushes: true,
    saves: SaveFiles {
        sav: "td3.sav",
        stt: "td3.stt",
    },
    save_labels: "e3.saves.save",
    restore_labels: "e3.saves.restore",
};

impl EpisodeCampaign for CampaignE3 {
    const FRONT: FrontEnd = E3_FRONT;

    fn start(start: BlockStart) -> Self {
        CampaignE3::new(start)
    }

    fn step(
        &mut self,
        prog: &mut Program,
        ctx: &mut FlowContext<'_>,
        input: &TickInput,
    ) -> Option<BlockResult> {
        CampaignE3::step(self, prog, ctx, input)
    }

    fn draw(&self, fb: &mut Framebuffer) {
        CampaignE3::draw(self, fb);
    }

    fn picture(prog: &Program, item: u8) -> Result<StoryScreen, StoryError> {
        let bg = if item == 6 { Bg::Aboutus } else { Bg::Order };
        scenes_e3::scene_card(&prog.data, &prog.assets, bg, ["", "", ""])
    }
}
