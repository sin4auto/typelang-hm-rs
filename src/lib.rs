// パス: src/lib.rs
// 役割: Crate root wiring modules and exports
// 意図: Expose minimal API surface for language components
// 関連ファイル: src/ast.rs, src/parser.rs, src/errors.rs
//! TypeLang (Rust) ルートモジュール
//!
//! 目的:
//! - 学習用に最小構成の関数型言語処理系を提供する。
//! - 実装は読みやすさと変更容易性を最優先。
//!
//! 方針:
//! - コメント/ドキュメントは日本語、識別子は英語。
//! - 外部依存なし（標準ライブラリのみ）。
//! - パブリックAPIは最小限。
#![allow(unexpected_cfgs)]
#![cfg_attr(coverage, feature(coverage_attribute))]

pub mod ast;
pub mod codegen;
pub mod core_ir;
pub mod errors;
pub mod evaluator;
pub mod infer;
pub mod lexer;
pub mod parser;
pub(crate) mod primitives;
pub mod repl;
pub mod runtime;
pub mod typesys;

// 便利な再エクスポート（必要最小限: 利用側からAST/エラー/パーサのみ直接参照可）
pub use crate::ast::*;
pub use crate::errors::*;
pub use crate::parser::*;
// NOTE: `infer` と `evaluator` は曖昧な `initial_env` を持つため再エクスポートを控える。

/// AST プログラムを Core IR へ変換する。
pub fn compile_core_ir(program: &ast::Program) -> Result<core_ir::Module, core_ir::CoreIrError> {
    core_ir::lower::lower_program(program)
}

/// ネイティブビルド時に得られるメタデータ。
#[derive(Clone, Debug)]
pub struct NativeBuildArtifacts {
    pub dictionaries: Vec<core_ir::DictionaryInit>,
}

/// AST プログラムを解析してネイティブ実行ファイルを生成する。
#[allow(clippy::result_large_err)]
pub fn emit_native(
    program: &ast::Program,
    output: &std::path::Path,
) -> Result<NativeBuildArtifacts, codegen::NativeError> {
    emit_native_with_options(
        program,
        output,
        NativeBackend::Cranelift,
        NativeOptimLevel::Debug,
    )
}

/// 利用可能なネイティブバックエンド。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeBackend {
    Cranelift,
    Llvm,
}

/// コード生成時の最適化レベル指定。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeOptimLevel {
    Debug,
    Fast,
    Best,
}

#[allow(clippy::result_large_err)]
pub fn emit_native_with_options(
    program: &ast::Program,
    output: &std::path::Path,
    backend: NativeBackend,
    optim_level: NativeOptimLevel,
) -> Result<NativeBuildArtifacts, codegen::NativeError> {
    match backend {
        NativeBackend::Cranelift => {
            let _ = optim_level; // 現状は Cranelift 側に最適化レベルを伝搬しない
            let ir = compile_core_ir(program).map_err(codegen::NativeError::from)?;
            let dictionaries = ir.dictionaries.clone();
            codegen::cranelift::emit_native(&ir, output)?;
            Ok(NativeBuildArtifacts { dictionaries })
        }
        NativeBackend::Llvm => Err(codegen::NativeError::unsupported(
            "CODEGEN900",
            "LLVM backend はまだ実装されていません",
        )),
    }
}
