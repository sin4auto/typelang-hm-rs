# 型クラス辞書モノモーフ化とリンク（2025 再設計版）

## 4.0 目的
- Core IR の型クラス制約を具象辞書へモノモーフ化し、Cranelift が辞書経由で Unknown 型の演算を安全に解決できるパイプラインを構築する。
- `typelang build --emit native examples/basics.tl` が Cargo なしでも実行でき、辞書不足時には `COREIR40x` / `CODEGEN21x` の明示的なエラーで停止する。
- CLI/REPL/テスト/CI で辞書メタ情報を共通利用し、辞書 ABI の退行を継続的に検出する。

## 4.0.1 前提タスク（段階的な実装計画）
### 1. 辞書メタ情報の注入（Step A）
- `FunctionSig` / `Parameter` に `dict_type_repr` を追加し、CLI/JSON 出力へ反映する。
- `DictionaryInit` / `DictionaryMethod` の `method_id`・`symbol` を必須化し、`NativeBuildArtifacts` に辞書メタを保存する。
- この段階では `ValueTy` は既存どおり具体型のままとし、既存テストを壊さないことを優先する。

### 2. 辞書プレースホルダと Codegen 経路の整備（Step B）
- `Expr::DictionaryPlaceholder` を辞書付き関数本体にも差し込み、`define_functions` → `CodegenEnv` まで辞書パラメータを伝搬させる。
- `CodegenEnv` に `(classname, type_repr)` キーの辞書キャッシュ API（`ensure_dictionary`, `lookup_dictionary_param` など）を追加し、辞書生成・再利用を統一する。

### 3. 型表現の再設計 + Cranelift 連携準備（Step C + Phase 4.3）
#### Step C-1: Unknown を保持する型変換
- `convert_type_with_overrides` を改修し、`ValueTy::Unknown` や抽象型を削除せずに Core IR へ残す。
- `LoweringContext` で Unknown のまま IR へ落ちる経路でも `PrimOp` / `Parameter` の `dict_type_repr` が失われないことを確認する。

#### Step C-2: map_binop の新仕様
- 具体型（`Int`, `Double`, `Bool`）の場合は従来どおり直接命令を返し、Unknown の場合は `PrimOp::dictionary_method()` 情報を添えて `PrimOp::DictionaryFallback` にマッピングする。
- Unknown となり得る演算（`add`, `sub`, `mul`, `div`, `neg`, `eq`, `lt`, `le`, `gt`, `ge`, `and`, `or`, `xor`）を列挙し、`map_binop` が必ず辞書経路を返すようにする。
- `map_binop` は「辞書フォールバックか否か」の二値を返す構造に変更し、後続の `lower_primop` が判定できるようにする。

#### Step C-3: lower_primop の再設計
- 具体型経路: 既存の Cranelift 命令を即時生成。
- 辞書フォールバック経路:
  1. `(classname, type_repr)` をキーとして `CodegenEnv` から辞書 `Value` を取得（なければ `ensure_dictionary`）。
  2. `tl_dict_lookup(dict, method_id)` → `TlValueKind::Pointer` でメソッドシンボルを受け取り、`declare_function` 済みシムを `builder.ins().call` する。
  3. 引数・戻り値の Cranelift 値を `coerce_value` で `TlValue` ⇔ 具象型に変換。
- 未解決辞書は `CODEGEN211`、メソッド未定義は `CODEGEN212` としてエラーを返す。

#### Step C-4: 辞書フォールバックが発生する PrimOp 一覧
`PrimOp::dictionary_method()` が唯一の真実のソースとなる。Unknown 型（または `map_binop` で `dict_fallback=true` と判断されたケース）のみ辞書経路に落ち、既知型は従来どおり Cranelift 命令を直接生成する。現在のマッピングは下表のとおり。

| 区分 | PrimOp バリアント | 型クラス / メソッド | `method_id` | 備考 |
| --- | --- | --- | --- | --- |
| 加減乗 | `AddInt`, `SubInt`, `MulInt` | `Num.add/sub/mul` | 0 / 1 / 2 | `Num<Int>` と `Num<Double>` で共通の ID を使用し、コンパイラ側は型表現から適切な辞書を選択する |
| 浮動小数 | `AddDouble`, `SubDouble`, `MulDouble`, `DivDouble` | `Num.add/sub/mul`, `Fractional.div` | 0 / 1 / 2 / 0 | `DivDouble` のみ `Fractional` クラスを参照 |
| 整数除算 | `DivInt`, `ModInt` | `Integral.div/mod` | 0 / 1 | `Integral` 辞書を要求。`mod` 追加に合わせ ID=1 を割り当て済み |
| 等価 | `EqInt`, `EqDouble`, `NeqInt`, `NeqDouble` | `Eq.eq/neq` | 0 / 1 | Bool 用の `Eq` も同じ ID を共有する |
| 比較 | `Lt*`, `Le*`, `Gt*`, `Ge*` | `Ord.lt/le/gt/ge` | 0 / 1 / 2 / 3 | Int/Double いずれも `Ord` 辞書を経由 |
| 論理 | `AndBool`, `OrBool`, `NotBool` | `BoolLogic.and/or/not` | 0 / 1 / 2 | 短絡演算ではなく辞書が直接ブール演算を決定 |

- `method_id` は辞書自動生成 (`dictionary_codegen`) および Cranelift の辞書呼び出しで共有し、`tl_dict_lookup(dict, method_id)` で実装シンボルを取得する。
- 新しい PrimOp を追加する際は `PrimOp::dictionary_method()` とこの表の両方を更新する。`method_id` が衝突すると `CODEGEN212` が発生するため、型クラスごとにユニークな ID を割り振ること。
- Unknown 以外の型で辞書経路に入った場合はバグとして扱い、`tests/core_ir_tests.rs` のスナップショットで退行を検知する。

#### Step C-5: テスト方針
| レイヤ | 目的 | 具体的なテストケース / アクション |
| --- | --- | --- |
| Core IR スナップショット | `ValueTy::Unknown` を保持したまま `PrimOp` に `dict_fallback` が付与されること、既知型では辞書に入らないことを固定 | `tests/core_ir_tests.rs` に Unknown を含む `map_binop` / `lower_primop` ケースを追加し、`dict_fallback=true` の IR が生成されるか `insta` で検証する。既知型の ± 演算は `dict_fallback=false` であることを同ファイルで確認する。 |
| ネイティブ E2E（辞書生成 + 実行） | 代表的な辞書 (`Fractional<Double>`, `Eq<Bool>`, 既存の `Num<Int>`) が生成され、インタプリタ結果と一致することを保証する。辞書メタ情報が CLI/JSON にも反映されることをスナップショット化 | `tests/native_build.rs`:<br>• `build_fractional_double_program_matches_interpreter` – `Fractional<Double>` を要求するプログラムの出力比較＋辞書メタチェック。<br>• `emit_module_with_eq_bool_dictionary_runs` – Core IR を直に組み、`Eq<Bool>` 辞書プレースホルダ → `tl_dict_lookup` → `call_indirect` の経路を実行。<br>• `build_basics_example_matches_interpreter` / `cli_prints_dictionary_json` – 既存の `Num` スナップショットを継続監視。 |
| ランタイム ABI | `tl_dict_lookup(dict, method_id)` が `TlValueKind::Pointer` を返し、`tl_value_to_ptr` で実装シンボルを取り出せること。Eq/BoolLogic など複数辞書の API がエラーを返さないこと | `runtime_native/tests/runtime.rs` にて `dictionary_builder_supports_metadata`/`eq_dictionary_exposes_methods`/`bool_logic_dictionary_supports_unary_method` を通じて、メソッド ID の取得およびエラーコード (`TlStatus`) を検証。 |
| CI / 自動化 | すべての辞書経路テストが常に走るようゲートを敷く | `make check`（`cargo fmt`→`cargo clippy -D warnings`→`cargo test --workspace --all-features`）を公式フローとし、PR ではこのコマンドを必須実行とする。 |

#### Step C-6: Cranelift 連携ロードマップ
1. **辞書メタ情報の受け渡し**  
   - `define_functions` で `Parameter::dict_type_repr` を `CodegenEnv::insert_existing` に渡し、`DictionaryParamBinding` を生成。  
   - `NativeBuildArtifacts` に格納した `DictionaryInit` を `declare_dictionary_symbols` で `tl_dict_build_*` シンボルへ変換し、`CodegenEnv::lookup_dictionary` が利用できるようにする。

2. **辞書キャッシュと ABI 呼び出し**  
   - `CodegenEnv::ensure_dictionary` を `(classname, type_repr)` キーのキャッシュとして実装し、`tl_dict_build_*` を 1 度だけ呼ぶ。  
   - `lower_dictionary_placeholder` はキャッシュを介して `Value` を返すだけにし、実際のメソッド呼び出しは `lower_dictionary_primop` で `tl_dict_lookup(dict, method_id)` → `tl_value_to_ptr` → `call_indirect` の順に行う。

3. **Unknown ⇔ 具体型の変換**  
   - `coerce_value` で Int/Double/Bool を `TlValue` にボックス化する経路と、`TlValue` から各プリミティブに戻す経路を双方向にサポート。辞書で取得した関数ポインタを呼び出す直前／直後に必ず通す。  
   - `value_ty_from_repr` で `DictionaryInit::type_repr`（`Int`, `Double`, `Bool` など）を `ValueTy` に復元し、辞書メソッドの ABI を決定する。

4. **PrimOp 落とし込み**  
   - `map_binop` が返す `dict_fallback` フラグを `lower_primop` で参照し、True の場合は `lower_dictionary_primop`、False の場合は従来の `iadd`/`fadd` 等を発行する。  
   - `PrimOp::dictionary_method()` の `method_id` を `ensure_dictionary_method_available` が `DictionaryInit` 上で検証し、未登録なら `CODEGEN212` を返す。

5. **テスト / CI パイプライン**  
   - `tests/core_ir_tests.rs`: Unknown 演算で `dict_fallback=true` の IR が出力されるか、`DictionaryPlaceholder` が差し込まれているかを確認。  
   - `tests/native_build.rs`: `build_fractional_double_program_matches_interpreter`（Fractional<Double>）、`emit_module_with_eq_bool_dictionary_runs`（Eq<Bool>）などで辞書経路の実行を E2E で検証。  
   - `runtime_native/tests/runtime.rs`: `tl_dict_lookup`/`tl_value_to_ptr` の ABI が崩れていないか監視。  
   - CI では `make check` を必須とし、辞書経路を含むすべてのテストを常に実行する。

## 4.1 Core IR: 型クラス制約のモノモーフ化
1. `core_ir::lower::DictionaryEmitter` を導入し、トップレベル `Scheme` と呼び出しサイト双方から `(classname, type_repr)` を収集して `TypeSubst` で具象化する。
2. `DictionaryInit` に `origin` / `source_span` / `scheme_repr` / `method_id` を必須で記録し、未解決インスタンスは `COREIR401`、型不一致は `COREIR402`、重複登録は `COREIR403` を報告する。
3. `FunctionSig` / `Parameter` の `dict_type_repr` を `lower_apply`・`lower_primop` などが参照できるようにし、辞書プレースホルダを IR に残す。
4. `PrimOp::as_dictionary_method()` が `class`, `method`, `signature` を返し、Unknown の場合のみ辞書フォールバックに入る仕様を Core IR 側で保証する。

## 4.2 辞書初期化コードの自動生成
1. `src/codegen/dictionary_codegen.rs` を実装し、`DictionaryInit` から `dict_autogen.rs` を生成する。`tl_dict_builder_*` API で `method_id` / 署名 / 実装シンボル（`tl_value_from_ptr(symbol as *mut c_void)`）を登録するコードを出力する。
2. `runtime_native/build.rs` で生成ファイルを `OUT_DIR` に配置し、`runtime_native/src/lib.rs` から `include!` する。`cargo fmt` / `cargo clippy` を通過するフォーマットとする。
3. 生成した辞書メタ情報を `NativeBuildArtifacts` に格納し、CLI/REPL/Cranelift/テストが同じソースを参照する。

## 4.3 Cranelift 連携
1. **PrimOp ↔ 辞書メソッド**: Step C で定義した対応表を利用し、`ValueTy::Unknown` かつ対応クラスが存在する場合のみ辞書経由へ切り替える。既知型は直接命令を生成する。
2. **メタ情報伝搬**: `Expr::DictionaryPlaceholder` を介して辞書引数が `define_functions` → `CodegenEnv` へ渡るよう IR を拡張し、`NativeBuildArtifacts` の `dict_type_repr` を Codegen が参照できる API を整備する。
3. **辞書キャッシュ**: `CodegenEnv` に `(classname, type_repr)` → `Value` のキャッシュを実装し、関数引数／グローバル初期化の両方から `ensure_dictionary` で取得する。
4. **辞書 ABI**: `tl_dict_lookup(dict, method_id)` → `TlValueKind::Pointer` を `tl_value_to_ptr` で取り出し、得られたシンボル名を `declare_function` → `builder.ins().call` で直接呼び出す。将来的に `call_indirect` へ移行する場合は別途 ABI ドキュメントを追加する。
5. **値変換**: `coerce_value` / `emit_load_tl_value` を拡張し、辞書経路でも `TlValue` ⇔ Cranelift SSA 値の相互変換が行えるようにする。

## 4.4 CLI / REPL の辞書可視化
1. CLI `--print-dictionaries` を辞書生成後の実データに差し替え、`class`, `type`, `origin`, `builder`, `span`, `methods (name/signature/symbol/method_id)` を表示する。`--json` でも同一情報を出力する。
2. REPL `:dictionaries` を実装し、現在ロード済み辞書と、未生成時の CLI 手順を一覧表示する。
3. CLI/REPL で共通フォーマットを使う軽量プリンタモジュールを用意し、重複実装を避ける。

## 4.5 テスト & CI
1. `tests/core_ir_tests.rs`: 辞書モノモーフ化スナップショットを追加し、`Num` 制約が `Num<Int>` 等に展開されること、Unknown 演算が辞書フォールバックになることを固定する。
2. `tests/native_build.rs`:
   - `examples/step2_functions.tl` や `examples/basics.tl` をネイティブ化し、辞書経由でもインタプリタと同じ結果になることを確認。
   - 辞書未初期化時に `CODEGEN202` / `CODEGEN211` を返す失敗ケースを追加。
   - `match` を含む Core IR のネイティブ実行 E2E を追加し、タグ判定と束縛型の整合性を確認する。
3. `runtime_native/tests/`: `tl_dict_lookup` の戻り値が `TlValueKind::Pointer` で取得できるか、複数回取得してもリークしないかを検証する。
4. CI (`make check`): `typelang build examples/basics.tl --print-dictionaries --json` のスナップショット確認を組み込み、辞書出力と Unknown 経路の退行を検出する。
