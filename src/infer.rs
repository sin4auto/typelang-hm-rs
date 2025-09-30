// パス: src/infer.rs
// 役割: Hindley–Milner 型推論とクラス制約管理を実装する
// 意図: 式の主型を算出し REPL へフィードバックできるようにする
// 関連ファイル: src/typesys.rs, src/ast.rs, src/evaluator.rs
//! 型推論モジュール
//!
//! - Algorithm W を基盤にしつつ、最小限の型クラス制約を扱う。
//! - 初期環境には演算子や `show` などのスキームを登録し、推論時に再利用する。
//! - `(^)` など特殊挙動を持つ演算子には直感的な型選択（Double へのフォールバック）を提供する。

use crate::ast as A;
use crate::errors::TypeError;
use crate::primitives::{PrimitiveKind, PRIMITIVES};
use crate::typesys::*;

#[derive(Clone, Debug)]
/// 型変数供給源と置換テーブルを束ねる推論ステート。
pub struct InferState {
    pub supply: TVarSupply,
    pub subst: Subst,
}

// `typesys::UnifyError` は既にコードを持つため TypeError へそのまま転送する

/// 標準的な型クラス階層を生成する。
pub fn initial_class_env() -> ClassEnv {
    let mut ce = ClassEnv::default();
    // クラス階層を宣言
    ce.add_class("Eq", std::iter::empty::<&str>());
    ce.add_class("Ord", ["Eq"]);
    ce.add_class("Show", std::iter::empty::<&str>());
    ce.add_class("Num", std::iter::empty::<&str>());
    ce.add_class("Fractional", ["Num"]);
    // 代表的なインスタンスを登録
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

/// 演算子などの既定スキームを備えた型環境を生成する。
pub fn initial_env() -> TypeEnv {
    let mut env = TypeEnv::new();
    let mut supply = TVarSupply::new();

    for def in PRIMITIVES {
        match def.kind {
            PrimitiveKind::Numeric(_) => env.extend(def.name, binop_scheme("Num", &mut supply)),
            PrimitiveKind::FractionalDiv => {
                env.extend(def.name, binop_scheme("Fractional", &mut supply));
            }
            PrimitiveKind::PowFloat => {
                env.extend(def.name, binop_scheme("Fractional", &mut supply));
            }
            PrimitiveKind::PowInt => env.extend(def.name, intpow_scheme(&mut supply)),
            PrimitiveKind::Eq(_) => env.extend(def.name, pred_scheme("Eq", &mut supply)),
            PrimitiveKind::Ord(_) => env.extend(def.name, pred_scheme("Ord", &mut supply)),
            PrimitiveKind::Show => env.extend(def.name, show_scheme(&mut supply)),
        }
    }

    env
}

/// 数値クラス制約を持つ二項演算子スキームを構築する。
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

/// 整数指数を扱う `(^)` 用のスキームを構築する。
fn intpow_scheme(s: &mut TVarSupply) -> Scheme {
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

/// `Eq` / `Ord` 制約を持つ比較演算子スキームを構築する。
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

/// `show` プリミティブのスキームを構築する。
fn show_scheme(s: &mut TVarSupply) -> Scheme {
    let a = Type::TVar(s.fresh());
    let ty = Type::TFun(TFun {
        arg: Box::new(a.clone()),
        ret: Box::new(t_string()),
    });
    let tv = match &a {
        Type::TVar(tv) => tv.clone(),
        _ => unreachable!(),
    };
    Scheme {
        vars: vec![tv],
        qual: qualify(
            ty,
            vec![Constraint {
                classname: "Show".into(),
                r#type: a.clone(),
            }],
        ),
    }
}

/// 式の主型と制約集合を返すトップレベルの推論関数。
pub fn infer_expr(
    env: &TypeEnv,
    ce: &ClassEnv,
    st: &mut InferState,
    e: &A::Expr,
) -> Result<(Subst, QualType), TypeError> {
    let mut ctx = InferCtx { _ce: ce, state: st };
    ctx.infer(env, e)
}

struct InferCtx<'a> {
    _ce: &'a ClassEnv,
    state: &'a mut InferState,
}

impl<'a> InferCtx<'a> {
    fn infer(&mut self, env: &TypeEnv, expr: &A::Expr) -> Result<(Subst, QualType), TypeError> {
        match expr {
            A::Expr::Var { name } => self.infer_var(env, name),
            A::Expr::IntLit { .. } => self.infer_constrained_literal("Num"),
            A::Expr::FloatLit { .. } => self.infer_constrained_literal("Fractional"),
            A::Expr::CharLit { .. } => self.infer_concrete_type(Type::TCon(TCon {
                name: "Char".into(),
            })),
            A::Expr::StringLit { .. } => self.infer_concrete_type(t_string()),
            A::Expr::BoolLit { .. } => self.infer_concrete_type(Type::TCon(TCon {
                name: "Bool".into(),
            })),
            A::Expr::ListLit { items } => self.infer_list(env, items),
            A::Expr::TupleLit { items } => self.infer_tuple(env, items),
            A::Expr::Lambda { params, body } => self.infer_lambda(env, params, body),
            A::Expr::LetIn { bindings, body } => self.infer_let(env, bindings, body),
            A::Expr::If {
                cond,
                then_branch,
                else_branch,
            } => self.infer_if(env, cond, then_branch, else_branch),
            A::Expr::App { func, arg } => self.infer_app(env, func, arg),
            A::Expr::BinOp { op, left, right } => self.infer_binop(env, op, left, right),
            A::Expr::Annot { expr, type_expr } => self.infer_annot(env, expr, type_expr),
        }
    }

    fn infer_var(&mut self, env: &TypeEnv, name: &str) -> Result<(Subst, QualType), TypeError> {
        if name == "_" || name.starts_with('?') {
            let a = Type::TVar(self.state.supply.fresh());
            return Ok((self.state.subst.clone(), qualify(a, vec![])));
        }

        if let Some(sch) = env.lookup(name) {
            let q = instantiate(sch, &mut self.state.supply);
            return Ok((self.state.subst.clone(), q.apply_subst(&self.state.subst)));
        }

        Err(TypeError::new(
            "TYPE010",
            format!("未束縛変数: {name}"),
            None,
        ))
    }

    fn infer_constrained_literal(
        &mut self,
        classname: &str,
    ) -> Result<(Subst, QualType), TypeError> {
        let a = Type::TVar(self.state.supply.fresh());
        Ok((
            self.state.subst.clone(),
            qualify(
                a.clone(),
                vec![Constraint {
                    classname: classname.into(),
                    r#type: a,
                }],
            ),
        ))
    }

    fn infer_concrete_type(&self, ty: Type) -> Result<(Subst, QualType), TypeError> {
        Ok((self.state.subst.clone(), qualify(ty, vec![])))
    }

    fn infer_list(
        &mut self,
        env: &TypeEnv,
        items: &[A::Expr],
    ) -> Result<(Subst, QualType), TypeError> {
        let elem = Type::TVar(self.state.supply.fresh());
        let mut s_acc = self.state.subst.clone();
        for item in items {
            self.state.subst = s_acc.clone();
            let (s_new, q) = self.infer(env, item)?;
            s_acc = s_new;
            let s2 = unify(elem.apply_subst(&s_acc), q.r#type.apply_subst(&s_acc))
                .map_err(|e| TypeError::new(e.code, e.message, None))?;
            s_acc = compose(&s2, &s_acc);
        }
        let ty = t_list(elem.apply_subst(&s_acc));
        Ok((s_acc.clone(), qualify(ty, vec![])))
    }

    fn infer_tuple(
        &mut self,
        env: &TypeEnv,
        items: &[A::Expr],
    ) -> Result<(Subst, QualType), TypeError> {
        let mut s_acc = self.state.subst.clone();
        let mut tys = Vec::with_capacity(items.len());
        for item in items {
            self.state.subst = s_acc.clone();
            let (s_new, q) = self.infer(env, item)?;
            s_acc = s_new;
            tys.push(q.r#type.apply_subst(&s_acc));
        }
        Ok((
            s_acc.clone(),
            qualify(Type::TTuple(TTuple { items: tys }), vec![]),
        ))
    }

    fn infer_lambda(
        &mut self,
        env: &TypeEnv,
        params: &[String],
        body: &A::Expr,
    ) -> Result<(Subst, QualType), TypeError> {
        let mut s_acc = self.state.subst.clone();
        let mut arg_tys: Vec<Type> = Vec::with_capacity(params.len());
        let mut env2 = env.clone_env();

        for _ in params {
            let tv = Type::TVar(self.state.supply.fresh());
            env2.extend(
                format!("$p{}", arg_tys.len()),
                Scheme {
                    vars: vec![],
                    qual: qualify(tv.clone(), vec![]),
                },
            );
            arg_tys.push(tv);
        }

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

        self.state.subst = s_acc.clone();
        let (s_body, q_body) = self.infer(&env2, body)?;
        s_acc = s_body;
        let mut ty = q_body.r#type.clone();
        for arg in arg_tys.iter().rev() {
            ty = Type::TFun(TFun {
                arg: Box::new(arg.apply_subst(&s_acc)),
                ret: Box::new(ty),
            });
        }
        Ok((
            s_acc.clone(),
            qualify(ty.apply_subst(&s_acc), q_body.constraints.clone()),
        ))
    }

    fn infer_let(
        &mut self,
        env: &TypeEnv,
        bindings: &[(String, Vec<String>, A::Expr)],
        body: &A::Expr,
    ) -> Result<(Subst, QualType), TypeError> {
        let mut s_acc = self.state.subst.clone();
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
            self.state.subst = s_acc.clone();
            let (s_new, q_rhs) = self.infer(&env2, &rhs_expr)?;
            s_acc = s_new;
            let sch = generalize(&env2, q_rhs.apply_subst(&s_acc));
            env2.extend(name.clone(), sch);
        }
        self.state.subst = s_acc.clone();
        let (s_body, q_body) = self.infer(&env2, body)?;
        s_acc = s_body;
        Ok((s_acc.clone(), q_body.apply_subst(&s_acc)))
    }

    fn infer_if(
        &mut self,
        env: &TypeEnv,
        cond: &A::Expr,
        then_branch: &A::Expr,
        else_branch: &A::Expr,
    ) -> Result<(Subst, QualType), TypeError> {
        let (s_cond, q_cond) = self.infer(env, cond)?;
        let s_bool = unify(
            q_cond.r#type.apply_subst(&s_cond),
            Type::TCon(TCon {
                name: "Bool".into(),
            }),
        )
        .map_err(|e| TypeError::new(e.code, e.message, None))?;
        let mut s_acc = compose(&s_bool, &s_cond);

        self.state.subst = s_acc.clone();
        let (s_then, q_then) = self.infer(env, then_branch)?;
        s_acc = s_then;

        self.state.subst = s_acc.clone();
        let (s_else, q_else) = self.infer(env, else_branch)?;
        s_acc = s_else;

        let s_merge = unify(
            q_then.r#type.apply_subst(&s_acc),
            q_else.r#type.apply_subst(&s_acc),
        )
        .map_err(|e| TypeError::new(e.code, e.message, None))?;
        let s_acc = compose(&s_merge, &s_acc);

        let q_then_applied = q_then.apply_subst(&s_acc);
        let q_else_applied = q_else.apply_subst(&s_acc);
        let mut cs = q_then_applied.constraints.clone();
        cs.extend(q_else_applied.constraints.clone());
        Ok((
            s_acc.clone(),
            QualType {
                constraints: cs,
                r#type: q_then_applied.r#type,
            },
        ))
    }

    fn infer_app(
        &mut self,
        env: &TypeEnv,
        func: &A::Expr,
        arg: &A::Expr,
    ) -> Result<(Subst, QualType), TypeError> {
        let (s_func, q_func) = self.infer(env, func)?;
        let mut s_acc = s_func;
        self.state.subst = s_acc.clone();
        let (s_arg, q_arg) = self.infer(env, arg)?;
        s_acc = s_arg;
        let result_ty = Type::TVar(self.state.supply.fresh());
        let s_fun = unify(
            q_func.r#type.apply_subst(&s_acc),
            Type::TFun(TFun {
                arg: Box::new(q_arg.r#type.apply_subst(&s_acc)),
                ret: Box::new(result_ty.clone()),
            }),
        )
        .map_err(|e| TypeError::new(e.code, e.message, None))?;
        let s_acc = compose(&s_fun, &s_acc);
        let q_func_applied = q_func.apply_subst(&s_acc);
        let q_arg_applied = q_arg.apply_subst(&s_acc);
        let mut cs = q_func_applied.constraints.clone();
        cs.extend(q_arg_applied.constraints.clone());
        Ok((
            s_acc.clone(),
            QualType {
                constraints: cs,
                r#type: result_ty.apply_subst(&s_acc),
            },
        ))
    }

    fn infer_binop(
        &mut self,
        env: &TypeEnv,
        op: &str,
        left: &A::Expr,
        right: &A::Expr,
    ) -> Result<(Subst, QualType), TypeError> {
        if op == "^" && self.is_negative_exponent(right) {
            let (s_left, q_left) = self.infer(env, left)?;
            let tl = q_left.r#type.apply_subst(&s_left);
            let mut cs = q_left.apply_subst(&s_left).constraints;
            cs.push(Constraint {
                classname: "Fractional".into(),
                r#type: tl,
            });
            return Ok((
                s_left,
                QualType {
                    constraints: cs,
                    r#type: Type::TCon(TCon {
                        name: "Double".into(),
                    }),
                },
            ));
        }

        let applied = A::Expr::App {
            func: Box::new(A::Expr::App {
                func: Box::new(A::Expr::Var {
                    name: op.to_string(),
                }),
                arg: Box::new(left.clone()),
            }),
            arg: Box::new(right.clone()),
        };
        self.infer(env, &applied)
    }

    fn infer_annot(
        &mut self,
        env: &TypeEnv,
        expr: &A::Expr,
        type_expr: &A::TypeExpr,
    ) -> Result<(Subst, QualType), TypeError> {
        let (s_base, q_base) = self.infer(env, expr)?;
        let ty_anno = type_from_texpr(type_expr);
        let s_eq = unify(q_base.r#type.apply_subst(&s_base), ty_anno.clone())
            .map_err(|e| TypeError::new(e.code, e.message, None))?;
        let s_acc = compose(&s_eq, &s_base);
        let q_base_applied = q_base.apply_subst(&s_acc);
        Ok((
            s_acc.clone(),
            QualType {
                constraints: q_base_applied.constraints,
                r#type: ty_anno.apply_subst(&s_acc),
            },
        ))
    }

    fn is_negative_exponent(&mut self, right: &A::Expr) -> bool {
        matches!(
            right,
            A::Expr::BinOp { op, left, right }
                if op == "-"
                    && matches!(**left, A::Expr::IntLit { value: 0, .. })
                    && matches!(**right, A::Expr::IntLit { .. })
        )
    }
}

/// 構文木上の型式を内部の `Type` へ変換する。
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

/// 型変数名を安定した整数 ID に写像する。
fn hash_str(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

/// 単一の式に対する推論結果を文字列表現で返す。
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

/// 既定化のオン/オフを切り替えて推論結果を整形する。
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
