# TD2192-Modern: Master AI Agent Development Guide

| | |
|---|---|
| **Document** | Agent Development Guide (ADG) |
| **Version** | 0.1, 2026-11-02 |
| **Audience** | AI coding agents (Claude Code and others) and the humans directing them |
| **Read with** | [MASTER_DESIGN_GUIDE.md](MASTER_DESIGN_GUIDE.md) (MDG): what and why. This guide: how. |

[`AGENTS.md`](../AGENTS.md) at the repository root is the short, always-loaded summary of this guide. This document is the full manual. Read it once per session before your first task, and re-read the section for your hat and milestone before you start work.

---

## Table of contents

1. [How to use this guide](#1-how-to-use-this-guide)
2. [Non-negotiable rules](#2-non-negotiable-rules)
3. [Orientation: where truth lives](#3-orientation-where-truth-lives)
4. [Environment and tooling](#4-environment-and-tooling)
5. [Hats and the clean-room protocol](#5-hats-and-the-clean-room-protocol)
6. [Task lifecycle](#6-task-lifecycle)
7. [Reverse-engineering playbook (Archaeologist)](#7-reverse-engineering-playbook-archaeologist)
8. [Spec-writing playbook (Spec author)](#8-spec-writing-playbook-spec-author)
9. [Implementation playbook (Implementer)](#9-implementation-playbook-implementer)
10. [Testing playbook](#10-testing-playbook)
11. [Verification against the original (Verifier)](#11-verification-against-the-original-verifier)
12. [Documentation duties](#12-documentation-duties)
13. [Git, branches, commits and reviews](#13-git-branches-commits-and-reviews)
14. [Multi-agent coordination](#14-multi-agent-coordination)
15. [Per-milestone agent playbooks](#15-per-milestone-agent-playbooks)
16. [Pitfall catalogue](#16-pitfall-catalogue)
17. [Templates](#17-templates)
18. [Quick reference](#18-quick-reference)

---

## 1. How to use this guide

- **Starting a session:** read `AGENTS.md`, then [STATUS.md](STATUS.md) to see what is in progress and what is blocked, then the section of this guide for your hat (§7–§11) and your milestone (§15).
- **Picking work:** take the lowest-numbered unblocked task in the current milestone whose dependencies are done, unless the user assigned one. See §6.1.
- **When unsure:** design questions go to the MDG; process questions go here. If neither answers it, **stop and ask the user**. Do not invent policy. Record any answer you get in the right document.
- **When you find this guide wrong or incomplete:** fix it in the same change (a small, focused edit) and mention it in your summary.

## 2. Non-negotiable rules

1. **Never modify the original game directory** (`P:\msdos\TD2192\TD2192`). Treat it as read-only. DOSBox runs a *copy* (`re-work\dosbox\c\TD`).
2. **Never commit original game data** or anything derived from it: files, slices, PNG/WAV renders, extracted text, JSON dumps of content. Hashes, sizes, offsets and numeric facts are fine (MDG §5.2–5.3).
3. **Never commit decompiler output or disassembly**, and never write production code by transliterating it. Implementers work from specs (§5).
4. **One hat per task per session.** State your hat at the start of the task. An Archaeologist session never writes production code for the subsystem it studied.
5. **Specs before code** for any behaviour of the original game. Code cites the spec section it implements.
6. **Determinism in `td-sim`:** no floats in Classic gameplay paths, no wall clock, no unseeded randomness, no hash-order iteration. See MDG §8.6.
7. **Respect crate boundaries** (MDG §8.2). Do not add a dependency edge that points "up". Do not add `unsafe` outside `td-platform` and `td-opl-sys`.
8. **Tests accompany changes.** Behaviour changes come with tests. Data-gated tests use `require_data!()` and must skip cleanly without data.
9. **Leave the tree green:** `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass before you hand off.
10. **Write knowledge down** in the same change that produced it: formats, unknowns, specs, STATUS, symbols CSV. Anything left only in your context is lost.
11. **Never reproduce hostile behaviour** of the original (for example the `label.exe` disk-relabel path).
12. **Ask before** anything irreversible or outward-facing: pushing, creating remote repos, publishing, deleting non-scratch files, changing ADR-level decisions, adding a copyleft dependency.
13. **Report honestly.** If a test fails, a step was skipped, or a finding is uncertain, say so plainly, with the confidence level.

## 3. Orientation: where truth lives

### 3.1 Document precedence

When sources disagree, the higher one wins. Then fix the lower one in the same change.

1. User instructions in the current conversation.
2. Accepted ADRs (`docs/adr/`).
3. MDG (`docs/MASTER_DESIGN_GUIDE.md`).
4. Specs (`docs/spec/`), at their stated confidence.
5. RE knowledge base (`docs/re/`).
6. This guide (on process questions it outranks 3–5).
7. Code comments.

### 3.2 Repository map

| Path | What | Who writes |
|---|---|---|
| `AGENTS.md`, `CLAUDE.md` | Always-on rules | Anyone (small edits), user approval for rule changes |
| `docs/MASTER_DESIGN_GUIDE.md` | Design | Spec authors / leads; design changes need an ADR or user OK |
| `docs/AGENT_DEVELOPMENT_GUIDE.md` | Process | Anyone (keep it accurate) |
| `docs/STATUS.md` | Task board | Every agent (claim, progress, done) |
| `docs/adr/` | Decisions | Proposed by agents, accepted by user |
| `docs/re/FILE_INVENTORY.md` | All 218 files, hashes, purposes | Archaeologists |
| `docs/re/FORMATS.md` | File format knowledge | Archaeologists, Implementers (when decoding proves something) |
| `docs/re/EXECUTABLES.md` | Exe facts, load bases, anchors | Archaeologists |
| `docs/re/UNKNOWNS.md` | Open questions register | Everyone |
| `docs/re/symbols/<exe>.csv` | Our names for original addresses (source of truth for Ghidra naming) | Archaeologists |
| `docs/re/exe-resources/<exe>.toml` | Offsets of exe-embedded text/tables | Archaeologists |
| `docs/re/*.md` (others) | Hardware access, globals, structures, load sites, episode deltas | Archaeologists |
| `docs/spec/` | Behaviour specs | Spec authors |
| `docs/parity/` | Parity reports, playthrough checklists | Verifiers |
| `docs/handoffs/` | Handoff notes for unfinished work | Any agent ending mid-task |
| `crates/` | Rust code | Implementers |
| `tools/ghidra/`, `tools/scripts/` | Our RE/dev scripts | Archaeologists / anyone |

### 3.3 Outside the repository

| Path | What | Rules |
|---|---|---|
| `P:\msdos\TD2192\TD2192\` | Pristine original install | Read-only. Never write. |
| `P:\msdos\TD2192\re-work\ghidra\` | Ghidra project | Single-writer (§14.4) |
| `P:\msdos\TD2192\re-work\exports\` | Decompiler and disassembly exports | **Archaeologist eyes only** |
| `P:\msdos\TD2192\re-work\dosbox\` | DOSBox configs + writable game copy | Verifiers / Archaeologists |
| `P:\msdos\TD2192\re-work\captures\` | Reference captures + `MANIFEST.md` | Verifiers; any hat may *view* captures |
| `P:\msdos\TD2192\re-work\scratch\` | Throwaway analysis | Anyone |
| `C:\Users\jason\Desktop\ghidra_12.2_PUBLIC\` | Ghidra install | Don't modify |

## 4. Environment and tooling

### 4.1 Machine facts (as surveyed 2026-11-02)

| Tool | Status / location |
|---|---|
| OS | Windows 11 Pro. PowerShell 7 is the primary shell; Git Bash is available. |
| Rust | rustup, `stable-x86_64-pc-windows-msvc` (cargo 1.95) and 1.99.1 installed. Pin in `rust-toolchain.toml` (M0-01). |
| MSVC | Visual Studio 2026 Community (`C:\Program Files\Microsoft Visual Studio\18\Community`) |
| LLVM | clang 22.1 (`C:\Program Files\LLVM\bin`) |
| CMake | 3.31 (no Ninja on PATH) |
| Git / GitHub CLI | git 2.45, gh 2.97 |
| Python | 3.10.11 with Pillow 12.3 and numpy 2.2 (scratch analysis only, never a runtime dependency) |
| Node | 22.18 (not used by the project) |
| .NET | 10.0 SDK (not used by the project) |
| Java | Temurin JDK 25 at `C:\Program Files\Eclipse Adoptium\jdk-25.0.5.8-hotspot` (**not on PATH**; set `JAVA_HOME` for Ghidra) |
| Ghidra | 12.2 at `C:\Users\jason\Desktop\ghidra_12.2_PUBLIC` (GUI `ghidraRun.bat`, headless `support\analyzeHeadless.bat`, PyGhidra `support\pyghidraRun.bat`) |
| DOSBox-X | Source at `P:\msdos\TD2192\dosbox-x` (GPL). Built locally (VS solution `vs\dosbox-x.sln`, Release x64, `/p:PlatformToolset=v145`; `C_DEBUG` and `C_HEAVY_DEBUG` are on in `vs\config.h`). It is a **tool only**: Implementers must not consult its sound, ADPCM or OPL source when writing the port's equivalents. Write those from public format descriptions. |

### 4.2 Environment variables

| Variable | Value | Used by |
|---|---|---|
| `TD2192_DATA` | `P:\msdos\TD2192\TD2192` | Data-gated tests, `td2192`, `tdtool` |
| `TD2192_REWORK` | `P:\msdos\TD2192\re-work` | Scripts |
| `GHIDRA_HOME` | `C:\Users\jason\Desktop\ghidra_12.2_PUBLIC` | `tools/scripts/ghidra-headless.ps1` |
| `JAVA_HOME` | `C:\Program Files\Eclipse Adoptium\jdk-25.0.5.8-hotspot` | Ghidra |

PowerShell (per session): `$env:TD2192_DATA = 'P:\msdos\TD2192\TD2192'`. Persistent: `[Environment]::SetEnvironmentVariable('TD2192_DATA','P:\msdos\TD2192\TD2192','User')`. **Ask the user before setting persistent variables.**

### 4.3 Everyday commands

```powershell
cargo build --workspace
cargo test --workspace                       # data-gated tests skip without TD2192_DATA
$env:TD2192_DATA='P:\msdos\TD2192\TD2192'; cargo test --workspace   # full run
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo run -p tdtool -- inventory --data $env:TD2192_DATA
cargo run -p td2192 -- --data $env:TD2192_DATA --dev-mapview e1     # from M4
```

### 4.4 Scratch analysis

Throwaway Python or PowerShell for probing files is fine (it is how the first format survey was done). Keep it in `re-work\scratch\` or the session scratchpad, never in the repo. When a probe proves something, record the finding in `docs/re/` and, where it matters, encode it as a Rust parser plus test. Scripts worth keeping are rewritten as `tdtool` commands or committed under `tools/scripts/`, and they must not embed game data.

## 5. Hats and the clean-room protocol

### 5.1 The hats

| Hat | Purpose | May read | May write | Must not |
|---|---|---|---|---|
| **Archaeologist (RE)** | Find out how the original works | Original binaries and data, Ghidra, `re-work/exports`, DOSBox debugger, captures, all docs | `docs/re/**`, `docs/spec/**` (drafts), `tools/ghidra/**`, `tools/scripts/**`, UNKNOWNS, STATUS | Write or edit `crates/**` production code for the subsystem studied in this session |
| **Spec author (SP)** | Turn findings into implementable specs | Everything an Archaeologist may read | `docs/spec/**`, `docs/re/**`, UNKNOWNS, STATUS | Paste decompiler output; transliterate code |
| **Implementer (IM)** | Write the port | `docs/**`, `crates/**`, captures (to look at), public format docs, third-party crate docs | `crates/**`, tests, `tdtool`, docs for code-level detail, STATUS | Open `re-work/exports`, Ghidra, or disassembly. If a spec is insufficient, file a question (§5.3) |
| **Verifier (VE)** | Prove the port matches the original | Running port, DOSBox (play/observe, debugger RAM views), captures, specs, code | Tests, replays, `docs/parity/**`, bug reports, captures (+ manifest), STATUS | Fix production code in the same task (file a bug, then an IM task fixes it) |
| **Reviewer (RV)** | Check a change before merge | The diff, the specs, the docs | Review comments; small doc fixes | Approve its own work |
| **Coordinator (CO)** | Plan, split and sequence work; keep STATUS healthy | Everything | STATUS, task briefs, handoffs | Do large implementation |

Pure infrastructure tasks (CI, build, tooling, generic engine plumbing with no original-behaviour content) can be done by an Implementer without a spec.

### 5.2 Hat switching

- One hat per *task*. A session may do several tasks, but **the same session must not do RE and IM on the same subsystem**. A fresh session (new context) starts the IM task with only the spec.
- Spec authoring right after RE in the same session is allowed and encouraged (the SP hat exists to turn findings into clean prose).
- Subagents are a good way to separate hats: the coordinator spawns an IM subagent whose brief contains only spec paths, and an RE subagent whose brief permits exports.

### 5.3 When a spec is not enough (Implementer)

1. Do not guess silently, and do not open Ghidra.
2. Add a question to the spec under `## Open questions` (or to `docs/re/UNKNOWNS.md` if it is cross-cutting), tagged with your task ID.
3. If you can proceed with a clearly isolated assumption, implement behind a named constant or function with `// ASSUMPTION(U-0xx): ...` and a test that pins the assumption, so it is easy to find and change.
4. Add an RE task to STATUS.md to resolve it.

### 5.4 What "in our own words" means for specs

- **Allowed:** tables of constants; record layouts (field, offset, size, meaning); state machines; formulas; event orderings; pseudocode written fresh with meaningful names and structured control flow at the level of *what is computed and in what order*. Exact order of operations often matters for T1 parity, and when it does, the pseudocode must preserve it.
- **Not allowed:** copying decompiler C or assembly; mechanically renaming decompiler variables and keeping its shape (gotos, temporaries, register juggling); comments that are verbatim from exports.
- **Provenance:** every spec has a `## Provenance` section: addresses (§7.8 notation), evidence type (static analysis, dynamic trace, capture, asset decode) and confidence.

## 6. Task lifecycle

### 6.1 Claim

1. Pull or rebase; read STATUS.md.
2. Pick a task whose dependencies are done and which is not claimed (or was assigned to you by the user).
3. Edit its row: `Status = IN PROGRESS`, `Owner = <agent/session label>`, `Started = <YYYY-MM-DD>`. Commit that edit alone, or include it as the first change in your branch if you work in a worktree (§14).
4. If the task is bigger than one focused session (roughly a day of human work), split it first into sub-tasks `Mx-yy.a`, `.b`, ... in STATUS.md.

### 6.2 Plan

- Re-read the relevant MDG section, specs, FORMATS/UNKNOWNS entries and any handoff note.
- Write a short plan (in your reply or the PR description): goal, files you will touch, tests you will add, how you will verify.

### 6.3 Execute

- Work in small commits that each build.
- Keep within your task's declared files and modules. If you must touch something shared (STATUS, FORMATS, UNKNOWNS, MDG), keep the edit minimal and focused.

### 6.4 Verify

- Run the full check suite (§4.3) with and without `TD2192_DATA`.
- For behaviour: run the relevant L2–L5 checks (§10, §11).
- Look at the output yourself (render to PNG in scratch, listen to the WAV). A green test is not proof that the output is right if the golden value was produced by the same code (§10.5).

### 6.5 Document

The documentation duties are in §12. At minimum: STATUS row updated, and any new knowledge in `docs/re` or `docs/spec`.

### 6.6 Hand off or finish

- **Done:** STATUS `DONE` with date and PR/commit; summary in the PR or final message: what changed, how it was verified, what is left, new unknowns.
- **Not done:** write `docs/handoffs/<YYYY-MM-DD>-<task-id>.md` (template §17.2); STATUS `PAUSED` with a link to the handoff. Leave the tree green: incomplete work goes behind a disabled code path or stays on a branch.

### 6.7 Definition of Done (all tasks)

- [ ] Task goal met; acceptance criteria in STATUS/MDG satisfied.
- [ ] fmt, clippy and test are green, with and without data.
- [ ] New behaviour has tests at the right level; data-gated tests skip cleanly.
- [ ] Specs, FORMATS, UNKNOWNS and symbols updated as applicable.
- [ ] No original data, derived assets, or decompiler output in the diff (`git diff --stat` reviewed; no binary files unless they are our own).
- [ ] Clean-room attestation in the PR (hat used; IM did not read exports).
- [ ] STATUS updated.

## 7. Reverse-engineering playbook (Archaeologist)

### 7.1 Targets and priorities

| Exe | Size | Role | Priority |
|---|---|---|---|
| `file0001.exe` | 267,264 | Episode 1: the full engine + E1 content | **1** (primary; everything is learned here first) |
| `td2.exe` | 242,496 | Episode 2 | 2 (via Version Tracking from file0001) |
| `td3.exe` | 224,128 | Episode 3 | 2 |
| `file0000.exe` | 47,728 | Intro | 3 |
| `td1.exe` | 33,776 | Launcher | 4 (small; switches, config, tdstatus.dat) |
| `aboutus/order/helpme/nuffmem/catalog/order2` | small | Extras | Ignore unless a decision (D4) requires them |

All game exes are Borland Pascal 7 MZ binaries, **unpacked**, **no overlays**. MZ header facts are in [re/EXECUTABLES.md](re/EXECUTABLES.md).

### 7.2 Ghidra project setup (M0-08)

GUI route: start `C:\Users\jason\Desktop\ghidra_12.2_PUBLIC\ghidraRun.bat`. If it asks for a JDK, give the Temurin 25 path. Create a non-shared project at `P:\msdos\TD2192\re-work\ghidra\TD2192`. Import each exe with **Format: Old-style DOS Executable (MZ)** and **Language: x86:LE:16:Real Mode:default**. Accept auto-analysis with defaults, then the extra steps in §7.4.

Headless route (preferred for agents):

```powershell
$env:JAVA_HOME = 'C:\Program Files\Eclipse Adoptium\jdk-25.0.5.8-hotspot'
$G = 'C:\Users\jason\Desktop\ghidra_12.2_PUBLIC'
$P = 'P:\msdos\TD2192\re-work\ghidra'
foreach ($exe in 'file0001.exe','td2.exe','td3.exe','file0000.exe','td1.exe') {
  & "$G\support\analyzeHeadless.bat" $P TD2192 `
     -import "P:\msdos\TD2192\TD2192\$exe" `
     -loader MzLoader -processor 'x86:LE:16:Real Mode' `
     -analysisTimeoutPerFile 3600 -log "$P\import-$exe.log"
}
```

Running our scripts against an existing program (read-only, no re-analysis):

```powershell
& "$G\support\analyzeHeadless.bat" $P TD2192 -process file0001.exe -noanalysis -readOnly `
   -scriptPath 'P:\msdos\TD2192\TD2192-Modern\tools\ghidra' `
   -postScript ExportFunctionIndex.java 'P:\msdos\TD2192\re-work\exports\file0001-functions.csv'
```

- A Ghidra project is **locked** while the GUI has it open. Headless runs against it will fail. See §14.4 for the single-writer rule.
- Record each program's **image base segment** (Program Information / Memory Map) in EXECUTABLES.md. The address notation (§7.8) depends on it.
- Java GhidraScripts always work headless. Python 3 scripts need PyGhidra (`support\pyghidraRun.bat`). Prefer Java for committed scripts unless PyGhidra headless is verified on this machine.

### 7.3 Our Ghidra scripts (`tools/ghidra/`, M0-09)

| Script | Does |
|---|---|
| `ExportFunctionIndex.java` | CSV: address, name, size, callers, callees, has-stack-check, segment |
| `ExportPascalStrings.java` | Scans data and code segments for plausible ShortStrings; CSV with address, length, text, xrefs |
| `ApplySymbols.java` | Reads `docs/re/symbols/<exe>.csv` and applies names, comments, signatures and data types |
| `ExportSymbols.java` | Writes Ghidra-side names back to CSV (for agents who annotated in the GUI) |
| `ExportDecompiled.java` | Per-function decompiled C into `re-work\exports\<exe>\<seg>_<off>_<name>.c` (**never into the repo**) |
| `FindPortIO.java` | Lists `IN`/`OUT`/`INT` instructions with operands to support `hardware-access.md` |

**`docs/re/symbols/<exe>.csv` is the source of truth for names.** Ghidra databases are binary and cannot be merged, but CSVs can. Format:

```csv
address,kind,name,signature,comment,confidence,source_task
0123:0045,func,Sim_UpdatePlayerShip,"void far pascal(void)","per-tick player update",LIKELY,M1-07
0456:0010,data,g_PlayerShipIndex,u16,"index into g_Ships",CONFIRMED,M1-07
```

`address` uses the image-relative notation of §7.8.

### 7.4 Borland Pascal 7 field guide

Knowing the compiler's habits turns unreadable 16-bit code into recognisable structure:

- **Segments:** each unit is compiled into its own code segment. The **main program's code** is typically the first segment and the **System unit** the last. The main block begins by calling every unit's initialisation in `uses` order, which gives you a free map of the unit boundaries.
- **DS = DGROUP:** globals and typed constants live in the data segment, addressed as `DS:offset`. **Set the DS register value across the whole program** (Ghidra: select all, *Set Register Values*, DS = data segment) so that string and global references resolve. This is the single most important manual step.
- **Prologue / epilogue:** `push bp; mov bp,sp`, often followed by a stack check (`mov ax,<locals>; call far <StackCheck>`) when compiled with `{$S+}`. Then `sub sp,n`. Functions end in `retf n` (far) or `ret n` (near); **the callee pops its arguments** (Pascal convention).
- **Argument order:** Pascal pushes left to right, so the *first* parameter is at the *highest* address. Ghidra's 16-bit callee-pop conventions assume C order, so **parameters appear reversed**. Fix the signature or note it.
- **Nested procedures:** receive the parent's frame pointer (static link) as a hidden extra parameter (`push bp` before the call). They show up as functions that read through `[bp+4]`/`[bp+6]` to an outer frame.
- **Return values:** byte/word in `AL`/`AX`; longint/pointer in `DX:AX`; `Real` (6-byte Real48) in `DX:BX:AX`; strings via a hidden far pointer to a caller-allocated temporary.
- **Strings:** ShortString = length byte + up to 255 chars. String literals live in the *code* segment right next to the code that uses them (that is why the story strings sit between code). Use Ghidra's `PascalString255`/`PascalString` data types. Value-string parameters are copied on entry by a System helper.
- **Typed constants:** initialised tables (ship stats, lookup tables) are typed constants in the data segment's initialised area. **Constants and tables are facts:** record values, not code.
- **Sets:** bit arrays up to 32 bytes; `in` tests compile to bit tests with shifts and masks.
- **Records and arrays:** packed byte alignment (BP7 default `{$A+}` is word alignment for some fields; check). Array bounds can start at 1 (Pascal), so index math may subtract 1 × element size.
- **Range/overflow checks:** `{$R+}`/`{$Q+}` produce calls to error routines after bound comparisons. Their presence tells you the declared bounds.
- **Real48:** if `{$N-}`, `Real` arithmetic goes through System helpers (add, mul, div, conversions). **Detect early (M1-01).** If gameplay uses Real48, we need a bit-exact Real48 type (risk R11).
- **Random:** `RandSeed` (a longint in DS) and an LCG step (the BP7 multiplier is believed to be `0x08088405` with increment 1). Confirm the multiplier, increment and **exactly how `Random(n)` scales the seed to the range** from the binary. `Randomize` seeds from the DOS clock (INT 21h AH=2Ch). The port's `BpRandom` must be bit-exact.
- **File I/O:** `Assign`/`Reset`/`BlockRead`/`Seek`/`Close` wrappers around INT 21h (3Dh open, 3Fh read, 42h seek, 3Eh close), working on `FileRec`. Filenames pass as ShortStrings. **String xrefs to filenames lead straight to load routines** (M1-05).
- **Crt unit:** `Delay` (calibrated busy loop), `KeyPressed`/`ReadKey` (INT 16h). The game probably installs its own INT 9 handler for flight controls.
- **Runtime errors:** the "Runtime error ... at ..." text is in the System unit. `RunError` and `Halt` paths are useful landmarks.

### 7.5 WORX Toolkit 2.1 field guide

- The banner string "WORX TOOLKIT VERSION 2.1 COPYRIGHT 1993 BY MYSTIC SOFTWARE" sits in the code segment of the WORX unit (all game exes and `td1.exe` link it).
- Expect: Sound Blaster detection (DSP reset on `2x6h`, read `2xAh`), DMA setup (ports `0Ah`/`0Bh`/`0Ch`/`02h`/`03h`/`83h`), DSP commands for 8-bit and **ADPCM** playback (`14h` PCM; `74h`/`75h` 4-bit, `76h`/`77h` 2.6-bit, `16h`/`17h` 2-bit ADPCM, with or without reference byte), OPL register writes (`388h`/`389h`), a PIT-driven music interrupt, and PC-speaker routines (port `61h`, PIT channel 2).
- Name the API surface `WORX_*` (for example `WORX_PlayVOC`, `WORX_PlayCMF`, `WORX_StopMusic`). Catalogue call sites with the argument (which file) to build music-cue and SFX tables (M1-02).
- The table of ascending bytes near the banner (`" %')+,-../0011..."`) looks like a lookup curve (volume or square-root). Identify it, but it is probably WORX-internal.
- **Tempo:** find how WORX derives the music tick from the CMF header (fields at 0x0A and 0x0C) or an API argument (U-007).

### 7.6 Hardware access patterns to search for

| What | Instruction patterns / bytes | Why it matters |
|---|---|---|
| Mode 13h | `mov ax,0013h; int 10h` (`B8 13 00 CD 10`) | Video init |
| Palette write | `mov dx,3C8h` (`BA C8 03`), `out dx,al`, then 3× `out` to `3C9h` | Palette loads, fades, IR effect |
| Palette read | `3C7h`/`3C9h` | Fades that read back the DAC |
| Vertical retrace | `mov dx,3DAh` (`BA DA 03`), `in al,dx`, `test al,8` | **Timing (U-001)**, page flips |
| PIT | `out 43h,al` (`E6 43`), `out 40h,al` (`E6 40`) | Timer rate, music tick, **game tick** |
| Interrupt vectors | INT 21h AH=25h/35h | Hooks for INT 8 (timer), INT 9 (keyboard) |
| Keyboard | `in al,60h` (`E4 60`), `out 20h,al` (EOI) | Scancode handler, key table layout |
| Joystick | `mov dx,201h` (`BA 01 02`) | Analog timing loop |
| SB DSP | `2x6h`, `2xAh`, `2xCh`, `2xEh` relative to a base variable | WORX |
| OPL | `mov dx,388h` (`BA 88 03`) | WORX music |
| PC speaker | `in al,61h`/`out 61h,al`, `out 42h,al` | PC-speaker SFX |
| DOS file | INT 21h AH=3Dh/3Fh/40h/42h/3Eh | Asset loading, saves |
| DOS time | INT 21h AH=2Ch | `Randomize`, timing |
| Program exec | INT 21h AH=4Bh | Launcher chain, `label.exe` path |

Output goes to `docs/re/hardware-access.md` as a table: address, routine, port or interrupt, purpose, confidence.

### 7.7 Finding things fast

- **By filename:** search for the ShortString (`hangar.scr`), take its xref, and you reach the loader call. Note buffer sizes and the destination.
- **By UI text:** in-game messages (Appendix B of the MDG) lead to the code that prints them. For example the objective strings lead to mission logic and the debrief tiers lead to the scoring thresholds.
- **By constant:** search scalar values for known numbers: 201 (map width), 1024 and 32 (tile size), 500 (path length), 20 (ship slots), 64000, 768, 3200/2400/2500/1500/1800 (armour and shield values), 0x08088405 (RNG).
- **By hardware:** §7.6.
- **By diffing episodes:** Ghidra Version Tracking (`file0001` → `td2` → `td3`) carries names across, and the unmatched functions are the episode-specific code. Use the *exact* correlators first, then the *similar* ones.
- **By dynamic tracing:** DOSBox debugger breakpoints on file open (INT 21h 3Dh) show *when* each asset loads, and memory dumps show live structure contents (§7.9).

### 7.8 Address notation

Every address in docs, CSVs and specs uses **image-relative segment:offset** plus the exe name:

```
file0001:0A3C:0012
```

- `0A3C` is the paragraph offset of the segment **relative to the start of the load image** (segment 0 = first paragraph after the MZ header).
- **Ghidra address** = `(image_base_seg + rel_seg):off`, with `image_base_seg` recorded per exe in EXECUTABLES.md (M0-08).
- **DOSBox runtime address** = `(PSP_seg + 0x10 + rel_seg):off`. Find PSP_seg in the debugger at program start (the initial `DS`/`ES` value, or the DOS MCB chain).
- **File offset** = `header_bytes + rel_seg*16 + off` (header_bytes: file0001 = 19136, td2 = 16768, td3 = 14544, file0000 = 1600, td1 = 544).

Always say which notation you use if it is not this one.

### 7.9 Dynamic analysis with DOSBox

- Use a debugger-enabled DOSBox (DOSBox-X has a built-in debugger in its standard builds; DOSBox Staging has heavy-debug builds). Run the **copy** in `re-work\dosbox\c\TD`.
- Typical commands (classic DOSBox debugger syntax; confirm in your build's help): `BP seg:off`, `BPINT 21 3D` (break on DOS open), `BPLIST`, `BPDEL n`, `D seg:off` (data view), `MEMDUMPBIN seg:off len` (dump memory to file), `SR reg val`, `LOG n` (trace n instructions), `F5` continue, `F10`/`F11` step.
- Recipes:
  - **Asset load order:** `BPINT 21 3D`, then log `DS:DX` (filename) at each hit while playing E1 up to mission 1.
  - **Structure discovery:** pause in flight, `MEMDUMPBIN` the DGROUP segment, move the ship, dump again, diff. The fields that change identify position, velocity and heading.
  - **Tick rate:** breakpoint on the main-loop head; count hits over 10 seconds of game time at different `cycles` settings (U-001).
  - **RNG:** watch `RandSeed` across ticks.
- Store dumps in `re-work\captures\ram\` with manifest entries (§11.2). They contain original program memory, so they never go in the repo.

### 7.10 Recording findings

Each finding goes into the right doc (FORMATS, structures, globals, hardware-access, a spec) using the template in §17.3, with:

- **Confidence:** `CONFIRMED` (decoded/observed directly, or two independent lines of evidence), `LIKELY` (strong single line of evidence), `PARTIAL`, `HYPOTHESIS`, `UNKNOWN`.
- **Evidence:** addresses, capture IDs, decode results.
- **Consequences:** which specs, tasks or unknowns it affects. Update those too.

## 8. Spec-writing playbook (Spec author)

### 8.1 Where specs live

`docs/spec/<area>/<topic>.md`. Areas: `assets`, `core`, `flow`, `render`, `audio`, `input`, `sim`, `narrative`, `frontend`, `missions`. Mission specs are named `missions/e1m01.md` etc. Start from [spec/_TEMPLATE.md](spec/_TEMPLATE.md).

### 8.2 Required sections

Summary · Fidelity tier · Definitions & units · Data (layouts/tables) · Behaviour (state machines, formulas, ordered steps) · Edge cases & original bugs · Test vectors (concrete input → expected output, from the original) · Open questions · Provenance · Revision history.

### 8.3 Rules

- Write for an Implementer who **cannot see the binary**. If they would need to ask "but what happens when...?", answer it or list it as an open question.
- Give **test vectors**: concrete values observed from the original (for example "holding thrust from rest in a Stiletto, speed per tick: 0, 3, 6, ... capped at N"). They become tests directly.
- Units on every number. Name the original's types (u8, i16, fixed 8.8 ...).
- Preserve order of operations and integer truncation where T1 depends on it, and say so explicitly ("divide *then* multiply; truncates toward zero").
- Bump the spec revision and add a history line on every behavioural change. Code cites `spec path + revision`.

### 8.4 Review checklist for specs

- [ ] Implementable without reading the binary.
- [ ] Every constant has a provenance address.
- [ ] Test vectors present for every non-trivial formula.
- [ ] No decompiler-shaped pseudocode.
- [ ] Confidence stated; open questions listed and mirrored in UNKNOWNS where cross-cutting.

## 9. Implementation playbook (Implementer)

### 9.1 Code organisation

- Follow the crate responsibilities in MDG §8.2 exactly. If a feature seems to need an upward dependency, introduce a trait or an event in the lower crate instead.
- Module header doc comments name the spec they implement:
  ```rust
  //! Player flight model.
  //! Spec: docs/spec/sim/flight-model.md (rev 3). Fidelity: T1.
  ```
- Constants from the original carry provenance:
  ```rust
  /// Stiletto top speed in internal units/tick.
  /// Spec: sim/ship-classes.md §2.4; original: file0001:0A3C:0120.
  pub const STILETTO_MAX_SPEED: i16 = /* value from spec */;
  ```

### 9.2 Rust conventions

- Edition 2024, stable toolchain pinned. `rustfmt` default style. Clippy clean with `-D warnings`.
- Libraries: `thiserror` errors, no `unwrap`/`expect` outside tests except on proven invariants (with a comment saying why it cannot fail). Binaries: `anyhow` with `.context()`.
- Public items documented. `#[must_use]` on pure constructors and results that matter.
- Prefer plain data plus free functions over trait hierarchies. No `dyn` in hot sim paths without reason.
- No global mutable state. No `lazy_static`/`OnceLock` for game state (fine for immutable tables).
- Logging with `tracing` (`debug!` for per-tick detail behind targets; never `println!` in libraries).

### 9.3 Numerics and 16-bit semantics

- Mirror the original's integer widths where behaviour depends on them. Use `i16`/`u16`/`u8` and **explicit** `wrapping_*`, `saturating_*` or `checked_*` matching the original's actual behaviour (which the spec states).
- `overflow-checks = true` for `td-sim` in all profiles, so accidental overflow panics in tests instead of silently diverging.
- Division: Rust `/` truncates toward zero, as x86 `IDIV` does. Arithmetic shifts (`>>` on signed) match `SAR`. Note where the original used `SHR` on signed values (logical shift) and use `as u16 >> n` explicitly.
- Fixed point: newtypes (`Fx8_8(i16)`, etc.) with only the operations the spec defines.
- Trig: if the original uses sine/cosine tables, read or replicate them **exactly** (the table values are facts in the spec or extracted from the exe). Never substitute `f32::sin`.
- Real48 (if needed): a dedicated, bit-exact type in `td-sim::real48` with tests from captured values.

### 9.4 Determinism rules (td-sim)

See MDG §8.6. In addition:
- Iterate entities in slot order, as the original does. Removing entities must not reorder survivors unless the original does.
- All randomness goes through `BpRandom` in call order identical to the original (the spec lists every call site that consumes randomness). An extra or missing call desynchronises everything after it.
- `TickInput` captures *everything* the original reads during a tick (keys as the original's key-state table, joystick values).

### 9.5 Rendering rules

- Draw into the indexed `Framebuffer` only. No RGBA in `td-render`.
- Transparency, clipping and draw order follow the specs exactly. Clipping at screen edges matches the original (it might skip partially offscreen sprites; check the spec).
- Rotations of sprites are exact pixel transforms (transpose, flip). Never resample.

### 9.6 Audio rules

- `td-audio` never touches devices or the sim. It consumes `AudioCommand`s and produces frames.
- Decoding is deterministic and unit-tested. Resampling quality matters for T2: test with a sweep and a known tone.
- `td-opl-sys` stays minimal: register write, sample generation, reset. Everything else is safe Rust.

### 9.7 Platform rules

- SDL3 types never leak out of `td-platform`.
- Window, input and audio code is thin. Logic lives in testable crates.

### 9.8 Performance

Correctness first. The whole game is tiny by modern standards: a full-screen indexed redraw every tick is fine. Profile before optimising, and never trade determinism for speed.

### 9.9 Dependencies

- Add a crate only when it removes real work. Check its licence (MDG §5.4), maintenance, and transitive weight. Record it in THIRD_PARTY.md.
- Never vendor GPL code. ymfm (BSD-3) is the planned vendored C++ component.

## 10. Testing playbook

### 10.1 Test levels

Test levels L0–L6 are defined in MDG §10.2. In practice:

| You changed... | Add or update... |
|---|---|
| A parser | L0 synthetic tests (including malformed input) + L1 decode test over every real file of that type |
| A render routine | L0 on a tiny synthetic framebuffer + L2 hash of a real scene |
| Sim behaviour | L3 scenario test with test vectors from the spec; update replay baselines (L4) only with a recorded reason |
| A mission script | L4 success replay + scenario tests for its triggers |
| Audio decoding | L0 synthetic vectors + L1 hash of decoded PCM |

### 10.2 Data-gated tests

```rust
#[test]
fn decodes_all_full_screens() {
    let data = td_test_support::require_data!(); // prints SKIPPED and returns if TD2192_DATA unset
    for name in td_assets::manifest::files_of_category(Category::Screen) {
        let bytes = std::fs::read(data.join(name)).unwrap();
        let screen = td_formats::screen::parse(&bytes).unwrap();
        td_test_support::assert_sha256!(screen.pixels_and_palette_bytes(), golden::SCREEN[name]);
    }
}
```

### 10.3 Golden values

- Goldens are **SHA-256 hex strings or small numeric facts** in Rust source (`golden.rs`). Never PNG or WAV files.
- **Producing a golden is a verification act.** Before you commit a hash, inspect the output it represents (render to scratch PNG and look at it, or listen to it) and compare it to a reference capture where one exists. Say in the commit message how you verified it.
- Never "update goldens to make tests pass" without understanding why they changed. Write the reason in the commit message.

### 10.4 Snapshot tests

`insta` snapshots are for *our* textual outputs (for example `tdtool dump-mission` formatting for a synthetic file). Snapshots of real game text are not allowed, because they would commit content.

### 10.5 Replays

- A replay file (`tests/replays/*.tdr`) is *our* data: header (episode, mission, seed, version), the per-tick `TickInput` stream, and the expected `state_hash` every N ticks plus the final outcome.
- Record them with `td2192 --record`. Verify with `tdtool replay --verify`.
- A replay that diverges after an intended behaviour change is re-recorded with the reason in the commit message, after a Verifier confirms that the new behaviour is the more faithful one.

### 10.6 Fuzzing and property tests

- Every parser gets a `proptest` "never panics on arbitrary bytes" test and a `cargo-fuzz` target (run on Linux CI).
- Sim invariants: entities stay within map bounds, health never goes negative, `state_hash` is stable across clone and serialise round-trips.

## 11. Verification against the original (Verifier)

### 11.1 DOSBox reference setup

- `re-work\dosbox\c\TD` is a **copy** of the original install. The game writes `td.cfg`, saves and `tdstatus.dat` into its directory.
- Configure sound as SB16 (or SB Pro 2 for authenticity: compare both once), OPL emulation, and a fixed `cycles` value recorded in the capture manifest. Speed-sensitive behaviour (U-001) must be captured at a documented cycles setting.
- Keep separate DOSBox configs per purpose (`ref-play.conf`, `ref-debug.conf`) under `re-work\dosbox\`.

### 11.2 Captures

- Every capture gets an ID and a manifest row in `re-work\captures\MANIFEST.md`: `id, date, exe, episode/mission/scene, method (screenshot/video/audio/ram), dosbox build + cycles, description, file path`.
- Screenshots: DOSBox raw-mode capture at 320×200 (no scaling or filtering) so pixel comparison is exact.
- Audio: DOSBox WAV capture.
- Video: DOSBox capture (ZMBV AVI) for timing analysis. Count frames for tick timing.

### 11.3 Comparison methods

| Kind | Method | Pass criterion |
|---|---|---|
| Static screen | `tdtool render-scene` vs capture with `tdtool diff-frames` | 0 differing pixels (T0) |
| Animation / timing | Frame-indexed comparison of capture video vs port recording | Same frame of each event ±0 ticks (T1) or ±1 (T2, justified) |
| Movement | Ship coordinates from RAM dumps (DOSBox) vs `state` dumps (port) for the same input script | Identical per tick (T1) |
| Audio | Duration, onset timing, spectrogram comparison + listening | T2: no audible difference in blind A/B |
| Outcomes | Mission success/failure and kill counts for the same inputs | Identical |

### 11.4 Parity reports

Write `docs/parity/<milestone>-<area>.md` using the template (§17.6). Each row is a behaviour, its method, its capture IDs, its result and notes. These are milestone exit evidence.

### 11.5 Bug reports

A deviation found by a Verifier becomes a bug entry (template §17.5) in STATUS.md under the milestone's "Bugs" list, with an IM task to fix it.

## 12. Documentation duties

| When you... | Update |
|---|---|
| Learn anything about a file format | `docs/re/FORMATS.md` (+ FILE_INVENTORY confidence column) |
| Name a function or global | `docs/re/symbols/<exe>.csv` |
| Map exe-embedded text or a table | `docs/re/exe-resources/<exe>.toml` |
| Resolve or discover an unknown | `docs/re/UNKNOWNS.md` |
| Define behaviour | `docs/spec/...` |
| Make a design decision | An ADR (`docs/adr/NNNN-*.md`, status Proposed) and ask the user to accept it |
| Change the architecture or a cross-cutting rule | MDG (with ADR) |
| Change a process | This guide (and AGENTS.md if it is an always-on rule) |
| Start, pause or finish a task | `docs/STATUS.md` |
| Stop mid-task | `docs/handoffs/` |
| Run a parity check | `docs/parity/` |

Style: plain Markdown, sentence-case headings, tables for structured facts, Mermaid for diagrams, relative links. Dates are absolute (YYYY-MM-DD). No game content beyond minimal anchors.

## 13. Git, branches, commits and reviews

### 13.1 Branching

- `main` is always green.
- Task branches: `<milestone>/<task-id>-<slug>`, for example `m2/M2-05-rotation-sets`.
- Parallel agents use **git worktrees** (§14.2).
- Commit and push only when the user has asked for it or it is part of an agreed workflow. Never force-push `main`.

### 13.2 Commit messages

```
<area>: <imperative summary> (<task-id>)

<what and why; how it was verified; spec revision; unknowns touched>

Co-Authored-By: <per the session's attribution instructions>
```

Areas: `formats`, `assets`, `sim`, `render`, `audio`, `platform`, `flow`, `tool`, `re`, `spec`, `docs`, `ci`, `build`.

### 13.3 Pull request checklist (copy into each PR)

- [ ] Task ID(s) and hat(s) stated
- [ ] Spec(s) implemented, with revision
- [ ] Tests added/updated (levels: L0/L1/...); data-gated tests skip cleanly
- [ ] fmt/clippy/test green with and without `TD2192_DATA`
- [ ] No original data, derived assets, or decompiler output in the diff
- [ ] Docs updated (FORMATS / UNKNOWNS / symbols / spec / STATUS)
- [ ] Clean-room attestation: "Implementer did not consult disassembly or decompiler output for this change"
- [ ] Verification evidence (how outputs were checked; capture IDs)

### 13.4 Review focus

Reviewers check, in this order: clean-room and data hygiene → spec conformance (constants, order of operations) → determinism → crate boundaries → tests actually covering the behaviour → error handling → readability.

## 14. Multi-agent coordination

### 14.1 Claiming

STATUS.md is the single claim board (§6.1). One active task per agent. Claims older than 3 days with no commits can be reclaimed after leaving a note.

### 14.2 Worktrees

```powershell
git -C P:\msdos\TD2192\TD2192-Modern worktree add ..\TD2192-Modern-wt\M2-05 -b m2/M2-05-rotation-sets
```

Each parallel agent works in its own worktree and branch. Rebase on `main` before merging. In Claude Code, subagents can be started with worktree isolation for the same effect.

### 14.3 File ownership and conflict avoidance

- A task brief declares the modules or files it owns. Two concurrent tasks must not own the same module.
- Shared docs (STATUS, FORMATS, UNKNOWNS, symbols CSVs) take small, focused edits. Keep CSVs sorted by address so that concurrent additions merge cleanly. Rebase often.
- New spec files and new mission modules are one file per unit, by design, to minimise conflicts.

### 14.4 The Ghidra project (single writer)

- The canonical project `re-work\ghidra\TD2192.gpr` has **one writer at a time**. Claim it in STATUS.md (`Ghidra lock:` line at the top) before opening it in the GUI or running non-read-only headless scripts. Release it when done.
- Other RE agents work **read-only** (`-readOnly` headless) or on a **private copy** of the project (`re-work\ghidra-<agent>\`). Their naming results flow through `docs/re/symbols/*.csv`, which the lock holder applies with `ApplySymbols`.
- After merging symbol CSV changes, the lock holder re-applies them to the canonical project.

### 14.5 Parallel lanes

| Phase | Lanes that can run concurrently |
|---|---|
| M0 | (a) workspace/CI/assets (IM) · (b) Ghidra setup + scripts (RE) · (c) DOSBox + capture tooling (VE) |
| M1/M2/M3 | (a) RE M1-01..M1-06 · (b) IM M2-01..M2-05 (formats already understood) · (c) IM M3-01..M3-04 (public formats) · (d) RE M1-05 → RE/IM M2-06..M2-12 |
| M4 | (a) platform window/present · (b) input · (c) audio device · (d) map viewer |
| M5–M8 | **Narrative lane:** M5 → M11 · **Gameplay lane:** M6 → M7 → M8 · **RE support:** specs ahead of both |
| M9/M10 | Mission tasks are independent units: many agents, one mission each (RE+SP session, then IM session, then VE) |

### 14.6 Handoffs

Use `docs/handoffs/<date>-<task-id>.md` (§17.2) whenever work stops before Done. The next agent starts by reading it. Delete it when the task is DONE (git history keeps it).

### 14.7 Coordinator duties

- Keep STATUS.md accurate: dependencies, claims, blockers (including user decisions D1–D10).
- Write task briefs (§17.1) for subagents: hat, goal, inputs (spec paths), owned files, done criteria, things not to touch.
- Watch for drift: duplicated helpers, conventions diverging, specs and code disagreeing. Schedule clean-ups.

## 15. Per-milestone agent playbooks

Use each playbook with the milestone's task table in MDG §12.

### M0: Foundation

- **Hats:** IM (workspace, CI, assets, tdtool), RE (Ghidra), VE (DOSBox, captures).
- **First moves:** M0-01 workspace stubs → M0-04/M0-05 locator and manifest (the FILE_INVENTORY table is the input) → M0-06 test support. In parallel: M0-07/08 Ghidra headless import of all five exes; M0-10 DOSBox.
- **Pitfalls:** SDL3 crate build features on Windows (use bundled or build-from-source with CMake); Ghidra needs `JAVA_HOME`; the Ghidra project lock; never point DOSBox at the pristine folder.
- **Done check:** `tdtool inventory` = 218/218, CI green on all three OSes, Ghidra programs analysed, DOSBox runs the copy to mission 1.

### M1: Executable archaeology

- **Hats:** RE (all tasks), SP for the spec outputs.
- **First moves:** set DS across each program → identify the System unit (look for the "Runtime error" text and `StackCheck`-style prologue calls) → unit init chain from the main block → WORX banner xrefs → filename-string xrefs (M1-05) → retrace and PIT sites (M1-06).
- **Order:** M1-01 → M1-03 → M1-06 (timing first, it blocks M4/M6) → M1-05 → M1-04 → M1-07/08 → M1-02 → M1-11 → M1-09 → M1-10 → M1-12.
- **Pitfalls:** reversed Pascal parameter order in Ghidra; nested procedure static links; strings in code segments; mistaking WORX internals for game logic; forgetting to record the image base.
- **Done check:** exit criteria in MDG §12 M1.

### M2: Static assets

- **Hats:** IM for formats already understood (screen, palette, tileset, map, sprite sets); RE then IM (separate sessions) for `.pic`, portraits, fonts, UI raw, dialogue, mission, saves.
- **First moves:** M2-01/02/04 (trivial, confirmed formats) to establish parser and test conventions everyone copies.
- **Pitfalls:** 201 vs 200 map width; transparency index; the E2/E3 blk trailing 864 bytes; dimensions that exist only in code (wait for M1-05); committing any PNG (don't).
- **Done check:** every inventory file is parsed or marked not needed; FORMATS updated to CONFIRMED.

### M3: Audio

- **Hats:** IM (VOC, ADPCM, CMF, OPL wrapper, mixer), RE (M3-05 tempo, SFX policy), VE (M3-08).
- **First moves:** VOC parser + PCM path with `afterb.voc`-style files; ADPCM from public descriptions; vendor ymfm; CMF sequencer with synthetic tests.
- **Pitfalls:** ADPCM reference byte handling; step-size tables (write them from the public spec and test them); CMF timing fields (U-007); OPL rate vs device rate; clipping in the mixer.
- **Done check:** all 34 audio files play; parity report.

### M4: Platform shell

- **Hats:** IM; VE for frame pacing and audio under-run checks.
- **Pitfalls:** `Alt` vs fullscreen hotkeys; DPI scaling; the vsync/fixed-tick interaction (no interpolation in Classic); keyboard layout independence (scancodes); SDL types leaking out of `td-platform`.

### M5: Narrative and screens

- **Hats:** RE/SP for per-screen layout and timing; IM; VE for pixel-exact comparisons.
- **Pitfalls:** text from exe resources (never hardcode it); the `"` apostrophe glyph; palette changes between scenes (each `.scr` has its own palette); fade step counts.

### M6–M8: Gameplay lane

- **Hats:** RE/SP ahead of IM at all times (specs one task ahead); VE with RAM-dump comparisons.
- **Pitfalls:** the original's update order (input → player → AI → projectiles → collisions → effects?), which must be spec'd exactly; RNG call parity; 16-bit wraparound; slot reuse order; collision tunnelling at afterburner speed (replicate the original's behaviour, don't "fix" it).
- **Done check:** replays of scripted scenarios match RAM-dump timelines from the original.

### M9–M10: Missions

- **Unit of work:** one mission. Per mission: an RE+SP session writes `spec/missions/eNmNN.md` (objectives, spawns, triggers, messages by exe-resource ID, guidance rules, success/failure, quirks, test vectors and a reference capture ID); a fresh IM session writes the script and scenario tests; a VE session records the success replay and parity notes.
- **Pitfalls:** mission logic shared through helper routines (put it in the runtime API, not copy-pasted across scripts); text by resource ID only; comm and transmission timing; missions with ship switching or special objects.

### M11–M13

- Standard IM work with VE acceptance. Release tasks (signing, publishing) need explicit user approval at each outward-facing step.

## 16. Pitfall catalogue

| # | Pitfall | Correct approach |
|---|---|---|
| 1 | Writing to the original directory | Read-only, always. DOSBox uses a copy. |
| 2 | Committing PNG/WAV/text extracted from the game | Hashes only. Exports go to `re-work` or scratch. |
| 3 | Map treated as 200×200 | Episode maps are **201×201**; `intro.map` is 200×200. |
| 4 | 6-bit palette used as 8-bit | Convert with `(v<<2)|(v>>4)`; keep 6-bit internally for effects. |
| 5 | Filename lookups fail (`helihangar.scr`) | 8.3 truncation + case-insensitive resolution. |
| 6 | Pascal strings read as C strings | Length byte prefix; no terminator. |
| 7 | Ghidra parameter order reversed | Pascal pushes left-to-right; fix signatures. |
| 8 | Unresolved `DS:` references in Ghidra | Set the DS register value across the program first. |
| 9 | Guessing a constant | Constants come from specs with provenance; otherwise `ASSUMPTION(U-xxx)`. |
| 10 | Using `f32` trig in the sim | Use the original's tables or fixed-point exactly. |
| 11 | `HashMap` iteration in the sim | `Vec`/arena in slot order. |
| 12 | Extra RNG call | Every RNG call site is in the spec; match the order exactly. |
| 13 | Signed overflow semantics | Explicit `wrapping_*` where the original wraps; overflow checks on. |
| 14 | Fullscreen on `Alt+Enter` | `Alt` is infra-red in the game; use `F11`. |
| 15 | Hardcoding game text | Exe-resource IDs or data files at runtime. |
| 16 | ADPCM VOC treated as PCM | Check the codec byte: 0 PCM, 1 4-bit, 2 2.6-bit, 3 2-bit. |
| 17 | CMF tempo from the wrong field | Resolve U-007 before accepting music timing. |
| 18 | Resampling sprite rotations | Exact transpose/flip of the four base angles. |
| 19 | Interpolating sim state for rendering in Classic | Present the last completed tick only. |
| 20 | "Fixing" an original quirk silently | Record it in `original-bugs.md` with a policy decision. |
| 21 | Golden hash updated without inspection | Look at or listen to the output, compare to a capture, explain in the commit. |
| 22 | Two agents editing the same module | Declare ownership in briefs; worktrees; small shared-doc edits. |
| 23 | Ghidra project opened by two writers | Single-writer lock; CSV is the source of truth. |
| 24 | RE and IM on the same subsystem in one session | Separate sessions; spec is the only bridge. |
| 25 | Reproducing `label.exe` behaviour | Never (NG4). |
| 26 | Lowercase-only text rendered with a modern font | Use the original bitmap fonts; respect their character set. |
| 27 | Episode 3 mission numbering assumed 1..20 | See U-012; files are m1..m19 + m21. |
| 28 | Treating `td.sc2/sc3/shr` as unrelated sprites | They are rotation sets of the same 20 sprites in 40×40 cells. |
| 29 | Speed-sensitive DOSBox captures without recorded `cycles` | Always record DOSBox build and cycles in the manifest. |
| 30 | Forgetting `tdstatus.dat` is runtime-generated | It is not in the install; don't treat it as missing data. |

## 17. Templates

### 17.1 Task brief (for spawning an agent)

```markdown
**Task:** M2-05 Rotation sets
**Hat:** Implementer (do NOT open re-work/exports, Ghidra, or disassembly)
**Goal:** Parse td.sc3/td.shr/td.sc2 40×40 rotation sets; build 16-heading sprite lookup.
**Inputs:** docs/re/FORMATS.md §Sprites; docs/spec/render/sprites.md (rev 1); MDG §9.3.5
**Owns:** crates/td-formats/src/rotation_set.rs, crates/td-render/src/sprites.rs (+ tests)
**Do not touch:** td-sim, td-platform
**Done when:** L0+L1 tests; tdtool sheet shows all 16 headings for each ship (inspect in scratch);
FORMATS confidence updated; STATUS updated; DoD checklist (ADG §6.7).
**Report back:** summary, verification evidence, new unknowns.
```

### 17.2 Handoff note

```markdown
# Handoff: <task-id> <title> (<YYYY-MM-DD>)
- Hat: ...
- State: what is done / what is not
- Branch/worktree: ...
- Next steps (ordered): 1. ... 2. ...
- Gotchas discovered: ...
- Open questions (with U-IDs): ...
- Verification status: ...
```

### 17.3 RE finding

```markdown
### <Finding title>
- **Confidence:** CONFIRMED | LIKELY | PARTIAL | HYPOTHESIS
- **Where:** file0001:0A3C:0012 (routine `Sim_UpdatePlayerShip`)
- **Evidence:** static (xref from "hangar.scr"), dynamic (capture C-0042 RAM dump), decode (tdtool sheet)
- **Finding:** <facts: layout, constants, behaviour in our words>
- **Consequences:** affects spec/sim/flight-model.md §3; resolves U-002; new task M6-03.a
```

### 17.4 Spec skeleton

See [spec/_TEMPLATE.md](spec/_TEMPLATE.md).

### 17.5 Bug report

```markdown
### BUG-<n>: <summary>
- Found by / date / milestone:
- Profile: Classic | Enhanced
- Steps (or replay file):
- Expected (original, with capture ID):
- Actual (port):
- Spec reference:
- Severity: blocker | major | minor | cosmetic
```

### 17.6 Parity report

```markdown
# Parity: <milestone> <area> (<date>)
| # | Behaviour | Tier | Method | Captures | Result | Notes |
|---|---|---|---|---|---|---|
| 1 | Stiletto top speed reached at tick N | T1 | RAM dump vs state dump | C-0101 | PASS | |
```

### 17.7 ADR

```markdown
# ADR-NNNN: <title>
- Status: Proposed | Accepted | Superseded by ADR-XXXX
- Date:
## Context
## Decision
## Consequences
## Alternatives considered
```

## 18. Quick reference

| Item | Value |
|---|---|
| Repo | `P:\msdos\TD2192\TD2192-Modern` |
| Original data (read-only) | `P:\msdos\TD2192\TD2192` |
| RE work area | `P:\msdos\TD2192\re-work` |
| Ghidra | `C:\Users\jason\Desktop\ghidra_12.2_PUBLIC` (needs `JAVA_HOME`) |
| JDK | `C:\Program Files\Eclipse Adoptium\jdk-25.0.5.8-hotspot` |
| Ghidra language | `x86:LE:16:Real Mode` / loader `MzLoader` |
| MZ header sizes | td1 544 · file0000 1600 · file0001 19136 · td2 16768 · td3 14544 bytes |
| Screen | 320×200, 8bpp, 6-bit palette, pixel aspect 5:6 (display 4:3) |
| Map | 201×201 tiles × 32×32 px (intro 200×200) |
| Sprites | 32×32 base ×20; rotation sets 40×40 ×24 (sc3 ≈22.5°, shr ≈45°, sc2 ≈67.5°) |
| Mission file | 20080 bytes = 80 header + 20×500 X + 20×500 Y (hypothesis) |
| Address notation | `<exe>:<relseg>:<off>` (§7.8) |
| Confidence levels | CONFIRMED · LIKELY · PARTIAL · HYPOTHESIS · UNKNOWN |
| Fidelity tiers | T0 bit-exact · T1 behaviour-exact · T2 perceptual · T3 intentional deviation |
