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

---

# TypeLang HM (Rust)（日本語）

TypeLang HMは、Rustで実装されたコンパクトなHindley–Milner型推論ランタイムです。このガイドでは、インタープリタの起動方法からサンプルプログラムの体験、言語の可能性を素早く把握する方法までをまとめています。

## クイックスタート
1. Rustツールチェーンがインストールされていることを確認します（例：`rustup default stable`）。
2. 本リポジトリをダウンロードまたはクローンし、プロジェクトルートでターミナルを開きます。
3. 対話型インタープリタを起動します。

```bash
cargo run
```

REPL内では`Ctrl+D`で終了、`Ctrl+C`で入力中の行を破棄、矢印キーの上で直前の入力を再実行できます。

## REPLでの最初の手順
- 式の型を確認：`:t 1 + 2`
- ヘルパーを定義して再利用：

```text
:let inc x = x + 1
inc 41
```

- 既存のスクリプトを読み込んで実行：

```text
:load examples/ebnf_blackbox.tl
```

## できること
- **コア構文**：`let`束縛、ラムダ抽象、`if/then/else`、網羅的な`case ... of`パターンマッチ。
- **型システム**：Hindley–Milner型推論に加え、`Eq` / `Ord` / `Show` / `Num` / `Fractional`といった一般的な型クラスとデフォルト化ルール。
- **データモデリング**：`data`による代数的データ型、タプル、リスト、ガード付きパターン、asパターン（`x@pattern`）。
- **数値とリテラル**：10進・2進・8進・16進リテラル、整数（`^`）と浮動小数（`**`）の累乗演算、共通エスケープシーケンスを共有するUnicode文字列と文字。

## サンプルプログラム
- `examples/intro.tl` — 束縛、条件分岐、パターンマッチを一通り体験できます。
- `examples/ebnf_blackbox.tl` — 参照文法と対応した演習問題です。
- `examples/adt_color.tl` — 代数的データ型の宣言とパターンマッチの例を示します。

任意のファイルを開き、スニペットをREPLに貼り付けるか、`:load`で読み込んでください。

## さらに学ぶ
- `EBNF.md`で形式文法を確認できます。
- `examples/`には段階的に難易度が上がるスクリプトが並んでおり、コピー＆ペーストでそのまま実行できます。
- `tests/`には追加シナリオがあります。エッジケースを知りたいときの上級リファレンスとして活用してください。

## トラブルシューティング
- **`cargo`が見つからない**：<https://rustup.rs>からRustをインストールし、ターミナルを再起動します。
- **初回ビルドが失敗する**：`rustup update`で最新の安定版ツールチェーンを取得し、改めて`cargo run`を実行します。
- **スクリプト実行でクラッシュする**：`RUST_BACKTRACE=1 cargo run`で再実行し、スタックトレースを添えてIssueを起票してください。

## 最新情報の入手
- リリース情報を追うには、リポジトリをStarまたはWatchしてください。
- 質問やアイデアはIssueトラッカーへ。新機能の提案も歓迎です。

## ライセンス
- MIT License（`LICENSE`参照）。

楽しんでください — シンプルに、観察可能に、十分にドキュメント化された形で進めましょう。
