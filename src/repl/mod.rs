//! REPL（対話環境）モジュール
//!
//! 入力受付・コマンド解釈・ファイルロード・表示などを薄く分割しています。
//! - `cmd`: ユーザー入力の解釈とメインループ
//! - `loader`: プログラムの型/値環境へのロード
//! - `printer`: 値やヘルプの表示
//! - `util`: REPL 内部で使う小さな正規化処理

pub mod cmd;
mod loader;
mod printer;
mod util;

// 既存パス互換のために公開 API を再公開
pub use cmd::run_repl;
pub use loader::load_program_into_env;
