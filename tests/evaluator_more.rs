use typelang::{evaluator, parser, EvalError};

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

#[test]
fn eval_pow_int_overflow_is_error() {
    // 2^(2^(2^(2^2))) = 2^65536 は Int に収まらず EvalError
    let e = parser::parse_expr("2 ^ 2 ^ 2 ^ 2 ^ 2").unwrap();
    let mut env = evaluator::initial_env();
    match evaluator::eval_expr(&e, &mut env) {
        Err(EvalError(info)) => assert_eq!(info.code, "EVAL060"),
        _ => panic!("expected EvalError for overflow"),
    }
}

#[test]
fn eval_map_over_list() {
    let e = parser::parse_expr("map (\\x -> x + 1) [1, 2, 3]").unwrap();
    let mut env = evaluator::initial_env();
    let v = evaluator::eval_expr(&e, &mut env).unwrap();
    match v {
        evaluator::Value::List(items) => {
            let ints: Vec<_> = items
                .into_iter()
                .map(|v| match v {
                    evaluator::Value::Int(i) => i,
                    _ => panic!("expected Int"),
                })
                .collect();
            assert_eq!(ints, vec![2, 3, 4]);
        }
        _ => panic!("expected List"),
    }
}

#[test]
fn eval_foldl_sum_list() {
    let e = parser::parse_expr("foldl (\\acc x -> acc + x) 0 [1, 2, 3, 4]").unwrap();
    let mut env = evaluator::initial_env();
    let v = evaluator::eval_expr(&e, &mut env).unwrap();
    match v {
        evaluator::Value::Int(i) => assert_eq!(i, 10),
        _ => panic!("expected Int"),
    }
}

#[test]
fn eval_foldr_sum_list() {
    let e = parser::parse_expr("foldr (\\x acc -> x + acc) 0 [1, 2, 3]").unwrap();
    let mut env = evaluator::initial_env();
    let v = evaluator::eval_expr(&e, &mut env).unwrap();
    match v {
        evaluator::Value::Int(i) => assert_eq!(i, 6),
        _ => panic!("expected Int"),
    }
}

#[test]
fn eval_map_non_list_is_error() {
    let e = parser::parse_expr("map (\\x -> x) 42").unwrap();
    let mut env = evaluator::initial_env();
    match evaluator::eval_expr(&e, &mut env) {
        Err(EvalError(info)) => assert_eq!(info.code, "EVAL050"),
        _ => panic!("expected EvalError"),
    }
}
