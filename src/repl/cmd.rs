// パス: src/repl/cmd.rs
// 役割: REPL command loop, command parsing, and evaluation orchestration
// 意図: Drive interactive usage by coordinating type and value environments
// 関連ファイル: src/infer.rs, src/evaluator.rs, src/repl/util.rs
//! TypeLang REPL におけるコマンド処理と状態遷移を担当するモジュール。
//! 利用者の入力をコマンドや式として解釈し、型推論と評価パイプラインへ橋渡しする。

use crate::ast as A;
use crate::evaluator::{eval_expr, initial_env as value_env_init, Value};
use crate::infer::{infer_expr, initial_class_env, initial_env as type_env_init, InferState};
use crate::parser::{parse_expr, parse_program};
use crate::typesys::{
    apply_defaulting_simple, generalize, pretty_qual, qualify, Substitutable, TCon, TVarSupply,
    Type,
};

use super::line_editor::{LineEditor, ReadResult};
use super::loader::load_program_into_env;
use super::printer::{print_help, print_value};
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
    println!("TypeLang REPL (Rust) :: :t EXPR で型 :: :help でヘルプ");
    let mut type_env = type_env_init();
    let class_env = initial_class_env();
    let mut value_env = value_env_init();
    let mut last_loaded_paths: Vec<String> = Vec::new();

    let mut editor = LineEditor::new();
    let mut buffer = String::new();
    let mut defaulting_on = false;
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
                        println!();
                        break 'repl;
                    }
                    break buffer.trim().to_string();
                }
                Ok(ReadResult::Interrupted) => {
                    continue 'repl;
                }
                Err(err) => {
                    eprintln!("入力エラー: {}", err);
                    break 'repl;
                }
            }
        };

        let input = input.trim();
        if input.is_empty() {
            continue;
        }

        editor.add_history(input);

        // コマンド解釈と副作用の実行を明示的に分離する。
        match parse_repl_command(input) {
            ReplCommand::Help => {
                print_help();
                continue;
            }
            ReplCommand::Quit => break,
            other => {
                let mut state = ReplState {
                    type_env,
                    class_env: class_env.clone(),
                    value_env,
                    last_loaded_paths: last_loaded_paths.clone(),
                    defaulting_on,
                };
                let fs = FsIo;
                let msgs = handle_command(&mut state, other, &fs);
                // 実行結果を既存のセッション状態へ反映する。
                type_env = state.type_env;
                value_env = state.value_env;
                last_loaded_paths = state.last_loaded_paths;
                defaulting_on = state.defaulting_on;
                for m in msgs {
                    match m {
                        ReplMsg::Out(s) => println!("{}", s),
                        ReplMsg::Err(s) => eprintln!("{}", s),
                        ReplMsg::Value(v) => print_value(&v),
                    }
                }
            }
        }
    }

    if let Err(err) = editor.save_history() {
        eprintln!("ヒストリーの保存に失敗しました: {}", err);
    }
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
/// 型・クラス・値環境を束ねて保持する REPL セッションのスナップショット。
pub(crate) struct ReplState {
    pub type_env: crate::typesys::TypeEnv,
    pub class_env: crate::typesys::ClassEnv,
    pub value_env: crate::evaluator::Env,
    pub last_loaded_paths: Vec<String>,
    pub defaulting_on: bool,
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
    state: &mut ReplState,
    cmd: ReplCommand,
    io: &I,
) -> Vec<ReplMsg> {
    use ReplCommand::*;
    match cmd {
        TypeOf(src) => handle_type_of(state, &src),
        Let(src) => handle_let(state, &src),
        Load(path) => handle_load(state, &path, io),
        Reload => handle_reload(state, io),
        Browse(prefix) => handle_browse(state, prefix),
        SetDefault(on) => handle_set_default(state, on),
        Unset(name) => handle_unset(state, &name),
        Eval(src) => handle_eval(state, &src),
        Help | Quit => Vec::new(),
        Invalid(s) => vec![ReplMsg::Err(format!(
            "エラー: コマンド形式が不正です: {}",
            s
        ))],
    }
}

fn handle_type_of(state: &mut ReplState, src: &str) -> Vec<ReplMsg> {
    match parse_expr(src) {
        Ok(expr) => match type_string_in_current_env(
            &state.type_env,
            &state.class_env,
            &expr,
            state.defaulting_on,
            &mut state.value_env,
        ) {
            Ok(s) => vec![ReplMsg::Out(format!("-- {}", s))],
            Err(msg) => vec![ReplMsg::Err(msg)],
        },
        Err(e) => vec![ReplMsg::Err(format!("{}", e))],
    }
}

fn handle_let(state: &mut ReplState, src: &str) -> Vec<ReplMsg> {
    match parse_program(src) {
        Ok(prog) => match apply_program(state, &prog) {
            Ok(loaded) => {
                if loaded.is_empty() {
                    Vec::new()
                } else {
                    vec![ReplMsg::Out(format!("Defined {}", loaded.join(", ")))]
                }
            }
            Err(msg) => vec![ReplMsg::Err(msg)],
        },
        Err(e) => vec![ReplMsg::Err(format!("{}", e))],
    }
}

fn handle_load<I: ReplIo>(state: &mut ReplState, path: &str, io: &I) -> Vec<ReplMsg> {
    match io.read_to_string(path) {
        Ok(src) => match parse_program(&src) {
            Ok(prog) => match apply_program(state, &prog) {
                Ok(loaded) => {
                    let mut msgs = vec![ReplMsg::Out(format!(
                        "Loaded {} def(s) from {}",
                        loaded.len(),
                        path
                    ))];
                    append_signature_summaries(state, &loaded, &mut msgs);
                    if !state.last_loaded_paths.iter().any(|p| p == path) {
                        state.last_loaded_paths.push(path.to_string());
                    }
                    msgs
                }
                Err(msg) => vec![ReplMsg::Err(msg)],
            },
            Err(e) => vec![ReplMsg::Err(format!("{}", e))],
        },
        Err(e) => vec![ReplMsg::Err(e)],
    }
}

fn handle_reload<I: ReplIo>(state: &mut ReplState, io: &I) -> Vec<ReplMsg> {
    if state.last_loaded_paths.is_empty() {
        return vec![ReplMsg::Err("エラー: 直近の :load がありません".into())];
    }

    let mut msgs = Vec::new();
    let paths = state.last_loaded_paths.clone();
    for path in paths {
        match io.read_to_string(&path) {
            Ok(src) => match parse_program(&src) {
                Ok(prog) => match apply_program(state, &prog) {
                    Ok(loaded) => msgs.push(ReplMsg::Out(format!(
                        "Reloaded {} def(s) from {}",
                        loaded.len(),
                        path
                    ))),
                    Err(msg) => msgs.push(ReplMsg::Err(msg)),
                },
                Err(e) => msgs.push(ReplMsg::Err(format!("{}", e))),
            },
            Err(e) => msgs.push(ReplMsg::Err(e)),
        }
    }
    msgs
}

fn handle_browse(state: &ReplState, prefix: Option<String>) -> Vec<ReplMsg> {
    let p = prefix.unwrap_or_default();
    let mut names: Vec<&String> = state
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
            if let Some(sch) = state.type_env.lookup(n) {
                ReplMsg::Out(format!("  {} :: {}", n, pretty_qual(&sch.qual)))
            } else {
                ReplMsg::Out(format!("  {}", n))
            }
        })
        .collect()
}

fn handle_set_default(state: &mut ReplState, on: bool) -> Vec<ReplMsg> {
    state.defaulting_on = on;
    vec![ReplMsg::Out(format!(
        "set default = {}",
        if on { "on" } else { "off" }
    ))]
}

fn handle_unset(state: &mut ReplState, name: &str) -> Vec<ReplMsg> {
    let mut removed = false;
    if state.type_env.env.remove(name).is_some() {
        removed = true;
    }
    if state.value_env.remove(name).is_some() {
        removed = true;
    }
    if removed {
        vec![ReplMsg::Out(format!("Unset {}", name))]
    } else {
        vec![ReplMsg::Err(format!("エラー: 未定義です: {}", name))]
    }
}

fn handle_eval(state: &mut ReplState, src: &str) -> Vec<ReplMsg> {
    match parse_expr(src) {
        Ok(expr) => match infer_and_generalize_for_repl(
            &state.type_env,
            &state.class_env,
            &expr,
            state.defaulting_on,
            &mut state.value_env,
        ) {
            Ok((sch, val)) => {
                state.type_env.extend("it", sch);
                state.value_env.insert("it".into(), val.clone());
                vec![ReplMsg::Value(val)]
            }
            Err(msg) => vec![ReplMsg::Err(msg)],
        },
        Err(e) => vec![ReplMsg::Err(format!("{}", e))],
    }
}

fn append_signature_summaries(state: &ReplState, names: &[String], msgs: &mut Vec<ReplMsg>) {
    for name in names {
        if let Some(sch) = state.type_env.lookup(name) {
            msgs.push(ReplMsg::Out(format!(
                "  {} :: {}",
                name,
                pretty_qual(&sch.qual)
            )));
        }
    }
}

fn apply_program(state: &mut ReplState, prog: &A::Program) -> Result<Vec<String>, String> {
    load_program_into_env(
        prog,
        &mut state.type_env,
        &state.class_env,
        &mut state.value_env,
    )
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
    let mut st = InferState {
        supply: TVarSupply::new(),
        subst: Default::default(),
    };
    let e = normalize_expr(expr);
    match infer_expr(type_env, class_env, &mut st, &e) {
        Ok((s, q)) => {
            let mut q2 = q.apply_subst(&s);
            if defaulting_on {
                q2 = apply_defaulting_simple(&q2);
            }
            Ok(pretty_qual(&q2))
        }
        Err(_) => {
            // 推論に失敗したら値を評価して代表的な型名へフォールバックする。
            let v = eval_expr(&e, value_env).map_err(|e| e.to_string())?;
            let name = match v {
                Value::Int(_) => "Int",
                Value::Double(_) => "Double",
                Value::Bool(_) => "Bool",
                Value::Char(_) => "Char",
                Value::String(_) => "[Char]",
                _ => "()",
            };
            let qt = qualify(Type::TCon(TCon { name: name.into() }), vec![]);
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
    let e = normalize_expr(expr);
    let mut st = InferState {
        supply: TVarSupply::new(),
        subst: Default::default(),
    };
    match infer_expr(type_env, class_env, &mut st, &e) {
        Ok((s, q)) => {
            let mut q2 = q.apply_subst(&s);
            if defaulting_on {
                q2 = apply_defaulting_simple(&q2);
            }
            let sch = generalize(type_env, q2);
            let v = eval_expr(&e, value_env).map_err(|e| e.to_string())?;
            Ok((sch, v))
        }
        Err(_) => {
            let v = eval_expr(&e, value_env).map_err(|e| e.to_string())?;
            let tname = match v {
                Value::Int(_) => "Int",
                Value::Double(_) => "Double",
                Value::Bool(_) => "Bool",
                Value::Char(_) => "Char",
                Value::String(_) => "[Char]",
                _ => "()",
            };
            let q = qualify(Type::TCon(TCon { name: tname.into() }), vec![]);
            let sch = generalize(type_env, q);
            Ok((sch, v))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        handle_command, needs_more_input, normalize_let_payload, parse_repl_command, ReplCommand,
        ReplIo, ReplMsg, ReplState,
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
    fn mk_state() -> ReplState {
        ReplState {
            type_env: TypeEnv::new(),
            class_env: infer::initial_class_env(),
            value_env: evaluator::initial_env(),
            last_loaded_paths: vec![],
            defaulting_on: false,
        }
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
}
