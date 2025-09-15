use typelang::{evaluator, infer, parser};

// 異常系: 未束縛変数を含むトップレベル定義は :load 相当で失敗を返す
#[test]
fn repl_load_program_unbound_name_returns_err() {
    let src = "let x = y"; // y が未定義
    let prog = parser::parse_program(src).unwrap();
    let mut tenv = infer::initial_env();
    let mut venv = evaluator::initial_env();
    let cenv = infer::initial_class_env();
    let res = typelang::repl::load_program_into_env(&prog, &mut tenv, &cenv, &mut venv);
    assert!(res.is_err());
}
