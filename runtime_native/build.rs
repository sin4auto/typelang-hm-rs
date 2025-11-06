// パス: runtime_native/build.rs
// 役割: ビルド時に辞書自動生成ファイルを出力ディレクトリへ配置する
// 意図: 環境変数経由のカスタム辞書があれば反映し、無ければフォールバックをコピーする
// 関連ファイル: runtime_native/src/dict_fallback.rs, runtime_native/src/dict.rs

use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let target = out_dir.join("dict_autogen.rs");
    println!("cargo:rerun-if-env-changed=TYPELANG_DICT_AUTOGEN");
    if let Some(source) = env::var_os("TYPELANG_DICT_AUTOGEN") {
        let source_path = PathBuf::from(&source);
        println!("cargo:rerun-if-changed={}", source_path.display());
        fs::copy(source_path, &target).expect("failed to copy dictionary source");
    } else {
        let fallback =
            PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("src/dict_fallback.rs");
        println!("cargo:rerun-if-changed={}", fallback.display());
        fs::copy(fallback, &target).expect("failed to copy fallback dictionary");
    }
}
