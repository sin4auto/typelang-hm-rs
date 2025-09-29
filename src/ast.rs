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

// 式を構成する列挙体
#[derive(Clone, Debug, PartialEq)]
/// 言語内の式を表す AST ノードの集合。
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
/// トップレベル定義の集まりとしてのプログラム。
pub struct Program {
    pub decls: Vec<TopLevel>,
}

/// 式ノードを文字列表現へ整形する。
impl fmt::Display for Expr {
    /// デバッグしやすい括弧付きの表記に変換する。
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
