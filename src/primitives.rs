// パス: src/primitives.rs
// 役割: 言語プリミティブの定義メタデータを集約する
// 意図: 型環境と評価環境で同じプリミティブ一覧を共有する
// 関連ファイル: src/infer.rs, src/evaluator.rs
//! プリミティブ定義モジュール
//!
//! - 名前と分類情報を一元管理し、型推論・評価で重複列挙を防ぐ。
//! - 各モジュールは `PrimitiveKind` をマッチして必要な初期化を行う。
//! - 実装ロジックは個別モジュール側に残しつつ、一覧のみ共有する。

/// 数値演算子の種別。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumericOp {
    Add,
    Sub,
    Mul,
}

/// Eq 制約を持つ比較演算子の種別。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EqOp {
    Eq,
    Ne,
}

/// Ord 制約を持つ比較演算子の種別。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrdOp {
    Lt,
    Le,
    Gt,
    Ge,
}

/// プリミティブの分類。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimitiveKind {
    Numeric(NumericOp),
    FractionalDiv,
    PowInt,
    PowFloat,
    Eq(EqOp),
    Ord(OrdOp),
    Show,
}

/// プリミティブ定義。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrimitiveDef {
    pub name: &'static str,
    pub kind: PrimitiveKind,
}

/// 言語が標準で提供するプリミティブの一覧。
pub const PRIMITIVES: &[PrimitiveDef] = &[
    PrimitiveDef {
        name: "+",
        kind: PrimitiveKind::Numeric(NumericOp::Add),
    },
    PrimitiveDef {
        name: "-",
        kind: PrimitiveKind::Numeric(NumericOp::Sub),
    },
    PrimitiveDef {
        name: "*",
        kind: PrimitiveKind::Numeric(NumericOp::Mul),
    },
    PrimitiveDef {
        name: "/",
        kind: PrimitiveKind::FractionalDiv,
    },
    PrimitiveDef {
        name: "^",
        kind: PrimitiveKind::PowInt,
    },
    PrimitiveDef {
        name: "**",
        kind: PrimitiveKind::PowFloat,
    },
    PrimitiveDef {
        name: "==",
        kind: PrimitiveKind::Eq(EqOp::Eq),
    },
    PrimitiveDef {
        name: "/=",
        kind: PrimitiveKind::Eq(EqOp::Ne),
    },
    PrimitiveDef {
        name: "<",
        kind: PrimitiveKind::Ord(OrdOp::Lt),
    },
    PrimitiveDef {
        name: "<=",
        kind: PrimitiveKind::Ord(OrdOp::Le),
    },
    PrimitiveDef {
        name: ">",
        kind: PrimitiveKind::Ord(OrdOp::Gt),
    },
    PrimitiveDef {
        name: ">=",
        kind: PrimitiveKind::Ord(OrdOp::Ge),
    },
    PrimitiveDef {
        name: "show",
        kind: PrimitiveKind::Show,
    },
];
