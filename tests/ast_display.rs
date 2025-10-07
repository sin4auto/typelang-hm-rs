// パス: tests/ast_display.rs
// 役割: AST表示の各バリアントが期待通り文字列化されるか検証する
// 意図: Expr::fmt 実装の全分岐を回帰テストでカバーする
// 関連ファイル: src/ast.rs, tests/errors.rs, src/parser.rs
use typelang::ast::{Expr, IntBase, TypeExpr};

fn assert_fmt(expr: Expr, expected: &str) {
    assert_eq!(format!("{}", expr), expected);
}

#[test]
/// Expr::Display の主要分岐が文字列化できることを検証する。
fn expr_display_variants_render_expected_strings() {
    let cases: Vec<(Expr, &str)> = vec![
        (
            Expr::Var {
                name: "ident".into(),
            },
            "ident",
        ),
        (
            Expr::IntLit {
                value: 42,
                base: IntBase::Dec,
            },
            "42",
        ),
        (Expr::FloatLit { value: 1.5 }, "1.5"),
        (Expr::CharLit { value: 'x' }, "'x'"),
        (
            Expr::StringLit {
                value: "hello".into(),
            },
            "\"hello\"",
        ),
        (Expr::BoolLit { value: true }, "True"),
        (Expr::BoolLit { value: false }, "False"),
        (
            Expr::ListLit {
                items: vec![
                    Expr::IntLit {
                        value: 1,
                        base: IntBase::Dec,
                    },
                    Expr::IntLit {
                        value: 2,
                        base: IntBase::Dec,
                    },
                ],
            },
            "[1, 2]",
        ),
        (
            Expr::TupleLit {
                items: vec![
                    Expr::IntLit {
                        value: 1,
                        base: IntBase::Dec,
                    },
                    Expr::BoolLit { value: true },
                ],
            },
            "(1, True)",
        ),
        (
            Expr::Lambda {
                params: vec!["x".into(), "y".into()],
                body: Box::new(Expr::Var { name: "y".into() }),
            },
            "\\x y -> y",
        ),
        (
            Expr::LetIn {
                bindings: vec![
                    (
                        "x".into(),
                        vec![],
                        Expr::IntLit {
                            value: 1,
                            base: IntBase::Dec,
                        },
                    ),
                    ("f".into(), vec!["a".into()], Expr::Var { name: "a".into() }),
                ],
                body: Box::new(Expr::Var { name: "x".into() }),
            },
            "let x = 1; f a = a in x",
        ),
        (
            Expr::If {
                cond: Box::new(Expr::BoolLit { value: true }),
                then_branch: Box::new(Expr::IntLit {
                    value: 1,
                    base: IntBase::Dec,
                }),
                else_branch: Box::new(Expr::IntLit {
                    value: 0,
                    base: IntBase::Dec,
                }),
            },
            "if True then 1 else 0",
        ),
        (
            Expr::App {
                func: Box::new(Expr::Var { name: "f".into() }),
                arg: Box::new(Expr::IntLit {
                    value: 10,
                    base: IntBase::Dec,
                }),
            },
            "(f 10)",
        ),
        (
            Expr::BinOp {
                op: "+".into(),
                left: Box::new(Expr::IntLit {
                    value: 1,
                    base: IntBase::Dec,
                }),
                right: Box::new(Expr::IntLit {
                    value: 2,
                    base: IntBase::Dec,
                }),
            },
            "(1 + 2)",
        ),
        (
            Expr::Annot {
                expr: Box::new(Expr::Var { name: "x".into() }),
                type_expr: TypeExpr::TECon("Int".into()),
            },
            "(x :: TECon(\"Int\"))",
        ),
    ];

    for (expr, expected) in cases {
        assert_fmt(expr, expected);
    }
}
