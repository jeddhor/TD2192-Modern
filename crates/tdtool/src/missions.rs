//! `tdtool mission-map`: draw every mission's ship paths over a city map (outside the repo).
//!
//! Each used slot's 500-step path is drawn as a coloured polyline over a 1/4-scale render of the
//! map (8 px per tile), with the slot's class code at its first point. Useful for mission RE
//! (M8-01) and for deciding which map a mission uses (U-006).

use std::path::Path;

use anyhow::Result;
use td_assets::DataDir;
use td_formats::map::TileMap;
use td_formats::mission::Mission;
use td_formats::palette::Palette;
use td_formats::tileset::{TILE_SIZE, Tileset};

const SCALE: usize = 4; // map pixels per output pixel
const CELL: usize = TILE_SIZE / SCALE;

/// Palette indices used for the path colours (chosen from td.pal's bright entries at runtime).
const SLOT_COLORS: usize = 20;

pub fn run(data: &Path, out: &Path, episode: u8, map_name: &str, blk_name: &str) -> Result<()> {
    crate::export::refuse_repo_output(out)?;
    std::fs::create_dir_all(out)?;
    let dir = DataDir::open(data)?;
    let map = TileMap::parse_any(&dir.read(map_name)?)?;
    let tiles = Tileset::parse(&dir.read(blk_name)?)?;
    let mut pal = Palette::parse(&dir.read("td.pal")?)?;
    // Reserve the last 20 palette entries for path colours (the export is for inspection only).
    for i in 0..SLOT_COLORS {
        let hue = i as f32 / SLOT_COLORS as f32 * 6.0;
        let (r, g, b) = hsv(hue);
        pal.colors[255 - i] = td_formats::palette::Rgb6 { r, g, b };
    }
    let size = map.size() * CELL;
    let mut base = vec![0u8; size * size];
    for ty in 0..map.size() {
        for tx in 0..map.size() {
            let t = map.get(tx, ty).unwrap_or(0) as usize;
            let Some(tile) = tiles.tiles.get(t) else {
                continue;
            };
            for y in 0..CELL {
                for x in 0..CELL {
                    base[(ty * CELL + y) * size + tx * CELL + x] =
                        tile[(y * SCALE) * TILE_SIZE + x * SCALE];
                }
            }
        }
    }
    let ext = match episode {
        1 => "td",
        2 => "td2",
        _ => "td3",
    };
    for n in 1..=21u8 {
        let name = format!("m{n}.{ext}");
        let Ok(bytes) = dir.read(&name) else {
            continue;
        };
        let m = Mission::parse(&bytes)?;
        let mut img = base.clone();
        for slot in m.used_slots() {
            let color = 255 - (slot % SLOT_COLORS) as u8;
            let pts: Vec<(i64, i64)> = m
                .path(slot)
                .expect("slot in range")
                .filter(|&(x, y)| x != 0 || y != 0)
                .map(|(x, y)| {
                    (
                        i64::from(x) * CELL as i64 + CELL as i64 / 2,
                        i64::from(y) * CELL as i64 + CELL as i64 / 2,
                    )
                })
                .collect();
            for w in pts.windows(2) {
                line(&mut img, size, w[0], w[1], color);
            }
            if let Some(&(x, y)) = pts.first() {
                for d in -3..=3 {
                    plot(&mut img, size, x + d, y, color);
                    plot(&mut img, size, x, y + d, color);
                }
            }
        }
        let file = out.join(format!("{name}.on.{map_name}.png"));
        crate::export::write_png(&file, &img, size, size, &pal)?;
        let classes: Vec<String> = m
            .used_slots()
            .map(|s| format!("{s}:{}", m.slot_class[s]))
            .collect();
        println!("{name}: slots {}", classes.join(" "));
    }
    Ok(())
}

fn hsv(h: f32) -> (u8, u8, u8) {
    let x = (1.0 - ((h % 2.0) - 1.0).abs()) * 63.0;
    let (r, g, b) = match h as u32 {
        0 => (63.0, x, 0.0),
        1 => (x, 63.0, 0.0),
        2 => (0.0, 63.0, x),
        3 => (0.0, x, 63.0),
        4 => (x, 0.0, 63.0),
        _ => (63.0, 0.0, x),
    };
    (r as u8, g as u8, b as u8)
}

fn plot(img: &mut [u8], size: usize, x: i64, y: i64, c: u8) {
    if (0..size as i64).contains(&x) && (0..size as i64).contains(&y) {
        img[y as usize * size + x as usize] = c;
    }
}

fn line(img: &mut [u8], size: usize, (x0, y0): (i64, i64), (x1, y1): (i64, i64), c: u8) {
    let n = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
    for i in 0..=n {
        plot(img, size, x0 + (x1 - x0) * i / n, y0 + (y1 - y0) * i / n, c);
    }
}
