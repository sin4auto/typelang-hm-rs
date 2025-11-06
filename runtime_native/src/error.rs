// パス: runtime_native/src/error.rs
// 役割: ランタイム共通のエラーコード管理と last error ストレージを実装する
// 意図: ネイティブ関数が失敗理由を報告し、ホスト側が診断できるようにする
// 関連ファイル: runtime_native/src/value.rs, runtime_native/src/dict.rs

use std::cell::Cell;
use std::ffi::{c_char, c_int};

/// ランタイム共通で利用するステータスコード。
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TlStatus {
    Ok = 0,
    InvalidArgument = 1,
    AllocationFailure = 2,
    NullPointer = 3,
}

impl TlStatus {
    pub fn from_code(code: i32) -> Self {
        match code {
            0 => Self::Ok,
            1 => Self::InvalidArgument,
            2 => Self::AllocationFailure,
            3 => Self::NullPointer,
            _ => Self::InvalidArgument,
        }
    }

    pub fn message(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::InvalidArgument => "invalid argument",
            Self::AllocationFailure => "allocation failure",
            Self::NullPointer => "null pointer",
        }
    }
}

#[derive(Debug)]
pub enum TlRuntimeError {
    InvalidArgument(&'static str),
    AllocationFailure,
    NullPointer,
}

impl TlRuntimeError {
    pub fn status(&self) -> TlStatus {
        match self {
            Self::InvalidArgument(_) => TlStatus::InvalidArgument,
            Self::AllocationFailure => TlStatus::AllocationFailure,
            Self::NullPointer => TlStatus::NullPointer,
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            Self::InvalidArgument(msg) => msg,
            Self::AllocationFailure => "allocation failure",
            Self::NullPointer => "null pointer",
        }
    }
}

thread_local! {
    static LAST_ERROR: Cell<TlStatus> = const { Cell::new(TlStatus::Ok) };
}

pub fn set_last_error(status: TlStatus) {
    LAST_ERROR.with(|cell| cell.set(status));
}

pub fn clear_last_error() {
    set_last_error(TlStatus::Ok);
}

#[no_mangle]
pub extern "C" fn tl_last_error() -> TlStatus {
    LAST_ERROR.with(|cell| cell.get())
}

#[no_mangle]
pub extern "C" fn tl_status_to_code(status: TlStatus) -> c_int {
    status as c_int
}

#[no_mangle]
pub extern "C" fn tl_status_from_code(code: c_int) -> TlStatus {
    TlStatus::from_code(code)
}

#[no_mangle]
pub extern "C" fn tl_status_message(code: TlStatus) -> *const c_char {
    match code {
        TlStatus::Ok => c"ok".as_ptr(),
        TlStatus::InvalidArgument => c"invalid argument".as_ptr(),
        TlStatus::AllocationFailure => c"allocation failure".as_ptr(),
        TlStatus::NullPointer => c"null pointer".as_ptr(),
    }
}

#[no_mangle]
pub extern "C" fn tl_abort_with_message(code: c_int) -> ! {
    eprintln!("TypeLang native runtime abort: code={code}");
    std::process::exit(1);
}
