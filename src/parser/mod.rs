// パス: src/parser/mod.rs
// 役割: トークン列から AST を生成する再帰下降パーサのエントリポイント
// 意図: 字句解析結果を型推論・評価に渡すためのモジュール構成を整理する
// 関連ファイル: src/parser/program.rs, src/parser/expr.rs, src/parser/types.rs
//! 構文解析モジュール
//!
//! - EBNF で定義された文法に従ってプログラム・式・型注釈を解析する。
//! - 演算子の結合規則・優先順位は `cmp > add > mul > pow > app` の順でハンドコードする。
//! - 単項マイナスなどの糖衣は `0 - x` など正規化した AST へ変換する。

use crate::ast::{
    CaseArm, Constraint as AConstraint, DataConstructor, DataDecl, Expr, IntBase, Pattern, Program,
    SigmaType, Span, TopLevel, TypeExpr,
};
use crate::errors::ParseError;
use crate::lexer::{lex, Token, TokenKind};

mod expr;
mod program;
mod types;

pub struct Parser {
    ts: Vec<Token>,
    i: usize,
}

#[derive(Clone, Copy)]
pub(super) enum Assoc {
    Left,
    Right,
    Non,
}

pub(super) struct InfixSpec {
    pub tokens: &'static [TokenKind],
    pub assoc: Assoc,
}

impl InfixSpec {
    pub(super) fn contains(&self, kind: &TokenKind) -> bool {
        self.tokens.iter().any(|tk| tk == kind)
    }
}

pub(super) const INFIX_LEVELS: &[InfixSpec] = &[
    InfixSpec {
        tokens: &[
            TokenKind::EQ,
            TokenKind::NE,
            TokenKind::LT,
            TokenKind::LE,
            TokenKind::GT,
            TokenKind::GE,
        ],
        assoc: Assoc::Non,
    },
    InfixSpec {
        tokens: &[TokenKind::PLUS, TokenKind::MINUS],
        assoc: Assoc::Left,
    },
    InfixSpec {
        tokens: &[TokenKind::STAR, TokenKind::SLASH],
        assoc: Assoc::Left,
    },
    InfixSpec {
        tokens: &[TokenKind::CARET, TokenKind::DBLSTAR],
        assoc: Assoc::Right,
    },
];

impl Parser {
    /// トークン列から新しいパーサインスタンスを構築する。
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { ts: tokens, i: 0 }
    }

    pub(super) fn peek(&self) -> &Token {
        &self.ts[self.i]
    }

    pub(super) fn pop_any(&mut self) -> Token {
        let t = self.ts[self.i].clone();
        self.i += 1;
        t
    }

    pub(super) fn peek_kind(&self, offset: usize) -> Option<TokenKind> {
        self.ts.get(self.i + offset).map(|t| t.kind.clone())
    }

    pub(super) fn pop(&mut self, kind: TokenKind) -> Result<Token, ParseError> {
        let t = self.peek().clone();
        if t.kind != kind {
            return Err(ParseError::at(
                "PAR001",
                format!("{:?} を期待しましたが {:?} ({})", kind, t.kind, t.value),
                Some(t.pos),
                Some(t.line),
                Some(t.col),
            ));
        }
        self.i += 1;
        Ok(t)
    }

    pub(super) fn accept(&mut self, kind: TokenKind) -> Option<Token> {
        if self.peek().kind == kind {
            let t = self.pop_any();
            Some(t)
        } else {
            None
        }
    }
}

pub(super) fn span_from_token(token: &Token) -> Span {
    Span::new(token.pos, token.line, token.col)
}

pub(super) fn decode_string(quoted: &str) -> Result<String, ParseError> {
    if !quoted.starts_with('"') || !quoted.ends_with('"') {
        return Err(ParseError::new("PAR201", "文字列リテラルが不正", None));
    }
    let s = &quoted[1..quoted.len() - 1];
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            let Some(e) = chars.next() else {
                return Err(ParseError::new("PAR202", "末尾のバックスラッシュ", None));
            };
            match e {
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                '\\' => out.push('\\'),
                '\'' => out.push('\''),
                '"' => out.push('"'),
                _ => out.push(e),
            }
        } else {
            out.push(ch);
        }
    }
    Ok(out)
}

pub(super) fn decode_char(quoted: &str) -> Result<char, ParseError> {
    if !quoted.starts_with('\'') || !quoted.ends_with('\'') {
        return Err(ParseError::new("PAR204", "文字リテラルが不正", None));
    }
    let s = &quoted[1..quoted.len() - 1];
    let ch = if s.starts_with('\\') {
        match s.chars().nth(1) {
            Some('n') => '\n',
            Some('r') => '\r',
            Some('t') => '\t',
            Some('\\') => '\\',
            Some('\'') => '\'',
            Some('"') => '"',
            Some(x) => x,
            None => return Err(ParseError::new("PAR203", "空のエスケープ", None)),
        }
    } else {
        s.chars()
            .next()
            .ok_or_else(|| ParseError::new("PAR205", "空の文字", None))?
    };
    Ok(ch)
}

pub fn parse_program(src: &str) -> Result<Program, ParseError> {
    let ts = lex(src).map_err(|e| ParseError::new("PAR100", format!("lex error: {}", e), None))?;
    Parser::new(ts).parse_program()
}

pub fn parse_expr(src: &str) -> Result<Expr, ParseError> {
    let ts = lex(src).map_err(|e| ParseError::new("PAR100", format!("lex error: {}", e), None))?;
    let mut p = Parser::new(ts);
    let e = p.parse_expr()?;
    if p.peek().kind != TokenKind::EOF {
        let t = p.peek().clone();
        return Err(ParseError::at(
            "PAR090",
            "余分なトークンが残っています",
            Some(t.pos),
            Some(t.line),
            Some(t.col),
        ));
    }
    Ok(e)
}

#[cfg(test)]
mod tests {
    use super::{decode_char, decode_string};

    #[test]
    /// 文字列リテラルの基本的なエスケープをテストする。
    fn decode_string_basic_escapes() {
        assert_eq!(decode_string("\"a\\n\"").unwrap(), "a\n");
        assert_eq!(decode_string("\"\\t\"\"").unwrap(), "\t\"");
        assert_eq!(decode_string("\"\\\\\"").unwrap(), "\\");
    }

    #[test]
    /// 文字リテラルのエスケープをテストする。
    fn decode_char_escapes_and_plain() {
        assert_eq!(decode_char("'a'").unwrap(), 'a');
        assert_eq!(decode_char("'\\n'").unwrap(), '\n');
        assert_eq!(decode_char("'\\\''").unwrap(), '\'');
        assert_eq!(decode_char("'\"'").unwrap(), '"');
    }
}
