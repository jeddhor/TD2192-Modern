@AGENTS.md

## Claude Code specifics

- Use subagents to keep hats separate. An Implementer subagent's brief lists only spec and doc paths and says explicitly not to open Ghidra, `re-work\exports` or disassembly. An Archaeologist subagent may use them. Use the task brief template in ADG §17.1.
- Run parallel implementation subagents with worktree isolation. One task per subagent; declare owned files in the brief.
- Only one session may hold the Ghidra project for writing (the `Ghidra lock` line in docs/STATUS.md). Others use `-readOnly` headless runs or a private project copy.
- Throwaway analysis scripts go in the session scratchpad or `P:\msdos\TD2192\re-work\scratch`, never in the repo.
- Commit only when the user asks. End commit messages with the attribution trailer given in the session instructions.
