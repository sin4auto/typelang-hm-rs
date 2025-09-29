// パス: src/infer.rs
// 役割: Hindley–Milner inference engine with lightweight class constraints
// 意図: Deduce principal types for expressions and REPL operations
// 関連ファイル: src/typesys.rs, src/ast.rs, src/evaluator.rs
//! 型推論（infer）
//!
//! 目的:
//! - Algorithm W をベースに、最小限の型クラス制約を加えた推論を提供する。
//!
//! 概要:
//! - `initial_env` に基本演算子のスキームを定義。
//! - 制約は `typesys::Constraint` を蓄積し、表示時に必要最小に整形する。
//! - `(^)` の負指数は Float へフォールバックする特例を設け、直感的な挙動を維持。

use crate::ast as A;
use crate::errors::TypeError;
use crate::typesys::*;

#[derive(Clone, Debug)]
/// 型推論で使う型変数サプライと置換を保持する状態。
pub struct InferState {
    pub supply: TVarSupply,
    pub subst: Subst,
}

// UnifyError はコード付きのため、そのまま TypeError へ移送

/// 標準の型クラス階層を初期化する。
pub fn initial_class_env() -> ClassEnv {
    let mut ce = ClassEnv::default();
    // クラス階層
    ce.add_class("Eq", std::iter::empty::<&str>());
    ce.add_class("Ord", ["Eq"]);
    ce.add_class("Show", std::iter::empty::<&str>());
    ce.add_class("Num", std::iter::empty::<&str>());
    ce.add_class("Fractional", ["Num"]);
    // インスタンス
    for ty in ["Int", "Integer", "Double", "Char", "Bool"] {
        ce.add_instance("Eq", ty);
        ce.add_instance("Ord", ty);
        ce.add_instance("Show", ty);
    }
    for ty in ["Int", "Integer", "Double"] {
        ce.add_instance("Num", ty);
    }
    ce.add_instance("Fractional", "Double");
    // String = [Char]
    ce.add_instance("Eq", "[Char]");
    ce.add_instance("Ord", "[Char]");
    ce.add_instance("Show", "[Char]");
    ce
}

/// 初期の型環境を構築する。
pub fn initial_env() -> TypeEnv {
    let mut env = TypeEnv::new();
    let mut s = TVarSupply::new();
    // (+), (-), (*) :: Num a => a -> a -> a
    /// 二項演算子用の型スキームを生成する。
    fn binop_scheme(cls: &str, s: &mut TVarSupply) -> Scheme {
        let a = Type::TVar(s.fresh());
        let ty = Type::TFun(TFun {
            arg: Box::new(a.clone()),
            ret: Box::new(Type::TFun(TFun {
                arg: Box::new(a.clone()),
                ret: Box::new(a.clone()),
            })),
        });
        let q = qualify(
            ty,
            vec![Constraint {
                classname: cls.into(),
                r#type: a.clone(),
            }],
        );
        let tv = match &a {
            Type::TVar(tv) => tv.clone(),
            _ => unreachable!(),
        };
        Scheme {
            vars: vec![tv],
            qual: q,
        }
    }
    /// Fractional制約を持つ演算子の型スキームを生成する。
    fn frlop_scheme(s: &mut TVarSupply) -> Scheme {
        let a = Type::TVar(s.fresh());
        let ty = Type::TFun(TFun {
            arg: Box::new(a.clone()),
            ret: Box::new(Type::TFun(TFun {
                arg: Box::new(a.clone()),
                ret: Box::new(a.clone()),
            })),
        });
        let q = qualify(
            ty,
            vec![Constraint {
                classname: "Fractional".into(),
                r#type: a.clone(),
            }],
        );
        let tv = match &a {
            Type::TVar(tv) => tv.clone(),
            _ => unreachable!(),
        };
        Scheme {
            vars: vec![tv],
            qual: q,
        }
    }
    /// 整数累乗演算の型スキームを生成する。
    fn intpow_scheme(s: &mut TVarSupply) -> Scheme {
        // (^) :: Num a => a -> Int -> a
        let a = Type::TVar(s.fresh());
        let ty = Type::TFun(TFun {
            arg: Box::new(a.clone()),
            ret: Box::new(Type::TFun(TFun {
                arg: Box::new(Type::TCon(TCon { name: "Int".into() })),
                ret: Box::new(a.clone()),
            })),
        });
        let q = qualify(
            ty,
            vec![Constraint {
                classname: "Num".into(),
                r#type: a.clone(),
            }],
        );
        let tv = match &a {
            Type::TVar(tv) => tv.clone(),
            _ => unreachable!(),
        };
        Scheme {
            vars: vec![tv],
            qual: q,
        }
    }

    env.extend("+", binop_scheme("Num", &mut s));
    env.extend("-", binop_scheme("Num", &mut s));
    env.extend("*", binop_scheme("Num", &mut s));
    env.extend("/", frlop_scheme(&mut s));
    env.extend("^", intpow_scheme(&mut s));
    env.extend("**", frlop_scheme(&mut s));
    // 比較演算: Eq/Ord a => a -> a -> Bool
    /// 比較演算子用の型スキームを生成する。
    fn pred_scheme(cls: &str, s: &mut TVarSupply) -> Scheme {
        let a = Type::TVar(s.fresh());
        let ty = Type::TFun(TFun {
            arg: Box::new(a.clone()),
            ret: Box::new(Type::TFun(TFun {
                arg: Box::new(a.clone()),
                ret: Box::new(Type::TCon(TCon {
                    name: "Bool".into(),
                })),
            })),
        });
        let q = qualify(
            ty,
            vec![Constraint {
                classname: cls.into(),
                r#type: a.clone(),
            }],
        );
        let tv = match &a {
            Type::TVar(tv) => tv.clone(),
            _ => unreachable!(),
        };
        Scheme {
            vars: vec![tv],
            qual: q,
        }
    }
    env.extend("==", pred_scheme("Eq", &mut s));
    env.extend("/=", pred_scheme("Eq", &mut s));
    env.extend("<", pred_scheme("Ord", &mut s));
    env.extend("<=", pred_scheme("Ord", &mut s));
    env.extend(">", pred_scheme("Ord", &mut s));
    env.extend(">=", pred_scheme("Ord", &mut s));
    // show :: Show a => a -> String
    let a = Type::TVar(s.fresh());
    let show_ty = Type::TFun(TFun {
        arg: Box::new(a.clone()),
        ret: Box::new(t_string()),
    });
    let tv = match &a {
        Type::TVar(tv) => tv.clone(),
        _ => unreachable!(),
    };
    env.extend(
        "show",
        Scheme {
            vars: vec![tv],
            qual: qualify(
                show_ty,
                vec![Constraint {
                    classname: "Show".into(),
                    r#type: a.clone(),
                }],
            ),
        },
    );
    env
}

/// 式の主型と制約を推論する。
pub fn infer_expr(
    env: &TypeEnv,
    _ce: &ClassEnv,
    st: &mut InferState,
    e: &A::Expr,
) -> Result<(Subst, QualType), TypeError> {
    match e {
        A::Expr::Var { name } => {
            if name == "_" || name.starts_with('?') {
                let a = Type::TVar(st.supply.fresh());
                Ok((st.subst.clone(), qualify(a, vec![])))
            } else if let Some(sch) = env.lookup(name) {
                let q = instantiate(sch, &mut st.supply);
                Ok((st.subst.clone(), apply_subst_q(&st.subst, &q)))
            } else {
                Err(TypeError::new(
                    "TYPE010",
                    format!("未束縛変数: {name}"),
                    None,
                ))
            }
        }
        A::Expr::IntLit { .. } => {
            let a = Type::TVar(st.supply.fresh());
            Ok((
                st.subst.clone(),
                qualify(
                    a.clone(),
                    vec![Constraint {
                        classname: "Num".into(),
                        r#type: a,
                    }],
                ),
            ))
        }
        A::Expr::FloatLit { .. } => {
            let a = Type::TVar(st.supply.fresh());
            Ok((
                st.subst.clone(),
                qualify(
                    a.clone(),
                    vec![Constraint {
                        classname: "Fractional".into(),
                        r#type: a,
                    }],
                ),
            ))
        }
        A::Expr::CharLit { .. } => Ok((
            st.subst.clone(),
            qualify(
                Type::TCon(TCon {
                    name: "Char".into(),
                }),
                vec![],
            ),
        )),
        A::Expr::StringLit { .. } => Ok((st.subst.clone(), qualify(t_string(), vec![]))),
        A::Expr::BoolLit { .. } => Ok((
            st.subst.clone(),
            qualify(
                Type::TCon(TCon {
                    name: "Bool".into(),
                }),
                vec![],
            ),
        )),
        A::Expr::ListLit { items } => {
            let a = Type::TVar(st.supply.fresh());
            let mut s = st.subst.clone();
            for it in items {
                st.subst = s.clone();
                let (s_new, q) = infer_expr(env, _ce, st, it)?;
                s = s_new;
                let s2 = unify(apply_subst_t(&s, &a), apply_subst_t(&s, &q.r#type))
                    .map_err(|e| TypeError::new(e.code, e.message, None))?;
                s = compose(&s2, &s);
            }
            Ok((s.clone(), qualify(t_list(apply_subst_t(&s, &a)), vec![])))
        }
        A::Expr::TupleLit { items } => {
            let mut s = st.subst.clone();
            let mut tys: Vec<Type> = Vec::new();
            for it in items {
                st.subst = s.clone();
                let (s_new, q) = infer_expr(env, _ce, st, it)?;
                s = s_new;
                tys.push(apply_subst_t(&s, &q.r#type));
            }
            Ok((
                s.clone(),
                qualify(Type::TTuple(TTuple { items: tys }), vec![]),
            ))
        }
        A::Expr::Lambda { params, body } => {
            let mut s = st.subst.clone();
            let mut arg_tys: Vec<Type> = Vec::new();
            let mut env2 = env.clone_env();
            for _ in params {
                let tv = Type::TVar(st.supply.fresh());
                env2.extend(
                    format!("$p{}", arg_tys.len()),
                    Scheme {
                        vars: vec![],
                        qual: qualify(tv.clone(), vec![]),
                    },
                );
                arg_tys.push(tv);
            }
            // env2 に本来はパラメータ名で束縛する
            for (idx, name) in params.iter().enumerate() {
                if let Type::TVar(tv) = &arg_tys[idx] {
                    env2.env.insert(
                        name.clone(),
                        Scheme {
                            vars: vec![],
                            qual: qualify(Type::TVar(tv.clone()), vec![]),
                        },
                    );
                }
            }
            st.subst = s.clone();
            let (s2, q_body) = infer_expr(&env2, _ce, st, body)?;
            s = s2;
            let mut t = q_body.r#type.clone();
            for t_arg in arg_tys.iter().rev() {
                t = Type::TFun(TFun {
                    arg: Box::new(apply_subst_t(&s, t_arg)),
                    ret: Box::new(t),
                });
            }
            Ok((
                s.clone(),
                qualify(apply_subst_t(&s, &t), q_body.constraints.clone()),
            ))
        }
        A::Expr::LetIn { bindings, body } => {
            let mut s = st.subst.clone();
            let mut env2 = env.clone_env();
            for (name, params, rhs) in bindings {
                let rhs_expr = if params.is_empty() {
                    rhs.clone()
                } else {
                    A::Expr::Lambda {
                        params: params.clone(),
                        body: Box::new(rhs.clone()),
                    }
                };
                st.subst = s.clone();
                let (s_new, q_rhs) = infer_expr(&env2, _ce, st, &rhs_expr)?;
                s = s_new;
                let sch = generalize(&env2, apply_subst_q(&s, &q_rhs));
                env2.extend(name.clone(), sch);
            }
            st.subst = s.clone();
            let (s_body, q_body) = infer_expr(&env2, _ce, st, body)?;
            s = s_body;
            Ok((s.clone(), apply_subst_q(&s, &q_body)))
        }
        A::Expr::If {
            cond,
            then_branch,
            else_branch,
        } => {
            let (s1, q_c) = infer_expr(env, _ce, st, cond)?;
            let s = s1; // cond
            let s2 = unify(
                apply_subst_t(&s, &q_c.r#type),
                Type::TCon(TCon {
                    name: "Bool".into(),
                }),
            )
            .map_err(|e| TypeError::new(e.code, e.message, None))?;
            let mut s = compose(&s2, &s);
            st.subst = s.clone();
            let (s_t, q_t) = infer_expr(env, _ce, st, then_branch)?;
            s = s_t;
            st.subst = s.clone();
            let (s_e, q_e) = infer_expr(env, _ce, st, else_branch)?;
            s = s_e;
            let s3 = unify(
                apply_subst_t(&s, &q_t.r#type),
                apply_subst_t(&s, &q_e.r#type),
            )
            .map_err(|e| TypeError::new(e.code, e.message, None))?;
            let s = compose(&s3, &s);
            let mut cs = apply_subst_q(&s, &q_t).constraints;
            cs.extend(apply_subst_q(&s, &q_e).constraints);
            Ok((
                s.clone(),
                QualType {
                    constraints: cs,
                    r#type: apply_subst_t(&s, &q_t.r#type),
                },
            ))
        }
        A::Expr::App { func, arg } => {
            let (s_f, q_f) = infer_expr(env, _ce, st, func)?;
            let mut s = s_f;
            st.subst = s.clone();
            let (s_x, q_x) = infer_expr(env, _ce, st, arg)?;
            s = s_x;
            let a = Type::TVar(st.supply.fresh());
            let s2 = unify(
                apply_subst_t(&s, &q_f.r#type),
                Type::TFun(TFun {
                    arg: Box::new(apply_subst_t(&s, &q_x.r#type)),
                    ret: Box::new(a.clone()),
                }),
            )
            .map_err(|e| TypeError::new(e.code, e.message, None))?;
            let s = compose(&s2, &s);
            let mut cs = apply_subst_q(&s, &q_f).constraints;
            cs.extend(apply_subst_q(&s, &q_x).constraints);
            Ok((
                s.clone(),
                QualType {
                    constraints: cs,
                    r#type: apply_subst_t(&s, &a),
                },
            ))
        }
        A::Expr::BinOp { op, left, right } => {
            // 負の指数の場合: '^' は Double にフォールバック
            if op == "^" {
                // 右辺が -n の糖衣（0 - n）かどうかを判定
                let is_neg = matches!(
                    &**right,
                    A::Expr::BinOp { op: op2, left: l2, right: r2 }
                        if op2 == "-"
                        && matches!((&**l2, &**r2), (
                            A::Expr::IntLit { value: 0, .. },
                            A::Expr::IntLit { .. }
                        ))
                );
                if is_neg {
                    // 左辺の型を推論し、Fractional 制約を付与して戻り型 Double とする
                    let (s_l, q_l) = infer_expr(env, _ce, st, left)?;
                    let s = s_l;
                    let tl = apply_subst_t(&s, &q_l.r#type);
                    let mut cs = apply_subst_q(&s, &q_l).constraints;
                    cs.push(Constraint {
                        classname: "Fractional".into(),
                        r#type: tl,
                    });
                    return Ok((
                        s,
                        QualType {
                            constraints: cs,
                            r#type: Type::TCon(TCon {
                                name: "Double".into(),
                            }),
                        },
                    ));
                }
            }
            let f = A::Expr::App {
                func: Box::new(A::Expr::App {
                    func: Box::new(A::Expr::Var { name: op.clone() }),
                    arg: left.clone(),
                }),
                arg: right.clone(),
            };
            infer_expr(env, _ce, st, &f)
        }
        A::Expr::Annot { expr, type_expr } => {
            let (s0, q) = infer_expr(env, _ce, st, expr)?;
            let s = s0;
            let ty_anno = type_from_texpr(type_expr);
            let s2 = unify(apply_subst_t(&s, &q.r#type), ty_anno.clone())
                .map_err(|e| TypeError::new(e.code, e.message, None))?;
            let s = compose(&s2, &s);
            Ok((
                s.clone(),
                QualType {
                    constraints: apply_subst_q(&s, &q).constraints,
                    r#type: apply_subst_t(&s, &ty_anno),
                },
            ))
        }
    }
}

/// 構文木上の型式を内部の型表現へ変換する。
pub fn type_from_texpr(te: &A::TypeExpr) -> Type {
    match te {
        A::TypeExpr::TEVar(name) => {
            let first = name.chars().next().unwrap_or('a');
            if first.is_ascii_uppercase() {
                Type::TCon(TCon { name: name.clone() })
            } else {
                let id = (hash_str(name) % 1_000_000_000) as i64;
                Type::TVar(TVar { id })
            }
        }
        A::TypeExpr::TECon(name) => {
            if name == "String" {
                t_string()
            } else {
                Type::TCon(TCon { name: name.clone() })
            }
        }
        A::TypeExpr::TEApp(f, a) => Type::TApp(TApp {
            func: Box::new(type_from_texpr(f)),
            arg: Box::new(type_from_texpr(a)),
        }),
        A::TypeExpr::TEFun(a, b) => Type::TFun(TFun {
            arg: Box::new(type_from_texpr(a)),
            ret: Box::new(type_from_texpr(b)),
        }),
        A::TypeExpr::TEList(inner) => t_list(type_from_texpr(inner)),
        A::TypeExpr::TETuple(items) => Type::TTuple(TTuple {
            items: items.iter().map(type_from_texpr).collect(),
        }),
    }
}

/// 型変数名から安定したハッシュ値を生成する。
fn hash_str(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

/// 単一の式に対する推論結果を文字列で返す。
pub fn infer_type_str(expr: &A::Expr) -> Result<String, TypeError> {
    let env = initial_env();
    let ce = initial_class_env();
    let mut st = InferState {
        supply: TVarSupply::new(),
        subst: Subst::new(),
    };
    let (_s, q) = infer_expr(&env, &ce, &mut st, expr)?;
    Ok(pretty_qual(&q))
}

/// 既定化の有無を切替ながら推論結果を整形する。
pub fn infer_type_str_with_defaulting(
    expr: &A::Expr,
    defaulting_on: bool,
) -> Result<String, TypeError> {
    let env = initial_env();
    let ce = initial_class_env();
    let mut st = InferState {
        supply: TVarSupply::new(),
        subst: Subst::new(),
    };
    let (_s, q) = infer_expr(&env, &ce, &mut st, expr)?;
    if defaulting_on {
        let qd = crate::typesys::apply_defaulting_simple(&q);
        Ok(pretty_qual(&qd))
    } else {
        Ok(pretty_qual(&q))
    }
}
