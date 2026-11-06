//! Developer map viewer (M4-06): scroll a city map, preview sprites at every heading, and
//! audition music and sound effects. Not part of the game; it exercises the platform layer.
//!
//! Keys: arrows scroll (Shift = faster), PgUp/PgDn change sprite, Space pauses rotation,
//! M next song, S next sound effect, 1-6 change map, F10 scale mode, F11 fullscreen,
//! F12 screenshot, Esc quit.

use std::sync::Arc;

use anyhow::{Context, Result, bail};
use td_assets::DataDir;
use td_audio::{AudioCommand, Pcm8, decode_voc};
use td_flow::{FlowContext, Screen, Transition};
use td_formats::cmf::Cmf;
use td_formats::map::TileMap;
use td_formats::palette::Palette;
use td_formats::sprites::SpriteSheet;
use td_formats::tileset::{TILE_SIZE, Tileset};
use td_input::Key;
use td_input::TickInput;
use td_render::headings::CELL;
use td_render::{Framebuffer, HeadingSprites, Image, Rect, draw_tile_map};

const MAPS: [(&str, &str, &str); 6] = [
    ("e1", "td.map", "td.blk"),
    ("e2a", "td.map", "td2.blk"),
    ("e2b", "td2.map", "td2.blk"),
    ("e3a", "td3.map", "td3.blk"),
    ("e3b", "td4.map", "td3.blk"),
    ("intro", "intro.map", "td.blk"),
];

const SONGS: [&str; 17] = [
    "menu.cmf",
    "td01.cmf",
    "td02.cmf",
    "td03.cmf",
    "hangar.cmf",
    "satair.cmf",
    "vulture.cmf",
    "mood.cmf",
    "mood2.cmf",
    "moodmaj.cmf",
    "low.cmf",
    "climax.cmf",
    "death.cmf",
    "intro.cmf",
    "intro1.cmf",
    "intro2.cmf",
    "mood3.cmf",
];

const SOUNDS: [&str; 17] = [
    "afterb.voc",
    "boomboom.voc",
    "home.voc",
    "hornet.voc",
    "income.voc",
    "missile.voc",
    "poink.voc",
    "pulse.voc",
    "punyexp.voc",
    "ric2.voc",
    "ric3.voc",
    "scratch.voc",
    "stiletto.voc",
    "vii.voc",
    "vulturei.voc",
    "whoosh.voc",
    "zeus.voc",
];

#[derive(Debug)]
pub struct MapView {
    maps: Vec<(TileMap, Tileset)>,
    current: usize,
    palette: Palette,
    sprites: HeadingSprites,
    songs: Vec<Arc<Cmf>>,
    sounds: Vec<Arc<Pcm8>>,
    cam: (i32, i32),
    sprite: usize,
    heading: usize,
    rotate: bool,
    tick: u64,
    song: Option<usize>,
    sound: usize,
}

impl MapView {
    pub fn new(dir: &DataDir, start: &str) -> Result<Self> {
        let current = MAPS
            .iter()
            .position(|(n, _, _)| *n == start)
            .with_context(|| format!("unknown map {start:?} (e1, e2a, e2b, e3a, e3b, intro)"))?;
        let mut maps = Vec::new();
        for (_, map, blk) in MAPS {
            maps.push((
                TileMap::parse_any(&dir.read(map)?)?,
                Tileset::parse(&dir.read(blk)?)?,
            ));
        }
        let palette = Palette::parse(&dir.read("td.pal")?)?;
        let base = SpriteSheet::parse_base(&dir.read("td.shp")?)?;
        let sc3 = SpriteSheet::parse_rotation_set(&dir.read("td.sc3")?)?;
        let shr = SpriteSheet::parse_rotation_set(&dir.read("td.shr")?)?;
        let sc2 = SpriteSheet::parse_rotation_set(&dir.read("td.sc2")?)?;
        let Some(sprites) = HeadingSprites::build(&base, [&sc3, &shr, &sc2], base.len()) else {
            bail!("sprite sheets are inconsistent");
        };
        let songs = SONGS
            .iter()
            .map(|n| Ok(Arc::new(Cmf::parse(&dir.read(n)?)?)))
            .collect::<Result<Vec<_>>>()?;
        let sounds = SOUNDS
            .iter()
            .map(|n| Ok(Arc::new(decode_voc(&dir.read(n)?)?)))
            .collect::<Result<Vec<_>>>()?;
        let size = maps[current].0.size() as i32 * TILE_SIZE as i32;
        Ok(Self {
            maps,
            current,
            palette,
            sprites,
            songs,
            sounds,
            cam: (size / 2 - 160, size / 2 - 100),
            sprite: 1,
            heading: 0,
            rotate: true,
            tick: 0,
            song: None,
            sound: 0,
        })
    }
}

impl Screen for MapView {
    fn tick(&mut self, ctx: &mut FlowContext<'_>, input: &TickInput) -> Transition {
        let audio = &mut *ctx.audio;
        self.tick += 1;
        if input.pressed(Key::ESCAPE) {
            return Transition::Quit;
        }
        let speed = if input.held(Key::LSHIFT) || input.held(Key::RSHIFT) {
            16
        } else {
            4
        };
        if input.held(Key::LEFT) {
            self.cam.0 -= speed;
        }
        if input.held(Key::RIGHT) {
            self.cam.0 += speed;
        }
        if input.held(Key::UP) {
            self.cam.1 -= speed;
        }
        if input.held(Key::DOWN) {
            self.cam.1 += speed;
        }
        if input.pressed(Key::PAGE_DOWN) {
            self.sprite = (self.sprite + 1) % self.sprites.len();
        }
        if input.pressed(Key::PAGE_UP) {
            self.sprite = (self.sprite + self.sprites.len() - 1) % self.sprites.len();
        }
        if input.pressed(Key::SPACE) {
            self.rotate = !self.rotate;
        }
        if self.rotate && self.tick.is_multiple_of(6) {
            self.heading = (self.heading + 1) % td_render::HEADINGS;
        }
        for d in 1..=6u8 {
            if Key::digit(d).is_some_and(|k| input.pressed(k)) {
                self.current = usize::from(d) - 1;
            }
        }
        if Key::letter('m').is_some_and(|k| input.pressed(k)) {
            let next = self.song.map_or(0, |s| (s + 1) % self.songs.len());
            self.song = Some(next);
            audio.push(AudioCommand::PlayMusic {
                song: Arc::clone(&self.songs[next]),
                looping: true,
            });
        }
        if Key::letter('s').is_some_and(|k| input.pressed(k)) {
            // Mixer level 11: what an unpositioned flight effect gets (15 x 12 div 16).
            audio.push(AudioCommand::PlaySfx {
                sound: Arc::clone(&self.sounds[self.sound]),
                level: 11,
            });
            self.sound = (self.sound + 1) % self.sounds.len();
        }
        Transition::None
    }

    fn draw(&self, fb: &mut Framebuffer) {
        fb.palette = self.palette.clone();
        let (map, tiles) = &self.maps[self.current];
        draw_tile_map(fb, map, tiles, self.cam.0, self.cam.1, Rect::SCREEN, 0);
        if let Some(frame) = self.sprites.frame(self.sprite, self.heading) {
            let at = (160 - CELL as i32 / 2, 100 - CELL as i32 / 2);
            fb.blit(
                Image::new(frame, CELL, CELL),
                at.0,
                at.1,
                Rect::SCREEN,
                Some(0),
            );
        }
    }
}
