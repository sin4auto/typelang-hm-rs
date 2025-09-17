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
        "annotatedSquare",
        "numericIdInt",
        "equalityCheck",
        "orderingCheck",
        "tupleOrderingCheck",
        "charOrdering",
        "shownFlag",
        "shownGreeting",
        "fractionalDemo",
        "mappedNumbers",
        "foldedSum",
        "foldedProduct",
        "composeResult",
        "applyTwiceResult",
    ] {
        assert!(loaded.contains(&symbol.to_string()));
    }

    let abs_ty = typesys::pretty_qual(&tenv.lookup("generalAbs").unwrap().qual);
    let compose_ty = typesys::pretty_qual(&tenv.lookup("compose").unwrap().qual);
    let apply_twice_ty = typesys::pretty_qual(&tenv.lookup("applyTwice").unwrap().qual);
    let pow_int_ty = typesys::pretty_qual(&tenv.lookup("powIntExample").unwrap().qual);
    let pow_frac_ty = typesys::pretty_qual(&tenv.lookup("powFracExample").unwrap().qual);
    let annotated_square_ty = typesys::pretty_qual(&tenv.lookup("annotatedSquare").unwrap().qual);
    let numeric_id_int_ty = typesys::pretty_qual(&tenv.lookup("numericIdInt").unwrap().qual);
    let equality_check_ty = typesys::pretty_qual(&tenv.lookup("equalityCheck").unwrap().qual);
    let ordering_check_ty = typesys::pretty_qual(&tenv.lookup("orderingCheck").unwrap().qual);
    let tuple_ordering_ty = typesys::pretty_qual(&tenv.lookup("tupleOrderingCheck").unwrap().qual);
    let char_ordering_ty = typesys::pretty_qual(&tenv.lookup("charOrdering").unwrap().qual);
    let shown_flag_ty = typesys::pretty_qual(&tenv.lookup("shownFlag").unwrap().qual);
    let shown_greeting_ty = typesys::pretty_qual(&tenv.lookup("shownGreeting").unwrap().qual);
    let fractional_demo_ty = typesys::pretty_qual(&tenv.lookup("fractionalDemo").unwrap().qual);
    let mapped_numbers_ty = typesys::pretty_qual(&tenv.lookup("mappedNumbers").unwrap().qual);
    let folded_sum_ty = typesys::pretty_qual(&tenv.lookup("foldedSum").unwrap().qual);
    let folded_product_ty = typesys::pretty_qual(&tenv.lookup("foldedProduct").unwrap().qual);
    let compose_result_ty = typesys::pretty_qual(&tenv.lookup("composeResult").unwrap().qual);
    let apply_twice_result_ty =
        typesys::pretty_qual(&tenv.lookup("applyTwiceResult").unwrap().qual);

    assert_eq!(abs_ty, "Num a => a -> a");
    assert!(
        compose_ty == "(b -> c) -> (a -> b) -> a -> c"
            || compose_ty == "(a -> b) -> (c -> a) -> c -> b"
    );
    assert_eq!(apply_twice_ty, "(a -> a) -> a -> a");
    assert!(pow_int_ty == "Int" || pow_int_ty == "Integer");
    assert_eq!(pow_frac_ty, "Double");
    assert!(annotated_square_ty == "Int" || annotated_square_ty == "Integer");
    assert!(numeric_id_int_ty == "Int" || numeric_id_int_ty == "Integer");
    assert_eq!(equality_check_ty, "Bool");
    assert_eq!(ordering_check_ty, "Bool");
    assert_eq!(tuple_ordering_ty, "Bool");
    assert_eq!(char_ordering_ty, "Bool");
    assert!(shown_flag_ty == "String" || shown_flag_ty == "[Char]");
    assert!(shown_greeting_ty == "String" || shown_greeting_ty == "[Char]");
    assert_eq!(fractional_demo_ty, "Double");
    assert!(mapped_numbers_ty == "[Int]" || mapped_numbers_ty == "[Integer]");
    assert!(folded_sum_ty == "Int" || folded_sum_ty == "Integer");
    assert!(folded_product_ty == "Int" || folded_product_ty == "Integer");
    assert!(compose_result_ty == "Int" || compose_result_ty == "Integer");
    assert!(apply_twice_result_ty == "Int" || apply_twice_result_ty == "Integer");

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

    let annotated_square_val = venv.get("annotatedSquare").unwrap();
    assert!(matches!(annotated_square_val, evaluator::Value::Int(25)));

    let numeric_id_int_val = venv.get("numericIdInt").unwrap();
    assert!(matches!(numeric_id_int_val, evaluator::Value::Int(7)));

    let equality_check_val = venv.get("equalityCheck").unwrap();
    assert!(matches!(equality_check_val, evaluator::Value::Bool(true)));

    let ordering_check_val = venv.get("orderingCheck").unwrap();
    assert!(matches!(ordering_check_val, evaluator::Value::Bool(true)));

    let tuple_ordering_val = venv.get("tupleOrderingCheck").unwrap();
    assert!(matches!(tuple_ordering_val, evaluator::Value::Bool(true)));

    let char_ordering_val = venv.get("charOrdering").unwrap();
    assert!(matches!(char_ordering_val, evaluator::Value::Bool(true)));

    let shown_flag_val = venv.get("shownFlag").unwrap();
    assert!(matches!(shown_flag_val, evaluator::Value::String(s) if s == "True"));

    let shown_greeting_val = venv.get("shownGreeting").unwrap();
    assert!(matches!(shown_greeting_val, evaluator::Value::String(s) if s == "Hello, TypeLang!"));

    let fractional_demo_val = venv.get("fractionalDemo").unwrap();
    if let evaluator::Value::Double(d) = fractional_demo_val {
        assert!((d - 3.5).abs() < f64::EPSILON);
    } else {
        panic!("fractionalDemo は Double のはずです");
    }

    let mapped_numbers_val = venv.get("mappedNumbers").unwrap();
    if let evaluator::Value::List(items) = mapped_numbers_val {
        let ints: Vec<i64> = items
            .iter()
            .map(|v| match v {
                evaluator::Value::Int(i) => *i,
                _ => panic!("mappedNumbers には Int 以外が含まれています"),
            })
            .collect();
        assert_eq!(ints, vec![11, 12, 13, 14]);
    } else {
        panic!("mappedNumbers はリストである必要があります");
    }

    let folded_sum_val = venv.get("foldedSum").unwrap();
    assert!(matches!(folded_sum_val, evaluator::Value::Int(10)));

    let folded_product_val = venv.get("foldedProduct").unwrap();
    assert!(matches!(folded_product_val, evaluator::Value::Int(24)));

    let compose_result_val = venv.get("composeResult").unwrap();
    assert!(matches!(compose_result_val, evaluator::Value::Int(16)));

    let apply_twice_result_val = venv.get("applyTwiceResult").unwrap();
    assert!(matches!(apply_twice_result_val, evaluator::Value::Int(3)));
}
