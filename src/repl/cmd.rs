// パス: src/repl/cmd.rs
// 役割: REPL command loop, command parsing, and evaluation orchestration
// 意図: Drive interactive usage by coordinating type and value environments
// 関連ファイル: src/infer.rs, src/evaluator.rs, src/repl/util.rs
//! TypeLang REPL におけるコマンド処理と状態遷移を担当するモジュール。
//! 利用者の入力をコマンドや式として解釈し、型推論と評価パイプラインへ橋渡しする。

use crate::ast as A;
use crate::evaluator::{initial_env as value_env_init, Value};
use crate::infer::{initial_class_env, initial_env as type_env_init};
use crate::parser::{parse_expr, parse_program};
use crate::typesys::{generalize, pretty_qual};

use std::io::{self, Write};

use super::line_editor::{LineEditor, ReadResult};
use super::loader::load_program_into_env;
use super::pipeline::{
    eval_expr_for_pipeline, fallback_qual_from_value, fallback_scheme_from_value, infer_qual_type,
};
use super::printer::{render_help, write_value};
use super::util::normalize_expr;

/// TypeLang の対話セッションを開始し、ユーザー入力を処理し続ける。
///
/// # Examples
/// ```no_run
/// # fn main() {
/// typelang::repl::run_repl();
/// # }
/// ```
pub fn run_repl() {
    let mut editor = LineEditor::new();
    let fs = FsIo;
    let mut stdout = io::stdout();
    let mut stderr = io::stderr();
    if let Err(err) = run_repl_with(&mut editor, &fs, &mut stdout, &mut stderr) {
        let _ = writeln!(stderr, "REPL 実行中にエラーが発生しました: {}", err);
    }
}

pub(crate) trait ReplLineSource {
    fn read_line(&mut self, prompt: &str) -> io::Result<ReadResult>;
    fn add_history(&mut self, entry: &str);
    fn save_history(&mut self) -> io::Result<()>;
}

impl ReplLineSource for LineEditor {
    fn read_line(&mut self, prompt: &str) -> io::Result<ReadResult> {
        LineEditor::read_line(self, prompt)
    }

    fn add_history(&mut self, entry: &str) {
        LineEditor::add_history(self, entry);
    }

    fn save_history(&mut self) -> io::Result<()> {
        LineEditor::save_history(self)
    }
}

fn run_repl_with<S, I, W, E>(
    editor: &mut S,
    file_io: &I,
    out: &mut W,
    err: &mut E,
) -> io::Result<()>
where
    S: ReplLineSource,
    I: ReplIo,
    W: Write,
    E: Write,
{
    writeln!(
        out,
        "TypeLang REPL (Rust) :: :t EXPR で型 :: :help でヘルプ"
    )?;
    let mut session = ReplSession::with_defaults();
    let mut buffer = String::new();

    'repl: loop {
        buffer.clear();
        let mut prompt = "> ";
        let mut first_line = true;
        let input = loop {
            match editor.read_line(prompt) {
                Ok(ReadResult::Line(line)) => {
                    buffer.push_str(&line);
                    buffer.push('\n');
                    if needs_more_input(&buffer) {
                        prompt = ".. ";
                        first_line = false;
                        continue;
                    }
                    break buffer.trim().to_string();
                }
                Ok(ReadResult::Eof) => {
                    if first_line && buffer.trim().is_empty() {
                        writeln!(out)?;
                        break 'repl;
                    }
                    break buffer.trim().to_string();
                }
                Ok(ReadResult::Interrupted) => {
                    continue 'repl;
                }
                Err(e) => {
                    writeln!(err, "入力エラー: {}", e)?;
                    break 'repl;
                }
            }
        };

        let input = input.trim();
        if input.is_empty() {
            continue;
        }

        editor.add_history(input);

        match parse_repl_command(input) {
            ReplCommand::Help => {
                render_help(out)?;
                continue;
            }
            ReplCommand::Quit => break,
            other => {
                let msgs = handle_command(&mut session, other, file_io);
                dispatch_messages(msgs, out, err)?;
            }
        }
    }

    if let Err(e) = editor.save_history() {
        writeln!(err, "ヒストリーの保存に失敗しました: {}", e)?;
    }

    Ok(())
}

fn dispatch_messages<W: Write, E: Write>(
    msgs: Vec<ReplMsg>,
    out: &mut W,
    err: &mut E,
) -> io::Result<()> {
    for msg in msgs {
        match msg {
            ReplMsg::Out(s) => writeln!(out, "{}", s)?,
            ReplMsg::Err(s) => writeln!(err, "{}", s)?,
            ReplMsg::Value(v) => write_value(out, &v)?,
        }
    }
    Ok(())
}

// 括弧や文字列リテラルの開放状態をざっくり検知して多行入力を判断する。
/// ソース文字列が追加の入力行を要求するかどうかを判定する。
fn needs_more_input(src: &str) -> bool {
    // コマンド行は常に単行扱いなので継続入力を抑止する。
    let s = src.trim_start();
    if s.starts_with(':') {
        return false;
    }
    let mut paren = 0i32;
    let mut bracket = 0i32;
    let mut in_str = false;
    let mut in_chr = false;
    let mut esc = false;
    for ch in src.chars() {
        if in_str {
            if esc {
                esc = false;
                continue;
            }
            match ch {
                '\\' => esc = true,
                '"' => in_str = false,
                _ => {}
            }
            continue;
        }
        if in_chr {
            if esc {
                esc = false;
                continue;
            }
            match ch {
                '\\' => esc = true,
                '\'' => in_chr = false,
                _ => {}
            }
            continue;
        }
        match ch {
            '(' => paren += 1,
            ')' => paren -= 1,
            '[' => bracket += 1,
            ']' => bracket -= 1,
            '"' => in_str = true,
            '\'' => in_chr = true,
            _ => {}
        }
    }
    paren > 0 || bracket > 0 || in_str || in_chr
}

#[derive(Clone)]
/// REPL の型・クラス・値環境をまとめて保持するセッション管理構造体。
pub(crate) struct ReplSession {
    pub type_env: crate::typesys::TypeEnv,
    pub class_env: crate::typesys::ClassEnv,
    pub value_env: crate::evaluator::Env,
    pub last_loaded_paths: Vec<String>,
    pub defaulting_on: bool,
}

impl ReplSession {
    /// 既定の初期状態でセッションを構築する。
    pub(crate) fn with_defaults() -> Self {
        Self::new(type_env_init(), initial_class_env(), value_env_init())
    }

    /// 既存の環境を引き継いでセッションを構築する。
    pub(crate) fn new(
        type_env: crate::typesys::TypeEnv,
        class_env: crate::typesys::ClassEnv,
        value_env: crate::evaluator::Env,
    ) -> Self {
        Self {
            type_env,
            class_env,
            value_env,
            last_loaded_paths: Vec::new(),
            defaulting_on: false,
        }
    }

    /// 解釈済みコマンドを実行し、出力メッセージを返す。
    pub(crate) fn execute<I: ReplIo>(&mut self, cmd: ReplCommand, io: &I) -> Vec<ReplMsg> {
        use ReplCommand::*;
        match cmd {
            TypeOf(src) => self.exec_type_of(&src),
            Let(src) => self.exec_let(&src),
            Load(path) => self.exec_load(&path, io),
            Reload => self.exec_reload(io),
            Browse(prefix) => self.exec_browse(prefix),
            SetDefault(on) => self.exec_set_default(on),
            Unset(name) => self.exec_unset(&name),
            Eval(src) => self.exec_eval(&src),
            Help | Quit => Vec::new(),
            Invalid(s) => vec![ReplMsg::Err(format!(
                "エラー: コマンド形式が不正です: {}",
                s
            ))],
        }
    }

    fn exec_type_of(&mut self, src: &str) -> Vec<ReplMsg> {
        match parse_expr(src) {
            Ok(expr) => match type_string_in_current_env(
                &self.type_env,
                &self.class_env,
                &expr,
                self.defaulting_on,
                &mut self.value_env,
            ) {
                Ok(s) => vec![ReplMsg::Out(format!("-- {}", s))],
                Err(msg) => vec![ReplMsg::Err(msg)],
            },
            Err(e) => vec![ReplMsg::Err(format!("{}", e))],
        }
    }

    fn exec_let(&mut self, src: &str) -> Vec<ReplMsg> {
        match self.parse_program_text(src) {
            Ok(prog) => match self.apply_program(&prog) {
                Ok(loaded) => {
                    if loaded.is_empty() {
                        Vec::new()
                    } else {
                        vec![ReplMsg::Out(format!("Defined {}", loaded.join(", ")))]
                    }
                }
                Err(msg) => vec![ReplMsg::Err(msg)],
            },
            Err(err) => vec![ReplMsg::Err(err)],
        }
    }

    fn exec_load<I: ReplIo>(&mut self, path: &str, io: &I) -> Vec<ReplMsg> {
        match self.read_and_apply_path(path, io) {
            Ok(loaded) => {
                let mut msgs = vec![ReplMsg::Out(format!(
                    "Loaded {} def(s) from {}",
                    loaded.len(),
                    path
                ))];
                self.append_signature_summaries(&loaded, &mut msgs);
                self.record_load_path(path);
                msgs
            }
            Err(err) => vec![ReplMsg::Err(err)],
        }
    }

    fn exec_reload<I: ReplIo>(&mut self, io: &I) -> Vec<ReplMsg> {
        if self.last_loaded_paths.is_empty() {
            return vec![ReplMsg::Err("エラー: 直近の :load がありません".into())];
        }

        let mut msgs = Vec::new();
        for path in self.last_loaded_paths.clone() {
            match self.read_and_apply_path(&path, io) {
                Ok(loaded) => msgs.push(ReplMsg::Out(format!(
                    "Reloaded {} def(s) from {}",
                    loaded.len(),
                    path
                ))),
                Err(err) => msgs.push(ReplMsg::Err(err)),
            }
        }
        msgs
    }

    fn exec_browse(&self, prefix: Option<String>) -> Vec<ReplMsg> {
        let p = prefix.unwrap_or_default();
        let mut names: Vec<&String> = self
            .type_env
            .env
            .keys()
            .filter(|n| n.starts_with(&p))
            .collect();
        names.sort();
        if names.is_empty() {
            return vec![ReplMsg::Out("(定義なし)".into())];
        }
        names
            .into_iter()
            .map(|n| {
                if let Some(sch) = self.type_env.lookup(n) {
                    ReplMsg::Out(format!("  {} :: {}", n, pretty_qual(&sch.qual)))
                } else {
                    ReplMsg::Out(format!("  {}", n))
                }
            })
            .collect()
    }

    fn exec_set_default(&mut self, on: bool) -> Vec<ReplMsg> {
        self.defaulting_on = on;
        vec![ReplMsg::Out(format!(
            "set default = {}",
            if on { "on" } else { "off" }
        ))]
    }

    fn exec_unset(&mut self, name: &str) -> Vec<ReplMsg> {
        let mut removed = false;
        if self.type_env.env.remove(name).is_some() {
            removed = true;
        }
        if self.value_env.remove(name).is_some() {
            removed = true;
        }
        if removed {
            vec![ReplMsg::Out(format!("Unset {}", name))]
        } else {
            vec![ReplMsg::Err(format!("エラー: 未定義です: {}", name))]
        }
    }

    fn exec_eval(&mut self, src: &str) -> Vec<ReplMsg> {
        match parse_expr(src) {
            Ok(expr) => match infer_and_generalize_for_repl(
                &self.type_env,
                &self.class_env,
                &expr,
                self.defaulting_on,
                &mut self.value_env,
            ) {
                Ok((sch, val)) => {
                    self.type_env.extend("it", sch);
                    self.value_env.insert("it".into(), val.clone());
                    vec![ReplMsg::Value(val)]
                }
                Err(msg) => vec![ReplMsg::Err(msg)],
            },
            Err(e) => vec![ReplMsg::Err(format!("{}", e))],
        }
    }

    fn read_and_apply_path<I: ReplIo>(
        &mut self,
        path: &str,
        io: &I,
    ) -> Result<Vec<String>, String> {
        let src = io.read_to_string(path)?;
        self.apply_program_from_source(&src)
    }

    fn apply_program_from_source(&mut self, src: &str) -> Result<Vec<String>, String> {
        let prog = self.parse_program_text(src)?;
        self.apply_program(&prog)
    }

    fn parse_program_text(&self, src: &str) -> Result<A::Program, String> {
        parse_program(src).map_err(|e| format!("{}", e))
    }

    fn apply_program(&mut self, prog: &A::Program) -> Result<Vec<String>, String> {
        load_program_into_env(
            prog,
            &mut self.type_env,
            &self.class_env,
            &mut self.value_env,
        )
    }

    fn append_signature_summaries(&self, names: &[String], msgs: &mut Vec<ReplMsg>) {
        for name in names {
            if let Some(sch) = self.type_env.lookup(name) {
                msgs.push(ReplMsg::Out(format!(
                    "  {} :: {}",
                    name,
                    pretty_qual(&sch.qual)
                )));
            }
        }
    }

    fn record_load_path(&mut self, path: &str) {
        if !self.last_loaded_paths.iter().any(|p| p == path) {
            self.last_loaded_paths.push(path.to_string());
        }
    }
}

/// 対話セッションがユーザーへ返す応答メッセージのカテゴリ。
pub(crate) enum ReplMsg {
    Out(String),
    Err(String),
    Value(Value),
}

/// REPL に必要な最小限のファイル読み込み抽象。
pub(crate) trait ReplIo {
    /// 指定されたパスのソースコードを文字列として取得する。
    fn read_to_string(&self, path: &str) -> Result<String, String>;
}

/// 実際のファイルシステムにアクセスする標準実装。
pub(crate) struct FsIo;
impl ReplIo for FsIo {
    /// ファイルシステムからテキストを読み出し、I/O エラーを文字列に変換する。
    fn read_to_string(&self, path: &str) -> Result<String, String> {
        std::fs::read_to_string(path).map_err(|e| format!("エラー: ファイルを開けません: {}", e))
    }
}

/// 解釈済みの REPL コマンドを適用し、状態と出力メッセージを更新する。
pub(crate) fn handle_command<I: ReplIo>(
    session: &mut ReplSession,
    cmd: ReplCommand,
    io: &I,
) -> Vec<ReplMsg> {
    session.execute(cmd, io)
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
/// REPL が解釈できるトップレベルコマンドの集合。
pub(crate) enum ReplCommand {
    /// `:help` / `:h` でヘルプメッセージを表示する。
    Help,
    /// `:quit` / `:q` でセッションを終了する。
    Quit,
    /// `:t` / `:type` で式の推論結果を照会する。
    TypeOf(String),
    /// `:let` のペイロードを正規化済みソースとして保持する。
    Let(String),
    /// `:load` によるファイル読込コマンド。
    Load(String),
    /// `:reload` で直近ロードしたファイル群を再評価する。
    Reload,
    /// `:browse` の接頭辞フィルタを含むコマンド。
    Browse(Option<String>),
    /// `:set default on|off` による defaulting 設定。
    SetDefault(bool),
    /// `:unset name` で定義を破棄する。
    Unset(String),
    /// 既知のコマンドに該当しない入力を通常式として扱う。
    Eval(String),
    /// シンタックスが認識できなかったコマンド入力。
    Invalid(String),
}

#[cfg_attr(not(test), allow(dead_code))]
/// 生の入力文字列を `ReplCommand` 列挙に解析する。
pub(crate) fn parse_repl_command(input: &str) -> ReplCommand {
    let s = input.trim();
    if s.is_empty() {
        return ReplCommand::Eval(String::new());
    }
    match s {
        ":help" | ":h" => return ReplCommand::Help,
        ":quit" | ":q" => return ReplCommand::Quit,
        _ => {}
    }
    if let Some(rest) = s.strip_prefix(":t ") {
        return ReplCommand::TypeOf(rest.trim().to_string());
    }
    if let Some(rest) = s.strip_prefix(":type ") {
        return ReplCommand::TypeOf(rest.trim().to_string());
    }
    if let Some(rest) = s.strip_prefix(":let ") {
        return ReplCommand::Let(normalize_let_payload(rest.trim()));
    }
    if let Some(rest) = s.strip_prefix(":load ") {
        return ReplCommand::Load(rest.trim().to_string());
    }
    if s == ":reload" {
        return ReplCommand::Reload;
    }
    if let Some(rest) = s.strip_prefix(":browse") {
        let pfx = rest.trim();
        return if pfx.is_empty() {
            ReplCommand::Browse(None)
        } else {
            ReplCommand::Browse(Some(pfx.to_string()))
        };
    }
    if let Some(rest) = s.strip_prefix(":set ") {
        let parts: Vec<&str> = rest.split_whitespace().collect();
        if parts.len() == 2 && parts[0] == "default" {
            return match parts[1] {
                "on" => ReplCommand::SetDefault(true),
                "off" => ReplCommand::SetDefault(false),
                _ => ReplCommand::Invalid(s.to_string()),
            };
        }
        return ReplCommand::Invalid(s.to_string());
    }
    if let Some(rest) = s.strip_prefix(":unset ") {
        let name = rest.trim();
        if name.is_empty() {
            return ReplCommand::Invalid(s.to_string());
        }
        return ReplCommand::Unset(name.to_string());
    }
    if s.starts_with("let ") {
        if parse_expr(s).is_ok() {
            return ReplCommand::Eval(s.to_string());
        }
        return ReplCommand::Let(normalize_let_payload(s));
    }
    ReplCommand::Eval(s.to_string())
}

/// `:let` に与えられた定義群を均一な `let` 形式へ整形する。
/// 1 行または `;` 区切りの複数行定義を、REPL で解釈しやすいテキストに揃える。
pub(crate) fn normalize_let_payload(payload: &str) -> String {
    if payload.contains(';') {
        let parts: Vec<String> = payload
            .split(';')
            .map(|seg| {
                let s = seg.trim();
                if s.is_empty() {
                    String::new()
                } else if s.starts_with("let ") || s.contains("::") {
                    s.to_string()
                } else {
                    format!("let {}", s)
                }
            })
            .collect();
        parts.join(";\n")
    } else {
        // 単独行の定義なら `let` が無ければ補う。
        let has_prefix_sig = payload
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect::<String>()
            + " ::"
            == payload;
        let keep = payload.starts_with("let ") || payload.contains("::") || has_prefix_sig;
        if keep {
            payload.to_string()
        } else {
            format!("let {}", payload)
        }
    }
}
// :t 応答のために推論・defaulting・評価フォールバックをまとめ上げる。
/// 型環境とクラス環境を用いて `:t` の表示用文字列を導出する。
fn type_string_in_current_env(
    type_env: &crate::typesys::TypeEnv,
    class_env: &crate::typesys::ClassEnv,
    expr: &A::Expr,
    defaulting_on: bool,
    value_env: &mut crate::evaluator::Env,
) -> Result<String, String> {
    let normalized = normalize_expr(expr);
    match infer_qual_type(type_env, class_env, &normalized, defaulting_on) {
        Ok(q) => Ok(pretty_qual(&q)),
        Err(_) => {
            let value =
                eval_expr_for_pipeline(&normalized, value_env).map_err(|e| e.to_string())?;
            let qt = fallback_qual_from_value(&value);
            Ok(pretty_qual(&qt))
        }
    }
}

// REPL が `it` バインディングを更新するための推論 + 評価経路。
/// 式を評価して `it` を再定義し、同時に一般化済みの型情報を返す。
fn infer_and_generalize_for_repl(
    type_env: &crate::typesys::TypeEnv,
    class_env: &crate::typesys::ClassEnv,
    expr: &A::Expr,
    defaulting_on: bool,
    value_env: &mut crate::evaluator::Env,
) -> Result<(crate::typesys::Scheme, Value), String> {
    let normalized = normalize_expr(expr);
    match infer_qual_type(type_env, class_env, &normalized, defaulting_on) {
        Ok(q) => {
            let scheme = generalize(type_env, q);
            let value =
                eval_expr_for_pipeline(&normalized, value_env).map_err(|e| e.to_string())?;
            Ok((scheme, value))
        }
        Err(_) => {
            let value =
                eval_expr_for_pipeline(&normalized, value_env).map_err(|e| e.to_string())?;
            let scheme = fallback_scheme_from_value(type_env, &value);
            Ok((scheme, value))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::ReadResult;
    use super::{
        handle_command, needs_more_input, normalize_let_payload, parse_repl_command, run_repl_with,
        ReplCommand, ReplIo, ReplLineSource, ReplMsg, ReplSession,
    };
    use crate::typesys::{qualify, Scheme, TCon, Type, TypeEnv};
    use crate::{evaluator, infer};

    #[test]
    /// 未閉じの括弧や角括弧で継続入力が必要かを確認する。
    fn needs_more_input_balancing_paren_bracket() {
        assert!(needs_more_input("(1 + 2"));
        assert!(!needs_more_input("(1 + 2)"));
        assert!(needs_more_input("[1, 2"));
        assert!(!needs_more_input("[1, 2]"));
    }

    #[test]
    /// 文字列と文字リテラルの閉じ忘れを検知できるか検証する。
    fn needs_more_input_strings_and_chars() {
        assert!(needs_more_input("\"abc"));
        assert!(!needs_more_input("\"abc\""));
        assert!(needs_more_input("'a"));
        assert!(!needs_more_input("'a'"));
        // エスケープ済みで閉じているケースは継続扱いにならない。
        assert!(!needs_more_input("\"a\\\"b\""));
        assert!(!needs_more_input("'\\''"));
    }

    #[test]
    /// コマンド行が常に単独で確定することを確かめる。
    fn needs_more_input_commands_do_not_continue() {
        // 先頭が ':' の行は常に単行で確定させる。
        assert!(!needs_more_input(":t ("));
        assert!(!needs_more_input(":load file.tl"));
    }

    #[test]
    /// `:let` の正規化が入力パターンごとに期待通り働くかを確認する。
    fn normalize_let_payload_single_and_multi() {
        assert_eq!(normalize_let_payload("f x = x"), "let f x = x");
        assert_eq!(
            normalize_let_payload("let f x = x; g y = y"),
            "let f x = x;\nlet g y = y"
        );
        assert_eq!(normalize_let_payload("id :: a -> a"), "id :: a -> a");
    }

    #[test]
    /// 代表的なコマンドが想定した `ReplCommand` に分類されるかを確認する。
    fn parse_repl_command_variants() {
        assert_eq!(parse_repl_command(":help"), ReplCommand::Help);
        assert_eq!(parse_repl_command(":q"), ReplCommand::Quit);
        assert_eq!(
            parse_repl_command(":t 1 + 2"),
            ReplCommand::TypeOf("1 + 2".into())
        );
        match parse_repl_command(":let f x = x") {
            ReplCommand::Let(src) => assert!(src.starts_with("let f")),
            other => panic!("unexpected: {:?}", other),
        }
        assert_eq!(
            parse_repl_command(":load examples/basics.tl"),
            ReplCommand::Load("examples/basics.tl".into())
        );
        assert_eq!(parse_repl_command(":reload"), ReplCommand::Reload);
        assert_eq!(
            parse_repl_command(":browse foo"),
            ReplCommand::Browse(Some("foo".into()))
        );
        assert_eq!(
            parse_repl_command(":set default on"),
            ReplCommand::SetDefault(true)
        );
        assert_eq!(
            parse_repl_command(":unset x"),
            ReplCommand::Unset("x".into())
        );
        match parse_repl_command("let id x = x") {
            ReplCommand::Let(src) => assert!(src.starts_with("let id")),
            other => panic!("unexpected: {:?}", other),
        }
        assert_eq!(
            parse_repl_command("1 + 2"),
            ReplCommand::Eval("1 + 2".into())
        );
    }

    #[test]
    /// 異常な `:set` 入力が `Invalid` へ落ちることを保証する。
    fn parse_repl_command_invalid_variants() {
        match parse_repl_command(":set default maybe") {
            ReplCommand::Invalid(src) => assert_eq!(src, ":set default maybe"),
            other => panic!("expected invalid, got {:?}", other),
        }
        match parse_repl_command(":set other on") {
            ReplCommand::Invalid(src) => assert_eq!(src, ":set other on"),
            other => panic!("expected invalid, got {:?}", other),
        }
    }

    /// ファイルアクセスを発生させないテスト専用のダミー I/O 実装。
    struct NoopIo;
    impl ReplIo for NoopIo {
        /// どのパスに対しても失敗を返す。
        fn read_to_string(&self, _path: &str) -> Result<String, String> {
            Err("unexpected io".into())
        }
    }

    /// テスト実行時に利用する空の REPL 状態を生成する。
    fn mk_state() -> ReplSession {
        ReplSession::new(
            TypeEnv::new(),
            infer::initial_class_env(),
            evaluator::initial_env(),
        )
    }

    #[test]
    /// `:browse`・`:set default`・`:unset` の挙動をまとめて検証する。
    fn handle_browse_and_set_default_and_unset() {
        let mut state = mk_state();
        // テスト用に 2 つの定義を型環境へ追加する。
        let sch = Scheme {
            vars: vec![],
            qual: qualify(Type::TCon(TCon { name: "Int".into() }), vec![]),
        };
        state.type_env.extend("foo", sch.clone());
        state.type_env.extend("bar", sch);

        // プレフィックスなしの :browse を確認する。
        let msgs = handle_command(&mut state, ReplCommand::Browse(None), &NoopIo);
        let outs: Vec<String> = msgs
            .into_iter()
            .filter_map(|m| match m {
                ReplMsg::Out(s) => Some(s),
                _ => None,
            })
            .collect();
        assert!(outs.iter().any(|s| s.contains("  bar :: Int")));
        assert!(outs.iter().any(|s| s.contains("  foo :: Int")));

        // プレフィックス付き :browse のフィルタ挙動を検証する。
        let msgs = handle_command(&mut state, ReplCommand::Browse(Some("fo".into())), &NoopIo);
        let outs: Vec<String> = msgs
            .into_iter()
            .filter_map(|m| match m {
                ReplMsg::Out(s) => Some(s),
                _ => None,
            })
            .collect();
        assert_eq!(outs.len(), 1);
        assert!(outs[0].contains("foo :: Int"));

        // defaulting を有効化する。
        let msgs = handle_command(&mut state, ReplCommand::SetDefault(true), &NoopIo);
        assert!(matches!(msgs[0], ReplMsg::Out(ref s) if s.contains("set default = on")));
        assert!(state.defaulting_on);

        // 既存定義を削除できることを確認する。
        let msgs = handle_command(&mut state, ReplCommand::Unset("foo".into()), &NoopIo);
        assert!(matches!(msgs[0], ReplMsg::Out(ref s) if s.contains("Unset foo")));
        assert!(state.type_env.lookup("foo").is_none());
        // 同じ定義を再度削除しようとするとエラーになる。
        let msgs = handle_command(&mut state, ReplCommand::Unset("foo".into()), &NoopIo);
        assert!(matches!(msgs[0], ReplMsg::Err(ref s) if s.contains("未定義")));
    }

    /// 事前に登録したレスポンスを返すテスト用モック I/O。
    struct MapIo(std::collections::HashMap<String, Result<String, String>>);
    impl ReplIo for MapIo {
        /// マップに登録されたレスポンスをそのまま返す。
        fn read_to_string(&self, path: &str) -> Result<String, String> {
            self.0
                .get(path)
                .cloned()
                .unwrap_or_else(|| Err("not found".into()))
        }
    }

    #[derive(Default)]
    struct ScriptedLineSource {
        events: std::collections::VecDeque<ScriptEvent>,
        history: Vec<String>,
        saved: bool,
    }

    impl ScriptedLineSource {
        fn new(events: impl IntoIterator<Item = ScriptEvent>) -> Self {
            Self {
                events: events.into_iter().collect(),
                history: Vec::new(),
                saved: false,
            }
        }
    }

    enum ScriptEvent {
        Line(&'static str),
        Eof,
    }

    impl ReplLineSource for ScriptedLineSource {
        fn read_line(&mut self, _prompt: &str) -> io::Result<ReadResult> {
            match self.events.pop_front().unwrap_or(ScriptEvent::Eof) {
                ScriptEvent::Line(s) => Ok(ReadResult::Line(s.to_string())),
                ScriptEvent::Eof => Ok(ReadResult::Eof),
            }
        }

        fn add_history(&mut self, entry: &str) {
            self.history.push(entry.to_string());
        }

        fn save_history(&mut self) -> io::Result<()> {
            self.saved = true;
            Ok(())
        }
    }

    fn first_err(msgs: Vec<ReplMsg>) -> Option<String> {
        msgs.into_iter().find_map(|m| match m {
            ReplMsg::Err(s) => Some(s),
            _ => None,
        })
    }

    #[test]
    /// `:load` と `:reload` の成功パスで状態が更新されるか検証する。
    fn handle_load_success_and_reload_paths() {
        let mut state = mk_state();
        let prog = "let x = 1;".to_string();
        let mut map = std::collections::HashMap::new();
        map.insert("mem://ok".into(), Ok(prog));
        let io = MapIo(map);
        let msgs = handle_command(&mut state, ReplCommand::Load("mem://ok".into()), &io);
        // ロード件数と型一覧がレスポンスに含まれることを確認する。
        let outs: Vec<String> = msgs
            .into_iter()
            .filter_map(|m| match m {
                ReplMsg::Out(s) => Some(s),
                _ => None,
            })
            .collect();
        assert!(outs
            .iter()
            .any(|s| s.contains("Loaded 1 def(s) from mem://ok")));
        assert!(outs.iter().any(|s| s.starts_with("  x ::")));
        assert!(state.last_loaded_paths.contains(&"mem://ok".to_string()));

        // :reload でも同様のメッセージが得られる。
        let msgs = handle_command(&mut state, ReplCommand::Reload, &io);
        let outs: Vec<String> = msgs
            .into_iter()
            .filter_map(|m| match m {
                ReplMsg::Out(s) => Some(s),
                _ => None,
            })
            .collect();
        assert!(outs
            .iter()
            .any(|s| s.contains("Reloaded 1 def(s) from mem://ok")));
    }

    #[test]
    /// 読み込み失敗時と履歴未設定時の `:reload` エラーを確認する。
    fn handle_load_error_and_reload_without_history() {
        let mut state = mk_state();
        // 未登録パスを指定すると読み込みが失敗する。
        let io = MapIo(std::collections::HashMap::new());
        let msgs = handle_command(&mut state, ReplCommand::Load("mem://missing".into()), &io);
        assert!(msgs.iter().any(|m| matches!(m, ReplMsg::Err(_))));

        // ロード履歴が空の状態で :reload を試す。
        let msgs = handle_command(&mut state, ReplCommand::Reload, &io);
        assert!(msgs
            .iter()
            .any(|m| matches!(m, ReplMsg::Err(s) if s.contains("直近の :load"))));
    }

    #[test]
    /// `:reload` が I/O 失敗を取りこぼさず伝搬することを検証する。
    fn handle_reload_propagates_io_error() {
        let mut state = mk_state();
        state.last_loaded_paths.push("mem://missing".into());
        let io = MapIo(std::collections::HashMap::new());
        let msgs = handle_command(&mut state, ReplCommand::Reload, &io);
        let err = first_err(msgs).expect("error response expected");
        assert!(err.contains("not found"));
    }

    #[test]
    /// `:t` のエラーパス (構文エラー / 推論・評価エラー) を網羅する。
    fn handle_typeof_error_paths() {
        let mut state = mk_state();
        let parse_err = handle_command(&mut state, ReplCommand::TypeOf("(1 +".into()), &NoopIo);
        assert!(
            first_err(parse_err).unwrap().contains("PAR"),
            "parser error expected"
        );

        let infer_err = handle_command(&mut state, ReplCommand::TypeOf("missing".into()), &NoopIo);
        let msg = first_err(infer_err).unwrap();
        assert!(msg.contains("未束縛変数"));
    }

    #[test]
    /// `:let` や `:load` の失敗が適切にエラーメッセージを返すことを確認する。
    fn handle_let_and_load_error_paths() {
        let mut state = mk_state();
        let msgs = handle_command(&mut state, ReplCommand::Let("let".into()), &NoopIo);
        assert!(
            first_err(msgs).unwrap().contains("PAR"),
            "parse failure expected"
        );

        let mut map = std::collections::HashMap::new();
        map.insert("mem://bad".into(), Ok("let".into()));
        let io = MapIo(map);
        let msgs = handle_command(&mut state, ReplCommand::Load("mem://bad".into()), &io);
        assert!(
            first_err(msgs).unwrap().contains("PAR"),
            "load parse failure expected"
        );
    }

    #[test]
    /// 評価系コマンドのエラーブランチ (構文・実行時) を網羅する。
    fn handle_eval_error_paths() {
        let mut state = mk_state();
        let parse_err = handle_command(&mut state, ReplCommand::Eval("let".into()), &NoopIo);
        assert!(
            first_err(parse_err).unwrap().contains("PAR"),
            "parse error expected"
        );

        let runtime_err = handle_command(&mut state, ReplCommand::Eval("missing".into()), &NoopIo);
        assert!(first_err(runtime_err).unwrap().contains("未束縛変数"));
    }

    #[test]
    /// `Invalid` コマンドが標準化されたエラーメッセージを返すことを検証する。
    fn handle_invalid_command_variant() {
        let mut state = mk_state();
        let msgs = handle_command(&mut state, ReplCommand::Invalid("???".into()), &NoopIo);
        let err = first_err(msgs).unwrap();
        assert!(err.contains("コマンド形式が不正"));
    }

    #[test]
    /// 空入力が空文字列評価コマンドとして扱われることを確認する。
    fn parse_repl_command_empty_string_is_eval() {
        assert_eq!(parse_repl_command(""), ReplCommand::Eval(String::new()));
    }

    #[test]
    /// `:t`・式評価・`:let` が一連のフローとして機能するか確かめる。
    fn handle_typeof_eval_and_let_flow() {
        let mut state = mk_state();
        let msgs = handle_command(&mut state, ReplCommand::TypeOf("1".into()), &NoopIo);
        assert!(matches!(msgs.first(), Some(ReplMsg::Out(s)) if s.starts_with("-- ")));

        let msgs = handle_command(&mut state, ReplCommand::Eval("1 + 1".into()), &NoopIo);
        assert!(matches!(msgs.first(), Some(ReplMsg::Value(_))));
        assert!(state.value_env.contains_key("it"));
        assert!(state.type_env.lookup("it").is_some());

        let msgs = handle_command(&mut state, ReplCommand::Let("let two = 2".into()), &NoopIo);
        assert!(msgs
            .iter()
            .any(|m| matches!(m, ReplMsg::Out(s) if s.contains("Defined two"))));
        assert!(state.value_env.contains_key("two"));
        assert!(state.type_env.lookup("two").is_some());
    }

    #[test]
    /// スクリプト駆動で REPL ループ全体を通し、入出力が記録されることを確認する。
    fn run_repl_with_script_executes_commands() {
        let events = vec![
            ScriptEvent::Line(":help"),
            ScriptEvent::Line(":set default on"),
            ScriptEvent::Line("(1 +"),
            ScriptEvent::Line("2)"),
            ScriptEvent::Line(":browse"),
            ScriptEvent::Line(":quit"),
            ScriptEvent::Eof,
        ];
        let mut script = ScriptedLineSource::new(events);
        let io = NoopIo;
        let mut out = Vec::new();
        let mut err = Vec::new();
        run_repl_with(&mut script, &io, &mut out, &mut err).unwrap();

        let stdout = String::from_utf8(out).unwrap();
        assert!(stdout.contains("TypeLang REPL (Rust)"));
        assert!(stdout.contains("利用可能なコマンド"));
        assert!(stdout.contains("set default = on"));
        assert!(stdout.contains("  it ::"));
        assert!(stdout.contains("3\n"));
        assert!(script.saved);
        assert!(script.history.iter().any(|h| h.contains(":set default on")));
        assert!(err.is_empty());
    }
}
