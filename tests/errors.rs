// パス: tests/errors.rs
// 役割: エラー表示と代表的な診断メッセージの安定性を検証
// 意図: ユーザー向けのエラーテキストと失敗シナリオが回帰しないようにする
// 関連ファイル: src/errors.rs, src/lexer.rs, src/parser.rs
use typelang::errors::ErrorInfo;
use typelang::{infer, lexer, parser, typesys};

#[test]
fn error_display_line_col_pos_and_snippet() {
    let msg = ErrorInfo::at("E001", "msg", Some(12), Some(3), Some(5))
        .with_snippet("abcdef")
        .to_string();
    assert_eq!(msg, "[E001] msg @line=3,col=5 @pos=12\nabcdef\n    ^");
}

#[test]
fn error_display_line_col_only() {
    let msg = ErrorInfo::at("E002", "msg", None, Some(2), Some(1)).to_string();
    assert_eq!(msg, "[E002] msg @line=2,col=1");
}

#[test]
fn error_display_pos_only() {
    let msg = ErrorInfo::new("E003", "msg", Some(7)).to_string();
    assert_eq!(msg, "[E003] msg @pos=7");
}

#[test]
fn error_display_plain() {
    let msg = ErrorInfo::new("E004", "plain", None).to_string();
    assert_eq!(msg, "[E004] plain");
}

#[test]
fn lexer_error_unclosed_block_comment_message() {
    let err = lexer::lex("{- never closed").expect_err("block comment error");
    let s = err.to_string();
    assert!(s.contains("[LEX001]"));
    assert!(s.contains("ブロックコメントが閉じていません"));
}

#[test]
fn lexer_error_unclosed_string_message() {
    let err = lexer::lex("\"abc").expect_err("unclosed string error");
    let s = err.to_string();
    assert!(s.contains("[LEX003]"));
    assert!(s.contains("文字列リテラルが閉じていません"));
}

#[test]
fn parser_error_extra_tokens_message() {
    let err = parser::parse_expr("1; 2").expect_err("extra tokens");
    let s = err.to_string();
    assert!(s.contains("[PAR090]"));
    assert!(s.contains("余分なトークンが残っています"));
}

#[test]
fn parse_error_unclosed_paren() {
    assert!(parser::parse_expr("(1 + 2").is_err());
}

#[test]
fn type_error_if_condition_not_bool() {
    let expr = parser::parse_expr("if 'a' then 2 else 3").unwrap();
    let env = infer::initial_env();
    let ce = infer::initial_class_env();
    let mut st = infer::InferState {
        supply: typesys::TVarSupply::new(),
        subst: Default::default(),
    };
    assert!(infer::infer_expr(&env, &ce, &mut st, &expr).is_err());
}
