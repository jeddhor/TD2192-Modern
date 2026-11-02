# ADR-0001: Rust + SDL3 as the implementation stack

- **Status:** Accepted (user decision D1, 2026-11-02)
- **Date:** 2026-11-02

## Context

We are reimplementing a 320×200, 8-bit, single-player DOS game. Most of the work is careful binary-format parsing, deterministic integer simulation that mirrors 16-bit Pascal semantics, a small software renderer, audio decoding and synthesis, and thin platform glue. Most of the code will be written by AI agents working in parallel, so compile-time guarantees, enforceable module boundaries and built-in test tooling matter more than usual.

Available on the development machine: Rust (rustup, MSVC host), Visual Studio 2026, clang 22, CMake 3.31.

## Decision

- **Language:** Rust, stable, edition 2024, toolchain pinned in `rust-toolchain.toml`.
- **Platform:** SDL3 through the `sdl3` crate (window, presentation, input, gamepad, audio stream, dialogs).
- **Structure:** a Cargo workspace with the crates in MDG §8.2; dependency direction enforced by crate graph; `unsafe` forbidden everywhere (a justified, reviewed exception is possible only in `td-platform`).

## Consequences

- Memory safety and explicit integer-width semantics (`wrapping_*`, overflow checks) help both stability and T1 fidelity.
- Built-in `cargo test`, `clippy` and `fmt` give every agent the same quality gates.
- One C++ component (ymfm, ADR-0005) is compiled by the `ymfm-sys` crate through `cxx`, which needs a C++ compiler (MSVC on Windows).
- SDL3 bindings may need the crate's bundled or build-from-source feature on Windows (CMake is installed).

## Alternatives considered

- **C++20 + SDL3 + CMake:** the most conventional choice for source ports, with native ymfm. It was rejected as the default because of weaker guardrails for agent-written code (undefined behaviour on signed overflow, which matters for emulating 16-bit math, plus manual module discipline and heavier build configuration). It remains a valid choice if the user prefers it. Only this ADR, MDG §7.1/§8.2 and ADG §9 would change.
- **Rust + Bevy:** the user considered it. It was not chosen for these reasons. Bevy's ECS schedule and its float-based transforms and time work against a bit-deterministic, slot-ordered 16-bit simulation. Its GPU sprite pipeline would have to be bypassed anyway to keep the indexed 320×200 framebuffer (ADR-0004) with exact palette effects. It brings a large dependency tree and long compile times for agents, and frequent breaking releases. Everything we need from an engine (window, texture upload, input, audio stream) is a thin SDL3 layer. If an Enhanced feature ever needs a modern renderer, it can be added behind `td-platform` without touching the core crates.
- **C# + MonoGame/SDL:** viable, but a heavier runtime and less natural fit for exact integer semantics.
- **Engine (Godot/Unity):** overkill for a 320×200 indexed renderer, and it makes deterministic simulation harder.
