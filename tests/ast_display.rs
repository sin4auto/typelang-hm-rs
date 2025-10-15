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
        (
            Expr::Case {
                scrutinee: Box::new(Expr::Var {
                    name: "value".into(),
                    span: Span::new(10, 1, 11),
                }),
                arms: vec![
                    typelang::ast::CaseArm {
                        pattern: typelang::ast::Pattern::Constructor {
                            name: "Just".into(),
                            args: vec![typelang::ast::Pattern::Var {
                                name: "x".into(),
                                span: Span::new(11, 1, 12),
                            }],
                            span: Span::new(10, 1, 11),
                        },
                        guard: None,
                        body: Expr::Var {
                            name: "x".into(),
                            span: Span::new(11, 1, 12),
                        },
                    },
                    typelang::ast::CaseArm {
                        pattern: typelang::ast::Pattern::Wildcard {
                            span: Span::new(12, 1, 13),
                        },
                        guard: None,
                        body: Expr::IntLit {
                            value: 0,
                            base: IntBase::Dec,
                            span: Span::new(12, 1, 13),
                        },
                    },
                ],
                span: Span::dummy(),
            },
            "case value of Just x -> x; _ -> 0",
        ),
    ];

    for (expr, expected) in cases {
        assert_fmt(expr, expected);
    }
}

#[test]
/// Pattern::Display と span アクセサを広く網羅する。
fn pattern_display_and_span_metadata() {
    use typelang::ast::Pattern;

    let wildcard = Pattern::Wildcard {
        span: Span::new(1, 1, 1),
    };
    assert_eq!(format!("{}", wildcard), "_");
    assert_eq!(wildcard.span(), Span::new(1, 1, 1));

    let constructor = Pattern::Constructor {
        name: "Pair".into(),
        args: vec![
            Pattern::Int {
                value: 1,
                base: IntBase::Dec,
                span: Span::new(2, 1, 2),
            },
            Pattern::Var {
                name: "y".into(),
                span: Span::new(3, 1, 3),
            },
        ],
        span: Span::new(2, 1, 2),
    };
    assert_eq!(format!("{}", constructor), "Pair 1 y");
    assert_eq!(constructor.span(), Span::new(2, 1, 2));

    let nullary = Pattern::Constructor {
        name: "None".into(),
        args: Vec::new(),
        span: Span::new(7, 1, 7),
    };
    assert_eq!(format!("{}", nullary), "None");
    assert_eq!(nullary.span(), Span::new(7, 1, 7));

    let bool_false = Pattern::Bool {
        value: false,
        span: Span::new(8, 1, 8),
    };
    assert_eq!(format!("{}", bool_false), "False");
    assert_eq!(bool_false.span(), Span::new(8, 1, 8));

    let float_pat = Pattern::Float {
        value: 1.5,
        span: Span::new(9, 1, 9),
    };
    assert_eq!(format!("{}", float_pat), "1.5");
    assert_eq!(float_pat.span(), Span::new(9, 1, 9));

    let string_pat = Pattern::String {
        value: "ok".into(),
        span: Span::new(10, 1, 10),
    };
    assert_eq!(format!("{}", string_pat), "\"ok\"");
    assert_eq!(string_pat.span(), Span::new(10, 1, 10));

    let list_pat = Pattern::List {
        items: vec![Pattern::Wildcard {
            span: Span::new(11, 1, 11),
        }],
        span: Span::new(11, 1, 11),
    };
    assert_eq!(format!("{}", list_pat), "[_]");
    assert_eq!(list_pat.span(), Span::new(11, 1, 11));

    let tuple_pat = Pattern::Tuple {
        items: vec![
            Pattern::Int {
                value: 1,
                base: IntBase::Dec,
                span: Span::new(12, 1, 12),
            },
            Pattern::Bool {
                value: true,
                span: Span::new(13, 1, 13),
            },
        ],
        span: Span::new(12, 1, 12),
    };
    assert_eq!(format!("{}", tuple_pat), "(1, True)");
    assert_eq!(tuple_pat.span(), Span::new(12, 1, 12));

    let as_pat = Pattern::As {
        binder: "xs".into(),
        pattern: Box::new(Pattern::List {
            items: vec![Pattern::Wildcard {
                span: Span::new(14, 1, 14),
            }],
            span: Span::new(14, 1, 14),
        }),
        span: Span::new(14, 1, 14),
    };
    assert_eq!(format!("{}", as_pat), "xs@[_]");
    assert_eq!(as_pat.span(), Span::new(14, 1, 14));

    let expr = Expr::If {
        cond: Box::new(Expr::BoolLit {
            value: true,
            span: Span::new(4, 1, 4),
        }),
        then_branch: Box::new(Expr::Var {
            name: "x".into(),
            span: Span::new(5, 1, 5),
        }),
        else_branch: Box::new(Expr::Var {
            name: "y".into(),
            span: Span::new(6, 1, 6),
        }),
        span: Span::new(4, 1, 4),
    };
    assert_eq!(expr.span(), Span::new(4, 1, 4));
}
