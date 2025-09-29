// パス: src/repl/printer.rs
// 役割: Helpers for rendering REPL help and value output
// 意図: Keep interactive messaging consistent across commands
// 関連ファイル: src/repl/cmd.rs, src/evaluator.rs, src/repl/util.rs
//! REPL で用いるヘルプメッセージと値出力を集約したモジュール。
//! 表示形式を一箇所にまとめ、対話時の出力を統一する。

use crate::evaluator::Value;
/// 利用可能な REPL コマンド一覧を標準出力へ表示する。
pub(crate) fn print_help() {
    println!("利用可能なコマンド:");
    println!("  :help              ヘルプ（本メッセージ）");
    println!("  :t EXPR            型を表示");
    println!("  :type EXPR         :t と同じ");
    println!("  :let DEF[; ...]    その場で定義（複数は ; 区切り）");
    println!("  :load PATH         ファイルからロード");
    println!("  :reload            直近ロードしたファイルを再読み込み");
    println!("  :browse [PFX]      定義一覧（接頭辞フィルタ）");
    println!("  :unset NAME        定義を削除");
    println!("  :set default on|off 型表示の defaulting を切替");
    println!("  :quit              終了");
    println!();
    println!("例:");
    println!("  > :let square x = x * x");
    println!("  > square 12             -- 144 を表示");
    println!(r"  > :t \\x -> x ** 2      -- Fractional a => a -> a");
    println!("  > 1 + 2                 -- 3 を表示、直近結果は it で参照可");
    println!("  > it * 10               -- 30");
}
/// 評価結果を REPL 向けのフォーマットで出力する。
pub(crate) fn print_value(v: &Value) {
    match v {
        Value::String(s) => println!("\"{}\"", s),
        Value::Int(i) => println!("{}", i),
        Value::Double(d) => println!("{}", d),
        Value::Bool(b) => println!("{}", if *b { "True" } else { "False" }),
        Value::Char(c) => println!("'{}'", c),
        other => println!("{:?}", other),
    }
}

#[cfg(test)]
mod tests {
    use super::print_value;
    use crate::evaluator::Value;

    #[test]
    /// 代表的な値種別が問題なくフォーマットされることを確認する。
    fn print_value_variants_execute() {
        // 厳密な文字列比較ではなく分岐ごとの実行確認に留める。
        print_value(&Value::String("s".into()));
        print_value(&Value::Int(1));
        print_value(&Value::Double(1.5));
        print_value(&Value::Bool(true));
        print_value(&Value::Char('x'));
        print_value(&Value::List(vec![Value::Int(1), Value::Int(2)]));
        print_value(&Value::Tuple(vec![Value::Int(1), Value::Bool(false)]));
    }
}
