// パス: tests/evaluator.rs
// 役割: 評価器の正常系と代表的な失敗ケースを最小構成で検証
// 意図: 数値演算・比較・pow・show の挙動が回帰しないようにする
// 関連ファイル: src/evaluator.rs, src/parser.rs, tests/types_infer.rs
#[path = "test_support.rs"]
mod support;

use support::{approx_eq, eval_result, eval_value};
use typelang::ast::{Expr, IntBase, Span};
use typelang::errors::EvalError;
use typelang::evaluator::{self, Value};

#[derive(Clone, Copy)]
struct EvalCase {
    expr: &'static str,
    expect: Expect,
    note: &'static str,
}

#[derive(Clone, Copy)]
enum Expect {
    Bool(bool),
    Int(i64),
    Double(f64),
    String(&'static str),
    Error(&'static str),
}

fn verify_case(case: &EvalCase) {
    match case.expect {
        Expect::Bool(expected) => match eval_value(case.expr) {
            Value::Bool(actual) => assert_eq!(actual, expected, "{}", case.note),
            other => panic!(
                "{}: expected Bool({expected}), got {:?} for {:?}",
                case.note, other, case.expr
            ),
        },
        Expect::Int(expected) => match eval_value(case.expr) {
            Value::Int(actual) => assert_eq!(actual, expected, "{}", case.note),
            other => panic!(
                "{}: expected Int({expected}), got {:?} for {:?}",
                case.note, other, case.expr
            ),
        },
        Expect::Double(expected) => match eval_value(case.expr) {
            Value::Double(actual) => assert!(
                approx_eq(actual, expected),
                "{}: expected ≈ {expected}, got {actual} for {:?}",
                case.note,
                case.expr
            ),
            other => panic!(
                "{}: expected Double({expected}), got {:?} for {:?}",
                case.note, other, case.expr
            ),
        },
        Expect::String(expected) => match eval_value(case.expr) {
            Value::String(actual) => assert_eq!(actual, expected, "{}", case.note),
            other => panic!(
                "{}: expected String({expected}), got {:?} for {:?}",
                case.note, other, case.expr
            ),
        },
        Expect::Error(expected_code) => match eval_result(case.expr) {
            Err(EvalError(info)) => assert_eq!(
                info.code, expected_code,
                "{}: unexpected code for {:?}",
                case.note, case.expr
            ),
            Ok(value) => panic!(
                "{}: expected error {}, got value {:?} for {:?}",
                case.note, expected_code, value, case.expr
            ),
        },
    }
}

#[test]
/// 評価器の代表ケースをテーブルドリブンで検証する。
fn evaluator_smoke_suite() {
    let cases = [
        EvalCase {
            expr: "1 == 1",
            expect: Expect::Bool(true),
            note: "整数の等価比較",
        },
        EvalCase {
            expr: "2 < 1",
            expect: Expect::Bool(false),
            note: "整数の大小比較",
        },
        EvalCase {
            expr: "(1,2) == (1,2)",
            expect: Expect::Bool(true),
            note: "タプルの構造比較",
        },
        EvalCase {
            expr: "[1,2] < [1,3]",
            expect: Expect::Bool(true),
            note: "リストの辞書順比較",
        },
        EvalCase {
            expr: "(1,3) < (1,4)",
            expect: Expect::Bool(true),
            note: "タプルの辞書順比較",
        },
        EvalCase {
            expr: "1 / 2",
            expect: Expect::Double(0.5),
            note: "除算で Double に昇格",
        },
        EvalCase {
            expr: "2 ^ 3 ^ 2",
            expect: Expect::Int(512),
            note: "右結合の累乗",
        },
        EvalCase {
            expr: "2 ^ -1",
            expect: Expect::Double(0.5),
            note: "負指数の累乗で Double",
        },
        EvalCase {
            expr: "(-2) ^ 3",
            expect: Expect::Int(-8),
            note: "負数累乗の奇数指数",
        },
        EvalCase {
            expr: "2 ** -1",
            expect: Expect::Double(0.5),
            note: "powf の負指数",
        },
        EvalCase {
            expr: "div 7 3",
            expect: Expect::Int(2),
            note: "整数 Euclid 除算",
        },
        EvalCase {
            expr: "mod 7 3",
            expect: Expect::Int(1),
            note: "整数 Euclid 剰余",
        },
        EvalCase {
            expr: "div (-7) 3",
            expect: Expect::Int(-3),
            note: "負値の Euclid 除算",
        },
        EvalCase {
            expr: "mod (-7) 3",
            expect: Expect::Int(2),
            note: "負値の Euclid 剰余",
        },
        EvalCase {
            expr: "quot (-7) 3",
            expect: Expect::Int(-2),
            note: "quot のゼロ方向切り捨て",
        },
        EvalCase {
            expr: "rem (-7) 3",
            expect: Expect::Int(-1),
            note: "rem は被除数と同符号",
        },
        EvalCase {
            expr: "(0.0 / 0.0) == (0.0 / 0.0)",
            expect: Expect::Bool(false),
            note: "NaN 同士の比較は false",
        },
        EvalCase {
            expr: "show 42",
            expect: Expect::String("42"),
            note: "show が文字列を返す",
        },
        EvalCase {
            expr: "case True of True -> 1; False -> 0",
            expect: Expect::Int(1),
            note: "case 式で Bool を分岐",
        },
        EvalCase {
            expr: "case 42 of x -> x",
            expect: Expect::Int(42),
            note: "case 変数束縛",
        },
        EvalCase {
            expr: "case [1,2,3] of [x, _, z] -> x + z; _ -> 0",
            expect: Expect::Int(4),
            note: "リストパターンで要素抽出",
        },
        EvalCase {
            expr: "case (3,4) of pair@(n, _) -> case pair of (m, _) -> m",
            expect: Expect::Int(3),
            note: "as パターンで値全体を再利用",
        },
        EvalCase {
            expr: "case 5 of n | n > 3 -> 1; _ -> 0",
            expect: Expect::Int(1),
            note: "ガードが True で分岐",
        },
        EvalCase {
            expr: "case \"ok\" of \"ok\" -> 1; _ -> 0",
            expect: Expect::Int(1),
            note: "文字列リテラルパターン",
        },
        EvalCase {
            expr: "case 1.5 of 1.5 -> 1; _ -> 0",
            expect: Expect::Int(1),
            note: "浮動小数リテラルパターン",
        },
    ];

    let failure_cases = [
        EvalCase {
            expr: "2 ^ 2 ^ 2 ^ 2 ^ 2",
            expect: Expect::Error("EVAL060"),
            note: "巨大累乗のオーバーフロー",
        },
        EvalCase {
            expr: "(0.0 / 0.0) < 1.0",
            expect: Expect::Error("EVAL090"),
            note: "NaN 比較でエラー",
        },
        EvalCase {
            expr: "show (\\x -> x)",
            expect: Expect::Error("EVAL050"),
            note: "関数への show 適用",
        },
        EvalCase {
            expr: "case 1 of x | 1 -> 0; _ -> 1",
            expect: Expect::Error("EVAL080"),
            note: "ガードは Bool を返す必要がある",
        },
        EvalCase {
            expr: "1 2",
            expect: Expect::Error("EVAL020"),
            note: "非関数値の適用",
        },
        EvalCase {
            expr: "div 1 0",
            expect: Expect::Error("EVAL061"),
            note: "div のゼロ除算",
        },
        EvalCase {
            expr: "quot 1 0",
            expect: Expect::Error("EVAL061"),
            note: "quot のゼロ除算",
        },
        EvalCase {
            expr: "case False of True -> 1",
            expect: Expect::Error("EVAL070"),
            note: "case で該当分岐なし",
        },
    ];

    for case in cases.iter().chain(failure_cases.iter()) {
        verify_case(case);
    }

    // PrimOp 部分適用を AST 直接指定で検証する。
    let partial_expr = Expr::App {
        func: Box::new(Expr::Var {
            name: "+".into(),
            span: Span::dummy(),
        }),
        arg: Box::new(Expr::IntLit {
            value: 1,
            base: IntBase::Dec,
            span: Span::dummy(),
        }),
        span: Span::dummy(),
    };
    let env = evaluator::initial_env();
    let partial = evaluator::eval_expr(&partial_expr, &env).expect("partial eval");
    match partial {
        Value::Prim(op) => match op.clone().apply(Value::Int(41)).expect("apply second arg") {
            Value::Int(result) => assert_eq!(result, 42, "PrimOp 部分適用"),
            other => panic!("partial application should yield Int, got {:?}", other),
        },
        other => panic!("partial application should yield Prim, got {:?}", other),
    }

    let eq_partial_expr = Expr::App {
        func: Box::new(Expr::Var {
            name: "==".into(),
            span: Span::dummy(),
        }),
        arg: Box::new(Expr::IntLit {
            value: 2,
            base: IntBase::Dec,
            span: Span::dummy(),
        }),
        span: Span::dummy(),
    };
    let env = evaluator::initial_env();
    let eq_partial = evaluator::eval_expr(&eq_partial_expr, &env).expect("eq partial eval");
    match eq_partial {
        Value::Prim(op) => match op
            .clone()
            .apply(Value::Int(2))
            .expect("apply eq second arg")
        {
            Value::Bool(result) => assert!(result, "Eq PrimOp 部分適用"),
            other => panic!("partial equality should yield Bool, got {:?}", other),
        },
        other => panic!("partial equality should yield Prim, got {:?}", other),
    }
}

#[test]
/// 実行時エラーに位置情報とスタックトレースが含まれることを検証する。
fn eval_error_reports_stack_with_location() {
    let err = eval_result("1 2").expect_err("非関数適用エラー");
    let rendered = err.to_string();
    assert!(
        rendered.contains("@line=1"),
        "line metadata missing: {rendered}"
    );
    assert!(
        rendered.contains("Stack trace:"),
        "stack trace missing: {rendered}"
    );
    assert!(
        rendered.contains("(1 2)"),
        "stack summary missing expression: {rendered}"
    );
}
