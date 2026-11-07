# Parity: M12-07 accessibility review (2026-11-06, VE-8)

Verifier: VE-8 (Verifier hat; no disassembly, decompiler output or RE exports read). **Method:** a code and spec review of the port against the accessibility row of [MDG §11](../MASTER_DESIGN_GUIDE.md#11-quality-performance-and-accessibility-targets), MDG §9.5, §9.8, §9.10 and §9.12, [port-front-end](../spec/frontend/port-front-end.md) rev 3 and what the [player guide](../player/README.md) promises. Every screen of the three episode programs was traced for the keys and controller inputs it reads. Flash and legibility figures are computed from the specs' constants (flight tick 19.2015 Hz, retrace 70.086 Hz, the 5 × 5 small font) and the presenter's scaling code. Contrast ratios are WCAG 2.x relative-luminance ratios of the 6-bit palette values (×255/63, sRGB). **Not run:** no build, no tests, no game, no captures, no controller, no real display (§5). **Tree under test:** `cba89f1` (this worktree's HEAD; other agents' uncommitted edits not included).

## 1. Summary

| MDG §11 item | Verdict | In short |
|---|---|---|
| Full rebinding | PARTIAL | The 20 flight keys are rebindable on keyboard and controller, with conflict moves and a reset. The screen keys (menus, prompts, "hit any key" waits, the flight's modal screens), the host keys F2 and F10–F12, and the controller's stick, D-pad, South (fire) and East (afterburner) are fixed (F4, F14). A button acts as its flight key's *keyboard binding*, so keyboard rebinding changes what buttons do on the original screens (F6) |
| No required simultaneous key chords | MEETS | No screen or action needs a chord. The cheat chords (D9) are optional. Alt+Enter is not used (F11 is fullscreen). Flying and firing at once needs two held inputs, as in the original; the "hold/toggle afterburner" option of MDG §6.2 does not exist |
| Colour-blind-safe guidance-line option (Enhanced) | MEETS | An accessibility option outside the preset that also works under Classic. That is more than MDG §11's "(Enhanced)" asks, and it matches D5 and MDG §6.2. Other colour-only cues are not covered (F8) |
| Adjustable dialogue paging (Enhanced) | GAP | There is no setting. Dialogue and transmission pages already wait for a key, but timed text screens advance on their own, and keys pressed during typing skip later pages (F7) |
| Separate music/SFX volumes | MEETS | 0–100 in tens, applied live (`crates/td2192/src/run.rs:284-291`, `crates/td-flow/src/front/options.rs` audio page) |
| Pause anywhere | PARTIAL | P pauses in flight, F2 freezes any screen, and the computer menu stops the ticks. A controller has no pause by default and cannot reach F2. Nothing pauses when the window loses focus. Timed screens can be held only with F2 (F5, F12) |
| Readable scaling at 4K | MEETS | Glyphs are at least 40 px tall at 3840 × 2160 in every mode (§3). Caveats: Enhanced widescreen makes all screen text smaller in narrower windows (F10). HiDPI and xBR on text were not tested (§5) |
| *Beyond §11:* controller-only play (MDG §9.5, port-front-end §5) | GAP | A controller cannot finish the save-name entry, and in Classic it cannot leave it either (F2) |
| *Beyond §11:* photosensitivity (WCAG 2.3.1) | GAP | Holding the infra-red key makes the whole screen flicker between dark and red at 5.45 Hz (F1). No setting reduces flashing (F3, §4) |

## 2. Findings, ranked by impact

Severity: blocker (a player with that need cannot finish the game), major, minor, cosmetic. **Safety** marks a photosensitivity hazard.

**F1. Major, safety: holding the infra-red key flickers the whole screen at 5.45 Hz.**
- Evidence:
  - The host turns every held bound key into typematic make codes, every 91.7 ms after the first 500 ms (`crates/td2192/src/flight.rs:107-108`, `:179-195`). Alt is included.
  - td-sim sets the key byte again on each make code (`crates/td-sim/src/input.rs:339-345`). The Alt toggle flips palette mode 1 ↔ 2 and clears the byte (`crates/td-sim/src/flight/mod.rs:889-898`; [weapons §4.1](../spec/sim/weapons.md)). The host then rewrites all 256 DAC entries (`crates/td2192/src/flight.rs:846-853`; [flight-screens §4.5.1](../spec/flow/flight-screens.md)).
  - Mode 1 is dark (R/8, G/8, B/4) and mode 2 is saturated red (min(63, 1.5R), 0, 0). So holding Alt or right Alt, or West on a controller (bound to alt by default, `crates/td-flow/src/controls.rs:219`, held through `pad_keys`), for more than 0.5 s gives 10.9 full-screen flips a second: 5.45 dark/red cycles a second.
  - It happens in the flights that start in infra-red ([mission-setup](../spec/sim/mission-setup.md), [e2-mission-setup](../spec/sim/e2-mission-setup.md)):
    - E1 flights 4 and 8, under both profiles;
    - E2 flights 3 and 10, only with the Enhanced fixes. Under Classic, E2's toggle back to mode 1 rewrites only entries 37 and 255 (OB-023, BUG-9; `crates/td2192/src/flight.rs:847-849`), so the picture stays red and only those entries flicker.
  - That is far above WCAG 2.3.1's general and red flash thresholds over the whole picture.
  - It follows the original's input model, because an AT keyboard repeats held modifier keys too, so the original very likely does the same. This was not checked against the oracle. A slow key release, common with motor impairments, is enough to trigger it.
- Recommendation:
  - The reduce-flashing option (§4) drops typematic repeats of the infra-red keys.
  - Propose an original-bugs entry for this behaviour and let the owner decide whether to fix it in all profiles on safety grounds. The bug policy (MDG §6.3) does not name health hazards.
  - Add a photosensitivity warning to the player guide.

**F2. Major (soft lock): a controller cannot finish a save, and under Classic cannot leave the name entry.**
- Evidence:
  - The name entry reads keys only (`crates/td-flow/src/save_screens.rs:305-310`) and ends only on Enter or keypad Enter (`:441`). Esc cancels it only with the Enhanced fixes (`:424`, OB-056).
  - No button stands for Enter (`crates/td-flow/src/controls.rs:207-227`): South types a space, the right shoulder types `m` and Start is Esc.
  - port-front-end §5 lists the controller additions (menu boxes, script choice, "hit any key") but not this screen, and the player guide does not mention saving with a controller.
  - A controller-only player who chooses save and then a slot is stuck under Classic until the window is closed. Under Enhanced, Start cancels the entry, but saving is impossible. This is not a blocker by the definition, because the campaign can be played without saving.
- Recommendation: South (joystick button 2) ends the entry like Enter under both profiles. That is a port addition like the menu boxes. Small fix candidate S1. Optionally save a default name when nothing was typed.

**F3. Major, safety: no setting reduces flashing.**
- The original's flashes are kept exactly, with no option to soften them:
  - the full-screen white flash of every homing-missile kill;
  - the hit flash of every nearby kill, in saturated red under infra-red mode 2;
  - the mission flashes of E1 flights 17, 18 and 20 and of the flight-17 cut-scene.
- The Enhanced preset's subtle lighting adds a large explosion glow per kill and a 9.6 Hz muzzle-flash flicker.
- A single event stays under WCAG's three flashes per second. Bursts of kills or homing kills within one second can exceed it.
- Analysis and design: §4.

**F4. Major: screen keys and host keys cannot be rebound ("full rebinding").**
- Evidence:
  - The original screens read fixed keys:
    - main menu: arrows, keypad 8/2, digits 8/2, Space/Enter/keypad Enter, Esc and `y` (`crates/td-flow/src/main_menu.rs:499`, `:555-560`);
    - save screens (`crates/td-flow/src/save_screens.rs:333-336`);
    - script choice: `1`/`2` on the main row only (`crates/td-flow/src/program.rs:806`);
    - computer menu: Up/Down, keypad, Esc, Space and Enter (`crates/td-flow/src/flight_screens/menu.rs:556-579`);
    - comm viewer: Space, then F6 or Esc (`crates/td-flow/src/flight_screens/comm.rs:416-427`);
    - tactical map: Esc, F5 and Tab (`crates/td-flow/src/flight_screens/map.rs:625-628`).
  - The port's own screens read fixed keys too (`crates/td-flow/src/front/ui.rs:367-377`).
  - Host keys: F2 (`crates/td2192/src/run.rs:68`) and F10–F12 (`:300-313`).
  - A player who moves the flight keys within reach of one hand still needs Space, Enter, Esc, the arrows, F5, F6 and `1`/`2` on the screens. MDG §9.5 lists Confirm, Menu/Back, DialogueNext and DialogueSkip as actions.
- Recommendation:
  - Add "menu" rows to the controls page (confirm, back, up, down, next page, skip). Feed them to the screens as the original keys, the way buttons are already injected, with the original keys as defaults.
  - Add rows for the host actions (options, scaling, fullscreen, screenshot).

**F5. Minor: during play, a controller cannot open the options, pause by default, or use the host keys.**
- Evidence:
  - F2 and F10–F12 are keyboard keys only. The host keys are handled before the button injection (`crates/td2192/src/run.rs:117`, `:124-127`).
  - F2 is removed before any screen sees it (`:135`), so it cannot even be captured as a binding.
  - P (pause) has no default button (`crates/td-flow/src/controls.rs:214-222`). Start (the computer menu) does stop the ticks.
  - The options are reachable by controller only from the start screen, which means quitting the episode. Because of F2, a controller player cannot save first.
- Recommendation:
  - A default pause button. Small fix candidate S3.
  - A bindable host "options" action with a controller default, agreed with the owner. Guide is often taken by the OS or Steam.

**F6. Minor: buttons stand for the *keyboard bindings*, and the "hit any key" waits do not read the joystick buttons.**
- Evidence:
  - `pad_keys` injects the player's keyboard key for the button's flight key (`crates/td-flow/src/controls.rs:289-306`).
  - The dialogue and story waits do not read joystick buttons 1 and 2, which the original reads ([dialogue-player §4.7](../spec/narrative/dialogue-player.md), [screens §2.2](../spec/flow/screens.md); "not modelled", `crates/td-flow/src/dialogue.rs:26-27`; `crates/td-flow/src/story.rs:306-311`).
  - So binding fire to left-ctrl makes South inject left-ctrl, which no wait accepts as a key (`story.rs:176-189`). South then no longer turns dialogue pages or ends story, hint and retry screens. Start still works there, but as Esc it skips the scene or declines the retry.
  - Rebinding the computer-menu key makes Start stop closing the computer menu and comm viewer and stop opening the quit box.
  - East (button 1) and West (Alt) never end a "hit any key" screen, although the guide says "a controller button" does.
- Recommendation:
  - The waits read buttons 1 and 2 as the original does. This is also a fidelity fix.
  - Inject the flight key's *original* key for the screens, and give the flight the button's flight key directly, so the keyboard and controller tables are independent.

**F7. Minor: no adjustable paging. Timed text screens can be missed.**
- Evidence:
  - Dialogue and comm pages wait for a key, and typing is fast (one character per frame, `crates/td-render/src/small_text.rs:36`). So paging is already at the reader's pace.
  - Timed screens close by themselves after their typing:
    - assignment orders: 5 DOS seconds, 4.74 s (`crates/td-flow/src/program.rs:285`);
    - enemy dossier and debrief: 10, or 15 in E3 (`:317`, `:421`);
    - pilot dossier: 25 (`:447`);
    - story so far: 30 and 45;
    - stills and cards ([screens §3.4–§3.10](../spec/flow/screens.md)).
  - Only F2 holds them.
  - Keys pressed while text types are kept, up to 15, and taken by the next page breaks (`crates/td-flow/src/dialogue.rs:508-513`). Pressing Space to hurry the typing therefore skips pages unread.
  - The HUD ticker scrolls at a fixed pace. M (repeat) mitigates this.
- Recommendation: an accessibility option "text waits for a key", outside the preset and in both profiles:
  - `Hold::Seconds` acts as `Hold::Key` (`crates/td-flow/src/story.rs:405-415`);
  - page breaks ignore keys pressed while the page was typing.
  - Both are front-end only: no td-sim state.

**F8. Minor: colour-only cues outside the guidance line.**
- Evidence:
  - The port's menus mark the selected row only by ink: red 225 against grey 224 (`crates/td-flow/src/front/options.rs:813-821`, `crates/td-flow/src/front/start.rs:274-281`). The two differ by only 1.72:1, the selected row is the *darker* one, and red looks darker still to protanopes.
  - Infra-red mode 2 is red-only, so protanopes lose most of its luminance in the four infra-red flights.
  - Radar ships (55h), shots (22h) and the player (46h) are single pixels told apart by colour only ([hud §3.4](../spec/render/hud.md)).
  - The compass lamps give the number of ships as red intensity and home as blue (hud §4.3).
  - The guidance markers differ only in colour, but the Okabe–Ito set keeps them apart.
- Recommendation:
  - A cursor mark on the port's screens, for example the font's block glyph before the row.
  - An "infra-red tint" option at the RGBA stage that maps mode 2's red to a neutral or amber of equal luminance.
  - Check the radar colours against `td.pal` (not read here).

**F9. Minor: two port inks are below 4.5:1 on black (WCAG 1.4.3).**
- Ink 229 (notes; 40, 10, 10) is 2.87:1. It carries "enter chooses - escape goes back", "affects gameplay", "enhanced options need the enhanced profile" and the profile line.
- Ink 226 (unavailable; 18, 18, 18) is 2.33:1. It carries `- not installed`, the Classic values of the Enhanced rows and unbound cells.
- Proposal, small fix candidate S4:
  - 229 → (56, 32, 32): 7.78:1, still reddish, 1.48:1 from the cursor red;
  - 226 → (28, 28, 28): 4.30:1, still 2.1:1 darker than the text grey.

**F10. Minor: Enhanced widescreen shrinks all screen text in narrower windows.**
- Evidence:
  - Front-end screens are centred in the W-wide frame ([presentation §4.3](../spec/enhanced/presentation.md); `crates/td2192/src/enhanced.rs:346-351`).
  - So in the default 1280 × 960 window (`crates/td-platform/src/lib.rs:202-203`) the preset's 16:9 draws text 17.9 px tall instead of 24 (aspect) and 10 instead of 20 (integer).
  - 21:9 on a 16:9 display gives 20.6 instead of 27 (aspect) and 15 instead of 25 (integer) (§3).
  - The upscaler also runs on every screen's text. xBR on a 5 × 5 font was not viewed.
- Recommendation: present non-flight frames with a 320-wide base, or make `auto` the preset width. This needs presentation §4.3 changed (owner).

**F11. Minor: a key capture accepts F10–F12.**
- The keyboard capture binds the first key pressed (`crates/td-flow/src/front/options.rs:585-597`), F10–F12 included. The host acts on those keys at the same time (`crates/td2192/src/run.rs:300-313`).
- A flight action bound to F11 therefore toggles fullscreen every time it is used.
- Small fix candidate S2.

**F12. Minor: nothing pauses when the window loses focus.**
- `Platform::poll` handles no window events (`crates/td-platform/src/lib.rs:297-322`), so a flight goes on behind another window. Alt+Tab also presses a game key.
- Recommendation: stop the clock while the window is unfocused, as the run loop already does after a data dialog (`crates/td2192/src/run.rs:192-195`), or open the flight's pause. This is host-only; td-sim is untouched.

**F13. Minor: save names and the yes key depend on key position, not on the character.**
- Names are built from HID usages (`crates/td-flow/src/save_screens.rs:511-533`) and the yes key is the `y` position (`crates/td-flow/src/main_menu.rs:499`).
- SDL text input is never started, and `InputSnapshot::text` is never read (`crates/td-platform/src/lib.rs:309`). MDG §9.5 asks for SDL text input.
- So AZERTY typists get `q` for `a`, and QWERTZ players must press the key labelled Z to answer yes.
- Recommendation: SDL text input for the name, and the typed character for the box.

**F14. Minor: limits of the controller bindings.**
- Only Delete clears a binding, so a controller cannot clear one (`crates/td-flow/src/front/options.rs:643-650`).
- A gamepad capture binds the first button, so a controller can cancel it only by waiting for the 5 s timeout (`:574-583`).
- South (fire), East (afterburner), the left stick and the D-pad are fixed as the original joystick (`crates/td-platform/src/lib.rs:486-490`), with a fixed 16,000 threshold (`:437`).
- Analog steering (a D5 option) is not implemented, and only the first controller is read (`:322`).

**F15. Minor: a hit on the player is mostly an audio cue.**
- The hit sound ([sfx-policy §3.2](../spec/audio/sfx-policy.md)) and the bars are the only cues. The "hit flash" belongs to kills and the player's death, not to hits ([hit-flash §4.2](../spec/sim/hit-flash.md)).
- The incoming transmission has a visual cue too: its chime comes with the F6 lamp animation.
- Recommendation: an optional steady, non-flashing hit indicator at the RGBA stage, with the §4 option.

**F16. Cosmetic: documentation and a release-build developer override.**
- The player guide:
  - says "hit any key" screens take "a controller button" (F6);
  - has no photosensitivity warning;
  - does not say how to save with a controller.
- MDG §11's "(Enhanced)" for the colour-blind line and for the paging item predates D5. Suggest "(accessibility option, both profiles)".
- `dialogue.rs:26-27` still says there is no joystick input.
- `[dev] tick_hz` in `settings.toml` changes the pace of every screen in release builds (`crates/td2192/src/run.rs:101`). MDG §9.12 keeps such overrides out of release, and it is no usable slow-down.

**Out of scope or by design:**
- There is no screen reader or system text-to-speech: all text is drawn into a 320 × 200 bitmap. A future option could speak dialogue and transmission lines through the OS.
- There is no mouse, as in the original.
- The first-run message box and file dialogs are the OS's own. They work by keyboard and screen reader, not by controller.
- Flights keep the original's time limits, and Classic must keep 19.2015 Hz. An accessibility "game speed" (70–100 % of the flight tick rate) would change only wall-clock pacing: td-sim stays per tick and replays are unaffected. Owner decision.

### 2.1 Small fix candidates

Each is a few lines. Each needs a test and the spec or guide line named.

- **S1 (F2), `crates/td-flow/src/save_screens.rs`.**
  - Add the field `south_held: bool` after `pad: Option<Pad>,` in `SaveScreen`, and `south_held: false,` after `pad: None,` in `SaveScreen::new`.
  - In `step`, replace

    ```rust
    for &k in &input.pressed {
        if is_keystroke(k) && self.keys.len() < KEY_BUFFER {
            self.keys.push((k, shift));
        }
    }
    ```

    with

    ```rust
    // Port addition (M11-04): South (joystick button 2) ends the name entry like Enter, so a
    // controller alone can save. The keys its binding stands for in this frame are dropped.
    let south = input.pad.is_some_and(|p| p.button2);
    let south_pressed = south && !self.south_held;
    self.south_held = south;
    if south_pressed && self.phase == Phase::Name {
        self.keys.push((Key::ENTER, false));
    } else {
        for &k in &input.pressed {
            if is_keystroke(k) && self.keys.len() < KEY_BUFFER {
                self.keys.push((k, shift));
            }
        }
    }
    ```

  - The South press that chooses the slot arrives while the phase is still `Choose`, so it does not save at once.
  - Also add the save-name entry to the list in port-front-end §5 and to player guide §5.
- **S2 (F11), `crates/td-flow/src/front/options.rs` `capture_tick`.**
  - Replace `if let (true, Some(&key)) = (pressed.is_empty(), input.pressed.first()) {` with

    ```rust
    // F10..F12 belong to the host (scaling, fullscreen, screenshot) and are never bound.
    let key = input
        .pressed
        .iter()
        .find(|k| !matches!(**k, Key::F10 | Key::F11 | Key::F12));
    if let (true, Some(&key)) = (pressed.is_empty(), key) {
    ```

- **S3 (F5), `crates/td-flow/src/controls.rs` `PadBindings::default`.**
  - After `(FlightKey::M, PadButton::RightShoulder),` add `(FlightKey::P, PadButton::RightStick),`.
  - Also update the doc comment, port-front-end §4 "Gamepad defaults" and the guide's button table.
- **S4 (F9), `crates/td-flow/src/front/ui.rs` `INKS`.**
  - Set `INK_NOTE` from `r: 40, g: 10, b: 10` to `r: 56, g: 32, b: 32`.
  - Set `INK_DISABLED` from `r: 18, g: 18, b: 18` to `r: 28, g: 28, b: 28`.
  - Also update the port-front-end §1 table and the start-screen snapshot hashes (`crates/td-flow/src/front/start.rs:611`).

## 3. Legibility

The small font (`td.lt2`) has 5 × 5 glyph cells, lower case only ([text §3.1](../spec/render/text.md)). Lines are 7 px apart in dialogue and on the controls page, and 13 px apart on the port's menus. The table below gives physical pixels for one glyph:
- aspect 4x3: horizontal × vertical scale, then glyph w × h;
- integer: the factor k, then the glyph;
- stretch: glyph w × h.

The scaling code is `crates/td-platform/src/present.rs:244-266` and `:213-234`, with sharp bilinear above the integer factor. The widescreen columns apply to *every* screen under the Enhanced profile with that width (F10). `auto` picks 320 in integer mode and 428 in aspect mode on a 16:9 window (`crates/td-render/src/enhanced/mod.rs:56-63`).

| Window or fullscreen | Aspect 4x3 | Integer | Stretch | 16x9 wide: aspect / integer h | 21x9 wide: aspect / integer h |
|---|---|---|---|---|---|
| 1280 × 720 | 3.00 × 3.60 → 15 × 18 | 3 → 15 | 20 × 18 | 17.9 / 10 | 13.7 / 10 |
| 1366 × 768 | 3.20 × 3.84 → 16 × 19.2 | 3 → 15 | 21.3 × 19.2 | 19.1 / 15 | 14.6 / 10 |
| 1280 × 960 (default window) | 4.00 × 4.80 → 20 × 24 | 4 → 20 | 20 × 24 | 17.9 / 10 | 13.7 / 10 |
| 1920 × 1080 | 4.50 × 5.40 → 22.5 × 27 | 5 → 25 | 30 × 27 | 26.9 / 20 | 20.6 / 15 |
| 2560 × 1440 | 6.00 × 7.20 → 30 × 36 | 7 → 35 | 40 × 36 | 35.9 / 25 | 27.4 / 20 |
| 3840 × 2160 | 9.00 × 10.80 → 45 × 54 | 10 → 50 | 60 × 54 | 53.8 / 40 | 41.1 / 30 |

- At 4K every Classic mode gives glyphs 50–54 px tall. On a 27-inch panel (about 163 ppi) 54 px is about 8.4 mm. At 1920 × 1080 on 24 inches (about 92 ppi) 27 px is about 7.5 mm. At 1366 × 768 on 14 inches 19 px is about 4.4 mm. All are larger than ordinary desktop UI text, so "readable scaling at 4K" is met.
- The window is created with `high_pixel_density` (`crates/td-platform/src/lib.rs:238`), and the presenter sizes against `canvas.output_size()`, which is in physical pixels (`present.rs:148`). So the picture uses every physical pixel.
- No minimum window size is set. How Windows display scaling of 150 % or more treats the 1280 × 960 initial window, which may then be taller than a 1080-line screen, was not checked.
- What limits legibility is the font itself: 5 × 5 lower case with six punctuation marks. That is T0 on the original screens.

## 4. Flashing analysis and a "reduce flashing" option

The flight DAC changes once per tick (52.1 ms). Thresholds are from WCAG 2.3.1: no more than 3 general or red flashes in any second unless the area is small (0.006 sr, about 28,000 px at 60 cm from a 24-inch 1080p screen). A flash is a pair of opposite changes of at least 0.1 relative luminance, the darker below 0.8. Saturated red counts whatever the luminance change.

| Source | Trigger | Area | Course | Highest rate | Assessment |
|---|---|---|---|---|---|
| Infra-red key held | Alt, right Alt or West held > 0.5 s in E1 flights 4 and 8 (and E2 flights 3 and 10 with the Enhanced fixes) | whole picture | dark ↔ saturated red every 91.7 ms | 5.45 cycles/s | **fails** both thresholds (F1) |
| White flash | homing-missile kill ([weapons §4.3](../spec/sim/weapons.md) step 3) | whole picture | white, (63, 63, B), (63, B, B), one tick each (156 ms), then restore ([flight-screens §4.5.4](../spec/flow/flight-screens.md)) | one per homing kill; launches at least 4 ticks apart (4.8/s ceiling) | one is under; two or three landing within a second exceed; the third frame is near-saturated red |
| Hit flash | a ship destroyed by a shot within about 19.5 tiles; the player's death ([hit-flash §4.2](../spec/sim/hit-flash.md)) | DAC 0..45 except 36 and 38 (share of the picture not measured) | rises at once by up to +18 on R and G (mode 2: R only), then falls over at most 9 ticks (469 ms) | one per kill that raises it | grey (20, 20, 20) → (38, 38, 20) is 0.08 → 0.31, so each is a flash; three kills about 5 ticks apart within a second exceed; red flashes in mode 2 |
| Mission flashes (E1) | flight 17 base destruction; flight 18 bomb; flight 20 lander; flight 17 cut-scene ([mission-logic §5.17–§5.20](../spec/sim/mission-logic.md), [flight-screens §4.10.1](../spec/flow/flight-screens.md)) | whole picture | 6 ticks red–yellow–white–yellow–red; white then end; 100 ms black, 100 ms yellow, 100 ms black | single events | under the count, but saturated and large; the cut-scene is two flashes in 0.4 s |
| Low-armour pulse, missile lamp, F6 lamp | armour ≤ 6; launches; pending transmission ([hud §4.4–§4.8](../spec/render/hud.md)) | HUD lamps and bar | 8-tick saw tooth (2.4 Hz) and lamp animations | continuous | small area: under the threshold |
| Enhanced subtle lighting (in the preset) | explosion: intensity 1.6 → 0.25 over 6 frames, r 56; muzzle flash: 1.2 on each volley's first tick, r 24 ([presentation §6.1](../spec/enhanced/presentation.md)) | gain ≥ 0.5 within about 37 px (6.8 % of 320 × 200) per explosion; about 14 px (1.0 %) for the muzzle | explosion: one rise and fall; muzzle: every second tick while the guns are held, 9.6 Hz | per kill (5 explosions each); continuous | the explosion glow adds a large flash per kill (about 3.7 times the area threshold at 1080p); the muzzle flicker is about half the area threshold at desktop distance, more on large screens viewed close |
| Fades, bloom | screen changes; bloom off in the preset | | 17 levels over about 34 frames | | gradual: not flashes |

**Recommended design: `accessibility.reduce_flashing`.** It is an options row on the accessibility page. It defaults to off, sits outside the preset and works in both profiles, like the colour-blind line.

1. **Infra-red keys: one toggle per press.** The host makes no typematic repeats for alt and right-alt (`crates/td2192/src/flight.rs:179-195`). Optionally, the presented palette cross-fades the toggle over 8 frames.
2. **Flight DAC excursions are capped.** Where the flight draws `self.dac.current()` (`crates/td2192/src/flight.rs:1011-1014`) and in the cut-scene's flash steps (`crates/td-flow/src/flight_screens/interlude.rs`), each entry is blended from the mode's steady palette toward the flash colour only as far as a relative-luminance change of 0.05. The steady palette is the DAC with no hit or white flash; 0.05 is half the WCAG step. This covers the hit flash, the white flash and the mission flashes. If the luminance cap proves awkward, scaling every excursion by 1/8 is a simpler fallback.
3. **Enhanced lighting is softened.** Explosion intensities are × 0.4, there is no muzzle-flash light, and bloom is off while the option is on (`td_render::enhanced` lights).
4. **Optional:** the steady hit indicator of F15.

**What it must not change:**
- td-sim code and state: `g_HitFlash`, `g_FlashFrames`, the palette mode, RNG and tick order.
- `FlightDac`'s own state, so the OB-029 and OB-032 variants stay exact and the option can be switched live.
- The frame with the option off. Pin this with a test like `td2192::flight::tests::colour_blind_guidance_changes_only_the_guidance_line`.

The infra-red repeat filter changes only the input stream, and replays record that stream (the made bits, `crates/td-sim/src/input.rs:313-319`). So a replay recorded with the option plays back identically, and other replays are unaffected. With the option on, td-sim's state hashes for a given recorded input are those of Classic.

## 5. What was not checked

- Nothing was built or run: no unit tests, no `options::tests::dump_pages`, no play.
- No real controller (Xbox or PlayStation layout), and no check of whether Steam Input or the OS takes the Guide button.
- No real 4K or HiDPI display. Windows display scaling at 125–200 % with the 1280 × 960 initial window was not tried.
- The xBR and scaleNx upscalers on the 5 × 5 font (text legibility under the Enhanced preset) were not viewed.
- Flash areas and rates in real play were not measured:
  - which part of the picture DAC entries 0..45 colour (needs `td.pal` and the tiles);
  - kill and homing-kill bursts;
  - no photosensitivity tool (for example PEAT or Harding) was run on recorded frames.
- The original's behaviour with Alt held (F1) was inferred from the spec and the typematic model, not captured in the oracle.
- The `td.pal` inks of the original screens (radar, save screen, comm viewer background 178) were not evaluated for colour-vision deficiencies.
- Screen readers, OS text-to-speech, mouse-only and switch access were not covered: the port draws all text as bitmaps and has no mouse support by design.
- Episodes 2 and 3 were checked only through the code paths they share with Episode 1, plus the E2 script choice (`crates/td-flow/src/program_e2.rs:582-588`).
