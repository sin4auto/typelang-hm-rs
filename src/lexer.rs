// パス: src/lexer.rs
// 役割: UTF-8 対応の字句解析器とトークン定義を提供する
// 意図: 構文解析に必要な位置付きトークンを生成する
// 関連ファイル: src/parser/mod.rs, src/errors.rs, tests/lexer_parser.rs
//! 字句解析モジュール
//!
//! - EBNF の規則に従い、ソースをトークン列へ変換する。
//! - 正規表現ライブラリを使わず、標準ライブラリのみで実装する。
//! - すべてのトークンに行・列・バイト位置を記録し、診断情報と連携させる。

use crate::errors::LexerError;

#[derive(Debug, Clone, PartialEq, Eq)]
/// 生成されたトークンとその位置情報を保持するレコード。
pub struct Token {
    pub kind: TokenKind,
    pub value: String,
    pub pos: usize,
    pub line: usize,
    pub col: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// 字句解析で識別されるトークンの分類。
pub enum TokenKind {
    EOF,
    // 演算子・記号トークン
    LAMBDA, // バックスラッシュ `\`
    ARROW,  // 右矢印 `->`
    DCOLON, // 型注釈用のダブルコロン `::`
    DARROW, // 制約矢印 `=>`
    LE,
    GE,
    EQ,
    NE,
    LT,
    GT,
    PLUS,
    MINUS,
    DBLSTAR,
    STAR,
    SLASH,
    CARET,
    LPAREN,
    RPAREN,
    LBRACK,
    RBRACK,
    COMMA,
    SEMI,
    EQUAL,
    QMARK,
    AT,
    UNDERSCORE,
    BAR,
    // リテラル分類
    CHAR,
    STRING,
    HEX,
    OCT,
    BIN,
    FLOAT,
    INT,
    // 識別子分類
    CONID,
    VARID,
    // キーワード分類
    LET,
    IN,
    IF,
    THEN,
    ELSE,
    CASE,
    OF,
    DATA,
    CLASS,
    INSTANCE,
    WHERE,
    TRUE,
    FALSE,
}

#[derive(Debug)]
/// 行頭オフセットを事前計算し、行・列情報を素早く算出するヘルパ。
struct LineMap {
    starts: Vec<usize>,
}

impl LineMap {
    /// 入力全体を 1 度だけ走査して行頭インデックスを収集する。
    fn new(src: &str) -> Self {
        let mut starts = vec![0];
        for (idx, ch) in src.char_indices() {
            if ch == '\n' {
                let next = idx + ch.len_utf8();
                if next <= src.len() {
                    starts.push(next);
                }
            }
        }
        Self { starts }
    }

    /// 指定バイト位置の行番号と桁位置を返す。
    fn locate(&self, src: &str, pos: usize) -> (usize, usize) {
        let idx = match self.starts.binary_search(&pos) {
            Ok(i) => i,
            Err(0) => 0,
            Err(i) => i - 1,
        };
        let line = idx + 1;
        let start = self.starts[idx];
        let col = src[start..pos].chars().count() + 1;
        (line, col)
    }

    /// 指定行に対応するテキスト断片を返す（改行は除去する）。
    fn line_text<'a>(&self, src: &'a str, line: usize) -> &'a str {
        if line == 0 {
            return "";
        }
        let idx = line - 1;
        if idx >= self.starts.len() {
            return "";
        }
        let start = self.starts[idx];
        let end = self.starts.get(idx + 1).copied().unwrap_or(src.len());
        let slice = &src[start..end];
        slice.strip_suffix('\n').unwrap_or(slice)
    }
}

/// 空白文字かどうかを判定するユーティリティ。
fn is_whitespace(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\r' | '\n')
}
/// 10 進数字かどうかを判定するユーティリティ。
fn is_digit(c: char) -> bool {
    c.is_ascii_digit()
}
/// 16 進数字かどうかを判定するユーティリティ。
fn is_hexdigit(c: char) -> bool {
    c.is_ascii_hexdigit()
}
/// 8 進数字かどうかを判定するユーティリティ。
fn is_octdigit(c: char) -> bool {
    matches!(c, '0'..='7')
}
/// 2 進数字かどうかを判定するユーティリティ。
fn is_bindigit(c: char) -> bool {
    matches!(c, '0' | '1')
}
/// 識別子の先頭に使用可能な文字かどうかを判定する。
fn is_letter(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}
/// 識別子の後続として許容される文字か判定する。
fn is_ident_rest(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '\''
}

struct Lexer<'a> {
    src: &'a str,
    cursor: usize,
    len: usize,
    line_map: LineMap,
    tokens: Vec<Token>,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            src,
            cursor: 0,
            len: src.len(),
            line_map: LineMap::new(src),
            tokens: Vec::new(),
        }
    }

    fn run(mut self) -> Result<Vec<Token>, LexerError> {
        while self.cursor < self.len {
            if self.consume_trivia()? {
                continue;
            }
            if self.cursor >= self.len {
                break;
            }
            self.lex_token()?;
        }
        self.push_simple(TokenKind::EOF, "", self.len);
        Ok(self.tokens)
    }

    fn consume_trivia(&mut self) -> Result<bool, LexerError> {
        let mut advanced = false;
        loop {
            if self.consume_whitespace() {
                advanced = true;
                continue;
            }
            if self.cursor >= self.len {
                break;
            }
            if self.consume_block_comment()? {
                advanced = true;
                continue;
            }
            if self.cursor >= self.len {
                break;
            }
            if self.consume_line_comment() {
                advanced = true;
                continue;
            }
            break;
        }
        Ok(advanced)
    }

    fn consume_whitespace(&mut self) -> bool {
        let mut advanced = false;
        while let Some(ch) = self.peek_char() {
            if is_whitespace(ch) {
                self.advance_char();
                advanced = true;
            } else {
                break;
            }
        }
        advanced
    }

    fn consume_line_comment(&mut self) -> bool {
        if !self.starts_with("--") {
            return false;
        }
        self.advance_bytes(2);
        while let Some(ch) = self.advance_char() {
            if ch == '\n' {
                break;
            }
        }
        true
    }

    fn consume_block_comment(&mut self) -> Result<bool, LexerError> {
        if !self.starts_with("{-") {
            return Ok(false);
        }
        let mut idx = self.cursor + 2;
        let mut depth = 1;
        while idx < self.len {
            if self.src[idx..].starts_with("-}") {
                depth -= 1;
                idx += 2;
                if depth == 0 {
                    self.cursor = idx;
                    return Ok(true);
                }
                continue;
            }
            if self.src[idx..].starts_with("{-") {
                depth += 1;
                idx += 2;
                continue;
            }
            let Some(ch) = self.src[idx..].chars().next() else {
                break;
            };
            idx += ch.len_utf8();
        }
        let pos = idx.min(self.len);
        Err(self.err("LEX001", "ブロックコメントが閉じていません", pos))
    }

    fn lex_token(&mut self) -> Result<(), LexerError> {
        let start = self.cursor;
        let ch = self
            .peek_char()
            .expect("lex_token は EOF では呼び出されない");
        if self.try_multi_char_symbol(ch) {
            return Ok(());
        }
        if self.try_single_char_symbol(ch) {
            return Ok(());
        }
        if ch == '\'' {
            return self.lex_char_literal();
        }
        if ch == '"' {
            return self.lex_string_literal();
        }
        if is_digit(ch) {
            return self.lex_number();
        }
        if is_letter(ch) {
            return self.lex_identifier_or_keyword();
        }
        Err(self.err("LEX090", format!("字句解析に失敗: {:?}", ch), start))
    }

    fn try_multi_char_symbol(&mut self, first: char) -> bool {
        let Some(second) = self.peek_second_char() else {
            return false;
        };
        let token = match (first, second) {
            ('-', '>') => Some((TokenKind::ARROW, "->")),
            (':', ':') => Some((TokenKind::DCOLON, "::")),
            ('=', '>') => Some((TokenKind::DARROW, "=>")),
            ('<', '=') => Some((TokenKind::LE, "<=")),
            ('>', '=') => Some((TokenKind::GE, ">=")),
            ('=', '=') => Some((TokenKind::EQ, "==")),
            ('/', '=') => Some((TokenKind::NE, "/=")),
            ('*', '*') => Some((TokenKind::DBLSTAR, "**")),
            _ => None,
        };
        if let Some((kind, value)) = token {
            let start = self.cursor;
            self.advance_bytes(first.len_utf8());
            self.advance_bytes(second.len_utf8());
            self.push_simple(kind, value, start);
            return true;
        }
        false
    }

    fn try_single_char_symbol(&mut self, ch: char) -> bool {
        let token = match ch {
            '\\' => Some((TokenKind::LAMBDA, "\\")),
            '<' => Some((TokenKind::LT, "<")),
            '>' => Some((TokenKind::GT, ">")),
            '+' => Some((TokenKind::PLUS, "+")),
            '-' => Some((TokenKind::MINUS, "-")),
            '*' => Some((TokenKind::STAR, "*")),
            '/' => Some((TokenKind::SLASH, "/")),
            '^' => Some((TokenKind::CARET, "^")),
            '(' => Some((TokenKind::LPAREN, "(")),
            ')' => Some((TokenKind::RPAREN, ")")),
            '[' => Some((TokenKind::LBRACK, "[")),
            ']' => Some((TokenKind::RBRACK, "]")),
            ',' => Some((TokenKind::COMMA, ",")),
            ';' => Some((TokenKind::SEMI, ";")),
            '=' => Some((TokenKind::EQUAL, "=")),
            '|' => Some((TokenKind::BAR, "|")),
            '?' => Some((TokenKind::QMARK, "?")),
            '@' => Some((TokenKind::AT, "@")),
            '_' => Some((TokenKind::UNDERSCORE, "_")),
            _ => None,
        };
        if let Some((kind, value)) = token {
            let start = self.cursor;
            self.advance_bytes(ch.len_utf8());
            self.push_simple(kind, value, start);
            return true;
        }
        false
    }

    fn lex_char_literal(&mut self) -> Result<(), LexerError> {
        let start = self.cursor;
        self.advance_bytes(1); // 開始クォート
        let mut escaped = false;
        let mut ok = false;
        while let Some(ch) = self.advance_char() {
            if !escaped {
                if ch == '\\' {
                    escaped = true;
                    continue;
                }
                if ch == '\'' {
                    ok = true;
                    break;
                }
                if ch == '\n' {
                    break;
                }
            } else {
                escaped = false;
            }
        }
        if !ok {
            return Err(self.err("LEX002", "文字リテラルが閉じていません", start));
        }
        let end = self.cursor;
        self.push_slice(TokenKind::CHAR, start, end);
        Ok(())
    }

    fn lex_string_literal(&mut self) -> Result<(), LexerError> {
        let start = self.cursor;
        self.advance_bytes(1); // 開始ダブルクォート
        let mut escaped = false;
        let mut ok = false;
        while let Some(ch) = self.advance_char() {
            if !escaped {
                if ch == '\\' {
                    escaped = true;
                    continue;
                }
                if ch == '"' {
                    ok = true;
                    break;
                }
                if ch == '\n' {
                    break;
                }
            } else {
                escaped = false;
            }
        }
        if !ok {
            return Err(self.err("LEX003", "文字列リテラルが閉じていません", start));
        }
        let end = self.cursor;
        self.push_slice(TokenKind::STRING, start, end);
        Ok(())
    }

    fn lex_number(&mut self) -> Result<(), LexerError> {
        let start = self.cursor;
        if self.starts_with("0x") || self.starts_with("0X") {
            return self.lex_prefixed_number(
                start,
                2,
                is_hexdigit,
                TokenKind::HEX,
                "LEX010",
                "16進数の桁がありません",
            );
        }
        if self.starts_with("0o") || self.starts_with("0O") {
            return self.lex_prefixed_number(
                start,
                2,
                is_octdigit,
                TokenKind::OCT,
                "LEX011",
                "8進数の桁がありません",
            );
        }
        if self.starts_with("0b") || self.starts_with("0B") {
            return self.lex_prefixed_number(
                start,
                2,
                is_bindigit,
                TokenKind::BIN,
                "LEX012",
                "2進数の桁がありません",
            );
        }

        while let Some(ch) = self.peek_char() {
            if is_digit(ch) {
                self.advance_char();
            } else {
                break;
            }
        }

        let mut is_float = false;
        if self.peek_char() == Some('.') {
            if let Some(next_digit) = self.peek_second_char() {
                if is_digit(next_digit) {
                    is_float = true;
                    self.advance_char(); // '.'
                    while let Some(ch) = self.peek_char() {
                        if is_digit(ch) {
                            self.advance_char();
                        } else {
                            break;
                        }
                    }
                }
            }
        }

        if let Some('e') | Some('E') = self.peek_char() {
            let mut idx = self.cursor + 1;
            if let Some(sign) = self.char_at(idx) {
                if sign == '+' || sign == '-' {
                    idx += 1;
                }
            }
            let mut count = 0;
            let mut scan = idx;
            while let Some(ch) = self.char_at(scan) {
                if is_digit(ch) {
                    scan += ch.len_utf8();
                    count += 1;
                } else {
                    break;
                }
            }
            if count > 0 {
                is_float = true;
                self.cursor = scan;
            }
        }

        let end = self.cursor;
        let kind = if is_float {
            TokenKind::FLOAT
        } else {
            TokenKind::INT
        };
        self.push_slice(kind, start, end);
        Ok(())
    }

    fn lex_prefixed_number<F>(
        &mut self,
        start: usize,
        prefix_len: usize,
        mut predicate: F,
        kind: TokenKind,
        code: &'static str,
        msg: &str,
    ) -> Result<(), LexerError>
    where
        F: FnMut(char) -> bool,
    {
        self.advance_bytes(prefix_len);
        let mut count = 0;
        while let Some(ch) = self.peek_char() {
            if predicate(ch) {
                self.advance_char();
                count += 1;
            } else {
                break;
            }
        }
        if count == 0 {
            return Err(self.err(code, msg, start));
        }
        let end = self.cursor;
        self.push_slice(kind, start, end);
        Ok(())
    }

    fn lex_identifier_or_keyword(&mut self) -> Result<(), LexerError> {
        let start = self.cursor;
        self.advance_char();
        while let Some(ch) = self.peek_char() {
            if is_ident_rest(ch) {
                self.advance_char();
            } else {
                break;
            }
        }
        let slice = &self.src[start..self.cursor];
        let (kind, value) = match slice {
            "let" => (TokenKind::LET, slice),
            "in" => (TokenKind::IN, slice),
            "if" => (TokenKind::IF, slice),
            "then" => (TokenKind::THEN, slice),
            "else" => (TokenKind::ELSE, slice),
            "case" => (TokenKind::CASE, slice),
            "of" => (TokenKind::OF, slice),
            "data" => (TokenKind::DATA, slice),
            "class" => (TokenKind::CLASS, slice),
            "instance" => (TokenKind::INSTANCE, slice),
            "where" => (TokenKind::WHERE, slice),
            "True" => (TokenKind::TRUE, slice),
            "False" => (TokenKind::FALSE, slice),
            _ => {
                let first = slice.chars().next().expect("識別子は少なくとも1文字");
                if first.is_ascii_uppercase() {
                    (TokenKind::CONID, slice)
                } else {
                    (TokenKind::VARID, slice)
                }
            }
        };
        self.push_simple(kind, value, start);
        Ok(())
    }

    fn push_simple(&mut self, kind: TokenKind, value: &str, start: usize) {
        let (line, col) = self.line_map.locate(self.src, start);
        self.tokens.push(Token {
            kind,
            value: value.into(),
            pos: start,
            line,
            col,
        });
    }

    fn push_slice(&mut self, kind: TokenKind, start: usize, end: usize) {
        let (line, col) = self.line_map.locate(self.src, start);
        self.tokens.push(Token {
            kind,
            value: self.src[start..end].into(),
            pos: start,
            line,
            col,
        });
    }

    fn peek_char(&self) -> Option<char> {
        if self.cursor >= self.len {
            None
        } else {
            self.src[self.cursor..].chars().next()
        }
    }

    fn peek_second_char(&self) -> Option<char> {
        let mut iter = self.src[self.cursor..].chars();
        iter.next()?;
        iter.next()
    }

    fn char_at(&self, idx: usize) -> Option<char> {
        if idx >= self.len {
            None
        } else {
            self.src[idx..].chars().next()
        }
    }

    fn advance_char(&mut self) -> Option<char> {
        let ch = self.peek_char()?;
        self.advance_bytes(ch.len_utf8());
        Some(ch)
    }

    fn advance_bytes(&mut self, count: usize) {
        self.cursor = (self.cursor + count).min(self.len);
    }

    fn starts_with(&self, pattern: &str) -> bool {
        self.src[self.cursor..].starts_with(pattern)
    }

    fn err(&self, code: &'static str, message: impl Into<String>, pos: usize) -> LexerError {
        let (line, col) = self.line_map.locate(self.src, pos);
        LexerError::at_with_snippet(
            code,
            message,
            Some(pos),
            Some(line),
            Some(col),
            self.line_map.line_text(self.src, line).to_string(),
        )
    }
}

pub fn lex(src: &str) -> Result<Vec<Token>, LexerError> {
    Lexer::new(src).run()
}
