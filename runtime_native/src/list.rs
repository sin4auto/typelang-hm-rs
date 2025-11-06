// パス: runtime_native/src/list.rs
// 役割: リスト (TlListNode) の構造体定義と操作関数を提供する
// 意図: TypeLang のリスト値をネイティブランタイムで生成・走査できるようにする
// 関連ファイル: runtime_native/src/value.rs, runtime_native/src/data.rs

use crate::error::{set_last_error, TlRuntimeError};
use crate::value::TlValue;

#[repr(C)]
pub struct TlListNode {
    tag: u8,
    head: TlValue,
    tail: *mut TlListNode,
}

impl TlListNode {
    const EMPTY_TAG: u8 = 0;
    const CONS_TAG: u8 = 1;

    fn empty() -> Self {
        Self {
            tag: Self::EMPTY_TAG,
            head: TlValue::null(),
            tail: std::ptr::null_mut(),
        }
    }

    fn new_cons(head: TlValue, tail: *mut TlListNode) -> Self {
        Self {
            tag: Self::CONS_TAG,
            head,
            tail,
        }
    }

    fn is_empty(&self) -> bool {
        self.tag == Self::EMPTY_TAG
    }
}

#[no_mangle]
pub extern "C" fn tl_list_empty() -> *mut TlListNode {
    Box::into_raw(Box::new(TlListNode::empty()))
}

#[no_mangle]
pub extern "C" fn tl_list_cons(head: TlValue, tail: *mut TlListNode) -> *mut TlListNode {
    Box::into_raw(Box::new(TlListNode::new_cons(head, tail)))
}

#[no_mangle]
pub unsafe extern "C" fn tl_list_is_empty(list: *const TlListNode) -> bool {
    list.as_ref().is_none_or(TlListNode::is_empty)
}

#[no_mangle]
pub unsafe extern "C" fn tl_list_head(list: *const TlListNode) -> TlValue {
    match list.as_ref() {
        Some(node) if !node.is_empty() => node.head,
        _ => {
            set_last_error(TlRuntimeError::InvalidArgument("empty list").status());
            TlValue::null()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn tl_list_tail(list: *const TlListNode) -> *mut TlListNode {
    match list.as_ref() {
        Some(node) if !node.is_empty() => node.tail,
        _ => {
            set_last_error(TlRuntimeError::InvalidArgument("empty list").status());
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn tl_list_free(mut list: *mut TlListNode) {
    while let Some(node) = list.as_mut() {
        let tail = node.tail;
        drop(Box::from_raw(list));
        list = tail;
    }
}
