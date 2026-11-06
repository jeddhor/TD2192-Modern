//! Top-level game flow: front end, narrative presentation, mission hosting and debriefing.
//!
//! Owns the simulation during missions and drives the renderer and audio. Implemented from
//! `docs/spec/flow/`, `docs/spec/narrative/` and `docs/spec/frontend/`.

pub mod campaign;
pub mod campaign_e2;
pub mod dialogue;
pub mod flight_screens;
pub mod main_menu;
pub mod program;
pub mod program_e2;
pub mod save_screens;
pub mod scenes_e2;
pub mod screen;
pub mod settings;
pub mod story;
pub mod timing;

pub use campaign::{FlightLauncher, FlightResult, FlightScreen, PlaceholderLauncher};
pub use dialogue::DialogueScene;
pub use program::Episode1;
pub use program_e2::Episode2;
pub use screen::{FLIGHT_TICK_HZ, Flow, FlowContext, RETRACE_HZ, Screen, Transition};
pub use settings::Settings;
pub use timing::{DEFAULT_TICK_HZ, FixedStep};
