use typelang::{evaluator, infer, parser, typesys};

#[test]
fn parse_error_unclosed_paren() {
    // 不完全な括弧列 "(1 + 2" をパースするとエラーになることを確認する
    let res = parser::parse_expr("(1 + 2");
    assert!(res.is_err());
}

#[test]
fn type_error_if_condition_not_bool() {
    // if 式の条件が Bool 型でない場合に型推論がエラーを返すことを確認する
    // 例: if 'a' then 2 else 3
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
    // examples/basics.tl をビルド時に埋め込んだ文字列から読み込み、
    // プログラムをパースして型環境・値環境にロードする
    const BASICS_TL: &str =
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/basics.tl"));
    let prog1 = parser::parse_program(BASICS_TL).unwrap();
    let mut tenv = infer::initial_env();
    let mut venv = evaluator::initial_env();
    let cenv = infer::initial_class_env();
    typelang::repl::load_program_into_env(&prog1, &mut tenv, &cenv, &mut venv).unwrap();

    // 上で作成した環境を複製し、式 "2 ** -1" を評価して
    // 結果が 0.5 (Double 型) であることを確認する
    let mut venv2 = venv.clone();
    let v = evaluator::eval_expr(&parser::parse_expr("2 ** -1").unwrap(), &mut venv2).unwrap();
    match v {
        evaluator::Value::Double(d) => assert!((d - 0.5).abs() < 1e-12),
        _ => panic!("expected Double(0.5)"),
    }
}
