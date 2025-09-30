// パス: src/evaluator.rs
// 役割: 言語の評価器と組み込みプリミティブ環境を提供する
// 意図: REPL とテストから式を安全に実行できるようにする
// 関連ファイル: src/ast.rs, src/infer.rs, tests/evaluator.rs
//! 評価モジュール
//!
//! - 正格評価戦略で式を還元し、副作用なしの実装に保つ。
//! - プリミティブ演算は部分適用可能な値として登録し、REPL 操作を簡潔にする。
//! - べき乗や比較など一部演算子は直感的な型へフォールバックする設計を採用する。

use std::cmp::Ordering;
use std::collections::HashMap;

use crate::ast as A;
use crate::errors::EvalError;
use crate::primitives::{EqOp, NumericOp, OrdOp, PrimitiveKind, PRIMITIVES};

#[derive(Clone, Debug)]
/// 評価器が返す値の列挙体。
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
/// 2 引数プリミティブの部分適用を支援するラッパー型。
pub struct Prim2 {
    pub f: fn(Value, Value) -> Result<Value, EvalError>,
    pub a: Option<Box<Value>>,
}

/// 名前と値を紐づける評価環境。
pub type Env = HashMap<String, Value>;

/// REPL とテストで利用する初期プリミティブ環境を構築する。
pub fn initial_env() -> Env {
    let mut env: Env = HashMap::new();

    /// 値を文字列へ変換し `String` 値として返す。
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

    /// 2 引数関数を `Prim2` ラッパーに変換する。
    fn prim2(f: fn(Value, Value) -> Result<Value, EvalError>) -> Value {
        Value::Prim2(Prim2 { f, a: None })
    }

    fn add_op(a: Value, b: Value) -> Result<Value, EvalError> {
        Ok(Value::Int(to_int(&a)? + to_int(&b)?))
    }

    fn sub_op(a: Value, b: Value) -> Result<Value, EvalError> {
        Ok(Value::Int(to_int(&a)? - to_int(&b)?))
    }

    fn mul_op(a: Value, b: Value) -> Result<Value, EvalError> {
        Ok(Value::Int(to_int(&a)? * to_int(&b)?))
    }

    fn div_op(a: Value, b: Value) -> Result<Value, EvalError> {
        Ok(Value::Double(to_double(&a)? / to_double(&b)?))
    }

    /// 整数指数を優先的に扱い、必要に応じて浮動小数へフォールバックする。
    fn powi(a: Value, b: Value) -> Result<Value, EvalError> {
        match (a, b) {
            (Value::Int(x), Value::Int(y)) if y >= 0 => {
                if y > u32::MAX as i64 {
                    return Err(EvalError::new("EVAL060", "(^) の指数が大きすぎます", None));
                }
                x.checked_pow(y as u32).map(Value::Int).ok_or_else(|| {
                    EvalError::new("EVAL060", "(^) の結果が Int の範囲を超えました", None)
                })
            }
            (x, y) => Ok(Value::Double(to_double(&x)?.powf(to_double(&y)?))),
        }
    }

    /// 常に浮動小数でべき乗を評価する。
    fn powf(a: Value, b: Value) -> Result<Value, EvalError> {
        Ok(Value::Double(to_double(&a)?.powf(to_double(&b)?)))
    }

    enum CompareFailure {
        Mismatch,
        NaN,
    }

    /// `Value` の構造を比較し、順序または失敗理由を返す。
    fn structural_compare(a: &Value, b: &Value) -> Result<std::cmp::Ordering, CompareFailure> {
        match (a, b) {
            (Value::Int(x), Value::Int(y)) => Ok(x.cmp(y)),
            (Value::Double(x), Value::Double(y)) => x.partial_cmp(y).ok_or(CompareFailure::NaN),
            (Value::Int(x), Value::Double(y)) => {
                (*x as f64).partial_cmp(y).ok_or(CompareFailure::NaN)
            }
            (Value::Double(x), Value::Int(y)) => {
                x.partial_cmp(&(*y as f64)).ok_or(CompareFailure::NaN)
            }
            (Value::Bool(x), Value::Bool(y)) => Ok(x.cmp(y)),
            (Value::Char(x), Value::Char(y)) => Ok(x.cmp(y)),
            (Value::String(x), Value::String(y)) => Ok(x.cmp(y)),
            (Value::List(xs), Value::List(ys)) => {
                for (vx, vy) in xs.iter().zip(ys.iter()) {
                    let ord = structural_compare(vx, vy)?;
                    if ord != std::cmp::Ordering::Equal {
                        return Ok(ord);
                    }
                }
                Ok(xs.len().cmp(&ys.len()))
            }
            (Value::Tuple(xs), Value::Tuple(ys)) => {
                for (vx, vy) in xs.iter().zip(ys.iter()) {
                    let ord = structural_compare(vx, vy)?;
                    if ord != std::cmp::Ordering::Equal {
                        return Ok(ord);
                    }
                }
                Ok(xs.len().cmp(&ys.len()))
            }
            _ => Err(CompareFailure::Mismatch),
        }
    }

    /// リストやタプルを含む値の構造的な等価判定を行う。
    fn eqv(a: &Value, b: &Value) -> Result<bool, EvalError> {
        match structural_compare(a, b) {
            Ok(std::cmp::Ordering::Equal) => Ok(true),
            Ok(_) => Ok(false),
            Err(CompareFailure::Mismatch) => Err(EvalError::new(
                "EVAL050",
                "==: 未対応の型の組み合わせ",
                None,
            )),
            Err(CompareFailure::NaN) => Ok(false),
        }
    }

    /// 値を辞書式で比較し、必要に応じて再帰的に判定する。
    fn compare(a: &Value, b: &Value) -> Result<std::cmp::Ordering, EvalError> {
        match structural_compare(a, b) {
            Ok(ord) => Ok(ord),
            Err(CompareFailure::Mismatch) => Err(EvalError::new(
                "EVAL050",
                "比較演算: 未対応の型の組み合わせ",
                None,
            )),
            Err(CompareFailure::NaN) => Err(EvalError::new("EVAL090", "NaN 比較", None)),
        }
    }

    fn eq_op(a: Value, b: Value) -> Result<Value, EvalError> {
        Ok(Value::Bool(eqv(&a, &b)?))
    }

    fn ne_op(a: Value, b: Value) -> Result<Value, EvalError> {
        Ok(Value::Bool(!eqv(&a, &b)?))
    }

    fn lt_op(a: Value, b: Value) -> Result<Value, EvalError> {
        Ok(Value::Bool(compare(&a, &b)? == Ordering::Less))
    }

    fn le_op(a: Value, b: Value) -> Result<Value, EvalError> {
        Ok(Value::Bool({
            let o = compare(&a, &b)?;
            o == Ordering::Less || o == Ordering::Equal
        }))
    }

    fn gt_op(a: Value, b: Value) -> Result<Value, EvalError> {
        Ok(Value::Bool(compare(&a, &b)? == Ordering::Greater))
    }

    fn ge_op(a: Value, b: Value) -> Result<Value, EvalError> {
        Ok(Value::Bool({
            let o = compare(&a, &b)?;
            o == Ordering::Greater || o == Ordering::Equal
        }))
    }

    for def in PRIMITIVES {
        match def.kind {
            PrimitiveKind::Numeric(kind) => {
                let f: fn(Value, Value) -> Result<Value, EvalError> = match kind {
                    NumericOp::Add => add_op,
                    NumericOp::Sub => sub_op,
                    NumericOp::Mul => mul_op,
                };
                env.insert(def.name.into(), prim2(f));
            }
            PrimitiveKind::FractionalDiv => {
                env.insert(def.name.into(), prim2(div_op));
            }
            PrimitiveKind::PowInt => {
                env.insert(def.name.into(), prim2(powi));
            }
            PrimitiveKind::PowFloat => {
                env.insert(def.name.into(), prim2(powf));
            }
            PrimitiveKind::Eq(kind) => {
                let f: fn(Value, Value) -> Result<Value, EvalError> = match kind {
                    EqOp::Eq => eq_op,
                    EqOp::Ne => ne_op,
                };
                env.insert(def.name.into(), prim2(f));
            }
            PrimitiveKind::Ord(kind) => {
                let f: fn(Value, Value) -> Result<Value, EvalError> = match kind {
                    OrdOp::Lt => lt_op,
                    OrdOp::Le => le_op,
                    OrdOp::Gt => gt_op,
                    OrdOp::Ge => ge_op,
                };
                env.insert(def.name.into(), prim2(f));
            }
            PrimitiveKind::Show => {
                env.insert(def.name.into(), Value::Prim1(py_show));
            }
        }
    }

    env
}

/// 値を関数として扱い、引数を適用して評価するヘルパ。
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

/// `Value` から整数を取り出し、必要ならば丸めて返す。
fn to_int(v: &Value) -> Result<i64, EvalError> {
    match v {
        Value::Int(i) => Ok(*i),
        Value::Double(d) => Ok(*d as i64),
        _ => Err(EvalError::new("EVAL050", "Int 変換に失敗", None)),
    }
}
/// `Value` から浮動小数を取り出し、整数も安全に変換する。
fn to_double(v: &Value) -> Result<f64, EvalError> {
    match v {
        Value::Double(d) => Ok(*d),
        Value::Int(i) => Ok(*i as f64),
        _ => Err(EvalError::new("EVAL050", "Double 変換に失敗", None)),
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
