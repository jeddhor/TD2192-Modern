# Credits screen

| | |
|---|---|
| **Spec ID** | `frontend/credits` |
| **Revision** | 1 |
| **Status** | Draft |
| **Fidelity tier** | T3. The original has no separate credits screen: its only credits are the captions of the intro flyover. Mode A replays that sequence exactly (T0/T1 as [intro](../flow/intro.md)); mode B is a static presentation of the same text |
| **Confidence** | CONFIRMED for the source of the text and its grouping (static + C-0047); the page layout of mode B is a proposal |
| **Tasks** | M11-05 |
| **Unknowns** | — |

## 1. Summary

The game's credits exist once, in the intro program: the 25 strings of resource `intro.credits` in `file0000.exe` ([file0000.toml](../../re/exe-resources/file0000.toml)), shown one at a time as flyover captions ([intro](../flow/intro.md) §3.5). Nothing else in the install carries credits:

- E1, E2, E3 and `td1.exe` contain none;
- `td.hlp` is two pages of control help;
- `aboutus.scr` (the about screen of E2/E3 menu item 6) is a publisher blurb about P² Multimedia;
- the episode end cards are story text;
- the TFP extras (`aboutus.tfp`, `order.tfp`) are decision D4.

`file0001`'s configuration screen has a one-line copyright notice, but it sits inside a crunched text page, not a string resource ([launcher](../flow/launcher.md) §4.9).

The port's credits screen therefore takes `intro.credits` and shows it either as the original flyover (mode A) or as grouped pages (mode B). The port's own credits (authors, licences, third-party notices) are separate and never mixed into the original's text.

## 2. Definitions

| Term | Meaning |
|---|---|
| `C[k]` | `intro.credits` entry k, 0..24 (ShortStrings, lower case, may start with spaces) |
| Text A | the intro's spaced small-font string routine ([intro](../flow/intro.md) §4.4) |

## 3. Data

The flyover's order defines the groups. Names that appear under several headings are the same string used again (`C[4]` three times, `C[5]` twice).

| Group | Heading | Names |
|---|---|---|
| 1. Title | — | `C[0]` (presenter line), `C[1]` (production line; its `^` is the superscript 2 of "P²"), `C[2]` (game title, letter-spaced) |
| 2. Programming | `C[3]` | `C[4]`, `C[5]`, `C[6]` |
| 3. Graphics | `C[7]` | `C[4]`, `C[8]`, `C[9]`, `C[10]` |
| 4. Music | `C[11]` | `C[5]` |
| 5. Story | `C[12]` | `C[13]` |
| 6. Thanks | `C[14]` | `C[15]` … `C[22]` (eight names) |
| 7. Director | `C[24]` | `C[4]` |

`C[23]` is fifteen spaces (a blank caption) and is not shown.

## 4. Behaviour

### 4.1 Mode A: the original sequence (Classic)

Run the flyover of [intro](../flow/intro.md) §4.6 exactly: setup with `td.pal`, steps 1..1618 with the caption table §3.5, the hoverskid, the scroll, `intro.cmf` then `intro2.cmf`, and the sound effects. Then a full fade out. Any key ends it early, as in the intro, and returns to the caller (the port's front end) instead of continuing to `intro1`. This is the only place the original shows the credits, so mode A is the faithful one.

### 4.2 Mode B: static pages (Enhanced, or when the flyover data is unavailable)

Proposal for M11-05. It reuses the intro's caption look so that no new art or text is needed:

- 320 × 200, black background (palette entry 0 = (0, 0, 0)), music `intro2.cmf` looping;
- every line in Text A, centred like the flyover captions: pen x = 160 − 6 × (len div 2), with `len` including leading spaces; a black copy at (x − 1, y + 1) first, then ink (0, 63, 63);
- one group per page. The heading pen row is 70 (the title group has none). Names start 14 rows below the heading (row 84 for the title group's first line) with a 10-row pitch. Group 6 (nine lines) starts its heading at row 50;
- each page: full fade in (65 levels at one per frame), hold 4 s or until a key, full fade out. Esc leaves the screen at once.

## 5. Edge cases

- `C[0]` starts with one space and `C[2]` with two. Centring must use the full length, as the original does, so these lines sit a few pixels left of an exact centre.
- `^` must be drawn as a `2` four rows higher (intro §4.4), never as a caret.
- The text is lower case only; render it with `td.lt2`, not a modern font (ADG pitfall 26).

## 6. Test vectors

| # | Check | Expected | Evidence |
|---|---|---|---|
| 1 | Mode A frames | identical to the intro flyover frames (intro §6 rows 4..6) | C-0047 |
| 2 | Mode B page 2 | heading `C[3]` at pen (160 − 6 × (15 div 2), 70) = (118, 70); names at rows 84, 94, 104 | this spec |

## 7. Presentation and audio hooks

Mode A: as the intro flyover. Mode B: `intro2.cmf` looping; no sound effects.

## 8. Open questions

- [ ] Whether the port's own credits (team, licences) follow on extra pages in the same style or on a separate modern screen is a UI decision for M11 (not RE).

## 9. Provenance

| Claim | Address / capture | Method | Confidence |
|---|---|---|---|
| Credits are only in `file0000` (`intro.credits`) | string search of every program; `td.hlp`, `aboutus.scr` viewed | static + decode | CONFIRMED |
| Grouping and order | `file0000:0000:1FA6..2353` (caption table) | static + C-0047 | CONFIRMED |
| Caption look (Text A, ink 46 = (0, 63, 63), black copy) | `file0000:0000:235F..263B`, `0000:0302` | static + C-0047 | CONFIRMED |

## 10. Revision history

| Rev | Date | Author (session/agent) | Change |
|---|---|---|---|
| 1 | 2026-11-05 | RE-13 (Archaeologist + Spec author) | Initial: credits source, grouping, two presentation modes |
