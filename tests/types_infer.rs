// パス: tests/types_infer.rs
// 役割: 型システムと推論ロジックの最小カバレッジ
// 意図: unify・defaulting・型推論の重要挙動を簡潔に検証する
// 関連ファイル: src/infer.rs, src/typesys.rs, tests/evaluator.rs
use typelang::{evaluator, infer, parser, typesys};

/// 式の主型を文字列として推論するヘルパ。
fn infer_type(src: &str) -> String {
    let expr = parser::parse_expr(src).expect("parse");
    infer::infer_type_str(&expr).expect("infer")
}

/// 既定化の有無を指定して推論するヘルパ。
fn infer_type_with_defaulting(src: &str, enable: bool) -> String {
    let expr = parser::parse_expr(src).expect("parse");
    infer::infer_type_str_with_defaulting(&expr, enable).expect("infer")
}

/// 式を推論して `pretty_qual` の文字列表現を取得するヘルパ。
fn infer_result(src: &str) -> Result<String, typelang::errors::TypeError> {
    let expr = parser::parse_expr(src).expect("parse");
    let env = infer::initial_env();
    let ce = infer::initial_class_env();
    let mut st = infer::InferState {
        supply: typesys::TVarSupply::new(),
        subst: Default::default(),
    };
    infer::infer_expr(&env, &ce, &mut st, &expr).map(|(_, q)| typesys::pretty_qual(&q))
}

#[test]
/// 同一の関数型を単一化できることを検証する。
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
/// オカーズチェックで単一化が失敗することを検証する。
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
/// 型コンストラクタの不一致がエラーになることを検証する。
fn unify_constructor_mismatch_is_error() {
    use typesys::*;
    let a = Type::TCon(TCon { name: "Int".into() });
    let b = Type::TCon(TCon {
        name: "Bool".into(),
    });
    assert!(unify(a, b).is_err());
}

#[test]
/// `pretty_qual` が不要な制約を抑制することを検証する。
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
/// `==` を含むラムダに Eq 制約が付与されることを検証する。
fn infer_lambda_eq_has_eq_constraint() {
    assert_eq!(infer_type("\\x -> x == x"), "Eq a => a -> Bool");
}

#[test]
/// 加算を含むラムダに Num 制約が付与されることを検証する。
fn infer_lambda_num_has_num_constraint() {
    assert_eq!(infer_type("\\x -> x + 1"), "Num a => a -> a");
}

#[test]
/// 数値に Bool 注釈を付けると Bool が返ることを確認する。
fn infer_annotation_on_num_to_bool_shows_bool() {
    assert_eq!(infer_type("1 :: Bool"), "Bool");
}

#[test]
/// 負の整数指数の累乗が Double になることを検証する。
fn infer_pow_negative_int_yields_double() {
    assert_eq!(infer_type("2 ^ -3"), "Double");
}

#[test]
/// `**` が既定化によって Double になることを検証する。
fn infer_starstar_defaulted_is_double() {
    assert_eq!(infer_type_with_defaulting("2 ** -1", true), "Double");
}

#[test]
/// defaulting を無効にすると Num 制約が維持されることを検証する。
fn infer_add_without_defaulting_keeps_constraint() {
    assert_eq!(infer_type_with_defaulting("1 + 2", false), "Num a => a");
}

#[test]
/// `show` の defaulting 挙動を確認する。
fn infer_defaulting_controls_show_constraints() {
    let txt = infer_result("show 1").expect("infer show 1");
    assert!(txt.ends_with("[Char]") || txt.ends_with("String"));

    let defaulted = infer_type_with_defaulting("show 1", true);
    assert!(defaulted == "String" || defaulted == "[Char]");
}

#[test]
/// let 多相が評価でも利用できることを検証する。
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
/// 未定義変数が型エラーになることを検証する。
fn infer_unknown_variable_is_error() {
    assert!(infer_result("foo").is_err());
}

#[test]
/// if の分岐で型が一致しないとエラーになることを検証する。
fn infer_if_branches_must_align() {
    assert!(infer_result("if True then (1 :: Int) else ('a' :: Char)").is_err());
}

#[test]
/// if 条件が Bool でないとエラーになることを検証する。
fn infer_if_condition_must_be_bool() {
    assert!(infer_result("if 'a' then 2 else 3").is_err());
}
