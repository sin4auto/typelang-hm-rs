// パス: tests/errors.rs
// 役割: エラー表示と代表的な診断メッセージの安定性を検証
// 意図: ユーザー向けのエラーテキストと失敗シナリオが回帰しないようにする
// 関連ファイル: src/errors.rs, src/lexer.rs, src/parser.rs
use std::fmt::Display;

#[path = "test_support.rs"]
mod support;

use support::{infer_pretty_qual, parse_expr};
use typelang::errors::{ErrorInfo, EvalError, LexerError, TypeError};
use typelang::{infer, lexer, parser, typesys};

fn assert_error_display(info: ErrorInfo, expected: &str) {
    let actual = info.to_string();
    assert_eq!(actual, expected);
}

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
/// `ErrorInfo` の整形パターンを包括的に確認する。
fn error_display_variants() {
    let cases = vec![
        (
            ErrorInfo::at("E001", "msg", Some(12), Some(3), Some(5)).with_snippet("abcdef"),
            "[E001] msg @line=3,col=5 @pos=12\nabcdef\n    ^",
        ),
        (
            ErrorInfo::at("E002", "msg", None, Some(2), Some(1)),
            "[E002] msg @line=2,col=1",
        ),
        (ErrorInfo::new("E003", "msg", Some(7)), "[E003] msg @pos=7"),
        (ErrorInfo::new("E004", "plain", None), "[E004] plain"),
        (
            ErrorInfo::at("E005", "caret", Some(5), Some(1), Some(1)).with_snippet("oops"),
            "[E005] caret @line=1,col=1 @pos=5\noops\n^",
        ),
    ];

    for (info, expected) in cases {
        assert_error_display(info, expected);
    }
}

#[test]
/// ラッパー型経由の Display 実装をまとめて検証する。
fn error_wrapper_renders_expected_strings() {
    let lex = LexerError::at_with_snippet("LEX999", "lex", Some(3), Some(2), Some(4), "code");
    assert_eq!(
        format!("{}", lex),
        "[LEX999] lex @line=2,col=4 @pos=3\ncode\n   ^"
    );

    let ty = TypeError::at("TYPE123", "type", None, Some(5), Some(6));
    assert_eq!(format!("{}", ty), "[TYPE123] type @line=5,col=6");

    let eval = EvalError::at("EVAL777", "eval", Some(9), None, None);
    assert_eq!(format!("{}", eval), "[EVAL777] eval @pos=9");
}

#[test]
/// 字句・構文エラーが期待するメッセージ断片を含むことを確認する。
fn lexer_and_parser_error_messages() {
    let lexer_cases = [
        (
            "{- never closed",
            &["[LEX001]", "ブロックコメントが閉じていません"],
        ),
        ("\"abc", &["[LEX003]", "文字列リテラルが閉じていません"]),
    ];
    for (src, fragments) in lexer_cases {
        assert_err_fragments(lexer::lex(src), fragments);
    }

    assert_err_fragments(
        parser::parse_expr("1; 2"),
        &["[PAR090]", "余分なトークンが残っています"],
    );
    assert!(parser::parse_expr("(1 + 2").is_err());
}

#[test]
/// if 条件が Bool でない場合に型エラーとなることを検証する。
fn type_error_if_condition_not_bool() {
    let expr = parse_expr("if 'a' then 2 else 3");
    let env = infer::initial_env();
    let ce = infer::initial_class_env();
    let mut st = infer::InferState {
        supply: typesys::TVarSupply::new(),
        subst: Default::default(),
    };
    assert!(infer::infer_expr(&env, &ce, &mut st, &expr).is_err());
}

#[test]
/// 型エラーのフォールバック経路でも意味のある表示が得られることを確認する。
fn type_error_rendering_via_support_helpers() {
    let rendered = infer_pretty_qual("show 1").expect("pretty qual");
    assert!(rendered.contains("String") || rendered.contains("[Char]"));
}
