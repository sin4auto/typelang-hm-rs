// パス: src/codegen/mod.rs
// 役割: バックエンド共通のエラー型とエイリアスを定義し、各バックエンド実装を束ねる
// 意図: Cranelift / LLVM など複数バックエンドをスイッチしやすくする
// 関連ファイル: src/codegen/cranelift.rs, src/codegen/dictionary_codegen.rs

pub mod cranelift;
pub mod dictionary_codegen;

use std::io;
use std::process::ExitStatus;

use thiserror::Error;

/// ネイティブコード生成で発生しうるエラー種別。
#[derive(Debug, Error)]
pub enum NativeError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("Cranelift module error: {0}")]
    Module(#[from] cranelift_module::ModuleError),
    #[error("Cranelift codegen error: {0}")]
    Codegen(#[from] cranelift_codegen::CodegenError),
    #[error("ネイティブバックエンド未対応: {message}")]
    Unsupported { code: &'static str, message: String },
    #[error("外部コマンド実行に失敗しました: {command} (status: {status:?})")]
    CommandFailure {
        command: String,
        status: Option<ExitStatus>,
        stderr: String,
    },
}

impl NativeError {
    pub fn unsupported(code: &'static str, message: impl Into<String>) -> Self {
        Self::Unsupported {
            code,
            message: message.into(),
        }
    }

    pub fn command_failure(
        command: impl Into<String>,
        status: Option<ExitStatus>,
        stderr: impl Into<String>,
    ) -> Self {
        Self::CommandFailure {
            command: command.into(),
            status,
            stderr: stderr.into(),
        }
    }
}

/// ネイティブコード生成の結果を表す型。
pub type NativeResult<T> = Result<T, NativeError>;

impl From<crate::core_ir::CoreIrError> for NativeError {
    fn from(err: crate::core_ir::CoreIrError) -> Self {
        Self::unsupported(err.code, err.message)
    }
}

impl From<tempfile::PersistError> for NativeError {
    fn from(err: tempfile::PersistError) -> Self {
        NativeError::Io(err.error)
    }
}
