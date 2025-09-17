//! 評価器（evaluator）
//!
//! 目的:
//! - 正格・副作用なしの簡易評価器。
//! - プリミティブは二項演算をカリー化して提供し、REPL/テストで扱いやすくする。
//!
//! 仕様要点:
//! - `(^)` は非負整数指数で整数計算、それ以外は `Double` にフォールバック。
//! - 比較は辞書式（リスト/タプル）をサポート。

use std::collections::HashMap;

use crate::ast as A;
use crate::errors::EvalError;

#[derive(Clone, Debug)]
pub enum Value {
    Int(i64),
    Double(f64),
    Bool(bool),
    Char(char),
    String(String),
    List(Vec<Value>),
    Tuple(Vec<Value>),
    Closure {
        params: Vec<String>,
        body: Box<A::Expr>,
        env: Env,
    },
    Prim1(fn(Value) -> Result<Value, EvalError>),
    Prim2(Prim2),
}

#[derive(Clone, Debug)]
pub struct Prim2 {
    pub f: fn(Value, Value) -> Result<Value, EvalError>,
    pub a: Option<Box<Value>>,
}

pub type Env = HashMap<String, Value>;

pub fn initial_env() -> Env {
    let mut env: Env = HashMap::new();
    // show
    fn py_show(v: Value) -> Result<Value, EvalError> {
        Ok(Value::String(match v {
            Value::Int(i) => i.to_string(),
            Value::Double(d) => format!("{}", d),
            Value::Bool(b) => {
                if b {
                    "True".into()
                } else {
                    "False".into()
                }
            }
            Value::Char(c) => c.to_string(),
            Value::String(s) => s,
            _ => return Err(EvalError::new("EVAL050", "show: 未対応の値", None)),
        }))
    }
    env.insert("show".into(), Value::Prim1(py_show));

    // 二項演算（カリー化）
    fn prim2(f: fn(Value, Value) -> Result<Value, EvalError>) -> Value {
        Value::Prim2(Prim2 { f, a: None })
    }
    env.insert(
        "+".into(),
        prim2(|a, b| Ok(Value::Int(to_int(&a)? + to_int(&b)?))),
    );
    env.insert(
        "-".into(),
        prim2(|a, b| Ok(Value::Int(to_int(&a)? - to_int(&b)?))),
    );
    env.insert(
        "*".into(),
        prim2(|a, b| Ok(Value::Int(to_int(&a)? * to_int(&b)?))),
    );
    env.insert(
        "/".into(),
        prim2(|a, b| Ok(Value::Double(to_double(&a)? / to_double(&b)?))),
    );
    // (^) と (**) の仕様
    fn powi(a: Value, b: Value) -> Result<Value, EvalError> {
        match (a, b) {
            (Value::Int(x), Value::Int(y)) if y >= 0 => {
                if y > u32::MAX as i64 {
                    return Err(EvalError::new("EVAL060", "(^) の指数が大きすぎます", None));
                }
                // オーバーフローは利用者へ明示的なエラーとして返す
                x.checked_pow(y as u32).map(Value::Int).ok_or_else(|| {
                    EvalError::new("EVAL060", "(^) の結果が Int の範囲を超えました", None)
                })
            }
            (x, y) => Ok(Value::Double(to_double(&x)?.powf(to_double(&y)?))),
        }
    }
    fn powf(a: Value, b: Value) -> Result<Value, EvalError> {
        Ok(Value::Double(to_double(&a)?.powf(to_double(&b)?)))
    }
    env.insert("^".into(), prim2(powi));
    env.insert("**".into(), prim2(powf));
    // 比較演算（Eq/Ord 相当）
    fn eqv(a: &Value, b: &Value) -> Result<bool, EvalError> {
        Ok(match (a, b) {
            (Value::Int(x), Value::Int(y)) => x == y,
            (Value::Double(x), Value::Double(y)) => x == y,
            (Value::Int(x), Value::Double(y)) => (*x as f64) == *y,
            (Value::Double(x), Value::Int(y)) => *x == (*y as f64),
            (Value::Bool(x), Value::Bool(y)) => x == y,
            (Value::Char(x), Value::Char(y)) => x == y,
            (Value::String(x), Value::String(y)) => x == y,
            (Value::List(xs), Value::List(ys)) => {
                if xs.len() != ys.len() {
                    return Ok(false);
                }
                for (vx, vy) in xs.iter().zip(ys.iter()) {
                    if !eqv(vx, vy)? {
                        return Ok(false);
                    }
                }
                true
            }
            (Value::Tuple(xs), Value::Tuple(ys)) => {
                if xs.len() != ys.len() {
                    return Ok(false);
                }
                let mut ok = true;
                for (vx, vy) in xs.iter().zip(ys.iter()) {
                    if !eqv(vx, vy)? {
                        ok = false;
                        break;
                    }
                }
                ok
            }
            _ => {
                return Err(EvalError::new(
                    "EVAL050",
                    "==: 未対応の型の組み合わせ",
                    None,
                ))
            }
        })
    }
    use std::cmp::Ordering;
    fn compare(a: &Value, b: &Value) -> Result<Ordering, EvalError> {
        match (a, b) {
            (Value::Int(x), Value::Int(y)) => Ok(x.cmp(y)),
            (Value::Double(x), Value::Double(y)) => x
                .partial_cmp(y)
                .ok_or_else(|| EvalError::new("EVAL090", "NaN 比較", None)),
            (Value::Int(x), Value::Double(y)) => (*x as f64)
                .partial_cmp(y)
                .ok_or_else(|| EvalError::new("EVAL090", "NaN 比較", None)),
            (Value::Double(x), Value::Int(y)) => x
                .partial_cmp(&(*y as f64))
                .ok_or_else(|| EvalError::new("EVAL090", "NaN 比較", None)),
            (Value::Bool(x), Value::Bool(y)) => Ok(x.cmp(y)),
            (Value::Char(x), Value::Char(y)) => Ok(x.cmp(y)),
            (Value::String(x), Value::String(y)) => Ok(x.cmp(y)),
            (Value::List(xs), Value::List(ys)) => {
                let n = xs.len().min(ys.len());
                for i in 0..n {
                    let ord = compare(&xs[i], &ys[i])?;
                    if ord != Ordering::Equal {
                        return Ok(ord);
                    }
                }
                Ok(xs.len().cmp(&ys.len()))
            }
            (Value::Tuple(xs), Value::Tuple(ys)) => {
                let n = xs.len().min(ys.len());
                for i in 0..n {
                    let ord = compare(&xs[i], &ys[i])?;
                    if ord != Ordering::Equal {
                        return Ok(ord);
                    }
                }
                Ok(xs.len().cmp(&ys.len()))
            }
            _ => Err(EvalError::new(
                "EVAL050",
                "比較演算: 未対応の型の組み合わせ",
                None,
            )),
        }
    }
    env.insert("==".into(), prim2(|a, b| Ok(Value::Bool(eqv(&a, &b)?))));
    env.insert("/=".into(), prim2(|a, b| Ok(Value::Bool(!eqv(&a, &b)?))));
    env.insert(
        "<".into(),
        prim2(|a, b| Ok(Value::Bool(compare(&a, &b)? == std::cmp::Ordering::Less))),
    );
    env.insert(
        "<=".into(),
        prim2(|a, b| {
            Ok(Value::Bool({
                let o = compare(&a, &b)?;
                o == std::cmp::Ordering::Less || o == std::cmp::Ordering::Equal
            }))
        }),
    );
    env.insert(
        ">".into(),
        prim2(|a, b| Ok(Value::Bool(compare(&a, &b)? == std::cmp::Ordering::Greater))),
    );
    env.insert(
        ">=".into(),
        prim2(|a, b| {
            Ok(Value::Bool({
                let o = compare(&a, &b)?;
                o == std::cmp::Ordering::Greater || o == std::cmp::Ordering::Equal
            }))
        }),
    );
    env.insert("map".into(), prim2(map_prim));
    env.insert("foldl".into(), prim2(foldl_impl));
    env.insert("foldr".into(), prim2(foldr_impl));
    env
}

fn map_prim(fun: Value, xs: Value) -> Result<Value, EvalError> {
    match xs {
        Value::List(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                let mapped = apply(&fun, item)?;
                out.push(mapped);
            }
            Ok(Value::List(out))
        }
        _ => Err(EvalError::new(
            "EVAL050",
            "map: リスト以外に適用できません",
            None,
        )),
    }
}

fn foldl_impl(fun: Value, acc: Value) -> Result<Value, EvalError> {
    Ok(Value::Prim2(Prim2 {
        f: foldl_finish,
        a: Some(Box::new(Value::Tuple(vec![fun, acc]))),
    }))
}

fn foldl_finish(state: Value, xs: Value) -> Result<Value, EvalError> {
    let (fun, acc0) = unpack_pair(state, "foldl");
    match xs {
        Value::List(items) => {
            let mut acc = acc0;
            for item in items {
                let next = apply(&fun, acc)?;
                acc = apply(&next, item)?;
            }
            Ok(acc)
        }
        _ => Err(EvalError::new(
            "EVAL050",
            "foldl: リスト以外に適用できません",
            None,
        )),
    }
}

fn foldr_impl(fun: Value, acc: Value) -> Result<Value, EvalError> {
    Ok(Value::Prim2(Prim2 {
        f: foldr_finish,
        a: Some(Box::new(Value::Tuple(vec![fun, acc]))),
    }))
}

fn foldr_finish(state: Value, xs: Value) -> Result<Value, EvalError> {
    let (fun, acc0) = unpack_pair(state, "foldr");
    match xs {
        Value::List(items) => {
            let mut acc = acc0;
            for item in items.into_iter().rev() {
                let step = apply(&fun, item)?;
                acc = apply(&step, acc)?;
            }
            Ok(acc)
        }
        _ => Err(EvalError::new(
            "EVAL050",
            "foldr: リスト以外に適用できません",
            None,
        )),
    }
}

fn unpack_pair(state: Value, label: &str) -> (Value, Value) {
    if let Value::Tuple(items) = state {
        if items.len() == 2 {
            let mut iter = items.into_iter();
            let first = iter.next().expect("内部タプルが存在する必要があります");
            let second = iter.next().expect("内部タプルが存在する必要があります");
            return (first, second);
        }
    }
    panic!("{}: 内部状態が不正です", label);
}

fn apply(f: &Value, x: Value) -> Result<Value, EvalError> {
    match f {
        Value::Prim1(g) => g(x),
        Value::Prim2(p) => {
            if let Some(a) = &p.a {
                (p.f)((*a.clone()).clone(), x)
            } else {
                Ok(Value::Prim2(Prim2 {
                    f: p.f,
                    a: Some(Box::new(x)),
                }))
            }
        }
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

fn to_int(v: &Value) -> Result<i64, EvalError> {
    match v {
        Value::Int(i) => Ok(*i),
        Value::Double(d) => Ok(*d as i64),
        _ => Err(EvalError::new("EVAL050", "Int 変換に失敗", None)),
    }
}
fn to_double(v: &Value) -> Result<f64, EvalError> {
    match v {
        Value::Double(d) => Ok(*d),
        Value::Int(i) => Ok(*i as f64),
        _ => Err(EvalError::new("EVAL050", "Double 変換に失敗", None)),
    }
}

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
