//! Physical/virtual keyboard history shared across screens (flight-screens rev 8 section 4.4).
use crate::Key;
use std::collections::BTreeSet;

/// The newest press identity and its integer age, independent of any game's key table.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KeyboardHistory {
    pub last: Option<Key>,
    /// Changes on every actual press; OS repeat events never restart this clock.
    pub serial: u64,
    pub elapsed_us: u64,
    /// The newest key remains held. Releasing it never selects an older held key.
    pub eligible: bool,
}

/// Run-wide input clock. The platform host supplies elapsed microseconds and actual ordered
/// press edges, including controller buttons mapped to keys. No repeats enter UI presses.
#[derive(Debug, Default)]
pub struct KeyboardRepeat {
    history: KeyboardHistory,
}

impl KeyboardRepeat {
    /// Observe one host poll. Advance before applying new edges, so a press starts at age 0.
    pub fn update(
        &mut self,
        held: &BTreeSet<Key>,
        pressed: &[Key],
        elapsed_us: u64,
    ) -> KeyboardHistory {
        if self.history.eligible {
            self.history.elapsed_us = self.history.elapsed_us.saturating_add(elapsed_us);
        }
        for &key in pressed {
            self.history.last = Some(key);
            self.history.serial = self.history.serial.wrapping_add(1);
            self.history.elapsed_us = 0;
        }
        self.history.eligible = self.history.last.is_some_and(|key| held.contains(&key));
        self.history
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn newest_key_release_never_falls_back_or_resets_at_a_screen_boundary() {
        let mut clock = KeyboardRepeat::default();
        let mut held = BTreeSet::from([Key::UP]);
        let a = clock.update(&held, &[Key::UP], 0);
        assert_eq!(a.elapsed_us, 0);
        held.insert(Key::ESCAPE);
        let b = clock.update(&held, &[Key::ESCAPE], 600_000);
        assert_eq!(b.last, Some(Key::ESCAPE));
        assert_ne!(a.serial, b.serial);
        held.remove(&Key::ESCAPE);
        let released = clock.update(&held, &[], 50_000);
        assert!(!released.eligible);
        assert_eq!(released.last, Some(Key::ESCAPE));
        let flight = clock.update(&held, &[], 900_000);
        assert_eq!(
            flight, released,
            "screen changes cannot revive the older key"
        );
        let fresh = clock.update(&held, &[Key::UP], 0);
        assert!(fresh.eligible && fresh.last == Some(Key::UP));
        let aged = clock.update(&held, &[], 1_133_000);
        assert_eq!(aged.elapsed_us, 1_133_000);
        assert_eq!(aged.serial, fresh.serial);
    }
}
