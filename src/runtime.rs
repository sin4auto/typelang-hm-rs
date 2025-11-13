// パス: src/runtime.rs
// 役割: 評価時に用いる値表現とプリミティブ生成ヘルパーを提供する
// 意図: 評価器・プリミティブ定義から共有される基盤ロジックを分離する
// 関連ファイル: src/evaluator.rs, src/primitives.rs
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::rc::Rc;

use crate::ast::Expr;
use crate::errors::EvalError;

thread_local! {
    static PRINTLN_CAPTURE: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
}

#[derive(Debug, Clone)]
pub struct Env {
    inner: Rc<EnvFrame>,
}

#[derive(Debug, Clone)]
pub struct CapturedEnv {
    inner: Rc<EnvFrame>,
}

#[derive(Debug)]
struct EnvFrame {
    bindings: RefCell<HashMap<String, BindingValue>>,
    parent: Option<Rc<EnvFrame>>,
}

impl Drop for EnvFrame {
    fn drop(&mut self) {
        self.bindings.borrow_mut().clear();
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        if Rc::strong_count(&self.inner) == 1 {
            // This is the last strong reference; clear bindings eagerly.
            self.inner.bindings.borrow_mut().clear();
        }
    }
}

impl Drop for CapturedEnv {
    fn drop(&mut self) {
        if Rc::strong_count(&self.inner) == 1 {
            self.inner.bindings.borrow_mut().clear();
        }
    }
}

#[derive(Clone, Debug)]
enum BindingValue {
    Present(Value),
    Tombstone,
}

impl EnvFrame {
    fn root(map: HashMap<String, Value>) -> Self {
        let mut bindings = HashMap::with_capacity(map.len());
        for (k, v) in map {
            bindings.insert(k, BindingValue::Present(v));
        }
        Self {
            bindings: RefCell::new(bindings),
            parent: None,
        }
    }

    fn child(parent: Rc<EnvFrame>) -> Self {
        Self {
            bindings: RefCell::new(HashMap::new()),
            parent: Some(parent),
        }
    }
}

fn collect_snapshot(frame: &Rc<EnvFrame>, acc: &mut HashMap<String, Value>) {
    if let Some(parent) = &frame.parent {
        collect_snapshot(parent, acc);
    }
    for (key, value) in frame.bindings.borrow().iter() {
        match value {
            BindingValue::Present(v) => {
                acc.insert(key.clone(), v.clone());
            }
            BindingValue::Tombstone => {
                acc.remove(key);
            }
        }
    }
}

fn lookup_binding(frame: &Rc<EnvFrame>, key: &str) -> Option<Value> {
    match lookup_binding_value(frame, key)? {
        BindingValue::Present(v) => Some(v),
        BindingValue::Tombstone => None,
    }
}

fn lookup_binding_value(frame: &Rc<EnvFrame>, key: &str) -> Option<BindingValue> {
    if let Some(value) = {
        let bindings = frame.bindings.borrow();
        bindings.get(key).cloned()
    } {
        return Some(value);
    }
    frame
        .parent
        .as_ref()
        .and_then(|parent| lookup_binding_value(parent, key))
}

impl Env {
    /// 空の環境を生成する。
    pub fn new() -> Self {
        Self::from_map(HashMap::new())
    }

    /// 既存のマップから環境を生成する。
    pub fn from_map(map: HashMap<String, Value>) -> Self {
        Self {
            inner: Rc::new(EnvFrame::root(map)),
        }
    }

    /// 現在の内容を複製した子環境を返す。
    pub fn child(&self) -> Self {
        Self {
            inner: Rc::new(EnvFrame::child(Rc::clone(&self.inner))),
        }
    }

    /// 環境をフラットな `HashMap` としてコピーする。
    pub fn snapshot(&self) -> HashMap<String, Value> {
        let mut acc = HashMap::new();
        collect_snapshot(&self.inner, &mut acc);
        acc
    }

    /// 環境を破棄する前に全束縛を明示的に解放し、`Rc` 循環を防ぐ。
    /// REPL 等で環境を継続利用する場合は呼び出さないこと。
    pub fn teardown(&mut self) {
        self.inner.bindings.borrow_mut().clear();
    }

    /// 束縛を追加または更新する。
    pub fn insert(&self, key: impl Into<String>, val: Value) -> Option<Value> {
        let key = key.into();
        let prev = self.get(&key);
        self.inner
            .bindings
            .borrow_mut()
            .insert(key, BindingValue::Present(val));
        prev
    }

    /// 束縛を取得する。
    pub fn get(&self, key: &str) -> Option<Value> {
        lookup_binding(&self.inner, key)
    }

    /// 束縛を除去する。
    pub fn remove(&self, key: &str) -> Option<Value> {
        let prev = self.get(key)?;
        let mut bindings = self.inner.bindings.borrow_mut();
        bindings.remove(key);
        if self.inner.parent.is_some() {
            bindings.insert(key.to_string(), BindingValue::Tombstone);
        }
        Some(prev)
    }

    /// クロージャ用に循環参照を追跡する捕捉環境を生成する。
    pub fn capture(&self) -> CapturedEnv {
        CapturedEnv {
            inner: Rc::clone(&self.inner),
        }
    }

    /// 所有権を捕捉環境へ移行する。
    pub fn into_capture(self) -> CapturedEnv {
        CapturedEnv {
            inner: Rc::clone(&self.inner),
        }
    }
}

impl Default for Env {
    fn default() -> Self {
        Self::new()
    }
}

impl CapturedEnv {
    pub fn upgrade(&self) -> Env {
        Env {
            inner: Rc::clone(&self.inner),
        }
    }
}

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
        env: CapturedEnv,
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

fn emit_line(text: &str) {
    let intercepted = PRINTLN_CAPTURE.with(|slot| {
        let mut guard = slot.borrow_mut();
        if let Some(buffer) = guard.as_mut() {
            buffer.push(text.to_string());
            true
        } else {
            false
        }
    });
    if !intercepted {
        println!("{}", text);
    }
}

pub(crate) fn println_op(value: Value) -> Result<Value, EvalError> {
    let rendered = py_show(value)?;
    match rendered {
        Value::String(text) => {
            emit_line(&text);
            Ok(Value::String(text))
        }
        other => Err(EvalError::new(
            "EVAL051",
            format!("println: 無効な表示結果 {:?}", other),
            None,
        )),
    }
}

#[cfg(test)]
fn capture_println<F, R>(action: F) -> (R, Vec<String>)
where
    F: FnOnce() -> R,
{
    PRINTLN_CAPTURE.with(|slot| {
        let mut guard = slot.borrow_mut();
        assert!(
            guard.is_none(),
            "capture_println: nested capture not supported"
        );
        *guard = Some(Vec::new());
    });
    let result = action();
    let lines = PRINTLN_CAPTURE.with(|slot| {
        slot.borrow_mut()
            .take()
            .expect("capture_println: buffer must exist")
    });
    (result, lines)
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
    fn env_child_reads_parent_without_copying() {
        let root = Env::new();
        root.insert("x", Value::Int(1));
        let child = root.child();
        assert!(matches!(child.get("x"), Some(Value::Int(1))));
        child.insert("x", Value::Int(2));
        assert!(matches!(child.get("x"), Some(Value::Int(2))));
        assert!(matches!(root.get("x"), Some(Value::Int(1))));
    }

    #[test]
    fn env_remove_shadows_parent_binding() {
        let root = Env::new();
        root.insert("x", Value::Int(1));
        let child = root.child();
        assert!(matches!(child.remove("x"), Some(Value::Int(1))));
        assert!(child.get("x").is_none());
        assert!(matches!(root.get("x"), Some(Value::Int(1))));
    }

    #[test]
    fn snapshot_respects_child_overrides_and_removals() {
        let root = Env::new();
        root.insert("a", Value::Int(1));
        let child = root.child();
        child.insert("b", Value::Int(2));
        child.remove("a");
        let snap = child.snapshot();
        assert!(!snap.contains_key("a"));
        assert!(matches!(snap.get("b"), Some(Value::Int(2))));
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

    #[test]
    fn py_show_handles_unit_like_and_list_variants() {
        let unit_like = Value::Data {
            constructor: "None".into(),
            fields: Vec::new(),
        };
        let rendered = py_show(unit_like).expect("show None");
        assert!(matches!(rendered, Value::String(s) if s == "None"));

        let list = Value::Data {
            constructor: "List".into(),
            fields: vec![Value::Int(1), Value::Int(2)],
        };
        let rendered = py_show(list).expect("show List 1 2");
        assert!(matches!(rendered, Value::String(s) if s == "List 1 2"));
    }

    #[test]
    fn println_op_formats_and_records_lines() {
        let (result, lines) = capture_println(|| println_op(Value::Int(7)).expect("println ok"));
        match result {
            Value::String(text) => assert_eq!(text, "7"),
            other => panic!("expected println to return String, got {:?}", other),
        }
        assert_eq!(lines, vec!["7".to_string()]);
    }

    #[test]
    fn powi_negative_exponent_falls_back_to_f64() {
        let result =
            powi(Value::Int(2), Value::Int(-3)).expect("negative exponent uses float math");
        match result {
            Value::Double(d) => assert!((d - 0.125).abs() < f64::EPSILON),
            other => panic!("expected Double from powi fallback, got {:?}", other),
        }
    }

    #[test]
    fn primop_binary_captured_error_path_propagates() {
        let add = PrimOp::binary(add_op);
        let partially = add
            .clone()
            .apply(Value::Bool(true))
            .expect("partial application regardless of type");
        let Value::Prim(op) = partially else {
            panic!("second stage should remain Prim");
        };
        let err = op.apply(Value::Int(1));
        assert_eq!(err_code(err), Some("EVAL050"));
    }

    #[test]
    fn compare_covers_constructor_order_and_arity_mismatch() {
        let lesser = Value::Data {
            constructor: "A".into(),
            fields: vec![Value::Int(1)],
        };
        let greater = Value::Data {
            constructor: "B".into(),
            fields: vec![Value::Int(0)],
        };
        let ordering = lt_op(lesser.clone(), greater.clone()).unwrap();
        assert!(matches!(ordering, Value::Bool(true)));
        let reverse = gt_op(greater, lesser).unwrap();
        assert!(matches!(reverse, Value::Bool(true)));

        let short = Value::Data {
            constructor: "C".into(),
            fields: vec![Value::Int(1)],
        };
        let long = Value::Data {
            constructor: "C".into(),
            fields: vec![Value::Int(1), Value::Int(2)],
        };
        let mismatch = le_op(short, long);
        assert_eq!(err_code(mismatch), Some("EVAL050"));
    }

    #[test]
    fn list_and_tuple_comparisons_cover_length_checks() {
        let list_short = Value::List(vec![Value::Int(1)]);
        let list_long = Value::List(vec![Value::Int(1), Value::Int(2)]);
        let result = le_op(list_short, list_long).unwrap();
        assert!(matches!(result, Value::Bool(true)));

        let tuple_short = Value::Tuple(vec![Value::Int(1)]);
        let tuple_long = Value::Tuple(vec![Value::Int(1), Value::Int(0)]);
        let result = gt_op(tuple_long, tuple_short).unwrap();
        assert!(matches!(result, Value::Bool(true)));
    }

    #[test]
    fn py_show_formats_scalar_variants() {
        let rendered = py_show(Value::Double(1.25)).expect("double formatted");
        assert!(matches!(rendered, Value::String(s) if s.starts_with("1.25")));

        let true_branch = py_show(Value::Bool(true)).expect("bool true");
        assert!(matches!(true_branch, Value::String(s) if s == "True"));
        let false_branch = py_show(Value::Bool(false)).expect("bool false");
        assert!(matches!(false_branch, Value::String(s) if s == "False"));

        let ch = py_show(Value::Char('λ')).expect("char formatting");
        assert!(matches!(ch, Value::String(s) if s == "λ"));

        let message = py_show(Value::String("ok".into())).expect("string passthrough");
        assert!(matches!(message, Value::String(s) if s == "ok"));
    }

    #[test]
    fn compare_handles_scalar_and_mixed_numeric_types() {
        let mixed = lt_op(Value::Int(1), Value::Double(2.0)).unwrap();
        assert!(matches!(mixed, Value::Bool(true)));

        let reverse = gt_op(Value::Double(2.5), Value::Int(2)).unwrap();
        assert!(matches!(reverse, Value::Bool(true)));

        let bool_compare = eq_op(Value::Bool(true), Value::Bool(false)).unwrap();
        assert!(matches!(bool_compare, Value::Bool(false)));

        let char_compare = lt_op(Value::Char('a'), Value::Char('z')).unwrap();
        assert!(matches!(char_compare, Value::Bool(true)));

        let string_compare = gt_op(Value::String("b".into()), Value::String("a".into())).unwrap();
        assert!(matches!(string_compare, Value::Bool(true)));
    }
}
