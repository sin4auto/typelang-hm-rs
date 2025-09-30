// パス: tests/evaluator.rs
// 役割: 評価器の正常系と代表的な失敗ケースを最小構成で検証
// 意図: 数値演算・比較・pow・show の挙動が回帰しないようにする
// 関連ファイル: src/evaluator.rs, src/parser.rs, tests/types_infer.rs
use typelang::{evaluator, parser, EvalError};

/// 式文字列を評価し、成功・失敗をそのまま返すヘルパ。
fn eval_result(src: &str) -> Result<evaluator::Value, EvalError> {
    let expr = parser::parse_expr(src).expect("parse");
    let mut env = evaluator::initial_env();
    evaluator::eval_expr(&expr, &mut env)
}

/// 式文字列を評価して `Value` を得るヘルパ。
fn eval_value(src: &str) -> evaluator::Value {
    eval_result(src).expect("eval")
}

/// 式文字列を評価して Bool を取り出すヘルパ。
fn eval_bool(src: &str) -> bool {
    match eval_value(src) {
        evaluator::Value::Bool(b) => b,
        other => panic!("expected Bool, got {:?}", other),
    }
}

/// Bool を返す式と期待値をまとめて検証するヘルパ。
fn assert_bool_result(src: &str, expected: bool) {
    assert_eq!(
        eval_bool(src),
        expected,
        "期待値 {expected} に一致しません: {src}"
    );
}

/// Double を返す式と期待値を比較検証するヘルパ。
fn assert_double_value(src: &str, expected: f64) {
    match eval_value(src) {
        evaluator::Value::Double(d) => {
            assert!(
                (d - expected).abs() < 1e-12,
                "期待値 {expected} と誤差が大きい: {d}"
            );
        }
        other => panic!("expected Double, got {:?}", other),
    }
}

/// Int を返す式と期待値を比較検証するヘルパ。
fn assert_int_value(src: &str, expected: i64) {
    match eval_value(src) {
        evaluator::Value::Int(i) => assert_eq!(i, expected),
        other => panic!("expected Int, got {:?}", other),
    }
}

/// 評価が指定したエラーコードで失敗することを検証するヘルパ。
fn assert_eval_error(src: &str, expected_code: &str) {
    match eval_result(src) {
        Err(EvalError(info)) => {
            assert_eq!(info.code, expected_code, "期待するコード {expected_code}");
        }
        Ok(value) => panic!("expected error {expected_code}, got value {:?}", value),
    }
}

#[test]
/// 整数の等価性と順序を検証する。
fn eval_int_equality_and_ordering() {
    assert_bool_result("1 == 1", true);
    assert_bool_result("2 < 1", false);
}

#[test]
/// 浮動小数の比較が正しく行われるかを検証する。
fn eval_double_comparisons() {
    assert_bool_result("2.0 >= -1", true);
}

#[test]
/// タプルとリストの比較挙動を検証する。
fn eval_tuple_and_list_comparisons() {
    assert_bool_result("(1,2) == (1,2)", true);
    assert_bool_result("[1,2] == [1,2]", true);
    assert_bool_result("[1,2] < [1,3]", true);
}

#[test]
/// タプルの順序が辞書式になることを検証する。
fn eval_tuple_ordering_is_lexicographic() {
    assert_bool_result("(1,3) < (1,4)", true);
}

#[test]
/// 除算結果が `Double` へ昇格することを検証する。
fn eval_division_promotes_to_double() {
    assert_double_value("1 / 2", 0.5);
}

#[test]
/// 累乗計算の挙動を検証する。
fn eval_pow_behaviour() {
    assert_int_value("2 ^ 3 ^ 2", 512);
    assert_double_value("2 ^ -1", 0.5);
    assert_int_value("(-2) ^ 3", -8);
}

#[test]
/// 非常に大きな累乗がオーバーフローエラーになることを検証する。
fn eval_pow_overflow_is_error() {
    assert_eval_error("2 ^ 2 ^ 2 ^ 2 ^ 2", "EVAL060");
}

#[test]
/// `**` が負の指数でもDoubleとして動作することを検証する。
fn eval_powf_negative_defaulting() {
    assert_double_value("2 ** -1", 0.5);
}

#[test]
/// NaN 比較がエラーになることを検証する。
fn eval_nan_comparison_is_error() {
    assert_eval_error("(0.0 / 0.0) < 1.0", "EVAL090");
}

#[test]
/// NaN 同士の等価比較が false を返し、エラーにならないことを検証する。
fn eval_nan_equality_is_false() {
    assert_bool_result("(0.0 / 0.0) == (0.0 / 0.0)", false);
}

#[test]
/// 関数に `show` を適用するとエラーになることを検証する。
fn eval_show_function_is_error() {
    assert_eval_error("show (\\x -> x)", "EVAL050");
}

#[test]
/// 関数でない値の適用がエラーになることを検証する。
fn eval_apply_non_function_is_error() {
    assert_eval_error("1 2", "EVAL020");
}

#[test]
/// `show` が整数を文字列化して返すことを検証する。
fn eval_show_int_returns_string() {
    match eval_value("show 42") {
        evaluator::Value::String(s) => assert_eq!(s, "42"),
        other => panic!("expected String, got {:?}", other),
    }
}
