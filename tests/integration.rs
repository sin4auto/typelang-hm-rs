// パス: tests/integration.rs
// 役割: 例題プログラムと REPL ローダーまわりの統合テスト
// 意図: ドキュメント掲載コードが読み込めることと REPL の失敗経路を保証する
// 関連ファイル: examples/basics.tl, examples/advanced.tl, src/repl/loader.rs
use typelang::typesys::TypeEnv;
use typelang::{evaluator, infer, parser};

const BASICS_TL: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/basics.tl"));
const ADVANCED_TL: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/advanced.tl"));

/// テキストを解析して型環境・値環境へロードするヘルパ。
fn load_program(src: &str) -> (TypeEnv, evaluator::Env, Vec<String>) {
    let prog = parser::parse_program(src).expect("parse program");
    let mut type_env = infer::initial_env();
    let mut value_env = evaluator::initial_env();
    let class_env = infer::initial_class_env();
    let loaded =
        typelang::repl::load_program_into_env(&prog, &mut type_env, &class_env, &mut value_env)
            .expect("load program");
    (type_env, value_env, loaded)
}

#[test]
/// サンプルプログラムが想定する名前をエクスポートするか検証する。
fn load_examples_program_exports_expected_names() {
    let (_tenv, _venv, loaded) = load_program(BASICS_TL);
    assert!(loaded.contains(&"id".to_owned()));
    assert!(loaded.contains(&"square".to_owned()));
}

#[test]
/// 基本例の型スナップショットが期待通りか検証する。
fn examples_basics_types_snapshot() {
    let (type_env, _value_env, _) = load_program(BASICS_TL);
    let id = type_env.lookup("id").unwrap().qual.clone();
    let square = type_env.lookup("square").unwrap().qual.clone();
    assert_eq!(typelang::typesys::pretty_qual(&id), "a -> a");
    assert_eq!(typelang::typesys::pretty_qual(&square), "Num a => a -> a");
}

#[test]
/// 応用例の型スナップショットが期待通りか検証する。
fn examples_advanced_types_snapshot() {
    let (type_env, _value_env, _) = load_program(ADVANCED_TL);
    let inv2 = type_env.lookup("inv2").unwrap().qual.clone();
    let powf = type_env.lookup("powf").unwrap().qual.clone();
    assert_eq!(typelang::typesys::pretty_qual(&inv2), "Double");
    assert_eq!(typelang::typesys::pretty_qual(&powf), "Double");
}

#[test]
/// ロードした環境での累乗計算が期待値になるか確認する。
fn examples_eval_pow_through_loaded_env() {
    let (_type_env, mut value_env, _) = load_program(BASICS_TL);
    let expr = parser::parse_expr("2 ** -1").unwrap();
    let value = evaluator::eval_expr(&expr, &mut value_env).unwrap();
    match value {
        evaluator::Value::Double(d) => assert!((d - 0.5).abs() < 1e-12),
        other => panic!("expected Double, got {:?}", other),
    }
}

#[test]
/// 未束縛変数を含む定義がエラーになることを確認する。
fn repl_load_program_unbound_name_returns_err() {
    let src = "let x = y"; // y が未定義
    let prog = parser::parse_program(src).unwrap();
    let mut type_env = infer::initial_env();
    let mut value_env = evaluator::initial_env();
    let class_env = infer::initial_class_env();
    let res =
        typelang::repl::load_program_into_env(&prog, &mut type_env, &class_env, &mut value_env);
    assert!(res.is_err());
}
