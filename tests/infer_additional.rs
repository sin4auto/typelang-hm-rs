// 日本語コメント: 型推論の正常/異常系を追加

use typelang::{infer, parser, typesys};

#[test]
fn infer_let_polymorphism_eval_ok() {
    // let 多相: 同じ id を Int と Bool の両方に適用
    let e = parser::parse_expr("let id x = x in (id 1, id True)").expect("parse");
    let mut venv = typelang::evaluator::initial_env();
    let v = typelang::evaluator::eval_expr(&e, &mut venv).expect("eval");
    match v {
        typelang::evaluator::Value::Tuple(items) => {
            assert!(matches!(items[0], typelang::evaluator::Value::Int(1)));
            assert!(matches!(items[1], typelang::evaluator::Value::Bool(true)));
        }
        _ => panic!("unexpected value: {:?}", v),
    }
}

#[test]
fn infer_unknown_variable_is_error() {
    let e = parser::parse_expr("foo").unwrap();
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
fn infer_if_branch_type_mismatch_error() {
    // then/else の型不一致（注釈により Int と Char を明示）
    let e = parser::parse_expr("if True then (1 :: Int) else ('a' :: Char)").unwrap();
    let env = infer::initial_env();
    let ce = infer::initial_class_env();
    let mut st = infer::InferState {
        supply: typesys::TVarSupply::new(),
        subst: Default::default(),
    };
    let res = infer::infer_expr(&env, &ce, &mut st, &e);
    assert!(res.is_err());
}
