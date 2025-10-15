<!-- Path: README.md -->
<!-- What: User-facing guide to explore the TypeLang HM Rust interpreter -->
<!-- Why : Help newcomers install, launch, and learn the language quickly -->
<!-- RELEVANT FILES: EBNF.md, examples/, .codex/AGENTS.md -->

# TypeLang HM (Rust)

TypeLang HM is a compact Hindley–Milner language runtime written in Rust. Use this guide to get the interpreter running, explore sample programs, and learn what the language can do without diving into contributor workflows.

## Quickstart
1. Make sure the Rust toolchain is installed (`rustup default stable`).
2. Download or clone this repository, then open a terminal in the project root.
3. Launch the interactive interpreter:

```bash
cargo run
```

Inside the REPL you can quit with `Ctrl+D`, cancel a line with `Ctrl+C`, and reload the last command with the up-arrow.

## First Steps in the REPL
- Inspect the type of an expression: `:t 1 + 2`.
- Define a helper and reuse it:

```text
:let inc x = x + 1
inc 41
```

- Load a ready-made script and run it:

```text
:load examples/ebnf_blackbox.tl
```

## What You Can Explore
- **Core syntax**: `let` bindings, lambda abstractions, `if/then/else`, and exhaustive `case ... of` pattern matches.
- **Types**: Hindley–Milner inference plus common type classes (`Eq`, `Ord`, `Show`, `Num`, `Fractional`) with defaulting rules.
- **Data modeling**: algebraic data types via `data`, tuples, lists, pattern guards, and as-patterns (`x@pattern`).
- **Numbers & literals**: decimal/binary/octal/hex literals, integer (`^`) vs floating (`**`) exponents, Unicode strings and chars with shared escape sequences.

## Sample Programs
- `examples/intro.tl` — a tour of binding, conditionals, and pattern matching.
- `examples/ebnf_blackbox.tl` — exercises that mirror the reference grammar.
- `examples/adt_color.tl` — shows how to declare and pattern-match on algebraic data types.

Open any file, then paste snippets into the REPL or load them with `:load`.

## Learn the Language Deeper
- `EBNF.md` captures the full grammar if you want the formal syntax.
- `examples/` contains progressively harder scripts you can run verbatim or modify.
- `tests/` houses additional scenarios; treat them as advanced references when you need edge cases.

## Troubleshooting
- **`cargo` not found**: install Rust from <https://rustup.rs> and restart your terminal.
- **Build fails on first run**: run `rustup update` to pick up the latest stable toolchain, then retry `cargo run`.
- **Interpreter crashes on your script**: re-run with `RUST_BACKTRACE=1 cargo run` and file an issue with the stack trace.

## Stay in the Loop
- Star or watch the repository for release notes.
- Share questions or ideas via the issue tracker; feature requests are welcome.

## License
- MIT License (see `LICENSE`).

Happy hacking — keep it simple, observable, and well-documented.
