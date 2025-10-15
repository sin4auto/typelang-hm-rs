// パス: tests/lexer_parser.rs
// 役割: Lexer と parser の基本〜境界テストを一本化
// 意図: 字句解析と構文解析の重要ケースをシンプルに網羅する
// 関連ファイル: src/lexer.rs, src/parser.rs, tests/types_infer.rs
#[path = "test_support.rs"]
mod support;

use support::{lex_ok, parse_expr, parse_program};
use typelang::ast::TypeExpr;
use typelang::lexer::{self, TokenKind};
use typelang::parser;

fn assert_token_presence(tokens: &[lexer::Token], kinds: &[TokenKind], note: &str) {
    for kind in kinds {
        assert!(
            tokens.iter().any(|t| &t.kind == kind),
            "{note}: expected token {:?}",
            kind
        );
    }
}

#[test]
/// 代表的な字句パターンをテーブル駆動で検証する。
fn lexer_happy_paths() {
    #[derive(Clone)]
    struct Case<'a> {
        src: &'a str,
        kinds: &'a [TokenKind],
        note: &'a str,
    }

    let cases = [
        Case {
            src: "let x = 0xFF; if True then 10 else 0b101",
            kinds: &[
                TokenKind::LET,
                TokenKind::HEX,
                TokenKind::BIN,
                TokenKind::TRUE,
                TokenKind::ELSE,
            ],
            note: "キーワードと基数付き整数",
        },
        Case {
            src: "-- comment\nlet s = \"a\\n\\\"\"; {- block -}\n",
            kinds: &[TokenKind::STRING, TokenKind::LET],
            note: "コメント + 文字列",
        },
        Case {
            src: "{- outer {- inner -} -} let _tmp = (\\x -> x);",
            kinds: &[TokenKind::LET, TokenKind::UNDERSCORE, TokenKind::LAMBDA],
            note: "入れ子コメントとラムダ・ワイルドカード",
        },
        Case {
            src: "let a = 0x1f; let b = 0o77; let c = 0b1010;",
            kinds: &[TokenKind::HEX, TokenKind::OCT, TokenKind::BIN],
            note: "数値プレフィックス",
        },
        Case {
            src: "let a = 1.0; let b = 1e0; let c = 1.2e-3;",
            kinds: &[TokenKind::FLOAT],
            note: "浮動小数フォーマット",
        },
        Case {
            src: "let c = '\\n'; let q = ?foo;",
            kinds: &[TokenKind::CHAR, TokenKind::QMARK, TokenKind::VARID],
            note: "文字リテラルと ? 識別子",
        },
        Case {
            src: "class Eq a where\ninstance Eq Int\ncase xs of x@[] -> x",
            kinds: &[
                TokenKind::CLASS,
                TokenKind::INSTANCE,
                TokenKind::WHERE,
                TokenKind::AT,
            ],
            note: "class/instance/where/as パターンのトークン",
        },
    ];

    for case in cases {
        let tokens = lex_ok(case.src);
        assert_token_presence(&tokens, case.kinds, case.note);
    }
}

#[test]
/// 誤った入力がエラーになることを検証する。
fn lexer_error_paths() {
    for src in [
        "let x = 0b;",
        "let y = 0o;",
        "let z = 0x;",
        "\"abc",
        "{- never closed",
    ] {
        assert!(
            lexer::lex(src).is_err(),
            "期待通り字句エラーになりません: {src}"
        );
    }
}

#[test]
/// Unicode や非 ASCII 境界の扱いを検証する。
fn lexer_unicode_handling() {
    assert!(lexer::lex("let x = 1 -ー 2").is_err());
    assert!(lexer::lex(r#"let f = \\x -> 'あ'"#).is_ok());
}

#[test]
/// 各種式が期待通りにパースされ文字列化できることを検証する。
fn parser_expr_round_trips() {
    struct ExprCase<'a> {
        src: &'a str,
        fragments: &'a [&'a str],
        exact: Option<&'a str>,
        note: &'a str,
    }

    let cases = [
        ExprCase {
            src: "2 ^ 3 ^ 2",
            fragments: &["^"],
            exact: None,
            note: "累乗は右結合",
        },
        ExprCase {
            src: "-1",
            fragments: &["- 1"],
            exact: None,
            note: "単項マイナスの糖衣展開",
        },
        ExprCase {
            src: "f 2 ^ 3 * 4 + 5",
            fragments: &["f 2", "^", "*", "+"],
            exact: None,
            note: "関数適用と中置演算子の優先順位",
        },
        ExprCase {
            src: "let a = 1; b x = x in b a",
            fragments: &["let", "in"],
            exact: None,
            note: "let-in の複数束縛",
        },
        ExprCase {
            src: "?x",
            fragments: &[],
            exact: Some("?x"),
            note: "? 識別子の保持",
        },
        ExprCase {
            src: "case n of x | x > 0 -> x; _ -> 0",
            fragments: &["|"],
            exact: None,
            note: "case ガード構文",
        },
        ExprCase {
            src: "case xs of ys@[] -> ys; _ -> []",
            fragments: &["@[]"],
            exact: None,
            note: "as パターン",
        },
        ExprCase {
            src: "case [1,2] of [a, b] -> a + b; _ -> 0",
            fragments: &["[a, b]"],
            exact: None,
            note: "リストリテラルパターン",
        },
    ];

    for case in cases {
        let expr = parse_expr(case.src);
        let rendered = format!("{}", expr);
        if let Some(expected) = case.exact {
            assert_eq!(rendered, expected, "{}", case.note);
        }
        for fragment in case.fragments {
            assert!(
                rendered.contains(fragment),
                "{}: missing fragment `{fragment}`",
                case.note
            );
        }
    }
}

#[test]
/// プログラム単位のパースとシグネチャ解釈を検証する。
fn parser_program_cases() {
    let plain = parse_program("foo :: Int -> Int\nlet foo x = x");
    assert_eq!(plain.decls.len(), 1);
    let sig = plain.decls[0]
        .signature
        .as_ref()
        .expect("signature should exist");
    assert!(sig.constraints.is_empty(), "制約なしシグネチャの保持");

    let constrained = parse_program("bar :: Num a => a -> a\nlet bar x = x");
    let sig = constrained.decls[0]
        .signature
        .as_ref()
        .expect("signature should exist");
    assert_eq!(sig.constraints.len(), 1);
    assert_eq!(sig.constraints[0].classname, "Num");

    let long_string = parse_program(&format!("let s = \"{}\";", "a".repeat(5000)));
    assert_eq!(long_string.decls.len(), 1, "長い文字列の解析");

    let class_prog = parse_program("class (Eq a) => Fancy a\ninstance Fancy Int\n");
    assert_eq!(class_prog.class_decls.len(), 1, "class 宣言のパース");
    let class_decl = &class_prog.class_decls[0];
    assert_eq!(class_decl.name, "Fancy");
    assert_eq!(class_decl.superclasses, vec!["Eq".to_string()]);
    assert_eq!(class_prog.instance_decls.len(), 1, "instance 宣言のパース");
    let instance_decl = &class_prog.instance_decls[0];
    assert_eq!(instance_decl.classname, "Fancy");
    assert_eq!(instance_decl.tycon, "Int");
}

#[test]
/// 余剰セミコロンや複合的な data 宣言を正しく処理できることを検証する。
fn parser_program_handles_semicolons_and_data_decl() {
    let src = r#";;;  
data Pair a b = Pair Int a b | MkPair (a, b)
data App f a = App (f a)
let id x = x
"#;
    let program = parse_program(src);
    assert_eq!(program.data_decls.len(), 2, "data 宣言を 2 件取得");

    let pair_decl = &program.data_decls[0];
    assert_eq!(pair_decl.name, "Pair");
    assert_eq!(pair_decl.params, vec!["a", "b"]);
    assert_eq!(pair_decl.constructors.len(), 2, "コンストラクタ数");

    let pair_ctor = &pair_decl.constructors[0];
    assert_eq!(pair_ctor.name, "Pair");
    assert_eq!(
        pair_ctor.args.len(),
        1,
        "Pair の引数型表現は 1 つに畳み込まれる"
    );
    if let TypeExpr::TEApp(lhs, rhs) = &pair_ctor.args[0] {
        assert!(matches!(**rhs, TypeExpr::TEVar(ref name) if name == "b"));
        if let TypeExpr::TEApp(inner_lhs, inner_rhs) = &**lhs {
            assert!(matches!(**inner_rhs, TypeExpr::TEVar(ref name) if name == "a"));
            assert!(matches!(**inner_lhs, TypeExpr::TECon(ref name) if name == "Int"));
        } else {
            panic!("期待したネストした型適用ではありません: {lhs:?}");
        }
    } else {
        panic!("期待した型適用ではありません: {:?}", pair_ctor.args[0]);
    }

    let tuple_ctor = &pair_decl.constructors[1];
    assert_eq!(tuple_ctor.name, "MkPair");
    assert!(matches!(tuple_ctor.args[0], TypeExpr::TETuple(ref items) if items.len() == 2));

    let app_decl = &program.data_decls[1];
    assert_eq!(app_decl.name, "App");
    let app_ctor = &app_decl.constructors[0];
    match &app_ctor.args[0] {
        TypeExpr::TEApp(lhs, rhs) => {
            assert!(matches!(**lhs, TypeExpr::TEVar(_)));
            assert!(matches!(**rhs, TypeExpr::TEVar(_)));
        }
        other => panic!("期待した型適用ではありません: {other:?}"),
    }

    assert_eq!(program.decls.len(), 1, "トップレベル宣言数");
    assert_eq!(program.decls[0].name, "id");
}

#[test]
/// class / instance 宣言のバリエーションと制約を網羅的に検証する。
fn parser_program_handles_class_and_instance_variants() {
    let src = r#"
class (Eq a, Show a) => Pretty a
class Marker
instance Pretty []
instance Pretty Int
"#;
    let program = parse_program(src);

    assert_eq!(program.class_decls.len(), 2, "class 宣言数");
    let pretty = &program.class_decls[0];
    assert_eq!(pretty.name, "Pretty");
    assert_eq!(
        pretty.superclasses,
        vec!["Eq".to_string(), "Show".to_string()]
    );
    assert_eq!(pretty.typevar.as_deref(), Some("a"));

    let marker = &program.class_decls[1];
    assert_eq!(marker.name, "Marker");
    assert!(marker.typevar.is_none(), "typevar が省略されたケース");

    assert_eq!(program.instance_decls.len(), 2, "instance 宣言数");
    assert!(
        program.instance_decls.iter().any(|inst| inst.tycon == "[]"),
        "[] の特殊ケースをパース"
    );
    assert!(program
        .instance_decls
        .iter()
        .any(|inst| inst.tycon == "Int" && inst.classname == "Pretty"));
}

#[test]
/// class / instance 宣言で想定外構文が出た際に適切なエラーを返すことを確認する。
fn parser_program_reports_class_instance_errors() {
    let err = parser::parse_program("class Eq a where\n").expect_err("class where をエラーにする");
    let rendered = err.to_string();
    assert!(rendered.contains("[PAR510]"));

    let err = parser::parse_program("instance Eq maybe\n").expect_err("小文字型を弾く");
    let rendered = err.to_string();
    assert!(rendered.contains("[PAR511]"));

    let err = parser::parse_program("instance Eq Int where\n")
        .expect_err("instance where をエラーにする");
    let rendered = err.to_string();
    assert!(rendered.contains("[PAR512]"));
}

#[test]
/// 不正な構文が適切に弾かれることを検証する。
fn parser_error_cases() {
    assert!(parser::parse_expr("[1,2").is_err());
    assert!(parser::parse_expr("if True then 1").is_err());

    let big = "9".repeat(50);
    let src = format!("let x = {};", big);
    let err = parser::parse_program(&src).expect_err("expect parse error for huge int");
    let rendered = err.to_string();
    assert!(rendered.contains("[PAR210]"));
    assert!(rendered.contains("範囲外"));
}

#[test]
/// 深い括弧ネストでもパースできることを確認する。
fn parser_handles_deep_parentheses() {
    let depth = 64;
    let mut src = String::new();
    for _ in 0..depth {
        src.push('(');
    }
    src.push('1');
    for _ in 0..depth {
        src.push(')');
    }
    let expr = parse_expr(&src);
    assert!(format!("{}", expr).contains('1'));
}
