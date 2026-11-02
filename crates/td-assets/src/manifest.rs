//! The asset manifest: every file of the original v1.0 installation with its size and SHA-256.
//!
//! The table itself (`manifest_data.rs`) is generated from `docs/re/FILE_INVENTORY.md` by
//! `tdtool gen-manifest`; do not edit it by hand.

use std::fmt;

/// Broad kind of an original file (the inventory's Category column).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Category {
    Exe,
    Doc,
    Palette,
    Screen,
    Picture,
    Portraits,
    UiGfx,
    Tileset,
    Map,
    Sprites,
    Mission,
    Dialogue,
    Font,
    Config,
    Save,
    Music,
    Sfx,
    Presentation,
}

/// The three episodes of the campaign.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Episode {
    E1,
    E2,
    E3,
}

impl Episode {
    /// All episodes in order.
    pub const ALL: [Episode; 3] = [Episode::E1, Episode::E2, Episode::E3];

    /// The usage flag for this episode.
    pub const fn usage(self) -> Usage {
        match self {
            Episode::E1 => Usage::E1,
            Episode::E2 => Usage::E2,
            Episode::E3 => Usage::E3,
        }
    }
}

impl fmt::Display for Episode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let n = match self {
            Episode::E1 => 1,
            Episode::E2 => 2,
            Episode::E3 => 3,
        };
        write!(f, "Episode {n}")
    }
}

/// Which original programs use a file (the inventory's "Referenced by" column).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Usage(u8);

impl Usage {
    pub const NONE: Usage = Usage(0);
    pub const LAUNCHER: Usage = Usage(1);
    pub const INTRO: Usage = Usage(2);
    pub const E1: Usage = Usage(4);
    pub const E2: Usage = Usage(8);
    pub const E3: Usage = Usage(16);

    /// Union of two usage sets.
    pub const fn union(self, other: Usage) -> Usage {
        Usage(self.0 | other.0)
    }

    /// True if every flag of `other` is set in `self`.
    pub const fn contains(self, other: Usage) -> bool {
        self.0 & other.0 == other.0 && other.0 != 0
    }

    /// True if no flag is set.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl fmt::Debug for Usage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names = [
            (Usage::LAUNCHER, "launcher"),
            (Usage::INTRO, "intro"),
            (Usage::E1, "E1"),
            (Usage::E2, "E2"),
            (Usage::E3, "E3"),
        ];
        let set: Vec<&str> = names
            .iter()
            .filter(|(u, _)| self.contains(*u))
            .map(|(_, n)| *n)
            .collect();
        write!(f, "Usage({})", set.join("|"))
    }
}

/// One original file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManifestEntry {
    /// 8.3 file name in lower case.
    pub name: &'static str,
    /// Size in bytes.
    pub size: u64,
    /// Lower-case hex SHA-256.
    pub sha256: &'static str,
    pub category: Category,
    /// Programs that use the file.
    pub used_by: Usage,
    /// Whether every episode in `used_by` needs this file to be playable. Optional files have a
    /// fallback (for example the edited dialogue script) or are not used by the port.
    pub required: bool,
}

include!("manifest_data.rs");

/// Every file of the original installation, sorted by name.
pub fn entries() -> &'static [ManifestEntry] {
    FILES
}

/// Looks up a file by any spelling the original code might use (8.3 and case rules apply).
pub fn entry(name: &str) -> Option<&'static ManifestEntry> {
    let key = crate::dos83(name);
    FILES.iter().find(|e| e.name == key)
}

/// Every file of a category.
pub fn of_category(category: Category) -> impl Iterator<Item = &'static ManifestEntry> {
    FILES.iter().filter(move |e| e.category == category)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_matches_inventory_totals() {
        assert_eq!(FILES.len(), 218);
        assert_eq!(FILES.iter().map(|e| e.size).sum::<u64>(), 9_008_601);
        assert!(
            FILES.windows(2).all(|w| w[0].name < w[1].name),
            "sorted, unique"
        );
        assert!(FILES.iter().all(|e| e.sha256.len() == 64));
    }

    #[test]
    fn lookup_applies_dos_rules() {
        let e = entry("HELIHANGAR.SCR").expect("helihang.scr");
        assert_eq!(e.name, "helihang.scr");
        assert_eq!(e.category, Category::Screen);
    }

    #[test]
    fn missions_belong_to_their_episode() {
        assert!(entry("m1.td").unwrap().used_by.contains(Usage::E1));
        assert!(entry("m1.td2").unwrap().used_by.contains(Usage::E2));
        assert!(entry("m21.td3").unwrap().used_by.contains(Usage::E3));
        assert_eq!(of_category(Category::Mission).count(), 60);
    }

    #[test]
    fn usage_flags_compose() {
        let u = Usage::E1.union(Usage::E3);
        assert!(u.contains(Usage::E1));
        assert!(!u.contains(Usage::E2));
        assert!(!u.contains(Usage::NONE));
        assert_eq!(format!("{u:?}"), "Usage(E1|E3)");
    }
}
