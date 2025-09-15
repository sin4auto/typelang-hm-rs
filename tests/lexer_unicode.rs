// 日本語コメント: UTF-8と2文字トークンの境界でパニックしないことを確認

use typelang::lexer;

#[test]
fn lexer_no_panic_on_unicode_after_minus() {
    // '-' の直後に全角ダッシュ等が来てもパニックせず、エラーで返る
    let src = "let x = 1 -ー 2"; // ー(U+30FC)
    let res = lexer::lex(src);
    assert!(res.is_err());
}

#[test]
fn lexer_lambda_arrow_before_unicode_char_literal() {
    // ラムダ '->' の直後に日本語文字リテラルがあっても安全
    let src = r#"let f = \\x -> 'あ'"#;
    let res = lexer::lex(src);
    assert!(res.is_ok());
}
