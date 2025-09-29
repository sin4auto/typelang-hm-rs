// パス: tests/types_infer.rs
// 役割: 型システムと推論ロジックの最小カバレッジ
// 意図: unify・defaulting・型推論の重要挙動を簡潔に検証する
// 関連ファイル: src/infer.rs, src/typesys.rs, tests/evaluator.rs
use typelang::{evaluator, infer, parser, typesys};

fn infer_type(src: &str) -> String {
    let expr = parser::parse_expr(src).expect("parse");
    infer::infer_type_str(&expr).expect("infer")
}

fn infer_type_with_defaulting(src: &str, enable: bool) -> String {
    let expr = parser::parse_expr(src).expect("parse");
    infer::infer_type_str_with_defaulting(&expr, enable).expect("infer")
}

#[test]
fn unify_simple_fun_types() {
    use typesys::*;
    let int = Type::TCon(TCon { name: "Int".into() });
    let fun = Type::TFun(TFun {
        arg: Box::new(int.clone()),
        ret: Box::new(int.clone()),
    });
    assert!(unify(fun.clone(), fun).is_ok());
}

#[test]
fn unify_occurs_check_fails() {
    use typesys::*;
    let tv = TVar { id: 1 };
    let tvar = Type::TVar(tv.clone());
    let fun = Type::TFun(TFun {
        arg: Box::new(Type::TVar(tv.clone())),
        ret: Box::new(Type::TCon(TCon { name: "Int".into() })),
    });
    assert!(unify(tvar, fun).is_err());
}

#[test]
fn unify_constructor_mismatch_is_error() {
    use typesys::*;
    let a = Type::TCon(TCon { name: "Int".into() });
    let b = Type::TCon(TCon {
        name: "Bool".into(),
    });
    assert!(unify(a, b).is_err());
}

#[test]
fn pretty_qual_suppresses_irrelevant_constraints() {
    use typesys::*;
    let tv = TVar { id: 1 };
    let q = QualType {
        constraints: vec![
            Constraint {
                classname: "Fractional".into(),
                r#type: Type::TVar(tv.clone()),
            },
            Constraint {
                classname: "Num".into(),
                r#type: Type::TVar(tv.clone()),
            },
        ],
        r#type: Type::TCon(TCon {
            name: "Double".into(),
        }),
    };
    assert_eq!(typesys::pretty_qual(&q), "Double");
}

#[test]
fn infer_lambda_eq_has_eq_constraint() {
    assert_eq!(infer_type("\\x -> x == x"), "Eq a => a -> Bool");
}

#[test]
fn infer_lambda_num_has_num_constraint() {
    assert_eq!(infer_type("\\x -> x + 1"), "Num a => a -> a");
}

#[test]
fn infer_annotation_on_num_to_bool_shows_bool() {
    assert_eq!(infer_type("1 :: Bool"), "Bool");
}

#[test]
fn infer_pow_negative_int_yields_double() {
    assert_eq!(infer_type("2 ^ -3"), "Double");
}

#[test]
fn infer_starstar_defaulted_is_double() {
    assert_eq!(infer_type_with_defaulting("2 ** -1", true), "Double");
}

#[test]
fn infer_add_without_defaulting_keeps_constraint() {
    assert_eq!(infer_type_with_defaulting("1 + 2", false), "Num a => a");
}

#[test]
fn infer_defaulting_controls_show_constraints() {
    let expr = parser::parse_expr("show 1").unwrap();
    let env = infer::initial_env();
    let ce = infer::initial_class_env();
    let mut st = infer::InferState {
        supply: typesys::TVarSupply::new(),
        subst: Default::default(),
    };
    let (_, q) = infer::infer_expr(&env, &ce, &mut st, &expr).unwrap();
    let txt = typesys::pretty_qual(&q);
    assert!(txt.ends_with("[Char]") || txt.ends_with("String"));

    let defaulted = infer::infer_type_str_with_defaulting(&expr, true).unwrap();
    assert!(defaulted == "String" || defaulted == "[Char]");
}

#[test]
fn infer_let_polymorphism_eval_ok() {
    let expr = parser::parse_expr("let id x = x in (id 1, id True)").unwrap();
    let mut env = evaluator::initial_env();
    let value = evaluator::eval_expr(&expr, &mut env).expect("eval");
    match value {
        evaluator::Value::Tuple(items) => {
            assert!(matches!(items[0], evaluator::Value::Int(1)));
            assert!(matches!(items[1], evaluator::Value::Bool(true)));
        }
        other => panic!("unexpected value: {:?}", other),
    }
}

#[test]
fn infer_unknown_variable_is_error() {
    let expr = parser::parse_expr("foo").unwrap();
    let env = infer::initial_env();
    let ce = infer::initial_class_env();
    let mut st = infer::InferState {
        supply: typesys::TVarSupply::new(),
        subst: Default::default(),
    };
    assert!(infer::infer_expr(&env, &ce, &mut st, &expr).is_err());
}

#[test]
fn infer_if_branches_must_align() {
    let expr = parser::parse_expr("if True then (1 :: Int) else ('a' :: Char)").unwrap();
    let env = infer::initial_env();
    let ce = infer::initial_class_env();
    let mut st = infer::InferState {
        supply: typesys::TVarSupply::new(),
        subst: Default::default(),
    };
    assert!(infer::infer_expr(&env, &ce, &mut st, &expr).is_err());
}

#[test]
fn infer_if_condition_must_be_bool() {
    let expr = parser::parse_expr("if 'a' then 2 else 3").unwrap();
    let env = infer::initial_env();
    let ce = infer::initial_class_env();
    let mut st = infer::InferState {
        supply: typesys::TVarSupply::new(),
        subst: Default::default(),
    };
    assert!(infer::infer_expr(&env, &ce, &mut st, &expr).is_err());
}
