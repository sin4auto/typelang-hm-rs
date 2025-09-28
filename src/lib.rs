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

pub mod ast;
pub mod errors;
pub mod evaluator;
pub mod infer;
pub mod lexer;
pub mod parser;
pub mod repl;
pub mod typesys;

// 便利な再エクスポート（必要最小限: 利用側からAST/エラー/パーサのみ直接参照可）
pub use crate::ast::*;
pub use crate::errors::*;
pub use crate::parser::*;
// NOTE: `infer` と `evaluator` は曖昧な `initial_env` を持つため再エクスポートを控える。
