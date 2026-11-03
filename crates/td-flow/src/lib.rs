//! Top-level game flow: front end, narrative presentation, mission hosting and debriefing.
//!
//! Owns the simulation during missions and drives the renderer and audio. Implemented from
//! `docs/spec/flow/`, `docs/spec/narrative/` and `docs/spec/frontend/`.

pub mod dialogue;
pub mod screen;
pub mod settings;
pub mod timing;

pub use dialogue::DialogueScene;
pub use screen::{FLIGHT_TICK_HZ, Flow, FlowContext, RETRACE_HZ, Screen, Transition};
pub use settings::Settings;
pub use timing::{DEFAULT_TICK_HZ, FixedStep};
