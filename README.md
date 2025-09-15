# TypeLang HM (Rust)

[![CI](https://github.com/sin4auto/typelang-hm-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/sin4auto/typelang-hm-rs/actions/workflows/ci.yml)
[![Coverage](https://codecov.io/gh/sin4auto/typelang-hm-rs/branch/main/graph/badge.svg)](https://codecov.io/gh/sin4auto/typelang-hm-rs)
[![MSRV](https://img.shields.io/badge/MSRV-1.70%2B-blue.svg)](#)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)

<!-- NOTE: CI/coverage バッジは sin4auto/typelang-hm-rs を参照しています。private の場合は Codecov のトークン設定が必要です。 -->

学習と検証に特化した極小の関数型言語処理系です。Rust 実装は Hindley–Milner 型推論を中核に、最小限の型クラス、簡易評価器、REPL を備えます。設計目標は「読みやすく、壊しにくい最小実装」。

## 特長
- HM 型推論（let 多相）と単一化
- 最小型クラス: `Eq` / `Ord` / `Show` / `Num` / `Fractional`
- 値: 整数/浮動小数/Bool/Char/String/リスト/タプル
- 演算: 比較（辞書式対応）、四則、累乗（`^` 整数指数、`**` 連続値指数）
- REPL で型照会・定義・ファイルロード

## カバレッジ
- ツール: `cargo-llvm-cov`（LLVM の source-based coverage）
- 導入: `cargo install cargo-llvm-cov`
- Makefile ターゲット（どれもワークスペース全体を対象）
  - `make coverage`          — HTML レポートを生成（`target/llvm-cov/html/index.html`）
  - `make coverage-summary`  — 概要のみを標準出力へ
  - `make coverage-lcov`     — `lcov.info` を生成（Codecov などに送信可能）
  - `make coverage-json`     — `coverage.json` を生成（CI の閾値チェックが利用）
  - `make coverage-clean`    — 生成物の掃除

CI 連携（GitHub Actions）
- `coverage` ジョブが `cargo-llvm-cov` を用いて `lcov.info` と `coverage.json` を生成し、
  - Codecov にアップロード（バッジ用）
  - PR には自動でカバレッジコメントを投稿（差分の閾値判定は `COVERAGE_MIN` で制御）
- 閾値は `.github/workflows/ci.yml` の `COVERAGE_MIN` で調整（既定: 70）。

## クイックスタート（1分）
```
cargo run --bin typelang-repl
> :let add x y = x + y
> add 2 3
5
> :t \x -> x ** 2
-- Fractional a => a -> a
> :load examples/basics.tl
> square 12
144
-- 例: ファイルをロードすると各定義の型が表示されます
> :load examples/advanced.tl
Loaded 6 def(s) from examples/advanced.tl
  plus1 :: Num a => a -> a
  one_int :: Int
  inv2 :: Double
  powf :: Double
  make_pair :: a -> b -> (a, b)
```

## REPL コマンド
- `:t EXPR` / `:type EXPR`  型を表示（`:set default on|off` で既定化の切替）
- `:let DEF [; DEF2 ...]`  その場で定義（複数は `;` 区切り）
- `:load PATH` / `:reload`  ファイル読込/再読込
- `:browse [PFX]`  定義一覧（接頭辞フィルタ）
- `:unset NAME`  定義削除
- `:quit`  終了

## 言語の要点
- 式: 変数 / 関数適用 / ラムダ `\x y -> ...` / `let ... in ...` / `if ... then ... else ...` / 注釈 `e :: T`
- リテラル: `Int`/`Integer`/`Double`/`Bool`/`Char`/`String`/`[a]`/`(a,b,...)`
- 優先順位: 関数適用 > `^/**`(右結合) > `* /` > `+ -` > 比較
- 累乗: `^` は非負の整数指数で整数計算。負指数は `Double` にフォールバック。`**` は常に連続値指数。

詳細仕様は `EBNF.md`、エラー体系は `ERROR_CODES.md` を参照してください。

## アーキテクチャ
- `lexer.rs` / `parser.rs`: UTF-8 安全な字句/構文解析（優先順位・結合性対応）
- `typesys.rs`: 型・制約・置換・単一化・表示（既定化は表示用）
- `infer.rs`: Algorithm W + 型クラス制約
- `evaluator.rs`: 正格・副作用なし。プリミティブをカリー化提供
- `repl/*`: 対話環境。`^` の負指数を `**` へ正規化し可読な型表示を維持

## 設計メモ
- 型クラス充足は学習重視で厳密検証を簡略化。誤用は表示抑制/フォールバックで扱う
- 既定化（defaulting）は「表示専用」。推論ロジックは既定化に依存しない
- 推論失敗時は評価結果から代表型（Int/Double/Bool/Char/String）にフォールバック

## 開発
- 要件: Rust 1.70+（標準ライブラリのみ）
- 品質: `cargo fmt` / `cargo clippy -D warnings` / `cargo test`
- まとめ: `make check`（fmt → clippy → test）/ `make doc`
- CI: GitHub Actions（`fmt`/`clippy`/`test`/`doc -D warnings`）

### コントリビュート
- 方針・規約は `AGENTS.md` を参照。
- 1 PR = 1 目的。小さく、読みやすく。`make check` が通ること。

## ライセンス
MIT（`LICENSE` 参照）。
