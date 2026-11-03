# Ghidra scripts

Our own Java GhidraScripts (MIT) for the original executables. They run headless and in the GUI (Script Manager → add this directory to the script paths; category `TD2192`). Background: [ADG §7.2–7.3](../../docs/AGENT_DEVELOPMENT_GUIDE.md#72-ghidra-project-setup-m0-08), [EXECUTABLES.md §1.1](../../docs/re/EXECUTABLES.md#11-ghidra-image-base-segments-and-dgroup-m0-08-2026-11-02).

All addresses that the scripts read or write are **image-relative** `SSSS:OOOO` (ADG §7.8). Ghidra address = `(0x1000 + SSSS):OOOO` for all five exes. The scripts convert through the containing memory block, so Ghidra's 64 KB-normalised display addresses (`2000:EF75`) never leak into the output.

## Running

Use the wrapper. It defaults to read-only, no re-analysis, the canonical project and the right JDK:

```powershell
tools/scripts/ghidra-headless.ps1 -Program file0001.exe -Script ExportFunctionIndex.java
tools/scripts/ghidra-headless.ps1 -Program all -Script SetupBp7Program.java -Write      # needs the Ghidra lock
tools/scripts/ghidra-headless.ps1 -Program file0001.exe -Script ExportDecompiled.java -ScriptArgs 'filter:3A9A:'
```

Options: `-Write` (save changes; hold the lock in `docs/STATUS.md` first), `-Analyze` (run auto-analysis after the script), `-Raw` (full console), `-GhidraHome`, `-JavaHome`, `-ProjectDir`, `-ProjectName`, `-LogDir` (default `re-work\logs`).

**Script arguments use `key:value`.** `analyzeHeadless.bat` is a cmd script: it splits arguments at `=` and swallows `^`. The scripts also accept `key=value`, which is fine from the GUI.

## Scripts

| Script | Writes to the program? | Output | Arguments |
|---|---|---|---|
| `SetupBp7Program.java` | **yes** | console report | `dgroup:<rel hex>`, `noanalyze`, `passes:<n>` |
| `ExportFunctionIndex.java` | no | `re-work\exports\<exe>-functions.csv` | `[out.csv]` |
| `ExportPascalStrings.java` | only with `create` | `re-work\exports\<exe>-strings.csv` (**game text: never commit**) | `[out.csv] [min:<n>] [create]` |
| `ApplySymbols.java` | **yes** | console report | `[symbols.csv]` (default `docs/re/symbols/<exe>.csv`) |
| `ExportSymbols.java` | no (writes the CSV) | `docs/re/symbols/<exe>.csv` | `[symbols.csv] [task:<id>] [confidence:<c>]` |
| `ExportDecompiled.java` | no | `re-work\exports\<exe>\SSSS_OOOO_<name>.c` (**never commit**) | `[outdir] [filter:<regex>] [timeout:<s>]` |
| `FindPortIO.java` | no | `re-work\exports\<exe>-portio.csv` | `[out.csv]` |

The default outputs go to `<project parent>\exports\`, which is `P:\msdos\TD2192\re-work\exports`. `ExportPascalStrings` and `ExportDecompiled` **refuse to write inside any git work tree**.

### SetupBp7Program

This is the one-time (and repeatable) preparation of a BP7 program imported with the MZ loader. Run it after every import, and again after manual disassembly in a segment that straddles a 64 KB line. It:

1. re-creates the last 512 bytes of the image when the MZ header has `e_cblp = 0` (Ghidra drops them; only `file0001.exe`);
2. finds DGROUP from System's start-up code (`mov reg,imm; mov ds,reg`), names the blocks `DGROUP`/`DGROUP_BSS` and clears their execute flag;
3. sets **DS = DGROUP** over every code block;
4. turns off the "Non-Returning Functions – Discovered" analyzer and clears false no-return flags (functions that contain a `RET`), re-disassembling the dropped fall-throughs;
5. **re-targets near `CALL`/`JMP rel16` instructions that Ghidra resolves 64 KB off.** Ghidra's x86 SLEIGH wraps `rel16` at a linear 64 KB boundary rather than at the segment base. The script replaces each wrong reference with an overriding reference, then removes the bogus functions and the "code" decoded from text at the wrong targets;
6. re-points `CS:[disp]` data references to the instruction's own segment. Ghidra's real-mode SLEIGH derives CS as `(linear >> 4) & 0xF000`. The decompiler still shows the raw constant, so read `CS:[x]` as own-segment:`x`;
7. marks real exits (INT 21h with AH=4Ch, such as System `Halt`) as no-return, and clears the unreferenced fall-through after calls to them;
8. seeds code from the entry point, every far-call relocation target, the prologues `55 89 E5` / `55 8B EC` / `C8 nn nn 00`, code pointers (an immediate next to `PUSH CS` or next to a relocated code-segment word, unless it points at a Pascal string), and every undefined call or jump target;
9. re-runs auto-analysis and repeats until nothing changes, then prints coverage (instructions, Pascal-string bytes, zero fill, other).

### ExportFunctionIndex

Columns: `address, ghidra_address, name, segment, size, ret, prologue, has_stack_check, callers, callees`. `ret` is `near`/`far`/`iret` plus the bytes popped (Pascal callee-pop). `prologue` is `bp89` (Pascal `push bp; mov bp,sp`), `enter` (`{$G+}`), `bp8b` (hand-written assembler) or `none`. The stack-check routine is detected as the most common `mov ax,n; call far X` target at procedure entry.

### ExportPascalStrings

Finds length-prefixed printable strings in the code segments and DGROUP. Cross-references are Ghidra references plus "immediate equal to the string offset with `PUSH CS` (or `PUSH DS`) within three instructions", which is BP7's way of passing a literal. With `create` (and `-Write`) it defines `PascalString255` data at referenced strings in undefined code bytes.

### ApplySymbols / ExportSymbols

`docs/re/symbols/<exe>.csv` is the source of truth (format: [docs/re/symbols/README.md](../../docs/re/symbols/README.md)). `ApplySymbols` creates or renames functions, labels and typed data. Data types come from the `signature` column: `u8 i8 u16 i16 u32 i32 ptr16 farptr real48 pstring`, `T[n]`, `string[n]`. Comments become plate/pre comments tagged `[confidence, task]`. `ExportSymbols` writes user-defined Ghidra names back: existing rows keep every field except the name, new rows get an empty comment, and the output is sorted by address. It never exports Ghidra comments, so decompiler text cannot leak into the repo.

### ExportDecompiled

One C file per function, named by image-relative address. The output is derived from the original program: it is for Archaeologists only, and must never be committed, pasted or transliterated (AGENTS.md rules 3–4).

### FindPortIO

Lists every `IN`/`OUT`/`INS`/`OUTS`/`INT`. It resolves the port from an immediate or by tracing `DX` back through `mov/inc/dec/add/sub dx` (a variable base is shown as `DX=[mem]+n`), the `AH` value for `INT` and the `AL` value for `OUT`. The trace stops at jump targets and calls, so `DX(?)` means "set elsewhere". Input to [docs/re/hardware-access.md](../../docs/re/hardware-access.md).

## Tested

2026-11-02, Ghidra 12.2 + Temurin 25, headless via the wrapper. `SetupBp7Program` was run from a clean import on all five programs, and a second run changed nothing. The other scripts were run against `file0001.exe`; the symbols round trip used a scratch CSV, then the real `docs/re/symbols/file0001.csv`.
