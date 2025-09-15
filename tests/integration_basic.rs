// 日本語コメント: 基本的な型推論と評価の統合テスト

use typelang::{evaluator, infer, parser};

// 実行時I/Oを避けて、テストデータをビルド時に埋め込む（Miri対応）
const BASICS_TL: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/basics.tl"));

#[test]
fn infer_lambda_num() {
    let e = parser::parse_expr("\\x -> x + 1").expect("parse");
    let ty = infer::infer_type_str(&e).expect("infer");
    assert_eq!(ty, "Num a => a -> a");
}

#[test]
fn eval_pow_assoc() {
    let e = parser::parse_expr("2 ^ 3 ^ 2").expect("parse");
    let mut env = evaluator::initial_env();
    let v = evaluator::eval_expr(&e, &mut env).expect("eval");
    match v {
        evaluator::Value::Int(i) => assert_eq!(i, 512),
        _ => panic!("unexpected value: {:?}", v),
    }
}

#[test]
fn eval_powf_neg() {
    let e = parser::parse_expr("2 ** -1").expect("parse");
    let mut env = evaluator::initial_env();
    let v = evaluator::eval_expr(&e, &mut env).expect("eval");
    match v {
        evaluator::Value::Double(d) => assert!((d - 0.5).abs() < 1e-12),
        _ => panic!("unexpected value: {:?}", v),
    }
}

#[test]
fn load_program_examples() {
    // 以前は std::fs::read_to_string で実行時I/Oを行っていたが、
    // Miri の isolation で失敗するため include_str! に変更
    let prog = parser::parse_program(BASICS_TL).expect("parse program");
    let mut tenv = infer::initial_env();
    let mut venv = evaluator::initial_env();
    let cenv = infer::initial_class_env();
    let loaded =
        typelang::repl::load_program_into_env(&prog, &mut tenv, &cenv, &mut venv).expect("load");
    assert!(loaded.contains(&"id".to_string()));
}
