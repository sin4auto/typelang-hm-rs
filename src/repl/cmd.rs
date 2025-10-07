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
use crate::typesys::pretty_qual;

use std::io::{self, Write};

use super::line_editor::{LineEditor, ReadResult};
use super::loader::load_program_into_env;
use super::pipeline::{run_repl_pipeline, EvaluationMode};
use super::printer::{render_help, write_value};

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
            Ok(expr) => match run_repl_pipeline(
                &self.type_env,
                &self.class_env,
                &expr,
                self.defaulting_on,
                &mut self.value_env,
                EvaluationMode::OnInferenceFailure,
            ) {
                Ok(result) => vec![ReplMsg::Out(format!("-- {}", pretty_qual(&result.qual)))],
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
            Ok(expr) => match run_repl_pipeline(
                &self.type_env,
                &self.class_env,
                &expr,
                self.defaulting_on,
                &mut self.value_env,
                EvaluationMode::Always,
            ) {
                Ok(result) => {
                    let value = result
                        .value
                        .expect("pipeline with Always mode must return a value");
                    self.type_env.extend("it", result.scheme);
                    self.value_env.insert("it".into(), value.clone());
                    vec![ReplMsg::Value(value)]
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
#[cfg(test)]
mod tests {
    use super::*;
    use super::{handle_command, needs_more_input, normalize_let_payload, parse_repl_command};
    use crate::repl::printer::write_value;
    use crate::typesys::TypeEnv;
    use crate::{evaluator, infer};
    use std::collections::{HashMap, VecDeque};
    use std::io;

    struct MapIo(HashMap<String, Result<String, String>>);

    impl MapIo {
        fn new() -> Self {
            Self(HashMap::new())
        }

        fn ok(mut self, path: &str, src: &str) -> Self {
            self.0.insert(path.to_string(), Ok(src.to_string()));
            self
        }
    }

    impl ReplIo for MapIo {
        fn read_to_string(&self, path: &str) -> Result<String, String> {
            self.0
                .get(path)
                .cloned()
                .unwrap_or_else(|| Err("not found".into()))
        }
    }

    struct NoopIo;

    impl ReplIo for NoopIo {
        fn read_to_string(&self, _: &str) -> Result<String, String> {
            Err("unexpected io".into())
        }
    }

    fn mk_state() -> ReplSession {
        ReplSession::new(
            TypeEnv::new(),
            infer::initial_class_env(),
            evaluator::initial_env(),
        )
    }

    #[derive(Debug)]
    enum Expected<'a> {
        Out(&'a str),
        Err(&'a str),
        Value(&'a str),
    }

    fn assert_msgs(msgs: Vec<ReplMsg>, expected: &[Expected<'_>]) {
        assert_eq!(msgs.len(), expected.len(), "message length mismatch");
        for (msg, expect) in msgs.into_iter().zip(expected.iter()) {
            match (msg, expect) {
                (ReplMsg::Out(actual), Expected::Out(fragment)) => {
                    assert!(
                        actual.contains(fragment),
                        "expected stdout to contain `{fragment}`, got `{actual}`"
                    );
                }
                (ReplMsg::Err(actual), Expected::Err(fragment)) => {
                    assert!(
                        actual.contains(fragment),
                        "expected stderr to contain `{fragment}`, got `{actual}`"
                    );
                }
                (ReplMsg::Value(value), Expected::Value(fragment)) => {
                    let mut buf = Vec::new();
                    write_value(&mut buf, &value).expect("value serialization");
                    let rendered = String::from_utf8(buf).expect("utf8");
                    assert!(
                        rendered.contains(fragment),
                        "expected value to contain `{fragment}`, got `{rendered}`"
                    );
                }
                (other, expect) => {
                    let actual = match other {
                        ReplMsg::Out(_) => "Out",
                        ReplMsg::Err(_) => "Err",
                        ReplMsg::Value(_) => "Value",
                    };
                    panic!("mismatched variants: actual {actual}, expected {expect:?}");
                }
            }
        }
    }

    #[test]
    fn needs_more_input_cases() {
        let cases = [
            ("(1 + 2", true),
            (r#""abc"#, true),
            ("'a'", false),
            (":t (", false),
            ("let x = 1", false),
        ];
        for (src, expected) in cases {
            assert_eq!(needs_more_input(src), expected, "case `{src}`");
        }
    }

    #[test]
    fn normalize_let_payload_cases() {
        let cases = [
            ("f x = x", "let f x = x"),
            ("let g y = y", "let g y = y"),
            ("id :: a -> a", "id :: a -> a"),
            ("f x = x; g y = y", "let f x = x;\nlet g y = y"),
        ];
        for (input, expected) in cases {
            assert_eq!(normalize_let_payload(input), expected);
        }
    }

    #[test]
    fn parse_repl_command_variants() {
        let cases = [
            (":help", ReplCommand::Help),
            (":h", ReplCommand::Help),
            (":quit", ReplCommand::Quit),
            (":type 1 + 2", ReplCommand::TypeOf("1 + 2".into())),
            (":t x", ReplCommand::TypeOf("x".into())),
            (":let f x = x", ReplCommand::Let("let f x = x".into())),
            (":load file.tl", ReplCommand::Load("file.tl".into())),
            (":browse fo", ReplCommand::Browse(Some("fo".into()))),
            (":browse", ReplCommand::Browse(None)),
            (":set default on", ReplCommand::SetDefault(true)),
            (":set default off", ReplCommand::SetDefault(false)),
            (":unset foo", ReplCommand::Unset("foo".into())),
            (":reload", ReplCommand::Reload),
            ("let x = x", ReplCommand::Let("let x = x".into())),
            ("1 + 2", ReplCommand::Eval("1 + 2".into())),
        ];
        for (input, expected) in cases {
            assert_eq!(parse_repl_command(input), expected, "input `{input}`");
        }
    }

    #[test]
    fn parse_repl_command_invalid_inputs() {
        for input in [":set default maybe", ":set default", ":set other on"] {
            match parse_repl_command(input) {
                ReplCommand::Invalid(s) => assert_eq!(s, input),
                other => panic!("expected invalid for `{input}`, got {other:?}"),
            }
        }
    }

    #[test]
    fn handle_command_core_scenarios() {
        let mut state = mk_state();
        let msgs = handle_command(&mut state, ReplCommand::Let("let foo = 1".into()), &NoopIo);
        assert_msgs(msgs, &[Expected::Out("Defined foo")]);

        let browse = handle_command(&mut state, ReplCommand::Browse(None), &NoopIo);
        assert_msgs(browse, &[Expected::Out("foo ::")]);

        let set_default = handle_command(&mut state, ReplCommand::SetDefault(true), &NoopIo);
        assert_msgs(set_default, &[Expected::Out("set default = on")]);
        assert!(state.defaulting_on);

        let unset_ok = handle_command(&mut state, ReplCommand::Unset("foo".into()), &NoopIo);
        assert_msgs(unset_ok, &[Expected::Out("Unset foo")]);

        let unset_err = handle_command(&mut state, ReplCommand::Unset("foo".into()), &NoopIo);
        assert_msgs(unset_err, &[Expected::Err("未定義")]);
    }

    #[test]
    fn type_and_eval_pipeline() {
        let mut state = ReplSession::with_defaults();
        let typeof_msgs = handle_command(&mut state, ReplCommand::TypeOf("True".into()), &NoopIo);
        assert_msgs(typeof_msgs, &[Expected::Out("Bool")]);

        let typeof_err = handle_command(&mut state, ReplCommand::TypeOf("missing".into()), &NoopIo);
        assert_msgs(typeof_err, &[Expected::Err("未束縛")]);

        let eval_msgs = handle_command(&mut state, ReplCommand::Eval("1 + 1".into()), &NoopIo);
        assert_msgs(eval_msgs, &[Expected::Value("2")]);
        assert!(state.type_env.lookup("it").is_some());

        let parse_err = handle_command(&mut state, ReplCommand::TypeOf("(1 +".into()), &NoopIo);
        assert_msgs(parse_err, &[Expected::Err("[PAR")]);
    }

    #[test]
    fn load_and_reload_flow() {
        let io = MapIo::new().ok("mem://ok", "let x = 1;");
        let mut state = mk_state();

        let load = handle_command(&mut state, ReplCommand::Load("mem://ok".into()), &io);
        assert_msgs(load, &[Expected::Out("Loaded"), Expected::Out("x ::")]);

        let reload = handle_command(&mut state, ReplCommand::Reload, &io);
        assert_msgs(reload, &[Expected::Out("Reloaded")]);

        let missing = handle_command(&mut state, ReplCommand::Load("mem://missing".into()), &io);
        assert_msgs(missing, &[Expected::Err("not found")]);

        let mut fresh = mk_state();
        let reload_err = handle_command(&mut fresh, ReplCommand::Reload, &io);
        assert_msgs(reload_err, &[Expected::Err("直近の :load")]);
    }

    #[derive(Default)]
    struct ScriptedLineSource {
        events: VecDeque<ScriptEvent>,
        history: Vec<String>,
        saved: bool,
    }

    enum ScriptEvent {
        Line(&'static str),
        Eof,
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

    impl ReplLineSource for ScriptedLineSource {
        fn read_line(&mut self, _prompt: &str) -> io::Result<ReadResult> {
            match self.events.pop_front().unwrap_or(ScriptEvent::Eof) {
                ScriptEvent::Line(line) => Ok(ReadResult::Line(line.to_string())),
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

    #[test]
    fn run_repl_with_script_executes_commands() {
        let events = vec![
            ScriptEvent::Line(":set default on"),
            ScriptEvent::Line("1 + 2"),
            ScriptEvent::Line(":quit"),
            ScriptEvent::Eof,
        ];
        let mut script = ScriptedLineSource::new(events);
        let io = NoopIo;
        let mut out = Vec::new();
        let mut err = Vec::new();

        run_repl_with(&mut script, &io, &mut out, &mut err).unwrap();

        let stdout = String::from_utf8(out).expect("utf8");
        assert!(stdout.contains("TypeLang REPL"));
        assert!(stdout.contains("set default = on"));
        assert!(stdout.contains("3"));
        assert!(script.saved);
        assert!(script.history.len() >= 2);
        assert!(err.is_empty());
    }
}
