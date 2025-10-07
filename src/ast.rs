// パス: src/ast.rs
// 役割: 抽象構文木(AST)の型定義と表示ユーティリティを管理する
// 意図: パーサ・型推論・評価器が同じデータ構造を共有できるように整える
// 関連ファイル: src/parser.rs, src/infer.rs, src/evaluator.rs
//! AST モジュール
//!
//! 概要:
//! - 構文解析で得た式やプログラム定義を列挙体・構造体で表現する。
//! - 型推論器と評価器が追加の変換なしに読み取れる中立的な形を提供する。
//! - 糖衣構文の解決や値の既定化は parser / repl 側に任せ、この層では正規化済みデータのみ扱う。

use std::fmt;

/// ソース上の位置情報を保持する軽量な構造体。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub pos: usize,
    pub line: usize,
    pub col: usize,
}

impl Span {
    /// スパンを明示指定して構築する。
    pub const fn new(pos: usize, line: usize, col: usize) -> Self {
        Self { pos, line, col }
    }

    /// 位置情報が未収集の場合に利用するダミー値。
    pub const fn dummy() -> Self {
        Self {
            pos: 0,
            line: 0,
            col: 0,
        }
    }
}

// 式を構成する列挙体
#[derive(Clone, Debug, PartialEq)]
/// 言語内の式を表す AST ノードの集合。
pub enum Expr {
    Var {
        name: String,
        span: Span,
    },
    IntLit {
        value: i64,
        base: IntBase,
        span: Span,
    },
    FloatLit {
        value: f64,
        span: Span,
    },
    CharLit {
        value: char,
        span: Span,
    },
    StringLit {
        value: String,
        span: Span,
    },
    BoolLit {
        value: bool,
        span: Span,
    },
    ListLit {
        items: Vec<Expr>,
        span: Span,
    },
    TupleLit {
        items: Vec<Expr>,
        span: Span,
    },
    Lambda {
        params: Vec<String>,
        body: Box<Expr>,
        span: Span,
    },
    LetIn {
        bindings: Vec<(String, Vec<String>, Expr)>,
        body: Box<Expr>,
        span: Span,
    },
    If {
        cond: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Box<Expr>,
        span: Span,
    },
    App {
        func: Box<Expr>,
        arg: Box<Expr>,
        span: Span,
    },
    BinOp {
        op: String,
        left: Box<Expr>,
        right: Box<Expr>,
        span: Span,
    },
    Annot {
        expr: Box<Expr>,
        type_expr: TypeExpr,
        span: Span,
    },
    Case {
        scrutinee: Box<Expr>,
        arms: Vec<CaseArm>,
        span: Span,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// 整数リテラルが使用した基数を保持する列挙体。
pub enum IntBase {
    Dec,
    Hex,
    Oct,
    Bin,
}

// パーサが扱う型式ノード
#[derive(Clone, Debug, PartialEq, Eq)]
/// 構文上の型注釈を表すバリアント集合。
pub enum TypeExpr {
    TEVar(String),
    TECon(String),
    TEApp(Box<TypeExpr>, Box<TypeExpr>),
    TEFun(Box<TypeExpr>, Box<TypeExpr>),
    TEList(Box<TypeExpr>),
    TETuple(Vec<TypeExpr>),
}

#[derive(Clone, Debug, PartialEq)]
/// パターンマッチングで使用されるパターン表現。
pub enum Pattern {
    Wildcard {
        span: Span,
    },
    Var {
        name: String,
        span: Span,
    },
    Int {
        value: i64,
        base: IntBase,
        span: Span,
    },
    Bool {
        value: bool,
        span: Span,
    },
    Constructor {
        name: String,
        args: Vec<Pattern>,
        span: Span,
    },
}

#[derive(Clone, Debug, PartialEq)]
/// `case` 式の 1 アームを表現する構造体。
pub struct CaseArm {
    pub pattern: Pattern,
    pub body: Expr,
}

// 型クラス制約とシグマ型
#[derive(Clone, Debug, PartialEq, Eq)]
/// 型クラス名と対象の型変数を結び付ける制約。
pub struct Constraint {
    pub classname: String,
    pub typevar: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// 制約と型式を組み合わせたシグマ型。
pub struct SigmaType {
    pub constraints: Vec<Constraint>,
    pub r#type: TypeExpr,
}

// トップレベル定義とプログラム全体
#[derive(Clone, Debug, PartialEq)]
/// トップレベルで宣言された関数を保持するレコード。
pub struct TopLevel {
    pub name: String,
    pub params: Vec<String>,
    pub expr: Expr,
    pub signature: Option<SigmaType>,
}

#[derive(Clone, Debug, PartialEq)]
/// データコンストラクタの宣言を表現する。
pub struct DataConstructor {
    pub name: String,
    pub args: Vec<TypeExpr>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
/// 代数的データ型の宣言を格納するレコード。
pub struct DataDecl {
    pub name: String,
    pub params: Vec<String>,
    pub constructors: Vec<DataConstructor>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
/// トップレベル定義の集まりとしてのプログラム。
pub struct Program {
    pub data_decls: Vec<DataDecl>,
    pub decls: Vec<TopLevel>,
}

/// 式ノードを文字列表現へ整形する。
impl fmt::Display for Expr {
    /// デバッグしやすい括弧付きの表記に変換する。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expr::Var { name, .. } => write!(f, "{name}"),
            Expr::IntLit { value, .. } => write!(f, "{value}"),
            Expr::FloatLit { value, .. } => write!(f, "{value}"),
            Expr::CharLit { value, .. } => write!(f, "'{value}'"),
            Expr::StringLit { value, .. } => write!(f, "\"{value}\""),
            Expr::BoolLit { value, .. } => write!(f, "{}", if *value { "True" } else { "False" }),
            Expr::ListLit { items, .. } => {
                let parts: Vec<String> = items.iter().map(|e| format!("{}", e)).collect();
                write!(f, "[{}]", parts.join(", "))
            }
            Expr::TupleLit { items, .. } => {
                let parts: Vec<String> = items.iter().map(|e| format!("{}", e)).collect();
                write!(f, "({})", parts.join(", "))
            }
            Expr::Lambda { params, body, .. } => write!(f, "\\{} -> {}", params.join(" "), body),
            Expr::LetIn { bindings, body, .. } => {
                let mut parts = Vec::new();
                for (n, ps, e) in bindings {
                    if ps.is_empty() {
                        parts.push(format!("{n} = {e}"));
                    } else {
                        parts.push(format!("{n} {} = {e}", ps.join(" ")));
                    }
                }
                write!(f, "let {} in {}", parts.join("; "), body)
            }
            Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                write!(f, "if {} then {} else {}", cond, then_branch, else_branch)
            }
            Expr::App { func, arg, .. } => write!(f, "({} {})", func, arg),
            Expr::BinOp {
                op, left, right, ..
            } => write!(f, "({} {} {})", left, op, right),
            Expr::Annot {
                expr, type_expr, ..
            } => write!(f, "({} :: {:?})", expr, type_expr),
            Expr::Case {
                scrutinee, arms, ..
            } => {
                write!(f, "case {} of ", scrutinee)?;
                let mut parts = Vec::new();
                for arm in arms {
                    parts.push(format!("{} -> {}", arm.pattern, arm.body));
                }
                write!(f, "{}", parts.join("; "))
            }
        }
    }
}

impl Expr {
    /// 現在の式に紐づく開始位置を返す。
    pub fn span(&self) -> Span {
        match self {
            Expr::Var { span, .. }
            | Expr::IntLit { span, .. }
            | Expr::FloatLit { span, .. }
            | Expr::CharLit { span, .. }
            | Expr::StringLit { span, .. }
            | Expr::BoolLit { span, .. }
            | Expr::ListLit { span, .. }
            | Expr::TupleLit { span, .. }
            | Expr::Lambda { span, .. }
            | Expr::LetIn { span, .. }
            | Expr::If { span, .. }
            | Expr::App { span, .. }
            | Expr::BinOp { span, .. }
            | Expr::Annot { span, .. }
            | Expr::Case { span, .. } => *span,
        }
    }
}

impl fmt::Display for Pattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Pattern::Wildcard { .. } => write!(f, "_"),
            Pattern::Var { name, .. } => write!(f, "{}", name),
            Pattern::Int { value, .. } => write!(f, "{}", value),
            Pattern::Bool { value, .. } => write!(f, "{}", if *value { "True" } else { "False" }),
            Pattern::Constructor { name, args, .. } => {
                if args.is_empty() {
                    write!(f, "{}", name)
                } else {
                    let mut parts = Vec::new();
                    for pat in args {
                        parts.push(format!("{}", pat));
                    }
                    write!(f, "{} {}", name, parts.join(" "))
                }
            }
        }
    }
}

impl Pattern {
    /// パターンに付随するスパンを返す。
    pub fn span(&self) -> Span {
        match self {
            Pattern::Wildcard { span }
            | Pattern::Var { span, .. }
            | Pattern::Int { span, .. }
            | Pattern::Bool { span, .. }
            | Pattern::Constructor { span, .. } => *span,
        }
    }
}
