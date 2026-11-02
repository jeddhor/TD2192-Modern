//! `tdtool export`: decode original assets to PNG for inspection.
//!
//! Output contains game content, so it must go **outside the repository** (MDG §5.3).

use std::fs::{self, File};
use std::io::BufWriter;
use std::path::Path;

use anyhow::{Context, Result, bail};
use td_assets::DataDir;
use td_formats::map::TileMap;
use td_formats::palette::Palette;
use td_formats::screen::{PagedScreens, Screen};
use td_formats::sprites::SpriteSheet;
use td_formats::tileset::{TILE_SIZE, Tileset};
use td_render::headings::CELL;
use td_render::{HEADINGS, HeadingSprites};

/// What to export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Kind {
    /// Every full-screen image and the help pages.
    Screens,
    /// Contact sheets of the three tilesets.
    Tiles,
    /// All sprites at all 16 headings.
    Sprites,
    /// Full renders of every map with its tileset.
    Maps,
    /// Every sound effect (decoded, native rate) and every song (rendered once through) as WAV.
    Audio,
    /// Everything above.
    All,
}

pub fn run(data: &Path, out: &Path, kind: Kind) -> Result<()> {
    refuse_repo_output(out)?;
    fs::create_dir_all(out)?;
    let dir = DataDir::open(data)?;
    let pal = Palette::parse(&dir.read("td.pal")?)?;
    let all = kind == Kind::All;
    if all || kind == Kind::Screens {
        screens(&dir, out)?;
    }
    if all || kind == Kind::Tiles {
        for name in ["td.blk", "td2.blk", "td3.blk"] {
            let ts = Tileset::parse(&dir.read(name)?)?;
            tile_sheet(&ts, &pal, &out.join(format!("{name}.png")))?;
        }
    }
    if all || kind == Kind::Sprites {
        sprites(&dir, &pal, out)?;
    }
    if all || kind == Kind::Maps {
        for (map, blk) in [
            ("td.map", "td.blk"),
            ("td2.map", "td2.blk"),
            ("td3.map", "td3.blk"),
            ("td4.map", "td3.blk"),
            ("intro.map", "td.blk"),
        ] {
            let m = TileMap::parse_any(&dir.read(map)?)?;
            let ts = Tileset::parse(&dir.read(blk)?)?;
            map_png(&m, &ts, &pal, &out.join(format!("{map}.png")))?;
        }
    }
    if all || kind == Kind::Audio {
        audio(&dir, out)?;
    }
    println!("exported to {}", out.display());
    Ok(())
}

/// Refuses to write inside a git work tree (the repository must never receive game content).
fn refuse_repo_output(out: &Path) -> Result<()> {
    let abs = std::path::absolute(out)?;
    let mut p: Option<&Path> = Some(abs.as_path());
    while let Some(dir) = p {
        if dir.join(".git").exists() {
            bail!(
                "refusing to export game content inside a git repository ({}); choose a directory \
                 such as P:\\msdos\\TD2192\\re-work\\scratch",
                dir.display()
            );
        }
        p = dir.parent();
    }
    Ok(())
}

fn screens(dir: &DataDir, out: &Path) -> Result<()> {
    let mut n = 0;
    for e in td_assets::manifest::of_category(td_assets::Category::Screen) {
        let bytes = dir.read(e.name)?;
        if e.name == "td.hlp" {
            let h = PagedScreens::parse(&bytes)?;
            for (i, page) in h.pages.iter().enumerate() {
                write_png(
                    &out.join(format!("td.hlp.{i}.png")),
                    page,
                    320,
                    200,
                    &h.palette,
                )?;
            }
        } else {
            let s = Screen::parse(&bytes).with_context(|| e.name.to_string())?;
            write_png(
                &out.join(format!("{}.png", e.name)),
                &s.pixels,
                320,
                200,
                &s.palette,
            )?;
        }
        n += 1;
    }
    println!("{n} screen files");
    Ok(())
}

fn tile_sheet(ts: &Tileset, pal: &Palette, path: &Path) -> Result<()> {
    const COLS: usize = 10;
    let pitch = TILE_SIZE + 1;
    let rows = ts.tiles.len().div_ceil(COLS);
    let (w, h) = (COLS * pitch, rows * pitch);
    let mut img = vec![0u8; w * h];
    for (i, tile) in ts.tiles.iter().enumerate() {
        let (ox, oy) = ((i % COLS) * pitch, (i / COLS) * pitch);
        for y in 0..TILE_SIZE {
            img[(oy + y) * w + ox..(oy + y) * w + ox + TILE_SIZE]
                .copy_from_slice(&tile[y * TILE_SIZE..(y + 1) * TILE_SIZE]);
        }
    }
    write_png(path, &img, w, h, pal)
}

fn sprites(dir: &DataDir, pal: &Palette, out: &Path) -> Result<()> {
    let base = SpriteSheet::parse_base(&dir.read("td.shp")?)?;
    let sc3 = SpriteSheet::parse_rotation_set(&dir.read("td.sc3")?)?;
    let shr = SpriteSheet::parse_rotation_set(&dir.read("td.shr")?)?;
    let sc2 = SpriteSheet::parse_rotation_set(&dir.read("td.sc2")?)?;
    let hs = HeadingSprites::build(&base, [&sc3, &shr, &sc2], base.len())
        .context("building headings")?;
    let pitch = CELL + 2;
    let (w, h) = (HEADINGS * pitch, hs.len() * pitch);
    let mut img = vec![0u8; w * h];
    for s in 0..hs.len() {
        for hd in 0..HEADINGS {
            let f = hs.frame(s, hd).expect("in range");
            let (ox, oy) = (hd * pitch, s * pitch);
            for y in 0..CELL {
                img[(oy + y) * w + ox..(oy + y) * w + ox + CELL]
                    .copy_from_slice(&f[y * CELL..(y + 1) * CELL]);
            }
        }
    }
    write_png(&out.join("sprites-16-headings.png"), &img, w, h, pal)?;
    for (name, set) in [("td.sc3", &sc3), ("td.shr", &shr), ("td.sc2", &sc2)] {
        let mut img = vec![0u8; set.len() * pitch * CELL];
        let w = set.len() * pitch;
        for i in 0..set.len() {
            let f = set.frame(i).expect("in range");
            for y in 0..CELL {
                img[y * w + i * pitch..y * w + i * pitch + CELL]
                    .copy_from_slice(&f[y * CELL..(y + 1) * CELL]);
            }
        }
        write_png(&out.join(format!("{name}.png")), &img, w, CELL, pal)?;
    }
    Ok(())
}

fn map_png(m: &TileMap, ts: &Tileset, pal: &Palette, path: &Path) -> Result<()> {
    let w = m.size() * TILE_SIZE;
    let mut img = vec![0u8; w * w];
    for ty in 0..m.size() {
        for tx in 0..m.size() {
            let idx = m.get(tx, ty).unwrap_or(0) as usize;
            let Some(tile) = ts.tiles.get(idx) else {
                continue;
            };
            for y in 0..TILE_SIZE {
                let row = (ty * TILE_SIZE + y) * w + tx * TILE_SIZE;
                img[row..row + TILE_SIZE]
                    .copy_from_slice(&tile[y * TILE_SIZE..(y + 1) * TILE_SIZE]);
            }
        }
    }
    write_png(path, &img, w, w, pal)
}

fn audio(dir: &DataDir, out: &Path) -> Result<()> {
    use std::sync::Arc;
    for e in td_assets::manifest::of_category(td_assets::Category::Sfx) {
        let pcm = td_audio::decode_voc(&dir.read(e.name)?).with_context(|| e.name.to_string())?;
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: pcm.rate,
            bits_per_sample: 8,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(out.join(format!("{}.wav", e.name)), spec)?;
        for &s in &pcm.samples {
            w.write_sample((i16::from(s) - 128) as i8)?;
        }
        w.finalize()?;
    }
    for e in td_assets::manifest::of_category(td_assets::Category::Music) {
        let song = Arc::new(td_formats::cmf::Cmf::parse(&dir.read(e.name)?)?);
        let secs = f64::from(song.end_tick) / f64::from(song.ticks_per_second) + 2.0;
        let mut player = td_audio::CmfPlayer::new(song, false);
        let rate = player.sample_rate();
        let mut buf = vec![0i32; (secs * f64::from(rate)) as usize];
        player.render(&mut buf);
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(out.join(format!("{}.wav", e.name)), spec)?;
        for &s in &buf {
            w.write_sample(s.clamp(-32768, 32767) as i16)?;
        }
        w.finalize()?;
    }
    println!("audio exported");
    Ok(())
}

/// Writes an indexed PNG with the palette expanded to 8 bits.
pub fn write_png(path: &Path, pixels: &[u8], w: usize, h: usize, pal: &Palette) -> Result<()> {
    let file = BufWriter::new(File::create(path).with_context(|| path.display().to_string())?);
    let mut enc = png::Encoder::new(file, w as u32, h as u32);
    enc.set_color(png::ColorType::Indexed);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_palette(pal.to_rgb8().concat());
    enc.write_header()?.write_image_data(pixels)?;
    Ok(())
}
