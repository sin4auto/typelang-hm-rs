# 運用ガイドライン（TypeLang HM）

## 言語・記法
- 会話・ドキュメント・コメントは日本語、識別子は英語で統一
- エラーメッセージも可能な限り日本語で表示

## 開発フロー
- 小規模確認: `make check`（`cargo fmt` → `cargo clippy -D warnings` → `cargo test`）
- 最終確認: `make full_local`（`clean` → `fmt` → `clippy` → `test` → `doc -D warnings` → `audit` → `outdated` → `coverage` → `release` → `udeps` → `miri`）
- Serena 要約と差分サマリ: `make serena-summarize` / `make diffpack` / `make test-brief` / `make build-brief`

## コーディング規約
- Rust 2021 / MSRV 1.70、外部依存は追加しない
- `panic!` の使用は最小限。利用者には `Result` を返す
- 公開 API (`pub`) の追加は慎重に行う

## ドキュメント更新
- 言語仕様: `EBNF.md`
- プロジェクト概要: `README.md`
- 運用詳細: `AGENTS.md`
- `.serena/` と `.summ/` は生成物ディレクトリ（コミットしない）

## REPL ヒント
- `cargo run --bin typelang-repl` で起動
- 矢印キーや `Backspace` による行編集、`:history` で履歴表示
- 多行入力は `.. ` プロンプトで継続

その他の詳細はリポジトリの `AGENTS.md` を参照してください。
