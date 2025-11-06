// パス: runtime_native/src/data.rs
// 役割: 代数的データ型 (TlData) の表現と操作ユーティリティを提供する
// 意図: ネイティブバックエンドがランタイム ABI を通じてデータコンストラクタを扱えるようにする
// 関連ファイル: runtime_native/src/value.rs, runtime_native/src/list.rs

use crate::error::{set_last_error, TlRuntimeError, TlStatus};
use crate::value::TlValue;

const TL_DATA_MAGIC: u64 = 0x544C5F4441544131; // "TL_DATA1"

#[repr(C)]
pub struct TlData {
    magic: u64,
    tag: u32,
    len: usize,
    fields: *mut TlValue,
}

impl TlData {
    fn new(tag: u32, fields: &[TlValue]) -> Result<*mut TlData, TlRuntimeError> {
        let ptr_fields = if fields.is_empty() {
            std::ptr::null_mut()
        } else {
            let mut boxed_fields = Vec::with_capacity(fields.len());
            boxed_fields.extend_from_slice(fields);
            let ptr = boxed_fields.as_mut_ptr();
            std::mem::forget(boxed_fields);
            ptr
        };

        let data = TlData {
            magic: TL_DATA_MAGIC,
            tag,
            len: fields.len(),
            fields: ptr_fields,
        };

        Ok(Box::into_raw(Box::new(data)))
    }

    unsafe fn ensure(ptr: *const TlData) -> Result<*const TlData, TlRuntimeError> {
        if ptr.is_null() {
            return Err(TlRuntimeError::NullPointer);
        }
        let data = &*ptr;
        if data.magic != TL_DATA_MAGIC {
            return Err(TlRuntimeError::InvalidArgument("invalid TlData handle"));
        }
        Ok(ptr)
    }

    unsafe fn ensure_mut(ptr: *mut TlData) -> Result<*mut TlData, TlRuntimeError> {
        if ptr.is_null() {
            return Err(TlRuntimeError::NullPointer);
        }
        let data_ref = &*ptr;
        if data_ref.magic != TL_DATA_MAGIC {
            return Err(TlRuntimeError::InvalidArgument("invalid TlData handle"));
        }
        Ok(ptr)
    }
}

#[no_mangle]
pub unsafe extern "C" fn tl_data_pack(tag: u32, fields: *const TlValue, len: usize) -> *mut TlData {
    if len == 0 {
        match TlData::new(tag, &[]) {
            Ok(ptr) => {
                set_last_error(TlStatus::Ok);
                return ptr;
            }
            Err(err) => {
                set_last_error(err.status());
                return std::ptr::null_mut();
            }
        }
    }
    if fields.is_null() {
        set_last_error(TlStatus::NullPointer);
        return std::ptr::null_mut();
    }
    let slice = std::slice::from_raw_parts(fields, len);
    match TlData::new(tag, slice) {
        Ok(ptr) => {
            set_last_error(TlStatus::Ok);
            ptr
        }
        Err(err) => {
            set_last_error(err.status());
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn tl_data_tag(data: *const TlData) -> u32 {
    match TlData::ensure(data) {
        Ok(value) => {
            let data = &*value;
            set_last_error(TlStatus::Ok);
            data.tag
        }
        Err(err) => {
            set_last_error(err.status());
            0
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn tl_data_arity(data: *const TlData) -> usize {
    match TlData::ensure(data) {
        Ok(value) => {
            let data = &*value;
            set_last_error(TlStatus::Ok);
            data.len
        }
        Err(err) => {
            set_last_error(err.status());
            0
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn tl_data_field(data: *const TlData, index: usize) -> TlValue {
    match TlData::ensure(data) {
        Ok(value) => {
            let data = &*value;
            if index >= data.len {
                set_last_error(TlStatus::InvalidArgument);
                TlValue::null()
            } else {
                set_last_error(TlStatus::Ok);
                *data.fields.add(index)
            }
        }
        Err(err) => {
            set_last_error(err.status());
            TlValue::null()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn tl_data_free(data: *mut TlData) {
    if let Ok(ptr) = TlData::ensure_mut(data) {
        let data_ref = &*ptr;
        if !data_ref.fields.is_null() && data_ref.len > 0 {
            let fields = Vec::from_raw_parts(data_ref.fields, data_ref.len, data_ref.len);
            drop(fields);
        }
        drop(Box::from_raw(ptr));
    }
}
