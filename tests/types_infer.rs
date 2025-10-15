// パス: tests/types_infer.rs
// 役割: 型システムと推論ロジックの最小カバレッジ
// 意図: unify・defaulting・型推論の重要挙動を簡潔に検証する
// 関連ファイル: src/infer.rs, src/typesys.rs, tests/evaluator.rs
#[path = "test_support.rs"]
mod support;

use support::{eval_value, infer_pretty_qual, infer_type_str, infer_type_str_with_defaulting};
use typelang::typesys::*;
use typelang::{evaluator, typesys};

#[test]
/// unify の成功・失敗を代表ケースで検証する。
fn unify_cases() {
    let int = Type::TCon(TCon { name: "Int".into() });
    let bool_ty = Type::TCon(TCon {
        name: "Bool".into(),
    });
    let fun = Type::TFun(TFun {
        arg: Box::new(int.clone()),
        ret: Box::new(int.clone()),
    });
    assert!(unify(fun.clone(), fun.clone()).is_ok(), "同型 unify は成功");

    let tv = TVar { id: 1 };
    let recursive_fun = Type::TFun(TFun {
        arg: Box::new(Type::TVar(tv.clone())),
        ret: Box::new(int.clone()),
    });
    assert!(
        unify(Type::TVar(tv.clone()), recursive_fun).is_err(),
        "occurs check が発火"
    );

    assert!(
        unify(int.clone(), bool_ty.clone()).is_err(),
        "異なる型コンストラクタは失敗"
    );
}

#[test]
/// pretty_qual が不要な制約を抑制することを検証する。
fn pretty_qual_suppresses_irrelevant_constraints() {
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
/// 推論結果の文字列表現をテーブルで検証する。
fn inference_type_strings() {
    struct Case<'a> {
        src: &'a str,
        expected: &'a str,
        note: &'a str,
    }

    let cases = [
        Case {
            src: "\\x -> x == x",
            expected: "Eq a => a -> Bool",
            note: "Eq 制約",
        },
        Case {
            src: "\\x -> x + 1",
            expected: "Num a => a -> a",
            note: "Num 制約",
        },
        Case {
            src: "1 :: Bool",
            expected: "Bool",
            note: "型注釈",
        },
        Case {
            src: "2 ^ -3",
            expected: "Double",
            note: "負整数指数の累乗",
        },
        Case {
            src: "div 4 2",
            expected: "Integer",
            note: "div は整数を返す",
        },
        Case {
            src: "case True of True -> False; False -> True",
            expected: "Bool",
            note: "case 式の分岐推論",
        },
        Case {
            src: "case [1] of [x] -> x; _ -> 0",
            expected: "Num a => a",
            note: "リストパターンと数値制約",
        },
    ];

    for case in cases {
        assert_eq!(infer_type_str(case.src), case.expected, "{}", case.note);
    }
}

#[test]
/// defaulting の有無による違いを確認する。
fn inference_defaulting_behaviour() {
    assert_eq!(infer_type_str_with_defaulting("2 ** -1", true), "Double");
    assert_eq!(infer_type_str_with_defaulting("1 + 2", false), "Num a => a");

    let defaulted = infer_type_str_with_defaulting("show 1", true);
    assert!(defaulted == "String" || defaulted == "[Char]");
}

#[test]
/// 推論失敗ケースをまとめて検証する。
fn inference_error_cases() {
    for src in [
        "foo",
        "if True then (1 :: Int) else ('a' :: Char)",
        "if 'a' then 2 else 3",
        "case 1 of x | x -> x; _ -> 0",
    ] {
        assert!(
            infer_pretty_qual(src).is_err(),
            "型エラーが検出されない: {src}"
        );
    }
}

#[test]
/// let 多相が評価でも利用できることを検証する。
fn infer_let_polymorphism_eval_ok() {
    let value = eval_value("let id x = x in (id 1, id True)");
    match value {
        evaluator::Value::Tuple(items) => {
            assert!(matches!(items[0], evaluator::Value::Int(1)));
            assert!(matches!(items[1], evaluator::Value::Bool(true)));
        }
        other => panic!("unexpected value: {:?}", other),
    }
}

#[test]
/// 相互再帰する let 束縛が型推論と評価で利用できることを確認する。
fn infer_and_eval_mutual_recursion_functions() {
    let src_fn = r#"
        let even n = if n == 0 then True else odd (n - 1);
            odd n = if n == 0 then False else even (n - 1)
        in even
    "#;
    let ty = infer_type_str(src_fn.trim());
    assert!(
        matches!(
            ty.as_str(),
            "Integer -> Bool" | "Int -> Bool" | "Num a => a -> Bool"
        ),
        "unexpected type: {ty}"
    );

    let value = eval_value(
        r#"
        let even n = if n == 0 then True else odd (n - 1);
            odd n = if n == 0 then False else even (n - 1)
        in even 6
        "#
        .trim(),
    );
    assert!(
        matches!(value, evaluator::Value::Bool(true)),
        "even 6 should be True"
    );
    let value_odd = eval_value(
        r#"
        let even n = if n == 0 then True else odd (n - 1);
            odd n = if n == 0 then False else even (n - 1)
        in odd 7
        "#
        .trim(),
    );
    assert!(
        matches!(value_odd, evaluator::Value::Bool(true)),
        "odd 7 should be True"
    );
}
