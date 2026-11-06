//! The port's settings screens (M11-03): video, audio, controls, profile and accessibility.
//!
//! Reached from the start screen and, during play, with the host's options key (F2). Every
//! change goes to the host through the [`SettingsPort`] at once; the host saves
//! `settings.toml` and applies what it can live (window, volumes, SFX policy, Enhanced
//! presentation; the flight reads the control bindings every tick).
//!
//! The Classic profile forces every Enhanced feature off (MDG §6.2), so their rows are shown
//! but cannot be changed until the profile is Enhanced; the choices made under Enhanced are
//! kept. Widescreen is marked "affects gameplay - visibility" (spec enhanced/presentation §2).
//!
//! Keys: arrows (or keypad 8/2/4/6) move and change, Enter or Space opens or changes, Esc or
//! Backspace goes back, Delete clears a binding. Controller: stick or D-pad, South confirms,
//! East goes back (M11-04).

use td_input::{Key, PadButton, TickInput};
use td_render::Framebuffer;
use td_sim::FlightKey;

use super::SettingsPort;
use super::ui::{
    Fader, INK_CURSOR, INK_DISABLED, INK_HEADING, INK_NOTE, INK_TEXT, INK_VALUE, Nav, PadNav,
    UiAssets, nav,
};
use crate::controls::{KeyBindings, PadBindings, flight_key_action, flight_key_name};
use crate::screen::{FlowContext, Screen, Transition};
use crate::settings::{Lighting, Profile, Scale, Settings, SfxMode, Upscaler, Widescreen};

/// A page of the options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Main,
    Video,
    Audio,
    Controls,
    Profile,
    Accessibility,
}

/// A row of a page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    Open(Page),
    Fullscreen,
    Scale,
    Widescreen,
    Upscaler,
    Lighting,
    Bloom,
    MusicVolume,
    SfxVolume,
    SfxMode,
    Profile,
    ColorblindGuidance,
    Binding(FlightKey),
    ResetControls,
    Back,
}

const MAIN_ROWS: [Row; 6] = [
    Row::Open(Page::Video),
    Row::Open(Page::Audio),
    Row::Open(Page::Controls),
    Row::Open(Page::Profile),
    Row::Open(Page::Accessibility),
    Row::Back,
];
const VIDEO_ROWS: [Row; 7] = [
    Row::Fullscreen,
    Row::Scale,
    Row::Widescreen,
    Row::Upscaler,
    Row::Lighting,
    Row::Bloom,
    Row::Back,
];
const AUDIO_ROWS: [Row; 4] = [Row::MusicVolume, Row::SfxVolume, Row::SfxMode, Row::Back];
const PROFILE_ROWS: [Row; 2] = [Row::Profile, Row::Back];
const ACCESSIBILITY_ROWS: [Row; 2] = [Row::ColorblindGuidance, Row::Back];

/// The rows of `page`, top to bottom.
pub fn rows(page: Page) -> Vec<Row> {
    match page {
        Page::Main => MAIN_ROWS.to_vec(),
        Page::Video => VIDEO_ROWS.to_vec(),
        Page::Audio => AUDIO_ROWS.to_vec(),
        Page::Controls => FlightKey::ALL
            .into_iter()
            .map(Row::Binding)
            .chain([Row::ResetControls, Row::Back])
            .collect(),
        Page::Profile => PROFILE_ROWS.to_vec(),
        Page::Accessibility => ACCESSIBILITY_ROWS.to_vec(),
    }
}

fn page_name(page: Page) -> &'static str {
    match page {
        Page::Main => "options",
        Page::Video => "video",
        Page::Audio => "audio",
        Page::Controls => "controls",
        Page::Profile => "profile",
        Page::Accessibility => "accessibility",
    }
}

/// True for the rows of Enhanced features (MDG §6.2), which the Classic profile forces off.
pub fn is_enhanced(row: Row) -> bool {
    matches!(
        row,
        Row::Widescreen
            | Row::Upscaler
            | Row::Lighting
            | Row::Bloom
            | Row::SfxMode
            | Row::ColorblindGuidance
    )
}

/// True if `row` can be chosen or changed with `s`.
pub fn row_enabled(row: Row, s: &Settings) -> bool {
    !is_enhanced(row) || s.profile == Profile::Enhanced
}

fn label(row: Row) -> String {
    match row {
        Row::Open(p) => page_name(p).to_string(),
        Row::Fullscreen => "fullscreen".into(),
        Row::Scale => "scaling".into(),
        Row::Widescreen => "widescreen".into(),
        Row::Upscaler => "pixel upscaler".into(),
        Row::Lighting => "dynamic lighting".into(),
        Row::Bloom => "bloom".into(),
        Row::MusicVolume => "music volume".into(),
        Row::SfxVolume => "sound volume".into(),
        Row::SfxMode => "sound effects".into(),
        Row::Profile => "profile".into(),
        Row::ColorblindGuidance => "colour-blind guidance line".into(),
        Row::Binding(k) => flight_key_action(k).into(),
        Row::ResetControls => "reset to defaults".into(),
        Row::Back => "back".into(),
    }
}

fn on_off(b: bool) -> String {
    if b { "on" } else { "off" }.into()
}

/// The value shown for `row`.
pub fn value(row: Row, s: &Settings) -> Option<String> {
    Some(match row {
        Row::Fullscreen => on_off(s.video.fullscreen),
        Row::Scale => match s.video.scale {
            Scale::Aspect => "aspect 4x3",
            Scale::Integer => "integer",
            Scale::Stretch => "stretch",
        }
        .into(),
        Row::Widescreen => match s.enhanced.widescreen {
            Widescreen::Off => "off",
            Widescreen::W16x10 => "16x10",
            Widescreen::W16x9 => "16x9",
            Widescreen::W21x9 => "21x9",
            Widescreen::Auto => "auto",
        }
        .into(),
        Row::Upscaler => match s.enhanced.upscaler {
            Upscaler::None => "none",
            Upscaler::Scale2x => "scale2x",
            Upscaler::Scale3x => "scale3x",
            Upscaler::Scale4x => "scale4x",
            Upscaler::Xbr2 => "xbr2",
            Upscaler::Xbr3 => "xbr3",
            Upscaler::Xbr4 => "xbr4",
        }
        .into(),
        Row::Lighting => match s.enhanced.lighting {
            Lighting::Off => "off",
            Lighting::Subtle => "subtle",
            Lighting::Full => "full",
        }
        .into(),
        Row::Bloom => on_off(s.enhanced.bloom),
        Row::MusicVolume => percent(s.audio.music_volume).to_string(),
        Row::SfxVolume => percent(s.audio.sfx_volume).to_string(),
        Row::SfxMode => match s.audio.sfx_mode {
            SfxMode::Classic => "classic",
            SfxMode::Polyphonic => "polyphonic",
        }
        .into(),
        Row::Profile => match s.profile {
            Profile::Classic => "classic",
            Profile::Enhanced => "enhanced",
        }
        .into(),
        Row::ColorblindGuidance => on_off(s.accessibility.colorblind_guidance),
        Row::Open(_) | Row::Binding(_) | Row::ResetControls | Row::Back => return None,
    })
}

/// The note shown while `row` is selected.
fn note(row: Row) -> Option<&'static str> {
    match row {
        Row::Widescreen => Some("affects gameplay - visibility"),
        Row::Scale => Some("aspect keeps the original 4x3 shape"),
        Row::Upscaler => Some("smooths the pixel art"),
        Row::SfxMode => Some("polyphonic plays several effects at once"),
        Row::ColorblindGuidance => Some("not drawn yet - saved for a later version"),
        _ => None,
    }
}

fn percent(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 100.0).round() as u8
}

/// `v` one tenth up or down, on the tenths.
fn step_volume(v: f32, dir: i8) -> f32 {
    let tenths = ((v.clamp(0.0, 1.0) * 10.0).round() as i8 + dir).clamp(0, 10);
    f32::from(tenths) / 10.0
}

fn cycle<T: Copy + PartialEq>(all: &[T], v: T, dir: i8) -> T {
    let n = all.len() as isize;
    let i = all.iter().position(|&x| x == v).unwrap_or(0) as isize;
    all[(i + isize::from(dir)).rem_euclid(n) as usize]
}

/// Changes `row` one step in `dir` (+1 or -1); false if it has no value or is disabled.
pub fn change(row: Row, s: &mut Settings, dir: i8) -> bool {
    if !row_enabled(row, s) {
        return false;
    }
    match row {
        Row::Fullscreen => s.video.fullscreen = !s.video.fullscreen,
        Row::Scale => {
            s.video.scale = cycle(
                &[Scale::Aspect, Scale::Integer, Scale::Stretch],
                s.video.scale,
                dir,
            );
        }
        Row::Widescreen => {
            s.enhanced.widescreen = cycle(&Widescreen::ALL, s.enhanced.widescreen, dir);
        }
        Row::Upscaler => s.enhanced.upscaler = cycle(&Upscaler::ALL, s.enhanced.upscaler, dir),
        Row::Lighting => s.enhanced.lighting = cycle(&Lighting::ALL, s.enhanced.lighting, dir),
        Row::Bloom => s.enhanced.bloom = !s.enhanced.bloom,
        Row::MusicVolume => s.audio.music_volume = step_volume(s.audio.music_volume, dir),
        Row::SfxVolume => s.audio.sfx_volume = step_volume(s.audio.sfx_volume, dir),
        Row::SfxMode => {
            s.audio.sfx_mode = cycle(
                &[SfxMode::Classic, SfxMode::Polyphonic],
                s.audio.sfx_mode,
                dir,
            );
        }
        Row::Profile => {
            s.profile = cycle(&[Profile::Classic, Profile::Enhanced], s.profile, dir);
        }
        Row::ColorblindGuidance => {
            s.accessibility.colorblind_guidance = !s.accessibility.colorblind_guidance;
        }
        Row::Open(_) | Row::Binding(_) | Row::ResetControls | Row::Back => return false,
    }
    true
}

/// Which binding column of the controls page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Column {
    Keyboard,
    Gamepad,
}

/// Where the "affects gameplay" mark of the widescreen row starts.
const GAMEPLAY_MARK_X: i32 = 240;

/// Frames a binding capture waits for a key or a button (5 s).
const CAPTURE_FRAMES: u32 = 350;
/// Frames a message stays (3 s).
const MESSAGE_FRAMES: u32 = 210;

/// The options screen.
#[derive(Debug)]
pub struct OptionsScreen {
    ui: UiAssets,
    settings: Settings,
    /// `ctx.settings` as last seen (changes made elsewhere are taken over).
    seen: Settings,
    port: SettingsPort,
    page: Page,
    cursor: usize,
    column: Column,
    /// A binding being captured, and the frames left.
    capture: Option<(FlightKey, Column, u32)>,
    message: Option<(String, u32)>,
    fader: Fader,
    leaving: bool,
    pad: PadNav,
    started: bool,
}

impl OptionsScreen {
    /// The options showing `settings`; changes go to `port`.
    pub fn new(ui: UiAssets, settings: Settings, port: SettingsPort) -> Self {
        port.set_open(true);
        Self {
            ui,
            seen: settings.clone(),
            settings,
            port,
            page: Page::Main,
            cursor: 0,
            column: Column::Keyboard,
            capture: None,
            message: None,
            fader: Fader::fading_in(),
            leaving: false,
            pad: PadNav::default(),
            started: false,
        }
    }

    /// The page shown.
    pub fn page(&self) -> Page {
        self.page
    }

    /// The selected row.
    pub fn row(&self) -> Row {
        rows(self.page)[self.cursor]
    }

    /// The settings as shown.
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// The binding column selected on the controls page.
    pub fn column(&self) -> Column {
        self.column
    }

    /// True while a binding waits for its key or button.
    pub fn capturing(&self) -> bool {
        self.capture.is_some()
    }

    /// True once the screen is faded in and takes input.
    pub fn waiting(&self) -> bool {
        !self.leaving && self.fader.done()
    }

    fn submit(&mut self) {
        self.port.submit(self.settings.clone());
    }

    fn say(&mut self, text: String) {
        self.message = Some((text, MESSAGE_FRAMES));
    }

    fn move_cursor(&mut self, dir: isize) {
        let rows = rows(self.page);
        let n = rows.len() as isize;
        let mut i = self.cursor as isize;
        for _ in 0..rows.len() {
            i = (i + dir).rem_euclid(n);
            if row_enabled(rows[i as usize], &self.settings) {
                break;
            }
        }
        self.cursor = i as usize;
    }

    fn open(&mut self, page: Page) {
        self.page = page;
        self.cursor = 0;
        self.column = Column::Keyboard;
        if !row_enabled(self.row(), &self.settings) {
            self.move_cursor(1);
        }
    }

    fn back(&mut self) {
        if self.page == Page::Main {
            self.leaving = true;
            self.fader.fade_out();
        } else {
            let from = Row::Open(self.page);
            self.page = Page::Main;
            self.cursor = MAIN_ROWS.iter().position(|&r| r == from).unwrap_or(0);
        }
    }

    fn bind(&mut self, k: FlightKey, column: Column, v: Option<BindingInput>) {
        let lost = match (column, v) {
            (Column::Keyboard, Some(BindingInput::Key(key))) => {
                self.settings.controls.keyboard.bind(k, Some(key))
            }
            (Column::Gamepad, Some(BindingInput::Button(b))) => {
                self.settings.controls.gamepad.bind(k, Some(b))
            }
            (Column::Keyboard, None) => self.settings.controls.keyboard.bind(k, None),
            (Column::Gamepad, None) => self.settings.controls.gamepad.bind(k, None),
            _ => return,
        };
        if let Some(other) = lost {
            self.say(format!(
                "taken from {} - {}",
                flight_key_action(other),
                flight_key_name(other)
            ));
        }
        self.submit();
    }

    /// A binding capture's tick.
    fn capture_tick(&mut self, input: &TickInput, pressed: &[PadButton]) {
        let Some((k, column, left)) = self.capture else {
            return;
        };
        let cancel = |s: &mut Self| s.capture = None;
        match column {
            Column::Gamepad => {
                if let Some(&b) = pressed.first() {
                    self.capture = None;
                    self.bind(k, column, Some(BindingInput::Button(b)));
                    return;
                }
                if input.pressed.contains(&Key::ESCAPE) {
                    cancel(self);
                    return;
                }
            }
            Column::Keyboard => {
                if pressed.contains(&PadButton::East) {
                    cancel(self);
                    return;
                }
                // A tick with a button press may carry the keys its binding stands for.
                if let (true, Some(&key)) = (pressed.is_empty(), input.pressed.first()) {
                    self.capture = None;
                    if key != Key::ESCAPE {
                        self.bind(k, column, Some(BindingInput::Key(key)));
                    }
                    return;
                }
            }
        }
        self.capture = (left > 1).then_some((k, column, left - 1));
    }
}

#[derive(Debug, Clone, Copy)]
enum BindingInput {
    Key(Key),
    Button(PadButton),
}

impl Screen for OptionsScreen {
    fn tick(&mut self, ctx: &mut FlowContext<'_>, input: &TickInput) -> Transition {
        if !self.started {
            // The key or button that opened the screen is not taken as its own.
            self.started = true;
            self.pad = PadNav::starting_with(input.pad);
        }
        if *ctx.settings != self.seen {
            self.seen = ctx.settings.clone();
            self.settings = ctx.settings.clone();
        }
        if let Some((_, left)) = &mut self.message {
            *left -= 1;
            if *left == 0 {
                self.message = None;
            }
        }
        let step = self.pad.update(input.pad);
        if self.leaving {
            if self.fader.step() {
                self.port.set_open(false);
                return Transition::Pop;
            }
            return Transition::None;
        }
        if !self.fader.step() {
            return Transition::None;
        }
        if self.capture.is_some() {
            self.capture_tick(input, &step.pressed);
            return Transition::None;
        }
        let row = self.row();
        if let Row::Binding(k) = row
            && step.pressed.is_empty()
            && input.pressed.contains(&Key::DELETE)
        {
            self.bind(k, self.column, None);
            return Transition::None;
        }
        match nav(input, &step) {
            Some(Nav::Up) => self.move_cursor(-1),
            Some(Nav::Down) => self.move_cursor(1),
            Some(Nav::Back) => self.back(),
            Some(d @ (Nav::Left | Nav::Right)) => {
                let dir = if d == Nav::Left { -1 } else { 1 };
                if let Row::Binding(_) = row {
                    self.column = match self.column {
                        Column::Keyboard => Column::Gamepad,
                        Column::Gamepad => Column::Keyboard,
                    };
                } else if change(row, &mut self.settings, dir) {
                    self.submit();
                }
            }
            Some(Nav::Confirm) => match row {
                Row::Open(p) => self.open(p),
                Row::Back => self.back(),
                Row::Binding(k) => {
                    self.message = None;
                    self.capture = Some((k, self.column, CAPTURE_FRAMES));
                }
                Row::ResetControls => {
                    self.settings.controls.keyboard = KeyBindings::default();
                    self.settings.controls.gamepad = PadBindings::default();
                    self.say("controls reset to the defaults".into());
                    self.submit();
                }
                _ => {
                    if change(row, &mut self.settings, 1) {
                        self.submit();
                    }
                }
            },
            None => {}
        }
        Transition::None
    }

    fn draw(&self, fb: &mut Framebuffer) {
        fb.clear(0);
        match self.page {
            Page::Controls => self.draw_controls(fb),
            _ => self.draw_list(fb),
        }
        self.fader.apply(&mut fb.palette, &self.ui.palette);
    }
}

impl OptionsScreen {
    fn draw_list(&self, fb: &mut Framebuffer) {
        let ui = &self.ui;
        let s = &self.settings;
        let main = self.page == Page::Main;
        ui.heading(fb, page_name(self.page), if main { 24 } else { 16 });
        let (x, y0, vx) = if main { (128, 64, 0) } else { (48, 48, 184) };
        let rows = rows(self.page);
        for (i, &row) in rows.iter().enumerate() {
            let y = y0 + i as i32 * 13;
            let enabled = row_enabled(row, s);
            let ink = if !enabled {
                INK_DISABLED
            } else if i == self.cursor {
                INK_CURSOR
            } else {
                INK_TEXT
            };
            ui.text(fb, &label(row), (x, y), ink);
            if let Some(v) = value(row, s) {
                let end = ui.text(
                    fb,
                    &v,
                    (vx, y),
                    if enabled { INK_VALUE } else { INK_DISABLED },
                );
                if row == Row::Widescreen {
                    // Marked on its row at all times (MDG §6.2, spec enhanced/presentation §2).
                    let ink = if enabled { INK_NOTE } else { INK_DISABLED };
                    ui.text(fb, "affects gameplay", (GAMEPLAY_MARK_X, y), ink);
                }
                if matches!(row, Row::MusicVolume | Row::SfxVolume) {
                    let filled = i32::from(percent(match row {
                        Row::MusicVolume => s.audio.music_volume,
                        _ => s.audio.sfx_volume,
                    })) / 2;
                    let bx = end.max(vx + 16) + 4;
                    for dx in 0..50 {
                        let ink = if dx < filled { INK_VALUE } else { INK_DISABLED };
                        for dy in 2..5 {
                            fb.put(bx + dx, y + dy, ink);
                        }
                    }
                }
            }
        }
        if main {
            let profile = value(Row::Profile, s).unwrap_or_default();
            ui.text_centred(fb, &format!("profile - {profile}"), 148, INK_NOTE);
        } else {
            if let Some(n) = note(self.row()) {
                ui.text_centred(fb, n, 148, INK_NOTE);
            }
            if s.profile == Profile::Classic && rows.iter().any(|&r| is_enhanced(r)) {
                ui.text_centred(
                    fb,
                    "enhanced options need the enhanced profile",
                    158,
                    INK_NOTE,
                );
            }
            if self.page == Page::Profile {
                let lines: &[&str] = match s.profile {
                    Profile::Classic => &["the original game", "every enhanced option is off"],
                    Profile::Enhanced => &[
                        "the enhanced options you turn on",
                        "in video, audio and accessibility",
                    ],
                };
                for (i, l) in lines.iter().enumerate() {
                    ui.text_centred(fb, l, 92 + i as i32 * 10, INK_TEXT);
                }
            }
        }
        if let Some((m, _)) = &self.message {
            ui.text_centred(fb, m, 162, INK_HEADING);
        }
        let hint = if main {
            "enter opens - escape goes back"
        } else {
            "left and right change - escape goes back"
        };
        ui.text_centred(fb, hint, 176, INK_NOTE);
    }

    fn draw_controls(&self, fb: &mut Framebuffer) {
        let ui = &self.ui;
        let s = &self.settings;
        ui.heading(fb, "controls", 2);
        const X: [i32; 4] = [8, 84, 152, 232];
        for (x, h) in X.iter().zip(["action", "original", "keyboard", "gamepad"]) {
            ui.text(fb, h, (*x, 18), INK_HEADING);
        }
        for (i, &row) in rows(Page::Controls).iter().enumerate() {
            let y = 27 + i as i32 * 7;
            let selected = i == self.cursor;
            let ink = if selected { INK_CURSOR } else { INK_TEXT };
            ui.text(fb, &label(row), (X[0], y), ink);
            let Row::Binding(k) = row else { continue };
            ui.text(fb, flight_key_name(k), (X[1], y), INK_TEXT);
            for (col, x) in [(Column::Keyboard, X[2]), (Column::Gamepad, X[3])] {
                let capturing = matches!(self.capture, Some((ck, cc, _)) if ck == k && cc == col);
                let bound = match col {
                    Column::Keyboard => s.controls.keyboard.get(k).map(|v| v.name()),
                    Column::Gamepad => s.controls.gamepad.get(k).map(|v| v.name().to_string()),
                };
                let (text, ink) = if capturing {
                    let t = match col {
                        Column::Keyboard => "press a key",
                        Column::Gamepad => "press a button",
                    };
                    (t.to_string(), INK_HEADING)
                } else {
                    let ink = if selected && self.column == col {
                        INK_CURSOR
                    } else if bound.is_some() {
                        INK_VALUE
                    } else {
                        INK_DISABLED
                    };
                    (bound.unwrap_or_else(|| "-".into()), ink)
                };
                ui.text(fb, &text, (x, y), ink);
            }
        }
        let footer = if let Some((m, _)) = &self.message {
            (m.as_str(), INK_HEADING)
        } else if self.capture.is_some() {
            ("escape cancels", INK_NOTE)
        } else {
            ("enter changes - delete clears - escape goes back", INK_NOTE)
        };
        ui.text_centred(fb, footer.0, 189, footer.1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use td_assets::{DataDir, ExeResources};
    use td_audio::AudioCommand;
    use td_input::Pad;

    struct Rig {
        data: DataDir,
        res: ExeResources,
        settings: Settings,
        audio: Vec<AudioCommand>,
        port: SettingsPort,
    }

    impl Rig {
        fn new() -> Self {
            Self {
                data: DataDir::open(std::env::temp_dir()).unwrap(),
                res: ExeResources::default(),
                settings: Settings::default(),
                audio: Vec::new(),
                port: SettingsPort::new(),
            }
        }

        fn screen(&self) -> OptionsScreen {
            OptionsScreen::new(UiAssets::blank(), self.settings.clone(), self.port.clone())
        }

        /// One tick, then the host's part: take the pending settings.
        fn tick(&mut self, s: &mut OptionsScreen, input: &TickInput) -> Transition {
            let mut ctx = FlowContext {
                data: &self.data,
                resources: &self.res,
                settings: &self.settings,
                audio: &mut self.audio,
            };
            let t = s.tick(&mut ctx, input);
            if let Some(new) = self.port.take() {
                self.settings = new;
            }
            t
        }

        fn key(&mut self, s: &mut OptionsScreen, k: Key) -> Transition {
            self.tick(
                s,
                &TickInput {
                    pressed: vec![k],
                    ..TickInput::default()
                },
            )
        }

        fn settle(&mut self, s: &mut OptionsScreen) {
            for _ in 0..100 {
                if s.waiting() {
                    return;
                }
                self.tick(s, &TickInput::default());
            }
            panic!("never settled");
        }
    }

    #[test]
    fn video_changes_reach_the_host() {
        let mut rig = Rig::new();
        let mut s = rig.screen();
        assert!(rig.port.is_open());
        rig.settle(&mut s);
        rig.key(&mut s, Key::ENTER);
        assert_eq!(s.page(), Page::Video);
        assert_eq!(s.row(), Row::Fullscreen);
        rig.key(&mut s, Key::RIGHT);
        assert!(rig.settings.video.fullscreen);
        rig.key(&mut s, Key::DOWN);
        rig.key(&mut s, Key::RIGHT);
        assert_eq!(rig.settings.video.scale, Scale::Integer);
        rig.key(&mut s, Key::LEFT);
        rig.key(&mut s, Key::LEFT);
        assert_eq!(rig.settings.video.scale, Scale::Stretch);
    }

    #[test]
    fn classic_locks_the_enhanced_rows_until_the_profile_changes() {
        let mut rig = Rig::new();
        let mut s = rig.screen();
        rig.settle(&mut s);
        rig.key(&mut s, Key::ENTER); // video
        rig.key(&mut s, Key::DOWN);
        rig.key(&mut s, Key::DOWN);
        assert_eq!(
            s.row(),
            Row::Back,
            "the Enhanced rows are skipped under Classic"
        );
        assert!(!change(Row::Widescreen, &mut rig.settings.clone(), 1));
        rig.key(&mut s, Key::ESCAPE);
        assert_eq!((s.page(), s.row()), (Page::Main, Row::Open(Page::Video)));
        // Profile: Enhanced.
        rig.key(&mut s, Key::DOWN);
        rig.key(&mut s, Key::DOWN);
        rig.key(&mut s, Key::DOWN);
        rig.key(&mut s, Key::ENTER);
        assert_eq!(s.row(), Row::Profile);
        rig.key(&mut s, Key::ENTER);
        assert_eq!(rig.settings.profile, Profile::Enhanced);
        rig.key(&mut s, Key::ESCAPE);
        rig.key(&mut s, Key::UP);
        rig.key(&mut s, Key::UP);
        rig.key(&mut s, Key::UP);
        rig.key(&mut s, Key::ENTER); // video
        rig.key(&mut s, Key::DOWN);
        rig.key(&mut s, Key::DOWN);
        assert_eq!(s.row(), Row::Widescreen);
        rig.key(&mut s, Key::RIGHT);
        assert_eq!(rig.settings.enhanced.widescreen, Widescreen::W16x10);
        assert_eq!(
            rig.settings.effective_enhanced().widescreen,
            Widescreen::W16x10
        );
        // Back to Classic: the choice is kept, but forced off.
        rig.settings.profile = Profile::Classic;
        assert!(rig.settings.effective_enhanced().is_off());
        assert_eq!(rig.settings.enhanced.widescreen, Widescreen::W16x10);
    }

    #[test]
    fn volumes_step_in_tenths() {
        let mut rig = Rig::new();
        let mut s = rig.screen();
        rig.settle(&mut s);
        rig.key(&mut s, Key::DOWN);
        rig.key(&mut s, Key::ENTER);
        assert_eq!(s.row(), Row::MusicVolume);
        for _ in 0..12 {
            rig.key(&mut s, Key::RIGHT);
        }
        assert_eq!(rig.settings.audio.music_volume, 1.0);
        rig.key(&mut s, Key::LEFT);
        assert_eq!(percent(rig.settings.audio.music_volume), 90);
        assert_eq!(step_volume(0.05, -1), 0.0);
    }

    #[test]
    fn rebinding_moves_a_taken_key_and_reset_restores() {
        let mut rig = Rig::new();
        let mut s = rig.screen();
        rig.settle(&mut s);
        rig.key(&mut s, Key::DOWN);
        rig.key(&mut s, Key::DOWN);
        rig.key(&mut s, Key::ENTER);
        assert_eq!(s.page(), Page::Controls);
        assert_eq!(s.row(), Row::Binding(FlightKey::Kp8));
        // Keypad 8 := Space (taken from fire).
        rig.key(&mut s, Key::ENTER);
        assert!(s.capturing());
        rig.key(&mut s, Key::SPACE);
        assert!(!s.capturing());
        assert_eq!(
            rig.settings.controls.keyboard.get(FlightKey::Kp8),
            Some(Key::SPACE)
        );
        assert_eq!(rig.settings.controls.keyboard.get(FlightKey::Space), None);
        assert!(s.message.as_ref().is_some_and(|(m, _)| m.contains("fire")));
        // Esc cancels a capture.
        rig.key(&mut s, Key::ENTER);
        rig.key(&mut s, Key::ESCAPE);
        assert!(!s.capturing());
        assert_eq!(s.page(), Page::Controls);
        // Delete clears; the gamepad column takes a button.
        rig.key(&mut s, Key::DELETE);
        assert_eq!(rig.settings.controls.keyboard.get(FlightKey::Kp8), None);
        rig.key(&mut s, Key::RIGHT);
        assert_eq!(s.column(), Column::Gamepad);
        rig.key(&mut s, Key::ENTER);
        let pad = |b: u32| TickInput {
            pad: Some(Pad {
                buttons: b,
                ..Pad::default()
            }),
            ..TickInput::default()
        };
        rig.tick(&mut s, &pad(PadButton::Start.bit()));
        assert_eq!(
            rig.settings.controls.gamepad.get(FlightKey::Kp8),
            Some(PadButton::Start)
        );
        assert_eq!(rig.settings.controls.gamepad.get(FlightKey::Esc), None);
        rig.tick(&mut s, &pad(0));
        // Reset to defaults.
        rig.key(&mut s, Key::UP);
        rig.key(&mut s, Key::UP);
        assert_eq!(s.row(), Row::ResetControls);
        rig.key(&mut s, Key::ENTER);
        assert_eq!(
            rig.settings.controls,
            crate::controls::ControlsSettings::default()
        );
    }

    #[test]
    fn a_capture_times_out() {
        let mut rig = Rig::new();
        let mut s = rig.screen();
        rig.settle(&mut s);
        rig.key(&mut s, Key::DOWN);
        rig.key(&mut s, Key::DOWN);
        rig.key(&mut s, Key::ENTER);
        rig.key(&mut s, Key::ENTER);
        for _ in 0..CAPTURE_FRAMES {
            rig.tick(&mut s, &TickInput::default());
        }
        assert!(!s.capturing());
        assert_eq!(
            rig.settings.controls,
            crate::controls::ControlsSettings::default()
        );
    }

    #[test]
    fn gamepad_navigates_and_backs_out() {
        let mut rig = Rig::new();
        let mut s = rig.screen();
        rig.settle(&mut s);
        let pad = |y: i8, b: u32| TickInput {
            pad: Some(Pad {
                y,
                buttons: b,
                ..Pad::default()
            }),
            ..TickInput::default()
        };
        rig.tick(&mut s, &pad(-1, 0)); // down: audio
        rig.tick(&mut s, &pad(0, 0));
        rig.tick(&mut s, &pad(0, PadButton::South.bit()));
        assert_eq!(s.page(), Page::Audio);
        rig.tick(&mut s, &pad(0, 0));
        rig.tick(&mut s, &pad(0, PadButton::East.bit()));
        assert_eq!(s.page(), Page::Main);
        rig.tick(&mut s, &pad(0, 0));
        rig.tick(&mut s, &pad(0, PadButton::East.bit()));
        let mut popped = false;
        for _ in 0..100 {
            if matches!(rig.tick(&mut s, &pad(0, 0)), Transition::Pop) {
                popped = true;
                break;
            }
        }
        assert!(popped);
        assert!(!rig.port.is_open());
    }

    #[test]
    fn changes_made_elsewhere_are_taken_over() {
        let mut rig = Rig::new();
        let mut s = rig.screen();
        rig.settle(&mut s);
        rig.settings.video.fullscreen = true; // F11 in the host
        rig.tick(&mut s, &TickInput::default());
        assert!(s.settings().video.fullscreen);
    }

    /// Visual check: writes every page, drawn with the game's fonts, as a PPM file into the
    /// folder `TD2192_UI_DUMP` names (outside the repository: the images contain game data).
    #[test]
    #[ignore = "writes images for a visual check"]
    fn dump_pages() {
        let data = td_test_support::require_data!();
        let Some(out) = std::env::var_os("TD2192_UI_DUMP") else {
            return;
        };
        let dir = DataDir::open(&data).unwrap();
        let mut rig = Rig::new();
        rig.settings.profile = Profile::Enhanced;
        let mut s =
            OptionsScreen::new(UiAssets::load(&dir), rig.settings.clone(), rig.port.clone());
        rig.settle(&mut s);
        let mut fb = Framebuffer::new();
        for page in [
            Page::Main,
            Page::Video,
            Page::Audio,
            Page::Controls,
            Page::Profile,
            Page::Accessibility,
        ] {
            s.open(page);
            s.draw(&mut fb);
            let mut ppm = b"P6 320 200 255\n".to_vec();
            for &p in &fb.pixels {
                let c = fb.palette.colors[usize::from(p)];
                ppm.extend([c.r, c.g, c.b].map(|v| (u16::from(v) * 255 / 63) as u8));
            }
            let path = std::path::Path::new(&out).join(format!("{page:?}.ppm"));
            std::fs::write(path, ppm).unwrap();
        }
    }

    #[test]
    fn every_page_draws() {
        let mut rig = Rig::new();
        rig.settings.profile = Profile::Enhanced;
        let mut s = rig.screen();
        rig.settle(&mut s);
        let mut fb = Framebuffer::new();
        for page in [
            Page::Main,
            Page::Video,
            Page::Audio,
            Page::Controls,
            Page::Profile,
            Page::Accessibility,
        ] {
            s.open(page);
            s.draw(&mut fb);
            assert!(fb.pixels.contains(&INK_CURSOR), "{page:?}");
        }
    }
}
