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
    assert_fmt(
        Expr::Var {
            name: "ident".into(),
        },
        "ident",
    );
    assert_fmt(
        Expr::IntLit {
            value: 42,
            base: IntBase::Dec,
        },
        "42",
    );
    assert_fmt(Expr::FloatLit { value: 1.5 }, "1.5");
    assert_fmt(Expr::CharLit { value: 'x' }, "'x'");
    assert_fmt(
        Expr::StringLit {
            value: "hello".into(),
        },
        "\"hello\"",
    );
    assert_fmt(Expr::BoolLit { value: true }, "True");
    assert_fmt(Expr::BoolLit { value: false }, "False");
    assert_fmt(
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
    );
    assert_fmt(
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
    );
    assert_fmt(
        Expr::Lambda {
            params: vec!["x".into(), "y".into()],
            body: Box::new(Expr::Var { name: "y".into() }),
        },
        "\\x y -> y",
    );
    assert_fmt(
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
    );
    assert_fmt(
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
    );
    assert_fmt(
        Expr::App {
            func: Box::new(Expr::Var { name: "f".into() }),
            arg: Box::new(Expr::IntLit {
                value: 10,
                base: IntBase::Dec,
            }),
        },
        "(f 10)",
    );
    assert_fmt(
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
    );
    assert_fmt(
        Expr::Annot {
            expr: Box::new(Expr::Var { name: "x".into() }),
            type_expr: TypeExpr::TECon("Int".into()),
        },
        "(x :: TECon(\"Int\"))",
    );
}
