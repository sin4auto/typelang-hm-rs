// パス: src/lexer.rs
// 役割: UTF-8 対応の字句解析器とトークン定義を提供する
// 意図: 構文解析に必要な位置付きトークンを生成する
// 関連ファイル: src/parser.rs, src/errors.rs, tests/lexer_parser.rs
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

/// 連続する空白文字を読み飛ばす。
fn skip_whitespace(src: &str, cursor: &mut usize) -> bool {
    let mut advanced = false;
    while *cursor < src.len() {
        let Some(ch) = src[*cursor..].chars().next() else {
            break;
        };
        if is_whitespace(ch) {
            *cursor += ch.len_utf8();
            advanced = true;
        } else {
            break;
        }
    }
    advanced
}

/// 行コメント "--" を読み飛ばす。
fn skip_line_comment(src: &str, cursor: &mut usize) -> bool {
    if !src[*cursor..].starts_with("--") {
        return false;
    }
    *cursor += 2; // "--"
    while *cursor < src.len() {
        let ch = src[*cursor..]
            .chars()
            .next()
            .expect("行コメント処理中にEOFへ到達しない");
        *cursor += ch.len_utf8();
        if ch == '\n' {
            break;
        }
    }
    true
}

/// ブロックコメント "{-" ... "-}" を読み飛ばす。
fn skip_block_comment(
    src: &str,
    cursor: &mut usize,
    line_map: &LineMap,
) -> Result<bool, LexerError> {
    if !src[*cursor..].starts_with("{-") {
        return Ok(false);
    }
    let mut idx = *cursor + 2; // consume "{-"
    let mut depth = 1;
    while idx < src.len() {
        if src[idx..].starts_with("-}") {
            depth -= 1;
            idx += 2;
            if depth == 0 {
                *cursor = idx;
                return Ok(true);
            }
            continue;
        }
        if src[idx..].starts_with("{-") {
            depth += 1;
            idx += 2;
            continue;
        }
        let Some(ch) = src[idx..].chars().next() else {
            break;
        };
        idx += ch.len_utf8();
    }
    let pos = idx.min(src.len());
    let (line, col) = line_map.locate(src, pos);
    Err(LexerError::at_with_snippet(
        "LEX001",
        "ブロックコメントが閉じていません",
        Some(pos),
        Some(line),
        Some(col),
        line_map.line_text(src, line).to_string(),
    ))
}

/// 単純なトークンをベクタへ追加するヘルパ。
fn push_simple_token(
    toks: &mut Vec<Token>,
    kind: TokenKind,
    value: &str,
    pos: usize,
    line_map: &LineMap,
    src: &str,
) {
    let (line, col) = line_map.locate(src, pos);
    toks.push(Token {
        kind,
        value: value.into(),
        pos,
        line,
        col,
    });
}

/// ソースコードを走査し、位置付きトークン列を返す。
pub fn lex(src: &str) -> Result<Vec<Token>, LexerError> {
    let mut toks: Vec<Token> = Vec::new();
    let mut i = 0usize;
    let bytes = src.as_bytes();
    let n = bytes.len();
    let next_char = |i: usize| -> Option<char> { src[i..].chars().next() };
    let line_map = LineMap::new(src);
    while i < n {
        if skip_whitespace(src, &mut i) {
            continue;
        }
        if i >= n {
            break;
        }
        if skip_block_comment(src, &mut i, &line_map)? {
            continue;
        }
        if i >= n {
            break;
        }
        if skip_line_comment(src, &mut i) {
            continue;
        }
        if i >= n {
            break;
        }

        let Some(c) = next_char(i) else { break };
        let c_next_idx = i + c.len_utf8();

        // 2 文字で構成される演算子や記号を処理する
        if c_next_idx <= n {
            if let Some(c2) = next_char(c_next_idx) {
                match (c, c2) {
                    ('-', '>') => {
                        push_simple_token(&mut toks, TokenKind::ARROW, "->", i, &line_map, src);
                        i = c_next_idx + c2.len_utf8();
                        continue;
                    }
                    (':', ':') => {
                        push_simple_token(&mut toks, TokenKind::DCOLON, "::", i, &line_map, src);
                        i = c_next_idx + c2.len_utf8();
                        continue;
                    }
                    ('=', '>') => {
                        push_simple_token(&mut toks, TokenKind::DARROW, "=>", i, &line_map, src);
                        i = c_next_idx + c2.len_utf8();
                        continue;
                    }
                    ('<', '=') => {
                        push_simple_token(&mut toks, TokenKind::LE, "<=", i, &line_map, src);
                        i = c_next_idx + c2.len_utf8();
                        continue;
                    }
                    ('>', '=') => {
                        push_simple_token(&mut toks, TokenKind::GE, ">=", i, &line_map, src);
                        i = c_next_idx + c2.len_utf8();
                        continue;
                    }
                    ('=', '=') => {
                        push_simple_token(&mut toks, TokenKind::EQ, "==", i, &line_map, src);
                        i = c_next_idx + c2.len_utf8();
                        continue;
                    }
                    ('/', '=') => {
                        push_simple_token(&mut toks, TokenKind::NE, "/=", i, &line_map, src);
                        i = c_next_idx + c2.len_utf8();
                        continue;
                    }
                    ('*', '*') => {
                        push_simple_token(&mut toks, TokenKind::DBLSTAR, "**", i, &line_map, src);
                        i = c_next_idx + c2.len_utf8();
                        continue;
                    }
                    _ => {}
                }
            }
        }

        // 1 文字記号
        match c {
            '\\' => {
                push_simple_token(&mut toks, TokenKind::LAMBDA, "\\", i, &line_map, src);
                i += 1;
                continue;
            }
            '<' => {
                push_simple_token(&mut toks, TokenKind::LT, "<", i, &line_map, src);
                i += 1;
                continue;
            }
            '>' => {
                push_simple_token(&mut toks, TokenKind::GT, ">", i, &line_map, src);
                i += 1;
                continue;
            }
            '+' => {
                push_simple_token(&mut toks, TokenKind::PLUS, "+", i, &line_map, src);
                i += 1;
                continue;
            }
            '-' => {
                push_simple_token(&mut toks, TokenKind::MINUS, "-", i, &line_map, src);
                i += 1;
                continue;
            }
            '*' => {
                push_simple_token(&mut toks, TokenKind::STAR, "*", i, &line_map, src);
                i += 1;
                continue;
            }
            '/' => {
                push_simple_token(&mut toks, TokenKind::SLASH, "/", i, &line_map, src);
                i += 1;
                continue;
            }
            '^' => {
                push_simple_token(&mut toks, TokenKind::CARET, "^", i, &line_map, src);
                i += 1;
                continue;
            }
            '(' => {
                push_simple_token(&mut toks, TokenKind::LPAREN, "(", i, &line_map, src);
                i += 1;
                continue;
            }
            ')' => {
                push_simple_token(&mut toks, TokenKind::RPAREN, ")", i, &line_map, src);
                i += 1;
                continue;
            }
            '[' => {
                push_simple_token(&mut toks, TokenKind::LBRACK, "[", i, &line_map, src);
                i += 1;
                continue;
            }
            ']' => {
                push_simple_token(&mut toks, TokenKind::RBRACK, "]", i, &line_map, src);
                i += 1;
                continue;
            }
            ',' => {
                push_simple_token(&mut toks, TokenKind::COMMA, ",", i, &line_map, src);
                i += 1;
                continue;
            }
            ';' => {
                push_simple_token(&mut toks, TokenKind::SEMI, ";", i, &line_map, src);
                i += 1;
                continue;
            }
            '=' => {
                push_simple_token(&mut toks, TokenKind::EQUAL, "=", i, &line_map, src);
                i += 1;
                continue;
            }
            '|' => {
                push_simple_token(&mut toks, TokenKind::BAR, "|", i, &line_map, src);
                i += 1;
                continue;
            }
            '?' => {
                push_simple_token(&mut toks, TokenKind::QMARK, "?", i, &line_map, src);
                i += 1;
                continue;
            }
            '_' => {
                push_simple_token(&mut toks, TokenKind::UNDERSCORE, "_", i, &line_map, src);
                i += 1;
                continue;
            }
            '\'' => {
                // 単一文字またはエスケープで構成される文字リテラル
                let start = i;
                i += 1; // 先頭のシングルクォートを読み飛ばす
                let mut escaped = false;
                let mut ok = false;
                while i < n {
                    let ch =
                        next_char(i).expect("安全性保証: 文字リテラル走査中に EOF へ到達しない");
                    i += ch.len_utf8();
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
                        // エスケープ済みなので通常処理へ戻す
                        escaped = false;
                    }
                }
                if !ok {
                    let (l, c) = line_map.locate(src, start);
                    return Err(LexerError::at_with_snippet(
                        "LEX002",
                        "文字リテラルが閉じていません",
                        Some(start),
                        Some(l),
                        Some(c),
                        line_map.line_text(src, l).to_string(),
                    ));
                }
                let s = &src[start..i];
                let (l, c) = line_map.locate(src, start);
                toks.push(Token {
                    kind: TokenKind::CHAR,
                    value: s.into(),
                    pos: start,
                    line: l,
                    col: c,
                });
                continue;
            }
            '"' => {
                // 文字列リテラルの読み取り
                let start = i;
                i += 1;
                let mut escaped = false;
                let mut ok = false;
                while i < n {
                    let ch =
                        next_char(i).expect("安全性保証: 文字列リテラル走査中に EOF へ到達しない");
                    i += ch.len_utf8();
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
                    let (l, c) = line_map.locate(src, start);
                    return Err(LexerError::at_with_snippet(
                        "LEX003",
                        "文字列リテラルが閉じていません",
                        Some(start),
                        Some(l),
                        Some(c),
                        line_map.line_text(src, l).to_string(),
                    ));
                }
                let s = &src[start..i];
                let (l, c) = line_map.locate(src, start);
                toks.push(Token {
                    kind: TokenKind::STRING,
                    value: s.into(),
                    pos: start,
                    line: l,
                    col: c,
                });
                continue;
            }
            _ => {}
        }

        // 数値リテラル（基数接頭辞・浮動小数・整数）を認識する
        if is_digit(c) {
            // プレフィックスで基数を切り替える
            if c == '0' {
                if let Some('x') | Some('X') = next_char(c_next_idx) {
                    let start = i;
                    i = c_next_idx + 'x'.len_utf8(); // `0x` を読み飛ばす
                    let mut cnt = 0;
                    while i < n {
                        let ch = next_char(i)
                            .expect("安全性保証: 16 進リテラル走査中に EOF へ到達しない");
                        if is_hexdigit(ch) {
                            i += ch.len_utf8();
                            cnt += 1;
                        } else {
                            break;
                        }
                    }
                    if cnt == 0 {
                        let (l, c) = line_map.locate(src, start);
                        return Err(LexerError::at_with_snippet(
                            "LEX010",
                            "16進数の桁がありません",
                            Some(start),
                            Some(l),
                            Some(c),
                            line_map.line_text(src, l).to_string(),
                        ));
                    }
                    let (l, c) = line_map.locate(src, start);
                    toks.push(Token {
                        kind: TokenKind::HEX,
                        value: src[start..i].into(),
                        pos: start,
                        line: l,
                        col: c,
                    });
                    continue;
                }
                if let Some('o') | Some('O') = next_char(c_next_idx) {
                    let start = i;
                    i = c_next_idx + 'o'.len_utf8(); // `0o` を読み飛ばす
                    let mut cnt = 0;
                    while i < n {
                        let ch = next_char(i)
                            .expect("安全性保証: 8 進リテラル走査中に EOF へ到達しない");
                        if is_octdigit(ch) {
                            i += ch.len_utf8();
                            cnt += 1;
                        } else {
                            break;
                        }
                    }
                    if cnt == 0 {
                        let (l, c) = line_map.locate(src, start);
                        return Err(LexerError::at_with_snippet(
                            "LEX011",
                            "8進数の桁がありません",
                            Some(start),
                            Some(l),
                            Some(c),
                            line_map.line_text(src, l).to_string(),
                        ));
                    }
                    let (l, c) = line_map.locate(src, start);
                    toks.push(Token {
                        kind: TokenKind::OCT,
                        value: src[start..i].into(),
                        pos: start,
                        line: l,
                        col: c,
                    });
                    continue;
                }
                if let Some('b') | Some('B') = next_char(c_next_idx) {
                    let start = i;
                    i = c_next_idx + 'b'.len_utf8(); // `0b` を読み飛ばす
                    let mut cnt = 0;
                    while i < n {
                        let ch = next_char(i)
                            .expect("安全性保証: 2 進リテラル走査中に EOF へ到達しない");
                        if is_bindigit(ch) {
                            i += ch.len_utf8();
                            cnt += 1;
                        } else {
                            break;
                        }
                    }
                    if cnt == 0 {
                        let (l, c) = line_map.locate(src, start);
                        return Err(LexerError::at_with_snippet(
                            "LEX012",
                            "2進数の桁がありません",
                            Some(start),
                            Some(l),
                            Some(c),
                            line_map.line_text(src, l).to_string(),
                        ));
                    }
                    let (l, c) = line_map.locate(src, start);
                    toks.push(Token {
                        kind: TokenKind::BIN,
                        value: src[start..i].into(),
                        pos: start,
                        line: l,
                        col: c,
                    });
                    continue;
                }
            }
            // 10 進数（整数・小数・指数表記）を処理する
            let start = i;
            // 整数部を取り込む
            while i < n {
                let ch = next_char(i).expect("安全性保証: 10 進整数部走査中に EOF へ到達しない");
                if is_digit(ch) {
                    i += ch.len_utf8();
                } else {
                    break;
                }
            }
            // 小数部や指数部が続くかを判断する
            let mut is_float = false;
            if i < n {
                if let Some('.') = next_char(i) {
                    let dot_next = i + '.'.len_utf8();
                    if let Some(d2) = next_char(dot_next) {
                        if d2.is_ascii_digit() {
                            // 小数部あり（例: `d.d`）
                            is_float = true;
                            i = dot_next; // '.' を含む位置から小数部へ進む
                            while i < n {
                                let ch = next_char(i)
                                    .expect("安全性保証: 小数部走査中に EOF へ到達しない");
                                if is_digit(ch) {
                                    i += ch.len_utf8();
                                } else {
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            // 指数部（e/E 記法）が続く場合は解析する
            if i < n {
                if let Some('e') | Some('E') = next_char(i) {
                    let mut j = i + 'e'.len_utf8();
                    if let Some('+') | Some('-') = next_char(j) {
                        j += '+'.len_utf8();
                    }
                    let mut cnt = 0;
                    while j < n {
                        let ch = next_char(j).expect("安全性保証: 指数部走査中に EOF へ到達しない");
                        if is_digit(ch) {
                            j += ch.len_utf8();
                            cnt += 1;
                        } else {
                            break;
                        }
                    }
                    if cnt > 0 {
                        i = j;
                        is_float = true;
                    } else { /* 例: 1e のように指数部が欠ける場合は整数扱い */
                    }
                }
            }
            let s = &src[start..i];
            let kind = if is_float {
                TokenKind::FLOAT
            } else {
                TokenKind::INT
            };
            let (l, c) = line_map.locate(src, start);
            toks.push(Token {
                kind,
                value: s.into(),
                pos: start,
                line: l,
                col: c,
            });
            continue;
        }

        // 識別子（大文字開始は CONID、小文字開始は VARID）を解析する
        if is_letter(c) {
            let start = i;
            i += c.len_utf8();
            while i < n {
                let ch = next_char(i).expect("安全性保証: 識別子走査中に EOF へ到達しない");
                if is_ident_rest(ch) {
                    i += ch.len_utf8();
                } else {
                    break;
                }
            }
            let s = &src[start..i];
            // 予約語かどうかを振り分ける
            let (line, col) = line_map.locate(src, start);
            let token = match s {
                "let" => Token {
                    kind: TokenKind::LET,
                    value: s.into(),
                    pos: start,
                    line,
                    col,
                },
                "in" => Token {
                    kind: TokenKind::IN,
                    value: s.into(),
                    pos: start,
                    line,
                    col,
                },
                "if" => Token {
                    kind: TokenKind::IF,
                    value: s.into(),
                    pos: start,
                    line,
                    col,
                },
                "then" => Token {
                    kind: TokenKind::THEN,
                    value: s.into(),
                    pos: start,
                    line,
                    col,
                },
                "else" => Token {
                    kind: TokenKind::ELSE,
                    value: s.into(),
                    pos: start,
                    line,
                    col,
                },
                "case" => Token {
                    kind: TokenKind::CASE,
                    value: s.into(),
                    pos: start,
                    line,
                    col,
                },
                "of" => Token {
                    kind: TokenKind::OF,
                    value: s.into(),
                    pos: start,
                    line,
                    col,
                },
                "data" => Token {
                    kind: TokenKind::DATA,
                    value: s.into(),
                    pos: start,
                    line,
                    col,
                },
                "True" => Token {
                    kind: TokenKind::TRUE,
                    value: s.into(),
                    pos: start,
                    line,
                    col,
                },
                "False" => Token {
                    kind: TokenKind::FALSE,
                    value: s.into(),
                    pos: start,
                    line,
                    col,
                },
                _ => {
                    let first = s
                        .chars()
                        .next()
                        .expect("識別子は少なくとも1文字である必要があります");
                    if first.is_ascii_uppercase() {
                        Token {
                            kind: TokenKind::CONID,
                            value: s.into(),
                            pos: start,
                            line,
                            col,
                        }
                    } else {
                        Token {
                            kind: TokenKind::VARID,
                            value: s.into(),
                            pos: start,
                            line,
                            col,
                        }
                    }
                }
            };
            toks.push(token);
            continue;
        }

        let (l, c2) = line_map.locate(src, i);
        return Err(LexerError::at_with_snippet(
            "LEX090",
            format!("字句解析に失敗: {:?}", c),
            Some(i),
            Some(l),
            Some(c2),
            line_map.line_text(src, l).to_string(),
        ));
    }
    push_simple_token(&mut toks, TokenKind::EOF, "", n, &line_map, src);
    Ok(toks)
}
