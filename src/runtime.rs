// パス: src/runtime.rs
// 役割: 評価時に用いる値表現とプリミティブ生成ヘルパーを提供する
// 意図: 評価器・プリミティブ定義から共有される基盤ロジックを分離する
// 関連ファイル: src/evaluator.rs, src/primitives.rs
use std::cmp::Ordering;
use std::collections::HashMap;

use crate::ast::Expr;
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
    Data {
        constructor: String,
        fields: Vec<Value>,
    },
    Closure {
        params: Vec<String>,
        body: Box<Expr>,
        env: HashMap<String, Value>,
    },
    Prim(PrimOp),
}

#[derive(Clone, Debug)]
pub enum PrimOp {
    Prim1(fn(Value) -> Result<Value, EvalError>),
    Prim2 {
        f: fn(Value, Value) -> Result<Value, EvalError>,
        captured: Option<Box<Value>>,
    },
    DataCtor {
        name: String,
        arity: usize,
        collected: Vec<Value>,
    },
}

pub type Env = HashMap<String, Value>;

impl PrimOp {
    pub const fn unary(f: fn(Value) -> Result<Value, EvalError>) -> Self {
        PrimOp::Prim1(f)
    }

    pub const fn binary(f: fn(Value, Value) -> Result<Value, EvalError>) -> Self {
        PrimOp::Prim2 { f, captured: None }
    }

    pub fn into_value(self) -> Value {
        Value::Prim(self)
    }

    pub fn to_value(&self) -> Value {
        self.clone().into_value()
    }

    pub fn apply(self, arg: Value) -> Result<Value, EvalError> {
        match self {
            PrimOp::Prim1(f) => f(arg),
            PrimOp::Prim2 { f, captured: None } => Ok(Value::Prim(PrimOp::Prim2 {
                f,
                captured: Some(Box::new(arg)),
            })),
            PrimOp::Prim2 {
                f,
                captured: Some(prev),
            } => f(*prev, arg),
            PrimOp::DataCtor {
                name,
                arity,
                mut collected,
            } => {
                collected.push(arg);
                if collected.len() >= arity {
                    Ok(Value::Data {
                        constructor: name,
                        fields: collected,
                    })
                } else {
                    Ok(Value::Prim(PrimOp::DataCtor {
                        name,
                        arity,
                        collected,
                    }))
                }
            }
        }
    }
}

pub(crate) fn py_show(v: Value) -> Result<Value, EvalError> {
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
        Value::Data {
            constructor,
            fields,
        } => {
            if fields.is_empty() {
                constructor
            } else {
                let mut parts = Vec::new();
                for field in fields {
                    match py_show(field)? {
                        Value::String(s) => parts.push(s),
                        _ => return Err(EvalError::new("EVAL050", "show: 未対応の値", None)),
                    }
                }
                format!("{} {}", constructor, parts.join(" "))
            }
        }
        _ => return Err(EvalError::new("EVAL050", "show: 未対応の値", None)),
    }))
}

fn numeric_binop<T, Conv, Wrap, Op>(
    a: Value,
    b: Value,
    conv: Conv,
    wrap: Wrap,
    op: Op,
) -> Result<Value, EvalError>
where
    Conv: Fn(&Value) -> Result<T, EvalError>,
    Wrap: Fn(T) -> Value,
    Op: FnOnce(T, T) -> T,
    T: Copy,
{
    let lhs = conv(&a)?;
    let rhs = conv(&b)?;
    Ok(wrap(op(lhs, rhs)))
}

pub(crate) fn add_op(a: Value, b: Value) -> Result<Value, EvalError> {
    numeric_binop(a, b, to_int, Value::Int, |x, y| x + y)
}

pub(crate) fn sub_op(a: Value, b: Value) -> Result<Value, EvalError> {
    numeric_binop(a, b, to_int, Value::Int, |x, y| x - y)
}

pub(crate) fn mul_op(a: Value, b: Value) -> Result<Value, EvalError> {
    numeric_binop(a, b, to_int, Value::Int, |x, y| x * y)
}

pub(crate) fn div_op(a: Value, b: Value) -> Result<Value, EvalError> {
    numeric_binop(a, b, to_double, Value::Double, |x, y| x / y)
}

fn ensure_nonzero(rhs: i64, op_name: &str) -> Result<(), EvalError> {
    if rhs == 0 {
        Err(EvalError::new(
            "EVAL061",
            format!("{op_name}: 0 で割ることはできません"),
            None,
        ))
    } else {
        Ok(())
    }
}

pub(crate) fn div_int_op(a: Value, b: Value) -> Result<Value, EvalError> {
    let lhs = to_int(&a)?;
    let rhs = to_int(&b)?;
    ensure_nonzero(rhs, "div")?;
    Ok(Value::Int(lhs.div_euclid(rhs)))
}

pub(crate) fn mod_int_op(a: Value, b: Value) -> Result<Value, EvalError> {
    let lhs = to_int(&a)?;
    let rhs = to_int(&b)?;
    ensure_nonzero(rhs, "mod")?;
    Ok(Value::Int(lhs.rem_euclid(rhs)))
}

pub(crate) fn quot_int_op(a: Value, b: Value) -> Result<Value, EvalError> {
    let lhs = to_int(&a)?;
    let rhs = to_int(&b)?;
    ensure_nonzero(rhs, "quot")?;
    Ok(Value::Int(lhs / rhs))
}

pub(crate) fn rem_int_op(a: Value, b: Value) -> Result<Value, EvalError> {
    let lhs = to_int(&a)?;
    let rhs = to_int(&b)?;
    ensure_nonzero(rhs, "rem")?;
    Ok(Value::Int(lhs % rhs))
}

pub(crate) fn powi(a: Value, b: Value) -> Result<Value, EvalError> {
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

pub(crate) fn powf(a: Value, b: Value) -> Result<Value, EvalError> {
    numeric_binop(a, b, to_double, Value::Double, |x, y| x.powf(y))
}

enum CompareFailure {
    Mismatch,
    NaN,
}

fn structural_compare(a: &Value, b: &Value) -> Result<std::cmp::Ordering, CompareFailure> {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => Ok(x.cmp(y)),
        (Value::Double(x), Value::Double(y)) => x.partial_cmp(y).ok_or(CompareFailure::NaN),
        (Value::Int(x), Value::Double(y)) => (*x as f64).partial_cmp(y).ok_or(CompareFailure::NaN),
        (Value::Double(x), Value::Int(y)) => x.partial_cmp(&(*y as f64)).ok_or(CompareFailure::NaN),
        (Value::Bool(x), Value::Bool(y)) => Ok(x.cmp(y)),
        (Value::Char(x), Value::Char(y)) => Ok(x.cmp(y)),
        (Value::String(x), Value::String(y)) => Ok(x.cmp(y)),
        (Value::List(xs), Value::List(ys)) => {
            for (vx, vy) in xs.iter().zip(ys.iter()) {
                let ord = structural_compare(vx, vy)?;
                if ord != Ordering::Equal {
                    return Ok(ord);
                }
            }
            Ok(xs.len().cmp(&ys.len()))
        }
        (Value::Tuple(xs), Value::Tuple(ys)) => {
            for (vx, vy) in xs.iter().zip(ys.iter()) {
                let ord = structural_compare(vx, vy)?;
                if ord != Ordering::Equal {
                    return Ok(ord);
                }
            }
            Ok(xs.len().cmp(&ys.len()))
        }
        (
            Value::Data {
                constructor: c1,
                fields: f1,
            },
            Value::Data {
                constructor: c2,
                fields: f2,
            },
        ) => {
            if c1 != c2 {
                return Ok(c1.cmp(c2));
            }
            if f1.len() != f2.len() {
                return Err(CompareFailure::Mismatch);
            }
            for (vx, vy) in f1.iter().zip(f2.iter()) {
                let ord = structural_compare(vx, vy)?;
                if ord != Ordering::Equal {
                    return Ok(ord);
                }
            }
            Ok(Ordering::Equal)
        }
        _ => Err(CompareFailure::Mismatch),
    }
}

fn eqv(a: &Value, b: &Value) -> Result<bool, EvalError> {
    match structural_compare(a, b) {
        Ok(Ordering::Equal) => Ok(true),
        Ok(_) => Ok(false),
        Err(CompareFailure::Mismatch) => Err(EvalError::new(
            "EVAL050",
            "==: 未対応の型の組み合わせ",
            None,
        )),
        Err(CompareFailure::NaN) => Ok(false),
    }
}

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

pub(crate) fn eq_op(a: Value, b: Value) -> Result<Value, EvalError> {
    Ok(Value::Bool(eqv(&a, &b)?))
}

pub(crate) fn ne_op(a: Value, b: Value) -> Result<Value, EvalError> {
    Ok(Value::Bool(!eqv(&a, &b)?))
}

pub(crate) fn lt_op(a: Value, b: Value) -> Result<Value, EvalError> {
    Ok(Value::Bool(compare(&a, &b)? == Ordering::Less))
}

pub(crate) fn le_op(a: Value, b: Value) -> Result<Value, EvalError> {
    Ok(Value::Bool({
        let o = compare(&a, &b)?;
        o == Ordering::Less || o == Ordering::Equal
    }))
}

pub(crate) fn gt_op(a: Value, b: Value) -> Result<Value, EvalError> {
    Ok(Value::Bool(compare(&a, &b)? == Ordering::Greater))
}

pub(crate) fn ge_op(a: Value, b: Value) -> Result<Value, EvalError> {
    Ok(Value::Bool({
        let o = compare(&a, &b)?;
        o == Ordering::Greater || o == Ordering::Equal
    }))
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

pub fn make_data_ctor(name: &str, arity: usize) -> Value {
    if arity == 0 {
        Value::Data {
            constructor: name.to_string(),
            fields: Vec::new(),
        }
    } else {
        Value::Prim(PrimOp::DataCtor {
            name: name.to_string(),
            arity,
            collected: Vec::new(),
        })
    }
}
