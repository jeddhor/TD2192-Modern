//! Episode 1 flights as the oracle captures start them: `FILE0001.EXE` started directly, so the
//! flight is the first of its program run (`Session::new()`, mission-ships §4.1), with the
//! world built from the installation (map, tile attributes, the muzzle table of the
//! executable and the direct start's two-digit-heading bytes, weapons rev 4 §3.3). Shared by
//! `tdtool plan` and `tdtool replay`.

use std::path::Path;

use anyhow::{Context, Result, bail};
use td_assets::{DataDir, ExeResources, ResourceValue};
use td_formats::map::TileMap;
use td_formats::mission::Mission;
use td_sim::flight::front_end::{FrontEnd, muzzle_world};
use td_sim::flight::{Episode, Session, episode};
use td_sim::{Flight, World};

/// What a flight needs from the installation.
#[derive(Debug, Clone)]
pub struct E1Flight {
    pub number: u8,
    pub world: World,
    pub mission: Option<Mission>,
}

impl E1Flight {
    /// Loads flight `number` (1..=20) of Episode 1.
    pub fn load(root: &Path, number: u8) -> Result<Self> {
        if !(1..=20).contains(&number) {
            bail!("Episode 1 flight number must be 1..=20");
        }
        let dir = DataDir::open(root)?;
        let res = ExeResources::load(&dir).context("file0001.exe resources")?;
        let Some(ResourceValue::Bytes(muzzles)) = res.get("flight.muzzle_offsets").cloned() else {
            bail!("file0001.exe resources: no muzzle table");
        };
        // A direct start's bytes past the table (weapons rev 4 §3.3, OB-017 preserved).
        let menu = dir.read("tdmen.pal")?;
        let muzzles = muzzle_world(&muzzles, &menu, &FrontEnd::direct_start());
        let map = TileMap::parse_episode(&dir.read(episode::map_file(Episode::E1, number))?)?;
        let world = episode::world(Episode::E1, number, map, muzzles);
        let mission = match episode::mission_file(Episode::E1, number) {
            Some(file) => Some(Mission::parse(&dir.read(&file)?)?),
            None => None,
        };
        Ok(Self {
            number,
            world,
            mission,
        })
    }

    /// The same flight with other muzzle bytes: the original reads DGROUP from DS:035C on for
    /// volleys on two-digit headings (U-160); the default world holds a direct start's bytes
    /// (weapons rev 4 §3.3). `bytes` come from a capture (game-derived data: kept outside the
    /// repository), for example one started through `td1.exe` or after a played story.
    pub fn with_muzzles(&self, bytes: Vec<u8>) -> Self {
        let mut alt = self.clone();
        alt.world.muzzles = td_sim::flight::shots::Muzzles(bytes);
        alt
    }

    /// The flight after setup, as the first flight of a program run.
    pub fn start(&self) -> Flight {
        Flight::start(
            self.number,
            self.mission.as_ref(),
            &self.world,
            Session::new(),
        )
    }
}
