// パス: src/repl/printer.rs
// 役割: Helpers for rendering REPL help and value output
// 意図: Keep interactive messaging consistent across commands
// 関連ファイル: src/repl/cmd.rs, src/evaluator.rs, src/repl/util.rs
//! REPL で用いるヘルプメッセージと値出力を集約したモジュール。
//! 表示形式を一箇所にまとめ、対話時の出力を統一する。

use crate::evaluator::Value;
use std::io::{self, Write};

const HELP_TEXT: &str = concat!(
    "利用可能なコマンド:\n",
    "  :help              ヘルプ（本メッセージ）\n",
    "  :t EXPR            型を表示\n",
    "  :type EXPR         :t と同じ\n",
    "  :let DEF[; ...]    その場で定義（複数は ; 区切り）\n",
    "  :load PATH         ファイルからロード\n",
    "  :reload            直近ロードしたファイルを再読み込み\n",
    "  :browse [PFX]      定義一覧（接頭辞フィルタ）\n",
    "  :unset NAME        定義を削除\n",
    "  :set default on|off 型表示の defaulting を切替\n",
    "  :quit              終了\n",
    "\n",
    "例:\n",
    "  > :let square x = x * x\n",
    "  > square 12             -- 144 を表示\n",
    "  > :t \\x -> x ** 2      -- Fractional a => a -> a\n",
    "  > 1 + 2                 -- 3 を表示、直近結果は it で参照可\n",
    "  > it * 10               -- 30\n",
);
/// 利用可能な REPL コマンド一覧を標準出力へ表示する。
#[allow(dead_code)]
pub(crate) fn print_help() {
    let mut out = io::stdout();
    let _ = render_help(&mut out);
}
/// 評価結果を REPL 向けのフォーマットで出力する。
#[allow(dead_code)]
pub(crate) fn print_value(v: &Value) {
    let mut out = io::stdout();
    let _ = write_value(&mut out, v);
}

/// ヘルプメッセージを任意のライターへ描画する。
pub(crate) fn render_help<W: Write>(out: &mut W) -> io::Result<()> {
    out.write_all(HELP_TEXT.as_bytes())
}

/// 値出力を任意のライターへ書き出す。
pub(crate) fn write_value<W: Write>(out: &mut W, v: &Value) -> io::Result<()> {
    match v {
        Value::String(s) => writeln!(out, "\"{}\"", s),
        Value::Int(i) => writeln!(out, "{}", i),
        Value::Double(d) => writeln!(out, "{}", d),
        Value::Bool(b) => writeln!(out, "{}", if *b { "True" } else { "False" }),
        Value::Char(c) => writeln!(out, "'{}'", c),
        other => writeln!(out, "{:?}", other),
    }
}

#[cfg(test)]
mod tests {
    use super::{render_help, write_value};
    use crate::evaluator::Value;

    fn write_to_string(v: &Value) -> String {
        let mut buf = Vec::new();
        write_value(&mut buf, v).unwrap();
        String::from_utf8(buf).unwrap()
    }

    #[test]
    /// ヘルプメッセージが定義済みテンプレートどおり出力されるか検証する。
    fn render_help_outputs_expected_text() {
        let mut buf = Vec::new();
        render_help(&mut buf).unwrap();
        let rendered = String::from_utf8(buf).unwrap();
        assert_eq!(rendered, super::HELP_TEXT);
    }

    #[test]
    /// 代表的な値が期待通りのフォーマットで出力されるか検証する。
    fn write_value_variants_render_expected_strings() {
        let v = Value::String("s".into());
        assert_eq!(write_to_string(&v), "\"s\"\n");

        let v = Value::Int(1);
        assert_eq!(write_to_string(&v), "1\n");

        let v = Value::Double(1.5);
        assert_eq!(write_to_string(&v), "1.5\n");

        let v = Value::Bool(true);
        assert_eq!(write_to_string(&v), "True\n");

        let v = Value::Bool(false);
        assert_eq!(write_to_string(&v), "False\n");

        let v = Value::Char('x');
        assert_eq!(write_to_string(&v), "'x'\n");

        let v = Value::List(vec![Value::Int(1), Value::Int(2)]);
        assert_eq!(write_to_string(&v), format!("{:?}\n", &v));

        let v = Value::Tuple(vec![Value::Int(1), Value::Bool(false)]);
        assert_eq!(write_to_string(&v), format!("{:?}\n", &v));
    }
}
