// パス: src/repl/util.rs
// 役割: Expression normalization utilities for REPL workflows
// 意図: Ensure inference and evaluation behave predictably on user input
// 関連ファイル: src/repl/cmd.rs, src/infer.rs, src/ast.rs
//! REPL 内部で共有する式変換ユーティリティを提供するモジュール。
//! 推論が安定するようユーザー入力を事前整形する。

use crate::ast as A;
/// ユーザー入力の式を正規化し、負のべき乗などを推論しやすい形に直す。
pub(crate) fn normalize_expr(e: &A::Expr) -> A::Expr {
    use A::Expr::*;
    /// `0 - n` 形式の負整数リテラルを検出する補助関数。
    fn neg_int(e: &A::Expr) -> Option<i64> {
        if let A::Expr::BinOp { op, left, right } = e {
            if op == "-" {
                if let A::Expr::IntLit { value: 0, .. } = **left {
                    if let A::Expr::IntLit { value: n, .. } = **right {
                        return Some(-n);
                    }
                }
            }
        }
        None
    }
    match e {
        BinOp { op, left, right } => {
            let l = Box::new(normalize_expr(left));
            let r = Box::new(normalize_expr(right));
            if op == "^" {
                if let Some(n) = neg_int(&r) {
                    return BinOp {
                        op: "**".into(),
                        left: l,
                        right: Box::new(FloatLit { value: n as f64 }),
                    };
                }
            }
            BinOp {
                op: op.clone(),
                left: l,
                right: r,
            }
        }
        App { func, arg } => App {
            func: Box::new(normalize_expr(func)),
            arg: Box::new(normalize_expr(arg)),
        },
        Lambda { params, body } => Lambda {
            params: params.clone(),
            body: Box::new(normalize_expr(body)),
        },
        LetIn { bindings, body } => {
            let bs: Vec<_> = bindings
                .iter()
                .map(|(n, ps, ex)| (n.clone(), ps.clone(), normalize_expr(ex)))
                .collect();
            LetIn {
                bindings: bs,
                body: Box::new(normalize_expr(body)),
            }
        }
        If {
            cond,
            then_branch,
            else_branch,
        } => If {
            cond: Box::new(normalize_expr(cond)),
            then_branch: Box::new(normalize_expr(then_branch)),
            else_branch: Box::new(normalize_expr(else_branch)),
        },
        Annot { expr, type_expr } => Annot {
            expr: Box::new(normalize_expr(expr)),
            type_expr: type_expr.clone(),
        },
        ListLit { items } => ListLit {
            items: items.iter().map(normalize_expr).collect(),
        },
        TupleLit { items } => TupleLit {
            items: items.iter().map(normalize_expr).collect(),
        },
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_expr;
    use crate::ast as A;

    #[test]
    /// 負の整数指数が `^` から `**` へ変換されることを確認する。
    fn normalize_pow_with_negative_int_exponent_changes_to_starstar() {
        let e = A::Expr::BinOp {
            op: "^".into(),
            left: Box::new(A::Expr::IntLit {
                value: 2,
                base: A::IntBase::Dec,
            }),
            right: Box::new(A::Expr::BinOp {
                op: "-".into(),
                left: Box::new(A::Expr::IntLit {
                    value: 0,
                    base: A::IntBase::Dec,
                }),
                right: Box::new(A::Expr::IntLit {
                    value: 3,
                    base: A::IntBase::Dec,
                }),
            }),
        };
        let n = normalize_expr(&e);
        match n {
            A::Expr::BinOp { op, left: _, right } => {
                assert_eq!(op, "**");
                assert!(
                    matches!(*right, A::Expr::FloatLit { value } if (value - (-3.0)).abs() < 1e-12)
                );
            }
            _ => panic!("not normalized to BinOp"),
        }
    }

    #[test]
    /// 対象外の演算子がそのまま残ることを検証する。
    fn normalize_keeps_other_ops_untouched() {
        let e = A::Expr::BinOp {
            op: "+".into(),
            left: Box::new(A::Expr::IntLit {
                value: 1,
                base: A::IntBase::Dec,
            }),
            right: Box::new(A::Expr::IntLit {
                value: 2,
                base: A::IntBase::Dec,
            }),
        };
        let n = normalize_expr(&e);
        match n {
            A::Expr::BinOp { op, .. } => assert_eq!(op, "+"),
            _ => panic!("unexpected"),
        }
    }

    #[test]
    /// コレクション内部でも再帰的に正規化されることを確認する。
    fn normalize_recurse_into_collections() {
        // [(2 ^ -1)] のような入れ子でも再帰的に正規化されることを確認する
        let e = A::Expr::ListLit {
            items: vec![A::Expr::BinOp {
                op: "^".into(),
                left: Box::new(A::Expr::IntLit {
                    value: 2,
                    base: A::IntBase::Dec,
                }),
                right: Box::new(A::Expr::BinOp {
                    op: "-".into(),
                    left: Box::new(A::Expr::IntLit {
                        value: 0,
                        base: A::IntBase::Dec,
                    }),
                    right: Box::new(A::Expr::IntLit {
                        value: 1,
                        base: A::IntBase::Dec,
                    }),
                }),
            }],
        };
        let n = normalize_expr(&e);
        if let A::Expr::ListLit { items } = n {
            if let A::Expr::BinOp { op, .. } = &items[0] {
                assert_eq!(op, "**");
                return;
            }
        }
        panic!("not normalized in collection");
    }
}
