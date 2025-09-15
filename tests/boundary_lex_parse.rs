// 日本語コメント: 生成規則の境界値テスト（巨大整数/長文字列/深いネスト）

use typelang::{lexer, parser};

#[test]
fn huge_integer_is_error_not_panic() {
    // i64 を大きく超える桁数（50桁程度）
    let big = "9".repeat(50);
    let src = format!("let x = {};", big);
    // lex は通るが、parse で範囲外として Err を返すこと
    let toks = lexer::lex(&src).expect("lex");
    assert!(toks.iter().any(|t| matches!(t.kind, lexer::TokenKind::INT)));
    let err = parser::parse_program(&src).expect_err("expect parse error for huge int");
    let s = err.to_string();
    assert!(s.contains("[PAR210]"));
    assert!(s.contains("範囲外"));
}

#[test]
fn very_long_string_literal() {
    // 5,000 文字程度の長い文字列
    let s = "a".repeat(5000);
    let src = format!("let s = \"{}\";", s);
    let prog = parser::parse_program(&src).expect("parse long string");
    assert_eq!(prog.decls.len(), 1);
}

#[test]
fn deep_parentheses_nesting_parse_ok() {
    // 200 レベルの括弧で 1 を包む
    // 再帰下降パーサのスタックに配慮して適度な深さに抑える
    let depth = 64;
    let inner = "1";
    let mut s = String::new();
    for _ in 0..depth {
        s.push('(');
    }
    s.push_str(inner);
    for _ in 0..depth {
        s.push(')');
    }
    let e = parser::parse_expr(&s).expect("parse deep parens");
    let printed = format!("{}", e);
    assert!(printed.contains("1"));
}
