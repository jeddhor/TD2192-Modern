//! User settings, stored as `settings.toml` in the per-user data directory (MDG §8.8).
//!
//! Every field has a default so that older or hand-edited files keep working; unknown keys are
//! ignored.
//!
//! **Profiles and the Enhanced preset** (MDG §6.2, decision D5 of 2026-11-06; spec
//! frontend/port-front-end rev 3 §3.4). The Classic profile (the default) forces every Enhanced
//! feature off. Choosing the Enhanced profile turns on a preset ([`ENHANCED_PRESET`],
//! [`PRESET_SFX_MODE`], [`PRESET_FIXES`]): 16:9 widescreen, xBR 3x, subtle lighting, bloom
//! off, polyphonic sound effects and every fix-enhanced original-bug fix
//! (`docs/spec/original-bugs.md`). Each stays adjustable: a value the player sets is an
//! explicit choice and overrides the preset, under the Enhanced profile only, and it is kept
//! when the profile changes. The accessibility options and the intro setting are separate and
//! off by default.
//!
//! **How the file tells a preset default from a choice** (format 2, `version = 2`): an
//! Enhanced key is written only when the player chose its value; an absent key takes the
//! preset. So `[enhanced]` holds only choices, and `audio.sfx_mode` and `enhanced.fixes` work
//! the same way. For example, the Enhanced preset with widescreen turned off:
//!
//! ```toml
//! version = 2
//! profile = "enhanced"
//!
//! [enhanced]
//! widescreen = "off"    # off | "16:10" | "16:9" | "21:9" | "auto"; absent: the preset (16:9)
//! # upscaler = none | scale2x | scale3x | scale4x | xbr2 | xbr3 | xbr4; absent: xbr3
//! # lighting = off | subtle | full; absent: subtle
//! # bloom = true | false; absent: false
//! # fixes = true | false (the fix-enhanced original-bug fixes); absent: true
//! ```
//!
//! **Older files** (format 1, no `version` key, written before D5) wrote every key, so a value
//! there cannot be told from an untouched default. On loading, the format-1 defaults (`"off"`,
//! `"none"`, `false`, `sfx_mode = "classic"`) are read as "no choice" (the preset), and any
//! other value as an explicit choice; the next save writes format 2 ([`Settings::migrate`]).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
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
    /// Whether the game pauses while its window is in the background (M12-18,
    /// port-front-end rev 5 §3.7); `None` takes the profile's default
    /// ([`Settings::pauses_in_background`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pause_in_background: Option<bool>,
}

impl Default for VideoSettings {
    fn default() -> Self {
        Self {
            scale: Scale::Aspect,
            fullscreen: false,
            pause_in_background: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioSettings {
    pub music_volume: f32,
    pub sfx_volume: f32,
    /// The sound-effect policy chosen for the Enhanced profile; `None` takes the preset
    /// ([`PRESET_SFX_MODE`]). Classic always mixes like the original
    /// ([`Settings::effective_sfx_mode`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sfx_mode: Option<SfxMode>,
}

impl Default for AudioSettings {
    /// Music and effects at the same volume: the mixer's gains reproduce the original's
    /// balance (docs/parity/M3-audio.md, VE-3), so equal settings keep it.
    fn default() -> Self {
        Self {
            music_volume: 0.8,
            sfx_volume: 0.8,
            sfx_mode: None,
        }
    }
}

/// The fidelity profile (MDG §6.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Profile {
    /// The original presentation, sound and rules: every Enhanced feature is forced off.
    #[default]
    Classic,
    /// The Enhanced preset (D5), with the player's explicit choices over it.
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

/// The Enhanced presentation features in effect (spec enhanced/presentation §2): what the
/// renderer reads. Widescreen affects gameplay (the player sees further).
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

/// The presentation half of the Enhanced preset (D5, user 2026-11-06): 16:9 widescreen, xBR
/// 3x, subtle lighting, bloom off.
pub const ENHANCED_PRESET: EnhancedSettings = EnhancedSettings {
    widescreen: Widescreen::W16x9,
    upscaler: Upscaler::Xbr3,
    lighting: Lighting::Subtle,
    bloom: false,
};

/// The Enhanced preset's sound effects (D5): polyphonic.
pub const PRESET_SFX_MODE: SfxMode = SfxMode::Polyphonic;

/// The Enhanced preset's original-bug fixes (D5): every fix-enhanced fix on.
pub const PRESET_FIXES: bool = true;

/// The `[enhanced]` section as stored: the player's explicit choices. `None` (an absent key)
/// takes the preset ([`ENHANCED_PRESET`], [`PRESET_FIXES`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct EnhancedChoices {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub widescreen: Option<Widescreen>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upscaler: Option<Upscaler>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lighting: Option<Lighting>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bloom: Option<bool>,
    /// The fix-enhanced original-bug fixes (`docs/spec/original-bugs.md`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fixes: Option<bool>,
}

impl EnhancedChoices {
    /// Every presentation feature chosen explicitly as `e` (`--dev-enhanced`); the fixes keep
    /// the preset.
    #[must_use]
    pub fn chosen(e: EnhancedSettings) -> Self {
        Self {
            widescreen: Some(e.widescreen),
            upscaler: Some(e.upscaler),
            lighting: Some(e.lighting),
            bloom: Some(e.bloom),
            fixes: None,
        }
    }

    /// The presentation features: each choice, else the preset's value.
    #[must_use]
    pub fn values(&self) -> EnhancedSettings {
        EnhancedSettings {
            widescreen: self.widescreen.unwrap_or(ENHANCED_PRESET.widescreen),
            upscaler: self.upscaler.unwrap_or(ENHANCED_PRESET.upscaler),
            lighting: self.lighting.unwrap_or(ENHANCED_PRESET.lighting),
            bloom: self.bloom.unwrap_or(ENHANCED_PRESET.bloom),
        }
    }

    /// True when nothing is chosen (the whole preset applies).
    pub fn is_preset(&self) -> bool {
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
    /// Reduce flashing (M12-18, port-front-end rev 5 §3.5): the colours on screen change at a
    /// limited rate in flight (`td_render::flash`), the cut-scene's flash steps are capped, and
    /// the Enhanced lighting is softened (no muzzle flashes, weaker explosions, no bloom).
    /// Presentation only.
    pub reduce_flashing: bool,
    /// Text waits for a key (M12-18, port-front-end rev 5 §3.6): timed text screens wait for a
    /// key instead of closing by themselves, and keys pressed while a page types or fades in
    /// do not end its wait.
    pub text_waits_for_key: bool,
}

/// Developer overrides (ignored unless set).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DevSettings {
    /// Overrides the simulation tick rate (Hz).
    pub tick_hz: Option<f64>,
}

/// The settings-file format written now: 2 since D5 (2026-11-06), where an absent Enhanced
/// key means the preset (module docs). Files without a `version` key are format 1.
pub const SETTINGS_VERSION: u32 = 2;

fn format_1() -> u32 {
    1
}

/// All user settings.
///
/// Serialised by the derived code (`remote = "Self"`), wrapped so that loading migrates older
/// formats ([`Settings::migrate`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", default)]
pub struct Settings {
    /// The settings-file format ([`SETTINGS_VERSION`]; 1 when the key is absent).
    #[serde(default = "format_1")]
    pub version: u32,
    /// The original game directory.
    pub data_dir: Option<PathBuf>,
    /// Classic forces every Enhanced feature off; Enhanced turns on the preset.
    pub profile: Profile,
    pub video: VideoSettings,
    pub audio: AudioSettings,
    /// The player's Enhanced choices (spec enhanced/presentation §2); absent keys take the
    /// preset.
    pub enhanced: EnhancedChoices,
    /// Host keys and controller buttons for the original's flight keys (M11-03).
    pub controls: ControlsSettings,
    pub accessibility: AccessibilitySettings,
    pub dev: DevSettings,
    /// When the intro plays before Episode 1 (Enhanced only; launcher spec §4.10).
    pub intro: crate::intro::IntroSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            data_dir: None,
            profile: Profile::default(),
            video: VideoSettings::default(),
            audio: AudioSettings::default(),
            enhanced: EnhancedChoices::default(),
            controls: ControlsSettings::default(),
            accessibility: AccessibilitySettings::default(),
            dev: DevSettings::default(),
            intro: crate::intro::IntroSettings::default(),
        }
    }
}

impl<'de> Deserialize<'de> for Settings {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        // The derived code (an inherent function under `remote = "Self"`), then the migration.
        let mut s = Settings::deserialize(d)?;
        s.migrate();
        Ok(s)
    }
}

impl Serialize for Settings {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        Settings::serialize(self, s)
    }
}

impl Settings {
    /// Brings settings read from an older file up to [`SETTINGS_VERSION`]. Format 1 (before
    /// D5) wrote every key and had "off" as the only default, so its default values record no
    /// choice and become the preset; other values stay explicit choices.
    pub fn migrate(&mut self) {
        if self.version < 2 {
            let e = &mut self.enhanced;
            e.widescreen = e.widescreen.filter(|&w| w != Widescreen::Off);
            e.upscaler = e.upscaler.filter(|&u| u != Upscaler::None);
            e.lighting = e.lighting.filter(|&l| l != Lighting::Off);
            e.bloom = e.bloom.filter(|&b| b);
            self.audio.sfx_mode = self.audio.sfx_mode.filter(|&m| m != SfxMode::Classic);
        }
        self.version = self.version.max(SETTINGS_VERSION);
    }

    /// True under the Enhanced profile.
    pub fn is_enhanced(&self) -> bool {
        self.profile == Profile::Enhanced
    }

    /// The presentation features the Enhanced profile uses: each explicit choice, else the
    /// preset. The options screen shows these values under either profile.
    pub fn enhanced_values(&self) -> EnhancedSettings {
        self.enhanced.values()
    }

    /// The presentation features in effect: all off under the Classic profile.
    pub fn effective_enhanced(&self) -> EnhancedSettings {
        match self.profile {
            Profile::Classic => EnhancedSettings::default(),
            Profile::Enhanced => self.enhanced_values(),
        }
    }

    /// The SFX policy the Enhanced profile uses: the choice, else the preset (polyphonic).
    pub fn sfx_mode_value(&self) -> SfxMode {
        self.audio.sfx_mode.unwrap_or(PRESET_SFX_MODE)
    }

    /// The SFX policy in effect: the original's under the Classic profile (MDG §6.2).
    pub fn effective_sfx_mode(&self) -> SfxMode {
        match self.profile {
            Profile::Classic => SfxMode::Classic,
            Profile::Enhanced => self.sfx_mode_value(),
        }
    }

    /// Whether the Enhanced profile fixes the fix-enhanced original bugs: the choice, else the
    /// preset (on).
    pub fn fixes_value(&self) -> bool {
        self.enhanced.fixes.unwrap_or(PRESET_FIXES)
    }

    /// Whether the fix-enhanced original-bug fixes are in effect (`docs/spec/original-bugs.md`:
    /// preserved in Classic, fixed in Enhanced). Never under the Classic profile.
    pub fn enhanced_fixes(&self) -> bool {
        self.is_enhanced() && self.fixes_value()
    }

    /// Whether the game pauses while its window is in the background: the player's choice,
    /// else the profile's default. Enhanced pauses; Classic keeps running, as the original did
    /// (a DOS program has no background).
    pub fn pauses_in_background(&self) -> bool {
        self.video
            .pause_in_background
            .unwrap_or(self.profile == Profile::Enhanced)
    }

    /// Drops every explicit Enhanced choice, so the whole preset applies again.
    pub fn restore_preset(&mut self) {
        self.enhanced = EnhancedChoices::default();
        self.audio.sfx_mode = None;
    }

    /// `self` (the file's settings) with every value that differs between `before` and
    /// `after` taken from `after`. The options screen edits the session's settings, which may
    /// hold command-line overrides (`--fullscreen`, `--dev-enhanced`); only what the player
    /// changed goes to the file. A choice `after` dropped (back to the preset) is removed.
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

    /// Loads settings from `path`; a missing file yields the defaults. Older formats are
    /// migrated ([`Settings::migrate`]).
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
                pause_in_background: None,
            },
            ..Settings::default()
        };
        let text = toml::to_string_pretty(&s).unwrap();
        assert_eq!(toml::from_str::<Settings>(&text).unwrap(), s);

        let partial: Settings = toml::from_str("[audio]\nsfx_mode = \"polyphonic\"\n").unwrap();
        assert_eq!(partial.audio.sfx_mode, Some(SfxMode::Polyphonic));
        assert_eq!(partial.audio.music_volume, 0.8);
        assert_eq!(partial.video, VideoSettings::default());
        assert_eq!(partial.version, SETTINGS_VERSION, "migrated on loading");
    }

    #[test]
    fn enhanced_choices_round_trip_and_classic_forces_them_off() {
        let s = Settings {
            profile: Profile::Enhanced,
            enhanced: EnhancedChoices {
                widescreen: Some(Widescreen::W21x9),
                upscaler: Some(Upscaler::Scale2x),
                lighting: Some(Lighting::Full),
                bloom: Some(true),
                fixes: Some(false),
            },
            ..Settings::default()
        };
        let text = toml::to_string_pretty(&s).unwrap();
        assert!(text.contains("widescreen = \"21:9\""), "{text}");
        assert!(text.contains("upscaler = \"scale2x\""), "{text}");
        assert!(text.contains("fixes = false"), "{text}");
        assert_eq!(toml::from_str::<Settings>(&text).unwrap(), s);
        assert_eq!(
            s.effective_enhanced(),
            EnhancedSettings {
                widescreen: Widescreen::W21x9,
                upscaler: Upscaler::Scale2x,
                lighting: Lighting::Full,
                bloom: true,
            }
        );
        assert!(!s.enhanced_fixes());
        let classic = Settings {
            profile: Profile::Classic,
            ..s.clone()
        };
        assert!(classic.effective_enhanced().is_off());
        for w in ["off", "16:10", "16:9", "21:9", "auto"] {
            let t = format!("version = 2\n[enhanced]\nwidescreen = \"{w}\"\n");
            assert!(toml::from_str::<Settings>(&t).is_ok(), "{w}");
        }
    }

    /// D5: the Enhanced profile turns on the preset; each feature stays adjustable.
    #[test]
    fn the_enhanced_profile_turns_on_the_preset() {
        let mut s = Settings {
            profile: Profile::Enhanced,
            ..Settings::default()
        };
        assert_eq!(
            s.effective_enhanced(),
            EnhancedSettings {
                widescreen: Widescreen::W16x9,
                upscaler: Upscaler::Xbr3,
                lighting: Lighting::Subtle,
                bloom: false,
            }
        );
        assert_eq!(s.effective_sfx_mode(), SfxMode::Polyphonic);
        assert!(s.enhanced_fixes());
        // The options of D5 stay off by default, and so do the M12-18 options.
        assert!(!s.accessibility.colorblind_guidance);
        assert!(!s.accessibility.reduce_flashing);
        assert!(!s.accessibility.text_waits_for_key);
        assert_eq!(s.intro.play, crate::intro::IntroPolicy::Always);
        // Explicit choices override the preset, one feature at a time.
        s.enhanced.lighting = Some(Lighting::Off);
        s.audio.sfx_mode = Some(SfxMode::Classic);
        s.enhanced.fixes = Some(false);
        let e = s.effective_enhanced();
        assert_eq!(
            (e.widescreen, e.upscaler, e.lighting),
            (Widescreen::W16x9, Upscaler::Xbr3, Lighting::Off)
        );
        assert_eq!(s.effective_sfx_mode(), SfxMode::Classic);
        assert!(!s.enhanced_fixes());
        // Restoring the preset drops the choices.
        s.restore_preset();
        assert!(s.enhanced.is_preset());
        assert_eq!(s.effective_enhanced(), ENHANCED_PRESET);
        assert_eq!(s.effective_sfx_mode(), PRESET_SFX_MODE);
        assert!(s.enhanced_fixes());
    }

    /// Format 2 writes only the explicit choices; an absent key is the preset.
    #[test]
    fn the_file_keeps_only_explicit_choices() {
        let default = Settings::default();
        let text = toml::to_string_pretty(&default).unwrap();
        assert!(text.contains("version = 2"), "{text}");
        for key in [
            "widescreen",
            "upscaler",
            "lighting",
            "bloom",
            "fixes",
            "sfx_mode",
        ] {
            assert!(!text.contains(key), "{key} is no choice: {text}");
        }
        // An explicit choice equal to the old "off" default survives a round trip.
        let off = Settings {
            profile: Profile::Enhanced,
            enhanced: EnhancedChoices {
                widescreen: Some(Widescreen::Off),
                bloom: Some(false),
                ..EnhancedChoices::default()
            },
            audio: AudioSettings {
                sfx_mode: Some(SfxMode::Classic),
                ..AudioSettings::default()
            },
            ..Settings::default()
        };
        let back: Settings = toml::from_str(&toml::to_string_pretty(&off).unwrap()).unwrap();
        assert_eq!(back, off);
        assert_eq!(back.effective_enhanced().widescreen, Widescreen::Off);
        assert_eq!(back.effective_enhanced().upscaler, Upscaler::Xbr3);
        assert_eq!(back.effective_sfx_mode(), SfxMode::Classic);
    }

    /// Format-1 files (before D5) keep loading: their "off" defaults become the preset, other
    /// values stay choices.
    #[test]
    fn older_files_load_and_migrate() {
        // What the M12-08 saver wrote for a Classic player: every key, all off.
        let classic = "data_dir = \"C:/games/td\"\nprofile = \"classic\"\n\
            [audio]\nmusic_volume = 0.5\nsfx_volume = 0.8\nsfx_mode = \"classic\"\n\
            [enhanced]\nwidescreen = \"off\"\nupscaler = \"none\"\nlighting = \"off\"\n\
            bloom = false\n";
        let s: Settings = toml::from_str(classic).unwrap();
        assert_eq!(s.version, SETTINGS_VERSION);
        assert_eq!(s.audio.music_volume, 0.5);
        assert!(s.enhanced.is_preset());
        assert_eq!(s.audio.sfx_mode, None);
        assert!(s.effective_enhanced().is_off(), "still Classic");
        let enhanced = Settings {
            profile: Profile::Enhanced,
            ..s
        };
        assert_eq!(enhanced.effective_enhanced(), ENHANCED_PRESET);
        // An Enhanced player of format 1 keeps what was turned on.
        let chosen = "profile = \"enhanced\"\n[audio]\nsfx_mode = \"polyphonic\"\n\
            [enhanced]\nwidescreen = \"21:9\"\nupscaler = \"none\"\nlighting = \"full\"\n\
            bloom = true\n";
        let s: Settings = toml::from_str(chosen).unwrap();
        assert_eq!(s.enhanced.widescreen, Some(Widescreen::W21x9));
        assert_eq!(s.enhanced.upscaler, None, "the old default is no choice");
        assert_eq!(s.enhanced.lighting, Some(Lighting::Full));
        assert_eq!(s.enhanced.bloom, Some(true));
        assert_eq!(s.audio.sfx_mode, Some(SfxMode::Polyphonic));
        // Saved again, the file is format 2 and reads back the same.
        let text = toml::to_string_pretty(&s).unwrap();
        assert!(text.contains("version = 2"), "{text}");
        assert_eq!(toml::from_str::<Settings>(&text).unwrap(), s);
        // A format-2 file is not migrated: its "off" is a choice.
        let v2: Settings = toml::from_str(
            "version = 2\nprofile = \"enhanced\"\n[enhanced]\nupscaler = \"none\"\n",
        )
        .unwrap();
        assert_eq!(v2.enhanced.upscaler, Some(Upscaler::None));
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
                pause_in_background: Some(false),
            },
            audio: AudioSettings {
                music_volume: 0.25,
                sfx_volume: 1.0,
                sfx_mode: Some(SfxMode::Classic),
            },
            enhanced: EnhancedChoices {
                widescreen: Some(Widescreen::Auto),
                upscaler: Some(Upscaler::Scale2x),
                lighting: None,
                bloom: Some(true),
                fixes: Some(false),
            },
            accessibility: AccessibilitySettings {
                colorblind_guidance: true,
                reduce_flashing: true,
                text_waits_for_key: true,
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
        session.enhanced.upscaler = Some(Upscaler::Xbr4);
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
        assert_eq!(saved.enhanced, EnhancedChoices::default());
        assert_eq!(saved.data_dir, file.data_dir);
        // Turning an override off is a change too.
        let mut off = session.clone();
        off.video.fullscreen = false;
        let mut file_fs = file.clone();
        file_fs.video.fullscreen = true;
        assert!(!file_fs.with_changes(&session, &off).video.fullscreen);
        // Unchanged: the file as it is.
        assert_eq!(file.with_changes(&session, &session), file);
        // A choice reaches the file; restoring the preset removes it again.
        let mut file = file;
        let mut chose = file.clone();
        chose.profile = Profile::Enhanced;
        chose.enhanced.lighting = Some(Lighting::Full);
        file = file.with_changes(&file.clone(), &chose);
        assert_eq!(file.enhanced.lighting, Some(Lighting::Full));
        let mut preset = chose.clone();
        preset.restore_preset();
        file = file.with_changes(&chose, &preset);
        assert!(file.enhanced.is_preset());
        assert_eq!(file.profile, Profile::Enhanced);
    }

    #[test]
    fn classic_forces_every_enhanced_feature_off() {
        let s = Settings {
            profile: Profile::Classic,
            audio: AudioSettings {
                sfx_mode: Some(SfxMode::Polyphonic),
                ..AudioSettings::default()
            },
            enhanced: EnhancedChoices {
                widescreen: Some(Widescreen::W21x9),
                upscaler: Some(Upscaler::Xbr4),
                lighting: Some(Lighting::Full),
                bloom: Some(true),
                fixes: Some(true),
            },
            accessibility: AccessibilitySettings {
                colorblind_guidance: true,
                reduce_flashing: true,
                ..AccessibilitySettings::default()
            },
            ..Settings::default()
        };
        assert!(s.effective_enhanced().is_off());
        assert_eq!(s.effective_sfx_mode(), SfxMode::Classic);
        assert!(!s.enhanced_fixes());
        // The accessibility options are not Enhanced features (MDG §6.2, D5).
        assert!(s.accessibility.colorblind_guidance);
        assert!(s.accessibility.reduce_flashing);
        // The choices themselves are kept for when the profile changes back.
        assert_eq!(s.enhanced.lighting, Some(Lighting::Full));
        assert_eq!(s.sfx_mode_value(), SfxMode::Polyphonic);
        assert_eq!(Settings::default().profile, Profile::Classic);
    }

    /// M12-18: the window in the background pauses under Enhanced and keeps running under
    /// Classic (the original's behaviour), unless the player chose; the choice is kept in the
    /// file and the default is not written.
    #[test]
    fn pausing_in_the_background_follows_the_profile_unless_chosen() {
        let mut s = Settings::default();
        assert!(!s.pauses_in_background(), "Classic keeps running");
        s.profile = Profile::Enhanced;
        assert!(s.pauses_in_background(), "Enhanced pauses");
        let text = toml::to_string_pretty(&s).unwrap();
        assert!(!text.contains("pause_in_background"), "{text}");
        s.video.pause_in_background = Some(false);
        assert!(!s.pauses_in_background());
        let back: Settings = toml::from_str(&toml::to_string_pretty(&s).unwrap()).unwrap();
        assert_eq!(back.video.pause_in_background, Some(false));
        s.profile = Profile::Classic;
        s.video.pause_in_background = Some(true);
        assert!(
            s.pauses_in_background(),
            "a choice applies under Classic too"
        );
    }

    #[test]
    fn missing_file_gives_defaults() {
        let s = Settings::load(Path::new("definitely/not/here/settings.toml")).unwrap();
        assert_eq!(s, Settings::default());
    }
}
