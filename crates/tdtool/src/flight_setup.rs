//! Flights as the oracle captures start them: the episode's program started directly (E1
//! `FILE0001.EXE` with a prepared `tdstatus.dat`; E2 `TD2.EXE /sb` and E3 `TD3.EXE /sb` through
//! New game or a prepared save slot, e2-program-flow §8, e3-program-flow §8), so the flight is
//! the first of its program run (`Session::for_episode`, mission-ships §4.1), with the world
//! built from the installation (map, tile attributes, the muzzle table of the executable;
//! E1 adds a direct start's two-digit-heading bytes, weapons rev 4 §3.3, OB-017).
//! Shared by `tdtool plan` and `tdtool replay` (VE-2 M9-21, VE-4 M10-45).

use std::path::Path;

use anyhow::{Context, Result, bail};
use td_assets::{DataDir, ExeResources, ResourceValue};
use td_formats::map::TileMap;
use td_formats::mission::Mission;
use td_sim::flight::front_end::{FrontEnd, muzzle_world};
use td_sim::flight::{Episode, Session, episode};
use td_sim::{Flight, World};

/// The episode of a number 1..=3.
pub fn episode_of(n: u8) -> Result<Episode> {
    Ok(match n {
        1 => Episode::E1,
        2 => Episode::E2,
        3 => Episode::E3,
        _ => bail!("episode must be 1, 2 or 3 (got {n})"),
    })
}

/// The number 1..=3 of an episode.
pub fn episode_number(ep: Episode) -> u8 {
    match ep {
        Episode::E1 => 1,
        Episode::E2 => 2,
        Episode::E3 => 3,
    }
}

/// What a flight needs from the installation.
#[derive(Debug, Clone)]
pub struct FlightSetup {
    pub episode: Episode,
    pub number: u8,
    pub world: World,
    pub mission: Option<Mission>,
}

impl FlightSetup {
    /// Loads flight `number` of `episode` (E1 and E2 1..=20, E3 1..=21 without 20).
    pub fn load(root: &Path, episode: Episode, number: u8) -> Result<Self> {
        let ok = match episode {
            Episode::E1 | Episode::E2 => (1..=20).contains(&number),
            Episode::E3 => (1..=21).contains(&number) && number != 20,
        };
        if !ok {
            bail!("{episode} has no flight {number} to plan");
        }
        let dir = DataDir::open(root)?;
        let res = ExeResources::load(&dir).context("executable resources")?;
        // The muzzle table is byte-identical in every episode (episode-deltas §11).
        let Some(ResourceValue::Bytes(muzzles)) = res.get("flight.muzzle_offsets").cloned() else {
            bail!("file0001.exe resources: no muzzle table");
        };
        // E1: a direct start's bytes past the table (weapons rev 4 §3.3, OB-017 preserved);
        // E2 and E3 keep (0, 0) past it (U-441).
        let muzzles = if episode == Episode::E1 {
            let menu = dir.read("tdmen.pal")?;
            muzzle_world(&muzzles, &menu, &FrontEnd::direct_start())
        } else {
            muzzles
        };
        let map = TileMap::parse_episode(&dir.read(episode::map_file(episode, number))?)?;
        let world = episode::world(episode, number, map, muzzles);
        let mission = match episode::mission_file(episode, number) {
            Some(file) => Some(Mission::parse(&dir.read(&file)?)?),
            None => None,
        };
        Ok(Self {
            episode,
            number,
            world,
            mission,
        })
    }

    /// The replay file stem: `e<E>f<NN>`.
    pub fn stem(&self) -> String {
        format!("e{}f{:02}", episode_number(self.episode), self.number)
    }

    /// The same flight with other muzzle bytes: the original reads DGROUP from the table's
    /// start on for volleys on two-digit headings (U-160); the default E1 world holds a direct
    /// start's bytes (weapons rev 4 §3.3). `bytes` come from a capture (game-derived data:
    /// kept outside the repository), for example one started through `td1.exe` or after a
    /// played story.
    pub fn with_muzzles(&self, bytes: Vec<u8>) -> Self {
        let mut alt = self.clone();
        alt.world.muzzles = td_sim::flight::shots::Muzzles(bytes);
        alt
    }

    /// The program-start session of the episode.
    pub fn session(&self) -> Session {
        Session::for_episode(self.episode)
    }

    /// The flight after setup, as the first flight of a program run.
    pub fn start(&self) -> Flight {
        Flight::start(
            self.number,
            self.mission.as_ref(),
            &self.world,
            self.session(),
        )
    }
}
