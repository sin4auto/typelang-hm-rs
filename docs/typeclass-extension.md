# 型クラス拡張仕様ドラフト（Functor / Foldable）

## 背景
TypeLang HM は Hindley–Milner 型推論の上に最小限の型クラス (`Eq`/`Ord`/`Show`/`Num`/`Fractional`) を提供しており、制約は静的に `ClassEnv` に登録されたインスタンスでのみ解決される。コレクション操作を中心とした抽象度の高い API は未提供であり、ユーザーはリスト操作を個別実装する必要がある。

## 目的
- 高階関数をより簡潔に再利用できる抽象化レイヤーを導入する。
- 既存の HM ベース実装を大きく崩さず、ビルトイン型に限定した `Functor` / `Foldable` サポートを追加する。
- 将来的な型クラス拡充（`Traversable` や自前型へのインスタンス拡張）に備え、クラス環境と推論器の拡張余地を整理する。

## スコープ
1. **型クラスの追加**: `Functor` と `Foldable` を追加し、現状は `[]`（リスト）にのみインスタンスを提供する。
2. **組み込み関数の追加**: 以下の高階関数を標準環境へ登録する。
   - `map :: Functor f => (a -> b) -> f a -> f b`
   - `foldl :: Foldable t => (b -> a -> b) -> b -> t a -> b`
   - `foldr :: Foldable t => (a -> b -> b) -> b -> t a -> b`
3. **推論系の調整**: 既存の制約処理を保ちつつ、`Functor f` など型コンストラクタを受け取る制約を正しく扱えるよう `ClassEnv::entails_one` を拡張する。
4. **表示・エラーメッセージ**: `pretty_qual`・エラー表示で新制約が自然なフォーマットになることを確認する。

## 仕様詳細
### 1. 型クラス階層
- `Functor` はスーパークラスを持たない新規クラス。
- `Foldable` もスーパークラスを持たないが、内部実装では `Foldable` の制約解決時に `Functor` との整合性チェックは行わない（将来的な `Traversable` 拡張時に検討）。
- 既存クラスとの依存は追加しないため、既存の型推論結果に影響しない。

### 2. インスタンス
- `Functor []`
- `Foldable []`
- 今後の拡張余地として、`Tuple2`（`(,) a`）や `Maybe` 等の型コンストラクタに対応する場合は `ClassEnv::instances` に追加で登録すればよい設計とする。

### 3. 推論器への影響
- `Constraint` の `r#type` に `Type::TVar` や `Type::TApp` が入る想定は従来から存在するため、新クラスの導入に伴うデータ構造変更は不要。
- `ClassEnv::entails_one` を以下の方針で拡張する:
  - `Type::TVar` が現れた場合は具体化されていないため解決不可として `false` を返す（従来挙動と一致）。
  - `Type::TApp` の場合、関数部分が `Type::TCon` であれば従来どおり `(classname, tycon)` の登録で判定する。
  - 追加で、`Type::TApp` の関数部分が `Type::TVar` の場合は、引数を含む制約を再帰的に処理せず、そのまま `false` を返す（未解決 constraint として保持）。
  - これにより `map` が `[]` に適用された場合、統一処理で `f` が `[]` に束縛され、制約 `Functor []` は既存登録から解決できる。
- `infer::initial_class_env` にクラスとインスタンスを追加する際、`ClassEnv::classes` に `Functor` / `Foldable` を登録し、`instances` に `[]` を追加する。

### 4. 環境初期化
- `infer::initial_env` に以下のスキームを追加する。
  ```
  map   :: Functor f => (a -> b) -> f a -> f b
  foldl :: Foldable t => (b -> a -> b) -> b -> t a -> b
  foldr :: Foldable t => (a -> b -> b) -> b -> t a -> b
  ```
  - 型変数 `f`/`t` は `Type::TVar`、`f a` / `t a` は `Type::TApp` を用いて表現する。
  - 制約リストは `Constraint { classname: "Functor", r#type: f_type }` のように生成する。
- 既存の `TypeEnv::extend` を利用し、`map` などを初期環境に登録する。

### 5. 評価器
- `evaluator::initial_env` に以下を追加する。
  - `map` は `Value::Prim2` を再利用し、`Prim2`/`Prim1` を組み合わせて 2 引数（関数とコンテナ）をカリー化で実装する。リストに対してのみ動作し、他の値が来た場合は `EVAL050` を返す。
  - `foldl` / `foldr` は 3 引数の関数であるため、`Prim2` だけでは足りない。`Prim2` の代わりに `Prim1` を段階的に返すラッパーを用意する、または新しい `Prim3` を導入する。実装コストを抑えるため、今回は「第一引数（関数）を受け取ったら `Prim2` を返す」クロージャ戦略を採用する。
  - リスト以外が渡された場合は型クラス制約に沿ってエラーを返す。

### 6. 表示・REPL
- `pretty_qual` で `Functor f` / `Foldable t` が表示されることを確認する。既存実装でも `constraint.classname` と型の組み合わせを文字列化するため追加作業は不要。
- REPL `:t` で新関数の型が期待どおりに出力されることをテストで保証する。

### 7. 互換性
- 既存の `Num`/`Eq` などの制約には影響しない。新規シンボルとして `map`/`foldl`/`foldr` を導入するが、既存ユーザー定義バインディングで同名を定義済みの場合は従来どおり後勝ち（ユーザー側が上書き可能）。

## 実装タスク（概算）
1. `ClassEnv` 拡張と `initial_class_env` の更新。
2. `initial_env` に新規スキームを追加。
3. `evaluator::initial_env` に対応する実装を追加。
4. テスト
   - `tests/infer.rs` 相当の場所へ `:t map` などの期待型を追加。
   - `tests/evaluator.rs`（存在しない場合は新設）で `map`/`foldl`/`foldr` の動作を確認。
5. `README.md` と `EBNF.md` の更新（型クラス一覧、標準関数一覧）。

## 今後の検討事項
- `Functor` の一般化（リスト以外へのインスタンス）には型コンストラクタに対するメタ情報が必要。`ClassEnv` へ arity 情報を持たせる案を次フェーズで検討する。
- `Foldable` の `foldMap` など追加メソッド、`Traversable` 拡張時の方針整理。
- ユーザー定義型へのインスタンス追加は現状サポート外。構文拡張を伴うため別タスクとする。


## テスト戦略（最小ケース案）
1. **型推論テスト**
   - `:t map` → `Functor f => (a -> b) -> f a -> f b`
   - `:t map (\x -> x + 1)` → `Functor f, Num a => f a -> f a` のように `Functor` と `Num` が複合制約として残ることを確認する。
   - `map (\x -> x + 1) [1,2,3]` の推論結果が `[Integer]` と一致すること（既定化後の表示も含む）。
2. **評価器テスト**
   - `map (\x -> x + 1) [1,2,3]` → `[2,3,4]`
   - `foldl (\acc x -> acc + x) 0 [1,2,3,4]` → `10`
   - `foldr (\x acc -> x : acc) [] [1,2,3]` → `[1,2,3]`
   - エラーケース: `map (\x -> x) 42` が `EVAL050` を返す。
3. **リグレッション**
   - 既存の `make check` に含まれる推論・評価テストがすべて通ること。
   - REPL `:load` で `map` 等を利用する簡易例が動作すること（必要に応じて `examples/` を追加）。

