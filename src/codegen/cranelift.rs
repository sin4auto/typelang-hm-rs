// パス: src/codegen/cranelift.rs
// 役割: Core IR から Cranelift を用いてネイティブオブジェクトと実行ファイルを生成する
// 意図: 既存の AST/IR から AOT バイナリを生成する最小のバックエンド実装を提供する
// 関連ファイル: src/codegen/dictionary_codegen.rs, runtime_native/src/lib.rs, documents/native.md
#![allow(clippy::result_large_err)]

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::{
    types, AbiParam, Function as ClifFunction, InstBuilder, Signature, StackSlotData,
    StackSlotKind, TrapCode, Type, UserFuncName, Value,
};
use cranelift_codegen::isa::{self, CallConv};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_module::{FuncId, Linkage, Module};
use cranelift_native;
use cranelift_object::{ObjectBuilder, ObjectModule};
use tempfile::tempdir;

use crate::codegen::{dictionary_codegen, NativeError, NativeResult};
use crate::core_ir::{
    self, Binding, ConstructorLayout, Expr, Function, Literal, MatchArm, MatchBinding, PrimOp,
    ValueTy, VarKind,
};

const SYMBOL_PREFIX: &str = "tl_";

/// Core IR モジュールをネイティブ実行可能ファイルとして出力する。
pub fn emit_native(ir: &core_ir::Module, output: &Path) -> NativeResult<()> {
    let entry_name = ir.entry().ok_or_else(|| {
        NativeError::unsupported("CODEGEN001", "エントリポイント関数 (main) が見つかりません")
    })?;
    let entry_fn = ir
        .functions
        .get(entry_name)
        .ok_or_else(|| NativeError::unsupported("CODEGEN002", "エントリポイントが不正です"))?;
    if !matches!(
        entry_fn.result,
        ValueTy::Int | ValueTy::Double | ValueTy::Bool | ValueTy::Unit
    ) {
        return Err(NativeError::unsupported(
            "CODEGEN003",
            format!(
                "main の戻り値型 {:?} は現在サポートされていません",
                entry_fn.result
            ),
        ));
    }

    let isa = build_isa()?;
    let obj_builder = ObjectBuilder::new(
        isa.clone(),
        "typelang_module",
        cranelift_module::default_libcall_names(),
    )?;
    let mut module = ObjectModule::new(obj_builder);
    let call_conv = module.isa().default_call_conv();

    let dict_source = dictionary_codegen::generate(&ir.dictionaries)?;
    let runtime = declare_runtime_symbols(&mut module, call_conv)?;
    let func_ids = declare_functions(ir, &mut module, call_conv)?;
    let dict_symbols = declare_dictionary_symbols(ir, &mut module, call_conv)?;
    define_functions(
        ir,
        &func_ids,
        &dict_symbols,
        &runtime,
        &mut module,
        call_conv,
    )?;
    define_entrypoint(
        entry_name,
        entry_fn,
        &func_ids,
        &runtime,
        &mut module,
        call_conv,
    )?;
    let product = module.finish();
    let obj_bytes = product.emit().map_err(|e| {
        NativeError::unsupported("CODEGEN105", format!("オブジェクト生成に失敗しました: {e}"))
    })?;

    let tmp_dir = tempdir()?;
    let obj_path = tmp_dir.path().join("program.o");
    fs::write(&obj_path, obj_bytes)?;

    if let Some(parent) = output.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }

    build_runtime_library(dict_source.as_ref().map(|tmp| tmp.path()))?;
    let runtime_lib_path = locate_runtime_library(&isa)?;

    let mut cmd = Command::new("cc");
    cmd.arg(&obj_path)
        .arg(&runtime_lib_path)
        .arg("-O0")
        .arg("-o")
        .arg(output);
    let output_status = cmd.output();
    match output_status {
        Ok(out) if out.status.success() => Ok(()),
        Ok(out) => Err(NativeError::command_failure(
            format!("cc {} {}", obj_path.display(), output.display()),
            Some(out.status),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )),
        Err(err) => Err(NativeError::command_failure(
            "cc",
            None,
            format!("failed to invoke cc: {err}"),
        )),
    }
}

fn build_isa() -> NativeResult<Arc<dyn isa::TargetIsa>> {
    let isa_builder = cranelift_native::builder().map_err(|e| {
        NativeError::unsupported(
            "CODEGEN004",
            format!("ホスト ISA サポートの初期化に失敗しました: {e}"),
        )
    })?;
    let mut flag_builder = settings::builder();
    flag_builder
        .set("opt_level", "none")
        .map_err(|e| NativeError::unsupported("CODEGEN005", format!("設定エラー: {e}")))?;
    let isa = isa_builder
        .finish(settings::Flags::new(flag_builder))
        .map_err(|e| {
            NativeError::unsupported("CODEGEN006", format!("ISA 構築に失敗しました: {e}"))
        })?;
    Ok(isa)
}

#[allow(dead_code)]
struct RuntimeSymbols {
    print_int: FuncId,
    print_double: FuncId,
    print_bool: FuncId,
    list_empty: FuncId,
    list_cons: FuncId,
    list_is_empty: FuncId,
    list_head: FuncId,
    list_tail: FuncId,
    list_free: FuncId,
    data_pack: FuncId,
    data_tag: FuncId,
    data_arity: FuncId,
    data_field: FuncId,
    data_free: FuncId,
    value_from_int: FuncId,
    value_from_double: FuncId,
    value_from_bool: FuncId,
    value_to_int: FuncId,
    value_to_double: FuncId,
    value_to_bool: FuncId,
    dict_lookup: FuncId,
    value_to_ptr: FuncId,
    abort: FuncId,
}

fn declare_runtime_symbols(
    module: &mut ObjectModule,
    call_conv: CallConv,
) -> NativeResult<RuntimeSymbols> {
    let ptr_ty = module.isa().pointer_type();

    let mut sig_int = Signature::new(call_conv);
    sig_int.params.push(AbiParam::new(types::I64));
    let print_int = module.declare_function("tl_print_int", Linkage::Import, &sig_int)?;

    let mut sig_double = Signature::new(call_conv);
    sig_double.params.push(AbiParam::new(types::F64));
    let print_double = module.declare_function("tl_print_double", Linkage::Import, &sig_double)?;

    let mut sig_bool = Signature::new(call_conv);
    sig_bool.params.push(AbiParam::new(types::I8));
    let print_bool = module.declare_function("tl_print_bool", Linkage::Import, &sig_bool)?;

    let mut sig_list_empty = Signature::new(call_conv);
    sig_list_empty.returns.push(AbiParam::new(ptr_ty));
    let list_empty = module.declare_function("tl_list_empty", Linkage::Import, &sig_list_empty)?;

    let mut sig_list_cons = Signature::new(call_conv);
    sig_list_cons.params.push(AbiParam::new(ptr_ty)); // head
    sig_list_cons.params.push(AbiParam::new(ptr_ty)); // tail
    sig_list_cons.returns.push(AbiParam::new(ptr_ty));
    let list_cons = module.declare_function("tl_list_cons", Linkage::Import, &sig_list_cons)?;

    let mut sig_list_is_empty = Signature::new(call_conv);
    sig_list_is_empty.params.push(AbiParam::new(ptr_ty));
    sig_list_is_empty.returns.push(AbiParam::new(types::I8));
    let list_is_empty =
        module.declare_function("tl_list_is_empty", Linkage::Import, &sig_list_is_empty)?;

    let mut sig_list_head = Signature::new(call_conv);
    sig_list_head.params.push(AbiParam::new(ptr_ty));
    sig_list_head.returns.push(AbiParam::new(ptr_ty));
    let list_head = module.declare_function("tl_list_head", Linkage::Import, &sig_list_head)?;

    let mut sig_list_tail = Signature::new(call_conv);
    sig_list_tail.params.push(AbiParam::new(ptr_ty));
    sig_list_tail.returns.push(AbiParam::new(ptr_ty));
    let list_tail = module.declare_function("tl_list_tail", Linkage::Import, &sig_list_tail)?;

    let mut sig_list_free = Signature::new(call_conv);
    sig_list_free.params.push(AbiParam::new(ptr_ty));
    let list_free = module.declare_function("tl_list_free", Linkage::Import, &sig_list_free)?;

    let mut sig_data_pack = Signature::new(call_conv);
    sig_data_pack.params.push(AbiParam::new(types::I32)); // tag
    sig_data_pack.params.push(AbiParam::new(ptr_ty)); // fields
    sig_data_pack.params.push(AbiParam::new(ptr_ty)); // len (usize)
    sig_data_pack.returns.push(AbiParam::new(ptr_ty));
    let data_pack = module.declare_function("tl_data_pack", Linkage::Import, &sig_data_pack)?;

    let mut sig_data_tag = Signature::new(call_conv);
    sig_data_tag.params.push(AbiParam::new(ptr_ty));
    sig_data_tag.returns.push(AbiParam::new(types::I32));
    let data_tag = module.declare_function("tl_data_tag", Linkage::Import, &sig_data_tag)?;

    let mut sig_data_arity = Signature::new(call_conv);
    sig_data_arity.params.push(AbiParam::new(ptr_ty));
    sig_data_arity.returns.push(AbiParam::new(ptr_ty));
    let data_arity = module.declare_function("tl_data_arity", Linkage::Import, &sig_data_arity)?;

    let mut sig_data_field = Signature::new(call_conv);
    sig_data_field.params.push(AbiParam::new(ptr_ty));
    sig_data_field.params.push(AbiParam::new(ptr_ty));
    sig_data_field.returns.push(AbiParam::new(ptr_ty));
    let data_field = module.declare_function("tl_data_field", Linkage::Import, &sig_data_field)?;

    let mut sig_data_free = Signature::new(call_conv);
    sig_data_free.params.push(AbiParam::new(ptr_ty));
    let data_free = module.declare_function("tl_data_free", Linkage::Import, &sig_data_free)?;

    let mut sig_value_from_int = Signature::new(call_conv);
    sig_value_from_int.params.push(AbiParam::new(types::I64));
    sig_value_from_int.returns.push(AbiParam::new(ptr_ty));
    let value_from_int =
        module.declare_function("tl_value_from_int", Linkage::Import, &sig_value_from_int)?;

    let mut sig_value_from_double = Signature::new(call_conv);
    sig_value_from_double.params.push(AbiParam::new(types::F64));
    sig_value_from_double.returns.push(AbiParam::new(ptr_ty));
    let value_from_double = module.declare_function(
        "tl_value_from_double",
        Linkage::Import,
        &sig_value_from_double,
    )?;

    let mut sig_value_from_bool = Signature::new(call_conv);
    sig_value_from_bool.params.push(AbiParam::new(types::I8));
    sig_value_from_bool.returns.push(AbiParam::new(ptr_ty));
    let value_from_bool =
        module.declare_function("tl_value_from_bool", Linkage::Import, &sig_value_from_bool)?;

    let mut sig_value_to_int = Signature::new(call_conv);
    sig_value_to_int.params.push(AbiParam::new(ptr_ty));
    sig_value_to_int.returns.push(AbiParam::new(types::I64));
    let value_to_int =
        module.declare_function("tl_value_to_int", Linkage::Import, &sig_value_to_int)?;

    let mut sig_value_to_double = Signature::new(call_conv);
    sig_value_to_double.params.push(AbiParam::new(ptr_ty));
    sig_value_to_double.returns.push(AbiParam::new(types::F64));
    let value_to_double =
        module.declare_function("tl_value_to_double", Linkage::Import, &sig_value_to_double)?;

    let mut sig_value_to_bool = Signature::new(call_conv);
    sig_value_to_bool.params.push(AbiParam::new(ptr_ty));
    sig_value_to_bool.returns.push(AbiParam::new(types::I8));
    let value_to_bool =
        module.declare_function("tl_value_to_bool", Linkage::Import, &sig_value_to_bool)?;

    let mut sig_dict_lookup = Signature::new(call_conv);
    sig_dict_lookup.params.push(AbiParam::new(ptr_ty));
    sig_dict_lookup.params.push(AbiParam::new(types::I64));
    sig_dict_lookup.returns.push(AbiParam::new(ptr_ty));
    let dict_lookup =
        module.declare_function("tl_dict_lookup", Linkage::Import, &sig_dict_lookup)?;

    let mut sig_value_to_ptr = Signature::new(call_conv);
    sig_value_to_ptr.params.push(AbiParam::new(ptr_ty));
    sig_value_to_ptr.returns.push(AbiParam::new(ptr_ty));
    let value_to_ptr =
        module.declare_function("tl_value_to_ptr", Linkage::Import, &sig_value_to_ptr)?;

    let mut sig_abort = Signature::new(call_conv);
    sig_abort.params.push(AbiParam::new(types::I32));
    let abort = module.declare_function("tl_abort_with_message", Linkage::Import, &sig_abort)?;

    Ok(RuntimeSymbols {
        print_int,
        print_double,
        print_bool,
        list_empty,
        list_cons,
        list_is_empty,
        list_head,
        list_tail,
        list_free,
        data_pack,
        data_tag,
        data_arity,
        data_field,
        data_free,
        value_from_int,
        value_from_double,
        value_from_bool,
        value_to_int,
        value_to_double,
        value_to_bool,
        dict_lookup,
        value_to_ptr,
        abort,
    })
}

fn declare_functions(
    ir: &core_ir::Module,
    module: &mut ObjectModule,
    call_conv: CallConv,
) -> NativeResult<HashMap<String, FuncId>> {
    let mut ids = HashMap::new();
    let ptr_ty = module.isa().pointer_type();
    for (name, func) in &ir.functions {
        ensure_supported_function(func)?;
        let signature = make_signature(func, call_conv, ptr_ty)?;
        let func_id = module.declare_function(&symbol_name(name), Linkage::Export, &signature)?;
        ids.insert(name.clone(), func_id);
    }
    Ok(ids)
}

type DictionarySymbols = HashMap<(String, String), FuncId>;

fn declare_dictionary_symbols(
    ir: &core_ir::Module,
    module: &mut ObjectModule,
    call_conv: CallConv,
) -> NativeResult<DictionarySymbols> {
    let mut symbols = DictionarySymbols::new();
    for dict in &ir.dictionaries {
        if let Some(symbol) = &dict.builder_symbol {
            let mut sig = Signature::new(call_conv);
            sig.returns.push(AbiParam::new(module.isa().pointer_type()));
            let func_id = module
                .declare_function(symbol, Linkage::Import, &sig)
                .map_err(|err| {
                    NativeError::unsupported(
                        "CODEGEN300",
                        format!("辞書ビルダー {symbol} の宣言に失敗しました: {err}"),
                    )
                })?;
            symbols.insert((dict.classname.clone(), dict.type_repr.clone()), func_id);
        }
    }
    Ok(symbols)
}

fn define_functions(
    ir: &core_ir::Module,
    func_ids: &HashMap<String, FuncId>,
    dict_symbols: &DictionarySymbols,
    runtime: &RuntimeSymbols,
    module: &mut ObjectModule,
    call_conv: CallConv,
) -> NativeResult<()> {
    let mut builder_ctx = FunctionBuilderContext::new();
    let mut ctx = module.make_context();
    let ptr_ty = module.isa().pointer_type();

    for (name, func) in &ir.functions {
        let func_id = *func_ids.get(name).ok_or_else(|| {
            NativeError::unsupported(
                "CODEGEN007",
                format!("関数 {name} の識別子が見つかりません"),
            )
        })?;
        ctx.func = ClifFunction::with_name_signature(
            UserFuncName::testcase(symbol_name(name)),
            make_signature(func, call_conv, ptr_ty)?,
        );

        {
            let mut builder = FunctionBuilder::new(&mut ctx.func, &mut builder_ctx);
            let entry_block = builder.create_block();
            builder.append_block_params_for_function_params(entry_block);
            builder.switch_to_block(entry_block);
            builder.seal_block(entry_block);

            let mut env = CodegenEnv::new(ptr_ty, dict_symbols.clone());
            env.next_index = func.params.len() as u32;
            for (idx, param) in func.params.iter().enumerate() {
                let var = Variable::from_u32(idx as u32);
                let cl_ty = clif_type(ptr_ty, &param.ty)?;
                builder.declare_var(var, cl_ty);
                let value = builder.block_params(entry_block)[idx];
                builder.def_var(var, value);
                env.insert_existing(
                    param.name.clone(),
                    var,
                    param.ty.clone(),
                    param.dict_type_repr.clone(),
                );
            }

            let lowered = lower_expr(
                module,
                ir,
                runtime,
                func_ids,
                &mut builder,
                &mut env,
                &func.body,
            )?;

            match func.result {
                ValueTy::Unit => {
                    builder.ins().return_(&[]);
                }
                _ => {
                    let lowered =
                        coerce_value(module, &mut builder, runtime, lowered, &func.result)?;
                    builder.ins().return_(&[lowered.value]);
                }
            }
            builder.finalize();
        }

        module.define_function(func_id, &mut ctx)?;
        module.clear_context(&mut ctx);
    }

    Ok(())
}

fn define_entrypoint(
    entry_name: &str,
    entry_func: &Function,
    func_ids: &HashMap<String, FuncId>,
    runtime: &RuntimeSymbols,
    module: &mut ObjectModule,
    call_conv: CallConv,
) -> NativeResult<()> {
    let mut sig = Signature::new(call_conv);
    sig.returns.push(AbiParam::new(types::I32));
    let main_id = module.declare_function("main", Linkage::Export, &sig)?;

    let mut ctx = module.make_context();
    ctx.func = ClifFunction::with_name_signature(UserFuncName::testcase("main"), sig);
    let mut builder_ctx = FunctionBuilderContext::new();

    {
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut builder_ctx);
        let block = builder.create_block();
        builder.switch_to_block(block);
        builder.seal_block(block);

        let entry_id = *func_ids.get(entry_name).ok_or_else(|| {
            NativeError::unsupported(
                "CODEGEN116",
                format!("エントリ関数 {} が宣言されていません", entry_name),
            )
        })?;
        let func_ref = module.declare_func_in_func(entry_id, builder.func);
        let call = builder.ins().call(func_ref, &[]);
        let results = builder.inst_results(call);

        match &entry_func.result {
            ValueTy::Unit => {}
            ValueTy::Int => {
                let value = *results.first().ok_or_else(|| {
                    NativeError::unsupported(
                        "CODEGEN117",
                        "Int 戻り値を期待しましたが値が存在しません",
                    )
                })?;
                let print_ref = module.declare_func_in_func(runtime.print_int, builder.func);
                builder.ins().call(print_ref, &[value]);
            }
            ValueTy::Double => {
                let value = *results.first().ok_or_else(|| {
                    NativeError::unsupported(
                        "CODEGEN118",
                        "Double 戻り値を期待しましたが値が存在しません",
                    )
                })?;
                let print_ref = module.declare_func_in_func(runtime.print_double, builder.func);
                builder.ins().call(print_ref, &[value]);
            }
            ValueTy::Bool => {
                let value = *results.first().ok_or_else(|| {
                    NativeError::unsupported(
                        "CODEGEN119",
                        "Bool 戻り値を期待しましたが値が存在しません",
                    )
                })?;
                let print_ref = module.declare_func_in_func(runtime.print_bool, builder.func);
                builder.ins().call(print_ref, &[value]);
            }
            other => {
                return Err(NativeError::unsupported(
                    "CODEGEN120",
                    format!("エントリ関数の戻り値型 {:?} は処理されていません", other),
                ));
            }
        }

        let exit_code = builder.ins().iconst(types::I32, 0);
        builder.ins().return_(&[exit_code]);
        builder.finalize();
    }

    module.define_function(main_id, &mut ctx)?;
    module.clear_context(&mut ctx);
    Ok(())
}

fn build_runtime_library(dict_source: Option<&Path>) -> NativeResult<()> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut cmd = Command::new("cargo");
    cmd.arg("build")
        .arg("-p")
        .arg("runtime_native")
        .arg("--release")
        .current_dir(manifest_dir);
    if let Some(path) = dict_source {
        cmd.env("TYPELANG_DICT_AUTOGEN", path);
    }
    let output = cmd.output().map_err(|err| {
        NativeError::command_failure(
            "cargo build -p runtime_native --release",
            None,
            err.to_string(),
        )
    })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(NativeError::command_failure(
            "cargo build -p runtime_native --release",
            Some(output.status),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ))
    }
}

fn locate_runtime_library(isa: &Arc<dyn isa::TargetIsa>) -> NativeResult<PathBuf> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut candidates = Vec::new();

    if let Ok(custom_target) = std::env::var("CARGO_TARGET_DIR") {
        let custom = PathBuf::from(custom_target);
        candidates.push(custom.join("release/libruntime_native.a"));
        candidates.push(custom.join(format!("{}/release/libruntime_native.a", isa.triple())));
    }

    candidates.push(manifest_dir.join("target/release/libruntime_native.a"));
    candidates.push(manifest_dir.join(format!(
        "target/{}/release/libruntime_native.a",
        isa.triple()
    )));

    if let Some(found) = candidates.into_iter().find(|path| path.exists()) {
        Ok(found)
    } else {
        Err(NativeError::unsupported(
            "CODEGEN115",
            "runtime_native の静的ライブラリが見つかりません",
        ))
    }
}

fn lower_expr(
    module: &mut ObjectModule,
    ir: &core_ir::Module,
    runtime: &RuntimeSymbols,
    func_ids: &HashMap<String, FuncId>,
    builder: &mut FunctionBuilder,
    env: &mut CodegenEnv,
    expr: &Expr,
) -> NativeResult<LoweredValue> {
    match expr {
        Expr::Literal { value, ty } => lower_literal(builder, value, ty),
        Expr::Var { name, kind, ty } => lower_var(builder, env, name, kind, ty),
        Expr::Let { bindings, body, .. } => {
            lower_let(module, ir, runtime, func_ids, builder, env, bindings, body)
        }
        Expr::PrimOp {
            op,
            args,
            ty,
            dict_fallback,
        } => lower_primop(
            module,
            ir,
            runtime,
            func_ids,
            builder,
            env,
            *op,
            args,
            ty,
            *dict_fallback,
        ),
        Expr::Apply { func, args, .. } => {
            lower_apply(module, ir, runtime, func_ids, builder, env, func, args)
        }
        Expr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => lower_if(
            module,
            ir,
            runtime,
            func_ids,
            builder,
            env,
            cond,
            then_branch,
            else_branch,
        ),
        Expr::DictionaryPlaceholder {
            classname,
            type_repr,
            ty,
        } => lower_dictionary_placeholder(module, builder, env, classname, type_repr, ty),
        Expr::Tuple { .. } | Expr::Lambda { .. } => Err(NativeError::unsupported(
            "CODEGEN030",
            "タプル・ラムダ式はまだサポートされていません",
        )),
        Expr::List { items, ty } => {
            lower_list_literal(module, ir, runtime, func_ids, builder, env, items, ty)
        }
        Expr::Match {
            scrutinee,
            arms,
            ty,
        } => lower_match(
            module, ir, runtime, func_ids, builder, env, scrutinee, arms, ty,
        ),
    }
}

fn lower_literal(
    builder: &mut FunctionBuilder,
    lit: &Literal,
    ty: &ValueTy,
) -> NativeResult<LoweredValue> {
    match (lit, ty) {
        (Literal::Int(v), ValueTy::Int) => Ok(LoweredValue::new(
            builder.ins().iconst(types::I64, *v),
            ValueTy::Int,
        )),
        (Literal::Bool(v), ValueTy::Bool) => Ok(LoweredValue::new(
            builder.ins().iconst(types::I8, if *v { 1 } else { 0 }),
            ValueTy::Bool,
        )),
        (Literal::Double(v), ValueTy::Double) => Ok(LoweredValue::new(
            builder.ins().f64const(*v),
            ValueTy::Double,
        )),
        (Literal::Char(_), _) | (Literal::String(_), _) => Err(NativeError::unsupported(
            "CODEGEN032",
            "Char/String リテラルは現在未対応です",
        )),
        (Literal::Unit, ValueTy::Unit) => Ok(LoweredValue::new(
            builder.ins().iconst(types::I8, 0),
            ValueTy::Unit,
        )),
        (Literal::EmptyList, _) => Err(NativeError::unsupported(
            "CODEGEN033",
            "空リストリテラルは現在未対応です",
        )),
        _ => Err(NativeError::unsupported(
            "CODEGEN034",
            "未知のリテラル型組み合わせです",
        )),
    }
}

fn lower_dictionary_placeholder(
    module: &mut ObjectModule,
    builder: &mut FunctionBuilder,
    env: &mut CodegenEnv,
    classname: &str,
    type_repr: &str,
    ty: &ValueTy,
) -> NativeResult<LoweredValue> {
    let func_id = env.lookup_dictionary(classname, type_repr).ok_or_else(|| {
        NativeError::unsupported(
            "CODEGEN301",
            format!("辞書 {classname}<{type_repr}> のビルダーが見つかりません"),
        )
    })?;
    let value = env.ensure_dictionary(module, builder, classname, type_repr, func_id)?;
    Ok(LoweredValue::new(value, ty.clone()))
}

fn lower_var(
    builder: &mut FunctionBuilder,
    env: &CodegenEnv,
    name: &str,
    kind: &VarKind,
    _ty: &ValueTy,
) -> NativeResult<LoweredValue> {
    match kind {
        VarKind::Local | VarKind::Param => {
            let info = env.get(name).ok_or_else(|| {
                NativeError::unsupported(
                    "CODEGEN040",
                    format!("変数 {name} がスコープ内に存在しません"),
                )
            })?;
            Ok(LoweredValue::new(
                builder.use_var(info.var),
                info.ty.clone(),
            ))
        }
        VarKind::Function | VarKind::Primitive => Err(NativeError::unsupported(
            "CODEGEN041",
            format!("関数 {name} を値として扱うことは現在サポートされていません"),
        )),
    }
}

#[allow(clippy::too_many_arguments)]
fn lower_let(
    module: &mut ObjectModule,
    ir: &core_ir::Module,
    runtime: &RuntimeSymbols,
    func_ids: &HashMap<String, FuncId>,
    builder: &mut FunctionBuilder,
    env: &mut CodegenEnv,
    bindings: &[Binding],
    body: &Expr,
) -> NativeResult<LoweredValue> {
    let mut scope = env.clone();
    for binding in bindings {
        if matches!(binding.ty, ValueTy::Function { .. }) {
            return Err(NativeError::unsupported(
                "CODEGEN050",
                "ローカル関数束縛は現在サポートされていません",
            ));
        }
        let lowered = lower_expr(
            module,
            ir,
            runtime,
            func_ids,
            builder,
            &mut scope,
            &binding.value,
        )?;
        let var = scope.insert(binding.name.clone(), binding.ty.clone());
        let cl_ty = clif_type(env.ptr_ty(), &binding.ty)?;
        builder.declare_var(var, cl_ty);
        builder.def_var(var, lowered.value);
    }
    lower_expr(module, ir, runtime, func_ids, builder, &mut scope, body)
}

#[allow(clippy::too_many_arguments)]
fn lower_primop(
    module: &mut ObjectModule,
    ir: &core_ir::Module,
    runtime: &RuntimeSymbols,
    func_ids: &HashMap<String, FuncId>,
    builder: &mut FunctionBuilder,
    env: &mut CodegenEnv,
    op: PrimOp,
    args: &[Expr],
    result_ty: &ValueTy,
    dict_fallback: bool,
) -> NativeResult<LoweredValue> {
    let expected_args = match op {
        PrimOp::NotBool => 1,
        _ => 2,
    };
    if args.len() != expected_args {
        return Err(NativeError::unsupported(
            "CODEGEN060",
            format!(
                "プリミティブ演算子 {} の引数数が不正です ({} 個)",
                expected_args,
                args.len()
            ),
        ));
    }
    let lhs = lower_expr(module, ir, runtime, func_ids, builder, env, &args[0])?;
    let rhs = if expected_args == 2 {
        Some(lower_expr(
            module, ir, runtime, func_ids, builder, env, &args[1],
        )?)
    } else {
        None
    };

    if dict_fallback {
        return lower_dictionary_primop(module, ir, runtime, builder, env, op, lhs, rhs, result_ty);
    }
    match op {
        PrimOp::AddInt => binary_int_op(builder, lhs, rhs.unwrap(), |b, l, r| b.ins().iadd(l, r)),
        PrimOp::SubInt => binary_int_op(builder, lhs, rhs.unwrap(), |b, l, r| b.ins().isub(l, r)),
        PrimOp::MulInt => binary_int_op(builder, lhs, rhs.unwrap(), |b, l, r| b.ins().imul(l, r)),
        PrimOp::DivInt => binary_int_op(builder, lhs, rhs.unwrap(), |b, l, r| b.ins().sdiv(l, r)),
        PrimOp::ModInt => binary_int_op(builder, lhs, rhs.unwrap(), |b, l, r| b.ins().srem(l, r)),
        PrimOp::AddDouble => {
            binary_double_op(builder, lhs, rhs.unwrap(), |b, l, r| b.ins().fadd(l, r))
        }
        PrimOp::SubDouble => {
            binary_double_op(builder, lhs, rhs.unwrap(), |b, l, r| b.ins().fsub(l, r))
        }
        PrimOp::MulDouble => {
            binary_double_op(builder, lhs, rhs.unwrap(), |b, l, r| b.ins().fmul(l, r))
        }
        PrimOp::DivDouble => {
            binary_double_op(builder, lhs, rhs.unwrap(), |b, l, r| b.ins().fdiv(l, r))
        }
        PrimOp::EqInt => compare_int(builder, lhs, rhs.unwrap(), IntCC::Equal),
        PrimOp::NeqInt => compare_int(builder, lhs, rhs.unwrap(), IntCC::NotEqual),
        PrimOp::LtInt => compare_int(builder, lhs, rhs.unwrap(), IntCC::SignedLessThan),
        PrimOp::LeInt => compare_int(builder, lhs, rhs.unwrap(), IntCC::SignedLessThanOrEqual),
        PrimOp::GtInt => compare_int(builder, lhs, rhs.unwrap(), IntCC::SignedGreaterThan),
        PrimOp::GeInt => compare_int(builder, lhs, rhs.unwrap(), IntCC::SignedGreaterThanOrEqual),
        PrimOp::EqDouble => compare_double(builder, lhs, rhs.unwrap(), FloatCC::Equal),
        PrimOp::NeqDouble => compare_double(builder, lhs, rhs.unwrap(), FloatCC::NotEqual),
        PrimOp::LtDouble => compare_double(builder, lhs, rhs.unwrap(), FloatCC::LessThan),
        PrimOp::LeDouble => compare_double(builder, lhs, rhs.unwrap(), FloatCC::LessThanOrEqual),
        PrimOp::GtDouble => compare_double(builder, lhs, rhs.unwrap(), FloatCC::GreaterThan),
        PrimOp::GeDouble => compare_double(builder, lhs, rhs.unwrap(), FloatCC::GreaterThanOrEqual),
        PrimOp::AndBool => binary_bool_op(builder, lhs, rhs.unwrap(), |b, l, r| b.ins().band(l, r)),
        PrimOp::OrBool => binary_bool_op(builder, lhs, rhs.unwrap(), |b, l, r| b.ins().bor(l, r)),
        PrimOp::NotBool => unary_bool_op(builder, lhs, |b, v| b.ins().bnot(v)),
    }
}

#[allow(clippy::too_many_arguments)]
fn lower_dictionary_primop(
    module: &mut ObjectModule,
    ir: &core_ir::Module,
    runtime: &RuntimeSymbols,
    builder: &mut FunctionBuilder,
    env: &mut CodegenEnv,
    op: PrimOp,
    lhs: LoweredValue,
    rhs: Option<LoweredValue>,
    target_ty: &ValueTy,
) -> NativeResult<LoweredValue> {
    let info = op.dictionary_method().ok_or_else(|| {
        NativeError::unsupported(
            "CODEGEN210",
            format!("演算子 {:?} は辞書情報を提供していません", op),
        )
    })?;
    let binding = env
        .dictionary_param(info.classname)
        .cloned()
        .ok_or_else(|| {
            NativeError::unsupported(
                "CODEGEN211",
                format!(
                    "辞書 {} のパラメータがスコープ内に存在しません",
                    info.classname
                ),
            )
        })?;
    ensure_dictionary_method_available(ir, &binding.classname, &binding.type_repr, info.method_id)?;
    let operand_ty = value_ty_from_repr(&binding.type_repr).ok_or_else(|| {
        NativeError::unsupported(
            "CODEGEN213",
            format!(
                "辞書 {}<{}> の型表現をサポートしていません",
                binding.classname, binding.type_repr
            ),
        )
    })?;

    let lhs = coerce_value(module, builder, runtime, lhs, &operand_ty)?;
    let rhs = match rhs {
        Some(value) => Some(coerce_value(module, builder, runtime, value, &operand_ty)?),
        None => None,
    };

    let dict_value = builder.use_var(binding.var);
    let method_id_value = builder.ins().iconst(types::I64, info.method_id as i64);
    let lookup_ref = module.declare_func_in_func(runtime.dict_lookup, builder.func);
    let lookup_call = builder
        .ins()
        .call(lookup_ref, &[dict_value, method_id_value]);
    let lookup_result = builder.inst_results(lookup_call)[0];
    let fn_ptr = call_runtime(builder, module, runtime.value_to_ptr, &[lookup_result]);

    let mut call_sig = Signature::new(builder.func.signature.call_conv);
    let operand_clif_ty = clif_type(env.ptr_ty(), &operand_ty)?;
    call_sig.params.push(AbiParam::new(operand_clif_ty));
    if rhs.is_some() {
        call_sig.params.push(AbiParam::new(operand_clif_ty));
    }
    let method_result_ty = if matches!(target_ty, ValueTy::Bool) {
        ValueTy::Bool
    } else {
        operand_ty.clone()
    };
    let result_clif_ty = clif_type(env.ptr_ty(), &method_result_ty)?;
    call_sig.returns.push(AbiParam::new(result_clif_ty));
    let sig_ref = builder.import_signature(call_sig);

    let mut arg_values = Vec::with_capacity(if rhs.is_some() { 2 } else { 1 });
    arg_values.push(lhs.value);
    if let Some(rhs) = rhs {
        arg_values.push(rhs.value);
    }
    let call_inst = builder.ins().call_indirect(sig_ref, fn_ptr, &arg_values);
    let call_results = builder.inst_results(call_inst);
    let result_value = *call_results.first().ok_or_else(|| {
        NativeError::unsupported(
            "CODEGEN214",
            format!("辞書メソッド {:?} が戻り値を返しません", op),
        )
    })?;
    let lowered = LoweredValue::new(result_value, method_result_ty);
    coerce_value(module, builder, runtime, lowered, target_ty)
}

fn ensure_dictionary_method_available(
    ir: &core_ir::Module,
    classname: &str,
    type_repr: &str,
    method_id: u64,
) -> NativeResult<()> {
    if let Some(dict) = ir
        .dictionaries
        .iter()
        .find(|d| d.classname == classname && d.type_repr == type_repr)
    {
        let has_method = dict.methods.iter().any(|m| m.method_id == Some(method_id));
        if !has_method {
            return Err(NativeError::unsupported(
                "CODEGEN212",
                format!(
                    "辞書 {}<{}> にメソッド ID {} が存在しません",
                    classname, type_repr, method_id
                ),
            ));
        }
    }
    Ok(())
}

fn value_ty_from_repr(type_repr: &str) -> Option<ValueTy> {
    match type_repr {
        repr if repr.eq_ignore_ascii_case("int") || repr.eq_ignore_ascii_case("integer") => {
            Some(ValueTy::Int)
        }
        repr if repr.eq_ignore_ascii_case("double") => Some(ValueTy::Double),
        repr if repr.eq_ignore_ascii_case("bool") => Some(ValueTy::Bool),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn lower_apply(
    module: &mut ObjectModule,
    ir: &core_ir::Module,
    runtime: &RuntimeSymbols,
    func_ids: &HashMap<String, FuncId>,
    builder: &mut FunctionBuilder,
    env: &mut CodegenEnv,
    func: &Expr,
    args: &[Expr],
) -> NativeResult<LoweredValue> {
    match func {
        Expr::Var {
            name,
            kind: VarKind::Function,
            ..
        } => lower_function_call(module, ir, runtime, func_ids, builder, env, name, args),
        Expr::Var {
            name,
            kind: VarKind::Primitive,
            ..
        } => lower_constructor_call(module, ir, runtime, func_ids, builder, env, name, args),
        _ => Err(NativeError::unsupported(
            "CODEGEN070",
            "高階関数や部分適用はまだサポートされていません",
        )),
    }
}

#[allow(clippy::too_many_arguments)]
fn lower_function_call(
    module: &mut ObjectModule,
    ir: &core_ir::Module,
    runtime: &RuntimeSymbols,
    func_ids: &HashMap<String, FuncId>,
    builder: &mut FunctionBuilder,
    env: &mut CodegenEnv,
    name: &str,
    args: &[Expr],
) -> NativeResult<LoweredValue> {
    let callee_ir = ir.functions.get(name).ok_or_else(|| {
        NativeError::unsupported("CODEGEN071", format!("関数 {name} が存在しません"))
    })?;
    if callee_ir.params.len() != args.len() {
        return Err(NativeError::unsupported(
            "CODEGEN072",
            format!(
                "関数 {} の引数数が一致しません: 期待 {}, 実際 {}",
                name,
                callee_ir.params.len(),
                args.len()
            ),
        ));
    }
    let func_id = *func_ids.get(name).ok_or_else(|| {
        NativeError::unsupported("CODEGEN073", format!("関数 {name} の ID が見つかりません"))
    })?;
    let callee_ref = module.declare_func_in_func(func_id, builder.func);

    let mut lowered_args = Vec::with_capacity(args.len());
    for (idx, arg_expr) in args.iter().enumerate() {
        let lowered = lower_expr(module, ir, runtime, func_ids, builder, env, arg_expr)?;
        let expected = &callee_ir.params[idx].ty;
        let coerced = coerce_value(module, builder, runtime, lowered, expected)?;
        lowered_args.push(coerced.value);
    }
    let call = builder.ins().call(callee_ref, &lowered_args);
    let results = builder.inst_results(call);
    let ret_val = *results
        .first()
        .ok_or_else(|| NativeError::unsupported("CODEGEN075", "関数の戻り値が存在しません"))?;
    Ok(LoweredValue::new(ret_val, callee_ir.result.clone()))
}

#[allow(clippy::too_many_arguments)]
fn lower_constructor_call(
    module: &mut ObjectModule,
    ir: &core_ir::Module,
    runtime: &RuntimeSymbols,
    _func_ids: &HashMap<String, FuncId>,
    builder: &mut FunctionBuilder,
    env: &mut CodegenEnv,
    name: &str,
    args: &[Expr],
) -> NativeResult<LoweredValue> {
    let layout = find_constructor_layout(ir, name).ok_or_else(|| {
        NativeError::unsupported(
            "CODEGEN130",
            format!("コンストラクタ {} が Core IR 上に見つかりません", name),
        )
    })?;
    if layout.arity != args.len() {
        return Err(NativeError::unsupported(
            "CODEGEN131",
            format!(
                "コンストラクタ {} の引数数が一致しません: 期待 {}, 実際 {}",
                name,
                layout.arity,
                args.len()
            ),
        ));
    }

    let ptr_ty = env.ptr_ty();

    let mut lowered_args = Vec::with_capacity(args.len());
    let mut stored_fields = Vec::with_capacity(args.len());
    for (idx, arg_expr) in args.iter().enumerate() {
        let lowered = lower_expr(module, ir, runtime, _func_ids, builder, env, arg_expr)?;
        let field_value =
            prepare_constructor_field(module, runtime, builder, ptr_ty, name, idx, &lowered)?;
        stored_fields.push(field_value);
        lowered_args.push(lowered);
    }

    let tag_value = builder.ins().iconst(types::I32, layout.tag as i64);

    let (fields_ptr, len_value) = if stored_fields.is_empty() {
        let null_ptr = builder.ins().iconst(ptr_ty, 0);
        let zero_len = builder.ins().iconst(ptr_ty, 0);
        (null_ptr, zero_len)
    } else {
        let elem_size = usize::try_from(ptr_ty.bytes()).map_err(|_| {
            NativeError::unsupported(
                "CODEGEN134",
                format!(
                    "ポインタサイズ {} がスタック配置に対応していません",
                    ptr_ty.bytes()
                ),
            )
        })?;
        let total_size = elem_size.checked_mul(lowered_args.len()).ok_or_else(|| {
            NativeError::unsupported(
                "CODEGEN135",
                format!(
                    "コンストラクタ {} のフィールドバッファ確保に失敗しました (size overflow)",
                    name
                ),
            )
        })?;
        let total_size_u32 = u32::try_from(total_size).map_err(|_| {
            NativeError::unsupported(
                "CODEGEN136",
                format!(
                    "コンストラクタ {} のフィールドバッファサイズ {} が不正です",
                    name, total_size
                ),
            )
        })?;
        let align_shift = ptr_ty.bytes().trailing_zeros() as u8;
        let slot = builder.func.create_sized_stack_slot(StackSlotData::new(
            StackSlotKind::ExplicitSlot,
            total_size_u32,
            align_shift,
        ));
        for (idx, value) in stored_fields.iter().enumerate() {
            let offset = i32::try_from(elem_size * idx).map_err(|_| {
                NativeError::unsupported(
                    "CODEGEN137",
                    format!(
                        "コンストラクタ {} のフィールド {} のオフセット計算に失敗しました",
                        name,
                        idx + 1
                    ),
                )
            })?;
            builder.ins().stack_store(*value, slot, offset);
        }
        let addr = builder.ins().stack_addr(ptr_ty, slot, 0);
        let len_value = builder.ins().iconst(ptr_ty, stored_fields.len() as i64);
        (addr, len_value)
    };

    let data_pack_ref = module.declare_func_in_func(runtime.data_pack, builder.func);
    let call = builder
        .ins()
        .call(data_pack_ref, &[tag_value, fields_ptr, len_value]);
    let results = builder.inst_results(call);
    let data_ptr = *results.first().ok_or_else(|| {
        NativeError::unsupported("CODEGEN132", "tl_data_pack の戻り値が取得できませんでした")
    })?;

    Ok(LoweredValue::new(
        data_ptr,
        ValueTy::Data {
            constructor: layout.name.clone(),
            args: lowered_args.into_iter().map(|lv| lv.ty).collect(),
        },
    ))
}

fn prepare_constructor_field(
    module: &mut ObjectModule,
    runtime: &RuntimeSymbols,
    builder: &mut FunctionBuilder,
    ptr_ty: Type,
    ctor_name: &str,
    index: usize,
    lowered: &LoweredValue,
) -> NativeResult<Value> {
    let context = format!("コンストラクタ {} のフィールド {}", ctor_name, index + 1);
    lower_value_to_tl_value(module, runtime, builder, ptr_ty, &context, lowered)
}

fn lower_value_to_tl_value(
    module: &mut ObjectModule,
    runtime: &RuntimeSymbols,
    builder: &mut FunctionBuilder,
    ptr_ty: Type,
    context: &str,
    lowered: &LoweredValue,
) -> NativeResult<Value> {
    match &lowered.ty {
        ValueTy::Int => {
            let func_ref = module.declare_func_in_func(runtime.value_from_int, builder.func);
            let call = builder.ins().call(func_ref, &[lowered.value]);
            let results = builder.inst_results(call);
            results.first().copied().ok_or_else(|| {
                NativeError::unsupported(
                    "CODEGEN138",
                    format!("{} の boxing に失敗しました", context),
                )
            })
        }
        ValueTy::Double => {
            let func_ref = module.declare_func_in_func(runtime.value_from_double, builder.func);
            let call = builder.ins().call(func_ref, &[lowered.value]);
            let results = builder.inst_results(call);
            results.first().copied().ok_or_else(|| {
                NativeError::unsupported(
                    "CODEGEN139",
                    format!("{} の boxing に失敗しました", context),
                )
            })
        }
        ValueTy::Bool => {
            let func_ref = module.declare_func_in_func(runtime.value_from_bool, builder.func);
            let call = builder.ins().call(func_ref, &[lowered.value]);
            let results = builder.inst_results(call);
            results.first().copied().ok_or_else(|| {
                NativeError::unsupported(
                    "CODEGEN140",
                    format!("{} の boxing に失敗しました", context),
                )
            })
        }
        ValueTy::Unit => Ok(builder.ins().iconst(ptr_ty, 0)),
        ValueTy::Data { .. } | ValueTy::Dictionary { .. } | ValueTy::List(_) => {
            let actual_ty = builder.func.dfg.value_type(lowered.value);
            if actual_ty == ptr_ty {
                Ok(lowered.value)
            } else {
                Err(NativeError::unsupported(
                    "CODEGEN141",
                    format!(
                        "{} はポインタ型が期待されますが {:?} でした",
                        context, lowered.ty
                    ),
                ))
            }
        }
        ValueTy::Unknown => {
            let actual_ty = builder.func.dfg.value_type(lowered.value);
            if actual_ty == ptr_ty {
                Ok(lowered.value)
            } else {
                Err(NativeError::unsupported(
                    "CODEGEN142",
                    format!("{} (型 {:?}) は現在未対応です", context, lowered.ty),
                ))
            }
        }
        ValueTy::Tuple(_) | ValueTy::Char | ValueTy::String | ValueTy::Function { .. } => {
            Err(NativeError::unsupported(
                "CODEGEN143",
                format!("{} (型 {:?}) は現在未対応です", context, lowered.ty),
            ))
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn lower_match(
    module: &mut ObjectModule,
    ir: &core_ir::Module,
    runtime: &RuntimeSymbols,
    func_ids: &HashMap<String, FuncId>,
    builder: &mut FunctionBuilder,
    env: &mut CodegenEnv,
    scrutinee_expr: &Expr,
    arms: &[MatchArm],
    result_ty: &ValueTy,
) -> NativeResult<LoweredValue> {
    if arms.is_empty() {
        return Err(NativeError::unsupported(
            "CODEGEN160",
            "Match 式に分岐がありません",
        ));
    }

    let scrutinee = lower_expr(module, ir, runtime, func_ids, builder, env, scrutinee_expr)?;

    if !matches!(scrutinee.ty, ValueTy::Data { .. } | ValueTy::Unknown) {
        return Err(NativeError::unsupported(
            "CODEGEN161",
            format!(
                "Match scrutinee の型 {:?} はネイティブローワリング未対応です",
                scrutinee.ty
            ),
        ));
    }

    let ptr_ty = env.ptr_ty();

    let tag = {
        let func_ref = module.declare_func_in_func(runtime.data_tag, builder.func);
        let call = builder.ins().call(func_ref, &[scrutinee.value]);
        *builder.inst_results(call).first().ok_or_else(|| {
            NativeError::unsupported("CODEGEN162", "tl_data_tag の戻り値が取得できませんでした")
        })?
    };

    let merge_block = builder.create_block();
    if !matches!(result_ty, ValueTy::Unit) {
        builder.append_block_param(merge_block, clif_type(ptr_ty, result_ty)?);
    }

    let unmatched_block = builder.create_block();
    let mut current_block = builder
        .current_block()
        .ok_or_else(|| NativeError::unsupported("CODEGEN163", "現在のブロックを特定できません"))?;

    for (index, arm) in arms.iter().enumerate() {
        if arm.tag.is_none() && arm.constructor.is_some() {
            return Err(NativeError::unsupported(
                "CODEGEN164",
                "コンストラクタタグが解決できませんでした",
            ));
        }

        if arm.tag.is_some() && arm.bindings.iter().any(|b| b.path.len() > 1) {
            return Err(NativeError::unsupported(
                "CODEGEN165",
                "ネストしたパターン束縛はまだサポートされていません",
            ));
        }

        let success_block = builder.create_block();
        let next_block = if index + 1 < arms.len() {
            Some(builder.create_block())
        } else {
            None
        };

        builder.switch_to_block(current_block);

        if let Some(tag_value) = arm.tag {
            let cmp = builder.ins().icmp_imm(IntCC::Equal, tag, tag_value as i64);
            let fail_block = next_block.unwrap_or(unmatched_block);
            builder.ins().brif(cmp, success_block, &[], fail_block, &[]);
        } else {
            builder.ins().jump(success_block, &[]);
        }

        builder.seal_block(success_block);
        builder.switch_to_block(success_block);

        let mut arm_env = env.clone();

        for binding in &arm.bindings {
            if binding.path.len() > 1 {
                return Err(NativeError::unsupported(
                    "CODEGEN166",
                    "ネストしたパターン束縛はまだサポートされていません",
                ));
            }

            let value = extract_match_binding_value(
                module,
                runtime,
                builder,
                ptr_ty,
                scrutinee.value,
                binding,
            )?;
            let var = arm_env.insert(binding.name.clone(), binding.ty.clone());
            let cl_ty = clif_type(ptr_ty, &binding.ty)?;
            builder.declare_var(var, cl_ty);
            builder.def_var(var, value);
        }

        let mut guard_pass_block = success_block;
        if let Some(guard_expr) = &arm.guard {
            let guard_true = builder.create_block();
            let guard_val = lower_expr(
                module,
                ir,
                runtime,
                func_ids,
                builder,
                &mut arm_env,
                guard_expr,
            )?;
            if guard_val.ty != ValueTy::Bool {
                return Err(NativeError::unsupported(
                    "CODEGEN167",
                    "Match ガードの型は Bool である必要があります",
                ));
            }
            let guard_cond = bool_to_b1(builder, guard_val.value);
            let fail_block = next_block.unwrap_or(unmatched_block);
            builder
                .ins()
                .brif(guard_cond, guard_true, &[], fail_block, &[]);
            builder.seal_block(guard_true);
            builder.switch_to_block(guard_true);
            guard_pass_block = guard_true;
        }

        builder.switch_to_block(guard_pass_block);

        let body_value = lower_expr(
            module,
            ir,
            runtime,
            func_ids,
            builder,
            &mut arm_env,
            &arm.body,
        )?;
        let body_value = coerce_value(module, builder, runtime, body_value, result_ty)?;

        if matches!(scrutinee.ty, ValueTy::Data { .. }) {
            let free_ref = module.declare_func_in_func(runtime.data_free, builder.func);
            builder.ins().call(free_ref, &[scrutinee.value]);
        }

        if matches!(result_ty, ValueTy::Unit) {
            builder.ins().jump(merge_block, &[]);
        } else {
            builder.ins().jump(merge_block, &[body_value.value]);
        }

        if let Some(next) = next_block {
            builder.switch_to_block(next);
            builder.seal_block(next);
            current_block = next;
        } else {
            current_block = unmatched_block;
        }
    }

    builder.switch_to_block(unmatched_block);
    builder.seal_block(unmatched_block);
    let abort_ref = module.declare_func_in_func(runtime.abort, builder.func);
    let code = builder.ins().iconst(types::I32, 2001);
    builder.ins().call(abort_ref, &[code]);
    builder.ins().trap(TrapCode::UnreachableCodeReached);

    builder.switch_to_block(merge_block);
    builder.seal_block(merge_block);

    let result_value = if matches!(result_ty, ValueTy::Unit) {
        builder.ins().iconst(types::I8, 0)
    } else {
        builder.block_params(merge_block)[0]
    };

    Ok(LoweredValue::new(result_value, result_ty.clone()))
}

#[allow(clippy::too_many_arguments)]
fn lower_list_literal(
    module: &mut ObjectModule,
    ir: &core_ir::Module,
    runtime: &RuntimeSymbols,
    func_ids: &HashMap<String, FuncId>,
    builder: &mut FunctionBuilder,
    env: &mut CodegenEnv,
    items: &[Expr],
    ty: &ValueTy,
) -> NativeResult<LoweredValue> {
    let ptr_ty = env.ptr_ty();
    let empty_ref = module.declare_func_in_func(runtime.list_empty, builder.func);
    let call = builder.ins().call(empty_ref, &[]);
    let mut acc = *builder.inst_results(call).first().ok_or_else(|| {
        NativeError::unsupported("CODEGEN174", "tl_list_empty の戻り値が取得できませんでした")
    })?;

    for (index, item_expr) in items.iter().enumerate().rev() {
        let lowered = lower_expr(module, ir, runtime, func_ids, builder, env, item_expr)?;
        let context = format!("リスト要素 {}", index + 1);
        let head = lower_value_to_tl_value(module, runtime, builder, ptr_ty, &context, &lowered)?;
        let cons_ref = module.declare_func_in_func(runtime.list_cons, builder.func);
        let call = builder.ins().call(cons_ref, &[head, acc]);
        acc = *builder.inst_results(call).first().ok_or_else(|| {
            NativeError::unsupported("CODEGEN175", "tl_list_cons の戻り値が取得できませんでした")
        })?;
    }

    Ok(LoweredValue::new(acc, ty.clone()))
}

fn find_constructor_layout<'a>(
    ir: &'a core_ir::Module,
    name: &str,
) -> Option<&'a ConstructorLayout> {
    ir.data_layouts
        .values()
        .flat_map(|layout| layout.constructors.iter())
        .find(|ctor| ctor.name == name)
}

fn extract_match_binding_value(
    module: &mut ObjectModule,
    runtime: &RuntimeSymbols,
    builder: &mut FunctionBuilder,
    ptr_ty: Type,
    scrutinee_ptr: Value,
    binding: &MatchBinding,
) -> NativeResult<Value> {
    let mut current = scrutinee_ptr;
    if let Some(index) = binding.path.first() {
        let field_ref = module.declare_func_in_func(runtime.data_field, builder.func);
        let idx_value = builder.ins().iconst(ptr_ty, *index as i64);
        let call = builder.ins().call(field_ref, &[scrutinee_ptr, idx_value]);
        current = *builder.inst_results(call).first().ok_or_else(|| {
            NativeError::unsupported("CODEGEN168", "tl_data_field の戻り値が取得できませんでした")
        })?;
    } else if !matches!(binding.ty, ValueTy::Data { .. } | ValueTy::Unknown) {
        return Err(NativeError::unsupported(
            "CODEGEN169",
            format!(
                "Match 束縛 {:?} を scrutinee 全体に割り当てることは未対応です",
                binding.ty
            ),
        ));
    }

    match &binding.ty {
        ValueTy::Int => {
            let func_ref = module.declare_func_in_func(runtime.value_to_int, builder.func);
            let call = builder.ins().call(func_ref, &[current]);
            Ok(*builder.inst_results(call).first().ok_or_else(|| {
                NativeError::unsupported(
                    "CODEGEN170",
                    "tl_value_to_int の戻り値が取得できませんでした",
                )
            })?)
        }
        ValueTy::Double => {
            let func_ref = module.declare_func_in_func(runtime.value_to_double, builder.func);
            let call = builder.ins().call(func_ref, &[current]);
            Ok(*builder.inst_results(call).first().ok_or_else(|| {
                NativeError::unsupported(
                    "CODEGEN171",
                    "tl_value_to_double の戻り値が取得できませんでした",
                )
            })?)
        }
        ValueTy::Bool => {
            let func_ref = module.declare_func_in_func(runtime.value_to_bool, builder.func);
            let call = builder.ins().call(func_ref, &[current]);
            Ok(*builder.inst_results(call).first().ok_or_else(|| {
                NativeError::unsupported(
                    "CODEGEN172",
                    "tl_value_to_bool の戻り値が取得できませんでした",
                )
            })?)
        }
        ValueTy::Unit => Ok(builder.ins().iconst(types::I8, 0)),
        ValueTy::Data { .. } | ValueTy::Dictionary { .. } | ValueTy::List(_) | ValueTy::Unknown => {
            Ok(current)
        }
        ValueTy::Tuple(_) | ValueTy::Function { .. } | ValueTy::Char | ValueTy::String => {
            Err(NativeError::unsupported(
                "CODEGEN173",
                format!("Match 束縛型 {:?} は現在未対応です", binding.ty),
            ))
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn lower_if(
    module: &mut ObjectModule,
    ir: &core_ir::Module,
    runtime: &RuntimeSymbols,
    func_ids: &HashMap<String, FuncId>,
    builder: &mut FunctionBuilder,
    env: &mut CodegenEnv,
    cond: &Expr,
    then_expr: &Expr,
    else_expr: &Expr,
) -> NativeResult<LoweredValue> {
    let condition = lower_expr(module, ir, runtime, func_ids, builder, env, cond)?;
    if condition.ty != ValueTy::Bool {
        return Err(NativeError::unsupported(
            "CODEGEN080",
            "if 条件式の型は Bool である必要があります",
        ));
    }

    let cond_b1 = builder.ins().icmp_imm(IntCC::NotEqual, condition.value, 0);

    let then_block = builder.create_block();
    let else_block = builder.create_block();
    let merge_block = builder.create_block();

    builder
        .ins()
        .brif(cond_b1, then_block, &[], else_block, &[]);
    builder.seal_block(then_block);
    builder.seal_block(else_block);

    builder.switch_to_block(then_block);
    let mut then_env = env.clone();
    let then_value = lower_expr(
        module,
        ir,
        runtime,
        func_ids,
        builder,
        &mut then_env,
        then_expr,
    )?;
    let result_ty = then_value.ty.clone();

    let merge_ty = clif_type(env.ptr_ty(), &then_value.ty)?;
    let merge_param = builder.append_block_param(merge_block, merge_ty);
    builder.ins().jump(merge_block, &[then_value.value]);

    builder.switch_to_block(else_block);
    let mut else_env = env.clone();
    let else_value = lower_expr(
        module,
        ir,
        runtime,
        func_ids,
        builder,
        &mut else_env,
        else_expr,
    )?;
    if else_value.ty != result_ty {
        return Err(NativeError::unsupported(
            "CODEGEN081",
            "if の両分岐は同じ型を返す必要があります",
        ));
    }
    builder.ins().jump(merge_block, &[else_value.value]);

    builder.switch_to_block(merge_block);
    builder.seal_block(merge_block);

    Ok(LoweredValue::new(merge_param, result_ty))
}

fn binary_int_op<F>(
    builder: &mut FunctionBuilder,
    lhs: LoweredValue,
    rhs: LoweredValue,
    f: F,
) -> NativeResult<LoweredValue>
where
    F: FnOnce(&mut FunctionBuilder, Value, Value) -> Value,
{
    if lhs.ty != ValueTy::Int || rhs.ty != ValueTy::Int {
        return Err(NativeError::unsupported(
            "CODEGEN082",
            "整数演算の引数型が一致しません",
        ));
    }
    Ok(LoweredValue::new(
        f(builder, lhs.value, rhs.value),
        ValueTy::Int,
    ))
}

fn binary_double_op<F>(
    builder: &mut FunctionBuilder,
    lhs: LoweredValue,
    rhs: LoweredValue,
    f: F,
) -> NativeResult<LoweredValue>
where
    F: FnOnce(&mut FunctionBuilder, Value, Value) -> Value,
{
    if lhs.ty != ValueTy::Double || rhs.ty != ValueTy::Double {
        return Err(NativeError::unsupported(
            "CODEGEN084",
            "浮動小数演算の引数型が一致しません",
        ));
    }
    Ok(LoweredValue::new(
        f(builder, lhs.value, rhs.value),
        ValueTy::Double,
    ))
}

fn compare_int(
    builder: &mut FunctionBuilder,
    lhs: LoweredValue,
    rhs: LoweredValue,
    cc: IntCC,
) -> NativeResult<LoweredValue> {
    if lhs.ty != ValueTy::Int || rhs.ty != ValueTy::Int {
        return Err(NativeError::unsupported(
            "CODEGEN083",
            "比較演算の引数型が Int ではありません",
        ));
    }
    let cmp = builder.ins().icmp(cc, lhs.value, rhs.value);
    let one = builder.ins().iconst(types::I8, 1);
    let zero = builder.ins().iconst(types::I8, 0);
    let as_i8 = builder.ins().select(cmp, one, zero);
    Ok(LoweredValue::new(as_i8, ValueTy::Bool))
}

fn compare_double(
    builder: &mut FunctionBuilder,
    lhs: LoweredValue,
    rhs: LoweredValue,
    cc: FloatCC,
) -> NativeResult<LoweredValue> {
    if lhs.ty != ValueTy::Double || rhs.ty != ValueTy::Double {
        return Err(NativeError::unsupported(
            "CODEGEN085",
            "比較演算の引数型が Double ではありません",
        ));
    }
    let cmp = builder.ins().fcmp(cc, lhs.value, rhs.value);
    let one = builder.ins().iconst(types::I8, 1);
    let zero = builder.ins().iconst(types::I8, 0);
    let as_i8 = builder.ins().select(cmp, one, zero);
    Ok(LoweredValue::new(as_i8, ValueTy::Bool))
}

fn bool_to_b1(builder: &mut FunctionBuilder, value: Value) -> Value {
    builder.ins().icmp_imm(IntCC::NotEqual, value, 0)
}

fn bool_from_b1(builder: &mut FunctionBuilder, value: Value) -> Value {
    let one = builder.ins().iconst(types::I8, 1);
    let zero = builder.ins().iconst(types::I8, 0);
    builder.ins().select(value, one, zero)
}

fn binary_bool_op<F>(
    builder: &mut FunctionBuilder,
    lhs: LoweredValue,
    rhs: LoweredValue,
    f: F,
) -> NativeResult<LoweredValue>
where
    F: FnOnce(&mut FunctionBuilder, Value, Value) -> Value,
{
    if lhs.ty != ValueTy::Bool || rhs.ty != ValueTy::Bool {
        return Err(NativeError::unsupported(
            "CODEGEN086",
            "Bool 演算の引数が Bool ではありません",
        ));
    }
    let lhs_b = bool_to_b1(builder, lhs.value);
    let rhs_b = bool_to_b1(builder, rhs.value);
    let out_b = f(builder, lhs_b, rhs_b);
    let as_i8 = bool_from_b1(builder, out_b);
    Ok(LoweredValue::new(as_i8, ValueTy::Bool))
}

fn unary_bool_op<F>(
    builder: &mut FunctionBuilder,
    val: LoweredValue,
    f: F,
) -> NativeResult<LoweredValue>
where
    F: FnOnce(&mut FunctionBuilder, Value) -> Value,
{
    if val.ty != ValueTy::Bool {
        return Err(NativeError::unsupported(
            "CODEGEN087",
            "Bool 単項演算の引数が Bool ではありません",
        ));
    }
    let b = bool_to_b1(builder, val.value);
    let out = f(builder, b);
    let as_i8 = bool_from_b1(builder, out);
    Ok(LoweredValue::new(as_i8, ValueTy::Bool))
}

fn ensure_supported_function(func: &Function) -> NativeResult<()> {
    if let Some(param) = func.params.iter().find(|p| !is_supported_param_type(&p.ty)) {
        return Err(NativeError::unsupported(
            "CODEGEN090",
            format!(
                "関数 {} の引数型 {:?} は現在サポートされていません",
                func.name, param.ty
            ),
        ));
    }
    if !is_supported_return_type(&func.result) {
        return Err(NativeError::unsupported(
            "CODEGEN091",
            format!(
                "関数 {} の戻り値型 {:?} は未対応です",
                func.name, func.result
            ),
        ));
    }
    Ok(())
}

fn is_supported_param_type(ty: &ValueTy) -> bool {
    matches!(
        ty,
        ValueTy::Int
            | ValueTy::Bool
            | ValueTy::Double
            | ValueTy::Data { .. }
            | ValueTy::List(_)
            | ValueTy::Dictionary { .. }
            | ValueTy::Unknown
    )
}

fn is_supported_return_type(ty: &ValueTy) -> bool {
    matches!(
        ty,
        ValueTy::Int
            | ValueTy::Bool
            | ValueTy::Double
            | ValueTy::Unit
            | ValueTy::Data { .. }
            | ValueTy::List(_)
            | ValueTy::Dictionary { .. }
            | ValueTy::Unknown
    )
}

fn make_signature(func: &Function, call_conv: CallConv, ptr_ty: Type) -> NativeResult<Signature> {
    let mut sig = Signature::new(call_conv);
    for param in &func.params {
        sig.params
            .push(AbiParam::new(clif_type(ptr_ty, &param.ty)?));
    }
    if !matches!(func.result, ValueTy::Unit) {
        sig.returns
            .push(AbiParam::new(clif_type(ptr_ty, &func.result)?));
    }
    Ok(sig)
}

fn clif_type(ptr_ty: Type, ty: &ValueTy) -> NativeResult<Type> {
    match ty {
        ValueTy::Int => Ok(types::I64),
        ValueTy::Bool => Ok(types::I8),
        ValueTy::Unit => Ok(types::I8),
        ValueTy::Double => Ok(types::F64),
        ValueTy::Data { .. } | ValueTy::List(_) | ValueTy::Dictionary { .. } | ValueTy::Unknown => {
            Ok(ptr_ty)
        }
        ValueTy::Char | ValueTy::String | ValueTy::Tuple(_) | ValueTy::Function { .. } => Err(
            NativeError::unsupported("CODEGEN100", format!("型 {:?} は現在未対応です", ty)),
        ),
    }
}

fn symbol_name(name: &str) -> String {
    format!("{SYMBOL_PREFIX}{name}")
}

#[derive(Clone)]
struct VarInfo {
    var: Variable,
    ty: ValueTy,
}

#[derive(Clone)]
struct DictionaryParamBinding {
    classname: String,
    type_repr: String,
    var: Variable,
}

#[derive(Clone)]
struct CodegenEnv {
    vars: HashMap<String, VarInfo>,
    next_index: u32,
    ptr_ty: Type,
    dict_symbols: DictionarySymbols,
    dict_cache: HashMap<(String, String), Value>,
    dict_params: Vec<DictionaryParamBinding>,
}

impl CodegenEnv {
    fn new(ptr_ty: Type, dict_symbols: DictionarySymbols) -> Self {
        Self {
            vars: HashMap::new(),
            next_index: 0,
            ptr_ty,
            dict_symbols,
            dict_cache: HashMap::new(),
            dict_params: Vec::new(),
        }
    }

    fn insert_existing(
        &mut self,
        name: String,
        var: Variable,
        ty: ValueTy,
        dict_type_repr: Option<String>,
    ) {
        if let (ValueTy::Dictionary { classname }, Some(repr)) = (&ty, &dict_type_repr) {
            self.dict_params.push(DictionaryParamBinding {
                classname: classname.clone(),
                type_repr: repr.clone(),
                var,
            });
        }
        self.vars.insert(name, VarInfo { var, ty });
    }

    fn insert(&mut self, name: String, ty: ValueTy) -> Variable {
        let var = Variable::from_u32(self.next_index);
        self.next_index += 1;
        self.vars.insert(name, VarInfo { var, ty });
        var
    }

    fn get(&self, name: &str) -> Option<&VarInfo> {
        self.vars.get(name)
    }

    fn ptr_ty(&self) -> Type {
        self.ptr_ty
    }

    fn lookup_dictionary(&self, classname: &str, type_repr: &str) -> Option<FuncId> {
        self.dict_symbols
            .get(&(classname.to_string(), type_repr.to_string()))
            .copied()
    }

    fn dictionary_param(&self, classname: &str) -> Option<&DictionaryParamBinding> {
        self.dict_params
            .iter()
            .find(|binding| binding.classname == classname)
    }

    fn ensure_dictionary(
        &mut self,
        module: &mut ObjectModule,
        builder: &mut FunctionBuilder,
        classname: &str,
        type_repr: &str,
        func_id: FuncId,
    ) -> NativeResult<Value> {
        let key = (classname.to_string(), type_repr.to_string());
        if let Some(value) = self.dict_cache.get(&key) {
            return Ok(*value);
        }
        let func_ref = module.declare_func_in_func(func_id, builder.func);
        let call = builder.ins().call(func_ref, &[]);
        let results = builder.inst_results(call);
        let value = *results.first().ok_or_else(|| {
            NativeError::unsupported(
                "CODEGEN302",
                format!("辞書 {classname}<{type_repr}> の生成に失敗しました"),
            )
        })?;
        self.dict_cache.insert(key, value);
        Ok(value)
    }
}

struct LoweredValue {
    value: Value,
    ty: ValueTy,
}

impl LoweredValue {
    fn new(value: Value, ty: ValueTy) -> Self {
        Self { value, ty }
    }
}

fn coerce_value(
    module: &mut ObjectModule,
    builder: &mut FunctionBuilder,
    runtime: &RuntimeSymbols,
    value: LoweredValue,
    target: &ValueTy,
) -> NativeResult<LoweredValue> {
    if &value.ty == target {
        return Ok(value);
    }
    match (&value.ty, target) {
        (ValueTy::Int, ValueTy::Unknown) => {
            let boxed = call_runtime(builder, module, runtime.value_from_int, &[value.value]);
            Ok(LoweredValue::new(boxed, ValueTy::Unknown))
        }
        (ValueTy::Double, ValueTy::Unknown) => {
            let boxed = call_runtime(builder, module, runtime.value_from_double, &[value.value]);
            Ok(LoweredValue::new(boxed, ValueTy::Unknown))
        }
        (ValueTy::Bool, ValueTy::Unknown) => {
            let boxed = call_runtime(builder, module, runtime.value_from_bool, &[value.value]);
            Ok(LoweredValue::new(boxed, ValueTy::Unknown))
        }
        (ValueTy::Unknown, ValueTy::Int) => {
            let raw = call_runtime(builder, module, runtime.value_to_int, &[value.value]);
            Ok(LoweredValue::new(raw, ValueTy::Int))
        }
        (ValueTy::Unknown, ValueTy::Double) => {
            let raw = call_runtime(builder, module, runtime.value_to_double, &[value.value]);
            Ok(LoweredValue::new(raw, ValueTy::Double))
        }
        (ValueTy::Unknown, ValueTy::Bool) => {
            let raw = call_runtime(builder, module, runtime.value_to_bool, &[value.value]);
            Ok(LoweredValue::new(raw, ValueTy::Bool))
        }
        _ => Err(NativeError::unsupported(
            "CODEGEN214",
            format!("型 {:?} から {:?} への変換は未対応です", value.ty, target),
        )),
    }
}

fn call_runtime(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    func_id: FuncId,
    args: &[Value],
) -> Value {
    let func_ref = module.declare_func_in_func(func_id, builder.func);
    let call = builder.ins().call(func_ref, args);
    builder.inst_results(call)[0]
}
