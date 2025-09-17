# TypeLang HM (Rust)

TypeLang HM は、Hindley–Milner 型推論を核にした最小構成の関数型言語処理系です。学習用途で読みやすく壊しにくいコードを目指し、Rust 標準ライブラリのみで実装しています。

## 特長
- Hindley–Milner 型推論（Algorithm W）と単一化をフルサポート
- 最小限の型クラス (`Eq` / `Ord` / `Show` / `Num` / `Fractional` / `Functor` / `Foldable`)
- Functor/Foldable 制約と高階関数 `map` / `foldl` / `foldr` を標準搭載
- 正格評価器とカリー化されたプリミティブ（整数・浮動小数・Bool・Char・String・リスト・タプル）
- 演算子優先順位/結合性の考慮、累乗演算 `^`（整数指数）と `**`（連続値指数）
- 使いやすい REPL（ヒストリー、矢印移動、`:history` 出力、多行入力）

## クイックスタート
```bash
# REPL を起動
cargo run --bin typelang-repl

# REPL 内の例
> :let square x = x * x
> :t square
-- Num a => a -> a
> square 12
144
> :load examples/basics.tl
Loaded 6 def(s) from examples/basics.tl
  factorial :: Num a => a -> a
  ...
> :history    # 直近の入力を一覧表示
```
REPL では矢印キー（↑/↓/←/→）と Backspace が利用でき、`Ctrl+C` で入力を中断、`Ctrl+D` で終了できます。複数行の式は `.. ` プロンプトで継続入力してください。

## プロジェクト構成
- `src/ast.rs` – 抽象構文木と表示ロジック
- `src/lexer.rs` / `src/parser.rs` – UTF-8 対応の字句・構文解析（優先順位/結合性込み）
- `src/typesys.rs` / `src/infer.rs` – 型表現、制約、単一化、defaulting 補助
- `src/evaluator.rs` – 正格評価器とプリミティブ実装
- `src/repl/` – REPL 本体（コマンド処理、表示、ファイル読込、ラインエディタ）
- `examples/*.tl` – 言語機能のサンプル
- `tests/` – lexer/parser/infer/evaluator/repl などの回帰テスト群
- `EBNF.md` – 言語仕様の EBNF 定義

## ビルド & テスト
小規模の変更確認: `make check`
```bash
make check   # cargo fmt → cargo clippy -D warnings → cargo test
```
最終確認（CI 相当フルセット）: `make full_local`
```bash
make full_local
# clean → fmt → clippy → test → doc -D warnings → audit → outdated → coverage → release → udeps → miri
```
その他の主なターゲット:
- `make doc` – `cargo doc` を警告をエラー扱いで生成
- `make coverage` – `cargo llvm-cov` による HTML/JSON/LCOV 出力
- `make add-tools` – 開発に必要なツール（rustfmt / clippy / cargo-llvm-cov 等）を導入

## トークン節約と Serena 連携
- `make serena-summarize` – `.serena/MODEL_INPUT.md` を最新化（Codex CLI 経由）
- `make diffpack` / `make test-brief` / `make build-brief` – `.summ/` 以下に差分やログ抜粋を生成
- `make model-pack` – `.serena` と `.summ` をまとめた `model_pack.zip` を作成
これらの生成物は `.gitignore` 済みでリポジトリには含めません。

## コントリビュート
- 方針・運用・リリース規約は `AGENTS.md` を参照してください
- コミット前に `make check` を実行し、PR 作成前またはリリース前に `make full_local` を通してください
- コメントやドキュメントは日本語、識別子は英語で統一します
- 依存は標準ライブラリのみ。外部クレートを追加する場合は事前に議論してください

## ライセンス
MIT License – 詳細は `LICENSE` を参照してください。
