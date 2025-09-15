//! 抽象構文木（AST）
//!
//! 目的:
//! - 構文解析結果を評価/型推論で共用できる中立的な表現に落とし込む。
//!
//! 設計ノート:
//! - 構文シュガーはここでは扱わず、parser/repl 側で正規化する。
//! - 数値の基数（`IntBase`）は表示や一部の最適化のために保持する。
//! - 型式（`TypeExpr`）は注釈・シグネチャ用で、型システムの `Type` とは分離。

use std::fmt;

// 式ノード
#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Var {
        name: String,
    },
    IntLit {
        value: i64,
        base: IntBase,
    },
    FloatLit {
        value: f64,
    },
    CharLit {
        value: char,
    },
    StringLit {
        value: String,
    },
    BoolLit {
        value: bool,
    },
    ListLit {
        items: Vec<Expr>,
    },
    TupleLit {
        items: Vec<Expr>,
    },
    Lambda {
        params: Vec<String>,
        body: Box<Expr>,
    },
    LetIn {
        bindings: Vec<(String, Vec<String>, Expr)>,
        body: Box<Expr>,
    },
    If {
        cond: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Box<Expr>,
    },
    App {
        func: Box<Expr>,
        arg: Box<Expr>,
    },
    BinOp {
        op: String,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Annot {
        expr: Box<Expr>,
        type_expr: TypeExpr,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntBase {
    Dec,
    Hex,
    Oct,
    Bin,
}

// 型式（パーサ用）
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeExpr {
    TEVar(String),
    TECon(String),
    TEApp(Box<TypeExpr>, Box<TypeExpr>),
    TEFun(Box<TypeExpr>, Box<TypeExpr>),
    TEList(Box<TypeExpr>),
    TETuple(Vec<TypeExpr>),
}

// 制約と多相型（シグマ）
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Constraint {
    pub classname: String,
    pub typevar: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SigmaType {
    pub constraints: Vec<Constraint>,
    pub r#type: TypeExpr,
}

// トップレベル定義とプログラム
#[derive(Clone, Debug, PartialEq)]
pub struct TopLevel {
    pub name: String,
    pub params: Vec<String>,
    pub expr: Expr,
    pub signature: Option<SigmaType>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Program {
    pub decls: Vec<TopLevel>,
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expr::Var { name } => write!(f, "{name}"),
            Expr::IntLit { value, .. } => write!(f, "{value}"),
            Expr::FloatLit { value } => write!(f, "{value}"),
            Expr::CharLit { value } => write!(f, "'{value}'"),
            Expr::StringLit { value } => write!(f, "\"{value}\""),
            Expr::BoolLit { value } => write!(f, "{}", if *value { "True" } else { "False" }),
            Expr::ListLit { items } => {
                let parts: Vec<String> = items.iter().map(|e| format!("{}", e)).collect();
                write!(f, "[{}]", parts.join(", "))
            }
            Expr::TupleLit { items } => {
                let parts: Vec<String> = items.iter().map(|e| format!("{}", e)).collect();
                write!(f, "({})", parts.join(", "))
            }
            Expr::Lambda { params, body } => write!(f, "\\{} -> {}", params.join(" "), body),
            Expr::LetIn { bindings, body } => {
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
            } => {
                write!(f, "if {} then {} else {}", cond, then_branch, else_branch)
            }
            Expr::App { func, arg } => write!(f, "({} {})", func, arg),
            Expr::BinOp { op, left, right } => write!(f, "({} {} {})", left, op, right),
            Expr::Annot { expr, type_expr } => write!(f, "({} :: {:?})", expr, type_expr),
        }
    }
}
