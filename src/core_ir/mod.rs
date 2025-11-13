// パス: src/core_ir/mod.rs
// 役割: Core IR のデータ構造定義と関連ユーティリティを提供する
// 意図: AST とバックエンドの橋渡しとなる SSA 風 IR を確立する
// 関連ファイル: src/core_ir/lower.rs, src/codegen/cranelift.rs
#![allow(clippy::module_name_repetitions)]

pub mod dict_specs;

pub mod lower;

use std::collections::BTreeMap;
use std::fmt;

use self::dict_specs::lookup_method_spec;
use crate::ast as A;

/// Core IR 全体を表すモジュール。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Module {
    pub functions: BTreeMap<String, Function>,
    pub entry: Option<String>,
    pub data_layouts: BTreeMap<String, DataTypeLayout>,
    pub dictionaries: Vec<DictionaryInit>,
}

impl Module {
    /// 新しいモジュールを初期化する。
    pub fn new() -> Self {
        Self {
            functions: BTreeMap::new(),
            entry: None,
            data_layouts: BTreeMap::new(),
            dictionaries: Vec::new(),
        }
    }

    /// 関数定義を追加する。既に存在する場合は上書きする。
    pub fn insert_function(&mut self, func: Function) -> Option<Function> {
        self.functions.insert(func.name.clone(), func)
    }

    /// エントリポイント関数名を設定する。
    pub fn set_entry<S: Into<String>>(&mut self, name: S) {
        self.entry = Some(name.into());
    }

    /// エントリポイントを取得する。
    pub fn entry(&self) -> Option<&str> {
        self.entry.as_deref()
    }
}

/// 代数的データ型のランタイムレイアウト。
#[derive(Clone, Debug, PartialEq)]
pub struct DataTypeLayout {
    pub name: String,
    pub type_params: Vec<String>,
    pub constructors: Vec<ConstructorLayout>,
}

/// データコンストラクタごとのタグ・フィールド情報。
#[derive(Clone, Debug, PartialEq)]
pub struct ConstructorLayout {
    pub name: String,
    pub tag: u32,
    pub arity: usize,
    pub parent: String,
    pub field_types: Vec<A::TypeExpr>,
}

/// 型クラス辞書初期化に必要な情報。
#[derive(Clone, Debug, PartialEq)]
pub struct DictionaryInit {
    pub classname: String,
    pub type_repr: String,
    pub value_ty: ValueTy,
    pub methods: Vec<DictionaryMethod>,
    pub scheme_repr: String,
    pub builder: DictionaryBuilder,
    pub origin: String,
    pub source_span: SourceRef,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DictionaryBuilder {
    Resolved(String),
    Unresolved,
}

impl DictionaryBuilder {
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Resolved(sym) => Some(sym.as_str()),
            Self::Unresolved => None,
        }
    }
}

/// 辞書に格納されるメソッド情報。
#[derive(Clone, Debug, PartialEq)]
pub struct DictionaryMethod {
    pub name: String,
    pub signature: Option<String>,
    pub symbol: String,
    pub method_id: u64,
}

/// Core IR 上の関数定義。
#[derive(Clone, Debug, PartialEq)]
pub struct Function {
    pub name: String,
    pub params: Vec<Parameter>,
    pub result: ValueTy,
    pub body: Expr,
    pub location: SourceRef,
}

/// 形引数を表現する。
#[derive(Clone, Debug, PartialEq)]
pub struct Parameter {
    pub name: String,
    pub ty: ValueTy,
    pub kind: ParameterKind,
    pub dict_type_repr: Option<String>,
    pub dict_value_ty: Option<ValueTy>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParameterKind {
    Value,
    Dictionary { classname: String },
}

impl Parameter {
    pub fn new(name: impl Into<String>, ty: ValueTy) -> Self {
        Self {
            name: name.into(),
            ty,
            kind: ParameterKind::Value,
            dict_type_repr: None,
            dict_value_ty: None,
        }
    }

    pub fn with_kind(
        name: impl Into<String>,
        ty: ValueTy,
        kind: ParameterKind,
        dict_type_repr: Option<String>,
        dict_value_ty: Option<ValueTy>,
    ) -> Self {
        Self {
            name: name.into(),
            ty,
            kind,
            dict_type_repr,
            dict_value_ty,
        }
    }
}

/// Core IR の式ノード。
#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Literal {
        value: Literal,
        ty: ValueTy,
    },
    Var {
        name: String,
        ty: ValueTy,
        kind: VarKind,
    },
    Let {
        bindings: Vec<Binding>,
        body: Box<Expr>,
        ty: ValueTy,
    },
    Lambda {
        params: Vec<Parameter>,
        body: Box<Expr>,
        ty: ValueTy,
    },
    Apply {
        func: Box<Expr>,
        args: Vec<Expr>,
        ty: ValueTy,
    },
    If {
        cond: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Box<Expr>,
        ty: ValueTy,
    },
    PrimOp {
        op: PrimOp,
        args: Vec<Expr>,
        ty: ValueTy,
        dict_fallback: bool,
    },
    Tuple {
        items: Vec<Expr>,
        ty: ValueTy,
    },
    List {
        items: Vec<Expr>,
        ty: ValueTy,
    },
    DictionaryPlaceholder {
        classname: String,
        type_repr: String,
        ty: ValueTy,
    },
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<MatchArm>,
        ty: ValueTy,
    },
}

impl Expr {
    /// この式ノードの型情報を返す。
    pub fn ty(&self) -> &ValueTy {
        match self {
            Self::Literal { ty, .. }
            | Self::Var { ty, .. }
            | Self::Let { ty, .. }
            | Self::Lambda { ty, .. }
            | Self::Apply { ty, .. }
            | Self::If { ty, .. }
            | Self::PrimOp { ty, .. }
            | Self::Tuple { ty, .. }
            | Self::List { ty, .. }
            | Self::DictionaryPlaceholder { ty, .. }
            | Self::Match { ty, .. } => ty,
        }
    }
}

/// let 束縛。
#[derive(Clone, Debug, PartialEq)]
pub struct Binding {
    pub name: String,
    pub value: Expr,
    pub ty: ValueTy,
}

/// `case` 式の各アーム。
#[derive(Clone, Debug, PartialEq)]
pub struct MatchArm {
    pub pattern: A::Pattern,
    pub guard: Option<Expr>,
    pub body: Expr,
    pub constructor: Option<String>,
    pub tag: Option<u32>,
    pub arity: usize,
    pub bindings: Vec<MatchBinding>,
}

/// パターン束縛に付随する型情報。
#[derive(Clone, Debug, PartialEq)]
pub struct MatchBinding {
    pub name: String,
    pub ty: ValueTy,
    pub path: Vec<usize>,
}

/// プリミティブ演算子列挙。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PrimOp {
    AddInt,
    SubInt,
    MulInt,
    DivInt,
    ModInt,
    AddDouble,
    SubDouble,
    MulDouble,
    DivDouble,
    EqInt,
    NeqInt,
    LtInt,
    LeInt,
    GtInt,
    GeInt,
    EqDouble,
    NeqDouble,
    LtDouble,
    LeDouble,
    GtDouble,
    GeDouble,
    AndBool,
    OrBool,
    NotBool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrimOpDictionaryInfo {
    pub classname: &'static str,
    pub method: &'static str,
    pub signature: &'static str,
    pub method_id: u64,
}

impl PrimOp {
    #[must_use]
    pub fn dictionary_method(&self) -> Option<PrimOpDictionaryInfo> {
        use PrimOp::*;
        let (classname, method) = match self {
            AddInt | AddDouble => ("Num", "add"),
            SubInt | SubDouble => ("Num", "sub"),
            MulInt | MulDouble => ("Num", "mul"),
            DivDouble => ("Fractional", "div"),
            DivInt => ("Integral", "div"),
            ModInt => ("Integral", "mod"),
            EqInt | EqDouble => ("Eq", "eq"),
            NeqInt | NeqDouble => ("Eq", "neq"),
            LtInt | LtDouble => ("Ord", "lt"),
            LeInt | LeDouble => ("Ord", "le"),
            GtInt | GtDouble => ("Ord", "gt"),
            GeInt | GeDouble => ("Ord", "ge"),
            AndBool => ("BoolLogic", "and"),
            OrBool => ("BoolLogic", "or"),
            NotBool => ("BoolLogic", "not"),
        };
        let spec = lookup_method_spec(classname, method)?;
        Some(PrimOpDictionaryInfo {
            classname,
            method,
            signature: spec.pattern.generic_signature(),
            method_id: spec.method_id,
        })
    }
}

/// 変数参照の種別。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VarKind {
    Local,
    Param,
    Function,
    Primitive,
    Intrinsic,
}

/// Core IR が扱う型。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ValueTy {
    Int,
    Double,
    Bool,
    Char,
    String,
    Unit,
    Tuple(Vec<ValueTy>),
    List(Box<ValueTy>),
    Function {
        params: Vec<ValueTy>,
        result: Box<ValueTy>,
    },
    Data {
        constructor: String,
        args: Vec<ValueTy>,
    },
    Dictionary {
        classname: String,
    },
    /// 型推論結果がまだ確定していない場合に利用する。
    Unknown,
}

impl ValueTy {
    /// 単純な型のみで構成されているか判定する。
    pub fn is_concrete(&self) -> bool {
        match self {
            Self::Int | Self::Double | Self::Bool | Self::Char | Self::String | Self::Unit => true,
            Self::Tuple(items) => items.iter().all(Self::is_concrete),
            Self::List(item) => item.is_concrete(),
            Self::Function { params, result } => {
                params.iter().all(Self::is_concrete) && result.is_concrete()
            }
            Self::Data { args, .. } => args.iter().all(Self::is_concrete),
            Self::Dictionary { .. } => true,
            Self::Unknown => false,
        }
    }
}

impl fmt::Display for ValueTy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int => write!(f, "Int"),
            Self::Double => write!(f, "Double"),
            Self::Bool => write!(f, "Bool"),
            Self::Char => write!(f, "Char"),
            Self::String => write!(f, "String"),
            Self::Unit => write!(f, "Unit"),
            Self::Tuple(items) => {
                let inner = items
                    .iter()
                    .map(|ty| ty.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "({inner})")
            }
            Self::List(item) => write!(f, "[{}]", item),
            Self::Function { params, result } => {
                let inputs = params
                    .iter()
                    .map(|ty| ty.to_string())
                    .collect::<Vec<_>>()
                    .join(" -> ");
                if inputs.is_empty() {
                    write!(f, "{}", result)
                } else {
                    write!(f, "{} -> {}", inputs, result)
                }
            }
            Self::Data { constructor, args } => {
                if args.is_empty() {
                    write!(f, "{constructor}")
                } else {
                    let inner = args
                        .iter()
                        .map(|ty| ty.to_string())
                        .collect::<Vec<_>>()
                        .join(", ");
                    write!(f, "{constructor}<{inner}>")
                }
            }
            Self::Dictionary { classname } => write!(f, "Dict<{classname}>"),
            Self::Unknown => write!(f, "_"),
        }
    }
}

/// Core IR で扱うリテラル値。
#[derive(Clone, Debug, PartialEq)]
pub enum Literal {
    Int(i64),
    Double(f64),
    Bool(bool),
    Char(char),
    String(String),
    Unit,
    EmptyList,
}

impl Literal {
    pub fn ty(&self) -> ValueTy {
        match self {
            Self::Int(_) => ValueTy::Int,
            Self::Double(_) => ValueTy::Double,
            Self::Bool(_) => ValueTy::Bool,
            Self::Char(_) => ValueTy::Char,
            Self::String(_) => ValueTy::String,
            Self::Unit => ValueTy::Unit,
            Self::EmptyList => ValueTy::List(Box::new(ValueTy::Unknown)),
        }
    }
}

/// ソース上の参照情報。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SourceRef {
    pub line: usize,
    pub column: usize,
}

impl SourceRef {
    pub const fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

/// Core IR 生成時のエラー。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreIrError {
    pub code: &'static str,
    pub message: String,
}

impl CoreIrError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for CoreIrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for CoreIrError {}
