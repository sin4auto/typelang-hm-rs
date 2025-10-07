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
use crate::errors::EvalError;
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
    use A::Expr::*;
    match e {
        Var { name } => env
            .get(name)
            .cloned()
            .ok_or_else(|| EvalError::new("EVAL010", format!("未束縛変数: {name}"), None)),
        IntLit { value, .. } => Ok(Value::Int(*value)),
        FloatLit { value } => Ok(Value::Double(*value)),
        CharLit { value } => Ok(Value::Char(*value)),
        StringLit { value } => Ok(Value::String(value.clone())),
        BoolLit { value } => Ok(Value::Bool(*value)),
        ListLit { items } => {
            let mut vs = Vec::new();
            for it in items {
                vs.push(eval_expr(it, env)?);
            }
            Ok(Value::List(vs))
        }
        TupleLit { items } => {
            let mut vs = Vec::new();
            for it in items {
                vs.push(eval_expr(it, env)?);
            }
            Ok(Value::Tuple(vs))
        }
        Lambda { params, body } => Ok(Value::Closure {
            params: params.clone(),
            body: body.clone(),
            env: env.clone(),
        }),
        LetIn { bindings, body } => {
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
        App { func, arg } => {
            let f = eval_expr(func, env)?;
            let x = eval_expr(arg, env)?;
            apply(&f, x)
        }
        BinOp { op, left, right } => {
            let f = eval_expr(&A::Expr::Var { name: op.clone() }, env)?;
            let l = eval_expr(left, env)?;
            let r = eval_expr(right, env)?;
            let tmp = apply(&f, l)?;
            apply(&tmp, r)
        }
        Annot { expr, .. } => eval_expr(expr, env),
    }
}
