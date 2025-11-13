// パス: src/core_ir/lower.rs
// 役割: AST から Core IR への変換を担当する
// 意図: 型検証済みプログラムをターゲット非依存な IR へ落とし込み、バックエンドから利用できるようにする
// 関連ファイル: src/core_ir/mod.rs, src/repl/loader.rs, src/infer.rs

use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet, HashMap},
};

use crate::ast as A;
use crate::core_ir::{
    dict_specs, Binding, ConstructorLayout, CoreIrError, DataTypeLayout, DictionaryBuilder,
    DictionaryInit, DictionaryMethod, Expr, Function, Literal, MatchArm, MatchBinding, Module,
    Parameter, ParameterKind, PrimOp, SourceRef, ValueTy, VarKind,
};
use crate::evaluator;
use crate::infer;
use crate::intrinsics::{self, IntrinsicKind};
use crate::repl;
use crate::typesys::{
    Constraint as TyConstraint, QualType, Scheme, TApp, TCon, TFun, TTuple, TVar, Type, TypeEnv,
};

/// AST プログラムを Core IR へ変換するエントリポイント。
pub fn lower_program(prog: &A::Program) -> Result<Module, CoreIrError> {
    if !prog.class_decls.is_empty() {
        return Err(CoreIrError::new(
            "COREIR001",
            "class 宣言はネイティブコンパイラではまだサポートされていません",
        ));
    }
    if !prog.instance_decls.is_empty() {
        return Err(CoreIrError::new(
            "COREIR002",
            "instance 宣言はネイティブコンパイラではまだサポートされていません",
        ));
    }
    // 既存パイプラインを利用して型検証を実施し、型環境を取得する。
    let mut type_env = infer::initial_env();
    let mut class_env = infer::initial_class_env();
    let mut value_env = evaluator::initial_env();
    let load_result =
        repl::load_program_into_env(prog, &mut type_env, &mut class_env, &mut value_env);
    value_env.teardown();
    load_result.map_err(|msg| CoreIrError::new(classify_loader_error(&msg), msg))?;

    let mut ctx = LoweringContext::new(type_env.clone_env());
    ctx.register_data_layouts(prog);
    ctx.register_signatures(prog)?;
    ctx.lower_program(prog)
}

struct LoweringContext {
    type_env: TypeEnv,
    function_sigs: BTreeMap<String, FunctionSig>,
    data_layouts: BTreeMap<String, DataTypeLayout>,
    constructor_map: HashMap<String, ConstructorLayout>,
    dictionaries: Vec<DictionaryInit>,
    dictionary_keys: BTreeSet<(String, String)>,
}

impl LoweringContext {
    fn new(type_env: TypeEnv) -> Self {
        Self {
            type_env,
            function_sigs: BTreeMap::new(),
            data_layouts: BTreeMap::new(),
            constructor_map: HashMap::new(),
            dictionaries: Vec::new(),
            dictionary_keys: BTreeSet::new(),
        }
    }

    fn register_data_layouts(&mut self, prog: &A::Program) {
        for data_decl in &prog.data_decls {
            let mut layout = DataTypeLayout {
                name: data_decl.name.clone(),
                type_params: data_decl.params.clone(),
                constructors: Vec::new(),
            };
            for (idx, ctor) in data_decl.constructors.iter().enumerate() {
                let ctor_layout = ConstructorLayout {
                    name: ctor.name.clone(),
                    tag: idx as u32,
                    arity: ctor.args.len(),
                    parent: data_decl.name.clone(),
                    field_types: ctor.args.clone(),
                };
                layout.constructors.push(ctor_layout.clone());
                self.constructor_map.insert(ctor.name.clone(), ctor_layout);
            }
            self.data_layouts.insert(data_decl.name.clone(), layout);
        }
    }

    fn register_signatures(&mut self, prog: &A::Program) -> Result<(), CoreIrError> {
        for decl in &prog.decls {
            let scheme = self.type_env.lookup(&decl.name).ok_or_else(|| {
                CoreIrError::new("COREIR020", format!("{} の型が見つかりません", decl.name))
            })?;
            let scheme_for_sig = if let Some(sig_ast) = &decl.signature {
                scheme_from_sigma(sig_ast)
            } else {
                scheme.clone()
            };
            let source_span = span_to_source_ref(expr_span(&decl.expr));
            let dict_reprs =
                self.record_dictionaries_for(&decl.name, &scheme_for_sig, source_span)?;
            let mut sig = FunctionSig::from_scheme(&decl.name, &scheme_for_sig)?;
            sig.patch_dictionary_repr(&dict_reprs);

            let value_param_count = sig
                .param_specs
                .iter()
                .filter(|spec| matches!(spec.kind, ParameterKind::Value))
                .count();
            if value_param_count != decl.params.len() {
                return Err(CoreIrError::new(
                    "COREIR021",
                    format!(
                        "{} の型注釈の引数数 ({}) と定義上の引数数 ({}) が一致しません",
                        decl.name,
                        value_param_count,
                        decl.params.len()
                    ),
                ));
            }
            self.function_sigs.insert(decl.name.clone(), sig);
        }
        Ok(())
    }

    fn record_dictionaries_for(
        &mut self,
        origin: &str,
        scheme: &Scheme,
        span: SourceRef,
    ) -> Result<Vec<String>, CoreIrError> {
        let mut reprs = Vec::new();
        for constraint in &scheme.qual.constraints {
            let mut init = DictionaryInit {
                classname: constraint.classname.clone(),
                type_repr: type_to_string(&constraint.r#type),
                value_ty: convert_type_with_overrides(&constraint.r#type)?,
                methods: Vec::new(),
                scheme_repr: scheme_to_string(scheme),
                builder: DictionaryBuilder::Unresolved,
                origin: origin.to_string(),
                source_span: span,
            };

            if let Some(resolution) =
                resolve_builtin_dictionary(&constraint.classname, &constraint.r#type)
            {
                init.type_repr = resolution.type_repr.to_string();
                init.builder = DictionaryBuilder::Resolved(resolution.builder.to_string());
                init.value_ty = resolution.value_ty.clone();
                init.methods = resolution
                    .methods
                    .iter()
                    .map(|method| DictionaryMethod {
                        name: method.name.to_string(),
                        signature: Some(method.signature.clone()),
                        symbol: method.symbol.to_string(),
                        method_id: method.method_id,
                    })
                    .collect();
            }

            let repr = init.type_repr.clone();
            let key = (init.classname.clone(), repr.clone());
            if self.dictionary_keys.insert(key) {
                self.dictionaries.push(init);
            }
            reprs.push(repr);
        }
        Ok(reprs)
    }

    fn lower_program(self, prog: &A::Program) -> Result<Module, CoreIrError> {
        let mut module = Module::new();
        for decl in &prog.decls {
            let func = self.lower_top_level(decl)?;
            module.insert_function(func);
        }
        if module.entry.is_none() && module.functions.contains_key("main") {
            module.set_entry("main");
        }
        module.data_layouts = self.data_layouts;
        module.dictionaries = self.dictionaries;
        Ok(module)
    }

    fn lower_top_level(&self, decl: &A::TopLevel) -> Result<Function, CoreIrError> {
        let sig = self.function_sigs.get(&decl.name).ok_or_else(|| {
            CoreIrError::new(
                "COREIR030",
                format!("{} の型情報が利用できません", decl.name),
            )
        })?;

        let mut env = Env::new();
        let mut value_param_iter = decl.params.iter();
        let mut params: Vec<Parameter> = Vec::new();

        for (idx, spec) in sig.param_specs.iter().enumerate() {
            match &spec.kind {
                ParameterKind::Dictionary { classname } => {
                    let name = format!("$dict{}_{}", idx, classname);
                    env.insert(name.clone(), spec.ty.clone());
                    params.push(Parameter::with_kind(
                        name,
                        spec.ty.clone(),
                        ParameterKind::Dictionary {
                            classname: classname.clone(),
                        },
                        spec.dict_type_repr.clone(),
                        spec.dict_value_ty.clone(),
                    ));
                }
                ParameterKind::Value => {
                    let param_name = value_param_iter.next().ok_or_else(|| {
                        CoreIrError::new(
                            "COREIR032",
                            format!("{} の引数整合性チェックに失敗しました", decl.name),
                        )
                    })?;
                    env.insert(param_name.clone(), spec.ty.clone());
                    params.push(Parameter::with_kind(
                        param_name.clone(),
                        spec.ty.clone(),
                        ParameterKind::Value,
                        None,
                        None,
                    ));
                }
            }
        }

        if value_param_iter.next().is_some() {
            return Err(CoreIrError::new(
                "COREIR033",
                format!("{} の実引数が余剰です", decl.name),
            ));
        }

        let body_expr = self.lower_expr(&decl.expr, &mut env)?;
        if !types_compatible(&sig.result, body_expr.ty()) {
            return Err(CoreIrError::new(
                "COREIR031",
                format!(
                    "{} の戻り値型が一致しません: 期待 {:?}, 実際 {:?}",
                    decl.name,
                    sig.result,
                    body_expr.ty()
                ),
            ));
        }

        Ok(Function {
            name: decl.name.clone(),
            params,
            result: sig.result.clone(),
            body: body_expr,
            location: SourceRef::default(),
        })
    }

    fn lower_expr(&self, expr: &A::Expr, env: &mut Env) -> Result<Expr, CoreIrError> {
        use A::Expr::*;
        match expr {
            IntLit { value, .. } => Ok(Expr::Literal {
                value: Literal::Int(*value),
                ty: ValueTy::Int,
            }),
            FloatLit { value, .. } => Ok(Expr::Literal {
                value: Literal::Double(*value),
                ty: ValueTy::Double,
            }),
            BoolLit { value, .. } => Ok(Expr::Literal {
                value: Literal::Bool(*value),
                ty: ValueTy::Bool,
            }),
            CharLit { value, .. } => Ok(Expr::Literal {
                value: Literal::Char(*value),
                ty: ValueTy::Char,
            }),
            StringLit { value, .. } => Ok(Expr::Literal {
                value: Literal::String(value.clone()),
                ty: ValueTy::String,
            }),
            ListLit { items, .. } => self.lower_list(items, env),
            TupleLit { items, .. } => self.lower_tuple(items, env),
            Var { name, .. } => self.lower_var(name, env),
            LetIn { bindings, body, .. } => self.lower_let(bindings, body, env),
            If {
                cond,
                then_branch,
                else_branch,
                ..
            } => self.lower_if(cond, then_branch, else_branch, env),
            App { func, arg, .. } => self.lower_app(func, arg, env),
            BinOp {
                op, left, right, ..
            } => self.lower_binop(op, left, right, env),
            Lambda { .. } => Err(CoreIrError::new(
                "COREIR050",
                "ラムダ式はトップレベル以外ではまだサポートされていません",
            )),
            Annot { expr, .. } => self.lower_expr(expr, env),
            Case {
                scrutinee, arms, ..
            } => self.lower_case(scrutinee, arms, env),
        }
    }

    fn lower_list(&self, items: &[A::Expr], env: &mut Env) -> Result<Expr, CoreIrError> {
        if items.is_empty() {
            return Ok(Expr::List {
                items: vec![],
                ty: ValueTy::List(Box::new(ValueTy::Unknown)),
            });
        }
        let mut lowered_items = Vec::with_capacity(items.len());
        for item in items {
            let lowered = self.lower_expr(item, env)?;
            lowered_items.push(lowered);
        }
        let elem_ty = lowered_items[0].ty().clone();
        if lowered_items
            .iter()
            .any(|item| !types_compatible(&elem_ty, item.ty()))
        {
            return Err(CoreIrError::new(
                "COREIR060",
                "リスト要素の型が一致しません",
            ));
        }
        Ok(Expr::List {
            items: lowered_items,
            ty: ValueTy::List(Box::new(elem_ty)),
        })
    }

    fn lower_tuple(&self, items: &[A::Expr], env: &mut Env) -> Result<Expr, CoreIrError> {
        let mut lowered_items = Vec::with_capacity(items.len());
        for item in items {
            lowered_items.push(self.lower_expr(item, env)?);
        }
        let tys = lowered_items.iter().map(|e| e.ty().clone()).collect();
        Ok(Expr::Tuple {
            items: lowered_items,
            ty: ValueTy::Tuple(tys),
        })
    }

    fn lower_var(&self, name: &str, env: &Env) -> Result<Expr, CoreIrError> {
        if let Some(ty) = env.lookup(name) {
            return Ok(Expr::Var {
                name: name.into(),
                ty,
                kind: VarKind::Local,
            });
        }
        if let Some(sig) = self.function_sigs.get(name) {
            let params = sig.param_specs.iter().map(|spec| spec.ty.clone()).collect();
            let ty = ValueTy::Function {
                params,
                result: Box::new(sig.result.clone()),
            };
            return Ok(Expr::Var {
                name: name.into(),
                ty,
                kind: VarKind::Function,
            });
        }
        if let Some(intr) = intrinsics::lookup(name) {
            let ty = intrinsic_function_ty(intr.kind);
            return Ok(Expr::Var {
                name: name.into(),
                ty,
                kind: VarKind::Intrinsic,
            });
        }
        if let Some(scheme) = self.type_env.lookup(name) {
            let sig = FunctionSig::from_scheme(name, scheme)?;
            let params = sig.param_specs.iter().map(|spec| spec.ty.clone()).collect();
            let ty = ValueTy::Function {
                params,
                result: Box::new(sig.result.clone()),
            };
            return Ok(Expr::Var {
                name: name.into(),
                ty,
                kind: VarKind::Function,
            });
        }
        if let Some(ctor) = self.constructor_map.get(name) {
            let result_ty = if let Some(data_layout) = self.data_layouts.get(&ctor.parent) {
                ValueTy::Data {
                    constructor: data_layout.name.clone(),
                    args: vec![ValueTy::Unknown; data_layout.type_params.len()],
                }
            } else {
                ValueTy::Unknown
            };
            let ty = ValueTy::Function {
                params: vec![ValueTy::Unknown; ctor.arity],
                result: Box::new(result_ty),
            };
            return Ok(Expr::Var {
                name: name.into(),
                ty,
                kind: VarKind::Primitive,
            });
        }
        Err(CoreIrError::new(
            "COREIR070",
            format!("{} はネイティブコンパイル時に解決できません", name),
        ))
    }

    fn lower_let(
        &self,
        bindings: &[(String, Vec<String>, A::Expr)],
        body: &A::Expr,
        env: &mut Env,
    ) -> Result<Expr, CoreIrError> {
        let mut local_env = env.clone();
        let mut lowered = Vec::with_capacity(bindings.len());
        for (name, params, expr) in bindings {
            if !params.is_empty() {
                return Err(CoreIrError::new(
                    "COREIR080",
                    "ローカル関数定義は現在サポートされていません",
                ));
            }
            let value = self.lower_expr(expr, &mut local_env)?;
            let ty = value.ty().clone();
            local_env.insert(name.clone(), ty.clone());
            lowered.push(Binding {
                name: name.clone(),
                value,
                ty,
            });
        }
        let body_expr = self.lower_expr(body, &mut local_env)?;
        Ok(Expr::Let {
            bindings: lowered,
            body: Box::new(body_expr.clone()),
            ty: body_expr.ty().clone(),
        })
    }

    fn lower_if(
        &self,
        cond: &A::Expr,
        then_branch: &A::Expr,
        else_branch: &A::Expr,
        env: &mut Env,
    ) -> Result<Expr, CoreIrError> {
        let cond_expr = self.lower_expr(cond, env)?;
        if !types_compatible(&ValueTy::Bool, cond_expr.ty()) {
            return Err(CoreIrError::new(
                "COREIR090",
                "if 条件式は Bool 型である必要があります",
            ));
        }
        let then_expr = self.lower_expr(then_branch, env)?;
        let else_expr = self.lower_expr(else_branch, env)?;
        if !types_compatible(then_expr.ty(), else_expr.ty()) {
            return Err(CoreIrError::new(
                "COREIR091",
                "if の分岐結果の型が一致しません",
            ));
        }
        Ok(Expr::If {
            cond: Box::new(cond_expr),
            then_branch: Box::new(then_expr.clone()),
            else_branch: Box::new(else_expr.clone()),
            ty: then_expr.ty().clone(),
        })
    }

    fn lower_app(&self, func: &A::Expr, arg: &A::Expr, env: &mut Env) -> Result<Expr, CoreIrError> {
        let (head, mut arg_nodes) = flatten_app(func, arg);
        let callee = self.lower_expr(head, env)?;
        let mut lowered_args = Vec::with_capacity(arg_nodes.len());
        for node in arg_nodes.drain(..) {
            lowered_args.push(self.lower_expr(node, env)?);
        }

        let lowered_args = if let Expr::Var { name, kind, .. } = &callee {
            match kind {
                VarKind::Function => {
                    let param_specs: Option<Cow<'_, [ParameterSpec]>> =
                        if let Some(sig) = self.function_sigs.get(name) {
                            Some(Cow::Borrowed(&sig.param_specs))
                        } else if let Some(scheme) = self.type_env.lookup(name) {
                            let sig = FunctionSig::from_scheme(name, scheme)?;
                            Some(Cow::Owned(sig.param_specs))
                        } else {
                            None
                        };
                    if let Some(specs) = param_specs {
                        self.inject_dictionary_args(&specs, lowered_args)?
                    } else {
                        lowered_args
                    }
                }
                VarKind::Intrinsic => lowered_args,
                _ => lowered_args,
            }
        } else {
            lowered_args
        };

        let result_ty = infer_apply_type(&callee, &lowered_args)?;
        Ok(Expr::Apply {
            func: Box::new(callee),
            args: lowered_args,
            ty: result_ty,
        })
    }

    fn lower_binop(
        &self,
        op: &str,
        left: &A::Expr,
        right: &A::Expr,
        env: &mut Env,
    ) -> Result<Expr, CoreIrError> {
        let lhs = self.lower_expr(left, env)?;
        let rhs = self.lower_expr(right, env)?;
        let mapping = map_binop(op, lhs.ty(), rhs.ty())?;
        Ok(Expr::PrimOp {
            op: mapping.prim_op,
            args: vec![lhs, rhs],
            ty: mapping.result_ty,
            dict_fallback: mapping.dict_fallback,
        })
    }

    fn lower_case(
        &self,
        scrutinee: &A::Expr,
        arms: &[A::CaseArm],
        env: &mut Env,
    ) -> Result<Expr, CoreIrError> {
        if arms.is_empty() {
            return Err(CoreIrError::new("COREIR052", "case 式に分岐がありません"));
        }
        let scrutinee_ir = self.lower_expr(scrutinee, env)?;
        let mut ir_arms = Vec::with_capacity(arms.len());
        let mut result_ty: Option<ValueTy> = None;

        for arm in arms {
            let mut branch_env = env.clone();

            let binding_infos = self.pattern_bindings(&arm.pattern, scrutinee_ir.ty())?;
            for binding in &binding_infos {
                branch_env.insert(binding.name.clone(), binding.ty.clone());
            }

            let guard_ir = if let Some(guard) = &arm.guard {
                let lowered = self.lower_expr(guard, &mut branch_env)?;
                if !types_compatible(&ValueTy::Bool, lowered.ty()) {
                    return Err(CoreIrError::new(
                        "COREIR053",
                        "case ガードは Bool 型である必要があります",
                    ));
                }
                Some(lowered)
            } else {
                None
            };

            let body_ir = self.lower_expr(&arm.body, &mut branch_env)?;
            let body_ty = body_ir.ty().clone();
            match &mut result_ty {
                Some(expected) => {
                    if matches!(expected, ValueTy::Unknown) && !matches!(body_ty, ValueTy::Unknown)
                    {
                        *expected = body_ty.clone();
                    } else if !types_compatible(expected, &body_ty) {
                        return Err(CoreIrError::new(
                            "COREIR054",
                            "case 式の分岐結果の型が一致しません",
                        ));
                    }
                }
                None => {
                    result_ty = Some(body_ty.clone());
                }
            }

            ir_arms.push(MatchArm {
                pattern: arm.pattern.clone(),
                guard: guard_ir,
                body: body_ir,
                constructor: pattern_constructor(&arm.pattern).map(|s| s.to_string()),
                tag: pattern_constructor(&arm.pattern)
                    .and_then(|ctor| self.constructor_map.get(ctor))
                    .map(|info| info.tag),
                arity: pattern_constructor(&arm.pattern)
                    .and_then(|ctor| self.constructor_map.get(ctor))
                    .map(|info| info.arity)
                    .unwrap_or(0),
                bindings: binding_infos,
            });
        }

        Ok(Expr::Match {
            scrutinee: Box::new(scrutinee_ir),
            arms: ir_arms,
            ty: result_ty.unwrap_or(ValueTy::Unknown),
        })
    }

    fn pattern_bindings(
        &self,
        pattern: &A::Pattern,
        expected_ty: &ValueTy,
    ) -> Result<Vec<MatchBinding>, CoreIrError> {
        let mut out = Vec::new();
        let mut path = Vec::new();
        self.collect_pattern_bindings(pattern, expected_ty, &mut out, &mut path)?;
        Ok(out)
    }

    fn collect_pattern_bindings(
        &self,
        pattern: &A::Pattern,
        expected_ty: &ValueTy,
        out: &mut Vec<MatchBinding>,
        path: &mut Vec<usize>,
    ) -> Result<(), CoreIrError> {
        match pattern {
            A::Pattern::Wildcard { .. }
            | A::Pattern::Int { .. }
            | A::Pattern::Float { .. }
            | A::Pattern::Char { .. }
            | A::Pattern::String { .. }
            | A::Pattern::Bool { .. } => {}
            A::Pattern::Var { name, .. } => out.push(MatchBinding {
                name: name.clone(),
                ty: expected_ty.clone(),
                path: path.clone(),
            }),
            A::Pattern::List { items, .. } => {
                if !items.is_empty() {
                    return Err(CoreIrError::new(
                        "COREIR162",
                        "list パターンのネイティブローワリングは未対応です",
                    ));
                }
            }
            A::Pattern::Tuple { items, .. } => {
                if !items.is_empty() {
                    return Err(CoreIrError::new(
                        "COREIR163",
                        "tuple パターンのネイティブローワリングは未対応です",
                    ));
                }
            }
            A::Pattern::As {
                binder, pattern, ..
            } => {
                out.push(MatchBinding {
                    name: binder.clone(),
                    ty: expected_ty.clone(),
                    path: path.clone(),
                });
                self.collect_pattern_bindings(pattern, expected_ty, out, path)?;
            }
            A::Pattern::Constructor { name, args, .. } => {
                let field_types = self.resolve_constructor_field_types(name, expected_ty)?;
                for (idx, arg_pattern) in args.iter().enumerate() {
                    let child_ty = field_types.get(idx).cloned().unwrap_or(ValueTy::Unknown);
                    path.push(idx);
                    self.collect_pattern_bindings(arg_pattern, &child_ty, out, path)?;
                    path.pop();
                }
            }
        }
        Ok(())
    }

    fn inject_dictionary_args(
        &self,
        param_specs: &[ParameterSpec],
        value_args: Vec<Expr>,
    ) -> Result<Vec<Expr>, CoreIrError> {
        let expected_value_args = param_specs
            .iter()
            .filter(|spec| matches!(spec.kind, ParameterKind::Value))
            .count();
        if expected_value_args != value_args.len() {
            return Err(CoreIrError::new(
                "COREIR132",
                format!(
                    "辞書引数を含む関数呼び出しの値引数数が一致しません: 期待 {}, 実際 {}",
                    expected_value_args,
                    value_args.len()
                ),
            ));
        }
        let mut value_iter = value_args.into_iter();
        let mut final_args = Vec::with_capacity(param_specs.len());
        for spec in param_specs {
            match &spec.kind {
                ParameterKind::Dictionary { classname } => {
                    let type_repr = spec
                        .dict_type_repr
                        .clone()
                        .unwrap_or_else(|| "_".to_string());
                    final_args.push(Expr::DictionaryPlaceholder {
                        classname: classname.clone(),
                        type_repr,
                        ty: spec.ty.clone(),
                    })
                }
                ParameterKind::Value => final_args.push(
                    value_iter
                        .next()
                        .expect("value_iter length validated by expected_value_args"),
                ),
            }
        }

        Ok(final_args)
    }

    fn resolve_constructor_field_types(
        &self,
        ctor_name: &str,
        expected_ty: &ValueTy,
    ) -> Result<Vec<ValueTy>, CoreIrError> {
        let ctor = self.constructor_map.get(ctor_name).ok_or_else(|| {
            CoreIrError::new(
                "COREIR160",
                format!("コンストラクタ {} が登録されていません", ctor_name),
            )
        })?;
        let data_layout = self.data_layouts.get(&ctor.parent).ok_or_else(|| {
            CoreIrError::new(
                "COREIR161",
                format!("データ型 {} のレイアウトが見つかりません", ctor.parent),
            )
        })?;

        let mut subst = HashMap::new();
        if let ValueTy::Data { constructor, args } = expected_ty {
            if constructor == &data_layout.name && args.len() == data_layout.type_params.len() {
                for (param, arg_ty) in data_layout.type_params.iter().zip(args.iter()) {
                    subst.insert(param.clone(), arg_ty.clone());
                }
            }
        }

        let mut result = Vec::with_capacity(ctor.field_types.len());
        for field in &ctor.field_types {
            result.push(type_expr_to_value_ty(field, &subst));
        }
        Ok(result)
    }
}

#[derive(Clone, Debug)]
struct ParameterSpec {
    ty: ValueTy,
    kind: ParameterKind,
    dict_type_repr: Option<String>,
    dict_value_ty: Option<ValueTy>,
}

#[derive(Clone, Debug)]
struct FunctionSig {
    param_specs: Vec<ParameterSpec>,
    result: ValueTy,
}

impl FunctionSig {
    fn from_scheme(_name: &str, scheme: &Scheme) -> Result<Self, CoreIrError> {
        let mut specs = Vec::new();
        for constraint in &scheme.qual.constraints {
            let dict_value_ty = convert_type_with_overrides(&constraint.r#type)?;
            specs.push(ParameterSpec {
                ty: ValueTy::Dictionary {
                    classname: constraint.classname.clone(),
                },
                kind: ParameterKind::Dictionary {
                    classname: constraint.classname.clone(),
                },
                dict_type_repr: None,
                dict_value_ty: Some(dict_value_ty),
            });
        }
        let (value_params, result_ty) = flatten_fun_type_types(&scheme.qual.r#type);
        for param_ty in value_params {
            specs.push(ParameterSpec {
                ty: convert_type_with_overrides(&param_ty)?,
                kind: ParameterKind::Value,
                dict_type_repr: None,
                dict_value_ty: None,
            });
        }
        let result = convert_type_with_overrides(&result_ty)?;
        Ok(Self {
            param_specs: specs,
            result,
        })
    }

    fn patch_dictionary_repr(&mut self, reprs: &[String]) {
        let mut iter = reprs.iter();
        for spec in &mut self.param_specs {
            if matches!(spec.kind, ParameterKind::Dictionary { .. }) {
                spec.dict_type_repr = iter.next().cloned();
            }
        }
    }
}

fn flatten_fun_type_types(ty: &Type) -> (Vec<Type>, Type) {
    match ty {
        Type::TFun(TFun { arg, ret }) => {
            let mut params = Vec::new();
            params.push(*arg.clone());
            let (mut rest, result) = flatten_fun_type_types(ret);
            params.append(&mut rest);
            (params, result)
        }
        _ => (Vec::new(), ty.clone()),
    }
}

fn flatten_fun_type(ty: &Type) -> Result<(Vec<ValueTy>, ValueTy), CoreIrError> {
    match ty {
        Type::TFun(fun) => {
            let arg_ty = convert_type(&fun.arg)?;
            let (mut rest, result) = flatten_fun_type(&fun.ret)?;
            rest.insert(0, arg_ty);
            Ok((rest, result))
        }
        _ => Ok((Vec::new(), convert_type(ty)?)),
    }
}

fn convert_type_with_overrides(ty: &Type) -> Result<ValueTy, CoreIrError> {
    match ty {
        Type::TVar(_) => Ok(ValueTy::Unknown),
        _ => convert_type(ty),
    }
}

fn convert_type(ty: &Type) -> Result<ValueTy, CoreIrError> {
    match ty {
        Type::TCon(TCon { name }) => match name.as_str() {
            "Int" | "Integer" => Ok(ValueTy::Int),
            "Double" => Ok(ValueTy::Double),
            "Bool" => Ok(ValueTy::Bool),
            "Char" => Ok(ValueTy::Char),
            "String" => Ok(ValueTy::String),
            "Unit" => Ok(ValueTy::Unit),
            other => Ok(ValueTy::Data {
                constructor: other.to_string(),
                args: Vec::new(),
            }),
        },
        Type::TTuple(TTuple { items }) => {
            let mut lowered = Vec::with_capacity(items.len());
            for item in items {
                lowered.push(convert_type(item)?);
            }
            Ok(ValueTy::Tuple(lowered))
        }
        Type::TApp(TApp { func, arg }) => {
            if let Type::TCon(TCon { name }) = func.as_ref() {
                if name == "[]" {
                    let elem_ty = convert_type(arg)?;
                    if matches!(elem_ty, ValueTy::Char) {
                        return Ok(ValueTy::String);
                    }
                    return Ok(ValueTy::List(Box::new(elem_ty)));
                }
            }
            let func_ty = convert_type(func)?;
            let arg_ty = convert_type(arg)?;
            match func_ty {
                ValueTy::Data {
                    constructor,
                    mut args,
                } => {
                    args.push(arg_ty);
                    Ok(ValueTy::Data { constructor, args })
                }
                ValueTy::Function { .. } | ValueTy::Unknown => Ok(ValueTy::Unknown),
                ValueTy::List(_)
                | ValueTy::Tuple(_)
                | ValueTy::Dictionary { .. }
                | ValueTy::Int
                | ValueTy::Double
                | ValueTy::Bool
                | ValueTy::Char
                | ValueTy::String
                | ValueTy::Unit => Ok(ValueTy::Unknown),
            }
        }
        Type::TVar(_) => Ok(ValueTy::Unknown),
        Type::TFun(_fun) => {
            let (params, result) = flatten_fun_type(ty)?;
            Ok(ValueTy::Function {
                params,
                result: Box::new(result),
            })
        }
    }
}

#[derive(Debug)]
struct Env {
    stack: HashMap<String, ValueTy>,
}

impl Env {
    fn new() -> Self {
        Self {
            stack: HashMap::new(),
        }
    }

    fn insert(&mut self, name: String, ty: ValueTy) {
        self.stack.insert(name, ty);
    }

    fn lookup(&self, name: &str) -> Option<ValueTy> {
        self.stack.get(name).cloned()
    }
}

impl Clone for Env {
    fn clone(&self) -> Self {
        Self {
            stack: self.stack.clone(),
        }
    }
}

fn flatten_app<'a>(func: &'a A::Expr, arg: &'a A::Expr) -> (&'a A::Expr, Vec<&'a A::Expr>) {
    let mut head = func;
    let mut args = vec![arg];
    let mut current = func;
    while let A::Expr::App {
        func: inner, arg, ..
    } = current
    {
        head = inner;
        args.push(arg);
        current = inner;
    }
    args.reverse();
    (head, args)
}

fn intrinsic_function_ty(kind: IntrinsicKind) -> ValueTy {
    match kind {
        IntrinsicKind::Println => ValueTy::Function {
            params: vec![ValueTy::Unknown],
            result: Box::new(ValueTy::Unknown),
        },
    }
}

fn infer_apply_type(func: &Expr, args: &[Expr]) -> Result<ValueTy, CoreIrError> {
    match func.ty() {
        ValueTy::Function { params, result } => {
            if params.len() < args.len() {
                return Err(CoreIrError::new(
                    "COREIR130",
                    format!(
                        "関数適用の引数が多すぎます: 期待 {} 個, 実際 {} 個",
                        params.len(),
                        args.len()
                    ),
                ));
            }
            for (idx, (expected, actual)) in params.iter().zip(args.iter()).enumerate() {
                if !types_compatible(expected, actual.ty()) {
                    return Err(CoreIrError::new(
                        "COREIR131",
                        format!(
                            "引数 {} の型が一致しません: 期待 {:?}, 実際 {:?}",
                            idx + 1,
                            expected,
                            actual.ty()
                        ),
                    ));
                }
            }
            if params.len() == args.len() {
                Ok(*result.clone())
            } else {
                let remaining = params[args.len()..].to_vec();
                Ok(ValueTy::Function {
                    params: remaining,
                    result: result.clone(),
                })
            }
        }
        _ => Ok(ValueTy::Unknown),
    }
}

fn pattern_constructor(pattern: &A::Pattern) -> Option<&str> {
    if let A::Pattern::Constructor { name, .. } = pattern {
        Some(name.as_str())
    } else {
        None
    }
}

#[derive(Clone, Debug)]
struct BinOpMapping {
    prim_op: PrimOp,
    result_ty: ValueTy,
    dict_fallback: bool,
}

impl BinOpMapping {
    fn direct(prim_op: PrimOp, result_ty: ValueTy) -> Self {
        Self {
            prim_op,
            result_ty,
            dict_fallback: false,
        }
    }

    fn dictionary(prim_op: PrimOp, result_ty: ValueTy) -> Self {
        Self {
            prim_op,
            result_ty,
            dict_fallback: true,
        }
    }
}

#[derive(Clone, Copy)]
enum OperandKind {
    Int,
    Double,
    Bool,
}

impl OperandKind {
    fn matches(self, ty: &ValueTy) -> bool {
        matches!(
            (self, ty),
            (OperandKind::Int, ValueTy::Int)
                | (OperandKind::Double, ValueTy::Double)
                | (OperandKind::Bool, ValueTy::Bool)
        )
    }
}

struct BinOpCase {
    lhs: OperandKind,
    rhs: OperandKind,
    prim_op: PrimOp,
    result: ValueTy,
}

impl BinOpCase {
    fn matches(&self, lhs: &ValueTy, rhs: &ValueTy) -> bool {
        self.lhs.matches(lhs) && self.rhs.matches(rhs)
    }
}

#[derive(Clone)]
struct BinOpDictFallback {
    prim_op: PrimOp,
    result: ValueTy,
}

struct BinOpSpec {
    symbol: &'static str,
    error_code: &'static str,
    cases: &'static [BinOpCase],
    dict_fallback: Option<BinOpDictFallback>,
}

impl BinOpSpec {
    fn lookup_case(&self, lhs: &ValueTy, rhs: &ValueTy) -> Option<&BinOpCase> {
        self.cases.iter().find(|case| case.matches(lhs, rhs))
    }

    fn type_mismatch(&self, lhs: &ValueTy, rhs: &ValueTy) -> CoreIrError {
        CoreIrError::new(
            self.error_code,
            format!(
                "{} 演算子の型が一致しません: {:?} vs {:?}",
                self.symbol, lhs, rhs
            ),
        )
    }
}

const BINOP_SPECS: &[BinOpSpec] = &[
    BinOpSpec {
        symbol: "+",
        error_code: "COREIR141",
        cases: &[
            BinOpCase {
                lhs: OperandKind::Int,
                rhs: OperandKind::Int,
                prim_op: PrimOp::AddInt,
                result: ValueTy::Int,
            },
            BinOpCase {
                lhs: OperandKind::Double,
                rhs: OperandKind::Double,
                prim_op: PrimOp::AddDouble,
                result: ValueTy::Double,
            },
        ],
        dict_fallback: Some(BinOpDictFallback {
            prim_op: PrimOp::AddInt,
            result: ValueTy::Unknown,
        }),
    },
    BinOpSpec {
        symbol: "-",
        error_code: "COREIR142",
        cases: &[
            BinOpCase {
                lhs: OperandKind::Int,
                rhs: OperandKind::Int,
                prim_op: PrimOp::SubInt,
                result: ValueTy::Int,
            },
            BinOpCase {
                lhs: OperandKind::Double,
                rhs: OperandKind::Double,
                prim_op: PrimOp::SubDouble,
                result: ValueTy::Double,
            },
        ],
        dict_fallback: Some(BinOpDictFallback {
            prim_op: PrimOp::SubInt,
            result: ValueTy::Unknown,
        }),
    },
    BinOpSpec {
        symbol: "*",
        error_code: "COREIR143",
        cases: &[
            BinOpCase {
                lhs: OperandKind::Int,
                rhs: OperandKind::Int,
                prim_op: PrimOp::MulInt,
                result: ValueTy::Int,
            },
            BinOpCase {
                lhs: OperandKind::Double,
                rhs: OperandKind::Double,
                prim_op: PrimOp::MulDouble,
                result: ValueTy::Double,
            },
        ],
        dict_fallback: Some(BinOpDictFallback {
            prim_op: PrimOp::MulInt,
            result: ValueTy::Unknown,
        }),
    },
    BinOpSpec {
        symbol: "/",
        error_code: "COREIR144",
        cases: &[BinOpCase {
            lhs: OperandKind::Double,
            rhs: OperandKind::Double,
            prim_op: PrimOp::DivDouble,
            result: ValueTy::Double,
        }],
        dict_fallback: Some(BinOpDictFallback {
            prim_op: PrimOp::DivDouble,
            result: ValueTy::Unknown,
        }),
    },
    BinOpSpec {
        symbol: "div",
        error_code: "COREIR145",
        cases: &[BinOpCase {
            lhs: OperandKind::Int,
            rhs: OperandKind::Int,
            prim_op: PrimOp::DivInt,
            result: ValueTy::Int,
        }],
        dict_fallback: Some(BinOpDictFallback {
            prim_op: PrimOp::DivInt,
            result: ValueTy::Unknown,
        }),
    },
    BinOpSpec {
        symbol: "mod",
        error_code: "COREIR146",
        cases: &[BinOpCase {
            lhs: OperandKind::Int,
            rhs: OperandKind::Int,
            prim_op: PrimOp::ModInt,
            result: ValueTy::Int,
        }],
        dict_fallback: Some(BinOpDictFallback {
            prim_op: PrimOp::ModInt,
            result: ValueTy::Unknown,
        }),
    },
    BinOpSpec {
        symbol: "==",
        error_code: "COREIR147",
        cases: &[
            BinOpCase {
                lhs: OperandKind::Int,
                rhs: OperandKind::Int,
                prim_op: PrimOp::EqInt,
                result: ValueTy::Bool,
            },
            BinOpCase {
                lhs: OperandKind::Bool,
                rhs: OperandKind::Bool,
                prim_op: PrimOp::EqInt,
                result: ValueTy::Bool,
            },
            BinOpCase {
                lhs: OperandKind::Double,
                rhs: OperandKind::Double,
                prim_op: PrimOp::EqDouble,
                result: ValueTy::Bool,
            },
        ],
        dict_fallback: Some(BinOpDictFallback {
            prim_op: PrimOp::EqInt,
            result: ValueTy::Bool,
        }),
    },
    BinOpSpec {
        symbol: "/=",
        error_code: "COREIR148",
        cases: &[
            BinOpCase {
                lhs: OperandKind::Int,
                rhs: OperandKind::Int,
                prim_op: PrimOp::NeqInt,
                result: ValueTy::Bool,
            },
            BinOpCase {
                lhs: OperandKind::Bool,
                rhs: OperandKind::Bool,
                prim_op: PrimOp::NeqInt,
                result: ValueTy::Bool,
            },
            BinOpCase {
                lhs: OperandKind::Double,
                rhs: OperandKind::Double,
                prim_op: PrimOp::NeqDouble,
                result: ValueTy::Bool,
            },
        ],
        dict_fallback: Some(BinOpDictFallback {
            prim_op: PrimOp::NeqInt,
            result: ValueTy::Bool,
        }),
    },
    BinOpSpec {
        symbol: "<",
        error_code: "COREIR149",
        cases: &[
            BinOpCase {
                lhs: OperandKind::Int,
                rhs: OperandKind::Int,
                prim_op: PrimOp::LtInt,
                result: ValueTy::Bool,
            },
            BinOpCase {
                lhs: OperandKind::Double,
                rhs: OperandKind::Double,
                prim_op: PrimOp::LtDouble,
                result: ValueTy::Bool,
            },
        ],
        dict_fallback: Some(BinOpDictFallback {
            prim_op: PrimOp::LtInt,
            result: ValueTy::Bool,
        }),
    },
    BinOpSpec {
        symbol: "<=",
        error_code: "COREIR150",
        cases: &[
            BinOpCase {
                lhs: OperandKind::Int,
                rhs: OperandKind::Int,
                prim_op: PrimOp::LeInt,
                result: ValueTy::Bool,
            },
            BinOpCase {
                lhs: OperandKind::Double,
                rhs: OperandKind::Double,
                prim_op: PrimOp::LeDouble,
                result: ValueTy::Bool,
            },
        ],
        dict_fallback: Some(BinOpDictFallback {
            prim_op: PrimOp::LeInt,
            result: ValueTy::Bool,
        }),
    },
    BinOpSpec {
        symbol: ">",
        error_code: "COREIR151",
        cases: &[
            BinOpCase {
                lhs: OperandKind::Int,
                rhs: OperandKind::Int,
                prim_op: PrimOp::GtInt,
                result: ValueTy::Bool,
            },
            BinOpCase {
                lhs: OperandKind::Double,
                rhs: OperandKind::Double,
                prim_op: PrimOp::GtDouble,
                result: ValueTy::Bool,
            },
        ],
        dict_fallback: Some(BinOpDictFallback {
            prim_op: PrimOp::GtInt,
            result: ValueTy::Bool,
        }),
    },
    BinOpSpec {
        symbol: ">=",
        error_code: "COREIR152",
        cases: &[
            BinOpCase {
                lhs: OperandKind::Int,
                rhs: OperandKind::Int,
                prim_op: PrimOp::GeInt,
                result: ValueTy::Bool,
            },
            BinOpCase {
                lhs: OperandKind::Double,
                rhs: OperandKind::Double,
                prim_op: PrimOp::GeDouble,
                result: ValueTy::Bool,
            },
        ],
        dict_fallback: Some(BinOpDictFallback {
            prim_op: PrimOp::GeInt,
            result: ValueTy::Bool,
        }),
    },
    BinOpSpec {
        symbol: "&&",
        error_code: "COREIR153",
        cases: &[BinOpCase {
            lhs: OperandKind::Bool,
            rhs: OperandKind::Bool,
            prim_op: PrimOp::AndBool,
            result: ValueTy::Bool,
        }],
        dict_fallback: Some(BinOpDictFallback {
            prim_op: PrimOp::AndBool,
            result: ValueTy::Bool,
        }),
    },
    BinOpSpec {
        symbol: "||",
        error_code: "COREIR154",
        cases: &[BinOpCase {
            lhs: OperandKind::Bool,
            rhs: OperandKind::Bool,
            prim_op: PrimOp::OrBool,
            result: ValueTy::Bool,
        }],
        dict_fallback: Some(BinOpDictFallback {
            prim_op: PrimOp::OrBool,
            result: ValueTy::Bool,
        }),
    },
];

fn map_binop(op: &str, lhs_ty: &ValueTy, rhs_ty: &ValueTy) -> Result<BinOpMapping, CoreIrError> {
    let spec = BINOP_SPECS
        .iter()
        .find(|spec| spec.symbol == op)
        .ok_or_else(|| {
            CoreIrError::new(
                "COREIR140",
                format!(
                    "演算子 {} はまだネイティブバックエンドで対応していません",
                    op
                ),
            )
        })?;

    if let Some(case) = spec.lookup_case(lhs_ty, rhs_ty) {
        return Ok(BinOpMapping::direct(case.prim_op, case.result.clone()));
    }

    let needs_dict = matches!(lhs_ty, ValueTy::Unknown) || matches!(rhs_ty, ValueTy::Unknown);
    if needs_dict {
        if let Some(dict) = &spec.dict_fallback {
            return Ok(BinOpMapping::dictionary(dict.prim_op, dict.result.clone()));
        }
    }

    Err(spec.type_mismatch(lhs_ty, rhs_ty))
}

fn type_expr_to_value_ty(expr: &A::TypeExpr, subst: &HashMap<String, ValueTy>) -> ValueTy {
    match expr {
        A::TypeExpr::TEVar(name) => subst.get(name).cloned().unwrap_or(ValueTy::Unknown),
        A::TypeExpr::TECon(name) => match name.as_str() {
            "Int" => ValueTy::Int,
            "Double" => ValueTy::Double,
            "Bool" => ValueTy::Bool,
            "Char" => ValueTy::Char,
            "String" => ValueTy::String,
            "Unit" => ValueTy::Unit,
            other => ValueTy::Data {
                constructor: other.to_string(),
                args: Vec::new(),
            },
        },
        A::TypeExpr::TEApp(func, arg) => {
            let func_ty = type_expr_to_value_ty(func, subst);
            let arg_ty = type_expr_to_value_ty(arg, subst);
            match func_ty {
                ValueTy::Data {
                    constructor,
                    mut args,
                } => {
                    args.push(arg_ty);
                    ValueTy::Data { constructor, args }
                }
                _ => ValueTy::Unknown,
            }
        }
        A::TypeExpr::TEFun(arg, result) => {
            let arg_ty = type_expr_to_value_ty(arg, subst);
            let res_ty = type_expr_to_value_ty(result, subst);
            ValueTy::Function {
                params: vec![arg_ty],
                result: Box::new(res_ty),
            }
        }
        A::TypeExpr::TEList(inner) => {
            let item_ty = type_expr_to_value_ty(inner, subst);
            ValueTy::List(Box::new(item_ty))
        }
        A::TypeExpr::TETuple(items) => {
            let lowered = items
                .iter()
                .map(|item| type_expr_to_value_ty(item, subst))
                .collect();
            ValueTy::Tuple(lowered)
        }
    }
}

fn type_to_string(ty: &Type) -> String {
    match ty {
        Type::TVar(tv) => format!("t{}", tv.id),
        Type::TCon(tc) => tc.name.clone(),
        Type::TApp(TApp { func, arg }) => match func.as_ref() {
            Type::TCon(TCon { name }) if name == "[]" => format!("[{}]", type_to_string(arg)),
            _ => {
                let func_str = type_to_string(func);
                let arg_str = match arg.as_ref() {
                    Type::TFun(_) => format!("({})", type_to_string(arg)),
                    _ => type_to_string(arg),
                };
                format!("{} {}", func_str, arg_str)
            }
        },
        Type::TFun(TFun { arg, ret }) => {
            let arg_str = match arg.as_ref() {
                Type::TFun(_) => format!("({})", type_to_string(arg)),
                _ => type_to_string(arg),
            };
            format!("{} -> {}", arg_str, type_to_string(ret))
        }
        Type::TTuple(TTuple { items }) => {
            let inner = items
                .iter()
                .map(type_to_string)
                .collect::<Vec<_>>()
                .join(", ");
            format!("({inner})")
        }
    }
}

struct DictionaryResolution {
    builder: &'static str,
    methods: Vec<DictionaryResolutionMethod>,
    type_repr: &'static str,
    value_ty: ValueTy,
}

struct DictionaryResolutionMethod {
    name: &'static str,
    signature: String,
    symbol: &'static str,
    method_id: u64,
}

const NUM_INT_METHOD_SYMBOLS: &[(&str, &str)] = &[
    ("add", "tl_num_int_add"),
    ("sub", "tl_num_int_sub"),
    ("mul", "tl_num_int_mul"),
    ("fromInt", "tl_num_int_from_int"),
];

const NUM_DOUBLE_METHOD_SYMBOLS: &[(&str, &str)] = &[
    ("add", "tl_num_double_add"),
    ("sub", "tl_num_double_sub"),
    ("mul", "tl_num_double_mul"),
    ("fromInt", "tl_num_double_from_int"),
];

const FRACTIONAL_DOUBLE_METHOD_SYMBOLS: &[(&str, &str)] = &[("div", "tl_fractional_double_div")];

const INTEGRAL_INT_METHOD_SYMBOLS: &[(&str, &str)] = &[
    ("div", "tl_integral_int_div"),
    ("mod", "tl_integral_int_mod"),
];

const EQ_INT_METHOD_SYMBOLS: &[(&str, &str)] = &[("eq", "tl_eq_int"), ("neq", "tl_neq_int")];

const EQ_DOUBLE_METHOD_SYMBOLS: &[(&str, &str)] =
    &[("eq", "tl_eq_double"), ("neq", "tl_neq_double")];

const EQ_BOOL_METHOD_SYMBOLS: &[(&str, &str)] = &[("eq", "tl_eq_bool"), ("neq", "tl_neq_bool")];

const ORD_INT_METHOD_SYMBOLS: &[(&str, &str)] = &[
    ("lt", "tl_ord_int_lt"),
    ("le", "tl_ord_int_le"),
    ("gt", "tl_ord_int_gt"),
    ("ge", "tl_ord_int_ge"),
];

const ORD_DOUBLE_METHOD_SYMBOLS: &[(&str, &str)] = &[
    ("lt", "tl_ord_double_lt"),
    ("le", "tl_ord_double_le"),
    ("gt", "tl_ord_double_gt"),
    ("ge", "tl_ord_double_ge"),
];

const BOOL_LOGIC_METHOD_SYMBOLS: &[(&str, &str)] = &[
    ("and", "tl_bool_logic_and"),
    ("or", "tl_bool_logic_or"),
    ("not", "tl_bool_logic_not"),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum BuiltinTypeKind {
    Int,
    Double,
    Bool,
}

#[derive(Clone)]
struct BuiltinDictionaryDescriptor {
    classname: &'static str,
    target: BuiltinTypeKind,
    type_repr: &'static str,
    builder: &'static str,
    value_ty: ValueTy,
    method_symbols: &'static [(&'static str, &'static str)],
}

const BUILTIN_DICTIONARY_DESCRIPTORS: &[BuiltinDictionaryDescriptor] = &[
    BuiltinDictionaryDescriptor {
        classname: "Num",
        target: BuiltinTypeKind::Int,
        type_repr: "Int",
        builder: "tl_dict_build_Num_Int",
        value_ty: ValueTy::Int,
        method_symbols: NUM_INT_METHOD_SYMBOLS,
    },
    BuiltinDictionaryDescriptor {
        classname: "Num",
        target: BuiltinTypeKind::Double,
        type_repr: "Double",
        builder: "tl_dict_build_Num_Double",
        value_ty: ValueTy::Double,
        method_symbols: NUM_DOUBLE_METHOD_SYMBOLS,
    },
    BuiltinDictionaryDescriptor {
        classname: "Fractional",
        target: BuiltinTypeKind::Double,
        type_repr: "Double",
        builder: "tl_dict_build_Fractional_Double",
        value_ty: ValueTy::Double,
        method_symbols: FRACTIONAL_DOUBLE_METHOD_SYMBOLS,
    },
    BuiltinDictionaryDescriptor {
        classname: "Integral",
        target: BuiltinTypeKind::Int,
        type_repr: "Int",
        builder: "tl_dict_build_Integral_Int",
        value_ty: ValueTy::Int,
        method_symbols: INTEGRAL_INT_METHOD_SYMBOLS,
    },
    BuiltinDictionaryDescriptor {
        classname: "Eq",
        target: BuiltinTypeKind::Int,
        type_repr: "Int",
        builder: "tl_dict_build_Eq_Int",
        value_ty: ValueTy::Int,
        method_symbols: EQ_INT_METHOD_SYMBOLS,
    },
    BuiltinDictionaryDescriptor {
        classname: "Eq",
        target: BuiltinTypeKind::Double,
        type_repr: "Double",
        builder: "tl_dict_build_Eq_Double",
        value_ty: ValueTy::Double,
        method_symbols: EQ_DOUBLE_METHOD_SYMBOLS,
    },
    BuiltinDictionaryDescriptor {
        classname: "Eq",
        target: BuiltinTypeKind::Bool,
        type_repr: "Bool",
        builder: "tl_dict_build_Eq_Bool",
        value_ty: ValueTy::Bool,
        method_symbols: EQ_BOOL_METHOD_SYMBOLS,
    },
    BuiltinDictionaryDescriptor {
        classname: "Ord",
        target: BuiltinTypeKind::Int,
        type_repr: "Int",
        builder: "tl_dict_build_Ord_Int",
        value_ty: ValueTy::Int,
        method_symbols: ORD_INT_METHOD_SYMBOLS,
    },
    BuiltinDictionaryDescriptor {
        classname: "Ord",
        target: BuiltinTypeKind::Double,
        type_repr: "Double",
        builder: "tl_dict_build_Ord_Double",
        value_ty: ValueTy::Double,
        method_symbols: ORD_DOUBLE_METHOD_SYMBOLS,
    },
    BuiltinDictionaryDescriptor {
        classname: "BoolLogic",
        target: BuiltinTypeKind::Bool,
        type_repr: "Bool",
        builder: "tl_dict_build_BoolLogic_Bool",
        value_ty: ValueTy::Bool,
        method_symbols: BOOL_LOGIC_METHOD_SYMBOLS,
    },
];

fn resolve_builtin_dictionary(classname: &str, ty: &Type) -> Option<DictionaryResolution> {
    let descriptor = descriptor_for(classname, ty)?;
    let methods = build_methods_for_descriptor(descriptor)?;
    Some(DictionaryResolution {
        builder: descriptor.builder,
        methods,
        type_repr: descriptor.type_repr,
        value_ty: descriptor.value_ty.clone(),
    })
}

fn descriptor_for(classname: &str, ty: &Type) -> Option<&'static BuiltinDictionaryDescriptor> {
    let fallback = match classname {
        "Num" | "Integral" | "Eq" | "Ord" => BuiltinTypeKind::Int,
        "Fractional" => BuiltinTypeKind::Double,
        "BoolLogic" => BuiltinTypeKind::Bool,
        _ => return None,
    };
    let kind = detect_builtin_type(ty).unwrap_or(fallback);
    BUILTIN_DICTIONARY_DESCRIPTORS
        .iter()
        .find(|desc| desc.classname == classname && desc.target == kind)
}

fn build_methods_for_descriptor(
    descriptor: &BuiltinDictionaryDescriptor,
) -> Option<Vec<DictionaryResolutionMethod>> {
    let specs = dict_specs::methods_for_class(descriptor.classname)?;
    let mut methods = Vec::with_capacity(specs.len());
    for spec in specs {
        let symbol = descriptor
            .method_symbols
            .iter()
            .find(|(name, _)| *name == spec.name)
            .map(|(_, sym)| *sym)?;
        let signature = spec.pattern.instantiate(descriptor.type_repr).into_owned();
        methods.push(DictionaryResolutionMethod {
            name: spec.name,
            signature,
            symbol,
            method_id: spec.method_id,
        });
    }
    Some(methods)
}

fn detect_builtin_type(ty: &Type) -> Option<BuiltinTypeKind> {
    match ty {
        Type::TCon(TCon { name }) => match name.as_str() {
            "Int" | "Integer" => Some(BuiltinTypeKind::Int),
            "Double" => Some(BuiltinTypeKind::Double),
            "Bool" => Some(BuiltinTypeKind::Bool),
            _ => None,
        },
        Type::TVar(_) => None,
        _ => None,
    }
}

fn scheme_to_string(scheme: &Scheme) -> String {
    let vars = if scheme.vars.is_empty() {
        String::new()
    } else {
        let names = scheme
            .vars
            .iter()
            .map(|tv| format!("t{}", tv.id))
            .collect::<Vec<_>>()
            .join(", ");
        format!("forall {names}. ")
    };
    let constraints = if scheme.qual.constraints.is_empty() {
        String::new()
    } else {
        let cs = scheme
            .qual
            .constraints
            .iter()
            .map(|c| format!("{} {}", c.classname, type_to_string(&c.r#type)))
            .collect::<Vec<_>>()
            .join(", ");
        format!("{cs} => ")
    };
    format!("{vars}{constraints}{}", type_to_string(&scheme.qual.r#type))
}

fn collect_type_vars(ty: &Type, out: &mut Vec<TVar>) {
    match ty {
        Type::TVar(tv) => out.push(tv.clone()),
        Type::TCon(_) => {}
        Type::TApp(TApp { func, arg }) => {
            collect_type_vars(func, out);
            collect_type_vars(arg, out);
        }
        Type::TFun(TFun { arg, ret }) => {
            collect_type_vars(arg, out);
            collect_type_vars(ret, out);
        }
        Type::TTuple(TTuple { items }) => {
            for item in items {
                collect_type_vars(item, out);
            }
        }
    }
}

fn scheme_from_sigma(sigma: &A::SigmaType) -> Scheme {
    use std::collections::HashMap as VarMap;

    let mut next_id = -1000;
    let mut prebound: VarMap<String, TVar> = VarMap::new();
    for constraint in &sigma.constraints {
        prebound
            .entry(constraint.typevar.clone())
            .or_insert_with(|| {
                let tv = TVar { id: next_id };
                next_id -= 1;
                tv
            });
    }
    let ty = if prebound.is_empty() {
        infer::type_from_texpr(&sigma.r#type)
    } else {
        infer::type_from_texpr_with_vars(&sigma.r#type, &prebound)
    };
    let mut vars = Vec::new();
    collect_type_vars(&ty, &mut vars);
    vars.extend(prebound.values().cloned());
    vars.sort_by_key(|tv| tv.id);
    vars.dedup();
    let constraints = sigma
        .constraints
        .iter()
        .map(|c| TyConstraint {
            classname: c.classname.clone(),
            r#type: Type::TVar(
                prebound
                    .get(&c.typevar)
                    .cloned()
                    .unwrap_or(TVar { id: next_id }),
            ),
        })
        .collect();
    Scheme {
        vars,
        qual: QualType {
            constraints,
            r#type: ty,
        },
    }
}

fn classify_loader_error(message: &str) -> &'static str {
    if message.contains("class") || message.contains("型クラス") {
        "COREIR401"
    } else {
        "COREIR010"
    }
}

fn span_to_source_ref(span: A::Span) -> SourceRef {
    SourceRef::new(span.line, span.col)
}

fn expr_span(expr: &A::Expr) -> A::Span {
    match expr {
        A::Expr::Var { span, .. }
        | A::Expr::IntLit { span, .. }
        | A::Expr::FloatLit { span, .. }
        | A::Expr::CharLit { span, .. }
        | A::Expr::StringLit { span, .. }
        | A::Expr::BoolLit { span, .. }
        | A::Expr::ListLit { span, .. }
        | A::Expr::TupleLit { span, .. }
        | A::Expr::Lambda { span, .. }
        | A::Expr::LetIn { span, .. }
        | A::Expr::If { span, .. }
        | A::Expr::App { span, .. }
        | A::Expr::BinOp { span, .. }
        | A::Expr::Annot { span, .. }
        | A::Expr::Case { span, .. } => *span,
    }
}

fn types_compatible(expected: &ValueTy, actual: &ValueTy) -> bool {
    matches!(expected, ValueTy::Unknown)
        || matches!(actual, ValueTy::Unknown)
        || expected == actual
        || match (expected, actual) {
            (ValueTy::Tuple(exp), ValueTy::Tuple(act)) => {
                exp.len() == act.len()
                    && exp
                        .iter()
                        .zip(act.iter())
                        .all(|(e, a)| types_compatible(e, a))
            }
            (ValueTy::List(exp), ValueTy::List(act)) => types_compatible(exp, act),
            (
                ValueTy::Function {
                    params: p1,
                    result: r1,
                },
                ValueTy::Function {
                    params: p2,
                    result: r2,
                },
            ) => {
                p1.len() == p2.len()
                    && p1
                        .iter()
                        .zip(p2.iter())
                        .all(|(a, b)| types_compatible(a, b))
                    && types_compatible(r1, r2)
            }
            (
                ValueTy::Data {
                    constructor: c1,
                    args: a1,
                },
                ValueTy::Data {
                    constructor: c2,
                    args: a2,
                },
            ) => {
                c1 == c2
                    && a1.len() == a2.len()
                    && a1
                        .iter()
                        .zip(a2.iter())
                        .all(|(x, y)| types_compatible(x, y))
            }
            (ValueTy::Dictionary { classname: a }, ValueTy::Dictionary { classname: b }) => a == b,
            _ => false,
        }
}
