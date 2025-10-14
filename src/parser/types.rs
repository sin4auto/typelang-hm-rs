// パス: src/parser/types.rs
// 役割: 型注釈・シグマ型・制約解析のロジックを提供する
// 意図: 型関連の処理を専用モジュールに集約し、Parser 実装を分割する
// 関連ファイル: src/parser/expr.rs, src/parser/program.rs, src/parser/mod.rs

use super::*;

impl Parser {
    pub(super) fn parse_sigma_type(&mut self) -> Result<SigmaType, ParseError> {
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

    pub(super) fn parse_type(&mut self) -> Result<TypeExpr, ParseError> {
        let mut ty = self.parse_type_app()?;
        if self.accept(TokenKind::ARROW).is_some() {
            let rhs = self.parse_type()?;
            ty = TypeExpr::TEFun(Box::new(ty), Box::new(rhs));
        }
        Ok(ty)
    }

    pub(super) fn parse_type_atom(&mut self) -> Result<TypeExpr, ParseError> {
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
                let inner = self.parse_type()?;
                if self.accept(TokenKind::COMMA).is_some() {
                    let mut items = vec![inner, self.parse_type()?];
                    while self.accept(TokenKind::COMMA).is_some() {
                        items.push(self.parse_type()?);
                    }
                    self.pop(TokenKind::RPAREN)?;
                    Ok(TypeExpr::TETuple(items))
                } else {
                    self.pop(TokenKind::RPAREN)?;
                    Ok(inner)
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

    pub(super) fn is_type_atom_start(&self, kind: &TokenKind) -> bool {
        matches!(
            kind,
            TokenKind::CONID | TokenKind::VARID | TokenKind::LPAREN | TokenKind::LBRACK
        )
    }

    fn parse_type_app(&mut self) -> Result<TypeExpr, ParseError> {
        let mut ty = self.parse_type_atom()?;
        while self.is_type_atom_start(&self.peek().kind) {
            let next = self.parse_type_atom()?;
            ty = TypeExpr::TEApp(Box::new(ty), Box::new(next));
        }
        Ok(ty)
    }

    #[cfg_attr(coverage, coverage(off))]
    fn try_parse_context(&mut self) -> Result<Option<Vec<AConstraint>>, ParseError> {
        if self.peek().kind == TokenKind::LPAREN {
            if matches!(self.peek_kind(1), Some(TokenKind::CONID))
                && matches!(self.peek_kind(2), Some(TokenKind::VARID))
            {
                let save = self.i;
                self.pop_any();
                let mut constraints = Vec::new();
                constraints.push(self.parse_constraint()?);
                while self.accept(TokenKind::COMMA).is_some() {
                    constraints.push(self.parse_constraint()?);
                }
                self.pop(TokenKind::RPAREN)?;
                if self.peek().kind == TokenKind::DARROW {
                    return Ok(Some(constraints));
                }
                self.i = save;
            }
            return Ok(None);
        }
        if self.peek().kind == TokenKind::CONID
            && matches!(self.peek_kind(1), Some(TokenKind::VARID))
        {
            let save = self.i;
            let constraint = self.parse_constraint()?;
            if self.peek().kind == TokenKind::DARROW {
                return Ok(Some(vec![constraint]));
            }
            self.i = save;
        }
        Ok(None)
    }

    fn parse_constraint(&mut self) -> Result<AConstraint, ParseError> {
        let class_tok = self.pop(TokenKind::CONID)?;
        let tyvar_tok = self.pop(TokenKind::VARID)?;
        Ok(AConstraint {
            classname: class_tok.value,
            typevar: tyvar_tok.value,
        })
    }
}
