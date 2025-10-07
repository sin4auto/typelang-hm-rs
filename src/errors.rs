// パス: src/errors.rs
// 役割: 共通エラー型とメッセージ整形ロジックを集約する
// 意図: 字句・構文・型・評価を横断して一貫した診断を提供する
// 関連ファイル: src/lexer.rs, src/parser.rs, src/evaluator.rs
//! エラー表現モジュール
//!
//! - 共有フォーマットの `ErrorInfo` を中心にメタデータを保持する。
//! - 各レイヤー向けの軽量な新種エラー型を薄いラッパーとして公開する。
//! - 位置情報とスニペットつきメッセージの整形を一箇所で実装する。

use std::error::Error as StdError;
use std::fmt::{self, Display, Formatter};

#[derive(Debug, Clone)]
/// エラーコード・本文・位置メタデータを保持する基礎構造体。
pub struct ErrorInfo {
    pub code: &'static str,
    pub msg: String,
    pub pos: Option<usize>,      // 原文バイトオフセット
    pub line: Option<usize>,     // 1 始まりの行番号
    pub col: Option<usize>,      // 1 始まりの列番号
    pub snippet: Option<String>, // 問題行の抜粋文字列
    pub stack: Vec<FrameInfo>,   // スタックトレース情報
}

#[derive(Debug, Clone)]
/// 評価や解析の呼び出し位置を表現する 1 フレーム分の情報。
pub struct FrameInfo {
    pub summary: String,
    pub pos: Option<usize>,
    pub line: Option<usize>,
    pub col: Option<usize>,
}

impl FrameInfo {
    /// 表示用のサマリと位置を指定してフレームを生成する。
    pub fn new(
        summary: impl Into<String>,
        pos: Option<usize>,
        line: Option<usize>,
        col: Option<usize>,
    ) -> Self {
        Self {
            summary: summary.into(),
            pos,
            line,
            col,
        }
    }
}

/// `ErrorInfo` 生成を簡潔にするためのファクトリ群。
impl ErrorInfo {
    /// コードと本文だけでエラー情報を初期化する。
    pub fn new(code: &'static str, msg: impl Into<String>, pos: Option<usize>) -> Self {
        Self {
            code,
            msg: msg.into(),
            pos,
            line: None,
            col: None,
            snippet: None,
            stack: Vec::new(),
        }
    }
    /// 行・列などの位置情報を付与してエラー情報を構築する。
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
            stack: Vec::new(),
        }
    }
    /// エラー周辺の抜粋を追加してチェーン可能にする。
    pub fn with_snippet(mut self, snippet: impl Into<String>) -> Self {
        self.snippet = Some(snippet.into());
        self
    }
    /// フレームを追加してチェーン可能にする。
    pub fn with_frame(mut self, frame: FrameInfo) -> Self {
        self.stack.push(frame);
        self
    }
    /// スタックへフレームを後置する。
    pub fn push_frame(&mut self, frame: FrameInfo) {
        self.stack.push(frame);
    }
    /// 既存の位置が未設定なら指定値で埋める。
    pub fn fill_position_if_absent(
        &mut self,
        pos: Option<usize>,
        line: Option<usize>,
        col: Option<usize>,
    ) {
        if self.pos.is_none() {
            self.pos = pos;
        }
        if self.line.is_none() {
            self.line = line;
        }
        if self.col.is_none() {
            self.col = col;
        }
    }
}

/// `ErrorInfo` の整形ルールを `Display` 経由で提供する。
impl Display for ErrorInfo {
    /// `[CODE] message @line=..` の形式で文字列化する。
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // 1 行目: 位置情報の有無で出力を切り替える
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
        // 2 行目以降: caret 付きスニペットを描画する
        if let (Some(s), Some(c)) = (&self.snippet, self.col) {
            let caret = if c > 1 {
                " ".repeat(c - 1) + "^"
            } else {
                "^".to_string()
            };
            write!(f, "\n{}\n{}", s, caret)?;
        }
        if !self.stack.is_empty() {
            write!(f, "\nStack trace:")?;
            for frame in self.stack.iter().rev() {
                write!(f, "\n  at {}", frame.summary)?;
                match (frame.line, frame.col, frame.pos) {
                    (Some(l), Some(c), Some(p)) => {
                        write!(f, " (line {}, col {}, pos {})", l, c, p)?
                    }
                    (Some(l), Some(c), None) => write!(f, " (line {}, col {})", l, c)?,
                    (Some(l), None, _) => write!(f, " (line {})", l)?,
                    (_, _, Some(p)) => write!(f, " (pos {})", p)?,
                    _ => {}
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
/// 字句解析で報告されるエラー型。
pub struct LexerError(pub Box<ErrorInfo>);
/// `LexerError` を生成するためのラッパー関数群。
impl LexerError {
    /// コードと位置を指定して字句解析エラーを作成する。
    pub fn new(code: &'static str, msg: impl Into<String>, pos: Option<usize>) -> Self {
        Self(Box::new(ErrorInfo::new(code, msg, pos)))
    }
    /// 行・列を含めた字句解析エラーを作成する。
    pub fn at(
        code: &'static str,
        msg: impl Into<String>,
        pos: Option<usize>,
        line: Option<usize>,
        col: Option<usize>,
    ) -> Self {
        Self(Box::new(ErrorInfo::at(code, msg, pos, line, col)))
    }
    /// スニペット付きの字句解析エラーを作成する。
    pub fn at_with_snippet(
        code: &'static str,
        msg: impl Into<String>,
        pos: Option<usize>,
        line: Option<usize>,
        col: Option<usize>,
        snippet: impl Into<String>,
    ) -> Self {
        Self(Box::new(
            ErrorInfo::at(code, msg, pos, line, col).with_snippet(snippet),
        ))
    }
}

#[derive(Debug, Clone)]
/// 構文解析で用いるエラー型。
pub struct ParseError(pub Box<ErrorInfo>);
/// `ParseError` を構築するヘルパーメソッド集。
impl ParseError {
    /// コードと本文だけで構文解析エラーを作成する。
    pub fn new(code: &'static str, msg: impl Into<String>, pos: Option<usize>) -> Self {
        Self(Box::new(ErrorInfo::new(code, msg, pos)))
    }
    /// 位置情報付きの構文解析エラーを作成する。
    pub fn at(
        code: &'static str,
        msg: impl Into<String>,
        pos: Option<usize>,
        line: Option<usize>,
        col: Option<usize>,
    ) -> Self {
        Self(Box::new(ErrorInfo::at(code, msg, pos, line, col)))
    }
}

#[derive(Debug, Clone)]
/// 型推論や型検査で利用するエラー型。
pub struct TypeError(pub Box<ErrorInfo>);
/// `TypeError` 向けの生成ショートカット。
impl TypeError {
    /// コードのみを指定して型エラーを作成する。
    pub fn new(code: &'static str, msg: impl Into<String>, pos: Option<usize>) -> Self {
        Self(Box::new(ErrorInfo::new(code, msg, pos)))
    }
    /// 位置情報付きの型エラーを作成する。
    pub fn at(
        code: &'static str,
        msg: impl Into<String>,
        pos: Option<usize>,
        line: Option<usize>,
        col: Option<usize>,
    ) -> Self {
        Self(Box::new(ErrorInfo::at(code, msg, pos, line, col)))
    }
    /// スニペットを添えて型エラーを作成する。
    pub fn at_with_snippet(
        code: &'static str,
        msg: impl Into<String>,
        pos: Option<usize>,
        line: Option<usize>,
        col: Option<usize>,
        snippet: impl Into<String>,
    ) -> Self {
        Self(Box::new(
            ErrorInfo::at(code, msg, pos, line, col).with_snippet(snippet),
        ))
    }
}

#[derive(Debug, Clone)]
/// 評価器で発生するエラー型。
pub struct EvalError(pub Box<ErrorInfo>);
/// `EvalError` を作成するヘルパメソッド。
impl EvalError {
    /// 評価時の汎用エラーを作成する。
    pub fn new(code: &'static str, msg: impl Into<String>, pos: Option<usize>) -> Self {
        Self(Box::new(ErrorInfo::new(code, msg, pos)))
    }
    /// 位置情報付きの評価エラーを作成する。
    pub fn at(
        code: &'static str,
        msg: impl Into<String>,
        pos: Option<usize>,
        line: Option<usize>,
        col: Option<usize>,
    ) -> Self {
        Self(Box::new(ErrorInfo::at(code, msg, pos, line, col)))
    }
}

/// `Display` 実装を `ErrorInfo` へ委譲する。
impl Display for LexerError {
    /// 内部の `ErrorInfo` をそのまま整形する。
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}
impl StdError for LexerError {}

/// `ParseError` の表示実装を `ErrorInfo` に委譲する。
impl Display for ParseError {
    /// `ErrorInfo` をそのまま書式化する。
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}
impl StdError for ParseError {}

/// `TypeError` の表示実装を `ErrorInfo` に委譲する。
impl Display for TypeError {
    /// `ErrorInfo` をそのまま書式化する。
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}
impl StdError for TypeError {}

/// `EvalError` の表示実装を `ErrorInfo` に委譲する。
impl Display for EvalError {
    /// `ErrorInfo` をそのまま書式化する。
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}
impl StdError for EvalError {}
