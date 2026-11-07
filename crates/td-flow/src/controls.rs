//! Control bindings (`[controls]` in `settings.toml`, M11-03): which host key and which
//! game-controller button stand for each key the original reads in flight
//! ([`td_sim::FlightKey`], `docs/re/globals.md` §3).
//!
//! ```toml
//! [controls.keyboard]      # flight key = host key (Key::name), or "none"
//! keypad-8 = "keypad-8"
//! space = "space"
//! esc = "escape"
//!
//! [controls.gamepad]       # flight key = controller button (PadButton::name), or "none"
//! esc = "start"
//! ```
//!
//! `options_button` (a key of `[controls]`, M12-18) is the controller button that opens the
//! options during play, as F2 does on the keyboard; `"none"` turns it off. A button stands for
//! one thing: binding it to a flight key takes it from the options and the other way round.
//!
//! Missing entries keep their defaults, so a partial or older file still works. The keyboard
//! defaults are the original's keys (MDG §9.5). The controller's stick and D-pad stay the
//! original's joystick (thrust; East = button 1, afterburner; South = button 2, fire), which the
//! original menus also read; the gamepad table adds buttons for the other keys.
//!
//! A bound controller button acts as its flight key's keyboard key ([`ControlsSettings::pad_keys`]):
//! the host adds that key to the input while the button is held. The flight then reads it like
//! the key, and every original screen (menus, "hit any key", the flight's modal screens) sees
//! a key it already understands, so a controller alone can reach every screen. A flight key
//! with no keyboard key gets nothing from its button.

use std::collections::BTreeMap;
use std::fmt;

use serde::de::Deserializer;
use serde::ser::{SerializeMap, Serializer};
use serde::{Deserialize, Serialize};
use td_input::{Key, Pad, PadButton};
use td_sim::FlightKey;

/// The settings-file name of an original flight key.
pub fn flight_key_name(k: FlightKey) -> &'static str {
    match k {
        FlightKey::Kp8 => "keypad-8",
        FlightKey::Up => "up",
        FlightKey::Kp2 => "keypad-2",
        FlightKey::Down => "down",
        FlightKey::Kp4 => "keypad-4",
        FlightKey::Left => "left",
        FlightKey::Kp6 => "keypad-6",
        FlightKey::Right => "right",
        FlightKey::KpPlus => "keypad-plus",
        FlightKey::Kp0 => "keypad-0",
        FlightKey::Alt => "alt",
        FlightKey::RightAlt => "right-alt",
        FlightKey::Space => "space",
        FlightKey::Esc => "esc",
        FlightKey::P => "p",
        FlightKey::F5 => "f5",
        FlightKey::F6 => "f6",
        FlightKey::S => "s",
        FlightKey::Tab => "tab",
        FlightKey::M => "m",
    }
}

/// The flight key named `name` (see [`flight_key_name`]).
pub fn flight_key_from_name(name: &str) -> Option<FlightKey> {
    FlightKey::ALL
        .into_iter()
        .find(|&k| flight_key_name(k) == name)
}

/// What a flight key does in flight (`docs/re/globals.md` §3), for the controls screen.
pub fn flight_key_action(k: FlightKey) -> &'static str {
    match k {
        FlightKey::Kp8 | FlightKey::Up => "thrust up",
        FlightKey::Kp2 | FlightKey::Down => "thrust down",
        FlightKey::Kp4 | FlightKey::Left => "thrust left",
        FlightKey::Kp6 | FlightKey::Right => "thrust right",
        FlightKey::KpPlus | FlightKey::Kp0 => "afterburner",
        FlightKey::Alt | FlightKey::RightAlt => "infra-red",
        FlightKey::Space => "fire",
        FlightKey::Esc => "computer menu",
        FlightKey::P => "pause",
        FlightKey::F5 => "tactical map",
        FlightKey::F6 => "transmission",
        FlightKey::S => "sound",
        FlightKey::Tab => "switch weapon",
        FlightKey::M => "repeat message",
    }
}

fn slot(k: FlightKey) -> usize {
    FlightKey::ALL.iter().position(|&f| f == k).unwrap_or(0)
}

/// A value a binding table holds: a host key or a controller button.
pub trait BindingValue: Copy + PartialEq + fmt::Debug {
    /// The name in the settings file.
    fn binding_name(&self) -> String;
    /// The value named `name`.
    fn from_binding_name(name: &str) -> Option<Self>;
}

impl BindingValue for Key {
    fn binding_name(&self) -> String {
        self.name()
    }
    fn from_binding_name(name: &str) -> Option<Self> {
        Key::from_name(name)
    }
}

impl BindingValue for PadButton {
    fn binding_name(&self) -> String {
        self.name().to_string()
    }
    fn from_binding_name(name: &str) -> Option<Self> {
        PadButton::from_name(name)
    }
}

/// One binding (or none) per flight key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bindings<T> {
    slots: [Option<T>; FlightKey::ALL.len()],
}

/// Host key per flight key.
pub type KeyBindings = Bindings<Key>;
/// Controller button per flight key.
pub type PadBindings = Bindings<PadButton>;

impl<T: BindingValue> Bindings<T> {
    /// No bindings at all.
    pub fn none() -> Self {
        Self {
            slots: [None; FlightKey::ALL.len()],
        }
    }

    /// What `k` is bound to.
    pub fn get(&self, k: FlightKey) -> Option<T> {
        self.slots[slot(k)]
    }

    /// The flight key `v` is bound to.
    pub fn flight_key(&self, v: T) -> Option<FlightKey> {
        FlightKey::ALL
            .into_iter()
            .find(|&k| self.slots[slot(k)] == Some(v))
    }

    /// Binds `k` to `v` (or unbinds it). A value stands for one flight key only: if another
    /// key had `v`, it loses it, and that key is returned (conflict detection, M11-03).
    pub fn bind(&mut self, k: FlightKey, v: Option<T>) -> Option<FlightKey> {
        let mut lost = None;
        if let Some(v) = v {
            for other in FlightKey::ALL {
                if other != k && self.slots[slot(other)] == Some(v) {
                    self.slots[slot(other)] = None;
                    lost = Some(other);
                }
            }
        }
        self.slots[slot(k)] = v;
        lost
    }

    /// Every (flight key, value) pair, in flight-key order.
    pub fn iter(&self) -> impl Iterator<Item = (FlightKey, T)> + '_ {
        FlightKey::ALL
            .into_iter()
            .filter_map(|k| self.slots[slot(k)].map(|v| (k, v)))
    }
}

impl Default for KeyBindings {
    /// The original's keys (`docs/re/globals.md` §3). Keypad 8/2/4/6/0 are the keypad keys
    /// (the grey Insert key is a different byte and is not read in flight).
    fn default() -> Self {
        let letter = |c| Key::letter(c).unwrap_or(Key::SPACE);
        let mut b = Self::none();
        for (k, host) in [
            (FlightKey::Kp8, Key(96)),
            (FlightKey::Up, Key::UP),
            (FlightKey::Kp2, Key(90)),
            (FlightKey::Down, Key::DOWN),
            (FlightKey::Kp4, Key(92)),
            (FlightKey::Left, Key::LEFT),
            (FlightKey::Kp6, Key(94)),
            (FlightKey::Right, Key::RIGHT),
            (FlightKey::KpPlus, Key::KP_PLUS),
            (FlightKey::Kp0, Key(98)),
            (FlightKey::Alt, Key::LALT),
            (FlightKey::RightAlt, Key::RALT),
            (FlightKey::Space, Key::SPACE),
            (FlightKey::Esc, Key::ESCAPE),
            (FlightKey::P, letter('p')),
            (FlightKey::F5, Key(62)),
            (FlightKey::F6, Key::F6),
            (FlightKey::S, letter('s')),
            (FlightKey::Tab, Key::TAB),
            (FlightKey::M, letter('m')),
        ] {
            b.slots[slot(k)] = Some(host);
        }
        b
    }
}

impl Default for PadBindings {
    /// Port defaults (the original had a two-button joystick): South fires (and advances "hit
    /// any key" screens), Start opens the computer menu (and acts as Esc everywhere), Back
    /// the tactical map, North switches weapon, West toggles infra-red, the shoulders read and
    /// repeat transmissions, and a press of the right stick pauses (M12-18). The left stick's
    /// press opens the options ([`ControlsSettings::options_button`]); the triggers and Guide
    /// are free (the system often takes Guide).
    fn default() -> Self {
        let mut b = Self::none();
        for (k, button) in [
            (FlightKey::Space, PadButton::South),
            (FlightKey::Esc, PadButton::Start),
            (FlightKey::F5, PadButton::Back),
            (FlightKey::Tab, PadButton::North),
            (FlightKey::Alt, PadButton::West),
            (FlightKey::F6, PadButton::LeftShoulder),
            (FlightKey::M, PadButton::RightShoulder),
            (FlightKey::P, PadButton::RightStick),
        ] {
            b.slots[slot(k)] = Some(button);
        }
        b
    }
}

impl<T: BindingValue> Serialize for Bindings<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(FlightKey::ALL.len()))?;
        for k in FlightKey::ALL {
            let v = self
                .get(k)
                .map_or_else(|| "none".to_string(), |v| v.binding_name());
            map.serialize_entry(flight_key_name(k), &v)?;
        }
        map.end()
    }
}

impl<'de, T: BindingValue> Deserialize<'de> for Bindings<T>
where
    Bindings<T>: Default,
{
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let entries = BTreeMap::<String, String>::deserialize(d)?;
        let mut b = Self::default();
        for (name, value) in entries {
            let Some(k) = flight_key_from_name(&name) else {
                tracing::warn!("settings: unknown flight key {name:?} in [controls]");
                continue;
            };
            if value == "none" {
                b.slots[slot(k)] = None;
            } else if let Some(v) = T::from_binding_name(&value) {
                b.slots[slot(k)] = Some(v);
            } else {
                tracing::warn!("settings: unknown binding {value:?} for {name:?} in [controls]");
            }
        }
        Ok(b)
    }
}

/// The controller button that opens the options by default: a press of the left stick.
pub const OPTIONS_BUTTON: PadButton = PadButton::LeftStick;

/// The `[controls]` section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ControlsSettings {
    /// The controller button that opens the options during play (the host's F2, M12-18), or
    /// none.
    #[serde(with = "button_or_none")]
    pub options_button: Option<PadButton>,
    pub keyboard: KeyBindings,
    pub gamepad: PadBindings,
}

impl Default for ControlsSettings {
    fn default() -> Self {
        Self {
            options_button: Some(OPTIONS_BUTTON),
            keyboard: KeyBindings::default(),
            gamepad: PadBindings::default(),
        }
    }
}

/// A controller button or `"none"` in the settings file.
mod button_or_none {
    use serde::{Deserialize, Deserializer, Serializer};
    use td_input::PadButton;

    pub fn serialize<S: Serializer>(b: &Option<PadButton>, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(b.map_or("none", PadButton::name))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<PadButton>, D::Error> {
        let name = String::deserialize(d)?;
        if name == "none" {
            return Ok(None);
        }
        Ok(Some(PadButton::from_name(&name).unwrap_or_else(|| {
            tracing::warn!("settings: unknown button {name:?} for the options in [controls]");
            super::OPTIONS_BUTTON
        })))
    }
}

/// A press of any controller button, for the waits that end on "any key" (M12-18,
/// port-front-end rev 5 §5): the story screens' holds, the dialogue's page breaks and the
/// flight's pause. The original reads joystick buttons 1 and 2 there; the port takes every
/// button, bound or not. A press counts in the tick it happens (it is not kept for a later
/// wait), and a button already held when the screen starts does not count.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AnyButton {
    prev: Option<u32>,
}

impl AnyButton {
    /// True if a button went down since the previous call.
    pub fn pressed(&mut self, pad: Option<Pad>) -> bool {
        let now = pad.map_or(0, |p| p.buttons);
        let prev = self.prev.replace(now).unwrap_or(now);
        now & !prev != 0
    }
}

/// The keys a controller's bound buttons stand for at one poll.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PadKeys {
    /// Keys whose button is held.
    pub held: Vec<Key>,
    /// Keys whose button went down since the previous poll.
    pub pressed: Vec<Key>,
}

impl ControlsSettings {
    /// (host key, flight key) pairs for the flight.
    pub fn flight_keys(&self) -> Vec<(Key, FlightKey)> {
        self.keyboard.iter().map(|(k, host)| (host, k)).collect()
    }

    /// True if the options button went down: it is held in `pad` and was not in `prev`
    /// ([`Pad::buttons`] at the previous poll).
    pub fn options_pressed(&self, pad: Option<Pad>, prev: u32) -> bool {
        match (self.options_button, pad) {
            (Some(b), Some(p)) => p.held(b) && prev & b.bit() == 0,
            _ => false,
        }
    }

    /// Binds the options to `b` (or none); a flight key that had `b` loses it and is returned.
    pub fn bind_options(&mut self, b: Option<PadButton>) -> Option<FlightKey> {
        let lost = b.and_then(|b| self.gamepad.flight_key(b));
        if let Some(k) = lost {
            self.gamepad.bind(k, None);
        }
        self.options_button = b;
        lost
    }

    /// Binds flight key `k` to button `b` (or none), taking `b` from another flight key or the
    /// options; returns the flight key that lost it and whether the options did.
    pub fn bind_pad(&mut self, k: FlightKey, b: Option<PadButton>) -> (Option<FlightKey>, bool) {
        let options = b.is_some() && self.options_button == b;
        if options {
            self.options_button = None;
        }
        (self.gamepad.bind(k, b), options)
    }

    /// The keyboard keys `pad`'s bound buttons stand for; `prev` = [`Pad::buttons`] at the
    /// previous poll (for the presses).
    pub fn pad_keys(&self, pad: Option<Pad>, prev: u32) -> PadKeys {
        let mut out = PadKeys::default();
        let Some(pad) = pad else { return out };
        for (k, button) in self.gamepad.iter() {
            let Some(host) = self.keyboard.get(k) else {
                continue;
            };
            if pad.held(button) {
                out.held.push(host);
                if prev & button.bit() == 0 {
                    out.pressed.push(host);
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_unique_and_drawable() {
        for k in FlightKey::ALL {
            assert_eq!(flight_key_from_name(flight_key_name(k)), Some(k));
            for s in [flight_key_name(k), flight_key_action(k)] {
                assert!(
                    s.chars().all(|c| c.is_ascii_lowercase()
                        || c.is_ascii_digit()
                        || c == '-'
                        || c == ' '),
                    "{s}"
                );
            }
        }
    }

    #[test]
    fn defaults_are_the_original_keys() {
        let c = ControlsSettings::default();
        assert_eq!(c.keyboard.get(FlightKey::Kp8), Some(Key(96)));
        assert_eq!(c.keyboard.get(FlightKey::F5), Some(Key(62)));
        assert_eq!(c.keyboard.get(FlightKey::Tab), Some(Key::TAB));
        assert_eq!(c.keyboard.get(FlightKey::M), Key::letter('m'));
        assert_eq!(c.flight_keys().len(), FlightKey::ALL.len());
        // Every host key stands for one flight key.
        let mut hosts: Vec<Key> = c.flight_keys().iter().map(|&(h, _)| h).collect();
        hosts.sort();
        hosts.dedup();
        assert_eq!(hosts.len(), FlightKey::ALL.len());
        assert_eq!(c.gamepad.get(FlightKey::Esc), Some(PadButton::Start));
        // M12-18: pause on the right stick, the options on the left; no button twice.
        assert_eq!(c.gamepad.get(FlightKey::P), Some(PadButton::RightStick));
        assert_eq!(c.options_button, Some(PadButton::LeftStick));
        let mut buttons: Vec<PadButton> = c.gamepad.iter().map(|(_, b)| b).collect();
        buttons.extend(c.options_button);
        let n = buttons.len();
        buttons.sort();
        buttons.dedup();
        assert_eq!(buttons.len(), n);
        // Fire and afterburner stay the joystick's buttons 2 and 1 (South, East).
        assert!(!buttons.contains(&PadButton::East));
    }

    /// M12-18: the options button, its press, the file and the conflicts with flight keys.
    #[test]
    fn the_options_button() {
        let mut c = ControlsSettings::default();
        let pad = |b: PadButton| {
            Some(Pad {
                buttons: b.bit(),
                ..Pad::default()
            })
        };
        let l3 = PadButton::LeftStick;
        assert!(c.options_pressed(pad(l3), 0));
        assert!(!c.options_pressed(pad(l3), l3.bit()), "held, not pressed");
        assert!(!c.options_pressed(pad(PadButton::South), 0));
        assert!(!c.options_pressed(None, 0));
        // Binding a flight key to it takes it from the options, and back.
        assert_eq!(c.bind_pad(FlightKey::Tab, Some(l3)), (None, true));
        assert_eq!(c.options_button, None);
        assert_eq!(c.gamepad.get(FlightKey::Tab), Some(l3));
        assert_eq!(c.bind_options(Some(l3)), Some(FlightKey::Tab));
        assert_eq!(c.gamepad.get(FlightKey::Tab), None);
        // The file: a choice, "none", and an unknown name (the default).
        let mut d = ControlsSettings::default();
        d.bind_options(Some(PadButton::Guide));
        let text = toml::to_string_pretty(&d).unwrap();
        assert!(text.contains("options_button = \"guide\""), "{text}");
        assert_eq!(toml::from_str::<ControlsSettings>(&text).unwrap(), d);
        let none: ControlsSettings = toml::from_str("options_button = \"none\"\n").unwrap();
        assert_eq!(none.options_button, None);
        let odd: ControlsSettings = toml::from_str("options_button = \"nope\"\n").unwrap();
        assert_eq!(odd.options_button, Some(OPTIONS_BUTTON));
        let old: ControlsSettings = toml::from_str("[keyboard]\n").unwrap();
        assert_eq!(
            old.options_button,
            Some(OPTIONS_BUTTON),
            "older files get the default"
        );
    }

    #[test]
    fn binding_a_taken_key_moves_it() {
        let mut b = KeyBindings::default();
        assert_eq!(
            b.bind(FlightKey::P, Some(Key::SPACE)),
            Some(FlightKey::Space)
        );
        assert_eq!(b.get(FlightKey::P), Some(Key::SPACE));
        assert_eq!(b.get(FlightKey::Space), None);
        assert_eq!(b.flight_key(Key::SPACE), Some(FlightKey::P));
        assert_eq!(b.bind(FlightKey::P, None), None);
        assert_eq!(b.flight_key(Key::SPACE), None);
        let mut p = PadBindings::default();
        assert_eq!(
            p.bind(FlightKey::P, Some(PadButton::Start)),
            Some(FlightKey::Esc)
        );
    }

    #[test]
    fn round_trips_and_tolerates_partial_tables() {
        let mut c = ControlsSettings::default();
        c.keyboard.bind(FlightKey::Space, Some(Key::LCTRL));
        c.keyboard.bind(FlightKey::S, None);
        c.gamepad.bind(FlightKey::P, Some(PadButton::RightTrigger));
        let text = toml::to_string_pretty(&c).unwrap();
        assert!(text.contains("space = \"left-ctrl\""), "{text}");
        assert!(text.contains("s = \"none\""), "{text}");
        assert_eq!(toml::from_str::<ControlsSettings>(&text).unwrap(), c);

        let partial: ControlsSettings = toml::from_str(
            "[keyboard]\nspace = \"left-ctrl\"\nbogus = \"a\"\ntab = \"not-a-key\"\n",
        )
        .unwrap();
        assert_eq!(partial.keyboard.get(FlightKey::Space), Some(Key::LCTRL));
        assert_eq!(partial.keyboard.get(FlightKey::Tab), Some(Key::TAB));
        assert_eq!(partial.keyboard.get(FlightKey::Up), Some(Key::UP));
        assert_eq!(partial.gamepad, PadBindings::default());
    }

    #[test]
    fn controller_buttons_stand_for_their_keys() {
        let c = ControlsSettings::default();
        let pad = |buttons| {
            Some(Pad {
                buttons,
                ..Pad::default()
            })
        };
        let start = PadButton::Start.bit();
        let first = c.pad_keys(pad(start), 0);
        assert_eq!(first.held, [Key::ESCAPE]);
        assert_eq!(first.pressed, [Key::ESCAPE]);
        let held = c.pad_keys(pad(start), start);
        assert_eq!(held.held, [Key::ESCAPE]);
        assert!(held.pressed.is_empty());
        // East is the original joystick's button 1: no key.
        assert_eq!(
            c.pad_keys(pad(PadButton::East.bit()), 0),
            PadKeys::default()
        );
        assert_eq!(c.pad_keys(None, start), PadKeys::default());
        // A flight key without a keyboard key gets nothing from its button.
        let mut c = c;
        c.keyboard.bind(FlightKey::Esc, None);
        assert_eq!(c.pad_keys(pad(start), 0), PadKeys::default());
    }
}
