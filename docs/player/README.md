# TD2192-Modern player guide

TD2192-Modern lets you play **Traffic Department 2192** (Safari Software, 1994) on a modern computer, without DOS or an emulator. It is a new program that reads the game files from **your own copy** of the original. No game files come with it.

By default it looks, sounds and plays like the 1994 original. If you want, it can also show the city in widescreen, smooth the pixel art and add lighting effects.

This guide covers:

1. [What you need](#1-what-you-need)
2. [Installing](#2-installing)
3. [Pointing the game at your copy of TD2192](#3-pointing-the-game-at-your-copy-of-td2192)
4. [The start screen and the episodes](#4-the-start-screen-and-the-episodes)
5. [Controls](#5-controls)
6. [Options (F2)](#6-options-f2)
7. [Classic and Enhanced](#7-classic-and-enhanced)
8. [Bugs of the original game](#8-bugs-of-the-original-game)
9. [Where your saves and settings live](#9-where-your-saves-and-settings-live)
10. [Troubleshooting](#10-troubleshooting)
11. [Command line](#11-command-line)

> TD2192-Modern is still being developed, and this guide describes the current version. Some parts of the game are not finished yet; see [the project page](../../README.md) for the current state.

## 1. What you need

- **A copy of Traffic Department 2192 version 1.0.** Either form works:
  - an installed game folder, the one with `TD1.EXE` and `FILE0001.EXE` in it;
  - the original distribution ZIP file, as you downloaded it.

  The full game has 218 files and contains all three episodes. Episode 1 is needed to play at all; Episodes 2 and 3 can be played when their files are present.
- **A computer** running Windows (64-bit) or Linux (64-bit x86, glibc 2.35 or newer, for example Ubuntu 22.04 or later, with X11 or Wayland). A macOS build is planned.
- **Optional:** a game controller (Xbox-style or PlayStation-style), and a mouse is not needed at all.

## 2. Installing

**Windows:**

1. Download the TD2192-Modern package (`TD2192-Modern-<version>-windows-x64.zip`) and unpack it into a folder of your choice, for example `C:\Games\TD2192-Modern`.
2. Run `td2192.exe`.

**Linux:**

1. Download `TD2192-Modern-<version>-linux-x64.tar.gz` and unpack it, for example with `tar -xzf TD2192-Modern-<version>-linux-x64.tar.gz -C ~/Games`.
2. Run `./td2192` in the unpacked folder. Sound and graphics use the system's usual libraries (PulseAudio, PipeWire or ALSA; X11 or Wayland); nothing else needs installing.

Neither build is code-signed. Windows SmartScreen may warn about an unknown publisher the first time; choose "More info", then "Run anyway".

Nothing is installed into the system, and nothing is written to your game folder. To remove TD2192-Modern, delete its folder, and also its user folder if you want your saves and settings gone (see [section 9](#9-where-your-saves-and-settings-live)).

The package also holds `tdtool.exe` (`tdtool` on Linux), a helper used by developers. You do not need it to play, but it can check a game folder for you (see [section 10](#10-troubleshooting)).

## 3. Pointing the game at your copy of TD2192

### The first start

On the first start TD2192-Modern asks where your copy of the game is:

- **Folder...**: choose the installed game folder (the one with `TD1.EXE` and `FILE0001.EXE`). The game uses the files where they are. It only reads them and never changes anything in that folder.
- **ZIP file...**: choose the original ZIP file. The game reads the files straight from the archive; you do not have to unpack it.
- **Quit**: leave without choosing.

The game then checks every file against a list of the original files (their exact sizes and fingerprints) and works out which episodes you can play:

- **Episode 1 is complete:** the window opens on the **game data** screen, which shows what was found:
  - the folder or ZIP file;
  - how many files match the original;
  - which episodes are playable.

  Choose **continue** to go on to the start screen.
- **Episode 1 is not complete:** a message box says how many files match, which episodes would be playable and which files are missing. Then you can choose again.

Files that are present but differ from the original are reported as **modified**. They are only a warning: patched or other releases can still work.

### Extracting the ZIP file (optional)

If you chose a ZIP file, the game data screen offers **extract the zip**. This copies the game's files out of the archive into your user folder (`...\TD2192-Modern\game`, the screen shows the full path). From then on the game uses that copy, and you can move or delete the ZIP file.

- Only files that belong to the game are copied. Read-me files and other programs in the archive are left out.
- Nothing is extracted unless you choose to. If you choose **continue** instead, the game keeps reading the ZIP file each time it starts, which takes a moment longer.

### Changing it later

Press **F2** for the options, then choose **game data**. The page shows the folder or ZIP file in use, where that choice came from, its file check and the playable episodes. You can:

- **choose a folder**;
- **choose a zip file**;
- **extract the zip** (when a ZIP file is shown).

A new choice is checked at once:

- **A usable choice** is saved and is used from the moment you are back at the start screen. A game in progress keeps its data until then.
- **An unusable choice** changes nothing, and the page says why.

If the saved folder or ZIP file disappears (you moved or deleted it), the next start asks you again.

### For advanced users

You can also name the folder or ZIP file when starting the game, which wins over the saved choice:

- `td2192 --data <folder or zip>`;
- the environment variable `TD2192_DATA`.

The order is: `--data`, then `TD2192_DATA`, then the saved choice, then the first-start question. While `--data` or `TD2192_DATA` is in use, the game data page shows the data but cannot change it.

## 4. The start screen and the episodes

The original game was three separate programs, one per episode. TD2192-Modern starts on its own **start screen**:

| Item | What it does |
|---|---|
| episode 1, episode 2, episode 3 | Start that episode, from the beginning, as the original program would. An episode whose files are missing is shown as `not installed` and cannot be chosen. |
| options | The settings ([section 6](#6-options-f2)). |
| credits | The game's credits: the intro's desert flyover with the credit captions under the classic profile, or pages of credits under enhanced. |
| quit | Leave the game. Escape moves to quit; press it again to leave. |

Use the arrow keys (or the stick or D-pad) to choose and Enter (or South / A) to start.

**The intro.** Starting Episode 1 plays the original intro first, as the original launcher did. Escape skips it. Under the enhanced profile, **intro before episode 1** on the profile page chooses always, only the first time, or never. `td2192 --no-intro` skips it for one run.

Inside an episode everything is the original game: its own main menu, new games, loading and saving, the story scenes, the briefings, the flights and the debriefings. When you quit from the episode's main menu, you come back to the start screen.

**Episode 1's script choice.** When Episode 1 starts, it asks, as the original did, whether you want the original dialogue (press **1**) or the edited version (press **2**). With a controller, South (A) picks 1 and East (B) picks 2.

## 5. Controls

### In flight

These are the original keys. You can change every one of them ([section 6](#6-options-f2)).

| Key | Action |
|---|---|
| Keypad 8 / 2 / 4 / 6, or the arrow keys | Thrust up / down / left / right (the ship turns toward the direction) |
| Space | Fire. Hold it for the guns; press once per missile |
| Tab | Switch between the guns and missiles |
| Keypad + or keypad 0 | Afterburner burst (only ships that have one) |
| Alt (left or right) | Infra-red view on or off, where the ship has it |
| Esc | Computer menu (ship library, help, quit the flight) |
| F5 | Tactical map |
| F6 | Read an incoming transmission (when the indicator lights) |
| M | Repeat the last message |
| P | Pause; any key goes on |
| S | The original's sound-device switch. Not used by TD2192-Modern yet. |

### Menus and screens

The original menus use the arrow keys (or keypad 8 and 2), Enter or Space to choose, and Escape to go back. Screens that say "hit any key" take any key or a controller button.

### Game controller

A controller works as the original joystick:

- the **left stick or the D-pad** steers;
- **South (A)** fires;
- **East (B)** is the afterburner.

The other buttons are bound to keys, and each acts exactly like its key everywhere, in flight and in the menus:

| Button | Acts as | In flight |
|---|---|---|
| South (A) | Space | Fire (and "hit any key", menu select) |
| Start | Esc | Computer menu (and "back" everywhere) |
| Back / Select | F5 | Tactical map |
| North (Y) | Tab | Switch weapon |
| West (X) | Alt | Infra-red |
| Left shoulder | F6 | Read a transmission |
| Right shoulder | M | Repeat the last message |

On TD2192-Modern's own screens (the start screen and the options), the stick or D-pad moves, South confirms and East goes back. The original's own menus read the stick and buttons as the original did.

On Episode 1's script choice, South answers 1 and East answers 2. The main menu's yes/no questions take South for yes and East for no.

### Keys that work everywhere

| Key | Action |
|---|---|
| F2 | Options. The game waits while they are open |
| F10 | Change how the picture is scaled to the window (see Scaling below) |
| F11 | Fullscreen on or off |
| F12 | Save a screenshot ([section 9](#9-where-your-saves-and-settings-live)) |

Alt+Enter does **not** switch to fullscreen, because Alt is the infra-red key in flight. Use F11.

## 6. Options (F2)

Open the options with **F2** at any time, or from the start screen. Arrow keys move; left and right change a value; Enter opens a page or changes a value; Escape goes back. Changes take effect at once and are saved automatically.

Options marked * are Enhanced features. They can only be changed when the profile is **enhanced** ([section 7](#7-classic-and-enhanced)). Under the classic profile they are greyed out but keep their values.

### video

| Setting | Values | What it does |
|---|---|---|
| fullscreen | on, off | Fullscreen at the desktop resolution (F11 also switches it for the session) |
| scaling | aspect 4x3, integer, stretch | **aspect 4x3** keeps the original 4:3 shape (the 1994 screen had slightly tall pixels); **integer** uses whole-number scaling with square pixels, the sharpest; **stretch** fills the window |
| widescreen* | off, 16x10, 16x9, 21x9, auto | Shows more of the city to the left and right in flight. Marked **affects gameplay**, because you can see further than in the original. **auto** picks the widest that fits your window |
| pixel upscaler* | none, scale2x, scale3x, scale4x, xbr2, xbr3, xbr4 | Smooths the pixel art when it is enlarged |
| dynamic lighting* | off, subtle, full | Light from explosions, shots and engines; **full** adds a night look with shadows from buildings |
| bloom* | on, off | Bright things glow |

### audio

| Setting | Values | What it does |
|---|---|---|
| music volume | 0 to 100 | |
| sound volume | 0 to 100 | Sound effects |
| sound effects | classic, polyphonic | **classic** plays one effect at a time, as the original sound card setup did (a new sound cuts off the last one); **polyphonic** lets several play together |

### controls

One row for each key the game reads in flight:

- **action**: what the key does;
- **original**: the original key;
- **keyboard**: your keyboard key;
- **gamepad**: your controller button.

To change a binding, move to the cell and press Enter (or South), then press the new key or button within 5 seconds. Escape cancels. Delete clears the binding. A key or button can do only one thing, so binding it to a new action takes it from the old one; the screen says which. **reset to defaults** restores the original keys and the controller defaults above.

### profile

| Setting | Values | What it does |
|---|---|---|
| profile | classic, enhanced | See [section 7](#7-classic-and-enhanced) |
| intro before episode 1 | always, first launch, never | Enhanced only; the classic profile always plays the intro, as the original did |

### accessibility

| Setting | What it does |
|---|---|
| colour-blind guidance line | The guidance line (the short dotted line next to your ship that points at your target) normally comes in red for a target, green for a friendly ship and cyan for home. Those are hard to tell apart with red-green colour blindness, and the dark dots vanish on dark ground. With this on, the line is drawn in bright colours chosen to stay distinct for the common kinds of colour blindness: **orange** for a target, **sky blue** for a friend and **white** for home, each with a black outline. It works under both profiles. |

### game data

The folder or ZIP file the game reads, its file check and the playable episodes, with **choose a folder**, **choose a zip file** and **extract the zip**. See [section 3](#3-pointing-the-game-at-your-copy-of-td2192).

## 7. Classic and Enhanced

TD2192-Modern has two **profiles**:

- **Classic** (the default) is the original presentation: 320 × 200 pixels in the original 4:3 shape, the original timing, controls, music and sound effects, and the original rules.
- **Enhanced** lets you turn on the Enhanced video features one by one. Each starts off. Switching back to Classic turns them all off again, but remembers your choices.

The profile covers only the video features marked *. The sound-effects setting and the accessibility option are separate and work under either profile. No Enhanced feature changes how the game itself plays, except widescreen, which lets you see more.

### Widescreen

In flight the city extends to the sides:

| Setting | Picture width | Extra city on each side |
|---|---:|---:|
| 16x10 | 384 pixels | 32 pixels |
| 16x9 | 428 pixels | 54 pixels |
| 21x9 | 560 pixels | 120 pixels |
| auto | the widest that fits the window | |

The middle of the picture is exactly the original view, and ships, shots and explosions are drawn across the whole width. The instrument panel stays in the middle, with its ends mirrored outward. Menus, story scenes and the flight's full-screen pages stay at their original size in the middle. The pause message is drawn over the wide flight picture.

### Pixel upscalers

These redraw the low-resolution pixel art at 2, 3 or 4 times the size before it is scaled to your window:

- **scale2x, scale3x, scale4x** keep hard edges but round off staircase diagonals;
- **xbr2, xbr3, xbr4** blend edges more smoothly.

Higher factors look smoother on large screens and cost more processing time.

### Dynamic lighting and bloom

- **subtle**: daylight as in the original, plus glows from explosions, shots, missiles, engines and the afterburner.
- **full**: night. The city is dark and blue-tinted; lights brighten it, and buildings cast soft shadows.
- **bloom**: makes bright things (explosions, shots, the instrument lamps) glow softly.

The instrument panel and menus are never lit. In the infra-red view lighting stays at subtle, so the view remains readable.

### Performance

The heaviest combination (21x9, xbr4, full lighting, bloom) is budgeted at about 6 milliseconds per frame on a mid-range desktop processor. If the game stutters on a slower computer, choose a smaller upscaler (scale2x or xbr2) or turn lighting off.

### Screenshots

F12 always saves the original 320 × 200 picture. With any Enhanced video feature, or the colour-blind guidance line, on, it also saves the picture as drawn with them (`...enhanced.png`), at its full size before it is fitted to your window.

## 8. Bugs of the original game

The 1994 game has bugs and quirks. TD2192-Modern records each one it knows about and decides what to do with it:

- **Kept** when it affects how the game plays or feels, so the game plays as you remember it. Examples:
  - enemy ships that fire through some walls but hold fire at others;
  - mines in Episode 1 flight 7 that can come back after being destroyed;
  - ship speed changes that carry over into later flights of the same session;
  - double kills when two shots hit a ship in the same moment (Episodes 2 and 3);
  - an Episode 3 flight that can be finished early by flying home;
  - full-screen pictures drawn one pixel to the right;
  - radar blips one pixel off.
- **Fixed everywhere** when it crashes the game, hangs it, damages saves or blocks your progress. For example:
  - a sound played from very far away stopped the original with an error; it is now simply not played;
  - missing save files made the save and load screens crash; now the slots are empty and saving creates the files;
  - a damaged dialogue file could hang the game;
  - timed screens could vanish at once when the computer clock wrapped round;
  - in Episode 3 a flight-16 effect leaked into later flights after a retry or a loaded game and froze some ships, which could block the campaign.
- **Fixed in the Enhanced profile only** when it is just an annoyance. Examples:
  - music restarting from the top between scenes;
  - colours staying wrong after leaving the infra-red view (Episode 2);
  - a transmission screen that only a keyboard could close;
  - no way to cancel typing a save name;
  - a fragment of the hoverskid flashing at the bottom of the intro.

  In this version only the last one is fixed (the intro under the enhanced profile). The others are planned, so for now those annoyances remain under both profiles.
- **Left out** when it is hostile or only made sense on a 1994 PC:
  - a path in the original that could change your disk's volume label;
  - memory checks;
  - sound card setup.

The full list, with the reasons, is in [docs/spec/original-bugs.md](../spec/original-bugs.md) (written for developers).

## 9. Where your saves and settings live

Everything TD2192-Modern writes goes into one user folder:

| System | User folder |
|---|---|
| Windows | `%APPDATA%\TD2192-Modern\TD2192-Modern\` (usually `C:\Users\<you>\AppData\Roaming\TD2192-Modern\TD2192-Modern\`) |
| Linux (planned) | `~/.local/share/TD2192-Modern/TD2192-Modern/` |
| macOS (planned) | `~/Library/Application Support/TD2192-Modern/TD2192-Modern/` |

Inside it:

| What | Where |
|---|---|
| Settings | `settings.toml`, a text file you can edit while the game is closed. Delete it to go back to all the default settings (the game then asks for your game data again) |
| Saved games | `saves\`: the save slots of each episode, in the original's own file format (`td.sav` and `td.stt` for Episode 1, `td2.*` and `td3.*` for the others) |
| Screenshots | `screenshots\`: `td2192-<number>.png`, plus `td2192-<number>.enhanced.png` with Enhanced features on |
| Extracted game files | `game\`, only if you extracted the ZIP file |

TD2192-Modern never writes into your original game folder. It does not read the original game's own save files either, so games saved with the original DOS version do not appear in TD2192-Modern.

## 10. Troubleshooting

**"does not hold a complete Traffic Department 2192" or "not installed" on the start screen.**
- Make sure you chose the folder that holds `FILE0001.EXE`, not a folder above or below it.
- Run `td2192 --check` in a terminal for a short report.
- For a file-by-file report, run `tdtool inventory --data <folder or zip>`.
- A ZIP file is searched for the game files in every folder inside it, so the layout of the archive does not matter.

**Some files are reported as modified.** That is a warning, not an error: your copy differs from the version 1.0 files this port was checked against, for example because it was patched. The game still tries to use them. Text that the game reads from the original programs may be missing if those programs differ.

**The ZIP file is "not a readable ZIP archive", or files in it are "unreadable".** The file may be damaged or incomplete, or it may use a compression method the game cannot read (encrypted archives, for example). Unpack it with your system's own tool, then choose the folder instead.

**I want to change the game data, but the page will not let me.** The data was set with `--data` or the `TD2192_DATA` environment variable, which take precedence. Change it there, or remove them.

**The Enhanced options are greyed out.** Set **profile** to **enhanced** under F2, profile.

**The picture is blurry, too small, or the wrong shape.** Press F10 to cycle the scaling (aspect 4x3, integer, stretch), or change it under F2, video. **integer** gives the sharpest pixels.

**No sound.** Check the music and sound volumes under F2, audio, and that your system's default playback device is working; TD2192-Modern plays through the default device.

**My controller does nothing.** Connect it before or while the game runs; it is picked up when it connects. Controllers that the system does not recognise as a game controller (for example some flight sticks) may not work.

**The game stutters.** Turn off lighting or choose a smaller pixel upscaler, see [Performance](#performance).

**Fullscreen does not work with Alt+Enter.** Use F11; Alt is a game key.

**I lost my settings or controls.** Delete `settings.toml` in the user folder to start again from the defaults, or use **reset to defaults** on the controls page.

**Something else went wrong.** To keep a log, start the game from a terminal with the environment variable `TD2192_LOG=debug`. The log is written to the terminal and helps the developers find the problem.

## 11. Command line

You do not need any of these to play, but they can help:

| Command | What it does |
|---|---|
| `td2192` | Start normally: the start screen |
| `td2192 --episode N` | Start Episode N (1, 2 or 3) directly, skipping the start screen, as the original programs started |
| `td2192 --data <folder or zip>` | Use this game folder or ZIP file for this run (it wins over the saved choice) |
| `td2192 --fullscreen` | Start in fullscreen |
| `td2192 --no-intro` | Start Episode 1 without the intro |
| `td2192 --check` | Print which game data is used, its file check and the playable episodes, then exit |
| `td2192 --help` | List every option |
