use typelang::{evaluator, infer, parser, typesys};

#[test]
fn parse_error_unclosed_paren() {
    let res = parser::parse_expr("(1 + 2");
    assert!(res.is_err());
}

#[test]
fn type_error_if_condition_not_bool() {
    // if 'a' then 2 else 3 は条件が Bool でないため型エラー
    let e = parser::parse_expr("if 'a' then 2 else 3").unwrap();
    let env = infer::initial_env();
    let ce = infer::initial_class_env();
    let mut st = infer::InferState {
        supply: typesys::TVarSupply::new(),
        subst: Default::default(),
    };
    let res = infer::infer_expr(&env, &ce, &mut st, &e);
    assert!(res.is_err());
}

#[test]
fn examples_regression() {
    // basics.tl / advanced.tl がロードでき、いくつかの値が評価できる
    let src1 = std::fs::read_to_string("examples/basics.tl").unwrap();
    let prog1 = parser::parse_program(&src1).unwrap();
    let mut tenv = infer::initial_env();
    let mut venv = evaluator::initial_env();
    let cenv = infer::initial_class_env();
    let _ = typelang::repl::load_program_into_env(&prog1, &mut tenv, &cenv, &mut venv).unwrap();

    // advanced の代表式を直接評価（べき乗の回帰）
    let mut venv2 = venv.clone();
    let v = evaluator::eval_expr(&parser::parse_expr("2 ** -1").unwrap(), &mut venv2).unwrap();
    if let evaluator::Value::Double(d) = v {
        assert!((d - 0.5).abs() < 1e-12)
    } else {
        panic!()
    }
}
