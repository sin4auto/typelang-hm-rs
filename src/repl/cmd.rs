// パス: src/repl/cmd.rs
// 役割: REPL command loop, command parsing, and evaluation orchestration
// 意図: Drive interactive usage by coordinating type and value environments
// 関連ファイル: src/infer.rs, src/evaluator.rs, src/repl/util.rs
//! REPL のコマンドとメインループ

use crate::ast as A;
use crate::evaluator::{eval_expr, initial_env as value_env_init, Value};
use crate::infer::{infer_expr, initial_class_env, initial_env as type_env_init, InferState};
use crate::parser::{parse_expr, parse_program};
use crate::typesys::{
    apply_defaulting_simple, apply_subst_q, generalize, pretty_qual, qualify, TCon, TVarSupply,
    Type,
};

use super::line_editor::{LineEditor, ReadResult};
use super::loader::load_program_into_env;
use super::printer::{print_help, print_value};
use super::util::normalize_expr;

/// 対話環境を起動する。
///
/// # Examples
/// ```no_run
/// fn main() {
///     // 実行例: `cargo run --bin typelang-repl`
///     // REPL 内で `:help` を入力するとコマンド一覧が表示されます。
/// }
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

        // 新しい純関数パス: コマンド解釈 → 作用
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
                // コミット
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

// 未閉括弧などの簡易判定（多行入力継続）
fn needs_more_input(src: &str) -> bool {
    // コマンドは多行にしない
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
pub(crate) struct ReplState {
    pub type_env: crate::typesys::TypeEnv,
    pub class_env: crate::typesys::ClassEnv,
    pub value_env: crate::evaluator::Env,
    pub last_loaded_paths: Vec<String>,
    pub defaulting_on: bool,
}

pub(crate) enum ReplMsg {
    Out(String),
    Err(String),
    Value(Value),
}

pub(crate) trait ReplIo {
    fn read_to_string(&self, path: &str) -> Result<String, String>;
}

pub(crate) struct FsIo;
impl ReplIo for FsIo {
    fn read_to_string(&self, path: &str) -> Result<String, String> {
        std::fs::read_to_string(path).map_err(|e| format!("エラー: ファイルを開けません: {}", e))
    }
}

pub(crate) fn handle_command<I: ReplIo>(
    state: &mut ReplState,
    cmd: ReplCommand,
    io: &I,
) -> Vec<ReplMsg> {
    use ReplMsg as M;
    match cmd {
        ReplCommand::TypeOf(src) => match parse_expr(&src) {
            Ok(expr) => match type_string_in_current_env(
                &state.type_env,
                &state.class_env,
                &expr,
                state.defaulting_on,
                &mut state.value_env,
            ) {
                Ok(s) => vec![M::Out(format!("-- {}", s))],
                Err(msg) => vec![M::Err(msg)],
            },
            Err(e) => vec![M::Err(format!("{}", e))],
        },
        ReplCommand::Let(src) => match parse_program(&src) {
            Ok(prog) => {
                let mut tent_env = state.type_env.clone_env();
                let mut tent_val = state.value_env.clone();
                let mut loaded: Vec<String> = Vec::new();
                for decl in prog.decls {
                    let body = if decl.params.is_empty() {
                        decl.expr
                    } else {
                        A::Expr::Lambda {
                            params: decl.params,
                            body: Box::new(decl.expr),
                        }
                    };
                    let mut st = InferState {
                        supply: TVarSupply::new(),
                        subst: Default::default(),
                    };
                    match infer_expr(&tent_env, &state.class_env, &mut st, &body) {
                        Ok((s, mut q_rhs)) => {
                            if let Some(sig) = &decl.signature {
                                let ty_anno = crate::infer::type_from_texpr(&sig.r#type);
                                match crate::typesys::unify(
                                    crate::typesys::apply_subst_t(&s, &q_rhs.r#type),
                                    ty_anno,
                                ) {
                                    Ok(s2) => {
                                        let s = crate::typesys::compose(&s2, &s);
                                        q_rhs = crate::typesys::apply_subst_q(&s, &q_rhs);
                                    }
                                    Err(e) => {
                                        return vec![M::Err(format!(
                                            "エラー: [{}] {}",
                                            e.code, e.message
                                        ))];
                                    }
                                }
                            }
                            let sch = generalize(&tent_env, q_rhs.clone());
                            tent_env.extend(decl.name.clone(), sch);
                            match eval_expr(&body, &mut tent_val) {
                                Ok(v) => {
                                    tent_val.insert(decl.name.clone(), v);
                                    loaded.push(decl.name.clone());
                                }
                                Err(e) => return vec![M::Err(format!("{}", e))],
                            }
                        }
                        Err(e) => return vec![M::Err(format!("{}", e))],
                    }
                }
                state.type_env = tent_env;
                state.value_env = tent_val;
                if loaded.is_empty() {
                    vec![]
                } else {
                    vec![M::Out(format!("Defined {}", loaded.join(", ")))]
                }
            }
            Err(e) => vec![M::Err(format!("{}", e))],
        },
        ReplCommand::Load(path) => match io.read_to_string(&path) {
            Ok(src) => match parse_program(&src) {
                Ok(prog) => match load_program_into_env(
                    &prog,
                    &mut state.type_env,
                    &state.class_env,
                    &mut state.value_env,
                ) {
                    Ok(loaded) => {
                        let mut msgs = vec![M::Out(format!(
                            "Loaded {} def(s) from {}",
                            loaded.len(),
                            path
                        ))];
                        for name in &loaded {
                            if let Some(sch) = state.type_env.lookup(name) {
                                msgs.push(M::Out(format!(
                                    "  {} :: {}",
                                    name,
                                    pretty_qual(&sch.qual)
                                )));
                            }
                        }
                        if !state.last_loaded_paths.contains(&path) {
                            state.last_loaded_paths.push(path);
                        }
                        msgs
                    }
                    Err(msg) => vec![M::Err(msg)],
                },
                Err(e) => vec![M::Err(format!("{}", e))],
            },
            Err(e) => vec![M::Err(e)],
        },
        ReplCommand::Reload => {
            if state.last_loaded_paths.is_empty() {
                vec![M::Err("エラー: 直近の :load がありません".into())]
            } else {
                let mut msgs = Vec::new();
                for path in &state.last_loaded_paths.clone() {
                    match io.read_to_string(path) {
                        Ok(src) => match parse_program(&src) {
                            Ok(prog) => match load_program_into_env(
                                &prog,
                                &mut state.type_env,
                                &state.class_env,
                                &mut state.value_env,
                            ) {
                                Ok(loaded) => msgs.push(M::Out(format!(
                                    "Reloaded {} def(s) from {}",
                                    loaded.len(),
                                    path
                                ))),
                                Err(msg) => msgs.push(M::Err(msg)),
                            },
                            Err(e) => msgs.push(M::Err(format!("{}", e))),
                        },
                        Err(e) => msgs.push(M::Err(e)),
                    }
                }
                msgs
            }
        }
        ReplCommand::Browse(pfx) => {
            let p = pfx.unwrap_or_default();
            let mut names: Vec<&String> = state
                .type_env
                .env
                .keys()
                .filter(|n| n.starts_with(&p))
                .collect();
            names.sort();
            if names.is_empty() {
                vec![M::Out("(定義なし)".into())]
            } else {
                names
                    .into_iter()
                    .map(|n| {
                        if let Some(sch) = state.type_env.lookup(n) {
                            M::Out(format!("  {} :: {}", n, pretty_qual(&sch.qual)))
                        } else {
                            M::Out(format!("  {}", n))
                        }
                    })
                    .collect()
            }
        }
        ReplCommand::SetDefault(on) => {
            state.defaulting_on = on;
            vec![M::Out(format!(
                "set default = {}",
                if on { "on" } else { "off" }
            ))]
        }
        ReplCommand::Unset(name) => {
            let mut removed = false;
            if state.type_env.env.remove(&name).is_some() {
                removed = true;
            }
            if state.value_env.remove(&name).is_some() {
                removed = true;
            }
            if removed {
                vec![M::Out(format!("Unset {}", name))]
            } else {
                vec![M::Err(format!("エラー: 未定義です: {}", name))]
            }
        }
        ReplCommand::Eval(src) => match parse_expr(&src) {
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
                    vec![M::Value(val)]
                }
                Err(msg) => vec![M::Err(msg)],
            },
            Err(e) => vec![M::Err(format!("{}", e))],
        },
        ReplCommand::Help | ReplCommand::Quit => vec![],
        ReplCommand::Invalid(s) => vec![M::Err(format!("エラー: コマンド形式が不正です: {}", s))],
    }
}
/// コマンド入力の字面を純粋に解釈して列挙型へ落とし込む（I/O は行わない）。
/// REPL 実装からの切り離し用ヘルパ。
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReplCommand {
    Help,
    Quit,
    /// `:t` / `:type`
    TypeOf(String),
    /// その場定義（正規化済みソース）
    Let(String),
    Load(String),
    Reload,
    Browse(Option<String>),
    SetDefault(bool),
    Unset(String),
    /// 上記に当てはまらなければ式として扱う
    Eval(String),
    /// 形式が不正なコマンド
    Invalid(String),
}

#[cfg_attr(not(test), allow(dead_code))]
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

/// `:let` のペイロード（定義群）を `let` 付きの正規化済みソースへ変換する。
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
        // 1本の定義行: `name ...` で始まるなら `let` を補う
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
// :t 用の型表示（正規化 + defaulting + フォールバック評価）
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
            let mut q2 = apply_subst_q(&s, &q);
            if defaulting_on {
                q2 = apply_defaulting_simple(&q2);
            }
            Ok(pretty_qual(&q2))
        }
        Err(_) => {
            // 推論失敗時は値を評価して代表型にマップ
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

// REPL用: 現在の型/値環境で推論し、it 用の Scheme と Value を返す
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
            let mut q2 = apply_subst_q(&s, &q);
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
    fn needs_more_input_balancing_paren_bracket() {
        assert!(needs_more_input("(1 + 2"));
        assert!(!needs_more_input("(1 + 2)"));
        assert!(needs_more_input("[1, 2"));
        assert!(!needs_more_input("[1, 2]"));
    }

    #[test]
    fn needs_more_input_strings_and_chars() {
        assert!(needs_more_input("\"abc"));
        assert!(!needs_more_input("\"abc\""));
        assert!(needs_more_input("'a"));
        assert!(!needs_more_input("'a'"));
        // エスケープを含むケース（閉じていれば false）
        assert!(!needs_more_input("\"a\\\"b\""));
        assert!(!needs_more_input("'\\''"));
    }

    #[test]
    fn needs_more_input_commands_do_not_continue() {
        // コマンドは先頭 ':' で常に単行扱い
        assert!(!needs_more_input(":t ("));
        assert!(!needs_more_input(":load file.tl"));
    }

    #[test]
    fn normalize_let_payload_single_and_multi() {
        assert_eq!(normalize_let_payload("f x = x"), "let f x = x");
        assert_eq!(
            normalize_let_payload("let f x = x; g y = y"),
            "let f x = x;\nlet g y = y"
        );
        assert_eq!(normalize_let_payload("id :: a -> a"), "id :: a -> a");
    }

    #[test]
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

    struct NoopIo;
    impl ReplIo for NoopIo {
        fn read_to_string(&self, _path: &str) -> Result<String, String> {
            Err("unexpected io".into())
        }
    }

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
    fn handle_browse_and_set_default_and_unset() {
        let mut state = mk_state();
        // 2 つの定義を型環境に追加
        let sch = Scheme {
            vars: vec![],
            qual: qualify(Type::TCon(TCon { name: "Int".into() }), vec![]),
        };
        state.type_env.extend("foo", sch.clone());
        state.type_env.extend("bar", sch);

        // :browse （全件）
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

        // :browse foo （接頭辞フィルタ）
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

        // :set default on
        let msgs = handle_command(&mut state, ReplCommand::SetDefault(true), &NoopIo);
        assert!(matches!(msgs[0], ReplMsg::Out(ref s) if s.contains("set default = on")));
        assert!(state.defaulting_on);

        // :unset foo（成功）
        let msgs = handle_command(&mut state, ReplCommand::Unset("foo".into()), &NoopIo);
        assert!(matches!(msgs[0], ReplMsg::Out(ref s) if s.contains("Unset foo")));
        assert!(state.type_env.lookup("foo").is_none());
        // :unset foo（再度）→ エラー
        let msgs = handle_command(&mut state, ReplCommand::Unset("foo".into()), &NoopIo);
        assert!(matches!(msgs[0], ReplMsg::Err(ref s) if s.contains("未定義")));
    }

    struct MapIo(std::collections::HashMap<String, Result<String, String>>);
    impl ReplIo for MapIo {
        fn read_to_string(&self, path: &str) -> Result<String, String> {
            self.0
                .get(path)
                .cloned()
                .unwrap_or_else(|| Err("not found".into()))
        }
    }

    #[test]
    fn handle_load_success_and_reload_paths() {
        let mut state = mk_state();
        let prog = "let x = 1;".to_string();
        let mut map = std::collections::HashMap::new();
        map.insert("mem://ok".into(), Ok(prog));
        let io = MapIo(map);
        let msgs = handle_command(&mut state, ReplCommand::Load("mem://ok".into()), &io);
        // Loaded メッセージと型行が返る
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

        // :reload が成功する
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
    fn handle_load_error_and_reload_without_history() {
        let mut state = mk_state();
        // 読み込み失敗
        let io = MapIo(std::collections::HashMap::new());
        let msgs = handle_command(&mut state, ReplCommand::Load("mem://missing".into()), &io);
        assert!(msgs.iter().any(|m| matches!(m, ReplMsg::Err(_))));

        // :reload 直近なし
        let msgs = handle_command(&mut state, ReplCommand::Reload, &io);
        assert!(msgs
            .iter()
            .any(|m| matches!(m, ReplMsg::Err(s) if s.contains("直近の :load"))));
    }
}
