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

#[cfg(test)]
mod tests {
    use super::*;

    fn err_code(result: Result<Value, EvalError>) -> Option<&'static str> {
        result.err().map(|e| e.0.code)
    }

    #[test]
    fn primop_variants_apply_and_wrap_values() {
        let show = PrimOp::unary(py_show);
        let displayed = show.clone().apply(Value::Int(7)).expect("py_show succeeds");
        match displayed {
            Value::String(s) => assert_eq!(s, "7"),
            other => panic!("expected string from show, got {:?}", other),
        }

        let plus = PrimOp::binary(add_op);
        let partially_applied = plus
            .clone()
            .apply(Value::Int(10))
            .expect("partial application succeeds");
        let second = match partially_applied {
            Value::Prim(op) => op,
            other => panic!("expected Prim after partial application, got {:?}", other),
        };
        let total = second.apply(Value::Int(32)).expect("apply second argument");
        assert!(matches!(total, Value::Int(42)));

        let ctor = PrimOp::DataCtor {
            name: "Pair".into(),
            arity: 2,
            collected: Vec::new(),
        };
        let first = ctor.clone().apply(Value::Int(1)).expect("first arg");
        let data = match first {
            Value::Prim(op) => op.apply(Value::Int(2)).expect("second arg"),
            other => panic!("expected Prim from first ctor apply, got {:?}", other),
        };
        match data {
            Value::Data {
                constructor,
                fields,
            } => {
                assert_eq!(constructor, "Pair");
                assert_eq!(fields.len(), 2);
                assert!(matches!(fields[0], Value::Int(1)));
                assert!(matches!(fields[1], Value::Int(2)));
            }
            other => panic!("constructor should yield data, got {:?}", other),
        }

        let prim_value = PrimOp::binary(sub_op).into_value();
        match prim_value {
            Value::Prim(op) => {
                let value = op.to_value();
                assert!(matches!(value, Value::Prim(_)));
            }
            other => panic!("PrimOp::into_value must return Prim, got {:?}", other),
        }
    }

    #[test]
    fn py_show_formats_data_and_reports_errors() {
        let value = Value::Data {
            constructor: "Just".into(),
            fields: vec![Value::Int(5)],
        };
        let rendered = py_show(value).expect("show Just 5");
        assert!(matches!(rendered, Value::String(s) if s == "Just 5"));

        let unsupported = Value::List(vec![Value::Prim(PrimOp::binary(add_op))]);
        let err = py_show(unsupported).expect_err("show should fail on unsupported list");
        assert_eq!(err.0.code, "EVAL050");
    }

    #[test]
    fn arithmetic_helpers_cover_success_and_failure_paths() {
        assert!(matches!(
            add_op(Value::Int(1), Value::Int(2)).unwrap(),
            Value::Int(3)
        ));
        assert!(
            matches!(div_op(Value::Int(8), Value::Int(4)).unwrap(), Value::Double(d) if (d - 2.0).abs() < 1e-12)
        );

        let bad = add_op(Value::Bool(true), Value::Int(1));
        assert_eq!(err_code(bad), Some("EVAL050"));

        let div_zero = div_int_op(Value::Int(1), Value::Int(0));
        assert_eq!(err_code(div_zero), Some("EVAL061"));
        let mod_zero = mod_int_op(Value::Int(1), Value::Int(0));
        assert_eq!(err_code(mod_zero), Some("EVAL061"));

        let pow_large = powi(Value::Int(2), Value::Int(16));
        assert!(pow_large.is_ok(), "small exponent stays in range");
        let pow_error = powi(Value::Int(2), Value::Int((u32::MAX as i64) + 1));
        assert_eq!(err_code(pow_error), Some("EVAL060"));
    }

    #[test]
    fn comparison_helpers_cover_mismatch_and_nan_cases() {
        let eq_true = eq_op(Value::Int(1), Value::Int(1)).unwrap();
        assert!(matches!(eq_true, Value::Bool(true)));
        let eq_false = eq_op(Value::Int(1), Value::Int(2)).unwrap();
        assert!(matches!(eq_false, Value::Bool(false)));

        let mismatch = eq_op(Value::Int(1), Value::String("x".into()));
        assert_eq!(err_code(mismatch), Some("EVAL050"));

        let nan_compare = lt_op(Value::Double(f64::NAN), Value::Double(0.0));
        assert_eq!(err_code(nan_compare), Some("EVAL090"));
    }

    #[test]
    fn make_data_ctor_returns_immediate_or_curried_value() {
        match make_data_ctor("Unit", 0) {
            Value::Data {
                constructor,
                fields,
            } => {
                assert_eq!(constructor, "Unit");
                assert!(fields.is_empty());
            }
            other => panic!(
                "zero arity ctor should yield Data immediately, got {:?}",
                other
            ),
        }

        match make_data_ctor("Pair", 2) {
            Value::Prim(op) => {
                let mid = op.clone().apply(Value::Int(1)).expect("first arg");
                let final_value = match mid {
                    Value::Prim(op2) => op2.apply(Value::Int(2)).expect("second arg"),
                    other => panic!("expected Prim after first apply, got {:?}", other),
                };
                if let Value::Data { fields, .. } = final_value {
                    assert_eq!(fields.len(), 2);
                } else {
                    panic!("expected Pair data after applications");
                }
            }
            other => panic!("curried ctor should start as Prim, got {:?}", other),
        }
    }
}
