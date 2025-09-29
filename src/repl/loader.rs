// パス: src/repl/loader.rs
// 役割: Program loader that merges definitions into REPL environments
// 意図: Safely import files with inference checks before evaluation
// 関連ファイル: src/infer.rs, src/evaluator.rs, src/repl/util.rs
//! TypeLang のプログラム定義を REPL 環境へ読み込むための補助モジュール。
//! 型推論・既定化・評価の順に処理し、安全に環境へ取り込む。

use crate::ast as A;
use crate::evaluator::{eval_expr, Value};
use crate::infer::{infer_expr, type_from_texpr, InferState};
use crate::typesys::{generalize, unify, TVarSupply};

use super::util::normalize_expr;
/// プログラムを型・クラス・値環境へ段階的に取り込む。
///
/// 定義ごとに式を正規化し、型推論・defaulting・評価を組み合わせて環境を更新する。
/// 型注釈が付いている場合は単一化で検証し、推論失敗時は評価結果から代表型を導出する。
///
/// # Errors
/// 型推論や評価、ファイル読み込みに失敗した場合は文字列化したエラーメッセージを返す。
pub fn load_program_into_env(
    prog: &A::Program,
    type_env: &mut crate::typesys::TypeEnv,
    class_env: &crate::typesys::ClassEnv,
    value_env: &mut crate::evaluator::Env,
) -> Result<Vec<String>, String> {
    let mut type_env_tmp = type_env.clone_env();
    let mut value_env_tmp = value_env.clone();
    let mut loaded: Vec<String> = Vec::new();
    for decl in &prog.decls {
        let orig = if decl.params.is_empty() {
            decl.expr.clone()
        } else {
            A::Expr::Lambda {
                params: decl.params.clone(),
                body: Box::new(decl.expr.clone()),
            }
        };
        let body = normalize_expr(&orig);
        let mut st = InferState {
            supply: TVarSupply::new(),
            subst: Default::default(),
        };
        let (s, q_rhs0) = match infer_expr(&type_env_tmp, class_env, &mut st, &body) {
            Ok(ok) => ok,
            Err(_e) => {
                // 推論が失敗した場合は値を評価し、代表的な型を一般化して登録する。
                let val = eval_expr(&body, &mut value_env_tmp).map_err(|e2| e2.to_string())?;
                let sch = generalize(
                    &type_env_tmp,
                    crate::typesys::qualify(
                        crate::typesys::Type::TCon(crate::typesys::TCon {
                            name: match val {
                                Value::Int(_) => "Int".into(),
                                Value::Double(_) => "Double".into(),
                                Value::Bool(_) => "Bool".into(),
                                Value::Char(_) => "Char".into(),
                                Value::String(_) => "[Char]".into(),
                                _ => "()".into(),
                            },
                        }),
                        vec![],
                    ),
                );
                type_env_tmp.extend(decl.name.clone(), sch);
                value_env_tmp.insert(decl.name.clone(), val);
                loaded.push(decl.name.clone());
                continue;
            }
        };
        let q_rhs1 = crate::typesys::apply_subst_q(&s, &q_rhs0);
        let is_fun = matches!(q_rhs1.r#type, crate::typesys::Type::TFun(_));
        let q_rhs = if decl.signature.is_none() && !is_fun {
            crate::typesys::apply_defaulting_simple(&q_rhs1)
        } else {
            q_rhs1
        };
        if let Some(sig) = &decl.signature {
            let ty_anno = type_from_texpr(&sig.r#type);
            let _s2 = unify(q_rhs.r#type.clone(), ty_anno)
                .map_err(|e| format!("[{}] {}", e.code, e.message))?;
        }
        let sch = generalize(&type_env_tmp, q_rhs.clone());
        type_env_tmp.extend(decl.name.clone(), sch);
        let val = eval_expr(&body, &mut value_env_tmp).map_err(|e| e.to_string())?;
        value_env_tmp.insert(decl.name.clone(), val);
        loaded.push(decl.name.clone());
    }
    *type_env = type_env_tmp;
    *value_env = value_env_tmp;
    Ok(loaded)
}
