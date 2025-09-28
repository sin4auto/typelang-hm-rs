<!-- パス: EBNF.md -->
<!-- 役割: TypeLang HM の正式な EBNF 仕様を提示するリファレンス -->
<!-- 意図: 実装・テスト・教材で共有する唯一の文法定義を提供する -->
<!-- 関連ファイル: README.md, src/parser.rs, src/lexer.rs -->

# TypeLang HM EBNF リファレンス

本書は TypeLang HM（Rust 実装版）の正準文法です。`src/lexer.rs` および `src/parser.rs` に変更が入る際は、必ずこの EBNF を先に更新し、後続実装とテストを同期させてください。

---

## 0. 表記ルール
- `(* ... *)` はコメント、`{ X }` は 0 回以上、`[ X ]` は任意（0 または 1 回）を表します。
- 二重引用符 `"` と単一引用符 `'` はリテラルの境界に使用します。バックスラッシュでエスケープしてください。
- レイアウト（空白や改行）は構文に影響しません。区切りは明示トークンで表します。

## 1. 字句レベル
```
letter        = 'A'..'Z' | 'a'..'z' | '_' ;
digit         = '0'..'9' ;
hexdigit      = digit | 'A'..'F' | 'a'..'f' ;
octdigit      = '0'..'7' ;
bindigit      = '0' | '1' ;

whitespace    = { ' ' | '\t' | '\r' | '\n' } ;
line_comment  = '-' '-' { ~'\n' } ;
block_comment = '{' '-' { any } '-' '}' ;

reserved      = 'let' | 'in' | 'if' | 'then' | 'else' | 'True' | 'False' ;
varid         = ( 'a'..'z' | '_' ) { letter | digit | '\'' } - reserved ;
conid         = ( 'A'..'Z' ) { letter | digit | '\'' } ;

int_lit       = '0x' hexdigit+ | '0o' octdigit+ | '0b' bindigit+ | digit+ ;
float_lit     = digit+ '.' digit+ [ exp ] | digit+ exp ;
exp           = ( 'e' | 'E' ) [ '+' | '-' ] digit+ ;
char_lit      = '\'' ( escape | ~('\\'|'\n'|'\'') ) '\'' ;
string_lit    = '"' { escape | ~('\\'|'\n'|'"') } '"' ;
escape        = '\\' ( '\\' | '\'' | '"' | 'n' | 'r' | 't' ) ;
```

### 補足
- コメントは入れ子可能です。`-}` の検出は UTF-8 コードポイント単位で行います。
- 数値リテラルは先頭接頭辞で基数を判別します。先頭 `0` のみでは 10 進として扱います。

## 2. トップレベル構造
```
program    = { toplevel } ;
toplevel   = [ type_sig ] 'let' fun_bind [ ';' ] ;
fun_bind   = varid { varid } '=' expr ;
```

## 3. 型
```
type_sig   = varid '::' sigma ;
sigma      = [ context '=>' ] type ;
context    = constraint | '(' constraint { ',' constraint } ')' ;
constraint = conid varid ;

type       = type_app [ '->' type ] ;           (* 右結合 *)
type_app   = type_atom { type_atom } ;          (* 左結合 *)
type_atom  = varid | conid | '[' type ']' | '(' type ')' | '(' type ',' type { ',' type } ')' ;
```

## 4. 式
```
expr       = ( lam | let_in | ifte | cmp ) [ '::' type ] ;
lam        = '\\' varid { varid } '->' expr ;
let_in     = 'let' binds 'in' expr ;
binds      = bind { ';' bind } ;
bind       = varid { varid } '=' expr ;
ifte       = 'if' expr 'then' expr 'else' expr ;

cmp        = add [ ( '==' | '/=' | '<' | '<=' | '>' | '>=' ) add ] ; (* 非結合 *)
add        = mul { ( '+' | '-' ) mul } ;                             (* 左結合 *)
mul        = pow { ( '*' | '/' ) pow } ;                             (* 左結合 *)
pow        = app [ ( '^' | '**' ) pow ] ;                            (* 右結合 *)
app        = atom { atom } ;                                         (* 左結合 *)

atom       = int_lit | float_lit | char_lit | string_lit
           | varid | '_' | '?' varid
           | '(' expr ')' | '[' [ expr { ',' expr } ] ']' | '(' expr ',' expr { ',' expr } ')' ;
```

## 5. 運用メモ
- `^` は整数指数専用。負指数を検出した場合は `Double` 型に自動昇格します。
- `**` は常に連続値指数で評価し、結果は `Double` になります。
- 文字列リテラルとリスト表記 `[Char]` は等価として扱います。
- 文法を変更する場合は、関連テスト（`tests/`）と REPL 表示も同時に更新してください。

この文法が唯一の真実のソースです。差異を見つけた場合は、まず本ファイルを更新した上で実装とドキュメントを揃えてください。
