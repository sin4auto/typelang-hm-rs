// 日本語コメント: 型システムまわりの追加テスト

use typelang::typesys::*;

#[test]
fn unify_con_mismatch_error() {
    let a = Type::TCon(TCon { name: "Int".into() });
    let b = Type::TCon(TCon {
        name: "Bool".into(),
    });
    assert!(unify(a, b).is_err());
}

#[test]
fn pretty_qual_suppresses_irrelevant_constraints() {
    // 戻り値に型変数を含まない場合の制約は表示から除去される（Double）
    let tv = TVar { id: 1 };
    let q = QualType {
        constraints: vec![
            Constraint {
                classname: "Fractional".into(),
                r#type: Type::TVar(tv.clone()),
            },
            Constraint {
                classname: "Num".into(),
                r#type: Type::TVar(tv),
            },
        ],
        r#type: Type::TCon(TCon {
            name: "Double".into(),
        }),
    };
    let s = pretty_qual(&q);
    assert_eq!(s, "Double");
}
