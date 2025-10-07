// パス: src/parser.rs
// 役割: トークン列から AST を生成する再帰下降パーサを実装する
// 意図: 字句解析結果を型推論・評価に直接渡せる構造へ変換する
// 関連ファイル: src/lexer.rs, src/ast.rs, src/errors.rs
//! 構文解析モジュール
//!
//! - EBNF で定義された文法に従ってプログラム・式・型注釈を解析する。
//! - 演算子の結合規則・優先順位は `cmp > add > mul > pow > app` の順でハンドコードする。
//! - 単項マイナスなどの糖衣は `0 - x` など正規化した AST へ変換する。

use crate::ast::{
    Constraint as AConstraint, Expr, IntBase, Program, SigmaType, Span, TopLevel, TypeExpr,
};
use crate::errors::ParseError;
use crate::lexer::{lex, Token, TokenKind};

/// 再帰下降パーサの進行状態を保持する構造体。
pub struct Parser {
    ts: Vec<Token>,
    i: usize,
}

#[derive(Clone, Copy)]
enum Assoc {
    Left,
    Right,
    Non,
}

struct InfixSpec {
    tokens: &'static [TokenKind],
    assoc: Assoc,
}

impl InfixSpec {
    fn contains(&self, kind: &TokenKind) -> bool {
        self.tokens.iter().any(|tk| tk == kind)
    }
}

const INFIX_LEVELS: &[InfixSpec] = &[
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

/// `Parser` が提供する各種解析メソッド。
impl Parser {
    /// トークン列から新しいパーサインスタンスを構築する。
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { ts: tokens, i: 0 }
    }
    /// 現在位置のトークンを参照する。
    fn peek(&self) -> &Token {
        &self.ts[self.i]
    }
    /// 現在位置のトークンを消費して返す。
    fn pop_any(&mut self) -> Token {
        let t = self.ts[self.i].clone();
        self.i += 1;
        t
    }
    /// 現在位置から任意のオフセットにあるトークン種別を先読みする。
    fn peek_kind(&self, offset: usize) -> Option<TokenKind> {
        self.ts.get(self.i + offset).map(|t| t.kind.clone())
    }
    /// 指定した種別のトークンを期待しつつ消費する。
    fn pop(&mut self, kind: TokenKind) -> Result<Token, ParseError> {
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
    /// 指定した種別が先頭にあれば消費し、なければ `None` を返す。
    fn accept(&mut self, kind: TokenKind) -> Option<Token> {
        if self.peek().kind == kind {
            let t = self.pop_any();
            Some(t)
        } else {
            None
        }
    }

    /// プログラム全体（`program := { toplevel }`）を解析する。
    ///
    /// # Examples
    /// ```
    /// use typelang::parser::{self, Parser};
    /// let src = "square :: Num a => a -> a;\nlet square x = x * x;";
    /// let tokens = typelang::lexer::lex(src).unwrap();
    /// let mut p = Parser::new(tokens);
    /// let prog = p.parse_program().unwrap();
    /// assert_eq!(prog.decls.len(), 1);
    /// ```
    pub fn parse_program(&mut self) -> Result<Program, ParseError> {
        let mut decls = Vec::new();
        while self.peek().kind != TokenKind::EOF {
            let mut sig: Option<SigmaType> = None;
            // 先読み: varid '::' ...
            let save = self.i;
            if self.peek().kind == TokenKind::VARID {
                let _name_tok = self.pop_any();
                if self.accept(TokenKind::DCOLON).is_some() {
                    sig = Some(self.parse_sigma_type()?);
                    self._expect_semicolon_optional()?;
                } else {
                    self.i = save;
                }
            }
            // let binding
            self.pop(TokenKind::LET)?;
            let name_tok = self.pop(TokenKind::VARID)?;
            let mut params: Vec<String> = Vec::new();
            while self.peek().kind == TokenKind::VARID {
                params.push(self.pop_any().value);
            }
            self.pop(TokenKind::EQUAL)?;
            let expr = self.parse_expr()?;
            self._expect_semicolon_optional()?;
            decls.push(TopLevel {
                name: name_tok.value,
                params,
                expr,
                signature: sig,
            });
        }
        Ok(Program { decls })
    }

    /// 文末のセミコロンがあれば消費する（省略可）。
    fn _expect_semicolon_optional(&mut self) -> Result<(), ParseError> {
        self.accept(TokenKind::SEMI);
        Ok(())
    }

    /// 型注釈を含む単一式（`expr := core_expr ['::' type]`）を解析する。
    ///
    /// # Examples
    /// ```
    /// use typelang::parser;
    /// let e = parser::parse_expr("1 + 2").unwrap();
    /// match e { typelang::ast::Expr::BinOp{..} => (), _ => unreachable!() }
    /// ```
    pub fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        let expr = self.parse_expr_no_annot()?;
        if let Some(colon_tok) = self.accept(TokenKind::DCOLON) {
            let ty = self.parse_type()?;
            Ok(Expr::Annot {
                expr: Box::new(expr),
                type_expr: ty,
                span: span_from_token(&colon_tok),
            })
        } else {
            Ok(expr)
        }
    }

    /// 型注釈を除いた式（ラムダ/let-in/if/二項式）を解析する。
    fn parse_expr_no_annot(&mut self) -> Result<Expr, ParseError> {
        match self.peek().kind.clone() {
            TokenKind::LAMBDA => self.parse_lambda(),
            TokenKind::LET => self.parse_let_in(),
            TokenKind::IF => self.parse_if(),
            _ => self.parse_infix_level(0),
        }
    }

    /// ラムダ式（`\ params -> expr`）を解析する。
    fn parse_lambda(&mut self) -> Result<Expr, ParseError> {
        let lambda_tok = self.pop(TokenKind::LAMBDA)?;
        let mut params: Vec<String> = Vec::new();
        while self.peek().kind == TokenKind::VARID {
            params.push(self.pop_any().value);
        }
        self.pop(TokenKind::ARROW)?;
        let body = self.parse_expr()?;
        Ok(Expr::Lambda {
            params,
            body: Box::new(body),
            span: span_from_token(&lambda_tok),
        })
    }

    /// `let ... in ...` 式を解析し、束縛を収集する。
    fn parse_let_in(&mut self) -> Result<Expr, ParseError> {
        let let_tok = self.pop(TokenKind::LET)?;
        let mut bindings: Vec<(String, Vec<String>, Expr)> = Vec::new();
        loop {
            let name = self.pop(TokenKind::VARID)?.value;
            let mut params: Vec<String> = Vec::new();
            while self.peek().kind == TokenKind::VARID {
                params.push(self.pop_any().value);
            }
            self.pop(TokenKind::EQUAL)?;
            let e = self.parse_expr()?;
            bindings.push((name, params, e));
            if self.accept(TokenKind::SEMI).is_none() {
                break;
            }
        }
        self.pop(TokenKind::IN)?;
        let body = self.parse_expr()?;
        Ok(Expr::LetIn {
            bindings,
            body: Box::new(body),
            span: span_from_token(&let_tok),
        })
    }

    /// `if ... then ... else ...` 式を解析する。
    fn parse_if(&mut self) -> Result<Expr, ParseError> {
        let if_tok = self.pop(TokenKind::IF)?;
        let cond = self.parse_expr()?;
        self.pop(TokenKind::THEN)?;
        let th = self.parse_expr()?;
        self.pop(TokenKind::ELSE)?;
        let el = self.parse_expr()?;
        Ok(Expr::If {
            cond: Box::new(cond),
            then_branch: Box::new(th),
            else_branch: Box::new(el),
            span: span_from_token(&if_tok),
        })
    }

    fn parse_infix_level(&mut self, level: usize) -> Result<Expr, ParseError> {
        if level >= INFIX_LEVELS.len() {
            return self.parse_app();
        }
        let spec = &INFIX_LEVELS[level];
        let mut left = self.parse_infix_level(level + 1)?;
        match spec.assoc {
            Assoc::Left => {
                while spec.contains(&self.peek().kind) {
                    let op_token = self.pop_any();
                    let right = self.parse_infix_level(level + 1)?;
                    left = Self::mk_binop(left, op_token, right);
                }
                Ok(left)
            }
            Assoc::Right => {
                if spec.contains(&self.peek().kind) {
                    let op_token = self.pop_any();
                    let right = self.parse_infix_level(level)?;
                    Ok(Self::mk_binop(left, op_token, right))
                } else {
                    Ok(left)
                }
            }
            Assoc::Non => {
                if spec.contains(&self.peek().kind) {
                    let op_token = self.pop_any();
                    let right = self.parse_infix_level(level + 1)?;
                    Ok(Self::mk_binop(left, op_token, right))
                } else {
                    Ok(left)
                }
            }
        }
    }

    fn mk_binop(left: Expr, op_token: Token, right: Expr) -> Expr {
        let span = span_from_token(&op_token);
        let op_value = op_token.value;
        Expr::BinOp {
            op: op_value,
            left: Box::new(left),
            right: Box::new(right),
            span,
        }
    }

    /// 関数適用（左結合）を解析する。
    fn parse_app(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_atom()?;
        while Self::is_atom_start(&self.peek().kind) {
            let right = self.parse_atom()?;
            let func = left;
            let span = func.span();
            left = Expr::App {
                func: Box::new(func),
                arg: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    /// リテラル・識別子・括弧を含む原子式を解析する。
    fn parse_atom(&mut self) -> Result<Expr, ParseError> {
        let t = self.peek().clone();
        match t.kind {
            TokenKind::MINUS => {
                self.pop_any();
                let rhs = self.parse_atom()?;
                Ok(Expr::BinOp {
                    op: "-".into(),
                    left: Box::new(Expr::IntLit {
                        value: 0,
                        base: IntBase::Dec,
                        span: Span::dummy(),
                    }),
                    right: Box::new(rhs),
                    span: span_from_token(&t),
                })
            }
            TokenKind::INT | TokenKind::HEX | TokenKind::OCT | TokenKind::BIN => {
                self.pop_any();
                let s = t.value.clone();
                let parsed: Result<i64, _> = match t.kind {
                    TokenKind::HEX => i64::from_str_radix(&s[2..], 16),
                    TokenKind::OCT => i64::from_str_radix(&s[2..], 8),
                    TokenKind::BIN => i64::from_str_radix(&s[2..], 2),
                    _ => s.parse::<i64>(),
                };
                match parsed {
                    Ok(v) => {
                        let base = match t.kind {
                            TokenKind::HEX => IntBase::Hex,
                            TokenKind::OCT => IntBase::Oct,
                            TokenKind::BIN => IntBase::Bin,
                            _ => IntBase::Dec,
                        };
                        Ok(Expr::IntLit {
                            value: v,
                            base,
                            span: span_from_token(&t),
                        })
                    }
                    Err(_) => Err(ParseError::new(
                        "PAR210",
                        "整数リテラルが範囲外",
                        Some(t.pos),
                    )),
                }
            }
            TokenKind::FLOAT => {
                self.pop_any();
                match t.value.parse::<f64>() {
                    Ok(v) => Ok(Expr::FloatLit {
                        value: v,
                        span: span_from_token(&t),
                    }),
                    Err(_) => Err(ParseError::at(
                        "PAR220",
                        "浮動小数リテラルが不正",
                        Some(t.pos),
                        Some(t.line),
                        Some(t.col),
                    )),
                }
            }
            TokenKind::CHAR => {
                self.pop_any();
                let ch = decode_char(&t.value)?;
                Ok(Expr::CharLit {
                    value: ch,
                    span: span_from_token(&t),
                })
            }
            TokenKind::STRING => {
                self.pop_any();
                let s = decode_string(&t.value)?;
                Ok(Expr::StringLit {
                    value: s,
                    span: span_from_token(&t),
                })
            }
            TokenKind::TRUE => {
                self.pop_any();
                Ok(Expr::BoolLit {
                    value: true,
                    span: span_from_token(&t),
                })
            }
            TokenKind::FALSE => {
                self.pop_any();
                Ok(Expr::BoolLit {
                    value: false,
                    span: span_from_token(&t),
                })
            }
            TokenKind::VARID => {
                self.pop_any();
                let span = span_from_token(&t);
                Ok(Expr::Var {
                    name: t.value,
                    span,
                })
            }
            TokenKind::QMARK => {
                self.pop_any();
                let name = self.pop(TokenKind::VARID)?.value;
                Ok(Expr::Var {
                    name: format!("?{}", name),
                    span: span_from_token(&t),
                })
            }
            TokenKind::UNDERSCORE => {
                self.pop_any();
                Ok(Expr::Var {
                    name: "_".into(),
                    span: span_from_token(&t),
                })
            }
            TokenKind::LPAREN => {
                self.pop_any();
                let e = self.parse_expr()?;
                if self.accept(TokenKind::COMMA).is_some() {
                    let mut items = vec![e, self.parse_expr()?];
                    while self.accept(TokenKind::COMMA).is_some() {
                        items.push(self.parse_expr()?);
                    }
                    self.pop(TokenKind::RPAREN)?;
                    Ok(Expr::TupleLit {
                        items,
                        span: span_from_token(&t),
                    })
                } else {
                    self.pop(TokenKind::RPAREN)?;
                    Ok(e)
                }
            }
            TokenKind::LBRACK => {
                self.pop_any();
                let mut items = Vec::new();
                if self.peek().kind != TokenKind::RBRACK {
                    items.push(self.parse_expr()?);
                    while self.accept(TokenKind::COMMA).is_some() {
                        items.push(self.parse_expr()?);
                    }
                }
                self.pop(TokenKind::RBRACK)?;
                Ok(Expr::ListLit {
                    items,
                    span: span_from_token(&t),
                })
            }
            _ => Err(ParseError::new(
                "PAR002",
                format!("不正なトークン: {:?} {}", t.kind, t.value),
                Some(t.pos),
            )),
        }
    }

    fn is_atom_start(kind: &TokenKind) -> bool {
        matches!(
            kind,
            TokenKind::INT
                | TokenKind::HEX
                | TokenKind::OCT
                | TokenKind::BIN
                | TokenKind::FLOAT
                | TokenKind::CHAR
                | TokenKind::STRING
                | TokenKind::VARID
                | TokenKind::UNDERSCORE
                | TokenKind::QMARK
                | TokenKind::LPAREN
                | TokenKind::LBRACK
                | TokenKind::TRUE
                | TokenKind::FALSE
        )
    }

    // ===== 型式 =====
    /// 制約付きシグマ型（`context => type`）を解析する。
    pub fn parse_sigma_type(&mut self) -> Result<SigmaType, ParseError> {
        // 先読み context '=>'
        let mut constraints: Vec<AConstraint> = Vec::new();
        if matches!(self.peek().kind, TokenKind::CONID | TokenKind::LPAREN) {
            let save = self.i;
            if let Some(cs) = self.try_parse_context()? {
                constraints = cs;
                self.pop(TokenKind::DARROW)?;
            } else {
                self.i = save;
            }
        }
        let ty = self.parse_type()?;
        Ok(SigmaType {
            constraints,
            r#type: ty,
        })
    }

    /// 制約コンテキストを先読みし、存在すれば解析して返す。
    fn try_parse_context(&mut self) -> Result<Option<Vec<AConstraint>>, ParseError> {
        match self.peek().kind {
            TokenKind::CONID => {
                if !matches!(self.peek_kind(1), Some(TokenKind::VARID)) {
                    return Ok(None);
                }
                let save = self.i;
                let c = self.parse_constraint()?;
                if self.peek().kind == TokenKind::DARROW {
                    Ok(Some(vec![c]))
                } else {
                    self.i = save;
                    Ok(None)
                }
            }
            TokenKind::LPAREN => {
                let save = self.i;
                self.pop(TokenKind::LPAREN)?;
                if !(matches!(self.peek_kind(0), Some(TokenKind::CONID))
                    && matches!(self.peek_kind(1), Some(TokenKind::VARID)))
                {
                    self.i = save;
                    return Ok(None);
                }
                let mut cs = vec![self.parse_constraint()?];
                while self.accept(TokenKind::COMMA).is_some() {
                    if !(matches!(self.peek_kind(0), Some(TokenKind::CONID))
                        && matches!(self.peek_kind(1), Some(TokenKind::VARID)))
                    {
                        self.i = save;
                        return Ok(None);
                    }
                    cs.push(self.parse_constraint()?);
                }
                if self.accept(TokenKind::RPAREN).is_none() {
                    self.i = save;
                    return Ok(None);
                }
                if self.peek().kind == TokenKind::DARROW {
                    Ok(Some(cs))
                } else {
                    self.i = save;
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
    }

    /// `Class var` 形式の型クラス制約を解析する。
    fn parse_constraint(&mut self) -> Result<AConstraint, ParseError> {
        let cls = self.pop(TokenKind::CONID)?.value;
        let tv = self.pop(TokenKind::VARID)?.value;
        Ok(AConstraint {
            classname: cls,
            typevar: tv,
        })
    }

    /// 関数矢印を含む型式を解析する。
    pub fn parse_type(&mut self) -> Result<TypeExpr, ParseError> {
        let left = self.parse_type_app()?;
        if self.accept(TokenKind::ARROW).is_some() {
            let right = self.parse_type()?;
            Ok(TypeExpr::TEFun(Box::new(left), Box::new(right)))
        } else {
            Ok(left)
        }
    }

    /// 型適用（左結合）を解析する。
    fn parse_type_app(&mut self) -> Result<TypeExpr, ParseError> {
        let mut left = self.parse_type_atom()?;
        while let TokenKind::CONID | TokenKind::VARID | TokenKind::LPAREN | TokenKind::LBRACK =
            self.peek().kind
        {
            let right = self.parse_type_atom()?;
            left = TypeExpr::TEApp(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    /// 型式の原子要素（変数・コンストラクタ・タプル・リスト）を解析する。
    fn parse_type_atom(&mut self) -> Result<TypeExpr, ParseError> {
        let t = self.peek().clone();
        match t.kind {
            TokenKind::VARID => {
                self.pop_any();
                Ok(TypeExpr::TEVar(t.value))
            }
            TokenKind::CONID => {
                self.pop_any();
                Ok(TypeExpr::TECon(t.value))
            }
            TokenKind::LPAREN => {
                self.pop_any();
                let ty = self.parse_type()?;
                if self.accept(TokenKind::COMMA).is_some() {
                    let mut items = vec![ty, self.parse_type()?];
                    while self.accept(TokenKind::COMMA).is_some() {
                        items.push(self.parse_type()?);
                    }
                    self.pop(TokenKind::RPAREN)?;
                    Ok(TypeExpr::TETuple(items))
                } else {
                    self.pop(TokenKind::RPAREN)?;
                    Ok(ty)
                }
            }
            TokenKind::LBRACK => {
                self.pop_any();
                let inner = self.parse_type()?;
                self.pop(TokenKind::RBRACK)?;
                Ok(TypeExpr::TEList(Box::new(inner)))
            }
            _ => Err(ParseError::at(
                "PAR102",
                format!("型の原子が不正: {:?} {}", t.kind, t.value),
                Some(t.pos),
                Some(t.line),
                Some(t.col),
            )),
        }
    }
}

// ===== ユーティリティ =====
/// ソース上の文字列リテラルをアンエスケープし、内容だけを返す。
fn decode_string(quoted: &str) -> Result<String, ParseError> {
    // quoted には "..." を含む
    if !quoted.starts_with('"') || !quoted.ends_with('"') {
        return Err(ParseError::new("PAR201", "文字列リテラルが不正", None));
    }
    let s = &quoted[1..quoted.len() - 1];
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            // エスケープシーケンスを復号する
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

/// ソース上の文字リテラルをアンエスケープして単一文字を返す。
fn decode_char(quoted: &str) -> Result<char, ParseError> {
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

fn span_from_token(token: &Token) -> Span {
    Span::new(token.pos, token.line, token.col)
}

// エントリヘルパ
/// ソース文字列を完全に解析し、`Program` を返す高水準 API。
pub fn parse_program(src: &str) -> Result<Program, ParseError> {
    let ts = lex(src).map_err(|e| ParseError::new("PAR100", format!("lex error: {}", e), None))?;
    Parser::new(ts).parse_program()
}
/// ソース文字列から単一の式を解析する高水準 API。
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
