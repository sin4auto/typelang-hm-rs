// パス: src/repl/mod.rs
// 役割: REPL module facade and re-exports
// 意図: Expose interactive entry points without leaking internals
// 関連ファイル: src/repl/cmd.rs, src/repl/loader.rs, src/bin/typelang.rs
//! TypeLang の対話環境を構成するモジュール群をまとめたファサード。
//!
//! 入力・型推論・評価・表示を役割ごとに分け、外部には最小限の API のみを公開する。
//! - `cmd`: メインループとコマンド解釈
//! - `loader`: 定義のロードと環境更新
//! - `printer`: ユーザー向けの表示ロジック
//! - `util`: REPL 内部の軽量ユーティリティ

pub mod cmd;
mod line_editor;
mod loader;
mod pipeline;
mod printer;
mod util;

// 既存パス互換のために公開 API を再公開
pub use cmd::run_repl;
pub use loader::load_program_into_env;
