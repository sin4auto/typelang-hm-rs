// パス: runtime_native/tests/runtime.rs
// 役割: runtime_native クレートの公開 ABI をエンドツーエンドで検証する
// 意図: 辞書構築や TlValue 変換が期待通りに機能することを保証する
// 関連ファイル: runtime_native/src/value.rs, runtime_native/src/dict.rs, tests/native_build.rs

use runtime_native::{
    tl_dict_build_BoolLogic_Bool, tl_dict_build_Eq_Int, tl_dict_build_Num_Int, tl_dict_free,
    tl_dict_lookup, tl_last_error, tl_value_from_int_result, tl_value_release, tl_value_to_int,
    tl_value_to_ptr, TlStatus,
};

#[test]
fn value_result_retains_status() {
    let result = tl_value_from_int_result(42);
    assert_eq!(result.status as i32, TlStatus::Ok as i32);
    let value = result.value;
    let roundtrip = unsafe { tl_value_to_int(value) };
    assert_eq!(roundtrip, 42);
    unsafe { tl_value_release(value) };
}

#[test]
fn dictionary_builder_supports_metadata() {
    unsafe {
        let dict = tl_dict_build_Num_Int();
        assert!(!dict.is_null());
        let add_val = tl_dict_lookup(dict, 0);
        assert!(!add_val.as_raw().is_null());
        let fn_ptr = tl_value_to_ptr(add_val);
        assert!(!fn_ptr.is_null());
        assert_eq!(tl_last_error(), TlStatus::Ok);
        let missing = tl_dict_lookup(dict, 999);
        assert!(missing.as_raw().is_null());
        assert_eq!(tl_last_error(), TlStatus::InvalidArgument);
        tl_dict_free(dict);
    }
}

#[test]
fn eq_dictionary_exposes_methods() {
    unsafe {
        let dict = tl_dict_build_Eq_Int();
        assert!(!dict.is_null());
        let eq_val = tl_dict_lookup(dict, 0);
        assert!(!eq_val.as_raw().is_null());
        assert!(!tl_value_to_ptr(eq_val).is_null());
        let neq_val = tl_dict_lookup(dict, 1);
        assert!(!neq_val.as_raw().is_null());
        tl_dict_free(dict);
    }
}

#[test]
fn bool_logic_dictionary_supports_unary_method() {
    unsafe {
        let dict = tl_dict_build_BoolLogic_Bool();
        assert!(!dict.is_null());
        let not_val = tl_dict_lookup(dict, 2);
        assert!(!not_val.as_raw().is_null());
        assert!(!tl_value_to_ptr(not_val).is_null());
        tl_dict_free(dict);
    }
}
