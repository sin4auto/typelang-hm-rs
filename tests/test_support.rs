// パス: tests/test_support.rs
// 役割: 統合テスト共通の補助関数とアサーションを提供する
// 意図: 繰り返しがちな評価・推論・字句解析操作を一元化しテストを簡潔に保つ
// 関連ファイル: tests/evaluator.rs, tests/lexer_parser.rs, tests/types_infer.rs
#![allow(dead_code)]
use typelang::{
    ast,
    errors::{EvalError, TypeError},
    evaluator, infer, lexer, parser, typesys,
};

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
    let env = evaluator::initial_env();
    evaluator::eval_expr(&expr, &env)
}

pub fn eval_value(src: &str) -> evaluator::Value {
    eval_result(src).expect("eval value")
}

pub fn approx_eq(lhs: f64, rhs: f64) -> bool {
    if lhs.is_nan() && rhs.is_nan() {
        true
    } else {
        (lhs - rhs).abs() < 1e-12
    }
}
