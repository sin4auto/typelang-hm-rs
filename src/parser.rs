// パス: src/parser.rs
// 役割: Recursive-descent parser that converts tokens into AST nodes
// 意図: Bridge lexical tokens to structures used by inference and evaluation
// 関連ファイル: src/lexer.rs, src/ast.rs, src/errors.rs
//! 構文解析（parser）
//!
//! 目的:
//! - EBNFに基づきプログラム/式/型式を再帰下降で解析し、`ast` を生成する。
//!
//! 特記:
//! - 演算子は結合規則と優先順位を手続き的に実装（cmp > add > mul > pow > app）。
//! - 一部の糖衣（単項マイナス）は `0 - x` に変換し、意味の一貫性を保つ。

use crate::ast::{
    Constraint as AConstraint, Expr, IntBase, Program, SigmaType, TopLevel, TypeExpr,
};
use crate::errors::ParseError;
use crate::lexer::{lex, Token, TokenKind};

pub struct Parser {
    ts: Vec<Token>,
    i: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { ts: tokens, i: 0 }
    }
    fn peek(&self) -> &Token {
        &self.ts[self.i]
    }
    fn pop_any(&mut self) -> Token {
        let t = self.ts[self.i].clone();
        self.i += 1;
        t
    }
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
    fn accept(&mut self, kind: TokenKind) -> Option<Token> {
        if self.peek().kind == kind {
            let t = self.pop_any();
            Some(t)
        } else {
            None
        }
    }

    /// program := { toplevel }
    ///
    /// 与えられたトークン列からプログラム全体を解析します。
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

    fn _expect_semicolon_optional(&mut self) -> Result<(), ParseError> {
        self.accept(TokenKind::SEMI);
        Ok(())
    }

    /// expr := (lambda | let_in | if | infix_cmp) ['::' type]
    ///
    /// 単一の式を解析します（必要に応じて型注釈も受け付け）。
    ///
    /// # Examples
    /// ```
    /// use typelang::parser;
    /// let e = parser::parse_expr("1 + 2").unwrap();
    /// match e { typelang::ast::Expr::BinOp{..} => (), _ => unreachable!() }
    /// ```
    pub fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        let expr = self.parse_expr_no_annot()?;
        if self.accept(TokenKind::DCOLON).is_some() {
            let ty = self.parse_type()?;
            Ok(Expr::Annot {
                expr: Box::new(expr),
                type_expr: ty,
            })
        } else {
            Ok(expr)
        }
    }

    fn parse_expr_no_annot(&mut self) -> Result<Expr, ParseError> {
        match self.peek().kind.clone() {
            TokenKind::LAMBDA => self.parse_lambda(),
            TokenKind::LET => self.parse_let_in(),
            TokenKind::IF => self.parse_if(),
            _ => self.parse_infix_cmp(),
        }
    }

    fn parse_lambda(&mut self) -> Result<Expr, ParseError> {
        self.pop(TokenKind::LAMBDA)?;
        let mut params: Vec<String> = Vec::new();
        while self.peek().kind == TokenKind::VARID {
            params.push(self.pop_any().value);
        }
        self.pop(TokenKind::ARROW)?;
        let body = self.parse_expr()?;
        Ok(Expr::Lambda {
            params,
            body: Box::new(body),
        })
    }

    fn parse_let_in(&mut self) -> Result<Expr, ParseError> {
        self.pop(TokenKind::LET)?;
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
        })
    }

    fn parse_if(&mut self) -> Result<Expr, ParseError> {
        self.pop(TokenKind::IF)?;
        let cond = self.parse_expr()?;
        self.pop(TokenKind::THEN)?;
        let th = self.parse_expr()?;
        self.pop(TokenKind::ELSE)?;
        let el = self.parse_expr()?;
        Ok(Expr::If {
            cond: Box::new(cond),
            then_branch: Box::new(th),
            else_branch: Box::new(el),
        })
    }

    fn parse_infix_cmp(&mut self) -> Result<Expr, ParseError> {
        let left = self.parse_infix_add()?;
        match self.peek().kind {
            TokenKind::EQ
            | TokenKind::NE
            | TokenKind::LT
            | TokenKind::LE
            | TokenKind::GT
            | TokenKind::GE => {
                let op = self.pop_any().value;
                let right = self.parse_infix_add()?;
                Ok(Expr::BinOp {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                })
            }
            _ => Ok(left),
        }
    }

    fn parse_infix_add(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_infix_mul()?;
        while let TokenKind::PLUS | TokenKind::MINUS = self.peek().kind {
            let op = self.pop_any().value;
            let right = self.parse_infix_mul()?;
            left = Expr::BinOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_infix_mul(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_infix_pow()?;
        while let TokenKind::STAR | TokenKind::SLASH = self.peek().kind {
            let op = self.pop_any().value;
            let right = self.parse_infix_pow()?;
            left = Expr::BinOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_infix_pow(&mut self) -> Result<Expr, ParseError> {
        // 右結合: app ('^' | '**') infix_pow | app
        let left = self.parse_app()?;
        match self.peek().kind {
            TokenKind::CARET | TokenKind::DBLSTAR => {
                let op = self.pop_any().value;
                let right = self.parse_infix_pow()?;
                Ok(Expr::BinOp {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                })
            }
            _ => Ok(left),
        }
    }

    fn parse_app(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_atom()?;
        while let TokenKind::INT
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
        | TokenKind::FALSE = self.peek().kind
        {
            let right = self.parse_atom()?;
            left = Expr::App {
                func: Box::new(left),
                arg: Box::new(right),
            };
        }
        Ok(left)
    }

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
                    }),
                    right: Box::new(rhs),
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
                        Ok(Expr::IntLit { value: v, base })
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
                    Ok(v) => Ok(Expr::FloatLit { value: v }),
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
                Ok(Expr::CharLit { value: ch })
            }
            TokenKind::STRING => {
                self.pop_any();
                let s = decode_string(&t.value)?;
                Ok(Expr::StringLit { value: s })
            }
            TokenKind::TRUE => {
                self.pop_any();
                Ok(Expr::BoolLit { value: true })
            }
            TokenKind::FALSE => {
                self.pop_any();
                Ok(Expr::BoolLit { value: false })
            }
            TokenKind::VARID => {
                self.pop_any();
                Ok(Expr::Var { name: t.value })
            }
            TokenKind::QMARK => {
                self.pop_any();
                let name = self.pop(TokenKind::VARID)?.value;
                Ok(Expr::Var {
                    name: format!("?{}", name),
                })
            }
            TokenKind::UNDERSCORE => {
                self.pop_any();
                Ok(Expr::Var { name: "_".into() })
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
                    Ok(Expr::TupleLit { items })
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
                Ok(Expr::ListLit { items })
            }
            _ => Err(ParseError::new(
                "PAR002",
                format!("不正なトークン: {:?} {}", t.kind, t.value),
                Some(t.pos),
            )),
        }
    }

    // ===== 型式 =====
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

    fn try_parse_context(&mut self) -> Result<Option<Vec<AConstraint>>, ParseError> {
        match self.peek().kind {
            TokenKind::CONID => {
                let c = self.parse_constraint()?;
                if self.peek().kind == TokenKind::DARROW {
                    Ok(Some(vec![c]))
                } else {
                    Ok(None)
                }
            }
            TokenKind::LPAREN => {
                let save = self.i;
                self.pop(TokenKind::LPAREN)?;
                let mut cs = vec![self.parse_constraint()?];
                while self.accept(TokenKind::COMMA).is_some() {
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

    fn parse_constraint(&mut self) -> Result<AConstraint, ParseError> {
        let cls = self.pop(TokenKind::CONID)?.value;
        let tv = self.pop(TokenKind::VARID)?.value;
        Ok(AConstraint {
            classname: cls,
            typevar: tv,
        })
    }

    pub fn parse_type(&mut self) -> Result<TypeExpr, ParseError> {
        let left = self.parse_type_app()?;
        if self.accept(TokenKind::ARROW).is_some() {
            let right = self.parse_type()?;
            Ok(TypeExpr::TEFun(Box::new(left), Box::new(right)))
        } else {
            Ok(left)
        }
    }

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
            // エスケープ
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

// エントリヘルパ
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
    fn decode_string_basic_escapes() {
        assert_eq!(decode_string("\"a\\n\"").unwrap(), "a\n");
        assert_eq!(decode_string("\"\\t\"\"").unwrap(), "\t\"");
        assert_eq!(decode_string("\"\\\\\"").unwrap(), "\\");
    }

    #[test]
    fn decode_char_escapes_and_plain() {
        assert_eq!(decode_char("'a'").unwrap(), 'a');
        assert_eq!(decode_char("'\\n'").unwrap(), '\n');
        assert_eq!(decode_char("'\\\''").unwrap(), '\'');
        assert_eq!(decode_char("'\"'").unwrap(), '"');
    }
}
