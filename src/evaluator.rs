// パス: src/evaluator.rs
// 役割: 言語の評価器と組み込みプリミティブ環境を提供する
// 意図: REPL とテストから式を安全に実行できるようにする
// 関連ファイル: src/ast.rs, src/infer.rs, tests/evaluator.rs
//! 評価モジュール
//!
//! - 正格評価戦略で式を還元し、副作用なしの実装に保つ。
//! - プリミティブ演算は部分適用可能な値として登録し、REPL 操作を簡潔にする。
//! - べき乗や比較など一部演算子は直感的な型へフォールバックする設計を採用する。

use std::collections::HashMap;

use crate::ast as A;
use crate::errors::{EvalError, FrameInfo};
use crate::primitives::PRIMITIVES;
pub use crate::runtime::{Env, Value};

pub fn initial_env() -> Env {
    let mut env: Env = HashMap::new();
    for def in PRIMITIVES {
        env.insert(def.name.into(), def.op.to_value());
    }
    env
}

/// 値を関数として扱い、引数を適用して評価するヘルパ。
fn apply(f: &Value, x: Value) -> Result<Value, EvalError> {
    match f {
        Value::Prim(op) => op.clone().apply(x),
        Value::Closure { params, body, env } => {
            if params.is_empty() {
                return Err(EvalError::new("EVAL090", "関数に引数がありません", None));
            }
            let mut env2 = env.clone();
            env2.insert(params[0].clone(), x);
            if params.len() == 1 {
                eval_expr(body, &mut env2)
            } else {
                Ok(Value::Closure {
                    params: params[1..].to_vec(),
                    body: body.clone(),
                    env: env2,
                })
            }
        }
        _ => Err(EvalError::new(
            "EVAL020",
            "関数適用対象が関数ではありません",
            None,
        )),
    }
}

/// 抽象構文木の式を評価して `Value` へ還元するメインルーチン。
pub fn eval_expr(e: &A::Expr, env: &mut Env) -> Result<Value, EvalError> {
    eval_expr_inner(e, env).map_err(|mut err| {
        attach_frame(&mut err, e);
        err
    })
}

fn eval_expr_inner(e: &A::Expr, env: &mut Env) -> Result<Value, EvalError> {
    use A::Expr::*;
    match e {
        Var { name, .. } => env
            .get(name)
            .cloned()
            .ok_or_else(|| EvalError::new("EVAL010", format!("未束縛変数: {name}"), None)),
        IntLit { value, .. } => Ok(Value::Int(*value)),
        FloatLit { value, .. } => Ok(Value::Double(*value)),
        CharLit { value, .. } => Ok(Value::Char(*value)),
        StringLit { value, .. } => Ok(Value::String(value.clone())),
        BoolLit { value, .. } => Ok(Value::Bool(*value)),
        ListLit { items, .. } => {
            let mut vs = Vec::new();
            for it in items {
                vs.push(eval_expr(it, env)?);
            }
            Ok(Value::List(vs))
        }
        TupleLit { items, .. } => {
            let mut vs = Vec::new();
            for it in items {
                vs.push(eval_expr(it, env)?);
            }
            Ok(Value::Tuple(vs))
        }
        Lambda { params, body, .. } => Ok(Value::Closure {
            params: params.clone(),
            body: body.clone(),
            env: env.clone(),
        }),
        LetIn { bindings, body, .. } => {
            let mut env2 = env.clone();
            for (name, params, rhs) in bindings {
                let val = if params.is_empty() {
                    eval_expr(rhs, &mut env2)?
                } else {
                    Value::Closure {
                        params: params.clone(),
                        body: Box::new(rhs.clone()),
                        env: env2.clone(),
                    }
                };
                env2.insert(name.clone(), val);
            }
            eval_expr(body, &mut env2)
        }
        If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            let c = eval_expr(cond, env)?;
            if let Value::Bool(b) = c {
                if b {
                    eval_expr(then_branch, env)
                } else {
                    eval_expr(else_branch, env)
                }
            } else {
                Err(EvalError::new(
                    "EVAL050",
                    "if 条件は Bool である必要があります",
                    None,
                ))
            }
        }
        Case {
            scrutinee, arms, ..
        } => {
            let value = eval_expr(scrutinee, env)?;
            for arm in arms {
                if let Some(bindings) = match_pattern(&arm.pattern, &value) {
                    let mut env_branch = env.clone();
                    for (name, val) in bindings {
                        env_branch.insert(name, val);
                    }
                    return eval_expr(&arm.body, &mut env_branch);
                }
            }
            Err(EvalError::new(
                "EVAL070",
                "case 式で適用可能な分岐がありません",
                None,
            ))
        }
        App { func, arg, .. } => {
            let f = eval_expr(func, env)?;
            let x = eval_expr(arg, env)?;
            apply(&f, x)
        }
        BinOp {
            op, left, right, ..
        } => {
            let f = eval_expr(
                &A::Expr::Var {
                    name: op.clone(),
                    span: A::Span::dummy(),
                },
                env,
            )?;
            let l = eval_expr(left, env)?;
            let r = eval_expr(right, env)?;
            let tmp = apply(&f, l)?;
            apply(&tmp, r)
        }
        Annot { expr, .. } => eval_expr(expr, env),
    }
}

fn attach_frame(err: &mut EvalError, expr: &A::Expr) {
    let span = expr.span();
    let pos = nonzero(span.pos);
    let line = nonzero(span.line);
    let col = nonzero(span.col);
    let info = err.0.as_mut();
    info.fill_position_if_absent(pos, line, col);
    let summary = format!("{}", expr);
    info.push_frame(FrameInfo::new(summary, pos, line, col));
}

fn nonzero(value: usize) -> Option<usize> {
    if value == 0 {
        None
    } else {
        Some(value)
    }
}

fn merge_bindings(
    mut base: HashMap<String, Value>,
    extra: HashMap<String, Value>,
) -> Option<HashMap<String, Value>> {
    for (k, v) in extra {
        if base.contains_key(&k) {
            return None;
        }
        base.insert(k, v);
    }
    Some(base)
}

fn match_pattern(pattern: &A::Pattern, value: &Value) -> Option<HashMap<String, Value>> {
    match pattern {
        A::Pattern::Wildcard { .. } => Some(HashMap::new()),
        A::Pattern::Var { name, .. } => {
            let mut map = HashMap::new();
            map.insert(name.clone(), value.clone());
            Some(map)
        }
        A::Pattern::Int {
            value: expected, ..
        } => match value {
            Value::Int(v) if v == expected => Some(HashMap::new()),
            _ => None,
        },
        A::Pattern::Bool {
            value: expected, ..
        } => match value {
            Value::Bool(v) if v == expected => Some(HashMap::new()),
            _ => None,
        },
        A::Pattern::Constructor { name, args, .. } => match value {
            Value::Data {
                constructor,
                fields,
            } => {
                if constructor != name || fields.len() != args.len() {
                    return None;
                }
                let mut bindings = HashMap::new();
                for (subpat, field) in args.iter().zip(fields.iter()) {
                    let sub = match_pattern(subpat, field)?;
                    bindings = merge_bindings(bindings, sub)?;
                }
                Some(bindings)
            }
            _ => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Expr, IntBase, Span};

    #[test]
    fn apply_rejects_closure_without_params() {
        let closure = Value::Closure {
            params: Vec::new(),
            body: Box::new(Expr::IntLit {
                value: 0,
                base: IntBase::Dec,
                span: Span::dummy(),
            }),
            env: Env::new(),
        };
        let err = super::apply(&closure, Value::Int(1)).expect_err("missing params must error");
        assert_eq!(err.0.code, "EVAL090");
    }

    #[test]
    fn apply_partially_applies_multi_param_closure() {
        let closure = Value::Closure {
            params: vec!["x".into(), "y".into()],
            body: Box::new(Expr::Var {
                name: "y".into(),
                span: Span::dummy(),
            }),
            env: Env::new(),
        };
        let result = super::apply(&closure, Value::Int(1)).expect("partial application succeeds");
        match result {
            Value::Closure { params, .. } => assert_eq!(params, vec!["y".to_string()]),
            other => panic!("expected closure back, got {:?}", other),
        }
    }

    #[test]
    fn nonzero_filters_zero_and_keeps_positive() {
        assert_eq!(super::nonzero(0), None);
        assert_eq!(super::nonzero(42), Some(42));
    }
}
