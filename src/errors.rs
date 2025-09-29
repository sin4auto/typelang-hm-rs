// パス: src/errors.rs
// 役割: Shared error types and message formatting helpers
// 意図: Keep diagnostics uniform for lexer, parser, and runtime layers
// 関連ファイル: src/lexer.rs, src/parser.rs, src/evaluator.rs
//! エラー型の定義（共通フォーマット: \[CODE\] メッセージ @line:col / @pos）。

use std::error::Error as StdError;
use std::fmt::{self, Display, Formatter};

#[derive(Debug, Clone)]
/// エラーコードと位置情報を保持する構造体。
pub struct ErrorInfo {
    pub code: &'static str,
    pub msg: String,
    pub pos: Option<usize>,      // バイトオフセット（任意）
    pub line: Option<usize>,     // 1-origin（任意）
    pub col: Option<usize>,      // 1-origin（任意）
    pub snippet: Option<String>, // エラー行のスニペット（任意）
}

/// ErrorInfoに対する補助メソッド群。
impl ErrorInfo {
    /// コードとメッセージからエラー情報を生成する。
    pub fn new(code: &'static str, msg: impl Into<String>, pos: Option<usize>) -> Self {
        Self {
            code,
            msg: msg.into(),
            pos,
            line: None,
            col: None,
            snippet: None,
        }
    }
    /// 行・列などの位置情報を含めてエラー情報を構築する。
    pub fn at(
        code: &'static str,
        msg: impl Into<String>,
        pos: Option<usize>,
        line: Option<usize>,
        col: Option<usize>,
    ) -> Self {
        Self {
            code,
            msg: msg.into(),
            pos,
            line,
            col,
            snippet: None,
        }
    }
    /// エラー発生行のスニペットを付与して返す。
    pub fn with_snippet(mut self, snippet: impl Into<String>) -> Self {
        self.snippet = Some(snippet.into());
        self
    }
}

/// ErrorInfoを`Display`として整形する実装。
impl Display for ErrorInfo {
    /// 整形済みのエラーメッセージを書式化する。
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // 1行目: ヘッダ
        match (self.line, self.col, self.pos) {
            (Some(l), Some(c), Some(p)) => write!(
                f,
                "[{}] {} @line={},col={} @pos={}",
                self.code, self.msg, l, c, p
            )?,
            (Some(l), Some(c), None) => {
                write!(f, "[{}] {} @line={},col={}", self.code, self.msg, l, c)?
            }
            (_, _, Some(p)) => write!(f, "[{}] {} @pos={}", self.code, self.msg, p)?,
            _ => write!(f, "[{}] {}", self.code, self.msg)?,
        }
        // 2行目以降: スニペット
        if let (Some(s), Some(c)) = (&self.snippet, self.col) {
            let caret = if c > 1 {
                " ".repeat(c - 1) + "^"
            } else {
                "^".to_string()
            };
            write!(f, "\n{}\n{}", s, caret)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
/// 字句解析時のエラーを表すラッパー。
pub struct LexerError(pub ErrorInfo);
/// LexerErrorの生成ヘルパー。
impl LexerError {
    /// コードと位置を指定して字句解析エラーを生成する。
    pub fn new(code: &'static str, msg: impl Into<String>, pos: Option<usize>) -> Self {
        Self(ErrorInfo::new(code, msg, pos))
    }
    /// 行・列を含めた字句解析エラーを生成する。
    pub fn at(
        code: &'static str,
        msg: impl Into<String>,
        pos: Option<usize>,
        line: Option<usize>,
        col: Option<usize>,
    ) -> Self {
        Self(ErrorInfo::at(code, msg, pos, line, col))
    }
    /// スニペット付きの字句解析エラーを生成する。
    pub fn at_with_snippet(
        code: &'static str,
        msg: impl Into<String>,
        pos: Option<usize>,
        line: Option<usize>,
        col: Option<usize>,
        snippet: impl Into<String>,
    ) -> Self {
        Self(ErrorInfo::at(code, msg, pos, line, col).with_snippet(snippet))
    }
}

#[derive(Debug, Clone)]
/// 構文解析時のエラーを表すラッパー。
pub struct ParseError(pub ErrorInfo);
/// ParseErrorの生成ヘルパー。
impl ParseError {
    /// コードとメッセージだけで構文解析エラーを生成する。
    pub fn new(code: &'static str, msg: impl Into<String>, pos: Option<usize>) -> Self {
        Self(ErrorInfo::new(code, msg, pos))
    }
    /// 位置情報付きの構文解析エラーを生成する。
    pub fn at(
        code: &'static str,
        msg: impl Into<String>,
        pos: Option<usize>,
        line: Option<usize>,
        col: Option<usize>,
    ) -> Self {
        Self(ErrorInfo::at(code, msg, pos, line, col))
    }
}

#[derive(Debug, Clone)]
/// 型推論・型検査のエラーを表すラッパー。
pub struct TypeError(pub ErrorInfo);
/// TypeErrorの生成ヘルパー。
impl TypeError {
    /// コードのみを指定して型エラーを生成する。
    pub fn new(code: &'static str, msg: impl Into<String>, pos: Option<usize>) -> Self {
        Self(ErrorInfo::new(code, msg, pos))
    }
    /// 位置情報付きの型エラーを生成する。
    pub fn at(
        code: &'static str,
        msg: impl Into<String>,
        pos: Option<usize>,
        line: Option<usize>,
        col: Option<usize>,
    ) -> Self {
        Self(ErrorInfo::at(code, msg, pos, line, col))
    }
    /// スニペットを添えて型エラーを生成する。
    pub fn at_with_snippet(
        code: &'static str,
        msg: impl Into<String>,
        pos: Option<usize>,
        line: Option<usize>,
        col: Option<usize>,
        snippet: impl Into<String>,
    ) -> Self {
        Self(ErrorInfo::at(code, msg, pos, line, col).with_snippet(snippet))
    }
}

#[derive(Debug, Clone)]
/// 評価器で生じたエラーを表すラッパー。
pub struct EvalError(pub ErrorInfo);
/// EvalErrorの生成ヘルパー。
impl EvalError {
    /// 評価時の汎用エラーを生成する。
    pub fn new(code: &'static str, msg: impl Into<String>, pos: Option<usize>) -> Self {
        Self(ErrorInfo::new(code, msg, pos))
    }
    /// 位置情報付きの評価エラーを生成する。
    pub fn at(
        code: &'static str,
        msg: impl Into<String>,
        pos: Option<usize>,
        line: Option<usize>,
        col: Option<usize>,
    ) -> Self {
        Self(ErrorInfo::at(code, msg, pos, line, col))
    }
}

/// LexerErrorを`Display`として委譲する実装。
impl Display for LexerError {
    /// 内部のエラー情報をそのまま整形する。
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}
impl StdError for LexerError {}

/// ParseErrorを`Display`として委譲する実装。
impl Display for ParseError {
    /// 内部のエラー情報をそのまま整形する。
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}
impl StdError for ParseError {}

/// TypeErrorを`Display`として委譲する実装。
impl Display for TypeError {
    /// 内部のエラー情報をそのまま整形する。
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}
impl StdError for TypeError {}

/// EvalErrorを`Display`として委譲する実装。
impl Display for EvalError {
    /// 内部のエラー情報をそのまま整形する。
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}
impl StdError for EvalError {}
