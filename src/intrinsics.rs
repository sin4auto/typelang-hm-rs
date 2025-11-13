// パス: src/intrinsics.rs
// 役割: 言語レベルのビルトイン関数（intrinsic）のメタデータを集約し、
//       Core IR やコード生成側が共通参照できるようにする
// 意図: `println` などソース上では関数として見えるが実体はランタイムシンボルに
//       マッピングされる機能を中央集権的に管理する

/// ネイティブコード生成時に特別扱いする intrinsic の種類。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IntrinsicKind {
    Println,
}

/// intrinsic のメタデータ。
#[derive(Clone, Copy, Debug)]
pub struct Intrinsic {
    pub name: &'static str,
    pub kind: IntrinsicKind,
}

const INTRINSICS: &[Intrinsic] = &[Intrinsic {
    name: "println",
    kind: IntrinsicKind::Println,
}];

/// 名前から intrinsic を検索するユーティリティ。
pub fn lookup(name: &str) -> Option<Intrinsic> {
    INTRINSICS.iter().copied().find(|intr| intr.name == name)
}
