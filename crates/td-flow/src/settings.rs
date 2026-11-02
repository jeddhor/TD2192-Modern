//! User settings, stored as `settings.toml` in the per-user data directory (MDG §8.8).
//!
//! Every field has a default so that older or hand-edited files keep working; unknown keys are
//! ignored.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

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
    pub video: VideoSettings,
    pub audio: AudioSettings,
    pub dev: DevSettings,
}

impl Settings {
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
    fn missing_file_gives_defaults() {
        let s = Settings::load(Path::new("definitely/not/here/settings.toml")).unwrap();
        assert_eq!(s, Settings::default());
    }
}
