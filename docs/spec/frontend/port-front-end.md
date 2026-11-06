# Port front end: start screen, options, bindings and controller navigation

| | |
|---|---|
| **Spec ID** | `frontend/port-front-end` |
| **Revision** | 1 |
| **Status** | Draft (design, not reverse engineering) |
| **Fidelity tier** | T3, port additions. The original screens are unchanged except where §5 says so |
| **Tasks** | M11-01 (start screen), M11-03 (settings screens), M11-04 (controller navigation) |
| **Code** | `td_flow::front` (`start`, `options`, `ui`), `td_flow::controls`, `td_flow::settings`, `td2192::run` |

## 1. Summary

The original has no screen that chooses an episode: each episode is its own program. The port adds a start screen in front of the three episode programs, and settings screens reachable from it and, during play, from a host key. All are drawn like the original's own screens: indexed 320 × 200, the small font (`td.lt2`), the title font (`td-let.cin`), `tdmen.pal` with entries 224..229 set to the colours below, and 16-level fades at 2 frames a level.

| Entry | Use | RGB (6-bit) |
|---:|---|---|
| 224 | text | 42, 42, 42 |
| 225 | selected item (the main menu's cursor red) | 63, 0, 0 |
| 226 | unavailable | 18, 18, 18 |
| 227 | headings, prompts (the main menu's yellow) | 63, 63, 0 |
| 228 | values | 63, 63, 63 |
| 229 | notes and hints | 40, 10, 10 |

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

## 3. Options (M11-03)

Reached from the start screen and with **F2** during play (the original reads none of F1..F4; F7/F8 are the debug Enhanced hotkeys, F9 the debug mission complete, F10..F12 the other host keys). F2 pushes the options on top of whatever is running, which waits until they close. F2 never reaches the game.

| Page | Rows |
|---|---|
| main | video, audio, controls, profile, accessibility, back |
| video | fullscreen, scaling (aspect 4x3, integer, stretch), widescreen\*, pixel upscaler\*, dynamic lighting\*, bloom\* |
| audio | music volume, sound volume (0..100 in tens, with a bar), sound effects (classic, polyphonic) |
| controls | one row per original flight key: action, original key, keyboard binding, gamepad binding; reset to defaults |
| profile | classic or enhanced, with a two-line description; intro before episode 1 (always, first launch, never: `[intro] play`, read only under Enhanced, so it is drawn in entry 226 under Classic, which always plays the intro) |
| accessibility | colour-blind guidance line |

\* `[enhanced]` features (MDG §6.2). Under the Classic profile they are drawn in entry 226, cannot be chosen, and the page notes `enhanced options need the enhanced profile`. Their values are kept, and `Settings::effective_enhanced` is all off until the profile is Enhanced. The profile governs only `[enhanced]` (spec enhanced/presentation rev 2 §2): the SFX policy and the accessibility options are separate settings that default to off. Widescreen is marked `affects gameplay` on its row at all times, and shows the note `affects gameplay - visibility` while selected (spec enhanced/presentation §2).

- Left and right change a value; Enter or Space changes it forward or opens a page; Esc or Backspace goes back.
- Every change goes to the host at once (`SettingsPort`). The host applies it live (fullscreen, scaling, volumes, SFX policy, Enhanced presentation; the flight reads its bindings every tick) and writes `settings.toml`. Only the values the player changed are written (`Settings::with_changes`), so command-line overrides such as `--fullscreen` stay out of the file. F10 and F11 change the session only, and the options screen shows their state.
- The colour-blind guidance line is a saved setting with no drawing yet (TODO M12-04): the guidance dots use palette entries shared with the playfield.

### 3.1 Controls

- Enter (or South) on a cell waits up to 5 s for a key (keyboard column) or a controller button (gamepad column). Esc cancels a key capture, East cancels too; Esc cancels a button capture. Delete clears the selected cell.
- A key or button stands for one flight key: binding it to another flight key takes it away from the first, and the screen says `taken from <action> - <key>`.
- `reset to defaults` restores both columns.

## 4. Bindings (`[controls]`)

```toml
[controls.keyboard]     # flight key = host key, or "none"
keypad-8 = "keypad-8"
esc = "escape"
[controls.gamepad]      # flight key = controller button, or "none"
esc = "start"
```

- Flight-key names: `keypad-8 up keypad-2 down keypad-4 left keypad-6 right keypad-plus keypad-0 alt right-alt space esc p f5 f6 s tab m` (`td_sim::FlightKey`). Host keys use `td_input::Key::name` (letters, digits, `f1`..`f12`, `keypad-0`..`keypad-9`, named keys such as `left-alt`, else `key-N` with the USB HID usage ID). Buttons use `td_input::PadButton::name` (`south east west north back guide start left-stick right-stick left-shoulder right-shoulder dpad-up dpad-down dpad-left dpad-right left-trigger right-trigger`).
- Missing entries keep their defaults; unknown names are ignored with a warning.
- Keyboard defaults are the original's keys (`docs/re/globals.md` §3), including Tab (switch weapon) and M (repeat message), which the hard-coded table before M11-03 lacked.
- Gamepad defaults: space = south, esc = start, f5 = back, tab = north, alt = west, f6 = left-shoulder, m = right-shoulder.
- The stick and D-pad stay the original joystick (x, y), and East and South its buttons 1 (afterburner) and 2 (fire), as the original menus read them.
- **A bound button acts as its flight key's keyboard key.** While it is held the host adds that key to the input, and its press counts as a key press (`ControlsSettings::pad_keys`). So the flight reads it like the key, and every original screen sees a key it understands: Start is Esc everywhere, South is Space (fire, menu select, "hit any key"). A flight key without a keyboard key gets nothing from its button.

## 5. Controller navigation (M11-04)

- Port screens: stick or D-pad moves (first push, then repeats after 24 frames every 7), South confirms, East goes back. A tick with a button press ignores that tick's keys, which may be the keys the button's binding stands for; Start therefore works as back through its Esc binding.
- Original menus keep the original joystick rules (main menu, save screens and the flight's computer menu: the stick moves, either button selects). Port additions so that a controller alone never gets stuck:
  - the main menu's confirmation boxes (quit, current game lost), which read one key (`y` = yes): South answers yes, East no;
  - the script-choice screen, which reads only 1 and 2: South is 1 (original script), East is 2 (edited script).
- "Hit any key" screens (story cards, dialogue, pause, the tactical map, the comm viewer) take the keys of bound buttons, for example South (Space) or Start (Esc).

## 6. Tests

- `td_flow::front::start`: navigation and skipping of missing episodes, launch and return on quit, a failed launch, controller navigation, options on top, snapshot hashes (solid-block fonts; game fonts when `TD2192_DATA` is set).
- `td_flow::front::options`: live changes reach the host, Classic locks the Enhanced rows, volumes, rebinding with conflict and reset, capture timeout, controller navigation, changes made elsewhere, every page draws.
- `td_flow::settings` and `td_flow::controls`: round trips (string and file), partial files, Classic forces Enhanced off, only the player's changes reach the file, button-to-key mapping.
- `td_flow::main_menu`: a controller answers the boxes. `td_flow::screen`: quit returns to a screen that catches it.
