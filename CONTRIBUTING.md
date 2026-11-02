# Contributing

Read [AGENTS.md](AGENTS.md) first. It is short and applies to humans too. The full process is in [docs/AGENT_DEVELOPMENT_GUIDE.md](docs/AGENT_DEVELOPMENT_GUIDE.md), and the design is in [docs/MASTER_DESIGN_GUIDE.md](docs/MASTER_DESIGN_GUIDE.md).

The essentials:

1. Claim a task in [docs/STATUS.md](docs/STATUS.md).
2. Original-game behaviour is implemented from specs in `docs/spec/`, never from disassembly (clean-room protocol, ADR-0002).
3. Never commit original game data or anything derived from it (ADR-0003).
4. Before handing off: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`, with and without `TD2192_DATA` set.
5. Update the docs your change affects (formats, unknowns, specs, status).

By contributing you agree that your contributions are licensed under the [MIT License](LICENSE).
