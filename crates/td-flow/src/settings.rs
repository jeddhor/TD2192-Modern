//! User settings, stored as `settings.toml` in the per-user data directory (MDG §8.8).
//!
//! Every field has a default so that older or hand-edited files keep working; unknown keys are
//! ignored.
//!
//! The `[enhanced]` section (`docs/spec/enhanced/presentation.md` rev 1 §2) holds the Enhanced
//! presentation features. Every key defaults to off, and they only take effect with
//! `profile = "enhanced"`: the Classic profile (the default, MDG §6.2) forces them all off
//! ([`Settings::effective_enhanced`]). For example:
//!
//! ```toml
//! profile = "enhanced"
//!
//! [enhanced]
//! widescreen = "16:9"   # off | "16:10" | "16:9" | "21:9" | "auto"
//! upscaler = "xbr3"     # none | scale2x | scale3x | scale4x | xbr2 | xbr3 | xbr4
//! lighting = "off"      # off | subtle | full
//! bloom = false
//! ```

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use crate::controls::ControlsSettings;

/// Errors loading or saving settings.
#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("cannot read or write {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("invalid settings file {path}: {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("cannot serialise settings: {0}")]
    Serialize(#[from] toml::ser::Error),
}

/// How the 320x200 image is scaled to the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Scale {
    #[default]
    Aspect,
    Integer,
    Stretch,
}

/// Sound-effect mixing policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum SfxMode {
    /// One effect at a time, like the original hardware (Classic).
    #[default]
    Classic,
    /// Several effects at once (Enhanced).
    Polyphonic,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VideoSettings {
    pub scale: Scale,
    pub fullscreen: bool,
}

impl Default for VideoSettings {
    fn default() -> Self {
        Self {
            scale: Scale::Aspect,
            fullscreen: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioSettings {
    pub music_volume: f32,
    pub sfx_volume: f32,
    pub sfx_mode: SfxMode,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            music_volume: 0.6,
            sfx_volume: 0.8,
            sfx_mode: SfxMode::Classic,
        }
    }
}

/// The fidelity profile (MDG §6.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Profile {
    /// The original presentation: every `[enhanced]` key is forced off.
    #[default]
    Classic,
    /// Enhanced features as chosen per key.
    Enhanced,
}

/// Widescreen width preset (spec enhanced/presentation §4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Widescreen {
    /// 320 wide (Classic).
    #[default]
    #[serde(rename = "off")]
    Off,
    /// 384 wide.
    #[serde(rename = "16:10")]
    W16x10,
    /// 428 wide.
    #[serde(rename = "16:9")]
    W16x9,
    /// 560 wide.
    #[serde(rename = "21:9")]
    W21x9,
    /// The widest preset that fits the window's aspect.
    #[serde(rename = "auto")]
    Auto,
}

impl Widescreen {
    /// Every preset, in the order a menu or hotkey cycles them.
    pub const ALL: [Widescreen; 5] = [
        Widescreen::Off,
        Widescreen::W16x10,
        Widescreen::W16x9,
        Widescreen::W21x9,
        Widescreen::Auto,
    ];

    /// The base width W of a fixed preset (`None` for `auto`).
    pub fn width(self) -> Option<usize> {
        match self {
            Widescreen::Off => Some(320),
            Widescreen::W16x10 => Some(384),
            Widescreen::W16x9 => Some(428),
            Widescreen::W21x9 => Some(560),
            Widescreen::Auto => None,
        }
    }
}

/// Pixel-art upscaling filter (spec enhanced/presentation §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Upscaler {
    #[default]
    None,
    Scale2x,
    Scale3x,
    Scale4x,
    Xbr2,
    Xbr3,
    Xbr4,
}

impl Upscaler {
    /// Every filter, in the order a menu or hotkey cycles them.
    pub const ALL: [Upscaler; 7] = [
        Upscaler::None,
        Upscaler::Scale2x,
        Upscaler::Scale3x,
        Upscaler::Scale4x,
        Upscaler::Xbr2,
        Upscaler::Xbr3,
        Upscaler::Xbr4,
    ];
}

/// Dynamic lighting strength (spec enhanced/presentation §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Lighting {
    #[default]
    Off,
    Subtle,
    Full,
}

impl Lighting {
    /// Every level, in the order a menu or hotkey cycles them.
    pub const ALL: [Lighting; 3] = [Lighting::Off, Lighting::Subtle, Lighting::Full];
}

/// The Enhanced presentation features (`[enhanced]`, spec enhanced/presentation §2). Widescreen
/// affects gameplay (the player sees further).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct EnhancedSettings {
    pub widescreen: Widescreen,
    pub upscaler: Upscaler,
    pub lighting: Lighting,
    pub bloom: bool,
}

impl EnhancedSettings {
    /// True when every feature is off (the Classic presentation path).
    pub fn is_off(&self) -> bool {
        *self == Self::default()
    }
}

/// Accessibility options (`[accessibility]`, MDG §11). Each defaults to off. Like
/// `audio.sfx_mode`, they are separate from the profile, which governs only `[enhanced]`
/// (spec enhanced/presentation rev 2 §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AccessibilitySettings {
    /// Colour-blind-safe colours for the guidance line (MDG §6.2, M12-04): drawn at the RGBA
    /// stage in a colour pair per marker (spec frontend/port-front-end rev 2 §3.3).
    pub colorblind_guidance: bool,
}

/// Developer overrides (ignored unless set).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DevSettings {
    /// Overrides the simulation tick rate (Hz).
    pub tick_hz: Option<f64>,
}

/// All user settings.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// The original game directory.
    pub data_dir: Option<PathBuf>,
    /// Classic forces every Enhanced feature off.
    pub profile: Profile,
    pub video: VideoSettings,
    pub audio: AudioSettings,
    /// Enhanced presentation (spec enhanced/presentation §2).
    pub enhanced: EnhancedSettings,
    /// Host keys and controller buttons for the original's flight keys (M11-03).
    pub controls: ControlsSettings,
    pub accessibility: AccessibilitySettings,
    pub dev: DevSettings,
}

impl Settings {
    /// The `[enhanced]` features in effect: all off under the Classic profile.
    pub fn effective_enhanced(&self) -> EnhancedSettings {
        match self.profile {
            Profile::Classic => EnhancedSettings::default(),
            Profile::Enhanced => self.enhanced,
        }
    }

    /// `self` (the file's settings) with every value that differs between `before` and
    /// `after` taken from `after`. The options screen edits the session's settings, which may
    /// hold command-line overrides (`--fullscreen`, `--dev-enhanced`); only what the player
    /// changed goes to the file.
    pub fn with_changes(&self, before: &Settings, after: &Settings) -> Settings {
        fn merge(file: &mut toml::Value, before: Option<&toml::Value>, after: &toml::Value) {
            match (file, after) {
                (toml::Value::Table(f), toml::Value::Table(a)) => {
                    for (k, av) in a {
                        let bv = before.and_then(|b| b.get(k));
                        match f.get_mut(k) {
                            Some(fv) if av.is_table() => merge(fv, bv, av),
                            Some(fv) => {
                                if bv != Some(av) {
                                    *fv = av.clone();
                                }
                            }
                            None => {
                                if bv != Some(av) {
                                    f.insert(k.clone(), av.clone());
                                }
                            }
                        }
                    }
                    // Keys `after` dropped (an option set to none).
                    let gone: Vec<String> = f
                        .keys()
                        .filter(|k| {
                            !a.contains_key(*k) && before.is_some_and(|b| b.get(*k).is_some())
                        })
                        .cloned()
                        .collect();
                    for k in gone {
                        f.remove(&k);
                    }
                }
                (f, a) => {
                    if before != Some(a) {
                        *f = a.clone();
                    }
                }
            }
        }
        let value = |s: &Settings| toml::Value::try_from(s).ok();
        match (value(self), value(before), value(after)) {
            (Some(mut f), Some(b), Some(a)) => {
                merge(&mut f, Some(&b), &a);
                f.try_into().unwrap_or_else(|_| after.clone())
            }
            _ => after.clone(),
        }
    }

    /// Loads settings from `path`; a missing file yields the defaults.
    pub fn load(path: &Path) -> Result<Self, SettingsError> {
        match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).map_err(|source| SettingsError::Parse {
                path: path.to_path_buf(),
                source,
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(source) => Err(SettingsError::Io {
                path: path.to_path_buf(),
                source,
            }),
        }
    }

    /// Writes settings to `path`.
    pub fn save(&self, path: &Path) -> Result<(), SettingsError> {
        let text = toml::to_string_pretty(self)?;
        std::fs::write(path, text).map_err(|source| SettingsError::Io {
            path: path.to_path_buf(),
            source,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_tolerates_partial_files() {
        let s = Settings {
            data_dir: Some(PathBuf::from("C:/games/td")),
            video: VideoSettings {
                scale: Scale::Integer,
                fullscreen: true,
            },
            ..Settings::default()
        };
        let text = toml::to_string_pretty(&s).unwrap();
        assert_eq!(toml::from_str::<Settings>(&text).unwrap(), s);

        let partial: Settings = toml::from_str("[audio]\nsfx_mode = \"polyphonic\"\n").unwrap();
        assert_eq!(partial.audio.sfx_mode, SfxMode::Polyphonic);
        assert_eq!(partial.audio.music_volume, 0.6);
        assert_eq!(partial.video, VideoSettings::default());
    }

    #[test]
    fn enhanced_section_round_trips_and_classic_forces_it_off() {
        let s = Settings {
            profile: Profile::Enhanced,
            enhanced: EnhancedSettings {
                widescreen: Widescreen::W16x9,
                upscaler: Upscaler::Xbr3,
                lighting: Lighting::Full,
                bloom: true,
            },
            ..Settings::default()
        };
        let text = toml::to_string_pretty(&s).unwrap();
        assert!(text.contains("widescreen = \"16:9\""), "{text}");
        assert!(text.contains("upscaler = \"xbr3\""), "{text}");
        assert_eq!(toml::from_str::<Settings>(&text).unwrap(), s);
        assert_eq!(s.effective_enhanced(), s.enhanced);
        let classic = Settings {
            profile: Profile::Classic,
            ..s.clone()
        };
        assert!(classic.effective_enhanced().is_off());

        let parsed: Settings = toml::from_str(
            "profile = \"enhanced\"\n[enhanced]\nwidescreen = \"auto\"\nupscaler = \"scale4x\"\n",
        )
        .unwrap();
        assert_eq!(parsed.enhanced.widescreen, Widescreen::Auto);
        assert_eq!(parsed.enhanced.upscaler, Upscaler::Scale4x);
        assert_eq!(parsed.enhanced.lighting, Lighting::Off);
        assert!(!parsed.enhanced.bloom);
        // Every key defaults to off, and so does the profile.
        let default = Settings::default();
        assert_eq!(default.profile, Profile::Classic);
        assert!(default.enhanced.is_off());
        for w in ["off", "16:10", "16:9", "21:9", "auto"] {
            let t = format!("[enhanced]\nwidescreen = \"{w}\"\n");
            assert!(toml::from_str::<Settings>(&t).is_ok(), "{w}");
        }
    }

    #[test]
    fn every_section_round_trips_through_a_file() {
        use td_input::{Key, PadButton};
        use td_sim::FlightKey;
        let mut s = Settings {
            data_dir: Some(PathBuf::from("C:/games/td")),
            profile: Profile::Enhanced,
            video: VideoSettings {
                scale: Scale::Stretch,
                fullscreen: true,
            },
            audio: AudioSettings {
                music_volume: 0.25,
                sfx_volume: 1.0,
                sfx_mode: SfxMode::Polyphonic,
            },
            enhanced: EnhancedSettings {
                widescreen: Widescreen::Auto,
                upscaler: Upscaler::Scale2x,
                lighting: Lighting::Subtle,
                bloom: true,
            },
            accessibility: AccessibilitySettings {
                colorblind_guidance: true,
            },
            ..Settings::default()
        };
        s.controls.keyboard.bind(FlightKey::Space, Some(Key::LCTRL));
        s.controls
            .gamepad
            .bind(FlightKey::P, Some(PadButton::Guide));
        let dir = std::env::temp_dir().join(format!("td2192-settings-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.toml");
        s.save(&path).unwrap();
        let back = Settings::load(&path).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(back, s);
    }

    #[test]
    fn only_the_players_changes_reach_the_file() {
        use td_input::Key;
        use td_sim::FlightKey;
        let file = Settings {
            data_dir: Some(PathBuf::from("C:/games/td")),
            ..Settings::default()
        };
        // The session: --fullscreen and --dev-enhanced on top of the file.
        let mut session = file.clone();
        session.video.fullscreen = true;
        session.profile = Profile::Enhanced;
        session.enhanced.upscaler = Upscaler::Xbr3;
        // The player changes the scaling, the music volume and a key.
        let mut after = session.clone();
        after.video.scale = Scale::Integer;
        after.audio.music_volume = 0.3;
        after
            .controls
            .keyboard
            .bind(FlightKey::Space, Some(Key::LCTRL));
        let saved = file.with_changes(&session, &after);
        assert_eq!(saved.video.scale, Scale::Integer);
        assert_eq!(saved.audio.music_volume, 0.3);
        assert_eq!(saved.controls, after.controls);
        assert!(
            !saved.video.fullscreen,
            "the override stays out of the file"
        );
        assert_eq!(saved.profile, Profile::Classic);
        assert_eq!(saved.enhanced, EnhancedSettings::default());
        assert_eq!(saved.data_dir, file.data_dir);
        // Turning an override off is a change too.
        let mut off = session.clone();
        off.video.fullscreen = false;
        let mut file_fs = file.clone();
        file_fs.video.fullscreen = true;
        assert!(!file_fs.with_changes(&session, &off).video.fullscreen);
        // Unchanged: the file as it is.
        assert_eq!(file.with_changes(&session, &session), file);
    }

    #[test]
    fn classic_forces_every_enhanced_feature_off() {
        let s = Settings {
            profile: Profile::Classic,
            audio: AudioSettings {
                sfx_mode: SfxMode::Polyphonic,
                ..AudioSettings::default()
            },
            enhanced: EnhancedSettings {
                widescreen: Widescreen::W21x9,
                upscaler: Upscaler::Xbr4,
                lighting: Lighting::Full,
                bloom: true,
            },
            accessibility: AccessibilitySettings {
                colorblind_guidance: true,
            },
            ..Settings::default()
        };
        assert!(s.effective_enhanced().is_off());
        // The profile governs only [enhanced] (spec enhanced/presentation rev 2 §2).
        assert_eq!(s.audio.sfx_mode, SfxMode::Polyphonic);
        assert!(s.accessibility.colorblind_guidance);
        // The choices themselves are kept for when the profile changes back.
        assert_eq!(s.enhanced.lighting, Lighting::Full);
    }

    #[test]
    fn missing_file_gives_defaults() {
        let s = Settings::load(Path::new("definitely/not/here/settings.toml")).unwrap();
        assert_eq!(s, Settings::default());
    }
}
