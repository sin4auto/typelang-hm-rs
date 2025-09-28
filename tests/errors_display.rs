// パス: tests/errors_display.rs
// 役割: Unit tests for ErrorInfo display formatting paths
// 意図: Prevent regressions in diagnostic string rendering
// 関連ファイル: src/errors.rs, tests/error_message_patterns.rs, src/parser.rs
// エラー表示（Display）の分岐網羅テスト
use typelang::errors::ErrorInfo;

#[test]
fn error_display_line_col_pos_and_snippet() {
    let e = ErrorInfo::at("E001", "msg", Some(12), Some(3), Some(5)).with_snippet("abcdef");
    let s = e.to_string();
    assert_eq!(s, "[E001] msg @line=3,col=5 @pos=12\nabcdef\n    ^");
}

#[test]
fn error_display_line_col_only() {
    let e = ErrorInfo::at("E002", "msg", None, Some(2), Some(1));
    assert_eq!(e.to_string(), "[E002] msg @line=2,col=1");
}

#[test]
fn error_display_pos_only() {
    let e = ErrorInfo::new("E003", "msg", Some(7));
    assert_eq!(e.to_string(), "[E003] msg @pos=7");
}

#[test]
fn error_display_plain() {
    let e = ErrorInfo::new("E004", "plain", None);
    assert_eq!(e.to_string(), "[E004] plain");
}
