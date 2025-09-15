# CONTRIBUTING（TypeLang HM / Rust）

「読みやすく、壊しにくい最小実装」を守りつつ、学習・検証に役立つ処理系を一緒に育てましょう。

## 基本方針
- 小さくレビューしやすく（1 PR = 1 目的）
- 識別子は英語、コメント/ドキュメントは日本語
- 外部依存なし（標準ライブラリのみ）

## コーディング規約
- Rust 2021 / MSRV: `1.70`
- 必須ゲート: `cargo fmt` / `cargo clippy -D warnings` / `cargo test`
- パブリック API は最小限（`pub` の拡張は慎重に）
- `panic!` は到達不能やテスト補助に限定。利用者には `Result` を返す

## テスト方針
- 変更時はユニット/統合/回帰を実行
- 型表示は「必要部分の比較」を重視（表記ゆらぎへ過剰依存しない）
- 例示プログラム（`examples/*.tl`）は回帰対象

## ディレクトリ構成の目安
- `lexer.rs` / `parser.rs`: トークン化・構文解析（優先順位/結合性）
- `typesys.rs` / `infer.rs`: 型・制約・単一化・推論（表示用 defaulting を別扱い）
- `evaluator.rs`: 正格評価／プリミティブはカリー化
- `repl.rs`: 入力補助と親切なエラーを最優先

## PR 作法
- タイトル: 英語の命令形（例: `infer: add defaulting for Fractional`）
- 本文（日本語）: 背景 / 対処 / 影響 / テスト結果を簡潔に
- 変更前後で `make check` をパスさせること

## リリースポリシー
- SemVer: MAJOR（互換破壊）/ MINOR（追加）/ PATCH（修正）
- リリース前チェック: `fmt`/`clippy`/`test`/`doc -D warnings`

## 非推奨（Deprecation）
- 非推奨は最短 1 マイナーの移行期間（例: 0.1.x で非推奨 → 0.2.0 で削除）
- `#[deprecated(since, note)]` を付け、予定を本ファイルに明記

## 連絡先
バグ・提案は Issue へ。再現手順、期待値/実際の結果、使用環境（OS/Rust 版）を添えてください。
