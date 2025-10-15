// パス: src/parser/expr.rs
// 役割: 式・パターン解析に関する `Parser` 実装をまとめる
// 意図: 中置演算やラムダ式など複雑なロジックを専用モジュールに切り分ける
// 関連ファイル: src/parser/program.rs, src/parser/types.rs, src/parser/mod.rs

use super::*;

impl Parser {
    pub(super) fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        let expr = self.parse_expr_no_annot()?;
        if self.accept(TokenKind::DCOLON).is_some() {
            let ty = self.parse_type()?;
            Ok(Expr::Annot {
                expr: Box::new(expr),
                type_expr: ty,
                span: Span::dummy(),
            })
        } else {
            Ok(expr)
        }
    }

    fn parse_expr_no_annot(&mut self) -> Result<Expr, ParseError> {
        match self.peek().kind {
            TokenKind::LAMBDA => self.parse_lambda(),
            TokenKind::LET => self.parse_let_in(),
            TokenKind::IF => self.parse_if(),
            TokenKind::CASE => self.parse_case(),
            _ => self.parse_infix_level(0),
        }
    }

    fn parse_lambda(&mut self) -> Result<Expr, ParseError> {
        let lambda_tok = self.pop(TokenKind::LAMBDA)?;
        let mut params = Vec::new();
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

    fn parse_let_in(&mut self) -> Result<Expr, ParseError> {
        let let_tok = self.pop(TokenKind::LET)?;
        let mut bindings = Vec::new();
        loop {
            let name_tok = self.pop(TokenKind::VARID)?;
            let mut params = Vec::new();
            while self.peek().kind == TokenKind::VARID {
                params.push(self.pop_any().value);
            }
            self.pop(TokenKind::EQUAL)?;
            let rhs = self.parse_expr()?;
            bindings.push((name_tok.value, params, rhs));
            if self.accept(TokenKind::SEMI).is_none() {
                break;
            }
            if self.peek().kind == TokenKind::IN {
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

    fn parse_if(&mut self) -> Result<Expr, ParseError> {
        let if_tok = self.pop(TokenKind::IF)?;
        let cond = self.parse_expr()?;
        self.pop(TokenKind::THEN)?;
        let then_branch = self.parse_expr()?;
        self.pop(TokenKind::ELSE)?;
        let else_branch = self.parse_expr()?;
        Ok(Expr::If {
            cond: Box::new(cond),
            then_branch: Box::new(then_branch),
            else_branch: Box::new(else_branch),
            span: span_from_token(&if_tok),
        })
    }

    fn parse_case(&mut self) -> Result<Expr, ParseError> {
        let case_tok = self.pop(TokenKind::CASE)?;
        let scrutinee = self.parse_expr()?;
        self.pop(TokenKind::OF)?;
        if !Self::is_pattern_start(&self.peek().kind) {
            let t = self.peek().clone();
            return Err(ParseError::at(
                "PAR300",
                "case 式にパターンが必要です",
                Some(t.pos),
                Some(t.line),
                Some(t.col),
            ));
        }
        let mut arms = Vec::new();
        loop {
            let pat = self.parse_pattern()?;
            let guard = if self.accept(TokenKind::BAR).is_some() {
                Some(self.parse_expr()?)
            } else {
                None
            };
            self.pop(TokenKind::ARROW)?;
            let body = self.parse_expr()?;
            arms.push(CaseArm {
                pattern: pat,
                guard,
                body,
            });
            if self.accept(TokenKind::SEMI).is_some() && Self::is_pattern_start(&self.peek().kind) {
                continue;
            }
            break;
        }
        Ok(Expr::Case {
            scrutinee: Box::new(scrutinee),
            arms,
            span: span_from_token(&case_tok),
        })
    }

    fn parse_pattern(&mut self) -> Result<Pattern, ParseError> {
        if self.peek().kind == TokenKind::VARID && matches!(self.peek_kind(1), Some(TokenKind::AT))
        {
            let binder_tok = self.pop(TokenKind::VARID)?;
            let span = span_from_token(&binder_tok);
            let binder = binder_tok.value;
            self.pop(TokenKind::AT)?;
            let inner = self.parse_pattern()?;
            return Ok(Pattern::As {
                binder,
                pattern: Box::new(inner),
                span,
            });
        }
        match self.peek().kind {
            TokenKind::CONID => self.parse_pattern_constructor(),
            _ => self.parse_pattern_atom(),
        }
    }

    fn parse_pattern_constructor(&mut self) -> Result<Pattern, ParseError> {
        let ctor_tok = self.pop(TokenKind::CONID)?;
        let span = span_from_token(&ctor_tok);
        let mut args = Vec::new();
        while Self::is_pattern_start(&self.peek().kind) {
            if matches!(
                self.peek().kind,
                TokenKind::ARROW
                    | TokenKind::SEMI
                    | TokenKind::BAR
                    | TokenKind::OF
                    | TokenKind::EOF
            ) {
                break;
            }
            args.push(self.parse_pattern()?);
        }
        Ok(Pattern::Constructor {
            name: ctor_tok.value,
            args,
            span,
        })
    }

    fn parse_pattern_atom(&mut self) -> Result<Pattern, ParseError> {
        let tok = self.peek().clone();
        match tok.kind {
            TokenKind::UNDERSCORE => {
                self.pop_any();
                Ok(Pattern::Wildcard {
                    span: span_from_token(&tok),
                })
            }
            TokenKind::VARID => {
                self.pop_any();
                let span = span_from_token(&tok);
                let name = tok.value;
                Ok(Pattern::Var { name, span })
            }
            TokenKind::INT | TokenKind::HEX | TokenKind::OCT | TokenKind::BIN => {
                self.pop_any();
                let (value, base) = Self::parse_int_value(&tok)?;
                let span = span_from_token(&tok);
                Ok(Pattern::Int { value, base, span })
            }
            TokenKind::FLOAT => {
                self.pop_any();
                let span = span_from_token(&tok);
                let value = tok.value.parse::<f64>().map_err(|_| {
                    ParseError::at(
                        "PAR221",
                        "浮動小数リテラルが不正",
                        Some(tok.pos),
                        Some(tok.line),
                        Some(tok.col),
                    )
                })?;
                Ok(Pattern::Float { value, span })
            }
            TokenKind::CHAR => {
                self.pop_any();
                let span = span_from_token(&tok);
                let ch = decode_char(&tok.value)?;
                Ok(Pattern::Char { value: ch, span })
            }
            TokenKind::STRING => {
                self.pop_any();
                let span = span_from_token(&tok);
                let s = decode_string(&tok.value)?;
                Ok(Pattern::String { value: s, span })
            }
            TokenKind::TRUE => {
                self.pop_any();
                Ok(Pattern::Bool {
                    value: true,
                    span: span_from_token(&tok),
                })
            }
            TokenKind::FALSE => {
                self.pop_any();
                Ok(Pattern::Bool {
                    value: false,
                    span: span_from_token(&tok),
                })
            }
            TokenKind::LPAREN => {
                self.pop_any();
                if self.accept(TokenKind::RPAREN).is_some() {
                    return Ok(Pattern::Tuple {
                        items: Vec::new(),
                        span: span_from_token(&tok),
                    });
                }
                let first = self.parse_pattern()?;
                if self.accept(TokenKind::COMMA).is_some() {
                    let mut items = vec![first, self.parse_pattern()?];
                    while self.accept(TokenKind::COMMA).is_some() {
                        items.push(self.parse_pattern()?);
                    }
                    self.pop(TokenKind::RPAREN)?;
                    Ok(Pattern::Tuple {
                        items,
                        span: span_from_token(&tok),
                    })
                } else {
                    self.pop(TokenKind::RPAREN)?;
                    Ok(first)
                }
            }
            TokenKind::LBRACK => {
                self.pop_any();
                let mut items = Vec::new();
                if self.peek().kind != TokenKind::RBRACK {
                    items.push(self.parse_pattern()?);
                    while self.accept(TokenKind::COMMA).is_some() {
                        items.push(self.parse_pattern()?);
                    }
                }
                self.pop(TokenKind::RBRACK)?;
                Ok(Pattern::List {
                    items,
                    span: span_from_token(&tok),
                })
            }
            TokenKind::CONID => self.parse_pattern_constructor(),
            _ => Err(ParseError::at(
                "PAR301",
                format!("パターンとして不正なトークン: {:?} {}", tok.kind, tok.value),
                Some(tok.pos),
                Some(tok.line),
                Some(tok.col),
            )),
        }
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
        Expr::BinOp {
            op: op_token.value,
            left: Box::new(left),
            right: Box::new(right),
            span,
        }
    }

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
                let (value, base) = Self::parse_int_value(&t)?;
                Ok(Expr::IntLit {
                    value,
                    base,
                    span: span_from_token(&t),
                })
            }
            TokenKind::FLOAT => {
                self.pop_any();
                let value = t.value.parse::<f64>().map_err(|_| {
                    ParseError::at(
                        "PAR220",
                        "浮動小数リテラルが不正",
                        Some(t.pos),
                        Some(t.line),
                        Some(t.col),
                    )
                })?;
                Ok(Expr::FloatLit {
                    value,
                    span: span_from_token(&t),
                })
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
                let name = t.value;
                Ok(Expr::Var { name, span })
            }
            TokenKind::CONID => {
                self.pop_any();
                let span = span_from_token(&t);
                let name = t.value;
                Ok(Expr::Var { name, span })
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
                let expr = self.parse_expr()?;
                if self.accept(TokenKind::COMMA).is_some() {
                    let mut items = vec![expr, self.parse_expr()?];
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
                    Ok(expr)
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
                | TokenKind::CONID
                | TokenKind::UNDERSCORE
                | TokenKind::QMARK
                | TokenKind::LPAREN
                | TokenKind::LBRACK
                | TokenKind::TRUE
                | TokenKind::FALSE
        )
    }

    fn is_pattern_start(kind: &TokenKind) -> bool {
        matches!(
            kind,
            TokenKind::UNDERSCORE
                | TokenKind::VARID
                | TokenKind::CONID
                | TokenKind::INT
                | TokenKind::HEX
                | TokenKind::OCT
                | TokenKind::BIN
                | TokenKind::FLOAT
                | TokenKind::TRUE
                | TokenKind::FALSE
                | TokenKind::STRING
                | TokenKind::CHAR
                | TokenKind::LBRACK
                | TokenKind::LPAREN
        )
    }

    fn parse_int_value(token: &Token) -> Result<(i64, IntBase), ParseError> {
        let parsed = match token.kind {
            TokenKind::HEX => i64::from_str_radix(&token.value[2..], 16),
            TokenKind::OCT => i64::from_str_radix(&token.value[2..], 8),
            TokenKind::BIN => i64::from_str_radix(&token.value[2..], 2),
            _ => token.value.parse::<i64>(),
        };
        match parsed {
            Ok(value) => {
                let base = match token.kind {
                    TokenKind::HEX => IntBase::Hex,
                    TokenKind::OCT => IntBase::Oct,
                    TokenKind::BIN => IntBase::Bin,
                    _ => IntBase::Dec,
                };
                Ok((value, base))
            }
            Err(_) => Err(ParseError::new(
                "PAR210",
                "整数リテラルが範囲外",
                Some(token.pos),
            )),
        }
    }
}
