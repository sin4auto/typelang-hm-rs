// パス: tests/core_ir_tests.rs
// 役割: AST から Core IR への lowering を検証するユニットテスト
// 意図: 最小限のプログラムが期待通りの IR に変換されることを保証する
// 関連ファイル: src/core_ir/lower.rs, src/parser/mod.rs, documents/plan/native-compile-spec.md

use typelang::ast as A;
use typelang::compile_core_ir;
use typelang::core_ir::{Expr, MatchArm, ParameterKind, PrimOp, ValueTy};
use typelang::parser;

#[test]
/// 単純な整数演算プログラムが Core IR に正しく変換される。
fn lower_simple_program_to_core_ir() {
    let src = r#"
main :: Int;
let main = 1 + 2;
"#;
    let prog = parser::parse_program(src).expect("parse program");
    let module = compile_core_ir(&prog).expect("lower to core ir");
    assert_eq!(module.entry(), Some("main"));
    let main_fn = module.functions.get("main").expect("main function lowered");
    assert!(main_fn.params.is_empty());
    assert_eq!(main_fn.result, ValueTy::Int);
    match &main_fn.body {
        Expr::PrimOp {
            op,
            args,
            ty,
            dict_fallback,
        } => {
            assert_eq!(*op, PrimOp::AddInt);
            assert_eq!(*ty, ValueTy::Int);
            assert!(!dict_fallback, "Int 演算は辞書フォールバック不要");
            assert_eq!(args.len(), 2);
            assert!(matches!(
                args[0],
                Expr::Literal {
                    value: _,
                    ty: ValueTy::Int
                }
            ));
            assert!(matches!(
                args[1],
                Expr::Literal {
                    value: _,
                    ty: ValueTy::Int
                }
            ));
        }
        other => panic!("unexpected main body: {:?}", other),
    }
}

#[test]
/// 型クラス制約付き関数が辞書パラメータを持つことを検証する。
fn lower_function_with_dictionary_param() {
    let src = r#"
square :: Num a => a -> a;
let square x = x;
main :: Int;
let main = square 3;
"#;
    let prog = parser::parse_program(src).expect("parse program");
    let module = compile_core_ir(&prog).expect("lower to core ir");
    let square_fn = module
        .functions
        .get("square")
        .expect("square function lowered");
    assert_eq!(square_fn.params.len(), 2);
    match &square_fn.params[0].kind {
        ParameterKind::Dictionary { classname } => {
            assert_eq!(classname, "Num");
        }
        other => panic!("expected dictionary parameter, got {:?}", other),
    }
    assert!(matches!(
        square_fn.params[0].ty,
        ValueTy::Dictionary { ref classname } if classname == "Num"
    ));
    assert!(matches!(square_fn.params[1].kind, ParameterKind::Value));

    let main_fn = module.functions.get("main").expect("main lowered");
    match &main_fn.body {
        Expr::Apply { args, .. } => {
            assert_eq!(args.len(), 2, "辞書 + 値引数の 2 つになる想定");
            match &args[0] {
                Expr::DictionaryPlaceholder {
                    classname,
                    type_repr,
                    ty,
                } => {
                    assert_eq!(classname, "Num");
                    assert_eq!(type_repr, "Int");
                    assert!(matches!(ty, ValueTy::Dictionary { classname } if classname == "Num"));
                }
                other => panic!("expected dictionary placeholder, got {:?}", other),
            }
            assert!(matches!(
                args[1],
                Expr::Literal {
                    value: _,
                    ty: ValueTy::Int
                }
            ));
        }
        other => panic!("expected Apply body, got {:?}", other),
    }
}

#[test]
/// Unknown 型どうしの二項演算が辞書フォールバック指定になることを検証する。
fn lower_polymorphic_binop_marks_dictionary_fallback() {
    let src = r#"
square :: Num a => a -> a;
let square x = x * x;
"#;
    let prog = parser::parse_program(src).expect("parse program");
    let module = compile_core_ir(&prog).expect("lower to core ir");
    let square_fn = module.functions.get("square").expect("square lowered");
    match &square_fn.body {
        Expr::PrimOp {
            dict_fallback, ty, ..
        } => {
            assert!(*dict_fallback, "多相演算は辞書フォールバックが必要なはず");
            assert!(matches!(ty, ValueTy::Unknown));
        }
        other => panic!("expected PrimOp body, got {:?}", other),
    }
}

#[test]
/// case 式が IR 上の Match ノードへ変換されることを検証する。
fn lower_case_expression_to_match() {
    let src = r#"
data Maybe a = Nothing | Just a;
main :: Int;
let main = case Just 3 of
  Just x -> x;
  Nothing -> 0;
"#;
    let prog = parser::parse_program(src).expect("parse program");
    let module = compile_core_ir(&prog).expect("lower to core ir");
    let main_fn = module.functions.get("main").expect("main lowered");
    let maybe_layout = module
        .data_layouts
        .get("Maybe")
        .expect("Maybe layout present");
    assert_eq!(maybe_layout.constructors.len(), 2);
    assert_eq!(maybe_layout.constructors[0].name, "Nothing");
    assert_eq!(maybe_layout.constructors[0].tag, 0);
    assert_eq!(maybe_layout.constructors[1].name, "Just");
    assert_eq!(maybe_layout.constructors[1].tag, 1);
    match &main_fn.body {
        Expr::Match {
            scrutinee,
            arms,
            ty,
        } => {
            assert!(matches!(
                scrutinee.ty(),
                ValueTy::Data { .. } | ValueTy::Unknown
            ));
            assert_eq!(arms.len(), 2);
            assert_eq!(*ty, ValueTy::Int);
            assert_pattern_constructor(&arms[0], "Just");
            assert_eq!(arms[0].constructor.as_deref(), Some("Just"));
            assert_eq!(arms[0].tag, Some(1));
            assert_eq!(arms[0].arity, 1);
            assert!(arms[0]
                .bindings
                .iter()
                .any(|b| b.name == "x" && matches!(b.ty, ValueTy::Int | ValueTy::Unknown)));

            assert_pattern_constructor(&arms[1], "Nothing");
            assert_eq!(arms[1].constructor.as_deref(), Some("Nothing"));
            assert_eq!(arms[1].tag, Some(0));
            assert_eq!(arms[1].arity, 0);
        }
        other => panic!("expected Match expr, got {:?}", other),
    }
}

fn assert_pattern_constructor(arm: &MatchArm, expected_ctor: &str) {
    match &arm.pattern {
        A::Pattern::Constructor { name, .. } => assert_eq!(name, expected_ctor),
        other => panic!("expected constructor pattern, got {:?}", other),
    }
}
