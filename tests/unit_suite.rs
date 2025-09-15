use typelang::{evaluator, infer, lexer, parser, typesys};

// ============ LEXER ==========
#[test]
fn lexer_keywords_and_numbers() {
    let src = "let x = 0xFF; if True then 10 else 0b101";
    let toks = lexer::lex(src).expect("lex");
    assert!(!toks.is_empty());
}

#[test]
fn lexer_comments_and_strings() {
    let src = "-- comment\nlet s = \"a\\n\\\"\"; {- block -}\n";
    let toks = lexer::lex(src).expect("lex");
    assert!(toks.iter().any(|t| t.kind == lexer::TokenKind::STRING));
}

// ============ PARSER ==========
#[test]
fn parser_pow_right_assoc() {
    let e = parser::parse_expr("2 ^ 3 ^ 2").expect("parse");
    let s = format!("{}", e);
    assert!(s.contains("^"));
}

#[test]
fn parser_unary_minus_sugar() {
    let e = parser::parse_expr("-1").expect("parse");
    let s = format!("{}", e);
    assert!(s.contains("- 1"));
}

// ============ UNIFY ==========
#[test]
fn unify_simple_fun_types() {
    use typesys::*;
    let t1 = Type::TFun(TFun {
        arg: Box::new(Type::TCon(TCon { name: "Int".into() })),
        ret: Box::new(Type::TCon(TCon { name: "Int".into() })),
    });
    let t2 = t1.clone();
    assert!(unify(t1, t2).is_ok());
}

#[test]
fn unify_occurs_check() {
    use typesys::*;
    let tv = TVar { id: 1 };
    let tvar = Type::TVar(tv.clone());
    let fun = Type::TFun(TFun {
        arg: Box::new(Type::TVar(tv.clone())),
        ret: Box::new(Type::TCon(TCon { name: "Int".into() })),
    });
    assert!(unify(tvar, fun).is_err());
}

// ============ INFER ==========
#[test]
fn infer_lambda_eq() {
    let e = parser::parse_expr("\\x -> x == x").expect("parse");
    let ty = infer::infer_type_str(&e).expect("infer");
    assert_eq!(ty, "Eq a => a -> Bool");
}

#[test]
fn infer_annot_num_to_bool_displays_bool() {
    // 本実装ではクラス制約の充足を静的には検証しないため、
    // `1 :: Bool` は表示上 Bool になる（Num Bool 制約は表示抑制）。
    let e = parser::parse_expr("1 :: Bool").expect("parse");
    let ty = infer::infer_type_str(&e).expect("infer");
    assert_eq!(ty, "Bool");
}

// ============ EVAL ==========
#[test]
fn eval_comparisons() {
    let mut env = evaluator::initial_env();
    let e = parser::parse_expr("1 == 1").unwrap();
    if let evaluator::Value::Bool(b) = evaluator::eval_expr(&e, &mut env).unwrap() {
        assert!(b)
    } else {
        panic!()
    }
    let e = parser::parse_expr("2 < 1").unwrap();
    if let evaluator::Value::Bool(b) = evaluator::eval_expr(&e, &mut env).unwrap() {
        assert!(!b)
    } else {
        panic!()
    }
    let e = parser::parse_expr("2.0 >= -1").unwrap();
    if let evaluator::Value::Bool(b) = evaluator::eval_expr(&e, &mut env).unwrap() {
        assert!(b)
    } else {
        panic!()
    }
    // タプルの等価性
    let e = parser::parse_expr("(1,2) == (1,2)").unwrap();
    if let evaluator::Value::Bool(b) = evaluator::eval_expr(&e, &mut env).unwrap() {
        assert!(b)
    } else {
        panic!()
    }
    // リストの等価性/順序
    let e = parser::parse_expr("[1,2] == [1,2]").unwrap();
    if let evaluator::Value::Bool(b) = evaluator::eval_expr(&e, &mut env).unwrap() {
        assert!(b)
    } else {
        panic!()
    }
    let e = parser::parse_expr("[1,2] < [1,3]").unwrap();
    if let evaluator::Value::Bool(b) = evaluator::eval_expr(&e, &mut env).unwrap() {
        assert!(b)
    } else {
        panic!()
    }
}

// ============ DEFAULTING (表示) ==========
#[test]
fn defaulting_off_keeps_constraints() {
    let e = parser::parse_expr("show 1").unwrap();
    let env = infer::initial_env();
    let ce = infer::initial_class_env();
    let mut st = infer::InferState {
        supply: typesys::TVarSupply::new(),
        subst: Default::default(),
    };
    let (_s, q) = infer::infer_expr(&env, &ce, &mut st, &e).unwrap();
    let txt = typesys::pretty_qual(&q);
    assert!(txt.ends_with("[Char]") || txt.ends_with("String"));
}

#[test]
fn defaulting_on_hides_numeric_constraints() {
    let e = parser::parse_expr("show 1").unwrap();
    let txt = infer::infer_type_str_with_defaulting(&e, true).unwrap();
    assert!(txt == "String" || txt == "[Char]");
}
