// パス: src/parser/program.rs
// 役割: トップレベル宣言・データ宣言の構文解析ルーチンを実装する
// 意図: プログラム全体の解析ロジックを `Parser` から分離し可読性を高める
// 関連ファイル: src/parser/expr.rs, src/parser/types.rs, src/parser/mod.rs

use super::*;
use crate::ast::{ClassDecl, InstanceDecl};

impl Parser {
    pub(super) fn parse_program(&mut self) -> Result<Program, ParseError> {
        let mut decls = Vec::new();
        let mut data_decls = Vec::new();
        let mut class_decls = Vec::new();
        let mut instance_decls = Vec::new();
        while self.peek().kind != TokenKind::EOF {
            if self.peek().kind == TokenKind::SEMI {
                self.pop_any();
                continue;
            }
            if self.peek().kind == TokenKind::CLASS {
                let class_decl = self.parse_class_decl()?;
                self.expect_semicolon_optional()?;
                class_decls.push(class_decl);
                continue;
            }
            if self.peek().kind == TokenKind::INSTANCE {
                let instance_decl = self.parse_instance_decl()?;
                self.expect_semicolon_optional()?;
                instance_decls.push(instance_decl);
                continue;
            }
            if self.peek().kind == TokenKind::DATA {
                let data = self.parse_data_decl()?;
                self.expect_semicolon_optional()?;
                data_decls.push(data);
                continue;
            }
            let mut sig: Option<SigmaType> = None;
            let save = self.i;
            if self.peek().kind == TokenKind::VARID {
                let _name_tok = self.pop_any();
                if self.accept(TokenKind::DCOLON).is_some() {
                    sig = Some(self.parse_sigma_type()?);
                    self.expect_semicolon_optional()?;
                } else {
                    self.i = save;
                }
            }
            self.pop(TokenKind::LET)?;
            let name_tok = self.pop(TokenKind::VARID)?;
            let mut params: Vec<String> = Vec::new();
            while self.peek().kind == TokenKind::VARID {
                params.push(self.pop_any().value);
            }
            self.pop(TokenKind::EQUAL)?;
            let expr = self.parse_expr()?;
            self.expect_semicolon_optional()?;
            decls.push(TopLevel {
                name: name_tok.value,
                params,
                expr,
                signature: sig,
            });
        }
        Ok(Program {
            class_decls,
            instance_decls,
            data_decls,
            decls,
        })
    }

    fn expect_semicolon_optional(&mut self) -> Result<(), ParseError> {
        self.accept(TokenKind::SEMI);
        Ok(())
    }

    pub(super) fn parse_data_decl(&mut self) -> Result<DataDecl, ParseError> {
        let data_tok = self.pop(TokenKind::DATA)?;
        let name_tok = self.pop(TokenKind::CONID)?;
        let mut params = Vec::new();
        while self.peek().kind == TokenKind::VARID {
            params.push(self.pop_any().value);
        }
        self.pop(TokenKind::EQUAL)?;
        let mut ctors = Vec::new();
        loop {
            ctors.push(self.parse_data_constructor()?);
            if self.accept(TokenKind::BAR).is_some() {
                continue;
            }
            break;
        }
        Ok(DataDecl {
            name: name_tok.value,
            params,
            constructors: ctors,
            span: span_from_token(&data_tok),
        })
    }

    fn parse_data_constructor(&mut self) -> Result<DataConstructor, ParseError> {
        let ctor_tok = self.pop(TokenKind::CONID)?;
        let span = span_from_token(&ctor_tok);
        let name = ctor_tok.value;
        let mut args = Vec::new();
        while self.is_type_atom_start(&self.peek().kind) {
            args.push(self.parse_ctor_arg_type()?);
        }
        Ok(DataConstructor { name, args, span })
    }

    fn parse_ctor_arg_type(&mut self) -> Result<TypeExpr, ParseError> {
        let mut ty = self.parse_type_atom()?;
        while self.is_type_atom_start(&self.peek().kind) {
            let next = self.parse_type_atom()?;
            ty = TypeExpr::TEApp(Box::new(ty), Box::new(next));
        }
        Ok(ty)
    }

    fn parse_class_decl(&mut self) -> Result<ClassDecl, ParseError> {
        let class_tok = self.pop(TokenKind::CLASS)?;
        let span = span_from_token(&class_tok);
        let mut super_constraints = Vec::new();
        if let Some(ctx) = self.try_parse_class_context()? {
            super_constraints = ctx;
        }
        let name_tok = self.pop(TokenKind::CONID)?;
        let typevar = if self.peek().kind == TokenKind::VARID {
            Some(self.pop_any().value)
        } else {
            None
        };
        if self.accept(TokenKind::WHERE).is_some() {
            let next = self.peek().clone();
            return Err(ParseError::at(
                "PAR510",
                "class where ブロックは現在サポートされていません",
                Some(next.pos),
                Some(next.line),
                Some(next.col),
            ));
        }
        Ok(ClassDecl {
            name: name_tok.value,
            typevar,
            superclasses: super_constraints.into_iter().map(|c| c.classname).collect(),
            span,
        })
    }

    fn parse_instance_decl(&mut self) -> Result<InstanceDecl, ParseError> {
        let inst_tok = self.pop(TokenKind::INSTANCE)?;
        let span = span_from_token(&inst_tok);
        let class_tok = self.pop(TokenKind::CONID)?;
        let tycon = match self.peek().kind {
            TokenKind::CONID => self.pop_any().value,
            TokenKind::LBRACK => {
                self.pop_any();
                self.pop(TokenKind::RBRACK)?;
                "[]".to_string()
            }
            _ => {
                let tok = self.peek().clone();
                return Err(ParseError::at(
                    "PAR511",
                    format!(
                        "instance 対象の型は大文字で始まる識別子か [] である必要があります: {:?} {}",
                        tok.kind, tok.value
                    ),
                    Some(tok.pos),
                    Some(tok.line),
                    Some(tok.col),
                ));
            }
        };
        if self.accept(TokenKind::WHERE).is_some() {
            let next = self.peek().clone();
            return Err(ParseError::at(
                "PAR512",
                "instance where ブロックは現在サポートされていません",
                Some(next.pos),
                Some(next.line),
                Some(next.col),
            ));
        }
        Ok(InstanceDecl {
            classname: class_tok.value,
            tycon,
            span,
        })
    }

    fn try_parse_class_context(&mut self) -> Result<Option<Vec<AConstraint>>, ParseError> {
        if self.peek().kind == TokenKind::LPAREN {
            let save = self.i;
            self.pop_any();
            if self.peek().kind != TokenKind::CONID {
                self.i = save;
                return Ok(None);
            }
            let mut constraints = Vec::new();
            constraints.push(self.parse_class_constraint()?);
            while self.accept(TokenKind::COMMA).is_some() {
                constraints.push(self.parse_class_constraint()?);
            }
            if self.accept(TokenKind::RPAREN).is_some() && self.accept(TokenKind::DARROW).is_some()
            {
                return Ok(Some(constraints));
            }
            self.i = save;
            return Ok(None);
        }
        if self.peek().kind == TokenKind::CONID
            && matches!(self.peek_kind(1), Some(TokenKind::VARID))
            && matches!(self.peek_kind(2), Some(TokenKind::DARROW))
        {
            let constraint = self.parse_class_constraint()?;
            self.pop(TokenKind::DARROW)?;
            return Ok(Some(vec![constraint]));
        }
        Ok(None)
    }

    fn parse_class_constraint(&mut self) -> Result<AConstraint, ParseError> {
        let class_tok = self.pop(TokenKind::CONID)?;
        let tyvar_tok = self.pop(TokenKind::VARID)?;
        Ok(AConstraint {
            classname: class_tok.value,
            typevar: tyvar_tok.value,
        })
    }
}
