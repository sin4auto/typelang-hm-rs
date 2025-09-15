use typelang::parser;

// 異常系: if の else 省略
#[test]
fn parser_error_if_missing_else() {
    let res = parser::parse_expr("if True then 1");
    assert!(res.is_err());
}

// 正常系: let-in の複数束縛とセミコロン
#[test]
fn parser_let_in_multiple_bindings() {
    let e = parser::parse_expr("let a = 1; b x = x in b a").expect("parse");
    let s = format!("{}", e);
    assert!(s.contains("let") && s.contains("in"));
}

// 正常系: 疑問変数（型穴）
#[test]
fn parser_question_var() {
    let e = parser::parse_expr("?x").expect("parse");
    let s = format!("{}", e);
    assert!(s.contains("?x"));
}
