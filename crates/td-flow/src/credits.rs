//! The credits screen: the original's only credits, `file0000.exe`'s `intro.credits`, shown as
//! the intro flyover (mode A) or as grouped static pages in the caption style (mode B).
//!
//! Spec: `docs/spec/frontend/credits.md` (rev 1); `docs/spec/flow/intro.md` (rev 1) §3.5, §4.4,
//! §4.6. Fidelity: mode A as the intro flyover (T0/T1, it is [`Intro::flyover_only`]); mode B is
//! T3 (a proposal of the spec, §4.2).
//!
//! [`Credits`] is a reusable [`Screen`] that pops when it ends (or replaces itself with the
//! screen given to [`Credits::then`]), so a front end can push it from a menu entry. It stops
//! the music when it ends; the caller starts its own song again.

use std::sync::Arc;

use td_assets::{DataDir, ExeResources};
use td_audio::AudioCommand;
use td_formats::cmf::Cmf;
use td_formats::font::{Font, FontKind};
use td_formats::palette::{Palette, Rgb6};
use td_input::{Key, TickInput};
use td_render::intro::{self as draw, TextStyle};
use td_render::Framebuffer;

use crate::intro::{FlyoverAssets, Intro, IntroError, IntroOptions, load_song};
use crate::screen::{FlowContext, Screen, Transition};
use crate::story::is_keystroke;

/// Which presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreditsMode {
    /// A: the intro flyover with its captions, music and effects (credits §4.1; Classic).
    Flyover,
    /// B: one page per group (credits §4.2; Enhanced, or without the flyover's data).
    Pages,
}

/// Caption ink of mode B: the flyover's entry 46 = (0, 63, 63) (credits §4.2).
pub const PAGE_INK: u8 = 46;
/// Frames a page holds without a key: 4 s at 70.086 Hz.
pub const PAGE_HOLD: u32 = 280;

/// The groups of credits §3: heading (`None` for the title group) and names, as indices into
/// `intro.credits`. `C[23]` (blank) is not shown.
pub const GROUPS: [(Option<usize>, &[usize]); 7] = [
    (None, &[0, 1, 2]),
    (Some(3), &[4, 5, 6]),
    (Some(7), &[4, 8, 9, 10]),
    (Some(11), &[5]),
    (Some(12), &[13]),
    (Some(14), &[15, 16, 17, 18, 19, 20, 21, 22]),
    (Some(24), &[4]),
];

/// Mode B's lines: per page, each line's text and Text A pen (credits §4.2): centred at
/// x = 160 − 6 × (len div 2); headings at row 70 (50 for the nine-line thanks page); names 14
/// rows below the heading (84 on the title page), 10 rows apart.
#[must_use]
pub fn page_layout(credits: &[String]) -> Vec<Vec<(String, (i32, i32))>> {
    let text = |k: usize| credits.get(k).cloned().unwrap_or_default();
    GROUPS
        .iter()
        .map(|&(heading, names)| {
            let top = if names.len() + usize::from(heading.is_some()) >= 9 {
                50
            } else {
                70
            };
            let mut lines = Vec::new();
            if let Some(h) = heading {
                let t = text(h);
                lines.push((t.clone(), (draw::caption_x(&t, 160), top)));
            }
            for (i, &n) in names.iter().enumerate() {
                let t = text(n);
                let y = top + 14 + 10 * i as i32;
                lines.push((t.clone(), (draw::caption_x(&t, 160), y)));
            }
            lines
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Start,
    FadeIn(u8),
    Hold(u32),
    FadeOut(u8),
    Done,
}

/// Mode B.
struct Pages {
    pages: Vec<Framebuffer>,
    working: Palette,
    dac: Palette,
    song: Option<Arc<Cmf>>,
    page: usize,
    phase: Phase,
    keys: Vec<Key>,
}

impl Pages {
    fn new(font: &Font, credits: &[String], song: Option<Arc<Cmf>>) -> Self {
        let pages = page_layout(credits)
            .iter()
            .map(|lines| {
                let mut fb = Framebuffer::new();
                for (t, (x, y)) in lines {
                    draw::draw_text(&mut fb, font, TextStyle::A, t, (x - 1, y + 1), 0);
                    draw::draw_text(&mut fb, font, TextStyle::A, t, (*x, *y), PAGE_INK);
                }
                fb
            })
            .collect();
        let mut working = Palette::default();
        working.colors[usize::from(PAGE_INK)] = Rgb6 {
            r: 0,
            g: 63,
            b: 63,
        };
        Self {
            pages,
            working,
            dac: Palette::default(),
            song,
            page: 0,
            phase: Phase::Start,
            keys: Vec::new(),
        }
    }

    fn step(&mut self, input: &TickInput, audio: &mut Vec<AudioCommand>) {
        self.keys
            .extend(input.pressed.iter().copied().filter(|&k| is_keystroke(k)));
        if self.keys.contains(&Key::ESCAPE) && self.phase != Phase::Done {
            // Esc leaves at once (credits §4.2).
            audio.push(AudioCommand::StopMusic);
            self.phase = Phase::Done;
            return;
        }
        self.phase = match self.phase {
            Phase::Start => {
                if let Some(song) = self.song.clone() {
                    audio.push(AudioCommand::PlayMusic {
                        song,
                        looping: true,
                    });
                }
                self.keys.clear();
                draw::full_fade_level(&mut self.dac, &self.working, 0);
                Phase::FadeIn(1)
            }
            Phase::FadeIn(l) => {
                draw::full_fade_level(&mut self.dac, &self.working, l);
                if l < 64 {
                    Phase::FadeIn(l + 1)
                } else {
                    self.keys.clear();
                    Phase::Hold(PAGE_HOLD)
                }
            }
            Phase::Hold(n) => {
                if n == 0 || !self.keys.is_empty() {
                    self.keys.clear();
                    draw::full_fade_level(&mut self.dac, &self.working, 64);
                    Phase::FadeOut(63)
                } else {
                    Phase::Hold(n - 1)
                }
            }
            Phase::FadeOut(l) => {
                draw::full_fade_level(&mut self.dac, &self.working, l);
                if l > 0 {
                    Phase::FadeOut(l - 1)
                } else if self.page + 1 < self.pages.len() {
                    self.page += 1;
                    Phase::FadeIn(0)
                } else {
                    audio.push(AudioCommand::StopMusic);
                    Phase::Done
                }
            }
            Phase::Done => Phase::Done,
        };
    }

    fn draw(&self, fb: &mut Framebuffer) {
        if let Some(p) = self.pages.get(self.page) {
            fb.pixels.copy_from_slice(&p.pixels);
        }
        fb.palette.clone_from(&self.dac);
    }
}

enum Inner {
    Flyover(Box<Intro>),
    Pages(Pages),
}

/// The credits screen.
pub struct Credits {
    inner: Inner,
    then: Option<Box<dyn Screen>>,
}

impl std::fmt::Debug for Credits {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credits")
            .field("mode", &self.mode())
            .finish_non_exhaustive()
    }
}

impl Credits {
    /// Loads the credits in `mode`. Mode A falls back to mode B when the flyover's files are
    /// missing (credits §4.2). Fails only if `td.lt2` cannot be read.
    pub fn load(
        data: &DataDir,
        res: &ExeResources,
        mode: CreditsMode,
        options: IntroOptions,
    ) -> Result<Self, IntroError> {
        if mode == CreditsMode::Flyover {
            match FlyoverAssets::load(data, res) {
                Ok(a) => return Ok(Self::flyover(a, options)),
                Err(e) => tracing::warn!("credits flyover unavailable ({e}); showing pages"),
            }
        }
        let font = Font::parse(&data.read("td.lt2")?, FontKind::Small)?;
        let credits = res
            .texts("intro.credits")
            .map(<[String]>::to_vec)
            .unwrap_or_default();
        Ok(Self::pages(&font, &credits, load_song(data, "intro2.cmf")))
    }

    /// Mode A from loaded assets.
    #[must_use]
    pub fn flyover(assets: FlyoverAssets, options: IntroOptions) -> Self {
        Self {
            inner: Inner::Flyover(Box::new(Intro::flyover_only(assets, options))),
            then: None,
        }
    }

    /// Mode B: `credits` is `intro.credits`, `song` `intro2.cmf` (looping).
    #[must_use]
    pub fn pages(font: &Font, credits: &[String], song: Option<Arc<Cmf>>) -> Self {
        Self {
            inner: Inner::Pages(Pages::new(font, credits, song)),
            then: None,
        }
    }

    /// Replaces this screen with `next` when it ends (instead of popping).
    #[must_use]
    pub fn then(mut self, next: Box<dyn Screen>) -> Self {
        self.then = Some(next);
        self
    }

    /// The presentation in use.
    #[must_use]
    pub fn mode(&self) -> CreditsMode {
        match self.inner {
            Inner::Flyover(_) => CreditsMode::Flyover,
            Inner::Pages(_) => CreditsMode::Pages,
        }
    }

    /// True once the screen has ended.
    #[must_use]
    pub fn finished(&self) -> bool {
        match &self.inner {
            Inner::Flyover(i) => i.finished(),
            Inner::Pages(p) => p.phase == Phase::Done,
        }
    }

    /// Mode B's current page (0..7), if in mode B.
    #[must_use]
    pub fn page(&self) -> Option<usize> {
        match &self.inner {
            Inner::Flyover(_) => None,
            Inner::Pages(p) => Some(p.page),
        }
    }

    /// Advances one frame.
    pub fn step(&mut self, input: &TickInput, audio: &mut Vec<AudioCommand>) {
        match &mut self.inner {
            Inner::Flyover(i) => i.step(input, audio),
            Inner::Pages(p) => p.step(input, audio),
        }
    }
}

impl Screen for Credits {
    fn tick(&mut self, ctx: &mut FlowContext<'_>, input: &TickInput) -> Transition {
        self.step(input, ctx.audio);
        if !self.finished() {
            return Transition::None;
        }
        match self.then.take() {
            Some(next) => Transition::Replace(next),
            None => Transition::Pop,
        }
    }

    fn draw(&self, fb: &mut Framebuffer) {
        match &self.inner {
            Inner::Flyover(i) => i.draw(fb),
            Inner::Pages(p) => p.draw(fb),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn credits() -> Vec<String> {
        let mut c: Vec<String> = (0..25).map(|k| format!("name {k:02}")).collect();
        c[3] = "a".repeat(15);
        c
    }

    fn font() -> Font {
        let mut data = vec![0u8; FontKind::Small.file_len()];
        for g in data.chunks_exact_mut(25) {
            g[0] = 1;
        }
        Font::parse(&data, FontKind::Small).unwrap()
    }

    #[test]
    fn spec_vector_2_page_layout() {
        let pages = page_layout(&credits());
        assert_eq!(pages.len(), 7);
        // Page 2: heading C[3] (15 characters) at (160 − 6 × 7, 70) = (118, 70); names at
        // rows 84, 94, 104.
        assert_eq!(pages[1][0].1, (118, 70));
        let rows: Vec<i32> = pages[1][1..].iter().map(|l| l.1.1).collect();
        assert_eq!(rows, [84, 94, 104]);
        // Title page: no heading, first line at 84.
        assert_eq!(pages[0][0].1.1, 84);
        // Thanks: nine lines from row 50.
        assert_eq!(pages[5].len(), 9);
        assert_eq!((pages[5][0].1.1, pages[5][8].1.1), (50, 134));
        // C[23] is never shown.
        assert!(pages.iter().flatten().all(|l| l.0 != "name 23"));
    }

    #[test]
    fn pages_fade_hold_and_end() {
        let mut c = Credits::pages(&font(), &credits(), None);
        assert_eq!(c.mode(), CreditsMode::Pages);
        let mut audio = Vec::new();
        let none = TickInput::default();
        // Start, 64 more fade-in levels, the hold, 65 fade-out levels: one page.
        let per_page = 1 + 64 + PAGE_HOLD as usize + 1 + 65;
        for _ in 0..per_page {
            c.step(&none, &mut audio);
        }
        assert_eq!(c.page(), Some(1));
        let mut fb = Framebuffer::new();
        c.draw(&mut fb);
        assert_eq!(fb.palette.colors[usize::from(PAGE_INK)], Rgb6::default());
        // A key ends the hold.
        for _ in 0..70 {
            c.step(&none, &mut audio);
        }
        let space = TickInput {
            pressed: vec![Key::SPACE],
            ..TickInput::default()
        };
        c.step(&space, &mut audio);
        for _ in 0..66 {
            c.step(&none, &mut audio);
        }
        assert_eq!(c.page(), Some(2));
        // Esc leaves at once.
        c.step(
            &TickInput {
                pressed: vec![Key::ESCAPE],
                ..TickInput::default()
            },
            &mut audio,
        );
        assert!(c.finished());
        assert!(matches!(audio.last(), Some(AudioCommand::StopMusic)));
    }

    #[test]
    fn pages_draw_text_a_centred() {
        let mut c = Credits::pages(&font(), &credits(), None);
        let mut audio = Vec::new();
        for _ in 0..70 {
            c.step(&TickInput::default(), &mut audio);
        }
        let mut fb = Framebuffer::new();
        c.draw(&mut fb);
        // "name 00" (7 characters): pen 160 − 18 = 142, row 84; Text A puts 'n' at offset 2,
        // its first pixel at (142 + 2 + 1, 85).
        assert_eq!(fb.get(145, 85), Some(PAGE_INK));
        assert_eq!(fb.palette.colors[usize::from(PAGE_INK)].g, 63);
    }
}
