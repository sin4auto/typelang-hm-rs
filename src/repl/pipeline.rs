// パス: src/repl/pipeline.rs
// 役割: REPL向け推論・評価パイプラインの共通ヘルパを提供する
// 意図: コマンド処理とローダ間で推論ロジックを共有して重複と不整合を防ぐ
// 関連ファイル: src/repl/cmd.rs, src/repl/loader.rs, src/infer.rs

use crate::ast as A;
use crate::evaluator::Value;
use crate::infer::{infer_expr, InferState};
use crate::typesys::{
    apply_defaulting_simple, generalize, qualify, t_string, ClassEnv, QualType, Scheme,
    Substitutable, TCon, TTuple, Type, TypeEnv,
};
use crate::{
    errors::{EvalError, TypeError},
    evaluator,
};

use super::util::normalize_expr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EvaluationMode {
    Always,
    OnInferenceFailure,
}

pub(crate) struct PipelineResult {
    pub qual: QualType,
    pub scheme: Scheme,
    pub value: Option<Value>,
}

/// 式を推論し、必要に応じて defaulting を適用した `QualType` を返す。
pub(crate) fn infer_qual_type(
    type_env: &TypeEnv,
    class_env: &ClassEnv,
    expr: &A::Expr,
    defaulting_on: bool,
) -> Result<QualType, TypeError> {
    let mut st = InferState {
        supply: Default::default(),
    };
    let (subst, qual) = infer_expr(type_env, class_env, &mut st, expr)?;
    let mut applied = qual.apply_subst(&subst);
    if defaulting_on {
        applied = apply_defaulting_simple(&applied);
    }
    Ok(applied)
}

/// 評価結果から復旧用の型スキームを構築する。
pub(crate) fn fallback_scheme_from_value(type_env: &TypeEnv, value: &Value) -> Scheme {
    let fallback_type = fallback_type_from_value(value);
    generalize(type_env, qualify(fallback_type, vec![]))
}

/// 評価結果に対応する `QualType` を返す。
pub(crate) fn fallback_qual_from_value(value: &Value) -> QualType {
    QualType {
        constraints: vec![],
        r#type: fallback_type_from_value(value),
    }
}

/// 推論失敗時などに利用するフォールバック型を算出する。
fn fallback_type_from_value(value: &Value) -> Type {
    match value {
        Value::Int(_) => Type::TCon(TCon { name: "Int".into() }),
        Value::Double(_) => Type::TCon(TCon {
            name: "Double".into(),
        }),
        Value::Bool(_) => Type::TCon(TCon {
            name: "Bool".into(),
        }),
        Value::Char(_) => Type::TCon(TCon {
            name: "Char".into(),
        }),
        Value::String(_) => t_string(),
        Value::List(_) | Value::Tuple(_) => Type::TTuple(TTuple { items: vec![] }),
        Value::Data { .. } | Value::Closure { .. } | Value::Prim(_) => {
            Type::TTuple(TTuple { items: vec![] })
        }
    }
}

/// ユーティリティ: 式を評価し、結果値を返す。
pub(crate) fn eval_expr_for_pipeline(
    expr: &A::Expr,
    env: &mut evaluator::Env,
) -> Result<Value, EvalError> {
    evaluator::eval_expr(expr, env)
}

/// 型推論と評価を一度ずつ実行し、REPL コマンド共通の結果を返す。
pub(crate) fn run_repl_pipeline(
    type_env: &TypeEnv,
    class_env: &ClassEnv,
    expr: &A::Expr,
    defaulting_on: bool,
    value_env: &mut evaluator::Env,
    mode: EvaluationMode,
) -> Result<PipelineResult, String> {
    let normalized = normalize_expr(expr);
    match infer_qual_type(type_env, class_env, &normalized, defaulting_on) {
        Ok(qual) => {
            let scheme = generalize(type_env, qual.clone());
            let value = if matches!(mode, EvaluationMode::Always) {
                Some(eval_expr_for_pipeline(&normalized, value_env).map_err(|e| e.to_string())?)
            } else {
                None
            };
            Ok(PipelineResult {
                qual,
                scheme,
                value,
            })
        }
        Err(_) => {
            let value =
                eval_expr_for_pipeline(&normalized, value_env).map_err(|e| e.to_string())?;
            let qual = fallback_qual_from_value(&value);
            let scheme = fallback_scheme_from_value(type_env, &value);
            Ok(PipelineResult {
                qual,
                scheme,
                value: Some(value),
            })
        }
    }
}
