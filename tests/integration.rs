// パス: tests/integration.rs
// 役割: 例題プログラムと REPL ローダーまわりの統合テスト
// 意図: ドキュメント掲載コードが読み込めることと REPL の失敗経路を保証する
// 関連ファイル: examples/basics.tl, examples/advanced.tl, examples/ebnf_blackbox.tl, src/repl/loader.rs
#[path = "test_support.rs"]
mod support;

use support::{assert_value_bool, assert_value_double, assert_value_int, ProgramFixture};
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
        let fixture = ProgramFixture::load(case.src);
        for name in case.expected {
            assert!(
                fixture.exports.iter().any(|export| export == name),
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
        let fixture = ProgramFixture::load(case.src);
        let scheme = fixture.type_env.lookup(case.symbol).unwrap().qual.clone();
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
    let fixture = ProgramFixture::load(BASICS_TL);
    let value = fixture.eval_value("2 ** -1");
    assert_value_double(value, 0.5, "2 ** -1 via loaded env");

    let src = "let x = y"; // y が未定義
    let prog = parser::parse_program(src).unwrap();
    let mut type_env = infer::initial_env();
    let mut value_env = evaluator::initial_env();
    let mut class_env = infer::initial_class_env();
    let res =
        typelang::repl::load_program_into_env(&prog, &mut type_env, &mut class_env, &mut value_env);
    assert!(res.is_err());
    value_env.teardown();
}

#[test]
/// data 宣言と case 式を含むプログラムをロードして評価する。
fn load_data_and_case_evaluate() {
    let fixture = ProgramFixture::load(ADT_TL);
    assert!(fixture.exports.iter().any(|name| name == "fromMaybe"));
    assert!(fixture.type_env.lookup("Nothing").is_some());

    assert_value_int(
        fixture.eval_value("fromMaybe 0 Nothing"),
        0,
        "fromMaybe fallback",
    );
    assert_value_int(
        fixture.eval_value("fromMaybe 0 (Just 42)"),
        42,
        "fromMaybe unwrap",
    );
    assert_value_bool(
        fixture.eval_value("isJust (Just 1)"),
        true,
        "isJust detects Just",
    );
}

#[test]
/// class / instance 宣言が ClassEnv に反映されることを確認する。
fn load_user_defined_class_and_instance() {
    let src = "class Eqish a\ninstance Eqish Int\nlet id x = x";
    let fixture = ProgramFixture::load(src);
    assert!(fixture.class_env.classes.contains_key("Eqish"));
    assert!(fixture
        .class_env
        .instances
        .contains(&("Eqish".to_string(), "Int".to_string())));
}
