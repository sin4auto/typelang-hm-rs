# CONTRIBUTING（TypeLang HM / Rust）

TypeLang HM では「読みやすく、壊しにくい最小実装」を共同で維持することを目標にしています。以下の指針に従って作業してください。

## 基本方針
- 1 PR = 1 目的。差分は小さく保ちレビューしやすい単位でまとめる
- 識別子は英語、コメント/ドキュメント/会話は日本語
- 外部依存は原則禁止（標準ライブラリのみ）。追加が必要な場合は事前に議論する

## コーディング規約
- Rust 2021 / MSRV 1.70 を維持（`rust-toolchain.toml` 固定）
- 必須ゲート: `cargo fmt` / `cargo clippy -D warnings` / `cargo test`
- 公開 API (`pub`) の追加は慎重に行い、最小限に留める
- `panic!` は到達不能コードやテスト補助など限定的な場面でのみ使用し、利用者向け API では `Result` を返す

## テストと品質保証
- 小規模確認: `make check`（`fmt` → `clippy` → `test`）
- PR マージ・リリース前: `make full_local`（`clean` → `fmt` → `clippy` → `test` → `doc -D warnings` → `audit` → `outdated` → `coverage` → `release` → `udeps` → `miri`）
- 例示プログラム `examples/*.tl` を含む全テスト (`tests/`) が通ることを必須条件とする
- カバレッジ確認やログ要約には `make coverage` / `make test-brief` / `make build-brief` を活用

## ディレクトリ/モジュールの目安
- `src/lexer.rs` / `src/parser.rs`: 字句解析・構文解析（優先順位/結合性、型注釈）
- `src/typesys.rs` / `src/infer.rs`: 型表現、制約、単一化、defaulting 補助
- `src/evaluator.rs`: 正格評価器とプリミティブ実装
- `src/repl/`: コマンド処理、ラインエディタ（矢印ヒストリー対応）、表示、ファイル読込
- `examples/`: 学習サンプルと回帰テスト対象
- `tests/`: lexer / parser / infer / evaluator / repl / integration など領域別テスト
- `EBNF.md`: 言語仕様の参照定義

## 作業フロー
1. `make check` で整形・Lint・テストを通す
2. レビュー前または結合テスト前に `make full_local` を実行
3. Serena 要約は必要に応じて `make serena-summarize` で更新。差分サマリは `make diffpack`
4. 履歴やログは `.serena/`・`.summ/`・`.codex-home/` に出力される。これらは `.gitignore` 済みでコミットしない

## PR / コミット運用
- タイトルは英語の命令形 (`add`, `fix`, `update` など)
- 本文（日本語）に背景 / 対処 / 影響 / テスト結果を簡潔に記載
- コミットメッセージは [Conventional Commits](https://www.conventionalcommits.org/) 形式を推奨
- ブランチは `feature/*`, `fix/*`, `chore/*` など目的が分かる名前にする
- CI（GitHub Actions）では `make ci` を実行し、main ブランチへの直接 push は禁止

## リリース指針
- セマンティックバージョニング（MAJOR / MINOR / PATCH）
- リリース直前に `make full_local` と `cargo publish --dry-run` を実施
- タグは `vMAJOR.MINOR.PATCH` 形式。CHANGELOG 更新後にタグ付け

## ドキュメント・ナレッジ共有
- 新規モジュールを追加したら `docs/` または関連 Markdown に概要と使用例を追記
- 言語仕様の変更は `EBNF.md` と `README.md` を更新
- トークン節約や会話向け要約の更新は `.serena/MODEL_INPUT.md` を参照

## 連絡先
バグ報告・提案は GitHub Issue へ。再現手順、期待結果/実際の結果、使用環境（OS・Rust 版）を記載してください。

codex起動時は必ずこのREDEME.mdファイルを読み込むこと。