// パス: tests/ast_display.rs
// 役割: AST表示の各バリアントが期待通り文字列化されるか検証する
// 意図: Expr::fmt 実装の全分岐を回帰テストでカバーする
// 関連ファイル: src/ast.rs, tests/errors.rs, src/parser.rs
use typelang::ast::{Expr, IntBase, Span, TypeExpr};

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
                span: Span::dummy(),
            },
            "ident",
        ),
        (
            Expr::IntLit {
                value: 42,
                base: IntBase::Dec,
                span: Span::dummy(),
            },
            "42",
        ),
        (
            Expr::FloatLit {
                value: 1.5,
                span: Span::dummy(),
            },
            "1.5",
        ),
        (
            Expr::CharLit {
                value: 'x',
                span: Span::dummy(),
            },
            "'x'",
        ),
        (
            Expr::StringLit {
                value: "hello".into(),
                span: Span::dummy(),
            },
            "\"hello\"",
        ),
        (
            Expr::BoolLit {
                value: true,
                span: Span::dummy(),
            },
            "True",
        ),
        (
            Expr::BoolLit {
                value: false,
                span: Span::dummy(),
            },
            "False",
        ),
        (
            Expr::ListLit {
                items: vec![
                    Expr::IntLit {
                        value: 1,
                        base: IntBase::Dec,
                        span: Span::dummy(),
                    },
                    Expr::IntLit {
                        value: 2,
                        base: IntBase::Dec,
                        span: Span::dummy(),
                    },
                ],
                span: Span::dummy(),
            },
            "[1, 2]",
        ),
        (
            Expr::TupleLit {
                items: vec![
                    Expr::IntLit {
                        value: 1,
                        base: IntBase::Dec,
                        span: Span::dummy(),
                    },
                    Expr::BoolLit {
                        value: true,
                        span: Span::dummy(),
                    },
                ],
                span: Span::dummy(),
            },
            "(1, True)",
        ),
        (
            Expr::Lambda {
                params: vec!["x".into(), "y".into()],
                body: Box::new(Expr::Var {
                    name: "y".into(),
                    span: Span::dummy(),
                }),
                span: Span::dummy(),
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
                            span: Span::dummy(),
                        },
                    ),
                    (
                        "f".into(),
                        vec!["a".into()],
                        Expr::Var {
                            name: "a".into(),
                            span: Span::dummy(),
                        },
                    ),
                ],
                body: Box::new(Expr::Var {
                    name: "x".into(),
                    span: Span::dummy(),
                }),
                span: Span::dummy(),
            },
            "let x = 1; f a = a in x",
        ),
        (
            Expr::If {
                cond: Box::new(Expr::BoolLit {
                    value: true,
                    span: Span::dummy(),
                }),
                then_branch: Box::new(Expr::IntLit {
                    value: 1,
                    base: IntBase::Dec,
                    span: Span::dummy(),
                }),
                else_branch: Box::new(Expr::IntLit {
                    value: 0,
                    base: IntBase::Dec,
                    span: Span::dummy(),
                }),
                span: Span::dummy(),
            },
            "if True then 1 else 0",
        ),
        (
            Expr::App {
                func: Box::new(Expr::Var {
                    name: "f".into(),
                    span: Span::dummy(),
                }),
                arg: Box::new(Expr::IntLit {
                    value: 10,
                    base: IntBase::Dec,
                    span: Span::dummy(),
                }),
                span: Span::dummy(),
            },
            "(f 10)",
        ),
        (
            Expr::BinOp {
                op: "+".into(),
                left: Box::new(Expr::IntLit {
                    value: 1,
                    base: IntBase::Dec,
                    span: Span::dummy(),
                }),
                right: Box::new(Expr::IntLit {
                    value: 2,
                    base: IntBase::Dec,
                    span: Span::dummy(),
                }),
                span: Span::dummy(),
            },
            "(1 + 2)",
        ),
        (
            Expr::Annot {
                expr: Box::new(Expr::Var {
                    name: "x".into(),
                    span: Span::dummy(),
                }),
                type_expr: TypeExpr::TECon("Int".into()),
                span: Span::dummy(),
            },
            "(x :: TECon(\"Int\"))",
        ),
    ];

    for (expr, expected) in cases {
        assert_fmt(expr, expected);
    }
}
