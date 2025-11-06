// パス: runtime_native/src/dict_fallback.rs
// 役割: 型クラス辞書のフォールバック実装と組み込みメソッド解決を提供する
// 意図: 自動生成された辞書が存在しない場合でもネイティブバックエンドを動作させる
// 関連ファイル: runtime_native/build.rs, runtime_native/src/dict.rs

use std::ffi::{c_void, CString};

struct FallbackMethod {
    method_id: u64,
    name: &'static str,
    signature: &'static str,
    symbol: *mut c_void,
}

unsafe fn build_dictionary(label: &str, methods: &[FallbackMethod]) -> *mut crate::TlDictionary {
    let label_c = CString::new(label).unwrap();
    let builder = crate::tl_dict_builder_new(label_c.as_ptr());
    if builder.is_null() {
        return std::ptr::null_mut();
    }

    for method in methods {
        let name_c = CString::new(method.name).unwrap();
        let signature_c = CString::new(method.signature).unwrap();
        let raw_name = name_c.into_raw();
        let raw_signature = signature_c.into_raw();
        let fn_value = crate::tl_value_from_ptr(method.symbol);
        if fn_value.as_raw().is_null() {
            return std::ptr::null_mut();
        }
        crate::tl_dict_builder_push_ext(
            builder,
            raw_name,
            method.method_id,
            raw_signature,
            fn_value,
        );
        let _ = CString::from_raw(raw_name);
        let _ = CString::from_raw(raw_signature);
    }

    let dict = crate::tl_dict_builder_finish(builder);
    crate::tl_dict_builder_dispose(builder);
    dict
}

macro_rules! fallback_methods {
    ($($id:expr => ($name:literal, $sig:literal, $symbol:expr)),+ $(,)?) => {
        &[ $(FallbackMethod {
            method_id: $id,
            name: $name,
            signature: $sig,
            symbol: $symbol as *mut c_void,
        }),+ ]
    };
}

const NUM_INT_METHODS: &[FallbackMethod] = fallback_methods![
    0 => ("add", "Int -> Int -> Int", tl_num_int_add),
    1 => ("sub", "Int -> Int -> Int", tl_num_int_sub),
    2 => ("mul", "Int -> Int -> Int", tl_num_int_mul),
    3 => ("fromInt", "Int -> Int", tl_num_int_from_int),
];

const NUM_DOUBLE_METHODS: &[FallbackMethod] = fallback_methods![
    0 => ("add", "Double -> Double -> Double", tl_num_double_add),
    1 => ("sub", "Double -> Double -> Double", tl_num_double_sub),
    2 => ("mul", "Double -> Double -> Double", tl_num_double_mul),
    3 => ("fromInt", "Int -> Double", tl_num_double_from_int),
];

const FRACTIONAL_DOUBLE_METHODS: &[FallbackMethod] = fallback_methods![
    0 => ("div", "Double -> Double -> Double", tl_fractional_double_div),
];

const INTEGRAL_INT_METHODS: &[FallbackMethod] = fallback_methods![
    0 => ("div", "Int -> Int -> Int", tl_integral_int_div),
    1 => ("mod", "Int -> Int -> Int", tl_integral_int_mod),
];

const EQ_INT_METHODS: &[FallbackMethod] = fallback_methods![
    0 => ("eq", "Int -> Int -> Bool", tl_eq_int),
    1 => ("neq", "Int -> Int -> Bool", tl_neq_int),
];

const EQ_DOUBLE_METHODS: &[FallbackMethod] = fallback_methods![
    0 => ("eq", "Double -> Double -> Bool", tl_eq_double),
    1 => ("neq", "Double -> Double -> Bool", tl_neq_double),
];

const EQ_BOOL_METHODS: &[FallbackMethod] = fallback_methods![
    0 => ("eq", "Bool -> Bool -> Bool", tl_eq_bool),
    1 => ("neq", "Bool -> Bool -> Bool", tl_neq_bool),
];

const ORD_INT_METHODS: &[FallbackMethod] = fallback_methods![
    0 => ("lt", "Int -> Int -> Bool", tl_ord_int_lt),
    1 => ("le", "Int -> Int -> Bool", tl_ord_int_le),
    2 => ("gt", "Int -> Int -> Bool", tl_ord_int_gt),
    3 => ("ge", "Int -> Int -> Bool", tl_ord_int_ge),
];

const ORD_DOUBLE_METHODS: &[FallbackMethod] = fallback_methods![
    0 => ("lt", "Double -> Double -> Bool", tl_ord_double_lt),
    1 => ("le", "Double -> Double -> Bool", tl_ord_double_le),
    2 => ("gt", "Double -> Double -> Bool", tl_ord_double_gt),
    3 => ("ge", "Double -> Double -> Bool", tl_ord_double_ge),
];

const BOOL_LOGIC_METHODS: &[FallbackMethod] = fallback_methods![
    0 => ("and", "Bool -> Bool -> Bool", tl_bool_logic_and),
    1 => ("or", "Bool -> Bool -> Bool", tl_bool_logic_or),
    2 => ("not", "Bool -> Bool", tl_bool_logic_not),
];

#[no_mangle]
pub extern "C" fn tl_dict_build_Num_Int() -> *mut crate::TlDictionary {
    unsafe { build_dictionary("Num[Int]", NUM_INT_METHODS) }
}

#[no_mangle]
pub extern "C" fn tl_dict_build_Num_Double() -> *mut crate::TlDictionary {
    unsafe { build_dictionary("Num[Double]", NUM_DOUBLE_METHODS) }
}

#[no_mangle]
pub extern "C" fn tl_dict_build_Fractional_Double() -> *mut crate::TlDictionary {
    unsafe { build_dictionary("Fractional[Double]", FRACTIONAL_DOUBLE_METHODS) }
}

#[no_mangle]
pub extern "C" fn tl_dict_build_Integral_Int() -> *mut crate::TlDictionary {
    unsafe { build_dictionary("Integral[Int]", INTEGRAL_INT_METHODS) }
}

#[no_mangle]
pub extern "C" fn tl_dict_build_Eq_Int() -> *mut crate::TlDictionary {
    unsafe { build_dictionary("Eq[Int]", EQ_INT_METHODS) }
}

#[no_mangle]
pub extern "C" fn tl_dict_build_Eq_Double() -> *mut crate::TlDictionary {
    unsafe { build_dictionary("Eq[Double]", EQ_DOUBLE_METHODS) }
}

#[no_mangle]
pub extern "C" fn tl_dict_build_Eq_Bool() -> *mut crate::TlDictionary {
    unsafe { build_dictionary("Eq[Bool]", EQ_BOOL_METHODS) }
}

#[no_mangle]
pub extern "C" fn tl_dict_build_Ord_Int() -> *mut crate::TlDictionary {
    unsafe { build_dictionary("Ord[Int]", ORD_INT_METHODS) }
}

#[no_mangle]
pub extern "C" fn tl_dict_build_Ord_Double() -> *mut crate::TlDictionary {
    unsafe { build_dictionary("Ord[Double]", ORD_DOUBLE_METHODS) }
}

#[no_mangle]
pub extern "C" fn tl_dict_build_BoolLogic_Bool() -> *mut crate::TlDictionary {
    unsafe { build_dictionary("BoolLogic[Bool]", BOOL_LOGIC_METHODS) }
}
