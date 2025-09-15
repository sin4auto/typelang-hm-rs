# TypeLang（Rust）EBNF

実装に沿った最小の EBNF 仕様です。レイアウト規則は採用せず、セミコロンなどの明示トークンで区切ります。

```
(* 1. Lexical *)
letter      = 'A'..'Z' | 'a'..'z' | '_' ;
digit       = '0'..'9' ;
hexdigit    = digit | 'A'..'F' | 'a'..'f' ;
octdigit    = '0'..'7' ;
bindigit    = '0' | '1' ;

whitespace  = { ' ' | '\t' | '\r' | '\n' } ;
line_comment= '-' '-' { ~'\n' } ;
block_comment= '{' '-' { any } '-' '}' ;

reserved    = 'let' | 'in' | 'if' | 'then' | 'else' | 'True' | 'False' ;
varid       = ( 'a'..'z' | '_' ) { letter | digit | '\'' } - reserved ;
conid       = ( 'A'..'Z' ) { letter | digit | '\'' } ;

int_lit     = '0x' hexdigit+ | '0o' octdigit+ | '0b' bindigit+ | digit+ ;
float_lit   = digit+ '.' digit+ [ exp ] | digit+ exp ;
exp         = ( 'e' | 'E' ) [ '+' | '-' ] digit+ ;
char_lit    = '\'' ( escape | ~('\\'|'\n'|'\'') ) '\'' ;
string_lit  = '"' { escape | ~('\\'|'\n'|'"') } '"' ;
escape      = '\\' ( '\\' | '\'' | '"' | 'n' | 'r' | 't' ) ;

(* 2. Program *)
program     = { toplevel } ;
toplevel    = [ type_sig ] 'let' fun_bind [ ';' ] ;
fun_bind    = varid { varid } '=' expr ;

(* 3. Types *)
type_sig    = varid '::' sigma ;
sigma       = [ context '=>' ] type ;
context     = constraint | '(' constraint { ',' constraint } ')' ;
constraint  = conid varid ;

type        = type_app [ '->' type ] ;            (* right-assoc *)
type_app    = type_atom { type_atom } ;           (* left-assoc  *)
type_atom   = varid | conid | '[' type ']' | '(' type ')' | '(' type ',' type { ',' type } ')' ;

(* 4. Expressions *)
expr        = ( lam | let_in | ifte | cmp ) [ '::' type ] ;
lam         = '\\' varid { varid } '->' expr ;
let_in      = 'let' binds 'in' expr ;
binds       = bind { ';' bind } ;
bind        = varid { varid } '=' expr ;
ifte        = 'if' expr 'then' expr 'else' expr ;

cmp         = add [ ( '==' | '/=' | '<' | '<=' | '>' | '>=' ) add ] ; (* non-assoc *)
add         = mul { ( '+' | '-' ) mul } ;                             (* left     *)
mul         = pow { ( '*' | '/' ) pow } ;                             (* left     *)
pow         = app [ ( '^' | '**' ) pow ] ;                            (* right    *)
app         = atom { atom } ;                                         (* left     *)

atom        = int_lit | float_lit | char_lit | string_lit
            | varid | '_' | '?' varid
            | '(' expr ')' | '[' [ expr { ',' expr } ] ']' | '(' expr ',' expr { ',' expr } ')' ;

(* 5. Notes *)
- '^' は整数指数。負指数は Double にフォールバック
- '**' は連続値指数（常に Double）
- 文字列は `[Char]` と同一視
- 入力は UTF-8。2 文字演算子/コメント終端 "-}" はコードポイント境界で検出
```
