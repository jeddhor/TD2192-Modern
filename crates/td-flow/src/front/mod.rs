//! The port's own front end (M11): the start screen with the episode choice (M11-01), the
//! settings screens (M11-03) and their keyboard and controller navigation (M11-04).
//!
//! These screens are not in the original; they are drawn in its style (see [`ui`]). The
//! original's own menus (`main_menu`, `save_screens`, the flight's computer menu) stay as
//! specified and already read the joystick.

pub mod options;
pub mod start;
pub mod ui;

use std::cell::RefCell;
use std::rc::Rc;

use crate::settings::Settings;

pub use options::OptionsScreen;
pub use start::{EpisodeLauncher, StartScreen};
pub use ui::UiAssets;

/// Where the options screens hand settings to the host, which saves and applies them after
/// the tick (M11-03: changes apply live).
#[derive(Debug, Clone, Default)]
pub struct SettingsPort(Rc<RefCell<PortState>>);

#[derive(Debug, Default)]
struct PortState {
    pending: Option<Settings>,
    open: bool,
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
}
