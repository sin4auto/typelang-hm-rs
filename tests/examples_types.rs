use typelang::{evaluator, infer, parser, typesys};

// 実行時I/Oを避けるため、テストデータをビルド時に埋め込む
const BASICS_TL: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/basics.tl"));
const ADVANCED_TL: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/advanced.tl"));

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
