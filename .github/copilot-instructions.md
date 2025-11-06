# Copilot Instructions for TypeLang HM (Rust)

## Overview
TypeLang HM is a compact Hindley–Milner language runtime written in Rust. The codebase is organized for clarity, modularity, and ease of experimentation. AI agents should prioritize minimal, readable, and well-documented changes, following the project's conventions.

## Architecture & Key Components
- **src/**: Core interpreter logic. Submodules:
  - `ast.rs`, `typesys.rs`, `infer.rs`: Abstract syntax tree, type system, and inference engine.
  - `evaluator.rs`, `runtime.rs`, `primitives.rs`: Evaluation, runtime, and built-in primitives.
  - `lexer.rs`, `parser/`: Lexical analysis and parsing (see `parser/expr.rs`, `parser/program.rs`).
  - `repl/`: REPL implementation and utilities.
- **examples/**: Sample programs for language features and grammar exercises.
- **tests/**: Rust integration and unit tests for all major components.
- **EBNF.md**: Formal grammar reference for the language.

## Developer Workflows
- **Build & Run**: Use `cargo run` to launch the REPL. For debugging, run with `RUST_BACKTRACE=1 cargo run`.
- **Testing**: Run all tests with `cargo test`. Test files are in `tests/` and cover edge cases and advanced scenarios.
- **Script Loading**: In the REPL, use `:load examples/<file>.tl` to run sample scripts.
- **Troubleshooting**: If build fails, update Rust toolchain (`rustup update`). For interpreter crashes, provide stack traces.

## Project Conventions
- **Japanese 4-line header**: Add to every file (see `.codex/AGENTS.md`).
- **Minimal, focused changes**: 1 PR = 1 purpose. Prefer small, simple solutions (80/20 rule).
- **Comment non-obvious logic** and use clear identifiers.
- **Read all related files before editing**; start with a working prototype.
- **Risky operations**: Present reason, alternatives, and rollback plan before acting.
- **Push to GitHub only with explicit user approval.**

## Patterns & Examples
- **Type inference**: See `src/infer.rs` for Hindley–Milner logic.
- **Algebraic data types**: See `examples/adt_color.tl` and `src/typesys.rs`.
- **REPL commands**: Implemented in `src/repl/` (e.g., `:t`, `:let`, `:load`).
- **Grammar**: Reference `EBNF.md` and `examples/ebnf_blackbox.tl` for syntax coverage.

## Integration Points
- No external dependencies beyond Rust std and `Cargo.toml`.
- All communication is internal; no network or external service calls.

## References
- For agent principles and workflow, see `.codex/AGENTS.md`.
- For user-facing guide, see `README.md`.
- For grammar, see `EBNF.md`.

---

**Feedback requested:** If any section is unclear or missing, please specify so it can be improved for future AI agent productivity.
