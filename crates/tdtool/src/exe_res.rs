//! `tdtool exe-res`: verify the executable resource maps against an installation.
//!
//! Prints ids, kinds and sizes. With `--show`, it also prints the extracted text to the terminal
//! (for inspection only; never paste it into the repository).

use std::path::Path;

use anyhow::Result;
use td_assets::{DataDir, ExeResources, MapStatus, ResourceValue};

pub fn run(data: &Path, show: bool) -> Result<()> {
    let res = ExeResources::load(&DataDir::open(data)?)?;
    for (exe, status) in &res.status {
        println!("{exe}: {status:?}");
    }
    for id in res.ids() {
        let summary = match res.get(id).expect("listed id") {
            ResourceValue::Text(s) => format!("text, {} chars", s.len()),
            ResourceValue::TextList(v) => format!("{} texts", v.len()),
            ResourceValue::Bytes(b) => format!("{} bytes", b.len()),
            ResourceValue::U16s(v) => format!("{} u16", v.len()),
            ResourceValue::I16s(v) => format!("{} i16", v.len()),
        };
        println!("  {id}: {summary}");
        if show {
            match res.get(id).expect("listed id") {
                ResourceValue::Text(s) => println!("      {s}"),
                ResourceValue::TextList(v) => v.iter().for_each(|s| println!("      {s}")),
                other => println!("      {other:?}"),
            }
        }
    }
    if res
        .status
        .iter()
        .any(|(_, s)| !matches!(s, MapStatus::Loaded { .. }))
    {
        anyhow::bail!("some maps could not be applied");
    }
    Ok(())
}
