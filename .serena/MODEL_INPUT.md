# TypeLang HM / Rust 要約（更新: 2025-09-17）

## プロジェクトの目的・範囲
- Hindley–Milner 型推論を備えた極小の関数型言語処理系を Rust で実装し、学習・検証用途に活用する
- ライブラリ `typelang` と REPL バイナリ `typelang-repl` の二本立てで、読みやすさと拡張容易性を重視
- 外部依存は追加せず標準ライブラリのみで完結。MSRV は 1.70 を維持

## 使用技術と依存
- Rust 2021 Edition (`Cargo.toml` の `rust-version = 1.70`)
- 検証に `cargo fmt` / `cargo clippy` / `cargo test` / `cargo doc` / `cargo llvm-cov`
- CI は GitHub Actions で `make ci`（fmt-check → clippy → test → release）を実行

## ディレクトリと主モジュール
- `src/ast.rs`: 抽象構文木と pretty-printer
- `src/lexer.rs` / `src/parser.rs`: UTF-8 対応字句/構文解析、演算子優先順位・型注釈処理
- `src/typesys.rs` / `src/infer.rs`: 型表現、制約、単一化、defaulting 補助
- `src/evaluator.rs`: 正格評価器、整数/浮動/Bool/Char/String/リスト/タプルのプリミティブ
- `src/repl/`: `cmd.rs`（コマンド処理）・`line_editor.rs`（履歴付きライン入力）・`loader.rs`・`printer.rs`・`util.rs`
- `examples/*.tl`: サンプルコード。`tests/` とともに回帰対象
- `EBNF.md`: 言語仕様の参照定義

## ビルド・テスト・CI
- 小規模確認: `make check`（`cargo fmt` → `cargo clippy -D warnings` → `cargo test`）
- 最終確認/リリース前: `make full_local`（`clean` → `fmt` → `clippy` → `test` → `doc -D warnings` → `audit` → `outdated` → `coverage` → `release` → `udeps` → `miri`）
- 個別ターゲット: `make doc` / `make coverage` / `make add-tools`
- 生成物は `target/` に出力。コミット前に不要ファイルがないか確認

## Serena / トークン節約運用
- `make serena-summarize` で本ファイルなど `.serena/` の要約を自動更新
- `make diffpack` / `make test-brief` / `make build-brief` で `.summ/` に差分・ログ要約を生成
- `make model-pack` で `.serena` と `.summ` を `model_pack.zip` にまとめ会話入力を効率化
- `.serena/` と `.summ/` は `.gitignore` 登録済みでコミットしない

## REPL 利用メモ
- `cargo run --bin typelang-repl` で起動。矢印キーと Backspace による行編集、`:history` で履歴表示
- 多行入力は `.. ` プロンプトで継続。`Ctrl+C` は入力中断、`Ctrl+D` は終了
- 主なコマンド: `:type` / `:let` / `:load` / `:reload` / `:browse` / `:set default on|off` / `:unset` / `:quit`

## 補足
- コメント・ドキュメントは日本語、識別子は英語
- 変更時は `AGENTS.md` の貢献ガイドラインに従う
- ライセンスは MIT（`LICENSE`）
