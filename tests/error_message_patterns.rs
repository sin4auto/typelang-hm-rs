// 日本語コメント: エラーメッセージの詳細一致（部分一致）

use typelang::{lexer, parser};

#[test]
fn error_unclosed_block_comment_message() {
    let err = lexer::lex("{- never closed").expect_err("expect block comment error");
    let s = err.to_string();
    assert!(s.contains("[LEX001]"));
    assert!(s.contains("ブロックコメントが閉じていません"));
}
#[test]
fn error_unclosed_string_message() {
    let err = lexer::lex("\"abc").expect_err("expect unclosed string error");
    let s = err.to_string();
    assert!(s.contains("[LEX003]"));
    assert!(s.contains("文字列リテラルが閉じていません"));
}

#[test]
fn error_extra_tokens_remain_message() {
    // 式末尾に余計なセミコロン
    let err = parser::parse_expr("1; 2").expect_err("expect extra tokens error");
    let s = err.to_string();
    assert!(s.contains("[PAR090]"));
    assert!(s.contains("余分なトークンが残っています"));
}
