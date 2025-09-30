// パス: tests/errors.rs
// 役割: エラー表示と代表的な診断メッセージの安定性を検証
// 意図: ユーザー向けのエラーテキストと失敗シナリオが回帰しないようにする
// 関連ファイル: src/errors.rs, src/lexer.rs, src/parser.rs
use std::fmt::Display;

use typelang::errors::ErrorInfo;
use typelang::{infer, lexer, parser, typesys};

/// `ErrorInfo` から生成された文字列表現を検証するヘルパ。
fn assert_error_display(info: ErrorInfo, expected: &str) {
    let actual = info.to_string();
    assert_eq!(actual, expected);
}

/// エラーメッセージに特定の断片が含まれることを検証するヘルパ。
fn assert_err_fragments<T, E>(res: Result<T, E>, fragments: &[&str])
where
    E: Display,
{
    let rendered = match res {
        Ok(_) => panic!("エラーが返ること"),
        Err(err) => format!("{}", err),
    };
    for fragment in fragments {
        assert!(
            rendered.contains(fragment),
            "期待する断片 `{fragment}` が含まれていません: {rendered}"
        );
    }
}

#[test]
/// エラー表示に行・列・位置・スニペットが含まれることを確認する。
fn error_display_line_col_pos_and_snippet() {
    assert_error_display(
        ErrorInfo::at("E001", "msg", Some(12), Some(3), Some(5)).with_snippet("abcdef"),
        "[E001] msg @line=3,col=5 @pos=12\nabcdef\n    ^",
    );
}

#[test]
/// 行と列のみが表示されるケースを検証する。
fn error_display_line_col_only() {
    assert_error_display(
        ErrorInfo::at("E002", "msg", None, Some(2), Some(1)),
        "[E002] msg @line=2,col=1",
    );
}

#[test]
/// 位置情報のみ表示されるケースを検証する。
fn error_display_pos_only() {
    assert_error_display(ErrorInfo::new("E003", "msg", Some(7)), "[E003] msg @pos=7");
}

#[test]
/// 追加情報なしの表示を検証する。
fn error_display_plain() {
    assert_error_display(ErrorInfo::new("E004", "plain", None), "[E004] plain");
}

#[test]
/// カラム位置が 1 のスニペットでもキャレットが正しく描画されることを検証する。
fn error_display_snippet_column_one() {
    assert_error_display(
        ErrorInfo::at("E005", "caret", Some(5), Some(1), Some(1)).with_snippet("oops"),
        "[E005] caret @line=1,col=1 @pos=5\noops\n^",
    );
}

#[test]
/// ブロックコメント未閉鎖のエラーメッセージを検証する。
fn lexer_error_unclosed_block_comment_message() {
    assert_err_fragments(
        lexer::lex("{- never closed"),
        &["[LEX001]", "ブロックコメントが閉じていません"],
    );
}

#[test]
/// 文字列リテラル未閉鎖のエラーメッセージを検証する。
fn lexer_error_unclosed_string_message() {
    assert_err_fragments(
        lexer::lex("\"abc"),
        &["[LEX003]", "文字列リテラルが閉じていません"],
    );
}

#[test]
/// 余分なトークンが指摘されることを検証する。
fn parser_error_extra_tokens_message() {
    assert_err_fragments(
        parser::parse_expr("1; 2"),
        &["[PAR090]", "余分なトークンが残っています"],
    );
}

#[test]
/// 括弧閉じ忘れがエラーになることを確認する。
fn parse_error_unclosed_paren() {
    assert!(parser::parse_expr("(1 + 2").is_err());
}

#[test]
/// if 条件が Bool でない場合に型エラーとなることを検証する。
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
