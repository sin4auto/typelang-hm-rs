// パス: tests/native_build.rs
// 役割: ネイティブビルドパイプラインのエンドツーエンド検証
// 意図: TypeLang ソースを実行ファイルへ変換し、期待した出力が得られることを確認する
// 関連ファイル: runtime_native/tests/runtime.rs, documents/native.md, src/codegen/cranelift.rs

use std::{fs, path::PathBuf, process::Command};

use serde_json::Value;
use tempfile::tempdir;
use typelang::{
    codegen::NativeError,
    core_ir::{
        DictionaryInit, DictionaryMethod, Expr, Function, Literal, Module, Parameter,
        ParameterKind, PrimOp, SourceRef, ValueTy, VarKind,
    },
    evaluator, infer, repl,
};

#[cfg_attr(
    miri,
    ignore = "uses native backend and temp directories that Miri isolation forbids"
)]
#[test]
fn build_and_run_simple_program() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
main :: Int;
let main = 1 + 2;
"#;

    let program = typelang::parser::parse_program(src)?;

    let temp = tempdir()?;
    let output_path = temp.path().join("sample");

    let _artifacts = typelang::emit_native(&program, &output_path)?;

    assert!(
        output_path.exists(),
        "ネイティブバイナリが生成されていません"
    );

    let result = Command::new(&output_path).output()?;
    assert!(result.status.success(), "生成バイナリの実行に失敗しました");
    let stdout = String::from_utf8_lossy(&result.stdout);
    assert_eq!(stdout.trim(), "3");

    Ok(())
}

#[cfg_attr(
    miri,
    ignore = "uses native backend and temp directories that Miri isolation forbids"
)]
#[test]
fn emit_module_with_dictionary_param() -> Result<(), Box<dyn std::error::Error>> {
    let mut module = Module::new();

    let helper = Function {
        name: "helper".into(),
        params: vec![
            Parameter::with_kind(
                "$dict0_Num",
                ValueTy::Dictionary {
                    classname: "Num".into(),
                },
                ParameterKind::Dictionary {
                    classname: "Num".into(),
                },
                Some("Int".into()),
            ),
            Parameter::with_kind("x", ValueTy::Int, ParameterKind::Value, None),
        ],
        result: ValueTy::Int,
        body: Expr::Var {
            name: "x".into(),
            ty: ValueTy::Int,
            kind: VarKind::Param,
        },
        location: SourceRef::default(),
    };

    let main_fn = Function {
        name: "main".into(),
        params: Vec::new(),
        result: ValueTy::Int,
        body: Expr::Apply {
            func: Box::new(Expr::Var {
                name: "helper".into(),
                ty: ValueTy::Function {
                    params: vec![
                        ValueTy::Dictionary {
                            classname: "Num".into(),
                        },
                        ValueTy::Int,
                    ],
                    result: Box::new(ValueTy::Int),
                },
                kind: VarKind::Function,
            }),
            args: vec![
                Expr::DictionaryPlaceholder {
                    classname: "Num".into(),
                    type_repr: "Int".into(),
                    ty: ValueTy::Dictionary {
                        classname: "Num".into(),
                    },
                },
                Expr::Literal {
                    value: Literal::Int(42),
                    ty: ValueTy::Int,
                },
            ],
            ty: ValueTy::Int,
        },
        location: SourceRef::default(),
    };

    module.insert_function(helper);
    module.insert_function(main_fn);
    module.set_entry("main");

    let temp = tempdir()?;
    let output = temp.path().join("dict_param_module");

    let err = typelang::codegen::cranelift::emit_native(&module, &output)
        .expect_err("dictionary arguments should be unsupported for now");

    match err {
        NativeError::Unsupported { code, message } => {
            assert_eq!(code, "CODEGEN301");
            assert!(message.contains("Num"), "unexpected message: {message}");
        }
        other => panic!("expected Unsupported error, got {other:?}"),
    }

    Ok(())
}

#[cfg_attr(
    miri,
    ignore = "uses native backend and temp directories that Miri isolation forbids"
)]
#[test]
fn emit_module_with_eq_bool_dictionary_runs() -> Result<(), Box<dyn std::error::Error>> {
    let mut module = Module::new();

    let sameness = Function {
        name: "sameness".into(),
        params: vec![
            Parameter::with_kind(
                "$dict0_Eq",
                ValueTy::Dictionary {
                    classname: "Eq".into(),
                },
                ParameterKind::Dictionary {
                    classname: "Eq".into(),
                },
                Some("Bool".into()),
            ),
            Parameter::with_kind("x", ValueTy::Bool, ParameterKind::Value, None),
            Parameter::with_kind("y", ValueTy::Bool, ParameterKind::Value, None),
        ],
        result: ValueTy::Bool,
        body: Expr::PrimOp {
            op: PrimOp::EqInt,
            args: vec![
                Expr::Var {
                    name: "x".into(),
                    ty: ValueTy::Bool,
                    kind: VarKind::Param,
                },
                Expr::Var {
                    name: "y".into(),
                    ty: ValueTy::Bool,
                    kind: VarKind::Param,
                },
            ],
            ty: ValueTy::Bool,
            dict_fallback: true,
        },
        location: SourceRef::default(),
    };

    let main_fn = Function {
        name: "main".into(),
        params: Vec::new(),
        result: ValueTy::Bool,
        body: Expr::Apply {
            func: Box::new(Expr::Var {
                name: "sameness".into(),
                ty: ValueTy::Function {
                    params: vec![
                        ValueTy::Dictionary {
                            classname: "Eq".into(),
                        },
                        ValueTy::Bool,
                        ValueTy::Bool,
                    ],
                    result: Box::new(ValueTy::Bool),
                },
                kind: VarKind::Function,
            }),
            args: vec![
                Expr::DictionaryPlaceholder {
                    classname: "Eq".into(),
                    type_repr: "Bool".into(),
                    ty: ValueTy::Dictionary {
                        classname: "Eq".into(),
                    },
                },
                Expr::Literal {
                    value: Literal::Bool(true),
                    ty: ValueTy::Bool,
                },
                Expr::Literal {
                    value: Literal::Bool(false),
                    ty: ValueTy::Bool,
                },
            ],
            ty: ValueTy::Bool,
        },
        location: SourceRef::default(),
    };

    module.insert_function(sameness);
    module.insert_function(main_fn);
    module.set_entry("main");
    module.dictionaries.push(DictionaryInit {
        classname: "Eq".into(),
        type_repr: "Bool".into(),
        methods: vec![
            DictionaryMethod {
                name: "eq".into(),
                signature: Some("Bool -> Bool -> Bool".into()),
                symbol: Some("tl_eq_bool".into()),
                method_id: Some(0),
            },
            DictionaryMethod {
                name: "neq".into(),
                signature: Some("Bool -> Bool -> Bool".into()),
                symbol: Some("tl_neq_bool".into()),
                method_id: Some(1),
            },
        ],
        scheme_repr: "Eq Bool => Bool -> Bool -> Bool".into(),
        builder_symbol: Some("tl_dict_build_Eq_Bool".into()),
        origin: "sameness".into(),
        source_span: SourceRef::default(),
    });

    let temp = tempdir()?;
    let output = temp.path().join("eq_bool_dict");
    typelang::codegen::cranelift::emit_native(&module, &output)?;

    let result = Command::new(&output).output()?;
    assert!(result.status.success(), "生成バイナリの実行に失敗しました");
    let stdout = String::from_utf8_lossy(&result.stdout);
    assert_eq!(stdout.trim(), "False");

    Ok(())
}

#[cfg_attr(
    miri,
    ignore = "uses native backend and temp directories that Miri isolation forbids"
)]
#[test]
fn emit_native_reports_lambda_unsupported() -> Result<(), Box<dyn std::error::Error>> {
    let mut module = Module::new();

    let with_lambda = Function {
        name: "with_lambda".into(),
        params: vec![Parameter::with_kind(
            "x",
            ValueTy::Int,
            ParameterKind::Value,
            None,
        )],
        result: ValueTy::Int,
        body: Expr::Lambda {
            params: vec![Parameter::with_kind(
                "y",
                ValueTy::Int,
                ParameterKind::Value,
                None,
            )],
            body: Box::new(Expr::Var {
                name: "x".into(),
                ty: ValueTy::Int,
                kind: VarKind::Param,
            }),
            ty: ValueTy::Function {
                params: vec![ValueTy::Int],
                result: Box::new(ValueTy::Int),
            },
        },
        location: SourceRef::default(),
    };

    let main_fn = Function {
        name: "main".into(),
        params: Vec::new(),
        result: ValueTy::Int,
        body: Expr::Literal {
            value: Literal::Int(0),
            ty: ValueTy::Int,
        },
        location: SourceRef::default(),
    };

    module.insert_function(with_lambda);
    module.insert_function(main_fn);
    module.set_entry("main");

    let temp = tempdir()?;
    let output = temp.path().join("lambda_unsupported");

    let err = typelang::codegen::cranelift::emit_native(&module, &output)
        .expect_err("lambda expressions should be unsupported");

    match err {
        NativeError::Unsupported { code, message } => {
            assert_eq!(code, "CODEGEN030");
            assert!(
                message.contains("タプル・ラムダ式"),
                "unexpected message: {message}"
            );
        }
        other => panic!("expected Unsupported error, got {other:?}"),
    }

    Ok(())
}

#[cfg_attr(
    miri,
    ignore = "uses native backend and temp directories that Miri isolation forbids"
)]
#[test]
fn build_program_with_dictionary_metadata() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
square :: Num a => a -> a;
let square x = x;

main :: Int;
let main = square 0;
"#;

    let program = typelang::parser::parse_program(src)?;
    let temp = tempdir()?;
    let output_path = temp.path().join("square_dict");

    let artifacts = typelang::emit_native(&program, &output_path)?;
    dbg!(&artifacts.dictionaries);
    assert!(
        output_path.exists(),
        "ネイティブバイナリが生成されていません"
    );
    let dict = artifacts
        .dictionaries
        .iter()
        .find(|d| d.classname == "Num")
        .expect("Num dictionary not generated");
    assert_eq!(
        dict.builder_symbol.as_deref(),
        Some("tl_dict_build_Num_Int")
    );
    assert!(
        dict.scheme_repr.contains("Num"),
        "scheme should describe Num constraint: {}",
        dict.scheme_repr
    );
    let method_names: Vec<_> = dict.methods.iter().map(|m| m.name.as_str()).collect();
    assert!(method_names.contains(&"add"));
    assert!(method_names.contains(&"sub"));
    assert!(method_names.contains(&"mul"));
    assert!(method_names.contains(&"fromInt"));
    for method in &dict.methods {
        assert!(method
            .symbol
            .as_deref()
            .is_some_and(|s| s.starts_with("tl_num_int")));
        assert!(method.signature.as_deref().is_some());
    }

    Ok(())
}

#[cfg_attr(
    miri,
    ignore = "uses native backend and temp directories that Miri isolation forbids"
)]
#[test]
fn build_basics_example_matches_interpreter() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
square :: Num a => a -> a;
let square x = x;

main :: Int;
let main = square 3;
"#;
    let program = typelang::parser::parse_program(src)?;

    let expected_stdout = {
        let mut type_env = infer::initial_env();
        let mut class_env = infer::initial_class_env();
        let mut value_env = evaluator::initial_env();
        repl::load_program_into_env(&program, &mut type_env, &mut class_env, &mut value_env)?;
        let expr = typelang::parser::parse_expr("main")?;
        match evaluator::eval_expr(&expr, &value_env)? {
            evaluator::Value::Int(i) => i.to_string(),
            other => panic!("unexpected interpreter result: {:?}", other),
        }
    };

    let temp = tempdir()?;
    let output_path = temp.path().join("basics_native");
    let artifacts = typelang::emit_native(&program, &output_path)?;
    assert!(
        artifacts
            .dictionaries
            .iter()
            .any(|d| d.classname == "Num" && d.builder_symbol.is_some()),
        "dictionary metadata should include Num builder"
    );

    let result = Command::new(&output_path).output()?;
    assert!(
        result.status.success(),
        "生成バイナリの実行に失敗しました: {:?}",
        result.status
    );
    let stdout = String::from_utf8_lossy(&result.stdout);
    assert_eq!(stdout.trim(), expected_stdout);

    Ok(())
}

#[cfg_attr(
    miri,
    ignore = "uses native backend and temp directories that Miri isolation forbids"
)]
#[test]
fn build_fractional_double_program_matches_interpreter() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
ratio :: Fractional a => a -> a -> a;
let ratio x y = x / y;

main :: Double;
let main = ratio 7.5 2.5;
"#;

    let program = typelang::parser::parse_program(src)?;

    let expected_stdout = {
        let mut type_env = infer::initial_env();
        let mut class_env = infer::initial_class_env();
        let mut value_env = evaluator::initial_env();
        repl::load_program_into_env(&program, &mut type_env, &mut class_env, &mut value_env)?;
        let expr = typelang::parser::parse_expr("main")?;
        match evaluator::eval_expr(&expr, &value_env)? {
            evaluator::Value::Double(d) => d.to_string(),
            other => panic!("unexpected interpreter result: {:?}", other),
        }
    };

    let temp = tempdir()?;
    let output_path = temp.path().join("fractional_double");
    let artifacts = typelang::emit_native(&program, &output_path)?;
    assert!(
        artifacts
            .dictionaries
            .iter()
            .any(|dict| dict.classname == "Fractional"
                && dict.type_repr == "Double"
                && dict.builder_symbol.as_deref() == Some("tl_dict_build_Fractional_Double")),
        "Fractional<Double> dictionary metadata missing"
    );

    let result = Command::new(&output_path).output()?;
    assert!(result.status.success(), "生成バイナリの実行に失敗しました");
    let stdout = String::from_utf8_lossy(&result.stdout);
    assert_eq!(stdout.trim(), expected_stdout);

    Ok(())
}

#[cfg_attr(
    miri,
    ignore = "uses native backend and temp directories that Miri isolation forbids"
)]
#[test]
fn cli_prints_dictionary_json() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
square :: Num a => a -> a;
let square x = x;

main :: Int;
let main = square 4;
"#;

    let temp = tempdir()?;
    let input_path = temp.path().join("basics_cli.tl");
    let output_path = temp.path().join("basics_cli");
    fs::write(&input_path, src)?;

    let cli = typelang_cli_path();
    let output = Command::new(cli)
        .arg("build")
        .arg(&input_path)
        .arg("--emit")
        .arg("native")
        .arg("--output")
        .arg(&output_path)
        .arg("--print-dictionaries")
        .arg("--json")
        .output()?;

    assert!(
        output.status.success(),
        "CLI build failed: status={:?}, stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(json["status"], "ok");
    assert_eq!(json["backend"], "cranelift");
    assert_eq!(json["optim"], "debug");
    assert_eq!(json["input"], input_path.display().to_string());
    assert_eq!(json["output"], output_path.display().to_string());

    let dictionaries = json["dictionaries"]
        .as_array()
        .expect("dictionaries must be array");
    assert!(!dictionaries.is_empty());
    let num_dict = dictionaries
        .iter()
        .find(|entry| entry["class"] == "Num")
        .expect("Num dictionary missing in CLI JSON");
    assert_eq!(
        num_dict["builder"],
        Value::String("tl_dict_build_Num_Int".to_string())
    );
    let methods = num_dict["methods"].as_array().expect("methods array");
    let mut has_mul = false;
    for method in methods {
        let name = method["name"].as_str().unwrap_or("");
        let symbol = method["symbol"].as_str().unwrap_or("");
        if name == "mul" {
            has_mul = true;
            assert_eq!(symbol, "tl_num_int_mul");
        }
    }
    assert!(has_mul, "mul method not present in CLI output");

    assert!(
        output_path.exists(),
        "CLI should emit native binary to {}",
        output_path.display()
    );

    Ok(())
}

#[cfg_attr(
    miri,
    ignore = "uses native backend and temp directories that Miri isolation forbids"
)]
#[test]
fn emit_native_supports_builtin_ord_dictionary() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
foo :: Ord a => a -> a;
let foo x = x;

main :: Int;
let main = foo 0;
"#;

    let program = typelang::parser::parse_program(src)?;
    let temp = tempdir()?;
    let output_path = temp.path().join("missing_dict");

    let artifacts = typelang::emit_native(&program, &output_path)?;
    assert!(
        artifacts
            .dictionaries
            .iter()
            .any(|dict| dict.classname == "Ord"
                && dict.builder_symbol.as_deref() == Some("tl_dict_build_Ord_Int")),
        "Ord<Int> dictionary metadata missing"
    );
    assert!(output_path.exists(), "native binary should be emitted");

    Ok(())
}

fn typelang_cli_path() -> PathBuf {
    const CANDIDATES: [&str; 3] = [
        "CARGO_BIN_EXE_typelang",
        "CARGO_BIN_EXE_typelang_repl",
        "CARGO_BIN_EXE_typelang-repl",
    ];
    for key in CANDIDATES {
        if let Ok(path) = std::env::var(key) {
            return PathBuf::from(path);
        }
    }
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fallback_debug = manifest_dir.join("target/debug/typelang-repl");
    if fallback_debug.exists() {
        return fallback_debug;
    }
    let fallback_rel = manifest_dir.join("target/release/typelang-repl");
    if fallback_rel.exists() {
        return fallback_rel;
    }
    panic!("typelang CLI バイナリへのパスが取得できませんでした");
}
