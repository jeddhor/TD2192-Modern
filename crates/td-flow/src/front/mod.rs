//! The port's own front end (M11): the start screen with the episode choice (M11-01), the
//! settings screens (M11-03) and their keyboard and controller navigation (M11-04), and the
//! game-data page that the first run also shows (M13-02).
//!
//! These screens are not in the original; they are drawn in its style (see [`ui`]). The
//! original's own menus (`main_menu`, `save_screens`, the flight's computer menu) stay as
//! specified and already read the joystick.

pub mod data;
pub mod options;
pub mod start;
pub mod ui;

use std::cell::RefCell;
use std::rc::Rc;

use crate::settings::Settings;

pub use data::{DataInfo, DataRequest, DataSource, DataSummary};
pub use options::OptionsScreen;
pub use start::{EpisodeLauncher, StartScreen};
pub use ui::UiAssets;

/// Where the options screens hand settings to the host, which saves and applies them after
/// the tick (M11-03: changes apply live), and where the game-data page and the host talk
/// ([`data`]).
#[derive(Debug, Clone, Default)]
pub struct SettingsPort(Rc<RefCell<PortState>>);

#[derive(Debug, Default)]
struct PortState {
    pending: Option<Settings>,
    open: bool,
    data: Option<DataInfo>,
    data_request: Option<DataRequest>,
    data_switch: bool,
    data_generation: u32,
}

impl SettingsPort {
    /// A port with nothing pending.
    pub fn new() -> Self {
        Self::default()
    }

    /// The settings the options screen changed since the last call, if any.
    pub fn take(&self) -> Option<Settings> {
        self.0.borrow_mut().pending.take()
    }

    /// True while an options screen is shown (the host does not open a second one).
    pub fn is_open(&self) -> bool {
        self.0.borrow().open
    }

    fn submit(&self, s: Settings) {
        self.0.borrow_mut().pending = Some(s);
    }

    fn set_open(&self, open: bool) {
        self.0.borrow_mut().open = open;
    }

    /// The game data as the host last described it (`None` until it does).
    pub fn data_info(&self) -> Option<DataInfo> {
        self.0.borrow().data.clone()
    }

    /// Host: describes the game data for the game-data page.
    pub fn set_data_info(&self, info: DataInfo) {
        self.0.borrow_mut().data = Some(info);
    }

    /// The game-data page asks the host for `request` (after the tick).
    pub fn request_data(&self, request: DataRequest) {
        self.0.borrow_mut().data_request = Some(request);
    }

    /// Host: the request the game-data page made, if any.
    pub fn take_data_request(&self) -> Option<DataRequest> {
        self.0.borrow_mut().data_request.take()
    }

    /// The start screen, on top: a new choice of game data may be used now.
    pub fn want_data_switch(&self) {
        self.0.borrow_mut().data_switch = true;
    }

    /// Host: true once if the start screen asked to switch to the new game data.
    pub fn take_data_switch(&self) -> bool {
        std::mem::take(&mut self.0.borrow_mut().data_switch)
    }

    /// Host: the game data was switched; the screens' `FlowContext::data` is the new data
    /// from the next tick on.
    pub fn data_switched(&self) {
        let mut s = self.0.borrow_mut();
        s.data_generation = s.data_generation.wrapping_add(1);
    }

    /// Counts the switches ([`Self::data_switched`]), so a screen can tell its data changed.
    pub fn data_generation(&self) -> u32 {
        self.0.borrow().data_generation
    }

    /// True if a new choice of game data waits for the start screen.
    pub fn data_pending(&self) -> bool {
        self.0
            .borrow()
            .data
            .as_ref()
            .is_some_and(|d| d.pending.is_some())
    }
}
