# TypeLang ネイティブ開発ガイド（2025 版）

## 0. 目的とスコープ
本書は TypeLang HM 系言語のネイティブバックエンドを保守・拡張する開発者向けリファレンスである。インタプリタと共有する型推論フロントエンドから、Cranelift による機械語生成、`runtime_native` との連携、テストおよび運用フローまでを一望できるよう構成した。初めて担当するメンバーが 1 日以内にパイプラインを再現し、課題を切り分けられることを目標とする。

## 1. バックエンド構成の俯瞰
ネイティブ経路は次の段階で構成される。

```
.tl source (HM AST)
        │
        ▼
Parser + Typechecker
        │
        ▼
Core IR (Module, DictionaryInit)
        │
        ▼
Lowering (CodegenEnv, PrimOp)
        │
        ▼
Cranelift (MachInst, Faerie)
        │
        ▼
runtime_native + linker
        │
        ▼
ELF executable (x86_64)
```

- フロントエンドは既存の HM インタプリタと共通で、型クラス制約を `DictionaryInit` メタデータとして Core IR に埋め込む。
- Core IR は関数シグネチャと `Expr::DictionaryPlaceholder` によって辞書を明示し、`PrimOp` へ辞書フォールバックの可否を記録する。
- Codegen 層では `CodegenEnv` が辞書キャッシュと `ValueTy` 情報を保持し、Cranelift IR を生成する。
- 最終成果物は `runtime_native` と静的リンクされた x86_64 ELF 実行ファイルであり、`build/` 配下に出力される。

## 2. 前提条件とセットアップ
ネイティブビルドを行う前に次を満たしていることを確認する。

- Rust stable ツールチェーン（`rustup default stable`）と `cargo` が導入済みであること。
- Linux x86_64 ホストでのビルドを想定（CI も同構成）。他アーキテクチャは未検証。
- `cargo fetch` 済み、`make check` がエラーなく完走する初期状態を確保する。
- `runtime_native` クレートは `cargo test -p runtime_native` でグリーンであること。
- ネイティブ出力ディレクトリ（例: `build/`）は書き込み可能であり、古い生成物が残っている場合は必要に応じて削除する。

## 3. コンパイルパイプライン詳細
### 3.1 HM フロントエンド
`.tl` ソースは `parser` と `typecheck` によって AST → 型注釈付き Core IR へ変換される。ここで得た型情報はインタプリタと共有される。

### 3.2 Core IR 生成
`core_ir::lower` がモジュール単位の IR (`Module`, `Function`, `Expr`) を生成する。型クラス制約は `DictionaryInit` として枚挙され、関数境界で辞書パラメータを宣言する。

### 3.3 辞書メタ生成
`dictionary_codegen` が `TlValue` ベースの辞書初期化コードを生成し、`DictionaryMethod` ごとの `method_id` を割り当てる。生成物は `runtime_native::dict` に対応付けられる。

### 3.4 Cranelift コード生成
`codegen::cranelift::lower_module` が Core IR を Cranelift IR に変換する。`lower_primop` と `map_binop` が Unknown 型の演算を検知し、辞書経由のフォールバックパスを構築する。

### 3.5 出力アーティファクト
Cranelift で生成したオブジェクトは `runtime_native` のスタブとリンクされ、`build/<name>` に単一のバイナリとして出力される。デバッグ情報はデフォルトで有効（`dev` プロファイル）。

## 4. 型クラス辞書モノモーフ化の詳細
辞書関連の責務は段階的に分解されている。

- **Step A: メタ情報注入** – `PrimOp::dictionary_method()` が演算と辞書メソッドの対応を決定し、`FunctionSig`／`Parameter` に `dict_type_repr` を保持する。
- **Step B: プレースホルダ伝搬** – `Expr::DictionaryPlaceholder` が辞書パラメータの実引数位置に差し込まれ、`CodegenEnv::lookup_dictionary_param` がスコープ解決を担う。
- **Step C: Unknown 維持とフォールバック** – `convert_type_with_overrides` は `ValueTy::Unknown` を保持し、`lower_primop` が辞書フォールバックを構築する。辞書は `CodegenEnv::ensure_dictionary` により 1 度だけ初期化され、以降はキャッシュを参照する。

この結果、`PrimOp` は Int/Double/Bool の既存パスを保持しつつ、辞書が提供するメソッドシンボルに退避できる。

## 5. Cranelift コード生成の要点
- `lower_primop` は `PrimOp::dictionary_method()` の戻り値を基にフォールバックを選択し、`tl_dict_lookup(dict, method_id)` → 間接呼び出しというシーケンスを生成する。
- `map_binop` は未知型の二項演算を辞書経由に切り替え、既知型では従来通りの Cranelift 命令を使用する。
- `coerce_value` と `coerce_result` が `TlValue` とプリミティブ値間の変換を司り、辞書メソッドの ABI を満たす。
- 辞書キャッシュは `(class_name, type_repr)` をキーとしており、同一辞書の重複構築を防ぐ。
- 生成後のモジュールは `link_native_module` を経て `runtime_native` のシンボル群と結合される。

## 6. `runtime_native` ABI サマリ
ネイティブランタイムは `runtime_native/src` に配置され、以下のモジュールで構成される。

| モジュール | 代表 API | 解説 |
| --- | --- | --- |
| `value` | `TlValue`, `tl_value_from_int`, `tl_value_to_ptr`, `tl_value_release` | すべての値をボックス化し、参照カウントとエラーフラグを管理する。 |
| `dict` | `tl_dict_builder_*`, `tl_dict_lookup`, `tl_dict_build_record` | 辞書の組み立てと検索を担当。`method_id` による高速ルックアップが前提。 |
| `dict_fallback` | `tl_call_dict_method` など | Cranelift からの間接呼び出し補助と、失敗時のエラーメッセージ整形を行う。 |
| `list` | `tl_list_empty`, `tl_list_cons`, `tl_list_free` | リストの初期化と破棄。現在は主に将来のデータ型サポートのために保持。 |
| `data` | `tl_data_pack`, `tl_data_tag`, `tl_data_field` | 代数的データ型の構築とパターンマッチ支援。 |
| `error` | `tl_last_error`, `tl_clear_error` | ランタイムエラーの格納と取得。ネイティブバックエンドでは診断用に積極的に参照する。 |

## 7. ビルドと実行ワークフロー
### 7.1 CLI でのネイティブビルド
```bash
cargo run --bin typelang-repl -- \
  build examples/basics.tl \
  --emit native \
  --output build/basics_native \
  --print-dictionaries --json
```
- `--print-dictionaries` は生成された辞書を人間向けに表示し、`--json` を付与するとスナップショットテストに適した JSON を出力する。
- 出力バイナリは `./build/basics_native` に配置され、直接実行できる ELF となる。

### 7.2 REPL との連携
- REPL で定義した式はそのままネイティブ化できないため、エントリポイント `let main = ...` を `.tl` ファイルに用意して CLI からビルドする。
- `:dictionaries` コマンドは現在ヒント文字列を返すのみであり、詳細を確認する際は CLI 経由でのビルドを推奨する。

### 7.3 成果物の配置
- 生成されたバイナリと一時ファイルは `build/` 配下にまとまり、再ビルド時は同名ファイルを上書きする。
- `runtime_native` の静的ライブラリはビルド時に自動でリンクされ、追加設定は不要である。

## 8. テストと品質保証
- `cargo test core_ir_tests -- --nocapture` で Core IR の辞書注入挙動を確認する。
- `cargo test native_build -- --ignored` でネイティブ E2E テスト（辞書を含むサンプル）が実行される。
- `cargo test -p runtime_native` でランタイム ABI と辞書ビルダーの単体テストを網羅する。
- `make check` は `cargo fmt -- --check` → `cargo clippy --all-targets` → `cargo test --workspace` の順に実行され、CI と揃う。
- スナップショットが差分を検知した場合は `tests/native_build.rs` の期待値を確認し、辞書 JSON 出力が仕様通りか見直す。

## 9. デバッグとオブザーバビリティ
- `RUST_BACKTRACE=1` を付与して CLI を実行すると、ネイティブバックエンドで発生した panic のスタックトレースを取得できる。
- Cranelift の生成物を確認したい場合は `codegen::cranelift::debug_dump` 付近にログを追加し、一時的に `env_logger` を初期化する。
- 辞書関連の不具合は `--print-dictionaries --json` の出力と、`runtime_native/tests` の該当ケースを比較すると切り分けやすい。
- バイナリ実行時にランタイムエラーが発生した場合は `tl_last_error()` の内容が `stderr` に流れるため、再現手順とともに記録する。

## 10. よくあるエラーと対処

| エラーコード / 症状 | 典型的原因 | 推奨アクション |
| --- | --- | --- |
| `CODEGEN211` 「辞書パラメータがスコープ内に存在しません」 | `FunctionSig` に辞書パラメータが伝搬していない、または `Expr::DictionaryPlaceholder` が不足している | Core IR の関数引数を確認し、`dictionary_codegen` の出力と一致させる |
| `CODEGEN212` 「method_id が辞書に存在しません」 | `PrimOp::dictionary_method()` と辞書自動生成の `method_id` が不一致 | `dictionary_autogen.rs` と `PrimOp` のマッピングを同時に更新する |
| 「比較演算の引数型が Int ではありません」 | 辞書フォールバックが未実装の比較演算をネイティブ化した | Unknown 型向けの `PrimOp` を辞書対応させるか、現状はインタプリタで実行する |
| `PAR001` 関連メッセージ | `.tl` ファイルの構文が現状のパーサでサポートされていない（例: `=>` 付きの具象制約） | HM 構文へ書き換えるか、パーサ拡張を検討する |

## 11. 今後のロードマップ
- Bool/Double 向けの辞書テストを追加し、`tests/native_build.rs` でフォールバック経路を継続的に検証する。
- CLI `--print-dictionaries` の JSON スキーマを安定化し、`make check` のスナップショットテストと連携させる。
- クロスモジュール最適化（LTO）とリロケータブルオブジェクト出力の検証を行い、他ツールチェーンとの統合を可能にする。
- 将来的な `:dictionaries` 拡張に備え、REPL 側で辞書状態を取得する API を導出する。

## 12. ドキュメント運用ルール
- 仕様や ABI を変更した際は、本ドキュメントと `documents/plan/native-compile-spec.md` を同一ブランチで更新する。
- コマンド例は実際に動作確認したバージョンを記載し、将来の更新者が再現できるようにする。
- 新しいエラーコードを追加した場合はセクション 10 の表に追記し、再発防止策を整理する。
- リリース前には `make check` を必ず実行し、本書の内容と実装が乖離していないか確認する。
