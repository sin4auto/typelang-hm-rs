// パス: src/bin/typelang.rs
// 役割: Binary entrypoint that launches the REPL runtime
// 意図: Offer a CLI executable for interactive language exploration
// 関連ファイル: src/repl/mod.rs, src/lib.rs, src/repl/cmd.rs
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use serde_json::to_string;

/// TypeLang CLI
#[derive(Parser)]
#[command(author, version, about = "TypeLang HM CLI")]
struct Cli {
    #[arg(
        long,
        help = "デフォルトのコード生成バックエンドを指定します",
        value_enum,
        default_value = "cranelift"
    )]
    backend: Backend,
    #[arg(long, help = "最適化レベル", value_enum, default_value = "debug")]
    optim_level: OptimLevel,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// 対話型の REPL を起動する
    Repl {
        #[arg(
            long,
            help = "REPL 終了時に生成されたネイティブバイナリを残す",
            default_value_t = false
        )]
        keep_outputs: bool,
    },
    /// .tl ファイルをネイティブバイナリとしてビルドする
    Build {
        /// 入力ファイルパス (.tl)
        input: PathBuf,
        /// 出力形式（現在は native のみ）
        #[arg(long, default_value = "native")]
        emit: EmitFormat,
        /// 出力ファイルパス（未指定時は `target/typelang/<entry_name>`）
        #[arg(long)]
        output: Option<PathBuf>,
        /// 使用するバックエンドを上書き
        #[arg(long, value_enum)]
        backend: Option<Backend>,
        /// 最適化レベルを上書き
        #[arg(long, value_enum)]
        optim_level: Option<OptimLevel>,
        /// 生成された辞書一覧を表示
        #[arg(long, default_value_t = false)]
        print_dictionaries: bool,
        /// 成功時に出力情報を JSON で表示
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}

#[derive(Clone, ValueEnum)]
enum EmitFormat {
    Native,
}

#[derive(Clone, ValueEnum, Copy, Debug)]
enum Backend {
    Cranelift,
    Llvm,
}

impl Default for Backend {
    fn default() -> Self {
        Self::Cranelift
    }
}

#[derive(Clone, ValueEnum, Copy, Debug)]
enum OptimLevel {
    Debug,
    Fast,
    Best,
}

impl Default for OptimLevel {
    fn default() -> Self {
        Self::Debug
    }
}

impl Backend {
    fn as_str(self) -> &'static str {
        match self {
            Backend::Cranelift => "cranelift",
            Backend::Llvm => "llvm",
        }
    }
}

impl OptimLevel {
    fn as_str(self) -> &'static str {
        match self {
            OptimLevel::Debug => "debug",
            OptimLevel::Fast => "fast",
            OptimLevel::Best => "best",
        }
    }
}

impl From<Backend> for typelang::NativeBackend {
    fn from(value: Backend) -> Self {
        match value {
            Backend::Cranelift => typelang::NativeBackend::Cranelift,
            Backend::Llvm => typelang::NativeBackend::Llvm,
        }
    }
}

impl From<OptimLevel> for typelang::NativeOptimLevel {
    fn from(value: OptimLevel) -> Self {
        match value {
            OptimLevel::Debug => typelang::NativeOptimLevel::Debug,
            OptimLevel::Fast => typelang::NativeOptimLevel::Fast,
            OptimLevel::Best => typelang::NativeOptimLevel::Best,
        }
    }
}

fn main() {
    let cli = Cli::parse();
    if let Err(err) = dispatch(cli) {
        eprintln!("エラー: {err}");
        process::exit(1);
    }
}

fn dispatch(cli: Cli) -> Result<(), String> {
    let default_backend = cli.backend;
    let default_optim = cli.optim_level;
    match cli.command.unwrap_or(Command::Repl {
        keep_outputs: false,
    }) {
        Command::Repl { keep_outputs } => {
            typelang::repl::run_repl_with_native(
                default_backend.into(),
                default_optim.into(),
                keep_outputs,
            );
            Ok(())
        }
        Command::Build {
            input,
            emit,
            output,
            backend,
            optim_level,
            print_dictionaries,
            json,
        } => {
            if !matches!(emit, EmitFormat::Native) {
                return Err("現在サポートされる emit 形式は native のみです".into());
            }
            let options = BuildOptions {
                backend: backend.unwrap_or(default_backend),
                optim_level: optim_level.unwrap_or(default_optim),
                print_dictionaries,
                json,
            };
            build_native(&input, output.as_deref(), &options)
        }
    }
}

struct BuildOptions {
    backend: Backend,
    optim_level: OptimLevel,
    print_dictionaries: bool,
    json: bool,
}

fn build_native(input: &Path, out: Option<&Path>, opts: &BuildOptions) -> Result<(), String> {
    let source = fs::read_to_string(input)
        .map_err(|e| format!("入力ファイルの読み込みに失敗しました: {e}"))?;
    let program = typelang::parser::parse_program(&source)
        .map_err(|e| format!("パースに失敗しました: {e}"))?;

    let output_path = out
        .map(PathBuf::from)
        .unwrap_or_else(|| default_output_path(input));

    let artifacts = typelang::emit_native_with_options(
        &program,
        &output_path,
        opts.backend.into(),
        opts.optim_level.into(),
    )
    .map_err(|e| format!("ネイティブコード生成に失敗しました: {e}"))?;

    if opts.json {
        #[derive(Serialize)]
        struct JsonDictionaryMethod<'a> {
            name: &'a str,
            signature: Option<&'a str>,
            symbol: Option<&'a str>,
            method_id: Option<u64>,
        }
        #[derive(Serialize)]
        struct JsonSpan {
            line: usize,
            column: usize,
        }
        #[derive(Serialize)]
        struct JsonDictionary<'a> {
            class: &'a str,
            r#type: &'a str,
            builder: Option<&'a str>,
            scheme: &'a str,
            origin: &'a str,
            span: JsonSpan,
            methods: Vec<JsonDictionaryMethod<'a>>,
        }
        #[derive(Serialize)]
        struct JsonOutput<'a> {
            status: &'static str,
            input: String,
            output: String,
            backend: &'static str,
            optim: &'static str,
            dictionaries: Vec<JsonDictionary<'a>>,
        }
        let dictionaries = artifacts
            .dictionaries
            .iter()
            .map(|d| JsonDictionary {
                class: &d.classname,
                r#type: &d.type_repr,
                builder: d.builder_symbol.as_deref(),
                scheme: &d.scheme_repr,
                origin: &d.origin,
                span: JsonSpan {
                    line: d.source_span.line,
                    column: d.source_span.column,
                },
                methods: d
                    .methods
                    .iter()
                    .map(|m| JsonDictionaryMethod {
                        name: &m.name,
                        signature: m.signature.as_deref(),
                        symbol: m.symbol.as_deref(),
                        method_id: m.method_id,
                    })
                    .collect(),
            })
            .collect();
        let payload = JsonOutput {
            status: "ok",
            input: input.display().to_string(),
            output: output_path.display().to_string(),
            backend: opts.backend.as_str(),
            optim: opts.optim_level.as_str(),
            dictionaries,
        };
        match to_string(&payload) {
            Ok(json) => println!("{}", json),
            Err(err) => {
                return Err(format!("JSON 出力に失敗しました: {err}"));
            }
        }
    } else {
        println!(
            "✅ ビルド成功: {} -> {} (backend={}, optim={})",
            input.display(),
            output_path.display(),
            opts.backend.as_str(),
            opts.optim_level.as_str()
        );
        if opts.print_dictionaries {
            print_dictionaries(&artifacts);
        }
    }
    Ok(())
}

fn default_output_path(input: &Path) -> PathBuf {
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    PathBuf::from("target/typelang").join(stem)
}

fn print_dictionaries(artifacts: &typelang::NativeBuildArtifacts) {
    if artifacts.dictionaries.is_empty() {
        println!("(辞書は生成されませんでした)");
        return;
    }
    println!("辞書一覧:");
    for dict in &artifacts.dictionaries {
        println!("- {}<{}>", dict.classname, dict.type_repr);
        if let Some(builder) = &dict.builder_symbol {
            println!("    builder: {builder}");
        }
        println!(
            "    origin: {} @ {}:{}",
            dict.origin, dict.source_span.line, dict.source_span.column
        );
        println!("    scheme: {}", dict.scheme_repr);
        if !dict.methods.is_empty() {
            println!("    methods:");
            for method in &dict.methods {
                let signature = method.signature.as_deref().unwrap_or("(unknown)");
                let symbol = method.symbol.as_deref().unwrap_or("(unresolved)");
                if let Some(id) = method.method_id {
                    println!(
                        "      - [{}] {} :: {} => {symbol}",
                        id, method.name, signature
                    );
                } else {
                    println!("      - {} :: {} => {symbol}", method.name, signature);
                }
            }
        }
    }
}
