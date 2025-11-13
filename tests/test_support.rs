// パス: tests/test_support.rs
// 役割: 統合テスト共通の補助関数とアサーションを提供する
// 意図: 繰り返しがちな評価・推論・字句解析操作を一元化しテストを簡潔に保つ
// 関連ファイル: tests/evaluator.rs, tests/lexer_parser.rs, tests/types_infer.rs
#![allow(dead_code)]
use typelang::{
    ast,
    errors::{EvalError, TypeError},
    evaluator, infer, lexer, parser, repl, typesys,
};

/// `load_program_into_env` の結果をまとめて保持し、テスト終了時に自動で teardown するフィクスチャ。
pub struct ProgramFixture {
    pub type_env: typesys::TypeEnv,
    pub class_env: typesys::ClassEnv,
    pub value_env: evaluator::Env,
    pub exports: Vec<String>,
}

impl ProgramFixture {
    pub fn load(src: &str) -> Self {
        let program = parse_program(src);
        let mut type_env = infer::initial_env();
        let mut class_env = infer::initial_class_env();
        let mut value_env = evaluator::initial_env();
        let exports =
            repl::load_program_into_env(&program, &mut type_env, &mut class_env, &mut value_env)
                .expect("load program fixture");
        Self {
            type_env,
            class_env,
            value_env,
            exports,
        }
    }

    pub fn eval_value(&self, expr_src: &str) -> evaluator::Value {
        let expr = parse_expr(expr_src);
        evaluator::eval_expr(&expr, &self.value_env).expect("eval expr in fixture")
    }

    pub fn eval_result(&self, expr_src: &str) -> Result<evaluator::Value, EvalError> {
        let expr = parse_expr(expr_src);
        evaluator::eval_expr(&expr, &self.value_env)
    }
}

impl Drop for ProgramFixture {
    fn drop(&mut self) {
        self.value_env.teardown();
    }
}

pub fn parse_expr(src: &str) -> ast::Expr {
    parser::parse_expr(src).expect("parse expr")
}

pub fn parse_program(src: &str) -> ast::Program {
    parser::parse_program(src).expect("parse program")
}

pub fn lex_ok(src: &str) -> Vec<lexer::Token> {
    lexer::lex(src).expect("lex")
}

pub fn infer_type_str(src: &str) -> String {
    let expr = parse_expr(src);
    infer::infer_type_str(&expr).expect("infer type")
}

pub fn infer_type_str_with_defaulting(src: &str, enable: bool) -> String {
    let expr = parse_expr(src);
    infer::infer_type_str_with_defaulting(&expr, enable).expect("infer type with defaulting")
}

pub fn infer_pretty_qual(src: &str) -> Result<String, TypeError> {
    let expr = parse_expr(src);
    let env = infer::initial_env();
    let class_env = infer::initial_class_env();
    let mut state = infer::InferState {
        supply: typesys::TVarSupply::new(),
    };
    infer::infer_expr(&env, &class_env, &mut state, &expr)
        .map(|(_, qual)| typesys::pretty_qual(&qual))
}

pub fn eval_result(src: &str) -> Result<evaluator::Value, EvalError> {
    let expr = parse_expr(src);
    let mut env = evaluator::initial_env();
    let result = evaluator::eval_expr(&expr, &env);
    env.teardown();
    result
}

pub fn eval_value(src: &str) -> evaluator::Value {
    eval_result(src).expect("eval value")
}

pub fn assert_value_int(value: evaluator::Value, expected: i64, note: &str) {
    match value {
        evaluator::Value::Int(actual) => assert_eq!(actual, expected, "{note}"),
        other => panic!("{note}: expected Int({expected}), got {:?}", other),
    }
}

pub fn assert_value_bool(value: evaluator::Value, expected: bool, note: &str) {
    match value {
        evaluator::Value::Bool(actual) => assert_eq!(actual, expected, "{note}"),
        other => panic!("{note}: expected Bool({expected}), got {:?}", other),
    }
}

pub fn assert_value_string(value: evaluator::Value, expected: &str, note: &str) {
    match value {
        evaluator::Value::String(actual) => assert_eq!(actual, expected, "{note}"),
        other => panic!("{note}: expected String({expected}), got {:?}", other),
    }
}

pub fn assert_value_double(value: evaluator::Value, expected: f64, note: &str) {
    match value {
        evaluator::Value::Double(actual) => assert!(
            approx_eq(actual, expected),
            "{note}: expected ≈ {expected}, got {actual}"
        ),
        other => panic!("{note}: expected Double({expected}), got {:?}", other),
    }
}

pub fn approx_eq(lhs: f64, rhs: f64) -> bool {
    if lhs.is_nan() && rhs.is_nan() {
        true
    } else {
        (lhs - rhs).abs() < 1e-12
    }
}
