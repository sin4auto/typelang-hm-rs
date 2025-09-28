// パス: tests/infer_more.rs
// 役割: 数値制約と defaulting の推論カバレッジ
// 意図: 累乗演算と defaulting の特別挙動を検証する
// 関連ファイル: src/infer.rs, src/typesys.rs, tests/infer_additional.rs
use typelang::{infer, parser};

#[test]
fn infer_pow_neg_int_yields_double() {
    // '^' かつ負の指数は Double にフォールバック（特別扱い）
    let e = parser::parse_expr("2 ^ -3").expect("parse");
    let ty = infer::infer_type_str(&e).expect("infer");
    assert_eq!(ty, "Double");
}

#[test]
fn infer_starstar_with_defaulting_is_double() {
    // '**' は Fractional 制約。defaulting on で Double に既定化
    let e = parser::parse_expr("2 ** -1").expect("parse");
    let ty = infer::infer_type_str_with_defaulting(&e, true).expect("infer");
    assert_eq!(ty, "Double");
}

#[test]
fn infer_ambiguous_add_without_defaulting_keeps_constraint() {
    // defaulting off の場合、Num 制約が残る
    let e = parser::parse_expr("1 + 2").expect("parse");
    let ty = infer::infer_type_str_with_defaulting(&e, false).expect("infer");
    assert_eq!(ty, "Num a => a");
}
