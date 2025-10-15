// パス: tests/integration.rs
// 役割: 例題プログラムと REPL ローダーまわりの統合テスト
// 意図: ドキュメント掲載コードが読み込めることと REPL の失敗経路を保証する
// 関連ファイル: examples/basics.tl, examples/advanced.tl, examples/ebnf_blackbox.tl, src/repl/loader.rs
#[path = "test_support.rs"]
mod support;

use support::parse_expr;
use typelang::typesys::TypeEnv;
use typelang::{evaluator, infer, parser};

const BASICS_TL: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/basics.tl"));
const ADVANCED_TL: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/advanced.tl"));
const EBNF_BLACKBOX_TL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/examples/ebnf_blackbox.tl"
));
const ADT_TL: &str = r#"
data Maybe a = Nothing | Just a;

let fromMaybe default m =
  case m of
    Nothing -> default;
    Just x -> x;

let isJust m =
  case m of
    Just _ -> True;
    Nothing -> False;
"#;

/// テキストを解析して型環境・値環境へロードするヘルパ。
fn load_program(src: &str) -> (TypeEnv, evaluator::Env, Vec<String>) {
    let prog = parser::parse_program(src).expect("parse program");
    let mut type_env = infer::initial_env();
    let mut value_env = evaluator::initial_env();
    let mut class_env = infer::initial_class_env();
    let loaded =
        typelang::repl::load_program_into_env(&prog, &mut type_env, &mut class_env, &mut value_env)
            .expect("load program");
    (type_env, value_env, loaded)
}

#[test]
#[cfg_attr(
    miri,
    ignore = "Miri では examples_ebnf_blackbox.tl の読込時に大量 Clone が発生し、割り込みが必要になるため無効化"
)]
/// サンプルプログラムのエクスポートと型スナップショットをまとめて検証する。
fn load_examples_and_validate_types() {
    struct ExportCase<'a> {
        src: &'a str,
        expected: &'a [&'a str],
        note: &'a str,
    }

    let export_cases = [
        ExportCase {
            src: BASICS_TL,
            expected: &["id", "square"],
            note: "basics exports",
        },
        ExportCase {
            src: EBNF_BLACKBOX_TL,
            expected: &[
                "maybeDefault",
                "patternJudge",
                "arithSeries",
                "floatingCombo",
                "structurePack",
                "pipeline",
            ],
            note: "ebnf exports",
        },
        ExportCase {
            src: ADT_TL,
            expected: &["fromMaybe", "isJust"],
            note: "maybe exports",
        },
    ];

    for case in export_cases {
        let (_tenv, _venv, loaded) = load_program(case.src);
        for name in case.expected {
            assert!(
                loaded.iter().any(|export| export == name),
                "{}: missing export {name}",
                case.note
            );
        }
    }

    struct TypeCase<'a> {
        src: &'a str,
        symbol: &'a str,
        expected: &'a str,
    }

    let type_cases = [
        TypeCase {
            src: BASICS_TL,
            symbol: "id",
            expected: "a -> a",
        },
        TypeCase {
            src: BASICS_TL,
            symbol: "square",
            expected: "Num a => a -> a",
        },
        TypeCase {
            src: ADVANCED_TL,
            symbol: "inv2",
            expected: "Double",
        },
        TypeCase {
            src: ADVANCED_TL,
            symbol: "powf",
            expected: "Double",
        },
    ];

    for case in type_cases {
        let (type_env, _value_env, _) = load_program(case.src);
        let scheme = type_env.lookup(case.symbol).unwrap().qual.clone();
        assert_eq!(
            typelang::typesys::pretty_qual(&scheme),
            case.expected,
            "型スナップショットが一致しません: {}",
            case.symbol
        );
    }
}

#[test]
/// ロードした環境から演算を呼び出せることと、エラー経路を検証する。
fn evaluate_with_loaded_env_and_failure_paths() {
    let (_type_env, value_env, _) = load_program(BASICS_TL);
    let expr = parse_expr("2 ** -1");
    let value = evaluator::eval_expr(&expr, &value_env).unwrap();
    match value {
        evaluator::Value::Double(d) => assert!((d - 0.5).abs() < 1e-12),
        other => panic!("expected Double, got {:?}", other),
    }

    let src = "let x = y"; // y が未定義
    let prog = parser::parse_program(src).unwrap();
    let mut type_env = infer::initial_env();
    let mut value_env = evaluator::initial_env();
    let mut class_env = infer::initial_class_env();
    let res =
        typelang::repl::load_program_into_env(&prog, &mut type_env, &mut class_env, &mut value_env);
    assert!(res.is_err());
}

#[test]
/// data 宣言と case 式を含むプログラムをロードして評価する。
fn load_data_and_case_evaluate() {
    let (type_env, value_env, loaded) = load_program(ADT_TL);
    assert!(loaded.iter().any(|name| name == "fromMaybe"));
    assert!(type_env.lookup("Nothing").is_some());
    let expr_default = parse_expr("fromMaybe 0 Nothing");
    match evaluator::eval_expr(&expr_default, &value_env).expect("eval fromMaybe Nothing") {
        evaluator::Value::Int(i) => assert_eq!(i, 0),
        other => panic!("expected Int 0, got {:?}", other),
    }
    let expr_just = parse_expr("fromMaybe 0 (Just 42)");
    match evaluator::eval_expr(&expr_just, &value_env).expect("eval fromMaybe Just") {
        evaluator::Value::Int(i) => assert_eq!(i, 42),
        other => panic!("expected Int 42, got {:?}", other),
    }
    let expr_isjust = parse_expr("isJust (Just 1)");
    match evaluator::eval_expr(&expr_isjust, &value_env).expect("eval isJust") {
        evaluator::Value::Bool(b) => assert!(b),
        other => panic!("expected Bool True, got {:?}", other),
    }
}

#[test]
/// class / instance 宣言が ClassEnv に反映されることを確認する。
fn load_user_defined_class_and_instance() {
    let src = "class Eqish a\ninstance Eqish Int\nlet id x = x";
    let prog = parser::parse_program(src).expect("parse class program");
    let mut type_env = infer::initial_env();
    let mut value_env = evaluator::initial_env();
    let mut class_env = infer::initial_class_env();
    typelang::repl::load_program_into_env(&prog, &mut type_env, &mut class_env, &mut value_env)
        .expect("load program with class");
    assert!(class_env.classes.contains_key("Eqish"));
    assert!(class_env
        .instances
        .contains(&("Eqish".to_string(), "Int".to_string())));
}
