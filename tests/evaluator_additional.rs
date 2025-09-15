// 日本語コメント: 評価器の正常/異常系テスト追加

use typelang::{evaluator, parser};

#[test]
fn eval_show_function_is_error() {
    // show は関数値には未対応
    let e = parser::parse_expr("show (\\x -> x)").unwrap();
    let mut env = evaluator::initial_env();
    let res = evaluator::eval_expr(&e, &mut env);
    assert!(res.is_err());
}

#[test]
fn eval_tuple_ordering() {
    let e = parser::parse_expr("(1,3) < (1,4)").unwrap();
    let mut env = evaluator::initial_env();
    let v = evaluator::eval_expr(&e, &mut env).unwrap();
    match v {
        evaluator::Value::Bool(b) => assert!(b),
        _ => panic!("expected Bool"),
    }
}

#[test]
fn eval_negative_base_integer_pow_positive_exponent() {
    let e = parser::parse_expr("(-2) ^ 3").unwrap();
    let mut env = evaluator::initial_env();
    let v = evaluator::eval_expr(&e, &mut env).unwrap();
    match v {
        evaluator::Value::Int(i) => assert_eq!(i, -8),
        _ => panic!("expected Int"),
    }
}
