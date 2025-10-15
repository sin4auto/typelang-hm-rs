// パス: src/repl/loader.rs
// 役割: Program loader that merges definitions into REPL environments
// 意図: Safely import files with inference checks before evaluation
// 関連ファイル: src/infer.rs, src/evaluator.rs, src/repl/util.rs
//! TypeLang のプログラム定義を REPL 環境へ読み込むための補助モジュール。
//! 型推論・既定化・評価の順に処理し、安全に環境へ取り込む。

use std::collections::HashMap;

use crate::ast as A;
use crate::infer::{type_from_texpr, type_from_texpr_with_vars};
use crate::runtime::make_data_ctor;
use crate::typesys::{
    generalize, qualify, unify, Scheme, TApp, TCon, TFun, TVar, TVarSupply, Type,
};

use super::pipeline::{eval_expr_for_pipeline, fallback_scheme_from_value, infer_qual_type};
use super::util::normalize_expr;
/// プログラムを型・クラス・値環境へ段階的に取り込む。
///
/// 定義ごとに式を正規化し、型推論・defaulting・評価を組み合わせて環境を更新する。
/// 型注釈が付いている場合は単一化で検証し、推論失敗時は評価結果から代表型を導出する。
///
/// # Errors
/// 型推論や評価、ファイル読み込みに失敗した場合は文字列化したエラーメッセージを返す。
#[cfg_attr(coverage, coverage(off))]
pub fn load_program_into_env(
    prog: &A::Program,
    type_env: &mut crate::typesys::TypeEnv,
    class_env: &mut crate::typesys::ClassEnv,
    value_env: &mut crate::evaluator::Env,
) -> Result<Vec<String>, String> {
    let mut type_env_tmp = type_env.clone_env();
    let mut class_env_tmp = class_env.clone();
    let mut value_env_tmp = crate::evaluator::Env::from_map(value_env.snapshot());
    let mut loaded: Vec<String> = Vec::new();

    for class_decl in &prog.class_decls {
        register_class_decl(class_decl, &mut class_env_tmp).map_err(|e| format!("[TYPE] {e}"))?;
    }
    for instance_decl in &prog.instance_decls {
        register_instance_decl(instance_decl, &mut class_env_tmp)
            .map_err(|e| format!("[TYPE] {e}"))?;
    }
    for data_decl in &prog.data_decls {
        register_data_decl(data_decl, &mut type_env_tmp, &mut value_env_tmp)
            .map_err(|e| format!("[TYPE] {e}"))?;
    }
    for decl in &prog.decls {
        let orig = if decl.params.is_empty() {
            decl.expr.clone()
        } else {
            A::Expr::Lambda {
                params: decl.params.clone(),
                body: Box::new(decl.expr.clone()),
                span: A::Span::dummy(),
            }
        };
        let body = normalize_expr(&orig);
        let should_default = decl.signature.is_none() && decl.params.is_empty();
        match infer_qual_type(&type_env_tmp, &class_env_tmp, &body, should_default) {
            Ok(q_rhs) => {
                if let Some(sig) = &decl.signature {
                    let ty_anno = type_from_texpr(&sig.r#type);
                    unify(q_rhs.r#type.clone(), ty_anno)
                        .map_err(|e| format!("[{}] {}", e.code, e.message))?;
                }
                let sch = generalize(&type_env_tmp, q_rhs);
                let val =
                    eval_expr_for_pipeline(&body, &value_env_tmp).map_err(|e| e.to_string())?;
                type_env_tmp.extend(decl.name.clone(), sch);
                value_env_tmp.insert(decl.name.clone(), val);
                loaded.push(decl.name.clone());
            }
            Err(_) => {
                let val =
                    eval_expr_for_pipeline(&body, &value_env_tmp).map_err(|e| e.to_string())?;
                let sch = fallback_scheme_from_value(&type_env_tmp, &val);
                type_env_tmp.extend(decl.name.clone(), sch);
                value_env_tmp.insert(decl.name.clone(), val);
                loaded.push(decl.name.clone());
            }
        }
    }
    *type_env = type_env_tmp;
    *class_env = class_env_tmp;
    *value_env = value_env_tmp;
    Ok(loaded)
}

#[cfg_attr(coverage, coverage(off))]
fn register_data_decl(
    decl: &A::DataDecl,
    type_env: &mut crate::typesys::TypeEnv,
    value_env: &mut crate::evaluator::Env,
) -> Result<(), String> {
    let mut tv_supply = TVarSupply::new();
    let mut params: HashMap<String, TVar> = HashMap::new();
    for name in &decl.params {
        if params.contains_key(name) {
            return Err(format!("型パラメータ {name} が重複しています"));
        }
        params.insert(name.clone(), tv_supply.fresh());
    }

    let mut result_type = Type::TCon(TCon {
        name: decl.name.clone(),
    });
    for name in &decl.params {
        let tv = params
            .get(name)
            .ok_or_else(|| format!("型パラメータ {name} が未登録です"))?;
        result_type = Type::TApp(TApp {
            func: Box::new(result_type),
            arg: Box::new(Type::TVar(tv.clone())),
        });
    }

    let quantified: Vec<TVar> = decl
        .params
        .iter()
        .filter_map(|name| params.get(name).cloned())
        .collect();

    let mut seen_ctor: HashMap<String, ()> = HashMap::new();
    for ctor in &decl.constructors {
        if seen_ctor.insert(ctor.name.clone(), ()).is_some() {
            return Err(format!("コンストラクタ {} が重複しています", ctor.name));
        }
        if type_env.lookup(&ctor.name).is_some() {
            return Err(format!("{} は既に定義済みです", ctor.name));
        }

        let arg_types: Vec<Type> = ctor
            .args
            .iter()
            .map(|texpr| type_from_texpr_with_vars(texpr, &params))
            .collect();

        let mut ty = result_type.clone();
        for arg_ty in arg_types.iter().rev() {
            ty = Type::TFun(TFun {
                arg: Box::new(arg_ty.clone()),
                ret: Box::new(ty),
            });
        }

        let scheme = Scheme {
            vars: quantified.clone(),
            qual: qualify(ty, vec![]),
        };
        type_env.extend(ctor.name.clone(), scheme);
        if value_env
            .insert(
                ctor.name.clone(),
                make_data_ctor(&ctor.name, ctor.args.len()),
            )
            .is_some()
        {
            return Err(format!("{} は値として既に定義済みです", ctor.name));
        }
    }

    Ok(())
}

#[cfg_attr(coverage, coverage(off))]
fn register_class_decl(
    decl: &A::ClassDecl,
    class_env: &mut crate::typesys::ClassEnv,
) -> Result<(), String> {
    if class_env.classes.contains_key(&decl.name) {
        return Err(format!("クラス {} は既に定義済みです", decl.name));
    }
    class_env.add_class(decl.name.clone(), decl.superclasses.clone());
    Ok(())
}

#[cfg_attr(coverage, coverage(off))]
fn register_instance_decl(
    decl: &A::InstanceDecl,
    class_env: &mut crate::typesys::ClassEnv,
) -> Result<(), String> {
    if !class_env.classes.contains_key(&decl.classname) {
        return Err(format!("クラス {} が未定義です", decl.classname));
    }
    class_env.add_instance(decl.classname.clone(), decl.tycon.clone());
    Ok(())
}
