// パス: src/repl/loader.rs
// 役割: Program loader that merges definitions into REPL environments
// 意図: Safely import files with inference checks before evaluation
// 関連ファイル: src/infer.rs, src/evaluator.rs, src/repl/util.rs
//! TypeLang のプログラム定義を REPL 環境へ読み込むための補助モジュール。
//! 型推論・既定化・評価の順に処理し、安全に環境へ取り込む。

use crate::ast as A;
use crate::infer::type_from_texpr;
use crate::typesys::{generalize, unify};

use super::pipeline::{eval_expr_for_pipeline, fallback_scheme_from_value, infer_qual_type};
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
        let should_default = decl.signature.is_none() && decl.params.is_empty();
        match infer_qual_type(&type_env_tmp, class_env, &body, should_default) {
            Ok(q_rhs) => {
                if let Some(sig) = &decl.signature {
                    let ty_anno = type_from_texpr(&sig.r#type);
                    unify(q_rhs.r#type.clone(), ty_anno)
                        .map_err(|e| format!("[{}] {}", e.code, e.message))?;
                }
                let sch = generalize(&type_env_tmp, q_rhs);
                let val =
                    eval_expr_for_pipeline(&body, &mut value_env_tmp).map_err(|e| e.to_string())?;
                type_env_tmp.extend(decl.name.clone(), sch);
                value_env_tmp.insert(decl.name.clone(), val);
                loaded.push(decl.name.clone());
            }
            Err(_) => {
                let val =
                    eval_expr_for_pipeline(&body, &mut value_env_tmp).map_err(|e| e.to_string())?;
                let sch = fallback_scheme_from_value(&type_env_tmp, &val);
                type_env_tmp.extend(decl.name.clone(), sch);
                value_env_tmp.insert(decl.name.clone(), val);
                loaded.push(decl.name.clone());
            }
        }
    }
    *type_env = type_env_tmp;
    *value_env = value_env_tmp;
    Ok(loaded)
}
