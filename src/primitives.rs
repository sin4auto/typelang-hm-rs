// パス: src/primitives.rs
// 役割: 言語プリミティブの定義メタデータを集約する
// 意図: 型環境と評価環境で同じプリミティブ一覧を共有する
// 関連ファイル: src/infer.rs, src/evaluator.rs
//! プリミティブ定義モジュール
//!
//! - 名前と分類情報を一元管理し、型推論・評価で重複列挙を防ぐ。
//! - 各モジュールは `type_spec` / `op` を利用して必要な初期化を行う。
//! - 実装ロジックは個別モジュール側に残しつつ、一覧のみ共有する。

use crate::runtime::{
    add_op, div_int_op, div_op, eq_op, ge_op, gt_op, le_op, lt_op, mod_int_op, mul_op, ne_op, powf,
    powi, py_show, quot_int_op, rem_int_op, sub_op, PrimOp,
};

/// 型推論側で利用するスキーム分類。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimitiveTypeSpec {
    BinOp { classname: &'static str },
    IntPow,
    Pred { classname: &'static str },
    Show,
    IntBin,
}

/// プリミティブ定義。
#[derive(Clone, Debug)]
pub struct PrimitiveDef {
    pub name: &'static str,
    pub type_spec: PrimitiveTypeSpec,
    pub op: PrimOp,
}

/// 言語が標準で提供するプリミティブの一覧。
pub const PRIMITIVES: &[PrimitiveDef] = &[
    PrimitiveDef {
        name: "+",
        type_spec: PrimitiveTypeSpec::BinOp { classname: "Num" },
        op: PrimOp::binary(add_op),
    },
    PrimitiveDef {
        name: "-",
        type_spec: PrimitiveTypeSpec::BinOp { classname: "Num" },
        op: PrimOp::binary(sub_op),
    },
    PrimitiveDef {
        name: "*",
        type_spec: PrimitiveTypeSpec::BinOp { classname: "Num" },
        op: PrimOp::binary(mul_op),
    },
    PrimitiveDef {
        name: "/",
        type_spec: PrimitiveTypeSpec::BinOp {
            classname: "Fractional",
        },
        op: PrimOp::binary(div_op),
    },
    PrimitiveDef {
        name: "div",
        type_spec: PrimitiveTypeSpec::IntBin,
        op: PrimOp::binary(div_int_op),
    },
    PrimitiveDef {
        name: "mod",
        type_spec: PrimitiveTypeSpec::IntBin,
        op: PrimOp::binary(mod_int_op),
    },
    PrimitiveDef {
        name: "quot",
        type_spec: PrimitiveTypeSpec::IntBin,
        op: PrimOp::binary(quot_int_op),
    },
    PrimitiveDef {
        name: "rem",
        type_spec: PrimitiveTypeSpec::IntBin,
        op: PrimOp::binary(rem_int_op),
    },
    PrimitiveDef {
        name: "^",
        type_spec: PrimitiveTypeSpec::IntPow,
        op: PrimOp::binary(powi),
    },
    PrimitiveDef {
        name: "**",
        type_spec: PrimitiveTypeSpec::BinOp {
            classname: "Fractional",
        },
        op: PrimOp::binary(powf),
    },
    PrimitiveDef {
        name: "==",
        type_spec: PrimitiveTypeSpec::Pred { classname: "Eq" },
        op: PrimOp::binary(eq_op),
    },
    PrimitiveDef {
        name: "/=",
        type_spec: PrimitiveTypeSpec::Pred { classname: "Eq" },
        op: PrimOp::binary(ne_op),
    },
    PrimitiveDef {
        name: "<",
        type_spec: PrimitiveTypeSpec::Pred { classname: "Ord" },
        op: PrimOp::binary(lt_op),
    },
    PrimitiveDef {
        name: "<=",
        type_spec: PrimitiveTypeSpec::Pred { classname: "Ord" },
        op: PrimOp::binary(le_op),
    },
    PrimitiveDef {
        name: ">",
        type_spec: PrimitiveTypeSpec::Pred { classname: "Ord" },
        op: PrimOp::binary(gt_op),
    },
    PrimitiveDef {
        name: ">=",
        type_spec: PrimitiveTypeSpec::Pred { classname: "Ord" },
        op: PrimOp::binary(ge_op),
    },
    PrimitiveDef {
        name: "show",
        type_spec: PrimitiveTypeSpec::Show,
        op: PrimOp::unary(py_show),
    },
];
