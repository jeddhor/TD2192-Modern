# 0.9.1 candidate review (started 2026-11-09; validated 2026-11-10)

This continues Claude's interrupted RE-21 and IM-28 work. Source is main `21a8432`
plus the current uncommitted changes. No commit, tag, remote or publication was made.
The original installation remains read-only. Version 1.0 is not declared.

## Completed implementation

- BUG-34: E3 flight 15's truck and flight 16's convoy leave at (168,199); flight 15's
  other exit remains (199,126). Class-only removal preserves position and armour;
  flight 16 posts its transmission on the following tick. C-0378 compares all three exits.
- BUG-33: dialogue setup precedes the fade-start frame; typing overlaps the last fade
  writes and reveals one character per frame. C-0324 now passes with zero startup
  allowance. New dense C-0381 coverage pins the first characters independently.
- U-585: retry taunts retain their hidden character-count typing delay before fading in.
  Regression checks cover the numeric lengths of all ten E1 taunts.
- Windows game uses the GUI subsystem, attaches an existing terminal without creating
  a console, preserves redirected output and records diagnostics in the user folder's
  `td2192.log`. The platform-only Win32 exception has an explicit safety justification.
- Optional Enhanced analog steering provides proportional stick thrust and stable facing,
  with full-strength keyboard/D-pad precedence. It is off by default and inert in Classic.
  Resolved inputs replay without a controller; analog recordings require format 4.
- U-582: a run-wide integer host clock preserves the newest keyboard press and repeat
  eligibility across screens. Flight opening latches repeat makes without injecting UI
  presses. The before/after-Esc timelines match C-0383/C-0384 through all 30 ticks;
  C-0325 pause behavior remains covered.
- OB-175: live Enhanced fixes suppress wrapped ghost ships, brackets and engine lights
  outside inclusive offsets -16..80. Classic and fixes-off preserve them; simulation hashes
  do not change.
- Artifact-only Windows/Linux tag workflow, archive/notice/checksum validation, repeatable
  smoke helper, and a read-only Windows Sandbox configuration generator are prepared.
  The workflow never creates or publishes a release.

## Validation

Logs are outside the repository at `../re-work/scratch/codex-*`. Final package checks ran on 2026-11-10.

| Check | Result |
|---|---|
| Workspace gate | PASS: fmt, all-target Clippy with `-D warnings` with and without data, and both `cargo test --workspace` runs. Each: 1,031 passed, 0 failed, 9 ignored across 76 test binaries/doc-test groups; data/oracle-gated tests skip by design only in the no-data run |
| Whole-flight oracle suites | With data: E1 36/36, E2 27/27, E3 36/36 (including the selector regression in each shared harness). Dialogue 4/4, screens 18/18, story 16/16, release plumbing 5/5 |
| Release helper tests | 11 passed; standard library only |
| Workflow | `actionlint .github/workflows/release.yml`: exit 0, no diagnostics |
| Replay determinism | Exact packaged tdtool: 90 replays × 5 rounds = 450 runs, no result differs between rounds. 88 replay fixtures verify directly; two `e3f10-hangar-{b,c}` fixtures require their captured setup and deterministically fail standalone verification, as documented by replay-soak. Their original setup is covered by the L5 oracle suite |
| Windows package | Static C runtime; required import check found no Visual C++ Redistributable dependency. GUI subsystem and redirected CLI output covered by release tests |
| Linux package | Built in existing Ubuntu 22.04.5 WSL; shared dependencies only libc, libm, libstdc++, libgcc_s and loader; all found |
| Archive hygiene | Both archives contain exactly nine expected files, no game data, target-specific permissive notices and Rust notices. Linux entries have owner 0/0 and modes 755/644 |
| Windows smoke | Fresh temporary extraction; both versions/help, 218/218 data check and all three episodes playable; 20-second dummy-driver flight, 19.1506 ticks/s measured over 16.7 seconds, no panic/error |
| Linux smoke | Fresh native temporary extraction and user directory; same CLI/data results; 20-second flight, 19.1397 ticks/s over 18.6 seconds, no panic/error |
| Visual output | Packaged Linux Classic and Enhanced flight-1 screenshots exported and inspected; window captures 1280×960, Enhanced output 1284×600. These are headless image checks, not interactive desktop/controller acceptance |
| Independent review | No actionable findings in dialogue/retry timing or keyboard-history plumbing; reviewers did not edit the changes they reviewed |

### Packages

Both archive sidecars were checked against freshly calculated SHA-256 values. Combined manifest:
`P:/msdos/TD2192/dist/TD2192-Modern-0.9.1-SHA256SUMS`.

| Archive (under `P:/msdos/TD2192/dist`) | Bytes | SHA-256 |
|---|---:|---|
| `TD2192-Modern-0.9.1-windows-x64.zip` | 4,620,524 | `957bd343df9342093faec3438a046e4a7abf0ca667f1b1ecfe223edae5670c07` |
| `TD2192-Modern-0.9.1-linux-x64.tar.gz` | 5,272,948 | `7dfb1b5bea1c8045ef2ccd43077643e0100e084146110867286d270e78ad2824` |

The unpacked directories beside these archives are ready to run. The prepared Sandbox config is
`P:/msdos/TD2192/dist/sandbox/TD2192-Modern-0.9.1.wsb`; its `.files` folder holds the exact final
Windows zip (same checksum), and package/data mappings are read-only. Sandbox was not launched.

The first integrated gate caught a replay-version regression in fuzz testing: malformed
legacy input could activate analog metadata and change its serialized version. The parser
now rejects analog markers in v2/v3 and requires one in v4. Strict round-trip/truncation
assertions are retained and extended to v4; focused replay/fuzz tests pass. The complete
complete gate passed after this correction. A separate stable retry-image fixture also needed to
wait through the newly restored hidden typing before comparing its hold image; the
character-count timing regression remains strict and its focused suite passed. The new
framebuffer-only C-0381 recording reused C-0324's scenario tag; full-flight capture
selection now prefers the newest matching RAM recording while preserving a failure when
RAM evidence is absent. Both the C-0324 lost-flight comparison and a synthetic selector
regression passed. An initial inline WSL visual command had invalid shell-variable
forwarding; explicit-script checks from both native extraction and the shared drive passed.

## Owner review still required

These are the existing release gates from [RELEASE.md](../RELEASE.md), not new requirements:

1. **DONE 2026-11-10:** owner signed off M12: "audio is great, enhanced features are all awesome."
2. Run the clean-machine interactive checks on Windows and Linux: first-run folder/ZIP,
   picture and audio, fullscreen/options/screenshots, save/restart/load, controller and
   desktop X11/Wayland where available. Local smoke checks do not certify these.
3. Settle D7 hosting/visibility, confirm the existing D8 script-choice and D10 unknown-data
   policies, and complete the existing R5 rights review before public release.
4. Review and commit the changes, then use the owner's matching-tag/remote procedure.
   The hosted release workflow has not run. Its artifacts and successful install checks
   are still required before tagging 1.0.

Windows Sandbox is disabled on this workstation; no Windows feature was enabled. Docker's
client exists, but its daemon is unavailable; no service/container was installed or started.
A Sandbox configuration can be prepared with `tools/scripts/new-sandbox-config.ps1`.

## Research limits retained honestly

See [RE-21's handoff](2026-11-09-RE-21.md) and [the joystick injection design](../re/oracle-joystick-injection.md).
The latter is a design only, as requested; the emulator extension and captures are not implemented.
U-580 is resolved: C-0382 confirms the launcher music-off pointer `6795:0000`,
matching the implemented arithmetic; the direct pointer was already confirmed.
U-581/U-583 retain measured raster/interrupt timing variants rather than fabricated universal
per-line constants. C-0383/C-0384 resolve U-582: repeat selection/eligibility persists
across screens and does not fall back to an older held key after Esc is released. Host
history plumbing preserves that behavior and has capture-backed regression coverage. Shared off-page
VGA bracket lifetime is not emulated. These bounded residues are documented alongside the
existing Classic checklist (363/376 PASS, 96.5%).

Clean-room attestation: implementation used written specs and captures only; the implementers
did not consult disassembly, decompiler output or Ghidra. The separate RE agent updated
prose/numeric facts and kept all original recordings outside the repository.
