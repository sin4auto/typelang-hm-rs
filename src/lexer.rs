//! 字句解析（lexer）
//!
//! 目的:
//! - EBNFに従ってソース文字列をトークン列に変換する。
//!
//! 方針/制約:
//! - 標準ライブラリのみ（正規表現ライブラリは不使用）。
//! - UTF-8 前提。コードポイント単位で走査し、2文字演算子やコメント終端を安全に検出。
//! - エラーは一意なコード（`LEX***`）と位置（バイトオフセット）で報告。

use crate::errors::LexerError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub value: String,
    pub pos: usize,
    pub line: usize,
    pub col: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    EOF,
    // 記号/演算子
    LAMBDA, // '\\'
    ARROW,  // '->'
    DCOLON, // '::'
    DARROW, // '=>'
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
    // リテラル
    CHAR,
    STRING,
    HEX,
    OCT,
    BIN,
    FLOAT,
    INT,
    // 識別子
    CONID,
    VARID,
    // キーワード
    LET,
    IN,
    IF,
    THEN,
    ELSE,
    TRUE,
    FALSE,
}

fn is_whitespace(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\r' | '\n')
}
fn is_digit(c: char) -> bool {
    c.is_ascii_digit()
}
fn is_hexdigit(c: char) -> bool {
    c.is_ascii_hexdigit()
}
fn is_octdigit(c: char) -> bool {
    matches!(c, '0'..='7')
}
fn is_bindigit(c: char) -> bool {
    matches!(c, '0' | '1')
}
fn is_letter(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}
fn is_ident_rest(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '\''
}

pub fn lex(src: &str) -> Result<Vec<Token>, LexerError> {
    let mut toks: Vec<Token> = Vec::new();
    let mut i = 0usize;
    let bytes = src.as_bytes();
    let n = bytes.len();
    let next_char = |i: usize| -> Option<char> { src[i..].chars().next() };
    fn line_col_at(src: &str, pos: usize) -> (usize, usize) {
        let mut line = 1usize;
        let mut col = 1usize;
        for (bpos, ch) in src.char_indices() {
            if bpos >= pos {
                break;
            }
            if ch == '\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
        }
        (line, col)
    }
    fn line_text_at(src: &str, line: usize) -> String {
        src.lines()
            .nth(line.saturating_sub(1))
            .unwrap_or("")
            .to_string()
    }
    while i < n {
        let Some(c) = next_char(i) else { break };
        let c_next_idx = i + c.len_utf8();
        // 空白
        if is_whitespace(c) {
            i += c.len_utf8();
            continue;
        }

        // ブロックコメント "{-" ... "-}"（入れ子は非対応）
        if c == '{' {
            if let Some('-') = next_char(c_next_idx) {
                // 注意: これは UTF-8 長を厳密に扱わないが、ASCII のみ
                // 先頭の "{-"
                i = c_next_idx + '-'.len_utf8(); // '{' と '-'
                                                 // 終了トークン "-}"
                let mut closed = false;
                while i < n {
                    if let Some('-') = next_char(i) {
                        let dash_next = i + '-'.len_utf8();
                        if let Some('}') = next_char(dash_next) {
                            i = dash_next + '}'.len_utf8();
                            closed = true;
                            break;
                        }
                    }
                    i += next_char(i).map(|ch| ch.len_utf8()).unwrap_or(1);
                }
                if !closed {
                    let (l, c) = line_col_at(src, i);
                    return Err(LexerError::at_with_snippet(
                        "LEX001",
                        "ブロックコメントが閉じていません",
                        Some(i),
                        Some(l),
                        Some(c),
                        line_text_at(src, l),
                    ));
                }
                continue;
            }
        }

        // 行コメント "-- ... \n"
        if c == '-' {
            if let Some('-') = next_char(c_next_idx) {
                // "--"
                // 改行まで飛ばす
                i = c_next_idx + '-'.len_utf8();
                while i < n {
                    // 内部不変: i < n のため next_char(i) は必ず存在する
                    let ch =
                        next_char(i).expect("内部不変違反: 行コメント走査中にEOFに到達しました");
                    i += ch.len_utf8();
                    if ch == '\n' {
                        break;
                    }
                }
                continue;
            }
        }

        // 2文字記号（UTF-8安全）
        if c_next_idx <= n {
            if let Some(c2) = next_char(c_next_idx) {
                match (c, c2) {
                    ('-', '>') => {
                        let (l, c) = line_col_at(src, i);
                        toks.push(Token {
                            kind: TokenKind::ARROW,
                            value: "->".into(),
                            pos: i,
                            line: l,
                            col: c,
                        });
                        i = c_next_idx + c2.len_utf8();
                        continue;
                    }
                    (':', ':') => {
                        let (l, c) = line_col_at(src, i);
                        toks.push(Token {
                            kind: TokenKind::DCOLON,
                            value: "::".into(),
                            pos: i,
                            line: l,
                            col: c,
                        });
                        i = c_next_idx + c2.len_utf8();
                        continue;
                    }
                    ('=', '>') => {
                        let (l, c) = line_col_at(src, i);
                        toks.push(Token {
                            kind: TokenKind::DARROW,
                            value: "=>".into(),
                            pos: i,
                            line: l,
                            col: c,
                        });
                        i = c_next_idx + c2.len_utf8();
                        continue;
                    }
                    ('<', '=') => {
                        let (l, c) = line_col_at(src, i);
                        toks.push(Token {
                            kind: TokenKind::LE,
                            value: "<=".into(),
                            pos: i,
                            line: l,
                            col: c,
                        });
                        i = c_next_idx + c2.len_utf8();
                        continue;
                    }
                    ('>', '=') => {
                        let (l, c) = line_col_at(src, i);
                        toks.push(Token {
                            kind: TokenKind::GE,
                            value: ">=".into(),
                            pos: i,
                            line: l,
                            col: c,
                        });
                        i = c_next_idx + c2.len_utf8();
                        continue;
                    }
                    ('=', '=') => {
                        let (l, c) = line_col_at(src, i);
                        toks.push(Token {
                            kind: TokenKind::EQ,
                            value: "==".into(),
                            pos: i,
                            line: l,
                            col: c,
                        });
                        i = c_next_idx + c2.len_utf8();
                        continue;
                    }
                    ('/', '=') => {
                        let (l, c) = line_col_at(src, i);
                        toks.push(Token {
                            kind: TokenKind::NE,
                            value: "/=".into(),
                            pos: i,
                            line: l,
                            col: c,
                        });
                        i = c_next_idx + c2.len_utf8();
                        continue;
                    }
                    ('*', '*') => {
                        let (l, c) = line_col_at(src, i);
                        toks.push(Token {
                            kind: TokenKind::DBLSTAR,
                            value: "**".into(),
                            pos: i,
                            line: l,
                            col: c,
                        });
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
                let (l, c) = line_col_at(src, i);
                toks.push(Token {
                    kind: TokenKind::LAMBDA,
                    value: "\\".into(),
                    pos: i,
                    line: l,
                    col: c,
                });
                i += 1;
                continue;
            }
            '<' => {
                let (l, c) = line_col_at(src, i);
                toks.push(Token {
                    kind: TokenKind::LT,
                    value: "<".into(),
                    pos: i,
                    line: l,
                    col: c,
                });
                i += 1;
                continue;
            }
            '>' => {
                let (l, c) = line_col_at(src, i);
                toks.push(Token {
                    kind: TokenKind::GT,
                    value: ">".into(),
                    pos: i,
                    line: l,
                    col: c,
                });
                i += 1;
                continue;
            }
            '+' => {
                let (l, c) = line_col_at(src, i);
                toks.push(Token {
                    kind: TokenKind::PLUS,
                    value: "+".into(),
                    pos: i,
                    line: l,
                    col: c,
                });
                i += 1;
                continue;
            }
            '-' => {
                let (l, c) = line_col_at(src, i);
                toks.push(Token {
                    kind: TokenKind::MINUS,
                    value: "-".into(),
                    pos: i,
                    line: l,
                    col: c,
                });
                i += 1;
                continue;
            }
            '*' => {
                let (l, c) = line_col_at(src, i);
                toks.push(Token {
                    kind: TokenKind::STAR,
                    value: "*".into(),
                    pos: i,
                    line: l,
                    col: c,
                });
                i += 1;
                continue;
            }
            '/' => {
                let (l, c) = line_col_at(src, i);
                toks.push(Token {
                    kind: TokenKind::SLASH,
                    value: "/".into(),
                    pos: i,
                    line: l,
                    col: c,
                });
                i += 1;
                continue;
            }
            '^' => {
                let (l, c) = line_col_at(src, i);
                toks.push(Token {
                    kind: TokenKind::CARET,
                    value: "^".into(),
                    pos: i,
                    line: l,
                    col: c,
                });
                i += 1;
                continue;
            }
            '(' => {
                let (l, c) = line_col_at(src, i);
                toks.push(Token {
                    kind: TokenKind::LPAREN,
                    value: "(".into(),
                    pos: i,
                    line: l,
                    col: c,
                });
                i += 1;
                continue;
            }
            ')' => {
                let (l, c) = line_col_at(src, i);
                toks.push(Token {
                    kind: TokenKind::RPAREN,
                    value: ")".into(),
                    pos: i,
                    line: l,
                    col: c,
                });
                i += 1;
                continue;
            }
            '[' => {
                let (l, c) = line_col_at(src, i);
                toks.push(Token {
                    kind: TokenKind::LBRACK,
                    value: "[".into(),
                    pos: i,
                    line: l,
                    col: c,
                });
                i += 1;
                continue;
            }
            ']' => {
                let (l, c) = line_col_at(src, i);
                toks.push(Token {
                    kind: TokenKind::RBRACK,
                    value: "]".into(),
                    pos: i,
                    line: l,
                    col: c,
                });
                i += 1;
                continue;
            }
            ',' => {
                let (l, c) = line_col_at(src, i);
                toks.push(Token {
                    kind: TokenKind::COMMA,
                    value: ",".into(),
                    pos: i,
                    line: l,
                    col: c,
                });
                i += 1;
                continue;
            }
            ';' => {
                let (l, c) = line_col_at(src, i);
                toks.push(Token {
                    kind: TokenKind::SEMI,
                    value: ";".into(),
                    pos: i,
                    line: l,
                    col: c,
                });
                i += 1;
                continue;
            }
            '=' => {
                let (l, c) = line_col_at(src, i);
                toks.push(Token {
                    kind: TokenKind::EQUAL,
                    value: "=".into(),
                    pos: i,
                    line: l,
                    col: c,
                });
                i += 1;
                continue;
            }
            '?' => {
                let (l, c) = line_col_at(src, i);
                toks.push(Token {
                    kind: TokenKind::QMARK,
                    value: "?".into(),
                    pos: i,
                    line: l,
                    col: c,
                });
                i += 1;
                continue;
            }
            '_' => {
                let (l, c) = line_col_at(src, i);
                toks.push(Token {
                    kind: TokenKind::UNDERSCORE,
                    value: "_".into(),
                    pos: i,
                    line: l,
                    col: c,
                });
                i += 1;
                continue;
            }
            '\'' => {
                // 文字リテラル（単一文字 or エスケープ）
                let start = i;
                i += 1; // opening '
                let mut escaped = false;
                let mut ok = false;
                while i < n {
                    // 内部不変: i < n のため next_char(i) は必ず存在する
                    let ch =
                        next_char(i).expect("内部不変違反: 文字リテラル走査中にEOFに到達しました");
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
                        // 1 文字エスケープを許容
                        escaped = false;
                    }
                }
                if !ok {
                    let (l, c) = line_col_at(src, start);
                    return Err(LexerError::at_with_snippet(
                        "LEX002",
                        "文字リテラルが閉じていません",
                        Some(start),
                        Some(l),
                        Some(c),
                        line_text_at(src, l),
                    ));
                }
                let s = &src[start..i];
                let (l, c) = line_col_at(src, start);
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
                // 文字列リテラル
                let start = i;
                i += 1;
                let mut escaped = false;
                let mut ok = false;
                while i < n {
                    // 内部不変: i < n のため next_char(i) は必ず存在する
                    let ch = next_char(i)
                        .expect("内部不変違反: 文字列リテラル走査中にEOFに到達しました");
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
                    let (l, c) = line_col_at(src, start);
                    return Err(LexerError::at_with_snippet(
                        "LEX003",
                        "文字列リテラルが閉じていません",
                        Some(start),
                        Some(l),
                        Some(c),
                        line_text_at(src, l),
                    ));
                }
                let s = &src[start..i];
                let (l, c) = line_col_at(src, start);
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

        // 数値: 0x, 0o, 0b, 浮動小数, 整数
        if is_digit(c) {
            // 進数接頭辞
            if c == '0' {
                if let Some('x') | Some('X') = next_char(c_next_idx) {
                    let start = i;
                    i = c_next_idx + 'x'.len_utf8(); // 0x
                    let mut cnt = 0;
                    while i < n {
                        // 内部不変: i < n のため next_char(i) は必ず存在する
                        let ch = next_char(i)
                            .expect("内部不変違反: 16進リテラル走査中にEOFに到達しました");
                        if is_hexdigit(ch) {
                            i += ch.len_utf8();
                            cnt += 1;
                        } else {
                            break;
                        }
                    }
                    if cnt == 0 {
                        let (l, c) = line_col_at(src, start);
                        return Err(LexerError::at_with_snippet(
                            "LEX010",
                            "16進数の桁がありません",
                            Some(start),
                            Some(l),
                            Some(c),
                            line_text_at(src, l),
                        ));
                    }
                    let (l, c) = line_col_at(src, start);
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
                    i = c_next_idx + 'o'.len_utf8();
                    let mut cnt = 0;
                    while i < n {
                        // 内部不変: i < n のため next_char(i) は必ず存在する
                        let ch = next_char(i)
                            .expect("内部不変違反: 8進リテラル走査中にEOFに到達しました");
                        if is_octdigit(ch) {
                            i += ch.len_utf8();
                            cnt += 1;
                        } else {
                            break;
                        }
                    }
                    if cnt == 0 {
                        let (l, c) = line_col_at(src, start);
                        return Err(LexerError::at_with_snippet(
                            "LEX011",
                            "8進数の桁がありません",
                            Some(start),
                            Some(l),
                            Some(c),
                            line_text_at(src, l),
                        ));
                    }
                    let (l, c) = line_col_at(src, start);
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
                    i = c_next_idx + 'b'.len_utf8();
                    let mut cnt = 0;
                    while i < n {
                        // 内部不変: i < n のため next_char(i) は必ず存在する
                        let ch = next_char(i)
                            .expect("内部不変違反: 2進リテラル走査中にEOFに到達しました");
                        if is_bindigit(ch) {
                            i += ch.len_utf8();
                            cnt += 1;
                        } else {
                            break;
                        }
                    }
                    if cnt == 0 {
                        let (l, c) = line_col_at(src, start);
                        return Err(LexerError::at_with_snippet(
                            "LEX012",
                            "2進数の桁がありません",
                            Some(start),
                            Some(l),
                            Some(c),
                            line_text_at(src, l),
                        ));
                    }
                    let (l, c) = line_col_at(src, start);
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
            // 10進: 浮動小数/指数 or 整数
            let start = i;
            // 整数部
            while i < n {
                // 内部不変: i < n のため next_char(i) は必ず存在する
                let ch = next_char(i).expect("内部不変違反: 10進整数部走査中にEOFに到達しました");
                if is_digit(ch) {
                    i += ch.len_utf8();
                } else {
                    break;
                }
            }
            // 小数/指数の判定
            let mut is_float = false;
            if i < n {
                if let Some('.') = next_char(i) {
                    let dot_next = i + '.'.len_utf8();
                    if let Some(d2) = next_char(dot_next) {
                        if d2.is_ascii_digit() {
                            // "d.d"
                            is_float = true;
                            i = dot_next; // '.'
                            while i < n {
                                // 内部不変: i < n のため next_char(i) は必ず存在する
                                let ch = next_char(i).expect(
                                    "内部不変違反: 浮動小数点の小数部走査中にEOFに到達しました",
                                );
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
            // 指数部
            if i < n {
                if let Some('e') | Some('E') = next_char(i) {
                    let mut j = i + 'e'.len_utf8();
                    if let Some('+') | Some('-') = next_char(j) {
                        j += '+'.len_utf8();
                    }
                    let mut cnt = 0;
                    while j < n {
                        // 内部不変: j < n のため next_char(j) は必ず存在する
                        let ch =
                            next_char(j).expect("内部不変違反: 指数部走査中にEOFに到達しました");
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
                    } else { /* 例: 1e -> ここでは指数を解釈しない */
                    }
                }
            }
            let s = &src[start..i];
            let kind = if is_float {
                TokenKind::FLOAT
            } else {
                TokenKind::INT
            };
            let (l, c) = line_col_at(src, start);
            toks.push(Token {
                kind,
                value: s.into(),
                pos: start,
                line: l,
                col: c,
            });
            continue;
        }

        // 識別子（CONID/VARID）
        if is_letter(c) {
            let start = i;
            i += c.len_utf8();
            while i < n {
                // 内部不変: i < n のため next_char(i) は必ず存在する
                let ch = next_char(i).expect("内部不変違反: 識別子走査中にEOFに到達しました");
                if is_ident_rest(ch) {
                    i += ch.len_utf8();
                } else {
                    break;
                }
            }
            let s = &src[start..i];
            // 予約語判定
            let token = match s {
                "let" => Token {
                    kind: TokenKind::LET,
                    value: s.into(),
                    pos: start,
                    line: line_col_at(src, start).0,
                    col: line_col_at(src, start).1,
                },
                "in" => Token {
                    kind: TokenKind::IN,
                    value: s.into(),
                    pos: start,
                    line: line_col_at(src, start).0,
                    col: line_col_at(src, start).1,
                },
                "if" => Token {
                    kind: TokenKind::IF,
                    value: s.into(),
                    pos: start,
                    line: line_col_at(src, start).0,
                    col: line_col_at(src, start).1,
                },
                "then" => Token {
                    kind: TokenKind::THEN,
                    value: s.into(),
                    pos: start,
                    line: line_col_at(src, start).0,
                    col: line_col_at(src, start).1,
                },
                "else" => Token {
                    kind: TokenKind::ELSE,
                    value: s.into(),
                    pos: start,
                    line: line_col_at(src, start).0,
                    col: line_col_at(src, start).1,
                },
                "True" => Token {
                    kind: TokenKind::TRUE,
                    value: s.into(),
                    pos: start,
                    line: line_col_at(src, start).0,
                    col: line_col_at(src, start).1,
                },
                "False" => Token {
                    kind: TokenKind::FALSE,
                    value: s.into(),
                    pos: start,
                    line: line_col_at(src, start).0,
                    col: line_col_at(src, start).1,
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
                            line: line_col_at(src, start).0,
                            col: line_col_at(src, start).1,
                        }
                    } else {
                        Token {
                            kind: TokenKind::VARID,
                            value: s.into(),
                            pos: start,
                            line: line_col_at(src, start).0,
                            col: line_col_at(src, start).1,
                        }
                    }
                }
            };
            toks.push(token);
            continue;
        }

        let (l, c2) = line_col_at(src, i);
        return Err(LexerError::at_with_snippet(
            "LEX090",
            format!("字句解析に失敗: {:?}", c),
            Some(i),
            Some(l),
            Some(c2),
            line_text_at(src, l),
        ));
    }
    let (l, c) = line_col_at(src, n);
    toks.push(Token {
        kind: TokenKind::EOF,
        value: String::new(),
        pos: n,
        line: l,
        col: c,
    });
    Ok(toks)
}
