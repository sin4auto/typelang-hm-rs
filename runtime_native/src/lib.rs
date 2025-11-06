//! TypeLang native runtime crate
//!
//! この crate はネイティブバックエンドからリンクされる実行時機能を提供する。
//! フェーズ2の仕様に基づき、値・リスト・代数的データ・型クラス辞書を
//! 個別モジュールに分割し、API を明確化している。

#![allow(clippy::missing_safety_doc)]

mod data;
mod dict;
mod error;
mod list;
mod value;

pub use data::*;
pub use dict::*;
pub use error::*;
pub use list::*;
pub use value::*;

include!(concat!(env!("OUT_DIR"), "/dict_autogen.rs"));
