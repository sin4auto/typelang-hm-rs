// パス: tests/lexer_more.rs
// 役割: Lexer regression tests for error handling and floats
// 意図: Detect regressions around literal parsing edge cases
// 関連ファイル: src/lexer.rs, src/errors.rs, tests/lexer_additional.rs
use typelang::lexer;

// 追加: 字句解析の異常系テスト
#[test]
fn lexer_error_unclosed_string() {
    let src = "\"abc"; // 末尾が閉じない
    let res = lexer::lex(src);
    assert!(res.is_err());
}

#[test]
fn lexer_error_unclosed_block_comment() {
    let src = "{- never closed";
    let res = lexer::lex(src);
    assert!(res.is_err());
}

#[test]
fn lexer_error_invalid_hex_literal() {
    let src = "let x = 0x;"; // 16 進の桁なし
    let res = lexer::lex(src);
    assert!(res.is_err());
}

// 正常系: 浮動小数の指数表記
#[test]
fn lexer_float_with_exponent() {
    let src = "let x = 1.2e-3;";
    let toks = lexer::lex(src).expect("lex");
    assert!(toks
        .iter()
        .any(|t| matches!(t.kind, lexer::TokenKind::FLOAT)));
}
