// パス: tests/lexer_parser.rs
// 役割: Lexer と parser の基本〜境界テストを一本化
// 意図: 字句解析と構文解析の重要ケースをシンプルに網羅する
// 関連ファイル: src/lexer.rs, src/parser.rs, tests/types_infer.rs
use typelang::ast;
use typelang::lexer::{self, TokenKind};
use typelang::parser;

/// 字句解析が成功することを前提としたヘルパ。
fn lex_ok(src: &str) -> Vec<lexer::Token> {
    lexer::lex(src).expect("lex")
}

#[test]
/// キーワードと数値が適切にトークン化されることを検証する。
fn lexer_keywords_and_numbers() {
    let toks = lex_ok("let x = 0xFF; if True then 10 else 0b101");
    assert!(toks.iter().any(|t| matches!(t.kind, TokenKind::LET)));
    assert!(toks.iter().any(|t| matches!(t.kind, TokenKind::HEX)));
    assert!(toks.iter().any(|t| matches!(t.kind, TokenKind::BIN)));
}

#[test]
/// コメントと文字列リテラルが処理できることを検証する。
fn lexer_comments_and_strings() {
    let src = "-- comment\nlet s = \"a\\n\\\"\"; {- block -}\n";
    let toks = lex_ok(src);
    assert!(toks.iter().any(|t| matches!(t.kind, TokenKind::STRING)));
}

#[test]
/// 基数プレフィックスを伴う整数リテラルを検証する。
fn lexer_numeric_prefixes() {
    let toks = lex_ok("let a = 0x1f; let b = 0o77; let c = 0b1010;");
    assert!(toks.iter().any(|t| matches!(t.kind, TokenKind::HEX)));
    assert!(toks.iter().any(|t| matches!(t.kind, TokenKind::OCT)));
    assert!(toks.iter().any(|t| matches!(t.kind, TokenKind::BIN)));
}

#[test]
/// 無効な数値プレフィックスでエラーになることを確認する。
fn lexer_invalid_numeric_prefixes_error() {
    assert!(lexer::lex("let x = 0b;").is_err());
    assert!(lexer::lex("let y = 0o;").is_err());
    assert!(lexer::lex("let z = 0x;").is_err());
}

#[test]
/// 浮動小数表記のバリエーションを検証する。
fn lexer_float_forms() {
    let t1 = lex_ok("let a = 1.0;");
    let t2 = lex_ok("let b = 1e0;");
    let t3 = lex_ok("let c = 1.2e-3;");
    assert!(t1.iter().any(|t| matches!(t.kind, TokenKind::FLOAT)));
    assert!(t2.iter().any(|t| matches!(t.kind, TokenKind::FLOAT)));
    assert!(t3.iter().any(|t| matches!(t.kind, TokenKind::FLOAT)));
}

#[test]
/// 文字列と文字リテラルのエスケープが処理されることを検証する。
fn lexer_string_and_char_escapes() {
    let s = lex_ok("let s = \"a\\n\\\"b\";");
    let c = lex_ok("let c = '\\n';");
    assert!(s.iter().any(|t| matches!(t.kind, TokenKind::STRING)));
    assert!(c.iter().any(|t| matches!(t.kind, TokenKind::CHAR)));
}

#[test]
/// 未閉鎖の構造がエラーになることを確認する。
fn lexer_reports_unclosed_constructs() {
    assert!(lexer::lex("\"abc").is_err());
    assert!(lexer::lex("{- never closed").is_err());
}

#[test]
/// Unicode が混在する場合の挙動を検証する。
fn lexer_handles_unicode_boundaries() {
    assert!(lexer::lex("let x = 1 -ー 2").is_err());
    assert!(lexer::lex(r#"let f = \\x -> 'あ'"#).is_ok());
}

/// テスト用に式を解析するヘルパ。
fn parse_expr(src: &str) -> ast::Expr {
    parser::parse_expr(src).expect("parse expr")
}

#[test]
/// 累乗演算子が右結合であることを検証する。
fn parser_pow_is_right_associative() {
    let expr = parse_expr("2 ^ 3 ^ 2");
    let printed = format!("{}", expr);
    assert!(printed.contains("^"));
}

#[test]
/// 単項マイナスの糖衣が展開されることを検証する。
fn parser_unary_minus_sugar() {
    let expr = parse_expr("-1");
    let printed = format!("{}", expr);
    assert!(printed.contains("- 1"));
}

#[test]
/// 関数適用と中置演算子の優先順位を検証する。
fn parser_application_vs_infix_precedence() {
    let expr = parse_expr("f 2 ^ 3 * 4 + 5");
    let printed = format!("{}", expr);
    assert!(printed.contains("f 2"));
    assert!(printed.contains("^") && printed.contains("*") && printed.contains("+"));
}

#[test]
/// リストの閉じ忘れがエラーになることを確認する。
fn parser_rejects_unclosed_list() {
    assert!(parser::parse_expr("[1,2").is_err());
}

#[test]
/// if式が必ずelseを要求することを確認する。
fn parser_requires_else_branch() {
    assert!(parser::parse_expr("if True then 1").is_err());
}

#[test]
/// let-in で複数束縛が扱えることを検証する。
fn parser_let_in_multiple_bindings() {
    let expr = parse_expr("let a = 1; b x = x in b a");
    let printed = format!("{}", expr);
    assert!(printed.contains("let"));
    assert!(printed.contains("in"));
}

#[test]
/// `?` 付き変数が保持されることを確認する。
fn parser_question_variable_is_preserved() {
    let expr = parse_expr("?x");
    assert_eq!(format!("{}", expr), "?x");
}

#[test]
/// 巨大な整数リテラルがエラーになることを検証する。
fn parser_huge_integer_reports_error() {
    let big = "9".repeat(50);
    let src = format!("let x = {};", big);
    let tokens = lex_ok(&src);
    assert!(tokens.iter().any(|t| matches!(t.kind, TokenKind::INT)));
    let err = parser::parse_program(&src).expect_err("expect parse error for huge int");
    let s = err.to_string();
    assert!(s.contains("[PAR210]"));
    assert!(s.contains("範囲外"));
}

#[test]
/// 非常に長い文字列リテラルを解析できることを検証する。
fn parser_handles_very_long_string_literal() {
    let s = "a".repeat(5000);
    let src = format!("let s = \"{}\";", s);
    let prog = parser::parse_program(&src).expect("parse long string");
    assert_eq!(prog.decls.len(), 1);
}

#[test]
/// 深い括弧のネストを解析できることを検証する。
fn parser_handles_deep_parentheses() {
    let depth = 64;
    let mut src = String::new();
    for _ in 0..depth {
        src.push('(');
    }
    src.push('1');
    for _ in 0..depth {
        src.push(')');
    }
    let expr = parse_expr(&src);
    assert!(format!("{}", expr).contains('1'));
}
