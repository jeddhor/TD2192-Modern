# Port front end: start screen, options, bindings and controller navigation

| | |
|---|---|
| **Spec ID** | `frontend/port-front-end` |
| **Revision** | 6 |
| **Status** | Draft (design, not reverse engineering) |
| **Fidelity tier** | T3, port additions. The original screens are unchanged except where §5 says so |
| **Tasks** | M11-01 (start screen), M11-03 (settings screens incl. game data), M11-04 (controller navigation), M13-02 (first run), M12-04 (colour-blind guidance line; the D5 Enhanced preset, §3.4), M12-18 (reduce flashing §3.5, text waits §3.6, the window in the background §3.7, controller options and pause §4, any button on waits and typed names §5) |
| **Code** | `td_flow::front` (`start`, `options`, `data`, `ui`), `td_flow::controls`, `td_flow::settings`, `td2192::run`, `td2192::data`, `td_assets::{archive, import}`, `td_render::enhanced::guidance`, `td_render::flash` |
| **Decisions** | D5 (user, 2026-11-06): the Enhanced preset (§3.4) |

## 1. Summary

The original has no screen that chooses an episode: each episode is its own program. The port adds a start screen in front of the three episode programs, and settings screens reachable from it and, during play, from a host key. All are drawn like the original's own screens: indexed 320 × 200, the small font (`td.lt2`), the title font (`td-let.cin`), `tdmen.pal` with entries 224..229 set to the colours below, and 16-level fades at 2 frames a level.

| Entry | Use | RGB (6-bit) |
|---:|---|---|
| 224 | text | 42, 42, 42 |
| 225 | selected item (the main menu's cursor red) | 63, 0, 0 |
| 226 | unavailable | 28, 28, 28 (rev 5: 4.30:1 on black; was 18, 18, 18) |
| 227 | headings, prompts (the main menu's yellow) | 63, 63, 0 |
| 228 | values | 63, 63, 63 |
| 229 | notes and hints | 56, 32, 32 (rev 5: 7.78:1 on black; was 40, 10, 10) |

The small font has no capitals, `:`, `/` or brackets, so every label is lower case and uses `-` and `x` (for example `16x9`, `affects gameplay - visibility`).

## 2. Start screen (M11-01)

- Shown when `td2192` runs without `--episode` (and without a developer flag). `--episode N` starts episode N's program directly, as before.
- Items: `episode 1`, `episode 2`, `episode 3`, `options`, `credits`, `quit`. An episode whose files `td_assets::validate` does not find is drawn in entry 226 with `- not installed` and the cursor skips it.
- `credits` (IM-15) pushes the credits screen ([credits](credits.md)): mode A (the intro flyover) under the Classic profile, mode B (grouped pages) under Enhanced; it pops back here. If its files are missing the screen says `the credits could not start`.
- Starting Episode 1 plays the intro first ([launcher](../flow/launcher.md) §4.10); Esc skips it.
- Choosing an episode fades out and starts that episode's program from its start, a new program run (its own flight session and RNG stream, as when the original starts the executable).
- The episode's quit item (`Transition::Quit`) returns to the start screen (`Screen::catches_quit`), which stops the music and effects and fades in again with the cursor where it was.
- Esc (or East) moves the cursor to `quit`; a second Esc or a confirm there quits.
- A launch failure stays on the screen with `episode N could not start` and logs the cause.
- A new choice of game data (§3.2) takes effect here. On each tick (it ticks only when it is on top) the screen asks the host to switch if a choice is waiting (`SettingsPort::want_data_switch`). After the host has switched (`SettingsPort::data_generation` changes), the screen reloads its fonts and palette from the new data and takes the installed episodes from the host's description.

### 2.1 First run (M13-02)

The data directory comes from `--data`, then `TD2192_DATA`, then `data_dir` in `settings.toml` (MDG §8.8). Each may name a folder or the original ZIP file. When none is set, or the saved `data_dir` no longer opens or has no complete Episode 1 (the player moved or deleted it), the first run starts:

1. The game's fonts are not available yet, so a system message box asks where the game is. The buttons are `Folder...`, `ZIP file...` and `Quit`. The text explains that a folder is used where it is, and that a ZIP is read where it is and can be extracted on the next screen.
2. The system folder or file dialog opens. Cancelling it returns to step 1.
3. The choice is opened and checked against the SHA-256 manifest (`td_assets::validate`):
   - **Unusable** (Episode 1 is not complete, or the file is not a readable ZIP): a message box at step 1 says why. It gives the file counts, the playable episodes and up to eight missing Episode 1 files. Then step 1 repeats.
   - **Usable**: the choice is saved as `data_dir`, the window opens, and the game-data page (§3.2) is shown on its own over the start screen as the result screen, in the game's fonts. For a ZIP the cursor starts on `extract the zip`, for a folder on `continue`.
4. On the result screen, `extract the zip` is the player's consent to copy the files (§3.2); `continue` keeps reading the ZIP in place. Either way the start screen follows.

**ZIP files** (`td_assets::archive`):
- The archive is read once into memory (about 9 MB). It is never unpacked unless the player extracts it.
- Every file entry is indexed by the DOS 8.3 form of its base name, whichever folder it is in, so archives with the files at the root or inside a folder both work. When two entries collapse to one name, the one nearest the root wins.
- Supported methods: stored, Deflate, and the PKZIP 1.x methods Shrink, Reduce and Implode. An entry that cannot be read is reported as unreadable, not as missing.
- Entries over 8 MiB are skipped; archives over 256 MiB in total or with more than 20,000 entries are refused.

**Extraction** (`td_assets::import`):
- It copies only the manifest files that the archive holds and can read, modified ones included (they count as present, MDG §9.1.2). Each goes under its manifest name into `<user data>/game`, through a temporary name and a rename.
- Names inside the archive are never used as paths.
- A manifest file left in that folder by an earlier import and missing from the new source is removed. Other files there are left alone.
- After the copy the folder is validated and becomes the new choice.

## 3. Options (M11-03)

Reached from the start screen and with **F2** during play (the original reads none of F1..F4; F7/F8 are the debug Enhanced hotkeys, F9 the debug mission complete, F10..F12 the other host keys). F2 pushes the options on top of whatever is running, which waits until they close. F2 never reaches the game.

| Page | Rows |
|---|---|
| main | video, audio, controls, profile, accessibility, game data, back |
| video | fullscreen, scaling (aspect 4x3, integer, stretch), in the background (pause, keep running; §3.7), widescreen\*, pixel upscaler\*, dynamic lighting\*, bloom\* |
| audio | music volume, sound volume (0..100 in tens, with a bar), sound effects\* (classic, polyphonic) |
| controls | one row per original flight key: action, original key, keyboard binding, gamepad binding; `options` (the controller button that opens the options, §3.1); reset to defaults |
| profile | classic or enhanced, with a two-line description; original bug fixes\* (on, off); intro before episode 1 (always, first launch, never: `[intro] play`, read only under Enhanced, so it is drawn in entry 226 under Classic, which always plays the intro); analog steering (Enhanced only, off by default; [spec](analog-steering.md)), restore the enhanced preset\* |
| accessibility | colour-blind guidance line (§3.3), reduce flashing (§3.5), text waits for a key (§3.6) |
| game data | the folder or ZIP in use, who set it, its file check and the playable episodes; choose a folder, choose a zip file, extract the zip, back (§3.2) |

\* Enhanced features (MDG §6.2, D5; §3.4). Under the Enhanced profile each row shows the value in effect: the player's choice, else the preset's. Under the Classic profile they are drawn in entry 226 with the values Enhanced would use, cannot be chosen, and the page notes `enhanced options need the enhanced profile`; the choices are kept, and everything they control is off (`Settings::effective_enhanced`, `effective_sfx_mode`, `enhanced_fixes`). The accessibility options and the intro setting are separate options that default to off. Widescreen is marked `affects gameplay` on its row at all times, and shows the note `affects gameplay - visibility` while selected (spec enhanced/presentation §2).

- Left and right change a value; Enter or Space changes it forward or opens a page; Esc or Backspace goes back.
- Every change goes to the host at once (`SettingsPort`). The host applies it live (fullscreen, scaling, volumes, SFX policy, Enhanced presentation; the flight reads its bindings every tick) and writes `settings.toml`. Only the values the player changed are written (`Settings::with_changes`), so command-line overrides such as `--fullscreen` stay out of the file. F10 and F11 change the session only, and the options screen shows their state.
- The colour-blind guidance line (§3.3) is drawn at the RGBA stage whenever the setting is on, under either profile (it is an option outside the Enhanced preset, §3.4).

### 3.1 Controls

- Enter (or South) on a cell waits up to 5 s for a key (keyboard column) or a controller button (gamepad column). Esc cancels a key capture, East cancels too; Esc cancels a button capture. Delete clears the selected cell.
- F10, F11 and F12 are never bound: a key capture ignores them and waits for another key (rev 4, M12-07). They belong to the host (scaling, fullscreen, screenshot), which acts on them whatever the screen.
- A key or button stands for one flight key: binding it to another flight key takes it away from the first, and the screen says `taken from <action> - <key>`.
- The `options` row (rev 5, M12-18) holds the controller button that opens the options during play, as F2 does (`[controls] options_button`, default the left stick's press, `none` for none). Its keyboard cell shows `f2`, which cannot be changed; Enter captures a button, Delete clears it. The button stands for one thing: binding it to a flight key takes it from the options (`taken from options`) and the other way round.
- `reset to defaults` restores both columns and the options button.

**What cannot be rebound, and why** (rev 5, M12-18; VE-8's F4). Only the flight keys and the controller's options button are bindings:
- **Menu keys** (confirm, back, up, down, page, the script choice's 1 and 2, the yes key) stay fixed. The original screens read the BIOS keys themselves: the main menu, the save screens, the dialogue and story waits, the computer menu, the comm viewer and the tactical map each read their own set, some through the type-ahead buffer and some as characters. Rebinding them would need a layer that turns the player's keys into the original keys per screen, kept apart from the name entry and the flight's key bytes. That is not a cheap change, so it is left for later. A controller reaches every screen already (§5).
- **Host keys** F2, F10, F11 and F12 stay fixed on the keyboard: a host binding would have to be taken out of the input before any screen sees it and excluded from every capture. The controller gets the options button instead; scaling, fullscreen and screenshots have no natural button and keep their keys (the options screen has scaling and fullscreen rows).

### 3.2 Game data (M11-03, M13-02)

The screens have no file system or dialogs, so they work through the `SettingsPort` (`td_flow::front::data`):
- The host describes the data in a `DataInfo`: the folder or ZIP in use and its check (file counts, playable episodes), where it was set, and any new choice of this session (pending). It also holds the outcome of the last request and the folder that extraction writes to.
- The page asks for a `DataRequest`: pick a folder, pick a ZIP, or extract. After the tick the host opens the system dialog over the window (leaving fullscreen meanwhile), or extracts. It then validates and updates the description. The wall time the dialog took is not turned into ticks.

Layout, all in the small font:
- `folder` or `zip file`, then the path. Letters are shown in lower case, and the port draws its own marks for `\ / : _ ( ) + ' &`, which the font lacks. A path too wide keeps its end after `...`.
- `set by`: `the command line --data`, `td2192_data`, `settings.toml` or `chosen now`.
- `files`: for example `218 of 218 files match` or `200 match - 3 modified - 15 missing`.
- `playable`: for example `episodes 1 2 3`.
- With a pending choice, the block describes it and `used from the start screen on` is shown under it.

Rows: `choose a folder`, `choose a zip file`, `extract the zip` (only for a ZIP), and `back`. On the first run's result screen `back` reads `continue` and leaves the screen.

The host's rules (`td2192::data::DataHost`):
- **Usable choice** (Episode 1 complete): it is saved at once as `data_dir` and becomes pending (message `saved to settings.toml`). The start screen switches to it when it is on top (§2), so the episode in progress keeps its data. With `--episode N` there is no start screen, so the choice is used at the next start.
- **Unusable choice**: nothing changes. The message gives the reason, for example `nothing changed - no complete episode 1 - 12 match - 206 missing`.
- **The data already in use**: chosen again, it cancels a pending choice.
- **Extraction**: it copies the ZIP shown (the pending one, else the one in use) into `<user data>/game` (§2.1), then chooses that folder. The message is `extracted N files - the zip is no longer needed`.
- **Precedence** (MDG §8.8): while `--data` or `TD2192_DATA` names the data, the three rows are disabled. The page says `set by --data on the command line - change it there` or `set by td2192_data - change it there`, because a choice saved to `settings.toml` would not be used.
- **Start-up**: a saved `data_dir` that no longer works sends the player through the first run again (§2.1). An unusable `--data` or `TD2192_DATA` is an error, as before.

### 3.3 Colour-blind guidance line (M12-04)

**The problem.** The original draws the six guidance dots (`render/hud.md` §3.4, the line from the ship toward the target) in palette entries the playfield shares:
- a target: a dark-to-bright red ramp, 6-bit (15, 3, 3) up to (63, 3, 15);
- a friend: the same ramp in green;
- home: the same ramp in cyan.

Red against green is the pair that protanopia and deuteranopia confuse. The dark end of each ramp also vanishes on dark ground. Changing those palette entries would recolour the city too.

**The choice.** With `accessibility.colorblind_guidance = true`, the line is drawn at the RGBA stage (`td_render::enhanced::guidance`), and no palette entry changes. Each marker gets its own colour pair, a flat core and a black outline:

| Marker | Core (sRGB) | Source |
|---|---|---|
| target (32h) | orange (230, 159, 0) | Okabe–Ito |
| friend (39h) | sky blue (86, 180, 233) | Okabe–Ito |
| home (3Dh) | white (255, 255, 255) | |
| outline | black (0, 0, 0): every 8-neighbour of a dot that is not itself a dot | |

- **Why these colours.** The Okabe–Ito set ("Color Universal Design", Okabe and Ito 2008) is designed to stay distinct under the common deficiencies. The three cores also differ in lightness, so they stay apart even when hue is lost. A unit test simulates protanopia, deuteranopia and tritanopia (Viénot, Brettel and Mollon 1999; Brettel et al. 1997) and requires every pair, and every core against the outline, to stay apart.
- **Why the outline.** The dark outline gives contrast on light ground, and the bright core gives contrast on dark ground.
- The same colours apply in the infra-red modes, where the original's IR-mode-2 dot colours do not apply.

**How it is drawn.**
- The dots take the original positions (the six dots after their shadow pixels, shifted by the widescreen margin m) and are clipped to the playfield rows and the frame.
- The marks are drawn after the palette stage, so the upscaler sees them, and again over the final image as k × k blocks, so lighting and bloom leave them at full strength.
- The setting is separate from the profile (an option outside the preset, §3.4). When it is on, the run loop takes the RGBA path even under the Classic profile: the Enhanced request has width 320 and no lighting, which gives the Classic picture plus the marks.
- **Invariant.** With the setting off, the frame takes the Classic path unchanged. With it on, only the guidance line's pixels differ from the Classic frame (test `td2192::flight::tests::colour_blind_guidance_changes_only_the_guidance_line`).

### 3.4 The Enhanced preset (D5, M12-04)

The user's decision D5 (2026-11-06): choosing the Enhanced profile turns on a preset, and each part of it stays adjustable.

| Feature | Setting | Preset | Classic |
|---|---|---|---|
| Widescreen | `enhanced.widescreen` | 16:9 (428 wide) | off |
| Pixel upscaler | `enhanced.upscaler` | xbr3 | none |
| Dynamic lighting | `enhanced.lighting` | subtle | off |
| Bloom | `enhanced.bloom` | off | off |
| Sound effects | `audio.sfx_mode` | polyphonic | classic (one at a time, sfx-policy §4) |
| Original-bug fixes | `enhanced.fixes` | on: every fix-enhanced fix of `original-bugs.md` | off: every bug preserved |

- Colour-blind guidance (§3.3), reduce flashing (§3.5), text waits for a key (§3.6), the intro setting (`[intro] play`, default `always`, so the intro is not skipped) and analog steering are options, off by default, outside the preset. Analog steering is implemented as optional proportional stick thrust with steady original facing ([analog-steering](analog-steering.md) rev 1). It is a separate Enhanced-only option on the profile page; Classic forces it off. The background setting (§3.7) has a default per profile.
- **Explicit choices.** Changing a preset row records the player's choice, which overrides the preset under Enhanced. The choice is kept when the profile changes and when a later version changes the preset. `restore the enhanced preset` (profile page, Enhanced only) drops every choice: the message is `enhanced preset restored`.
- **Classic** forces every row of the table off, whatever the choices say.
- The fixes in effect (`Settings::enhanced_fixes`) are read where each bug lives: OB-012 (music cues, `td_flow::program::cue_restart`), OB-022 (`td_sim::Flight::enhanced_fixes`), OB-023 and OB-032 (`td_render::flight_palette`), OB-024 and OB-057 (`td2192::flight`), OB-056 (`td_flow::save_screens`), OB-058 (`td_flow::intro::IntroOptions`). OB-024's radar table is chosen when a flight starts; the others follow a change at once.

**Storage** (`settings.toml` format 2, `version = 2`). A key of the table is written only when the player chose its value; an absent key means "the preset". So the file tells a preset default from a choice without a marker per key, and an explicit choice equal to the preset's value stays a choice:

```toml
version = 2
profile = "enhanced"

[enhanced]
widescreen = "off"    # chosen; upscaler, lighting, bloom and fixes take the preset

[audio]
music_volume = 0.8
sfx_volume = 0.8      # no sfx_mode: the preset (polyphonic)
```

**Older files** (format 1, no `version` key; written before D5) wrote every key, so their values cannot be told from untouched defaults. On loading, a format-1 value equal to the old default (`widescreen = "off"`, `upscaler = "none"`, `lighting = "off"`, `bloom = false`, `sfx_mode = "classic"`) is read as no choice, and any other value as an explicit choice. The next save writes format 2. A format-1 Enhanced player therefore keeps what was turned on and gets the preset for the rest.

### 3.5 Reduce flashing (M12-18)

An accessibility option (`accessibility.reduce_flashing`, default off), outside the preset and under either profile, against VE-8's F1 and F3 (`docs/parity/M12-accessibility.md` §4). It changes only what is presented: td-sim, the flight's DAC state (`FlightDac`, OB-029 and OB-032 included) and replays never see it, so it can be switched at any time.

- **Flight picture.** The palette presented for the flight picture follows the DAC at no more than 0.375 per second per entry, measured as the sum of the three channels' changes in linear light (`td_render::flash::FlashLimiter`), stepped once per host tick. A pixel's relative luminance and WCAG's red quantity max(0, R − G − B) then change at most that fast, so no one-second window can hold more than 1.9 general or 3 red flashes (WCAG 2.3.1: pairs of opposing changes of 0.1, or of more than 0.0625 for saturated red). A 6-bit level is shown only once the linear value has passed it, so the shown change never exceeds the linear one and a steady palette is shown exactly. This covers:
  - the hit flash and the white flash of a homing kill (small swells instead of flashes);
  - the infra-red toggle: a cross-fade of up to about 2.5 s for the brightest reds, about 1 s for most of the picture; with Alt held (10.9 flips a second by typematic repeat, as in the original: U-571, OB-176) the picture settles near the middle with a ripple of at most 0.034;
  - E3 flight 16's magenta wash.
- **Not limited:** the fades (the flight's opening and teardown, the fade-ins that close the computer menu and the real-time screens), which are gradual; the HUD's lamp entries, written over the presented palette (small areas, below WCAG's area threshold; they carry the transmission and low-armour cues); the flight's own modal screens, which draw their own pictures. The teardown starts from the presented palette, so a flash in progress is not shown at full strength.
- **Cut-scene.** The flight-17 cut-scene's flash steps (black, yellow-white, black; flight-screens §4.10.1) write each entry only within 0.05 (L1, linear) of its steady colour, the flight palette (`td_render::flash::cap_excursion`); the steps, their timing and the pixels are unchanged.
- **Enhanced lighting** (presentation §6.1): no muzzle-flash lights (they flicker at 9.6 Hz while the guns are held), explosions at 0.4 of their intensity, and bloom off. This softens but does not bound the explosion glow: near an explosion a pixel can still brighten by more than 0.1 in luminance for a few frames, so the WCAG bound above is the flight palette's; turning dynamic lighting off removes the rest (the player guide says so).
- Other screens: no full-screen flash was found outside the flights. The intro, the menus and the story screens change their palettes only by fades; the main menu's highlight pulse is one item.
- Tests: `td_render::flash::tests` (exact steady palette, the rate bound, a held infra-red key gives no WCAG flash, white and hit flashes become swells, the cap), `td2192::flight::tests::reduce_flashing_*` (with data: E1 flight 4 with Alt held, flown twice, option off and on: identical td-sim state hashes, DAC and pixels every tick; the raw palette flashes 4 or more times a second, the presented one never; the option switches live), `interlude_parity::reduce_flashing_caps_the_cut_scene_flash`, `lights::tests::reduce_flashing_softens_the_scene`.

### 3.6 Text waits for a key (M12-18)

An accessibility option (`accessibility.text_waits_for_key`, default off, so Classic timing is unchanged), against VE-8's F7:
- the timed waits of the story screens (assignment orders, dossiers, debriefs, the story so far, stills, scene and title cards) wait for a key instead of closing after their seconds;
- keys typed before a wait begins (during the fade-in, the typing, or on the previous screen) do not end it, and neither do keys typed while a dialogue page types: each page break waits for a new key. Esc still skips, when pressed at the wait;
- the screen layer only (`StoryScreen::set_text_waits`, `DialogueScene::set_text_waits`; their owners pass the setting each tick). Nothing reaches td-sim.

### 3.7 The window in the background (M12-18)

`video.pause_in_background` (the video page's `in the background`: `pause` or `keep running`), against VE-8's F12. Absent, it follows the profile: Enhanced pauses; Classic keeps running, as the original did (a DOS program has no background). While the window has lost the keyboard focus and the setting says pause, the run loop runs no ticks and renders no sound, and its clock starts again when the window comes back (the time away is not caught up). Whatever the setting, the keys held when the focus goes count as released (their releases go to the other window, Alt+Tab).

## 4. Bindings (`[controls]`)

```toml
[controls]
options_button = "left-stick"   # the controller button that opens the options, or "none"
[controls.keyboard]     # flight key = host key, or "none"
keypad-8 = "keypad-8"
esc = "escape"
[controls.gamepad]      # flight key = controller button, or "none"
esc = "start"
```

- Flight-key names: `keypad-8 up keypad-2 down keypad-4 left keypad-6 right keypad-plus keypad-0 alt right-alt space esc p f5 f6 s tab m` (`td_sim::FlightKey`). Host keys use `td_input::Key::name` (letters, digits, `f1`..`f12`, `keypad-0`..`keypad-9`, named keys such as `left-alt`, else `key-N` with the USB HID usage ID). Buttons use `td_input::PadButton::name` (`south east west north back guide start left-stick right-stick left-shoulder right-shoulder dpad-up dpad-down dpad-left dpad-right left-trigger right-trigger`).
- Missing entries keep their defaults; unknown names are ignored with a warning.
- Keyboard defaults are the original's keys (`docs/re/globals.md` §3), including Tab (switch weapon) and M (repeat message), which the hard-coded table before M11-03 lacked.
- Gamepad defaults: space = south, esc = start, f5 = back, tab = north, alt = west, f6 = left-shoulder, m = right-shoulder, p = right-stick (rev 5, M12-18: the pause); the options button is left-stick. The triggers and Guide stay free (the system often takes Guide). No natural button is left for F10–F12, which keep their keys.
- The stick and D-pad stay the original joystick (x, y), and East and South its buttons 1 (afterburner) and 2 (fire), as the original menus read them.
- **A bound button acts as its flight key's keyboard key.** While it is held the host adds that key to the input, and its press counts as a key press (`ControlsSettings::pad_keys`). So the flight reads it like the key, and every original screen sees a key it understands: Start is Esc everywhere, South is Space (fire, menu select, "hit any key"). A flight key without a keyboard key gets nothing from its button.

## 5. Controller navigation (M11-04)

- Port screens: stick or D-pad moves (first push, then repeats after 24 frames every 7), South confirms, East goes back. A tick with a button press ignores that tick's keys, which may be the keys the button's binding stands for; Start therefore works as back through its Esc binding.
- Original menus keep the original joystick rules (main menu, save screens and the flight's computer menu: the stick moves, either button selects). Port additions so that a controller alone never gets stuck:
  - the main menu's confirmation boxes (quit, current game lost), which read one key (`y` = yes): South answers yes, East no;
  - the script-choice screen, which reads only 1 and 2: South is 1 (original script), East is 2 (edited script);
  - the save screen's name entry ([save-screens](../flow/save-screens.md)), which ends only on Enter: a press of South (joystick button 2) ends it as Enter does, and the keys its binding stands for in that frame are not typed (rev 4, M12-07). The press that chooses the slot comes before the entry, so it does not save at once. Without this a controller-only player could not save, and under Classic (OB-056) could not leave the entry either.
- "Hit any key" screens (story cards, dialogue, pause, the tactical map, the comm viewer) take the keys of bound buttons, for example South (Space) or Start (Esc).
- (rev 5, M12-18; VE-8's F5, F6) The waits that end on any key (the story screens' holds, the dialogue's page breaks, the flight's pause) also end on a press of **any controller button**, bound or not (`td_flow::controls::AnyButton`): the original reads joystick buttons 1 and 2 there (dialogue-player §4.7, screens §2.2), the port every button. A press counts in the frame it happens and is not kept for a later wait, and a button already held when the screen starts does not count. The script choice keeps its own rule (South is 1, East is 2). Keyboard behaviour is unchanged.
- (rev 5) The options button (§3.1) opens the options during play, and the right stick's press pauses a flight (its default binding, P).
- (rev 5, VE-8's F13) **Typed names.** While the save-name entry runs (`Screen::wants_text`), the host turns the system's text input on, and the entry takes the characters the keyboard layout types (`TickInput::text`), with the same filter (a–z with upper case lowered, 0–9, space, `-`, `.`), so AZERTY and QWERTZ players get the letters on their keys. A frame without text (synthetic input) uses the keys' positions as before. Text input is off elsewhere, because an input method can take key presses while it is on. The main menu's yes key is still the `y` position (a menu key, §3.1).

## 6. Tests

- `td_flow::front::start`: navigation and skipping of missing episodes, launch and return on quit, a failed launch, controller navigation, options on top, snapshot hashes (solid-block fonts; game fonts when `TD2192_DATA` is set).
- `td_flow::front::options`: live changes reach the host, Classic locks the Enhanced rows, volumes, rebinding with conflict and reset, capture timeout, controller navigation, changes made elsewhere, every page draws.
- `td_flow::settings` and `td_flow::controls`: round trips (string and file), partial files, Classic forces Enhanced off, only the player's changes reach the file, button-to-key mapping.
- The Enhanced preset (rev 3): the profile turns on the preset; a choice overrides it and survives the profile and a round trip, also when it equals an old default; restoring the preset drops the choices (also from the file); format-1 files migrate; Classic forces the sound effects and the fixes off too; every preset row is locked under Classic (`settings::tests`, `options::tests::the_enhanced_preset_rows`); each fix in both profiles (`tests/enhanced_fixes.rs`, `save_screens::tests::esc_cancels_the_name_entry_only_with_the_fix`, `flight_palette::tests`, `logic::tests::flight_8_ob022_soft_lock_only_in_classic`, `td2192::flight::tests::ob057_*`, `ob024_*`).
- `td_flow::main_menu`: a controller answers the boxes. `td_flow::screen`: quit returns to a screen that catches it.
- Game data (rev 2):
  - `td_assets::archive`: stored and Deflate entries by 8.3 name, the nearest-the-root rule, not-a-ZIP errors, damaged entries listed as unreadable;
  - `td_assets::import`: only manifest files written, under their own names (a `../` entry is never written), a new import replaces the old one, a folder cannot be imported into itself; with data, the original files zipped at test time are read in place and extracted, both validating 218 of 218;
  - `td_flow::front::data`: the summary lines and the override lock;
  - `td_flow::front::options`: the page's rows follow the data, a request reaches the host, a new ZIP moves the cursor to extract, the override lock, and the first run's page on its own;
  - `td_flow::front::start`: a pending choice is switched to and the episodes follow;
  - `td2192::data`: unusable choices change nothing; with data, a synthetic ZIP is chosen, extracted and switched to.
- Colour-blind guidance (rev 2): the colours stay apart under simulated deficiencies, marks are built per marker with an outline, the pipeline changes only the marked pixels and only in flight frames, and (with data) the Classic invariance above.
- Rev 5 (M12-18): reduce flashing (§3.5); `story::tests::text_waits_for_a_key_holds_timed_screens`, `any_controller_button_ends_the_wait`, `dialogue::tests::buttons_turn_pages_and_text_waits_drops_early_keys`, `flight_screens::tests::any_button_ends_the_pause`; `controls::tests::the_options_button`, `options::tests::the_options_button_row`; `settings::tests::pausing_in_the_background_follows_the_profile_unless_chosen`; `td_input::tests::focus_loss_releases_the_keys`; `save_screens::tests::names_take_the_typed_characters`; `ui::tests::inks_have_enough_contrast`.

## 7. Revision history

- Rev 6 (2026-11-09, IM-28): optional Enhanced analog steering, outside the preset; profile page and replay-safe input rules in [analog-steering](analog-steering.md).

| Rev | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-11-05 | IM-14 | Start screen, options pages, bindings, controller navigation (M11-01, M11-03, M11-04) |
| 2 | 2026-11-06 | IM-16 | First run from a folder or the original ZIP (§2.1, M13-02); the game-data page and the data switch at the start screen (§2, §3.2, M11-03); the colour-blind guidance line design (§3.3, M12-04) |
| 3 | 2026-11-06 | IM-22 | The D5 Enhanced preset (§3.4): preset table, explicit choices, `settings.toml` format 2 (absent key = preset) and the format-1 migration; the sound effects and the original-bug fixes join the Enhanced rows (§3); the profile page's `original bug fixes` and `restore the enhanced preset` rows |
| 4 | 2026-11-06 | VE-8 | M12-07 small fixes: South ends the save-name entry (§5); F10..F12 cannot be bound (§3.1) |
| 5 | 2026-11-06 | IM-25 | M12-18 (VE-8's M12-07 findings): reduce flashing (§3.5), text waits for a key (§3.6), the window in the background (§3.7), the controller's options button and the pause on the right stick (§3.1, §4), what cannot be rebound and why (§3.1), any button ends a "hit any key" wait and typed save names (§5), inks 226 and 229 raised to 4.3:1 and 7.8:1 (§1) |


## 8. Windows launch and diagnostics (IM-28)

The Windows executable uses the GUI subsystem: launching it from Explorer opens no console window. At entry it attempts to attach to its parent's existing console, without creating one, and preserves inherited redirected standard handles. `--help`, `--version`, `--check` and argument errors therefore remain useful in terminals and redirected scripts. When there is no text output, help/version/check use an information dialog, and argument/startup errors use an error dialog. Terminal errors print to stderr and return a failing exit status without waiting for a dialog.

Runtime diagnostics also go to `<user data>/td2192.log`, replaced on each normal startup (argument help/version leave it alone). `TD2192_LOG` still controls the filter. A log-file creation failure does not prevent playing; stderr remains the fallback. The narrow Win32 unsafe exception is confined to `td-platform/src/windows_console.rs`, with borrowed handles, no pointer dereferences and no handle ownership transfer. Its API contracts are the Microsoft AttachConsole/GetStdHandle documentation cited in that module. All other crates retain the workspace unsafe prohibition.
