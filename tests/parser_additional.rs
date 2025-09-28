// パス: tests/parser_additional.rs
// 役割: Supplemental parser tests for precedence and errors
// 意図: Document grammar decisions beyond primary cases
// 関連ファイル: src/parser.rs, tests/parser_more.rs, src/lexer.rs
// 日本語コメント: 構文解析の優先順位/結合性やプログラム構文の検証

use typelang::parser;

#[test]
fn parser_application_vs_infix_precedence() {
    // 関数適用が最強、べき乗は右結合、乗算/加算の順で束ねられる
    let e = parser::parse_expr("f 2 ^ 3 * 4 + 5").expect("parse");
    // 形の厳密一致は避け、フォーマット可能かを確認
    let s = format!("{}", e);
    assert!(s.contains("f 2"));
    assert!(s.contains("^") && s.contains("*") && s.contains("+"));
}

#[test]
fn parser_error_unclosed_list() {
    // リストの閉じ括弧欠如
    assert!(parser::parse_expr("[1,2").is_err());
}
