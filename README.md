# TypeLang HM プロジェクトガイド

TypeLang HM は、Hindley–Milner 型推論を核とした学習向け関数型言語を Rust で実装したプロジェクトです。REPL による対話実行と Cranelift ベースのネイティブコード生成の両方をサポートし、ミニマルな言語処理系を使いながら型クラス辞書やコード生成の仕組みを学べます。

## 1. 主な特徴
- **Hindley–Milner 型推論**と `Eq` / `Ord` / `Show` / `Num` / `Fractional` などの型クラスを実装。
- **REPL と CLI** を共通フロントエンド（パーサ／型推論）で共有し、スクリプトの対話評価とビルドを同一コードで行う。
- **Cranelift ベースのネイティブバックエンド**を搭載し、型クラス辞書をモノモーフ化してランタイム (`runtime_native`) とリンク。
- `make check` や `make full_local` による **統合的な品質ゲート**を提供（フォーマット、Lint、テスト、監査、カバレッジまで一括実行）。
- 詳細なドキュメント群（`documents/` 配下）でコンパイラ内部の設計を段階的に把握可能。

## 2. リポジトリ構成
| パス | 説明 |
| ---- | ---- |
| `src/` | インタプリタ／型推論／コード生成のメイン実装 |
| `src/codegen/` | Cranelift ネイティブバックエンド |
| `runtime_native/` | ネイティブ実行時ランタイム（辞書 ABI など） |
| `examples/` | REPL やビルドで利用できるサンプル `.tl` ファイル |
| `tests/` | Core IR, ネイティブビルド, 型推論などの統合テスト |
| `documents/native.md` | ネイティブバックエンド全体の最新ガイド |
| `documents/plan/native-compile-spec.md` | 辞書モノモーフ化フェーズの仕様書 |

## 3. 必要環境
- Rust stable ツールチェーン (`rustup default stable`)
- Linux x86_64（CI と同一前提）。他アーキテクチャは未検証。
- `cargo` で依存取得が可能なネットワーク環境

## 4. クイックスタート
### 4.1 REPL（インタプリタ）を起動
```bash
cargo run --bin typelang-repl
```
- `Ctrl+D` で終了、`Ctrl+C` で入力キャンセル。
- 型確認：`:t 1 + 2`
- スクリプトロード：`:load examples/intro.tl`

### 4.2 ネイティブバイナリを生成
```bash
cargo run --bin typelang-repl -- \
  build examples/basics.tl \
  --emit native \
  --output build/basics_native \
  --print-dictionaries --json
```
- 生成バイナリは `build/basics_native` として保存。
- 辞書情報を JSON で確認可能。詳細は `documents/native.md` を参照。

## 5. 言語のエッセンス
- **基本構文**：`let` 束縛、ラムダ、`if/then/else`、`case ... of`。
- **データ定義**：`data` で代数的データ型、タプル、リスト、`x@pattern` などのパターンガード。
- **型クラス**：辞書ベースで実装。`Num` / `Eq` などは辞書初期化コードが自動生成される。
- **リテラル**：整数／浮動小数（`^` と `**` が使い分け）、Unicode 文字列と文字リテラル。
- 詳細な文法は `documents/EBNF.md` を参照。

## 6. 開発ワークフロー
| コマンド | 用途 |
| -------- | ---- |
| `make check` | `cargo fmt` → `cargo clippy` → `cargo test` をまとめて実行 |
| `make full_local` | 追加ツール導入 → clean → doc → fmt → clippy → test → release → audit → outdated → udeps → miri → coverage の総合検証（ネットワーク権限が必要） |
| `cargo run --bin typelang-repl -- --help` | CLI オプションの確認 |
| `cargo test -- --ignored` | ネイティブバックエンドの重いテストを含めて実行 |
| `cargo +nightly miri test --test native_build` | Miri による未定義動作検査（ネイティブ E2E は隔離のため自動で無効化） |

> **注意**: `make full_local` は `cargo audit` や `cargo llvm-cov` などの外部リソースへアクセスするため、CI/sandbox 環境では失敗する場合があります。必要に応じて個別コマンドを手動で実行してください。

## 7. テストと品質保証
1. **単体／統合テスト**: `cargo test --workspace --all-features`
2. **Core IR スナップショット**: `tests/core_ir_tests.rs`
3. **ネイティブ E2E**: `tests/native_build.rs`（Miri 実行時は自動 ignore）
4. **ランタイム ABI**: `runtime_native/tests/`
5. **カバレッジ**: `cargo +nightly llvm-cov --help` で利用方法を確認

## 8. トラブルシューティング
- **`cargo` が見つからない**：<https://rustup.rs> から Rust をインストール。
- **ビルドが失敗する**：`rustup update` 後に `cargo clean && cargo build` を再実行。
- **ネイティブビルドで辞書不足エラー**：`--print-dictionaries --json` の結果と `documents/plan/native-compile-spec.md` の仕様を参照し、必要な辞書を実装。
- **Miri で `mkdir` 関連エラー**：隔離下ではネイティブ E2E が実行できないため、`MIRIFLAGS=-Zmiri-disable-isolation` を設定するか（非推奨）、該当テストを通常の `cargo test` で確認。

## 9. 参考ドキュメント
- `documents/native.md` — ネイティブバックエンドの全体像とテスト戦略。
- `documents/plan/native-compile-spec.md` — 型クラス辞書モノモーフ化フェーズの詳細設計。
- `documents/EBNF.md` — 言語仕様（EBNF）。
- `documents/plan/` 以下 — その他の設計／ロードマップ資料。

## 10. 貢献ガイドライン（概要）
- 変更前に `make check` を実行し、最低限の品質を担保。
- 仕様変更や ABI 変更時は関連ドキュメントも同ブランチで更新。
- コミットメッセージはコンベンショナルコミット形式を推奨（例: `feat: add fractional dictionary fallback`）。
- Issue や PR では再現手順・検証結果を明記するとレビューが円滑です。

## 11. ライセンス
本プロジェクトは MIT ライセンスで配布されています。詳細は `LICENSE` を参照してください。

---

障害や疑問点があれば Issue を通じてフィードバックしてください。TypeLang HM が学習と実験の良き土台となることを願っています。
