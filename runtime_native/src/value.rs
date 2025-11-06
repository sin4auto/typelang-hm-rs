// パス: runtime_native/src/value.rs
// 役割: TlValue と TlBox の内部表現および値操作ユーティリティを提供する
// 意図: ランタイムが任意の値をボックス化して管理し、FFI 経由で安全に受け渡す
// 関連ファイル: runtime_native/src/error.rs, runtime_native/src/list.rs, runtime_native/src/data.rs

use crate::error::{clear_last_error, set_last_error, TlRuntimeError, TlStatus};
use std::ffi::c_void;

const TL_BOX_MAGIC: u64 = 0x544C5F424F585F31; // "TL_BOX_1"

#[repr(C)]
pub struct TlBox {
    magic: u64,
    kind: TlValueKind,
    payload: TlBoxPayload,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TlValueKind {
    Int = 0,
    Double = 1,
    Bool = 2,
    Pointer = 3,
}

#[repr(C)]
#[derive(Copy, Clone)]
union TlBoxPayload {
    int_value: i64,
    double_value: f64,
    bool_value: i8,
    ptr_value: *mut c_void,
}

impl TlBox {
    const fn new_int(value: i64) -> Self {
        Self {
            magic: TL_BOX_MAGIC,
            kind: TlValueKind::Int,
            payload: TlBoxPayload { int_value: value },
        }
    }

    const fn new_double(value: f64) -> Self {
        Self {
            magic: TL_BOX_MAGIC,
            kind: TlValueKind::Double,
            payload: TlBoxPayload {
                double_value: value,
            },
        }
    }

    const fn new_bool(value: bool) -> Self {
        Self {
            magic: TL_BOX_MAGIC,
            kind: TlValueKind::Bool,
            payload: TlBoxPayload {
                bool_value: if value { 1 } else { 0 },
            },
        }
    }

    fn new_ptr(ptr: *mut c_void) -> Self {
        Self {
            magic: TL_BOX_MAGIC,
            kind: TlValueKind::Pointer,
            payload: TlBoxPayload { ptr_value: ptr },
        }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TlValue(*mut TlBox);

impl TlValue {
    pub const fn null() -> Self {
        Self(std::ptr::null_mut())
    }

    pub fn from_raw(ptr: *mut c_void) -> Self {
        Self(ptr.cast())
    }

    pub fn as_raw(self) -> *mut c_void {
        self.0.cast()
    }

    fn validate(self) -> Result<*mut TlBox, TlRuntimeError> {
        if self.0.is_null() {
            return Err(TlRuntimeError::NullPointer);
        }
        // SAFETY: pointer checked for null
        let raw = unsafe { &*self.0 };
        if raw.magic != TL_BOX_MAGIC {
            return Err(TlRuntimeError::InvalidArgument("invalid TlValue handle"));
        }
        Ok(self.0)
    }
}

fn box_int(value: i64) -> Result<TlValue, TlRuntimeError> {
    let boxed = Box::new(TlBox::new_int(value));
    Ok(TlValue(Box::into_raw(boxed)))
}

fn box_double(value: f64) -> Result<TlValue, TlRuntimeError> {
    let boxed = Box::new(TlBox::new_double(value));
    Ok(TlValue(Box::into_raw(boxed)))
}

fn box_bool(value: bool) -> Result<TlValue, TlRuntimeError> {
    let boxed = Box::new(TlBox::new_bool(value));
    Ok(TlValue(Box::into_raw(boxed)))
}

fn box_ptr(ptr: *mut c_void) -> Result<TlValue, TlRuntimeError> {
    if ptr.is_null() {
        return Err(TlRuntimeError::NullPointer);
    }
    let boxed = Box::new(TlBox::new_ptr(ptr));
    Ok(TlValue(Box::into_raw(boxed)))
}

fn handle_result(result: Result<TlValue, TlRuntimeError>) -> TlValue {
    match result {
        Ok(value) => {
            clear_last_error();
            value
        }
        Err(err) => {
            set_last_error(err.status());
            TlValue::null()
        }
    }
}

#[no_mangle]
pub extern "C" fn tl_value_from_int(value: i64) -> TlValue {
    handle_result(box_int(value))
}

#[no_mangle]
pub extern "C" fn tl_value_from_double(value: f64) -> TlValue {
    handle_result(box_double(value))
}

#[no_mangle]
pub extern "C" fn tl_value_from_bool(value: i8) -> TlValue {
    handle_result(box_bool(value != 0))
}

#[no_mangle]
pub extern "C" fn tl_value_from_ptr(ptr: *mut c_void) -> TlValue {
    handle_result(box_ptr(ptr))
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TlValueResult {
    pub value: TlValue,
    pub status: TlStatus,
}

fn result_payload(result: Result<TlValue, TlRuntimeError>) -> TlValueResult {
    match result {
        Ok(value) => {
            clear_last_error();
            TlValueResult {
                value,
                status: TlStatus::Ok,
            }
        }
        Err(err) => {
            let status = err.status();
            set_last_error(status);
            TlValueResult {
                value: TlValue::null(),
                status,
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn tl_value_from_int_result(value: i64) -> TlValueResult {
    result_payload(box_int(value))
}

#[no_mangle]
pub extern "C" fn tl_value_from_double_result(value: f64) -> TlValueResult {
    result_payload(box_double(value))
}

#[no_mangle]
pub extern "C" fn tl_value_from_bool_result(value: i8) -> TlValueResult {
    result_payload(box_bool(value != 0))
}

#[no_mangle]
pub unsafe extern "C" fn tl_value_to_int(value: TlValue) -> i64 {
    match value.validate() {
        Ok(ptr) => {
            let boxed = &*ptr;
            match boxed.kind {
                TlValueKind::Int => boxed.payload.int_value,
                TlValueKind::Bool => boxed.payload.bool_value as i64,
                TlValueKind::Double => boxed.payload.double_value as i64,
                TlValueKind::Pointer => {
                    set_last_error(TlStatus::InvalidArgument);
                    0
                }
            }
        }
        Err(err) => {
            set_last_error(err.status());
            0
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn tl_value_to_double(value: TlValue) -> f64 {
    match value.validate() {
        Ok(ptr) => {
            let boxed = &*ptr;
            match boxed.kind {
                TlValueKind::Int => boxed.payload.int_value as f64,
                TlValueKind::Bool => boxed.payload.bool_value as f64,
                TlValueKind::Double => boxed.payload.double_value,
                TlValueKind::Pointer => {
                    set_last_error(TlStatus::InvalidArgument);
                    0.0
                }
            }
        }
        Err(err) => {
            set_last_error(err.status());
            0.0
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn tl_value_to_bool(value: TlValue) -> i8 {
    match value.validate() {
        Ok(ptr) => {
            let boxed = &*ptr;
            match boxed.kind {
                TlValueKind::Int => (boxed.payload.int_value != 0) as i8,
                TlValueKind::Double => (boxed.payload.double_value != 0.0) as i8,
                TlValueKind::Bool => boxed.payload.bool_value,
                TlValueKind::Pointer => {
                    set_last_error(TlStatus::InvalidArgument);
                    0
                }
            }
        }
        Err(err) => {
            set_last_error(err.status());
            0
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn tl_value_to_ptr(value: TlValue) -> *mut c_void {
    match value.validate() {
        Ok(ptr) => {
            let boxed = &*ptr;
            match boxed.kind {
                TlValueKind::Pointer => boxed.payload.ptr_value,
                _ => {
                    set_last_error(TlStatus::InvalidArgument);
                    std::ptr::null_mut()
                }
            }
        }
        Err(err) => {
            set_last_error(err.status());
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn tl_value_release(value: TlValue) {
    if value.0.is_null() {
        return;
    }
    let ptr = value.0;
    drop(Box::from_raw(ptr));
}

#[no_mangle]
pub extern "C" fn tl_print_int(value: i64) {
    println!("{value}");
}

#[no_mangle]
pub extern "C" fn tl_print_double(value: f64) {
    println!("{value}");
}

#[no_mangle]
pub extern "C" fn tl_print_bool(value: i8) {
    if value != 0 {
        println!("True");
    } else {
        println!("False");
    }
}

pub fn tl_value_kind(value: TlValue) -> Option<TlValueKind> {
    value.validate().ok().map(|ptr| unsafe { (*ptr).kind })
}
