//! エラー型の定義（共通フォーマット: \[CODE\] メッセージ @line:col / @pos）。

use std::error::Error as StdError;
use std::fmt::{self, Display, Formatter};

#[derive(Debug, Clone)]
pub struct ErrorInfo {
    pub code: &'static str,
    pub msg: String,
    pub pos: Option<usize>,      // バイトオフセット（任意）
    pub line: Option<usize>,     // 1-origin（任意）
    pub col: Option<usize>,      // 1-origin（任意）
    pub snippet: Option<String>, // エラー行のスニペット（任意）
}

impl ErrorInfo {
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
    pub fn with_snippet(mut self, snippet: impl Into<String>) -> Self {
        self.snippet = Some(snippet.into());
        self
    }
}

impl Display for ErrorInfo {
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
pub struct LexerError(pub ErrorInfo);
impl LexerError {
    pub fn new(code: &'static str, msg: impl Into<String>, pos: Option<usize>) -> Self {
        Self(ErrorInfo::new(code, msg, pos))
    }
    pub fn at(
        code: &'static str,
        msg: impl Into<String>,
        pos: Option<usize>,
        line: Option<usize>,
        col: Option<usize>,
    ) -> Self {
        Self(ErrorInfo::at(code, msg, pos, line, col))
    }
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
pub struct ParseError(pub ErrorInfo);
impl ParseError {
    pub fn new(code: &'static str, msg: impl Into<String>, pos: Option<usize>) -> Self {
        Self(ErrorInfo::new(code, msg, pos))
    }
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
pub struct TypeError(pub ErrorInfo);
impl TypeError {
    pub fn new(code: &'static str, msg: impl Into<String>, pos: Option<usize>) -> Self {
        Self(ErrorInfo::new(code, msg, pos))
    }
    pub fn at(
        code: &'static str,
        msg: impl Into<String>,
        pos: Option<usize>,
        line: Option<usize>,
        col: Option<usize>,
    ) -> Self {
        Self(ErrorInfo::at(code, msg, pos, line, col))
    }
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
pub struct EvalError(pub ErrorInfo);
impl EvalError {
    pub fn new(code: &'static str, msg: impl Into<String>, pos: Option<usize>) -> Self {
        Self(ErrorInfo::new(code, msg, pos))
    }
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

impl Display for LexerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}
impl StdError for LexerError {}

impl Display for ParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}
impl StdError for ParseError {}

impl Display for TypeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}
impl StdError for TypeError {}

impl Display for EvalError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}
impl StdError for EvalError {}

// 後方互換エイリアスは不要となったため削除
