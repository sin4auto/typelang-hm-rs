// 日本語コメント: 字句解析の正常系/異常系を追加検証

use typelang::lexer;

#[test]
fn lexer_hex_oct_bin_literals() {
    let src = "let a = 0x1f; let b = 0o77; let c = 0b1010;";
    let toks = lexer::lex(src).expect("lex");
    assert!(toks.iter().any(|t| t.kind == lexer::TokenKind::HEX));
    assert!(toks.iter().any(|t| t.kind == lexer::TokenKind::OCT));
    assert!(toks.iter().any(|t| t.kind == lexer::TokenKind::BIN));
}

#[test]
fn lexer_error_invalid_bin_and_oct_literal() {
    // 0b と 0o の後に桁がない場合はエラー
    assert!(lexer::lex("let x = 0b;").is_err());
    assert!(lexer::lex("let y = 0o;").is_err());
}

#[test]
fn lexer_float_forms() {
    // 1.0 / 1e0 を Float として認識
    let t1 = lexer::lex("let a = 1.0;").expect("lex 1.0");
    let t2 = lexer::lex("let b = 1e0;").expect("lex 1e0");
    assert!(t1.iter().any(|t| matches!(t.kind, lexer::TokenKind::FLOAT)));
    assert!(t2.iter().any(|t| matches!(t.kind, lexer::TokenKind::FLOAT)));
}

#[test]
fn lexer_string_and_char_escapes() {
    let s = lexer::lex("let s = \"a\\n\\\"b\";").expect("lex str");
    let c = lexer::lex("let c = '\\n';").expect("lex chr");
    assert!(s.iter().any(|t| t.kind == lexer::TokenKind::STRING));
    assert!(c.iter().any(|t| t.kind == lexer::TokenKind::CHAR));
}
