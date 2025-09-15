use typelang::{evaluator, parser};

#[test]
fn eval_apply_non_function_error() {
    // 1 2 は 1 を関数適用しようとして失敗
    let e = parser::parse_expr("1 2").unwrap();
    let mut env = evaluator::initial_env();
    let res = evaluator::eval_expr(&e, &mut env);
    assert!(res.is_err());
}

#[test]
fn eval_division_ints_yields_double() {
    let e = parser::parse_expr("1 / 2").unwrap();
    let mut env = evaluator::initial_env();
    let v = evaluator::eval_expr(&e, &mut env).unwrap();
    match v {
        evaluator::Value::Double(d) => assert!((d - 0.5).abs() < 1e-12),
        _ => panic!("expected Double"),
    }
}

#[test]
fn eval_pow_negative_int_exponent_is_double() {
    let e = parser::parse_expr("2 ^ -1").unwrap();
    let mut env = evaluator::initial_env();
    let v = evaluator::eval_expr(&e, &mut env).unwrap();
    match v {
        evaluator::Value::Double(d) => assert!((d - 0.5).abs() < 1e-12),
        _ => panic!("expected Double"),
    }
}

#[test]
fn eval_nan_comparison_is_error() {
    // 0.0/0.0 は NaN、比較はエラー
    let e = parser::parse_expr("(0.0 / 0.0) < 1.0").unwrap();
    let mut env = evaluator::initial_env();
    let res = evaluator::eval_expr(&e, &mut env);
    assert!(res.is_err());
}
