use typelang::{evaluator, infer, parser, typesys};

// 実行時I/Oを避けるため、テストデータをビルド時に埋め込む
const BASICS_TL: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/basics.tl"));
const ADVANCED_TL: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/advanced.tl"));
const FULL_SUITE_TL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/examples/full_suite.tl"
));

#[test]
fn basics_types_snapshot() {
    let prog = parser::parse_program(BASICS_TL).unwrap();
    let mut tenv = infer::initial_env();
    let mut venv = evaluator::initial_env();
    let cenv = infer::initial_class_env();
    let loaded = typelang::repl::load_program_into_env(&prog, &mut tenv, &cenv, &mut venv).unwrap();
    assert!(loaded.contains(&"id".to_string()) && loaded.contains(&"square".to_string()));

    let id = tenv.lookup("id").unwrap().qual.clone();
    let square = tenv.lookup("square").unwrap().qual.clone();
    let id_s = typesys::pretty_qual(&id);
    let sq_s = typesys::pretty_qual(&square);
    assert!(id_s == "a -> a");
    assert!(sq_s == "Num a => a -> a");
}

#[test]
fn advanced_types_snapshot() {
    let prog = parser::parse_program(ADVANCED_TL).unwrap();
    let mut tenv = infer::initial_env();
    let mut venv = evaluator::initial_env();
    let cenv = infer::initial_class_env();
    typelang::repl::load_program_into_env(&prog, &mut tenv, &cenv, &mut venv).unwrap();

    let inv2 = typesys::pretty_qual(&tenv.lookup("inv2").unwrap().qual);
    let powf = typesys::pretty_qual(&tenv.lookup("powf").unwrap().qual);
    assert!(inv2 == "Double");
    assert!(powf == "Double");
}

#[test]
fn full_suite_types_and_values() {
    let prog = parser::parse_program(FULL_SUITE_TL).unwrap();
    let mut tenv = infer::initial_env();
    let mut venv = evaluator::initial_env();
    let cenv = infer::initial_class_env();
    let loaded = typelang::repl::load_program_into_env(&prog, &mut tenv, &cenv, &mut venv).unwrap();

    for symbol in [
        "generalAbs",
        "compose",
        "applyTwice",
        "powIntExample",
        "powFracExample",
        "numbers",
        "choiceMessage",
    ] {
        assert!(loaded.contains(&symbol.to_string()));
    }

    let abs_ty = typesys::pretty_qual(&tenv.lookup("generalAbs").unwrap().qual);
    let compose_ty = typesys::pretty_qual(&tenv.lookup("compose").unwrap().qual);
    let apply_twice_ty = typesys::pretty_qual(&tenv.lookup("applyTwice").unwrap().qual);
    let pow_int_ty = typesys::pretty_qual(&tenv.lookup("powIntExample").unwrap().qual);
    let pow_frac_ty = typesys::pretty_qual(&tenv.lookup("powFracExample").unwrap().qual);

    assert_eq!(abs_ty, "Num a => a -> a");
    assert!(
        compose_ty == "(b -> c) -> (a -> b) -> a -> c"
            || compose_ty == "(a -> b) -> (c -> a) -> c -> b"
    );
    assert_eq!(apply_twice_ty, "(a -> a) -> a -> a");
    assert!(pow_int_ty == "Int" || pow_int_ty == "Integer");
    assert_eq!(pow_frac_ty, "Double");

    let numbers = venv.get("numbers").unwrap();
    assert!(matches!(numbers, evaluator::Value::List(list) if list.len() == 4));

    let combined_demo = venv.get("combinedDemo").unwrap();
    assert!(matches!(combined_demo, evaluator::Value::Int(_)));

    let pow_int_value = venv.get("powIntExample").unwrap();
    assert!(matches!(pow_int_value, evaluator::Value::Int(256)));

    let choice_msg = venv.get("choiceMessage").unwrap();
    assert!(matches!(choice_msg, evaluator::Value::String(s) if s == "Hello, TypeLang!"));

    let pow_frac_value = venv.get("powFracExample").unwrap();
    assert!(matches!(pow_frac_value, evaluator::Value::Double(_)));
}
