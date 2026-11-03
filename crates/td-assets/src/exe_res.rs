//! Text and tables embedded in the original executables, read at runtime from the user's copies.
//!
//! Spec: ADR-0003, MDG §9.1.4, `docs/re/exe-resources/README.md`. The offset maps are compiled
//! into the program; each map is only applied to an executable whose SHA-256 matches, so a
//! different version yields "unavailable" rather than wrong text.

use std::collections::BTreeMap;

use serde::Deserialize;
use sha2::{Digest, Sha256};
use td_formats::mz::pstring;
use thiserror::Error;

use crate::DataDir;

/// The compiled-in resource maps (TOML source, as committed under `docs/re/exe-resources/`).
const MAPS: &[(&str, &str)] = &[
    (
        "file0000.toml",
        include_str!("../../../docs/re/exe-resources/file0000.toml"),
    ),
    (
        "file0001.toml",
        include_str!("../../../docs/re/exe-resources/file0001.toml"),
    ),
    (
        "td2.toml",
        include_str!("../../../docs/re/exe-resources/td2.toml"),
    ),
    (
        "td3.toml",
        include_str!("../../../docs/re/exe-resources/td3.toml"),
    ),
];

/// Errors in a resource map itself (programming errors caught by tests).
#[derive(Debug, Error)]
pub enum ExeResError {
    #[error("resource map {map}: {source}")]
    Map {
        map: &'static str,
        source: toml::de::Error,
    },
    #[error("resource {id} in {exe}: {detail}")]
    Extract {
        exe: String,
        id: String,
        detail: String,
    },
}

#[derive(Debug, Deserialize)]
struct MapFile {
    exe: String,
    sha256: String,
    #[serde(default)]
    resource: Vec<ResourceDef>,
}

#[derive(Debug, Deserialize)]
struct ResourceDef {
    id: String,
    kind: Kind,
    offset: usize,
    #[serde(default)]
    count: usize,
    #[serde(default)]
    len: usize,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Kind {
    Pstring,
    PstringSeq,
    FixedWidthText,
    U8Table,
    U16Table,
    I16Table,
}

/// One extracted value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceValue {
    Text(String),
    TextList(Vec<String>),
    Bytes(Vec<u8>),
    U16s(Vec<u16>),
    I16s(Vec<i16>),
}

/// Status of one executable's map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MapStatus {
    /// Hash matched; all resources extracted.
    Loaded { resources: usize },
    /// The executable is missing.
    Missing,
    /// The executable differs from the version the map describes.
    VersionMismatch,
}

/// All extracted resources, keyed by id.
#[derive(Debug, Clone, Default)]
pub struct ExeResources {
    values: BTreeMap<String, ResourceValue>,
    /// Per executable: what happened when loading its map.
    pub status: Vec<(String, MapStatus)>,
}

fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| char::from(b)).collect()
}

impl ExeResources {
    /// Loads every compiled-in map against the executables in `dir`.
    pub fn load(dir: &DataDir) -> Result<Self, ExeResError> {
        let mut out = Self::default();
        for &(map_name, src) in MAPS {
            let map: MapFile = toml::from_str(src).map_err(|source| ExeResError::Map {
                map: map_name,
                source,
            })?;
            let Ok(bytes) = dir.read(&map.exe) else {
                out.status.push((map.exe, MapStatus::Missing));
                continue;
            };
            let hex: String = Sha256::digest(&bytes)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            if hex != map.sha256 {
                tracing::warn!(
                    "{} differs from the known version; its text is unavailable",
                    map.exe
                );
                out.status.push((map.exe, MapStatus::VersionMismatch));
                continue;
            }
            let n = map.resource.len();
            for def in &map.resource {
                let value = extract(&bytes, def).map_err(|detail| ExeResError::Extract {
                    exe: map.exe.clone(),
                    id: def.id.clone(),
                    detail,
                })?;
                out.values.insert(def.id.clone(), value);
            }
            out.status
                .push((map.exe, MapStatus::Loaded { resources: n }));
        }
        Ok(out)
    }

    /// A value by id.
    pub fn get(&self, id: &str) -> Option<&ResourceValue> {
        self.values.get(id)
    }

    /// A single text by id.
    pub fn text(&self, id: &str) -> Option<&str> {
        match self.values.get(id)? {
            ResourceValue::Text(s) => Some(s),
            _ => None,
        }
    }

    /// A list of texts by id.
    pub fn texts(&self, id: &str) -> Option<&[String]> {
        match self.values.get(id)? {
            ResourceValue::TextList(v) => Some(v),
            _ => None,
        }
    }

    /// All resource ids, sorted.
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.values.keys().map(String::as_str)
    }
}

fn extract(bytes: &[u8], def: &ResourceDef) -> Result<ResourceValue, String> {
    let slice = |len: usize| {
        bytes
            .get(def.offset..def.offset + len)
            .ok_or_else(|| format!("offset {:#x}+{len} beyond the file", def.offset))
    };
    Ok(match def.kind {
        Kind::Pstring => ResourceValue::Text(latin1(
            pstring(bytes, def.offset).map_err(|e| e.to_string())?,
        )),
        Kind::PstringSeq => {
            let seq = td_formats::mz::pstring_seq(bytes, def.offset, def.count)
                .map_err(|e| e.to_string())?;
            ResourceValue::TextList(seq.into_iter().map(latin1).collect())
        }
        Kind::FixedWidthText => ResourceValue::TextList(
            slice(def.len * def.count)?
                .chunks(def.len)
                .map(latin1)
                .collect(),
        ),
        Kind::U8Table => ResourceValue::Bytes(slice(def.count)?.to_vec()),
        Kind::U16Table => ResourceValue::U16s(
            slice(def.count * 2)?
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect(),
        ),
        Kind::I16Table => ResourceValue::I16s(
            slice(def.count * 2)?
                .chunks_exact(2)
                .map(|c| i16::from_le_bytes([c[0], c[1]]))
                .collect(),
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_maps_parse() {
        for &(name, src) in MAPS {
            let m: MapFile = toml::from_str(src).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(m.sha256.len(), 64, "{name}");
            let mut ids: Vec<&str> = m.resource.iter().map(|r| r.id.as_str()).collect();
            ids.sort_unstable();
            ids.dedup();
            assert_eq!(ids.len(), m.resource.len(), "{name}: duplicate ids");
        }
    }

    #[test]
    fn extracts_all_kinds() {
        let bytes = b"\x03abc\x02de\x05\x00\xFF\xFFxyzw";
        let def = |kind, offset, count, len| ResourceDef {
            id: "t".into(),
            kind,
            offset,
            count,
            len,
        };
        assert_eq!(
            extract(bytes, &def(Kind::Pstring, 0, 0, 0)).unwrap(),
            ResourceValue::Text("abc".into())
        );
        assert_eq!(
            extract(bytes, &def(Kind::PstringSeq, 0, 2, 0)).unwrap(),
            ResourceValue::TextList(vec!["abc".into(), "de".into()])
        );
        assert_eq!(
            extract(bytes, &def(Kind::U16Table, 7, 1, 0)).unwrap(),
            ResourceValue::U16s(vec![5])
        );
        assert_eq!(
            extract(bytes, &def(Kind::I16Table, 9, 1, 0)).unwrap(),
            ResourceValue::I16s(vec![-1])
        );
        assert_eq!(
            extract(bytes, &def(Kind::FixedWidthText, 11, 2, 2)).unwrap(),
            ResourceValue::TextList(vec!["xy".into(), "zw".into()])
        );
        assert!(extract(bytes, &def(Kind::U8Table, 10, 100, 0)).is_err());
    }

    #[test]
    fn loads_from_the_original_executables() {
        let data = td_test_support::require_data!();
        let res = ExeResources::load(&DataDir::open(data).unwrap()).unwrap();
        assert!(
            res.status
                .iter()
                .all(|(_, s)| matches!(s, MapStatus::Loaded { .. })),
            "{:?}",
            res.status
        );
        let seed = res.texts("e1.story_strings.seed").unwrap();
        assert_eq!(seed.len(), 8);
        // Facts only: lengths of the first two strings (no game text in the repository).
        assert_eq!((seed[0].len(), seed[1].len()), (27, 12));
        assert!(
            seed.iter()
                .all(|s| s.bytes().all(|b| (0x20..0x7f).contains(&b)))
        );
    }
}
