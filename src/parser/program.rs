// パス: src/parser/program.rs
// 役割: トップレベル宣言・データ宣言の構文解析ルーチンを実装する
// 意図: プログラム全体の解析ロジックを `Parser` から分離し可読性を高める
// 関連ファイル: src/parser/expr.rs, src/parser/types.rs, src/parser/mod.rs

use super::*;

impl Parser {
    pub(super) fn parse_program(&mut self) -> Result<Program, ParseError> {
        let mut decls = Vec::new();
        let mut data_decls = Vec::new();
        while self.peek().kind != TokenKind::EOF {
            if self.peek().kind == TokenKind::SEMI {
                self.pop_any();
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
        Ok(Program { data_decls, decls })
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
}
