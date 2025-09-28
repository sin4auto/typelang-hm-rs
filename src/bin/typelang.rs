// パス: src/bin/typelang.rs
// 役割: Binary entrypoint that launches the REPL runtime
// 意図: Offer a CLI executable for interactive language exploration
// 関連ファイル: src/repl/mod.rs, src/lib.rs, src/repl/cmd.rs
fn main() {
    typelang::repl::run_repl();
}
