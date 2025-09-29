// パス: tests/evaluator.rs
// 役割: 評価器の正常系と代表的な失敗ケースを最小構成で検証
// 意図: 数値演算・比較・pow・show の挙動が回帰しないようにする
// 関連ファイル: src/evaluator.rs, src/parser.rs, tests/types_infer.rs
use typelang::{evaluator, parser, EvalError};

fn eval_value(src: &str) -> evaluator::Value {
    let expr = parser::parse_expr(src).expect("parse");
    let mut env = evaluator::initial_env();
    evaluator::eval_expr(&expr, &mut env).expect("eval")
}

fn eval_bool(src: &str) -> bool {
    match eval_value(src) {
        evaluator::Value::Bool(b) => b,
        other => panic!("expected Bool, got {:?}", other),
    }
}

#[test]
fn eval_int_equality_and_ordering() {
    assert!(eval_bool("1 == 1"));
    assert!(!eval_bool("2 < 1"));
}

#[test]
fn eval_double_comparisons() {
    assert!(eval_bool("2.0 >= -1"));
}

#[test]
fn eval_tuple_and_list_comparisons() {
    assert!(eval_bool("(1,2) == (1,2)"));
    assert!(eval_bool("[1,2] == [1,2]"));
    assert!(eval_bool("[1,2] < [1,3]"));
}

#[test]
fn eval_tuple_ordering_is_lexicographic() {
    assert!(eval_bool("(1,3) < (1,4)"));
}

#[test]
fn eval_division_promotes_to_double() {
    match eval_value("1 / 2") {
        evaluator::Value::Double(d) => assert!((d - 0.5).abs() < 1e-12),
        other => panic!("expected Double, got {:?}", other),
    }
}

#[test]
fn eval_pow_behaviour() {
    match eval_value("2 ^ 3 ^ 2") {
        evaluator::Value::Int(i) => assert_eq!(i, 512),
        other => panic!("expected Int, got {:?}", other),
    }
    match eval_value("2 ^ -1") {
        evaluator::Value::Double(d) => assert!((d - 0.5).abs() < 1e-12),
        other => panic!("expected Double, got {:?}", other),
    }
    match eval_value("(-2) ^ 3") {
        evaluator::Value::Int(i) => assert_eq!(i, -8),
        other => panic!("expected Int, got {:?}", other),
    }
}

#[test]
fn eval_pow_overflow_is_error() {
    let expr = parser::parse_expr("2 ^ 2 ^ 2 ^ 2 ^ 2").unwrap();
    let mut env = evaluator::initial_env();
    match evaluator::eval_expr(&expr, &mut env) {
        Err(EvalError(info)) => assert_eq!(info.code, "EVAL060"),
        other => panic!("expected overflow error, got {:?}", other),
    }
}

#[test]
fn eval_powf_negative_defaulting() {
    match eval_value("2 ** -1") {
        evaluator::Value::Double(d) => assert!((d - 0.5).abs() < 1e-12),
        other => panic!("expected Double, got {:?}", other),
    }
}

#[test]
fn eval_nan_comparison_is_error() {
    let expr = parser::parse_expr("(0.0 / 0.0) < 1.0").unwrap();
    let mut env = evaluator::initial_env();
    assert!(evaluator::eval_expr(&expr, &mut env).is_err());
}

#[test]
fn eval_show_function_is_error() {
    let expr = parser::parse_expr("show (\\x -> x)").unwrap();
    let mut env = evaluator::initial_env();
    assert!(evaluator::eval_expr(&expr, &mut env).is_err());
}

#[test]
fn eval_apply_non_function_is_error() {
    let expr = parser::parse_expr("1 2").unwrap();
    let mut env = evaluator::initial_env();
    assert!(evaluator::eval_expr(&expr, &mut env).is_err());
}
