// パス: runtime_native/src/dict.rs
// 役割: 型クラス辞書の構築・検索・解放を行うランタイム ABI を提供する
// 意図: 自動生成された辞書初期化コードと Cranelift 生成コードを連携させる
// 関連ファイル: runtime_native/src/dict_fallback.rs, src/codegen/dictionary_codegen.rs

use crate::error::{set_last_error, TlStatus};
use crate::value::TlValue;
use std::ffi::{c_char, CStr, CString};

#[repr(C)]
pub struct TlDictEntry {
    pub name: *mut c_char,
    pub method_id: u64,
    pub signature: *mut c_char,
    pub value: TlValue,
}

struct TlDictEntryOwned {
    name: CString,
    signature: CString,
    method_id: u64,
    value: TlValue,
}

#[repr(C)]
pub struct TlDictionary {
    classname: *mut c_char,
    entries: *mut TlDictEntry,
    len: usize,
}

pub struct TlDictBuilder {
    classname: CString,
    entries: Vec<TlDictEntryOwned>,
}

#[no_mangle]
pub unsafe extern "C" fn tl_dict_builder_new(classname: *const c_char) -> *mut TlDictBuilder {
    if classname.is_null() {
        set_last_error(TlStatus::NullPointer);
        return std::ptr::null_mut();
    }
    let c_name = CStr::from_ptr(classname).to_owned();
    let builder = TlDictBuilder {
        classname: c_name,
        entries: Vec::new(),
    };
    set_last_error(TlStatus::Ok);
    Box::into_raw(Box::new(builder))
}

#[no_mangle]
pub unsafe extern "C" fn tl_dict_builder_push(
    builder: *mut TlDictBuilder,
    name: *const c_char,
    value: TlValue,
) {
    tl_dict_builder_push_ext(builder, name, 0, std::ptr::null(), value);
}

#[no_mangle]
pub unsafe extern "C" fn tl_dict_builder_push_ext(
    builder: *mut TlDictBuilder,
    name: *const c_char,
    method_id: u64,
    signature: *const c_char,
    value: TlValue,
) {
    let Some(builder) = builder.as_mut() else {
        set_last_error(TlStatus::NullPointer);
        return;
    };
    let name_str = CStr::from_ptr(name).to_owned();
    let signature_owned = if signature.is_null() {
        CString::new("?").unwrap()
    } else {
        CStr::from_ptr(signature).to_owned()
    };
    builder.entries.push(TlDictEntryOwned {
        name: name_str,
        signature: signature_owned,
        method_id,
        value,
    });
    set_last_error(TlStatus::Ok);
}

#[no_mangle]
pub unsafe extern "C" fn tl_dict_builder_finish(builder: *mut TlDictBuilder) -> *mut TlDictionary {
    let Some(builder) = builder.as_mut() else {
        set_last_error(TlStatus::NullPointer);
        return std::ptr::null_mut();
    };

    let len = builder.entries.len();
    let mut raw_entries: Vec<TlDictEntry> = Vec::with_capacity(len);
    for owned in builder.entries.drain(..) {
        raw_entries.push(TlDictEntry {
            name: owned.name.into_raw(),
            method_id: owned.method_id,
            signature: owned.signature.into_raw(),
            value: owned.value,
        });
    }
    let entries_ptr = if len == 0 {
        std::ptr::null_mut()
    } else {
        let ptr = raw_entries.as_mut_ptr();
        std::mem::forget(raw_entries);
        ptr
    };

    let dict = TlDictionary {
        classname: builder.classname.clone().into_raw(),
        entries: entries_ptr,
        len,
    };

    set_last_error(TlStatus::Ok);
    Box::into_raw(Box::new(dict))
}

#[no_mangle]
pub unsafe extern "C" fn tl_dict_builder_dispose(builder: *mut TlDictBuilder) {
    if builder.is_null() {
        return;
    }
    let builder_box = Box::from_raw(builder);
    drop(builder_box);
}

#[no_mangle]
pub unsafe extern "C" fn tl_dict_lookup(dict: *const TlDictionary, method_id: u64) -> TlValue {
    let Some(dict) = dict.as_ref() else {
        set_last_error(TlStatus::NullPointer);
        return TlValue::null();
    };

    let entries = std::slice::from_raw_parts(dict.entries, dict.len);
    for entry in entries {
        if entry.method_id == method_id {
            set_last_error(TlStatus::Ok);
            return entry.value;
        }
    }
    set_last_error(TlStatus::InvalidArgument);
    TlValue::null()
}

#[no_mangle]
pub unsafe extern "C" fn tl_dict_free(dict: *mut TlDictionary) {
    if dict.is_null() {
        return;
    }
    let dict_box = Box::from_raw(dict);
    let classname_ptr = dict_box.classname;
    let entries_ptr = dict_box.entries;
    let len = dict_box.len;
    drop(dict_box);

    if !classname_ptr.is_null() {
        let _ = CString::from_raw(classname_ptr);
    }
    if !entries_ptr.is_null() {
        let entries = Vec::from_raw_parts(entries_ptr, len, len);
        for entry in entries {
            if !entry.name.is_null() {
                let _ = CString::from_raw(entry.name);
            }
            if !entry.signature.is_null() {
                let _ = CString::from_raw(entry.signature);
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn tl_num_int_add(lhs: i64, rhs: i64) -> i64 {
    lhs.saturating_add(rhs)
}

#[no_mangle]
pub extern "C" fn tl_num_int_sub(lhs: i64, rhs: i64) -> i64 {
    lhs.saturating_sub(rhs)
}

#[no_mangle]
pub extern "C" fn tl_num_int_mul(lhs: i64, rhs: i64) -> i64 {
    lhs.saturating_mul(rhs)
}

#[no_mangle]
pub extern "C" fn tl_num_int_from_int(value: i64) -> i64 {
    value
}

#[no_mangle]
pub extern "C" fn tl_num_double_add(lhs: f64, rhs: f64) -> f64 {
    lhs + rhs
}

#[no_mangle]
pub extern "C" fn tl_num_double_sub(lhs: f64, rhs: f64) -> f64 {
    lhs - rhs
}

#[no_mangle]
pub extern "C" fn tl_num_double_mul(lhs: f64, rhs: f64) -> f64 {
    lhs * rhs
}

#[no_mangle]
pub extern "C" fn tl_num_double_from_int(value: i64) -> f64 {
    value as f64
}

#[no_mangle]
pub extern "C" fn tl_fractional_double_div(lhs: f64, rhs: f64) -> f64 {
    lhs / rhs
}

#[no_mangle]
pub extern "C" fn tl_integral_int_div(lhs: i64, rhs: i64) -> i64 {
    lhs.checked_div(rhs).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn tl_integral_int_mod(lhs: i64, rhs: i64) -> i64 {
    if rhs == 0 {
        0
    } else {
        lhs % rhs
    }
}

#[no_mangle]
pub extern "C" fn tl_eq_int(lhs: i64, rhs: i64) -> i8 {
    (lhs == rhs) as i8
}

#[no_mangle]
pub extern "C" fn tl_neq_int(lhs: i64, rhs: i64) -> i8 {
    (lhs != rhs) as i8
}

#[no_mangle]
pub extern "C" fn tl_eq_double(lhs: f64, rhs: f64) -> i8 {
    (lhs == rhs) as i8
}

#[no_mangle]
pub extern "C" fn tl_neq_double(lhs: f64, rhs: f64) -> i8 {
    (lhs != rhs) as i8
}

#[no_mangle]
pub extern "C" fn tl_eq_bool(lhs: i8, rhs: i8) -> i8 {
    ((lhs != 0) == (rhs != 0)) as i8
}

#[no_mangle]
pub extern "C" fn tl_neq_bool(lhs: i8, rhs: i8) -> i8 {
    ((lhs != 0) != (rhs != 0)) as i8
}

#[no_mangle]
pub extern "C" fn tl_ord_int_lt(lhs: i64, rhs: i64) -> i8 {
    (lhs < rhs) as i8
}

#[no_mangle]
pub extern "C" fn tl_ord_int_le(lhs: i64, rhs: i64) -> i8 {
    (lhs <= rhs) as i8
}

#[no_mangle]
pub extern "C" fn tl_ord_int_gt(lhs: i64, rhs: i64) -> i8 {
    (lhs > rhs) as i8
}

#[no_mangle]
pub extern "C" fn tl_ord_int_ge(lhs: i64, rhs: i64) -> i8 {
    (lhs >= rhs) as i8
}

#[no_mangle]
pub extern "C" fn tl_ord_double_lt(lhs: f64, rhs: f64) -> i8 {
    (lhs < rhs) as i8
}

#[no_mangle]
pub extern "C" fn tl_ord_double_le(lhs: f64, rhs: f64) -> i8 {
    (lhs <= rhs) as i8
}

#[no_mangle]
pub extern "C" fn tl_ord_double_gt(lhs: f64, rhs: f64) -> i8 {
    (lhs > rhs) as i8
}

#[no_mangle]
pub extern "C" fn tl_ord_double_ge(lhs: f64, rhs: f64) -> i8 {
    (lhs >= rhs) as i8
}

#[no_mangle]
pub extern "C" fn tl_bool_logic_and(lhs: i8, rhs: i8) -> i8 {
    ((lhs != 0) && (rhs != 0)) as i8
}

#[no_mangle]
pub extern "C" fn tl_bool_logic_or(lhs: i8, rhs: i8) -> i8 {
    ((lhs != 0) || (rhs != 0)) as i8
}

#[no_mangle]
pub extern "C" fn tl_bool_logic_not(value: i8) -> i8 {
    (value == 0) as i8
}
