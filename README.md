<!-- パス: README.md -->
<!-- 役割: TypeLang HM プロジェクトの全体像とセットアップ手順を案内するトップドキュメント -->
<!-- 意図: 利用者とコントリビューターが短時間で環境構築・開発に着手できるよう支援する -->
<!-- 関連ファイル: AGENTS.md, EBNF.md -->

# TypeLang HM (Rust) Overview

TypeLang HM は Hindley–Milner 型推論をコアに据えた教育向け関数型言語処理系です。Rust 標準ライブラリのみで構築し、読みやすさと壊しにくさを重視しています。

## 特徴ハイライト
- Algorithm W をベースにした HM 型推論と単一化エンジン
- `Eq` / `Ord` / `Show` / `Num` / `Fractional` など学習に必要な型クラスを同梱
- 基本演算、条件分岐、リスト操作など必修トピックを優先実装
- 整数累乗 `^` と実数指数 `**` の両方をサポート
- 履歴・矢印キー・複数行入力に対応した対話型 REPL

## クイックスタート
```bash
# REPL を起動
cargo run typelang-repl

# セッション例
> :let inc n = n + 1
> inc 5
6
> :t inc
-- Num a => a -> a
> :load examples/step2_functions.tl
Loaded 5 def(s)
```
`Ctrl+C` で現在の入力をキャンセル、`Ctrl+D` で REPL を終了できます。継続行は `.. ` プロンプトで示されます。

## ディレクトリガイド
- `src/ast.rs` — 抽象構文木の定義と表示補助
- `src/lexer.rs` / `src/parser.rs` — UTF-8 対応の字句/構文解析
- `src/typesys.rs` / `src/infer.rs` — 型表現、制約解決、defaulting
- `src/evaluator.rs` — 正格評価器とプリミティブ実装
- `src/repl/` — コマンド、レンダラ、履歴管理
- `examples/*.tl` — 段階別の教材スクリプト
- `tests/` — lexer / parser / infer / evaluator / repl の回帰テスト群
- `EBNF.md` — 言語仕様の正準 EBNF

## 学習ロードマップ
1. `examples/step1_numbers.tl` で数値演算とリテラルに慣れる。
2. `examples/step2_functions.tl` で関数定義と部分適用を体験。
3. `examples/step3_conditionals.tl` で条件式と論理演算を復習。
4. `tests/` を参照して仕様ベースのテスト作法を学ぶ。

## 開発ワークフロー
| フェーズ | コマンド | 目的 |
| --- | --- | --- |
| 日常開発 | `make check` | `cargo fmt` → `cargo clippy -D warnings` → `cargo test` を一括実行 |
| リリース前整備 | `make full_local` | クリーンビルド・テスト・ドキュメント・監査までを包括実行 |
| カバレッジ | `make coverage` | `cargo llvm-cov` による HTML/JSON/LCOV 出力 |

## 開発ポリシー
- 運用上の詳細は `AGENTS.md` に集約されています（Quickstart, チェックリスト等）。
- コメント・ドキュメントは日本語で、識別子は英語で統一します。
- 外部依存を追加する場合は事前に議論し、合意を得てから導入します。
- リモートへの push は必ずユーザーの明示承認を受けてください。

## ドキュメントリソース
- `AGENTS.md` — オペレーションガイドとテンプレート
- `EBNF.md` — 文法仕様の一次ソース

## トラブルシューティング
| 症状 | 対応策 |
| --- | --- |
| `make check` が失敗する | 直近の変更ファイルを確認し、`cargo test --test <name>` で局所再検証。 |
| REPL が固まる | `Ctrl+C` で現在の入力を一旦破棄し、多行入力が継続していないか確認。 |
| 推論結果が意図しない | `:t` で型を確認し、`Num` / `Fractional` 制約の残り具合をチェック。 |

## コミュニティ & ライセンス
- 質問・提案は GitHub Issue へ。発生手順、期待値と実際の結果、使用環境（OS・Rust 版）を添えてください。
- ライセンスは MIT（`LICENSE` を確認）。

Happy hacking!
