<!-- パス: README.md -->
<!-- 役割: TypeLang HM プロジェクトの全体像とセットアップ手順を案内するトップドキュメント -->
<!-- 意図: 利用者とコントリビューターが短時間で環境構築・開発に着手できるよう支援する -->
<!-- 関連ファイル: .codex/AGENTS.md, EBNF.md -->

# TypeLang HM (Rust) Overview

- TypeLang HM は Hindley–Milner 型推論とアルゴリズム W を核にした教育向け関数型言語処理系です。Rust 標準ライブラリのみで構築し、読みやすさと壊しにくさを最優先にしています。
- `data` 宣言／`case ... of`／パターンマッチまで含む最小実装を備え、式・型・字句仕様は `EBNF.md` を唯一のソースとして管理しています。
- Codex を活用した AI エージェント開発のリファレンスとしても利用でき、Makefile で `fmt` / `clippy` / `test` を一括オーケストレーションできます。

## ハイライト
- HM 型推論 + 型クラス (`Eq` / `Ord` / `Show` / `Num` / `Fractional`) による宣言的な型付け
- 代数的データ型 (`data`) と `case` パターンマッチ、ワイルドカード・コンストラクタ・ネスト対応
- 整数演算の強化: `div` / `mod` (Euclid) と `quot` / `rem` (切り捨て) に加え `^` / `**` を提供
- REPL はヒストリ・矢印キー・多行入力・`:load` に対応し、`EBNF` テンプレート全体の即時確認が可能
- `examples/ebnf_blackbox.tl` を用いた仕様ブラックボックステストと、`tests/` 以下の自動回帰群

## クイックスタート
```bash
# REPL を起動
cargo run

# セッション例
> data Maybe a = Nothing | Just a
> :let fromMaybe def opt = case opt of { Nothing -> def; Just x -> x }
> :t fromMaybe
-- a -> Maybe a -> a
> fromMaybe 0 (Just 42)
42
> div 7 3
2
> :load examples/ebnf_blackbox.tl
Loaded 24 def(s)
```
`Ctrl+C` で現在入力をキャンセル、`Ctrl+D` で REPL を終了。継続行は `.. ` プロンプトで表示されます。

## 言語機能スナップショット
- **データ定義**: `data Color = Red | Green | Blue` / パラメトリック `data Box a = Box a`
- **パターン**: ワイルドカード `_`、変数、リテラル、コンストラクタ (`Just (Box x)` など)
- **式**: `case`、`let-in`、ラムダ、部分適用、リスト／タプル／型注釈
- **プリミティブ**: 算術 `+ - * /`、冪乗 `^` / `**`、比較、`div/mod/quot/rem`
- **型クラス**: `Num` / `Fractional` に応じて数値推論・defaulting を制御 (`:t` / `--defaulting`)

## 学習ロードマップ
1. `examples/step1_numbers.tl` — リテラルと基本演算
2. `examples/step2_functions.tl` — 一階関数・部分適用
3. `examples/step3_conditionals.tl` — 条件分岐と論理演算
4. `examples/ebnf_blackbox.tl` — 文法全体の総合確認 (data / case / プリミティブ含む)
5. `tests/` — 仕様ベーステストを読み、追加シナリオを設計

## プロジェクト構成
- **`EBNF.md`** — 文法仕様の一次ソース
- `examples/` — 教材兼ブラックボックス検証スクリプト
- `src/ast.rs` / `src/lexer.rs` / `src/parser.rs` — AST と UTF-8 対応フロントエンド
- `src/typesys.rs` / `src/infer.rs` — 型表現・単一化・defaulting・パターン推論
- `src/runtime.rs` / `src/evaluator.rs` — 値表現と評価器 (`Value::Data` 等)
- `src/primitives.rs` — プリミティブ定義メタデータ
- `src/repl/` — REPL コマンド・ローダ・レンダラ
- `tests/` — lexer / parser / infer / evaluator / repl をカバーする統合テスト

## 開発ワークフロー
| フェーズ | コマンド | 目的 |
| --- | --- | --- |
| 日常開発 | `make check` | `cargo fmt` → `cargo clippy -D warnings` → `cargo test` を一括実行 |
| CI 確認 | `make ci` | `clean` → `fmt-check` → `clippy` → `test` → `release` (`--frozen --locked`) |
| リリース前整備 | `make full_local` | ビルド・テスト・ドキュメント・監査をワンコマンドで再現 |
| カバレッジ | `make coverage` | `cargo llvm-cov` による HTML / JSON / LCOV 生成 |

## 開発ポリシー
- 詳細な運用手順・テンプレは `.codex/AGENTS.md` を参照
- コメント・ドキュメントは日本語、識別子は英語で統一
- 仕様変更時は **EBNF → 実装 → テスト → examples** の順で同期

## ドキュメント & テストリソース
- `EBNF.md` — 公式仕様
- `examples/ebnf_blackbox.tl` — 仕様網羅テスト (ゼロベースで整備済み)
- `tests/integration.rs` — `:load` / REPL パイプライン / `data` 読み込み検証

## トラブルシューティング
| 症状 | 対応策 |
| --- | --- |
| `make check` が失敗 | 変更箇所に絞って `cargo test --test <name>` / `cargo clippy -p typelang-hm-rs` を実行 |
| `:load` で型エラー | `EBNF.md` と `examples/ebnf_blackbox.tl` の差分を確認し、仕様と実装の乖離を解消 |
| `case` が該当しない | パターン重複や抜け漏れを確認し、ワイルドカード `_` を追加 |
| `div/mod/quot/rem` の挙動が不明 | Euclid (`div`/`mod`) と trunc (`quot`/`rem`) の違いを `tests/evaluator.rs` に沿って検証 |

## ライセンス
- MIT License（詳細は `LICENSE` を参照）

Happy hacking!
