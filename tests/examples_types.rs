use typelang::{evaluator, infer, parser, typesys};

#[test]
fn basics_types_snapshot() {
    let src = std::fs::read_to_string("examples/basics.tl").unwrap();
    let prog = parser::parse_program(&src).unwrap();
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
    let src = std::fs::read_to_string("examples/advanced.tl").unwrap();
    let prog = parser::parse_program(&src).unwrap();
    let mut tenv = infer::initial_env();
    let mut venv = evaluator::initial_env();
    let cenv = infer::initial_class_env();
    let _ = typelang::repl::load_program_into_env(&prog, &mut tenv, &cenv, &mut venv).unwrap();
    // 既定化と正規化により以下が成り立つ
    let inv2 = typesys::pretty_qual(&tenv.lookup("inv2").unwrap().qual);
    let powf = typesys::pretty_qual(&tenv.lookup("powf").unwrap().qual);
    assert!(inv2 == "Double");
    assert!(powf == "Double");
}
