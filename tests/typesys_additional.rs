// Path: tests/typesys_additional.rs
// 役割: typesys モジュールの補完用ユニットテスト
// 意図: 型システムユーティリティのカバレッジ拡充
// 関連ファイル: src/typesys.rs, tests/types_infer.rs, src/infer.rs
use std::collections::HashSet;

use typelang::typesys::*;

#[test]
fn ftv_collects_unique_ids_from_nested_type() {
    let ty = Type::TFun(TFun {
        arg: Box::new(Type::TTuple(TTuple {
            items: vec![
                Type::TVar(TVar { id: 0 }),
                t_list(Type::TVar(TVar { id: 1 })),
            ],
        })),
        ret: Box::new(Type::TApp(TApp {
            func: Box::new(Type::TCon(TCon {
                name: "Maybe".into(),
            })),
            arg: Box::new(Type::TVar(TVar { id: 2 })),
        })),
    });
    let ids = ftv(&ty);
    let expected: HashSet<i64> = HashSet::from([0, 1, 2]);
    assert_eq!(ids, expected);
}

#[test]
fn scheme_apply_subst_respects_bound_variables() {
    let scheme = Scheme {
        vars: vec![TVar { id: 1 }],
        qual: QualType {
            constraints: vec![Constraint {
                classname: "Num".into(),
                r#type: Type::TVar(TVar { id: 0 }),
            }],
            r#type: Type::TFun(TFun {
                arg: Box::new(Type::TVar(TVar { id: 0 })),
                ret: Box::new(Type::TVar(TVar { id: 1 })),
            }),
        },
    };

    let mut subst = Subst::new();
    subst.insert(0, Type::TCon(TCon { name: "Int".into() }));
    subst.insert(
        1,
        Type::TCon(TCon {
            name: "Bool".into(),
        }),
    );

    let applied = scheme.apply_subst(&subst);

    match applied.qual.r#type {
        Type::TFun(TFun { arg, ret }) => {
            match *arg {
                Type::TCon(TCon { ref name }) => assert_eq!(name, "Int"),
                other => panic!("unexpected arg after substitution: {:?}", other),
            }
            match *ret {
                Type::TVar(TVar { id }) => assert_eq!(id, 1),
                other => panic!("bound variable should remain quantified: {:?}", other),
            }
        }
        other => panic!("unexpected type shape: {:?}", other),
    }

    assert!(applied
        .qual
        .constraints
        .iter()
        .all(|c| matches!(c.r#type, Type::TCon(TCon { ref name }) if name == "Int")));
}

#[test]
fn compose_applies_substitutions_in_correct_order() {
    let mut first = Subst::new();
    first.insert(
        0,
        Type::TCon(TCon {
            name: "Bool".into(),
        }),
    );

    let mut second = Subst::new();
    second.insert(1, Type::TVar(TVar { id: 0 }));

    let composed = compose(&first, &second);

    match composed.get(&1) {
        Some(Type::TCon(TCon { name })) => assert_eq!(name, "Bool"),
        other => panic!("id=1 should map to Bool after composition: {:?}", other),
    }
    match composed.get(&0) {
        Some(Type::TCon(TCon { name })) => assert_eq!(name, "Bool"),
        other => panic!(
            "id=0 should be preserved from first substitution: {:?}",
            other
        ),
    }
}

#[test]
fn generalize_and_instantiate_work_together() {
    let mut env = TypeEnv::new();
    env.extend(
        "y",
        Scheme {
            vars: vec![],
            qual: qualify(Type::TVar(TVar { id: 1 }), vec![]),
        },
    );

    let q = qualify(
        Type::TFun(TFun {
            arg: Box::new(Type::TVar(TVar { id: 0 })),
            ret: Box::new(Type::TVar(TVar { id: 1 })),
        }),
        vec![],
    );

    let scheme = generalize(&env, q);
    let quant_ids: Vec<i64> = scheme.vars.iter().map(|tv| tv.id).collect();
    assert_eq!(quant_ids, vec![0]);

    let mut supply = TVarSupply::new();
    let inst = instantiate(&scheme, &mut supply);

    match inst.r#type {
        Type::TFun(TFun { arg, ret }) => {
            match *arg {
                Type::TVar(TVar { id }) => assert_eq!(id, 0),
                other => panic!("argument should become fresh type variable: {:?}", other),
            }
            match *ret {
                Type::TVar(TVar { id }) => assert_eq!(id, 1),
                other => panic!("non-generalized variable should be preserved: {:?}", other),
            }
        }
        other => panic!("unexpected instantiated type: {:?}", other),
    }
}

#[test]
fn class_env_entails_handles_lists_and_tuples() {
    let mut ce = ClassEnv::default();
    ce.add_class("Ord", ["Eq"]);
    ce.add_instance("Eq", "Int");
    ce.add_instance("Eq", "Bool");
    ce.add_instance("Ord", "Int");

    assert!(ce.entails(&[Constraint {
        classname: "Ord".into(),
        r#type: Type::TCon(TCon { name: "Int".into() }),
    }]));

    assert!(ce.entails(&[Constraint {
        classname: "Eq".into(),
        r#type: t_list(Type::TCon(TCon { name: "Int".into() })),
    }]));

    assert!(ce.entails(&[Constraint {
        classname: "Eq".into(),
        r#type: Type::TTuple(TTuple {
            items: vec![
                Type::TCon(TCon { name: "Int".into() }),
                Type::TCon(TCon {
                    name: "Bool".into()
                }),
            ],
        }),
    }]));

    assert!(!ce.entails(&[Constraint {
        classname: "Show".into(),
        r#type: Type::TCon(TCon {
            name: "Bool".into()
        }),
    }]));
}

#[test]
fn apply_defaulting_simple_prefers_fractional_over_num() {
    let q = QualType {
        constraints: vec![
            Constraint {
                classname: "Num".into(),
                r#type: Type::TVar(TVar { id: 0 }),
            },
            Constraint {
                classname: "Fractional".into(),
                r#type: Type::TVar(TVar { id: 0 }),
            },
        ],
        r#type: Type::TVar(TVar { id: 0 }),
    };

    let defaulted = apply_defaulting_simple(&q);

    match defaulted.r#type {
        Type::TCon(TCon { ref name }) => assert_eq!(name, "Double"),
        other => panic!("defaulted type should become Double: {:?}", other),
    }

    assert!(defaulted
        .constraints
        .iter()
        .all(|c| { matches!(c.r#type, Type::TCon(TCon { ref name }) if name == "Double") }));
}

#[test]
fn t_string_builds_char_list() {
    match t_string() {
        Type::TApp(TApp { func, arg }) => {
            match *func {
                Type::TCon(TCon { ref name }) => assert_eq!(name, "[]"),
                other => panic!("String should use list constructor: {:?}", other),
            }
            match *arg {
                Type::TCon(TCon { ref name }) => assert_eq!(name, "Char"),
                other => panic!("String element type should be Char: {:?}", other),
            }
        }
        other => panic!("String helper should build list application: {:?}", other),
    }
}
