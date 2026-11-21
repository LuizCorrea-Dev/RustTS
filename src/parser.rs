use crate::ast::*;
use crate::lexer::{Span, Token, TokenKind};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

/// Parser Descendente Recursivo Canônico da linguagem RustTS (`.rts`).
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    disallow_struct_lit: bool,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            pos: 0,
            disallow_struct_lit: false,
        }
    }

    fn parse_expr_no_struct(&mut self) -> Result<Expr, ParseError> {
        let prev = self.disallow_struct_lit;
        self.disallow_struct_lit = true;
        let res = self.parse_expr();
        self.disallow_struct_lit = prev;
        res
    }

    pub fn parse_module(&mut self) -> Result<Module, ParseError> {
        let start_span = self.current_span();
        let mut path = Vec::new();

        if self.check(&TokenKind::KwModule) {
            self.advance();
            path.push(self.expect_path_segment()?);
            while self.match_token(&TokenKind::Dot) {
                path.push(self.expect_path_segment()?);
            }
            self.expect(&TokenKind::Semicolon)?;
        } else {
            path.push("root".to_string());
        }

        let mut imports = Vec::new();
        while self.match_token(&TokenKind::KwImport) {
            let mut imp_path = vec![self.expect_path_segment()?];
            while self.match_token(&TokenKind::Dot) {
                imp_path.push(self.expect_path_segment()?);
            }
            self.expect(&TokenKind::Semicolon)?;
            imports.push(imp_path);
        }

        let mut items = Vec::new();
        while !self.is_at_end() {
            if self.check(&TokenKind::KwImpl) {
                let mut impl_methods = self.parse_impl_block()?;
                items.append(&mut impl_methods);
            } else {
                items.push(self.parse_item()?);
            }
        }

        let end_span = self.previous_span();
        Ok(Module {
            path,
            imports,
            items,
            span: start_span.merge(end_span),
        })
    }

    fn is_attr_token(kind: &TokenKind) -> bool {
        matches!(
            kind,
            TokenKind::AttrComponent
                | TokenKind::AttrGpuKernel
                | TokenKind::AttrVertexShader
                | TokenKind::AttrFragmentShader
                | TokenKind::AttrShaderShared
                | TokenKind::AttrReprC
                | TokenKind::AttrView
                | TokenKind::AttrSealedSig
                | TokenKind::AttrOpenSig
                | TokenKind::AttrSystem
                | TokenKind::AttrParallelSystem
                | TokenKind::AttrUnrealActor
                | TokenKind::AttrUnrealImpl
                | TokenKind::AttrUProperty
                | TokenKind::AttrRoute
        )
    }

    fn parse_item(&mut self) -> Result<Item, ParseError> {
        let mut attributes = self.parse_attributes()?;
        let had_leading_pub = if attributes.is_empty()
            && self.check(&TokenKind::KwPub)
            && self
                .tokens
                .get(self.pos + 1)
                .map_or(false, |t| Self::is_attr_token(&t.kind))
        {
            self.advance();
            attributes = self.parse_attributes()?;
            true
        } else {
            false
        };

        if self.check(&TokenKind::KwStruct)
            || (self.check(&TokenKind::KwPub) && self.peek_next_is(&TokenKind::KwStruct))
        {
            Ok(Item::Struct(self.parse_struct(attributes)?))
        } else if self.check(&TokenKind::KwEnum)
            || (self.check(&TokenKind::KwPub) && self.peek_next_is(&TokenKind::KwEnum))
        {
            Ok(Item::Enum(self.parse_enum()?))
        } else if self.check(&TokenKind::KwConst)
            || (self.check(&TokenKind::KwPub) && self.peek_next_is(&TokenKind::KwConst))
        {
            let mut c = self.parse_const()?;
            c.is_pub |= had_leading_pub;
            Ok(Item::Const(c))
        } else if self.check(&TokenKind::KwExtern)
            || (self.check(&TokenKind::KwPub) && self.peek_next_is_extern())
        {
            let mut e = self.parse_extern_fn(attributes)?;
            e.is_pub |= had_leading_pub;
            Ok(Item::ExternFn(e))
        } else if self.check(&TokenKind::KwPub) || self.check(&TokenKind::KwFn) || had_leading_pub {
            let mut f = self.parse_fn(attributes)?;
            f.is_pub |= had_leading_pub;
            Ok(Item::Function(f))
        } else {
            Err(ParseError {
                message: format!(
                    "Expected 'const', 'struct', 'enum', 'impl', 'extern', 'pub fn', or 'fn', found '{}'",
                    self.peek_lexeme()
                ),
                span: self.current_span(),
            })
        }
    }

    fn peek_next_is(&self, kind: &TokenKind) -> bool {
        self.tokens
            .get(self.pos + 1)
            .map(|t| &t.kind == kind)
            .unwrap_or(false)
    }

    fn peek_next_is_extern(&self) -> bool {
        self.peek_next_is(&TokenKind::KwExtern)
    }

    fn parse_const(&mut self) -> Result<ConstDecl, ParseError> {
        let start = self.current_span();
        let is_pub = self.match_token(&TokenKind::KwPub);
        self.expect(&TokenKind::KwConst)?;
        let name = self.expect_ident()?;
        self.expect(&TokenKind::Colon)?;
        let ty = self.parse_type_expr()?;
        self.expect(&TokenKind::Assign)?;
        let value = self.parse_expr()?;
        let end = self.expect(&TokenKind::Semicolon)?.span;
        Ok(ConstDecl {
            is_pub,
            name,
            ty,
            value,
            span: start.merge(end),
        })
    }

    fn parse_extern_fn(&mut self, attributes: Vec<Attribute>) -> Result<ExternFnDecl, ParseError> {
        let start = self.current_span();
        let is_pub = self.match_token(&TokenKind::KwPub);
        self.expect(&TokenKind::KwExtern)?;
        let abi = if let Some(TokenKind::StringLit(s)) = self.peek_kind() {
            let a = s.clone();
            self.advance();
            a
        } else {
            "C".to_string()
        };
        self.expect(&TokenKind::KwFn)?;
        let name = self.expect_ident()?;
        self.expect(&TokenKind::LParen)?;

        let mut params = Vec::new();
        while !self.check(&TokenKind::RParen) && !self.is_at_end() {
            params.push(self.parse_fn_param()?);
            let _ = self.match_token(&TokenKind::Comma);
        }
        self.expect(&TokenKind::RParen)?;

        let return_type = if self.match_token(&TokenKind::Arrow) {
            Some(self.parse_type_expr()?)
        } else {
            None
        };

        let end = self.expect(&TokenKind::Semicolon)?.span;
        Ok(ExternFnDecl {
            attributes,
            is_pub,
            abi,
            name,
            params,
            return_type,
            span: start.merge(end),
        })
    }

    fn parse_impl_block(&mut self) -> Result<Vec<Item>, ParseError> {
        self.expect(&TokenKind::KwImpl)?;
        let target_struct = self.expect_ident()?;
        self.expect(&TokenKind::LBrace)?;
        let mut methods = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            let mut attrs = self.parse_attributes()?;
            if !attrs
                .iter()
                .any(|a| matches!(a, Attribute::UnrealImpl { .. }))
            {
                attrs.push(Attribute::UnrealImpl {
                    target_struct: target_struct.clone(),
                });
            }
            let func = self.parse_fn(attrs)?;
            methods.push(Item::Function(func));
        }
        self.expect(&TokenKind::RBrace)?;
        Ok(methods)
    }

    fn parse_attributes(&mut self) -> Result<Vec<Attribute>, ParseError> {
        let mut attrs = Vec::new();
        while !self.is_at_end() {
            match self.peek_kind() {
                Some(TokenKind::AttrComponent) => {
                    self.advance();
                    attrs.push(Attribute::Component);
                }
                Some(TokenKind::AttrGpuKernel) => {
                    self.advance();
                    attrs.push(Attribute::GpuKernel);
                }
                Some(TokenKind::AttrVertexShader) => {
                    self.advance();
                    attrs.push(Attribute::VertexShader);
                }
                Some(TokenKind::AttrFragmentShader) => {
                    self.advance();
                    attrs.push(Attribute::FragmentShader);
                }
                Some(TokenKind::AttrShaderShared) => {
                    self.advance();
                    attrs.push(Attribute::ShaderShared);
                }
                Some(TokenKind::AttrReprC) => {
                    self.advance();
                    attrs.push(Attribute::ReprC);
                }
                Some(TokenKind::AttrView) => {
                    self.advance();
                    attrs.push(Attribute::View);
                }
                Some(TokenKind::AttrSealedSig) => {
                    self.advance();
                    attrs.push(Attribute::SealedSig);
                }
                Some(TokenKind::AttrOpenSig) => {
                    self.advance();
                    attrs.push(Attribute::OpenSig);
                }
                Some(TokenKind::AttrSystem) => {
                    self.advance();
                    let mut stage = "Update".to_string();
                    if self.match_token(&TokenKind::LParen) {
                        let key = self.expect_ident()?;
                        self.expect(&TokenKind::Assign)?;
                        let val = self.expect_string_lit()?;
                        if key == "stage" {
                            stage = val;
                        }
                        self.expect(&TokenKind::RParen)?;
                    }
                    attrs.push(Attribute::System { stage });
                }
                Some(TokenKind::AttrParallelSystem) => {
                    self.advance();
                    let mut stage = "Update".to_string();
                    if self.match_token(&TokenKind::LParen) {
                        let key = self.expect_ident()?;
                        self.expect(&TokenKind::Assign)?;
                        let val = self.expect_string_lit()?;
                        if key == "stage" {
                            stage = val;
                        }
                        self.expect(&TokenKind::RParen)?;
                    }
                    attrs.push(Attribute::ParallelSystem { stage });
                }
                Some(TokenKind::AttrUnrealActor) => {
                    self.advance();
                    let mut replicate = false;
                    let mut category = "Default".to_string();
                    if self.match_token(&TokenKind::LParen) {
                        while !self.check(&TokenKind::RParen) && !self.is_at_end() {
                            let key = self.expect_ident()?;
                            self.expect(&TokenKind::Assign)?;
                            if key == "replicate" {
                                replicate = self.expect_bool_lit()?;
                            } else if key == "category" {
                                category = self.expect_string_lit()?;
                            }
                            let _ = self.match_token(&TokenKind::Comma);
                        }
                        self.expect(&TokenKind::RParen)?;
                    }
                    attrs.push(Attribute::UnrealActor {
                        replicate,
                        category,
                    });
                }
                Some(TokenKind::AttrUnrealImpl) => {
                    self.advance();
                    self.expect(&TokenKind::LParen)?;
                    let target_struct = self.expect_ident()?;
                    self.expect(&TokenKind::RParen)?;
                    attrs.push(Attribute::UnrealImpl { target_struct });
                }
                Some(TokenKind::AttrUProperty) => {
                    self.advance();
                    let mut flags = Vec::new();
                    if self.match_token(&TokenKind::LParen) {
                        while !self.check(&TokenKind::RParen) && !self.is_at_end() {
                            flags.push(self.expect_ident()?);
                            let _ = self.match_token(&TokenKind::Comma);
                        }
                        self.expect(&TokenKind::RParen)?;
                    }
                    attrs.push(Attribute::UProperty { flags });
                }
                Some(TokenKind::AttrGodotNode) => {
                    self.advance();
                    let mut base_class = "Node".to_string();
                    let mut category = "Default".to_string();
                    if self.match_token(&TokenKind::LParen) {
                        while !self.check(&TokenKind::RParen) && !self.is_at_end() {
                            let key = self.expect_ident()?;
                            self.expect(&TokenKind::Assign)?;
                            let val = self.expect_string_lit()?;
                            if key == "base" {
                                base_class = val;
                            } else if key == "category" {
                                category = val;
                            }
                            let _ = self.match_token(&TokenKind::Comma);
                        }
                        self.expect(&TokenKind::RParen)?;
                    }
                    attrs.push(Attribute::GodotNode {
                        base_class,
                        category,
                    });
                }
                Some(TokenKind::AttrGodotImpl) => {
                    self.advance();
                    self.expect(&TokenKind::LParen)?;
                    let target_struct = self.expect_ident()?;
                    self.expect(&TokenKind::RParen)?;
                    attrs.push(Attribute::GodotImpl { target_struct });
                }
                Some(TokenKind::AttrGdExport) => {
                    self.advance();
                    let mut hint = None;
                    let mut hint_string = None;
                    if self.match_token(&TokenKind::LParen) {
                        while !self.check(&TokenKind::RParen) && !self.is_at_end() {
                            let key = self.expect_ident()?;
                            self.expect(&TokenKind::Assign)?;
                            let val = self.expect_string_lit()?;
                            if key == "hint" {
                                hint = Some(val);
                            } else if key == "hint_string" {
                                hint_string = Some(val);
                            }
                            let _ = self.match_token(&TokenKind::Comma);
                        }
                        self.expect(&TokenKind::RParen)?;
                    }
                    attrs.push(Attribute::GdExport { hint, hint_string });
                }
                Some(TokenKind::AttrRoute) => {
                    self.advance();
                    self.expect(&TokenKind::LParen)?;
                    let method = self.expect_string_lit()?;
                    self.expect(&TokenKind::Comma)?;
                    let path = self.expect_string_lit()?;
                    self.expect(&TokenKind::RParen)?;
                    attrs.push(Attribute::Route { method, path });
                }
                _ => break,
            }
        }
        Ok(attrs)
    }

    fn parse_struct(&mut self, attributes: Vec<Attribute>) -> Result<StructDecl, ParseError> {
        let pub_span = if self.check(&TokenKind::KwPub) {
            Some(self.advance().unwrap().span)
        } else {
            None
        };
        let struct_span = self.expect(&TokenKind::KwStruct)?.span;
        let start = pub_span.unwrap_or(struct_span);
        let name = self.expect_ident()?;
        self.expect(&TokenKind::LBrace)?;
        let mut fields = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            let field_attrs = self.parse_attributes()?;
            let f_start = self.current_span();
            let _ = self.match_token(&TokenKind::KwPub);
            let field_name = self.expect_ident()?;
            self.expect(&TokenKind::Colon)?;
            let ty = self.parse_type_expr()?;
            let f_end = self.previous_span();
            let _ = self.match_token(&TokenKind::Comma);
            fields.push(StructField {
                attributes: field_attrs,
                name: field_name,
                ty,
                span: f_start.merge(f_end),
            });
        }
        let end = self.expect(&TokenKind::RBrace)?.span;
        Ok(StructDecl {
            attributes,
            name,
            fields,
            span: start.merge(end),
        })
    }

    fn parse_enum(&mut self) -> Result<EnumDecl, ParseError> {
        let pub_span = if self.check(&TokenKind::KwPub) {
            Some(self.advance().unwrap().span)
        } else {
            None
        };
        let enum_span = self.expect(&TokenKind::KwEnum)?.span;
        let start = pub_span.unwrap_or(enum_span);
        let name = self.expect_ident()?;
        self.expect(&TokenKind::LBrace)?;
        let mut variants = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            let v_start = self.current_span();
            let v_name = self.expect_ident()?;
            let payload = if self.match_token(&TokenKind::LParen) {
                let ty = self.parse_type_expr()?;
                self.expect(&TokenKind::RParen)?;
                Some(ty)
            } else {
                None
            };
            let v_end = self.previous_span();
            let _ = self.match_token(&TokenKind::Comma);
            variants.push(EnumVariant {
                name: v_name,
                payload,
                span: v_start.merge(v_end),
            });
        }
        let end = self.expect(&TokenKind::RBrace)?.span;
        Ok(EnumDecl {
            name,
            variants,
            span: start.merge(end),
        })
    }

    fn parse_fn(&mut self, attributes: Vec<Attribute>) -> Result<FnDecl, ParseError> {
        let start = self.current_span();
        let is_pub = self.match_token(&TokenKind::KwPub);
        self.expect(&TokenKind::KwFn)?;
        let name = self.expect_ident()?;
        self.expect(&TokenKind::LParen)?;

        let mut params = Vec::new();
        while !self.check(&TokenKind::RParen) && !self.is_at_end() {
            params.push(self.parse_fn_param()?);
            let _ = self.match_token(&TokenKind::Comma);
        }
        self.expect(&TokenKind::RParen)?;

        let return_type = if self.match_token(&TokenKind::Arrow) {
            Some(self.parse_type_expr()?)
        } else {
            None
        };

        let body = self.parse_block()?;
        let end = self.previous_span();

        Ok(FnDecl {
            attributes,
            is_pub,
            name,
            params,
            return_type,
            body,
            span: start.merge(end),
            source_file: None,
        })
    }

    fn parse_fn_param(&mut self) -> Result<FnParam, ParseError> {
        let p_start = self.current_span();
        let p_name = self.expect_ident()?;
        self.expect(&TokenKind::Colon)?;
        let is_cap = self.match_token(&TokenKind::KwCap);
        let is_inout = if !is_cap {
            self.match_token(&TokenKind::KwInout)
        } else {
            false
        };
        let (ty, capability) = if is_cap {
            let cap = self.parse_capability_spec()?;
            (TypeExpr::Capability(cap.clone()), Some(cap))
        } else {
            (self.parse_type_expr()?, None)
        };
        let p_end = self.previous_span();
        Ok(FnParam {
            name: p_name,
            is_cap,
            is_inout,
            ty,
            capability,
            span: p_start.merge(p_end),
        })
    }

    fn parse_capability_spec(&mut self) -> Result<CapabilitySpec, ParseError> {
        let domain_name = self.expect_ident()?;
        let generic_arg = if self.match_token(&TokenKind::LAngle) {
            let arg = match self.peek_kind() {
                Some(TokenKind::HexLit(val)) | Some(TokenKind::IntLit(val)) => {
                    let v = *val;
                    self.advance();
                    CapGenericArg::HardwareAddress(v)
                }
                _ => {
                    let ty = self.parse_type_expr()?;
                    if let TypeExpr::Named { name, generics } = &ty {
                        if generics.is_empty() {
                            CapGenericArg::TypeName(name.clone())
                        } else {
                            CapGenericArg::ElementType(Box::new(ty))
                        }
                    } else {
                        CapGenericArg::ElementType(Box::new(ty))
                    }
                }
            };
            self.expect_closing_rangle()?;
            Some(arg)
        } else {
            None
        };

        Ok(CapabilitySpec::from_name_and_arg(&domain_name, generic_arg))
    }

    fn expect_closing_rangle(&mut self) -> Result<Span, ParseError> {
        if self.match_token(&TokenKind::RAngle) {
            return Ok(self.previous_span());
        }
        if let Some(tok) = self.tokens.get_mut(self.pos) {
            if tok.kind == TokenKind::Shr {
                let first_span = Span::new(
                    tok.span.line_start,
                    tok.span.col_start,
                    tok.span.line_start,
                    tok.span.col_start + 1,
                    tok.span.byte_start,
                    tok.span.byte_start + 1,
                );
                tok.kind = TokenKind::RAngle;
                tok.lexeme = ">".to_string();
                tok.span.col_start += 1;
                tok.span.byte_start += 1;
                return Ok(first_span);
            }
        }
        Err(ParseError {
            message: format!("Expected '>', found '{}'", self.peek_lexeme()),
            span: self.current_span(),
        })
    }

    fn parse_type_expr(&mut self) -> Result<TypeExpr, ParseError> {
        if self.match_token(&TokenKind::KwCap) {
            let cap = self.parse_capability_spec()?;
            return Ok(TypeExpr::Capability(cap));
        }
        if self.match_token(&TokenKind::LBracket) {
            let inner = self.parse_type_expr()?;
            self.expect(&TokenKind::Semicolon)?;
            let tok = self.advance().cloned().ok_or_else(|| ParseError {
                span: self.current_span(),
                message: "Unexpected EOF in fixed array length `[T; N]`".to_string(),
            })?;
            let len = match tok.kind {
                TokenKind::IntLit(n) => n as usize,
                _ => {
                    return Err(ParseError {
                        span: tok.span,
                        message: format!(
                            "Expected fixed integer array length `[T; N]`, found {:?}",
                            tok.kind
                        ),
                    });
                }
            };
            self.expect(&TokenKind::RBracket)?;
            return Ok(TypeExpr::Array(Box::new(inner), len));
        }

        let eof_span = self.current_span();
        let tok = self.advance().cloned().ok_or_else(|| ParseError {
            message: "Unexpected EOF while parsing type".to_string(),
            span: eof_span,
        })?;

        match &tok.kind {
            TokenKind::TyI8 => Ok(TypeExpr::Primitive(PrimitiveType::I8)),
            TokenKind::TyI16 => Ok(TypeExpr::Primitive(PrimitiveType::I16)),
            TokenKind::TyI32 => Ok(TypeExpr::Primitive(PrimitiveType::I32)),
            TokenKind::TyI64 => Ok(TypeExpr::Primitive(PrimitiveType::I64)),
            TokenKind::TyU8 => Ok(TypeExpr::Primitive(PrimitiveType::U8)),
            TokenKind::TyU16 => Ok(TypeExpr::Primitive(PrimitiveType::U16)),
            TokenKind::TyU32 => Ok(TypeExpr::Primitive(PrimitiveType::U32)),
            TokenKind::TyU64 => Ok(TypeExpr::Primitive(PrimitiveType::U64)),
            TokenKind::TyUsize => Ok(TypeExpr::Primitive(PrimitiveType::Usize)),
            TokenKind::TyF32 => Ok(TypeExpr::Primitive(PrimitiveType::F32)),
            TokenKind::TyF64 => Ok(TypeExpr::Primitive(PrimitiveType::F64)),
            TokenKind::TyFix32 => Ok(TypeExpr::Primitive(PrimitiveType::Fix32)),
            TokenKind::TyBool => Ok(TypeExpr::Primitive(PrimitiveType::Bool)),
            TokenKind::TyStr => Ok(TypeExpr::Primitive(PrimitiveType::Str)),
            TokenKind::TyStr64 => Ok(TypeExpr::Primitive(PrimitiveType::Str64)),
            TokenKind::TyStr128 => Ok(TypeExpr::Primitive(PrimitiveType::Str128)),
            TokenKind::TyStr256 => Ok(TypeExpr::Primitive(PrimitiveType::Str256)),
            TokenKind::TyVec2
            | TokenKind::TyVec3
            | TokenKind::TyVec4
            | TokenKind::TyQuat
            | TokenKind::TyMat3
            | TokenKind::TyMat4 => {
                if self.match_token(&TokenKind::ColonColon) {
                    let ty_name = self.expect_ident()?;
                    let mut generics = Vec::new();
                    if self.match_token(&TokenKind::LAngle) {
                        while !self.check(&TokenKind::RAngle)
                            && !self.check(&TokenKind::Shr)
                            && !self.is_at_end()
                        {
                            generics.push(self.parse_type_expr()?);
                            let _ = self.match_token(&TokenKind::Comma);
                        }
                        self.expect_closing_rangle()?;
                    }
                    Ok(TypeExpr::Named {
                        name: ty_name,
                        generics,
                    })
                } else {
                    Ok(match &tok.kind {
                        TokenKind::TyVec2 => TypeExpr::Math(MathType::Vec2),
                        TokenKind::TyVec3 => TypeExpr::Math(MathType::Vec3),
                        TokenKind::TyVec4 => TypeExpr::Math(MathType::Vec4),
                        TokenKind::TyQuat => TypeExpr::Math(MathType::Quat),
                        TokenKind::TyMat3 => TypeExpr::Math(MathType::Mat3),
                        _ => TypeExpr::Math(MathType::Mat4),
                    })
                }
            }
            TokenKind::TyOption => {
                self.expect(&TokenKind::LAngle)?;
                let inner = self.parse_type_expr()?;
                self.expect_closing_rangle()?;
                Ok(TypeExpr::Option(Box::new(inner)))
            }
            TokenKind::TyResult => {
                self.expect(&TokenKind::LAngle)?;
                let ok_ty = self.parse_type_expr()?;
                self.expect(&TokenKind::Comma)?;
                let err_ty = self.parse_type_expr()?;
                self.expect_closing_rangle()?;
                Ok(TypeExpr::Result(Box::new(ok_ty), Box::new(err_ty)))
            }
            TokenKind::TyHandle => {
                self.expect(&TokenKind::LAngle)?;
                let inner = self.parse_type_expr()?;
                self.expect_closing_rangle()?;
                Ok(TypeExpr::Handle(Box::new(inner)))
            }
            TokenKind::Ident(name) => {
                let mut name_cloned = name.clone();
                if self.match_token(&TokenKind::ColonColon) {
                    name_cloned = self.expect_ident()?;
                }
                let mut generics = Vec::new();
                if self.match_token(&TokenKind::LAngle) {
                    while !self.check(&TokenKind::RAngle)
                        && !self.check(&TokenKind::Shr)
                        && !self.is_at_end()
                    {
                        generics.push(self.parse_type_expr()?);
                        let _ = self.match_token(&TokenKind::Comma);
                    }
                    self.expect_closing_rangle()?;
                }
                Ok(TypeExpr::Named {
                    name: name_cloned,
                    generics,
                })
            }
            _ => Err(ParseError {
                message: format!("Invalid type token '{}'", tok.lexeme),
                span: tok.span,
            }),
        }
    }

    fn parse_block(&mut self) -> Result<Vec<Stmt>, ParseError> {
        self.expect(&TokenKind::LBrace)?;
        let mut stmts = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            stmts.push(self.parse_stmt()?);
        }
        self.expect(&TokenKind::RBrace)?;
        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start = self.current_span();

        if self.match_token(&TokenKind::KwLet) {
            let is_mut = self.match_token(&TokenKind::KwMut);
            let name = self.expect_ident()?;
            let ty = if self.match_token(&TokenKind::Colon) {
                Some(self.parse_type_expr()?)
            } else {
                None
            };
            self.expect(&TokenKind::Assign)?;
            let value = self.parse_expr()?;
            let end = self.expect(&TokenKind::Semicolon)?.span;
            return Ok(Stmt::Let {
                name,
                is_mut,
                ty,
                value,
                span: start.merge(end),
            });
        }

        if self.match_token(&TokenKind::KwReturn) {
            let value = if !self.check(&TokenKind::Semicolon) {
                Some(self.parse_expr()?)
            } else {
                None
            };
            let end = self.expect(&TokenKind::Semicolon)?.span;
            return Ok(Stmt::Return {
                value,
                span: start.merge(end),
            });
        }

        if self.match_token(&TokenKind::KwIf) {
            let condition = self.parse_expr_no_struct()?;
            let then_branch = self.parse_block()?;
            let else_branch = if self.match_token(&TokenKind::KwElse) {
                if self.check(&TokenKind::KwIf) {
                    Some(vec![self.parse_stmt()?])
                } else {
                    Some(self.parse_block()?)
                }
            } else {
                None
            };
            let end = self.previous_span();
            return Ok(Stmt::If {
                condition,
                then_branch,
                else_branch,
                span: start.merge(end),
            });
        }

        if self.match_token(&TokenKind::KwMatch) {
            let scrutinee = self.parse_expr_no_struct()?;
            self.expect(&TokenKind::LBrace)?;
            let mut arms = Vec::new();
            while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
                let arm_start = self.current_span();
                let pattern = self.parse_match_pattern()?;
                self.expect(&TokenKind::FatArrow)?;
                let body = if self.check(&TokenKind::LBrace) {
                    self.parse_block()?
                } else {
                    vec![self.parse_stmt()?]
                };
                let _ = self.match_token(&TokenKind::Comma);
                let arm_end = self.previous_span();
                arms.push(MatchArm {
                    pattern,
                    body,
                    span: arm_start.merge(arm_end),
                });
            }
            let end = self.expect(&TokenKind::RBrace)?.span;
            return Ok(Stmt::Match {
                expr: scrutinee,
                arms,
                span: start.merge(end),
            });
        }

        if self.match_token(&TokenKind::KwFor) {
            let iterator = self.expect_ident()?;
            self.expect(&TokenKind::KwIn)?;
            let range_start = self.parse_expr_no_struct()?;
            self.expect(&TokenKind::DotDot)?;
            let range_end = self.parse_expr_no_struct()?;
            let body = self.parse_block()?;
            let end = self.previous_span();
            return Ok(Stmt::ForRange {
                iterator,
                start: range_start,
                end: range_end,
                body,
                span: start.merge(end),
            });
        }

        if self.match_token(&TokenKind::KwWhile) {
            let condition = self.parse_expr_no_struct()?;
            let body = self.parse_block()?;
            let end = self.previous_span();
            return Ok(Stmt::While {
                condition,
                body,
                span: start.merge(end),
            });
        }
        if self.match_token(&TokenKind::KwBreak) {
            self.expect(&TokenKind::Semicolon)?;
            let end = self.previous_span();
            return Ok(Stmt::Break {
                span: start.merge(end),
            });
        }
        if self.match_token(&TokenKind::KwContinue) {
            self.expect(&TokenKind::Semicolon)?;
            let end = self.previous_span();
            return Ok(Stmt::Continue {
                span: start.merge(end),
            });
        }

        let expr = self.parse_expr()?;
        if self.match_token(&TokenKind::Assign) {
            let value = self.parse_expr()?;
            let end = self.expect(&TokenKind::Semicolon)?.span;
            return Ok(Stmt::Assign {
                target: expr,
                value,
                span: start.merge(end),
            });
        }
        if let Some(compound_op) = match self.peek_kind() {
            Some(TokenKind::PlusEq) => Some(BinaryOp::Add),
            Some(TokenKind::MinusEq) => Some(BinaryOp::Sub),
            Some(TokenKind::StarEq) => Some(BinaryOp::Mul),
            Some(TokenKind::SlashEq) => Some(BinaryOp::Div),
            Some(TokenKind::PercentEq) => Some(BinaryOp::Rem),
            Some(TokenKind::AmpEq) => Some(BinaryOp::BitAnd),
            Some(TokenKind::PipeEq) => Some(BinaryOp::BitOr),
            Some(TokenKind::CaretEq) => Some(BinaryOp::BitXor),
            Some(TokenKind::ShlEq) => Some(BinaryOp::Shl),
            Some(TokenKind::ShrEq) => Some(BinaryOp::Shr),
            _ => None,
        } {
            self.advance();
            let rhs = self.parse_expr()?;
            let end = self.expect(&TokenKind::Semicolon)?.span;
            let full_span = start.merge(end);
            let value = Expr::Binary {
                left: Box::new(expr.clone()),
                op: compound_op,
                right: Box::new(rhs),
                span: full_span,
            };
            return Ok(Stmt::Assign {
                target: expr,
                value,
                span: full_span,
            });
        }

        let end = self.expect(&TokenKind::Semicolon)?.span;
        Ok(Stmt::ExprStmt {
            expr,
            span: start.merge(end),
        })
    }

    fn parse_match_pattern(&mut self) -> Result<MatchPattern, ParseError> {
        if let Some(TokenKind::Ident(s)) = self.peek_kind() {
            if s == "_" {
                self.advance();
                return Ok(MatchPattern::Wildcard);
            }
        }

        if self.match_token(&TokenKind::KwOk) {
            self.expect(&TokenKind::LParen)?;
            let bind = self.expect_ident()?;
            self.expect(&TokenKind::RParen)?;
            return Ok(MatchPattern::Variant {
                enum_name: None,
                variant: "Ok".to_string(),
                binding: Some(bind),
            });
        }
        if self.match_token(&TokenKind::KwErr) {
            self.expect(&TokenKind::LParen)?;
            let bind = self.expect_ident()?;
            self.expect(&TokenKind::RParen)?;
            return Ok(MatchPattern::Variant {
                enum_name: None,
                variant: "Err".to_string(),
                binding: Some(bind),
            });
        }
        if self.match_token(&TokenKind::KwSome) {
            self.expect(&TokenKind::LParen)?;
            let bind = self.expect_ident()?;
            self.expect(&TokenKind::RParen)?;
            return Ok(MatchPattern::Variant {
                enum_name: None,
                variant: "Some".to_string(),
                binding: Some(bind),
            });
        }
        if self.match_token(&TokenKind::KwNone) {
            return Ok(MatchPattern::Variant {
                enum_name: None,
                variant: "None".to_string(),
                binding: None,
            });
        }

        let first = self.expect_ident()?;
        let (enum_name, variant) = if self.match_token(&TokenKind::ColonColon) {
            let v = self.expect_ident()?;
            (Some(first), v)
        } else {
            (None, first)
        };

        let binding = if self.match_token(&TokenKind::LParen) {
            let b = self.expect_ident()?;
            self.expect(&TokenKind::RParen)?;
            Some(b)
        } else {
            None
        };

        Ok(MatchPattern::Variant {
            enum_name,
            variant,
            binding,
        })
    }

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_logical_or()
    }

    fn parse_logical_or(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_logical_and()?;
        while self.match_token(&TokenKind::OrOr) {
            let right = self.parse_logical_and()?;
            let span = left.span().merge(right.span());
            left = Expr::Binary {
                left: Box::new(left),
                op: BinaryOp::Or,
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_logical_and(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_comparison()?;
        while self.match_token(&TokenKind::AndAnd) {
            let right = self.parse_comparison()?;
            let span = left.span().merge(right.span());
            left = Expr::Binary {
                left: Box::new(left),
                op: BinaryOp::And,
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_comparison(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_bit_or()?;
        while let Some(op) = match self.peek_kind() {
            Some(TokenKind::EqEq) => Some(BinaryOp::Eq),
            Some(TokenKind::NotEq) => Some(BinaryOp::NotEq),
            Some(TokenKind::LAngle) => Some(BinaryOp::Lt),
            Some(TokenKind::LtEq) => Some(BinaryOp::LtEq),
            Some(TokenKind::RAngle) => Some(BinaryOp::Gt),
            Some(TokenKind::GtEq) => Some(BinaryOp::GtEq),
            _ => None,
        } {
            self.advance();
            let right = self.parse_bit_or()?;
            let span = left.span().merge(right.span());
            left = Expr::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_bit_or(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_bit_xor()?;
        while self.match_token(&TokenKind::Pipe) {
            let right = self.parse_bit_xor()?;
            let span = left.span().merge(right.span());
            left = Expr::Binary {
                left: Box::new(left),
                op: BinaryOp::BitOr,
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_bit_xor(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_bit_and()?;
        while self.match_token(&TokenKind::Caret) {
            let right = self.parse_bit_and()?;
            let span = left.span().merge(right.span());
            left = Expr::Binary {
                left: Box::new(left),
                op: BinaryOp::BitXor,
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_bit_and(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_shift()?;
        while self.match_token(&TokenKind::Amp) {
            let right = self.parse_shift()?;
            let span = left.span().merge(right.span());
            left = Expr::Binary {
                left: Box::new(left),
                op: BinaryOp::BitAnd,
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_shift(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_additive()?;
        while let Some(op) = match self.peek_kind() {
            Some(TokenKind::Shl) => Some(BinaryOp::Shl),
            Some(TokenKind::Shr) => Some(BinaryOp::Shr),
            _ => None,
        } {
            self.advance();
            let right = self.parse_additive()?;
            let span = left.span().merge(right.span());
            left = Expr::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_additive(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_multiplicative()?;
        while let Some(op) = match self.peek_kind() {
            Some(TokenKind::Plus) => Some(BinaryOp::Add),
            Some(TokenKind::Minus) => Some(BinaryOp::Sub),
            _ => None,
        } {
            self.advance();
            let right = self.parse_multiplicative()?;
            let span = left.span().merge(right.span());
            left = Expr::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_unary()?;
        while let Some(op) = match self.peek_kind() {
            Some(TokenKind::Star) => Some(BinaryOp::Mul),
            Some(TokenKind::Slash) => Some(BinaryOp::Div),
            Some(TokenKind::Percent) => Some(BinaryOp::Rem),
            _ => None,
        } {
            self.advance();
            let right = self.parse_unary()?;
            let span = left.span().merge(right.span());
            left = Expr::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        let start = self.current_span();
        if self.match_token(&TokenKind::Minus) {
            let operand = self.parse_unary()?;
            let span = start.merge(operand.span());
            return Ok(Expr::Unary {
                op: UnaryOp::Neg,
                expr: Box::new(operand),
                span,
            });
        }
        if self.match_token(&TokenKind::Bang) {
            let operand = self.parse_unary()?;
            let span = start.merge(operand.span());
            return Ok(Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(operand),
                span,
            });
        }
        if self.match_token(&TokenKind::Tilde) {
            let operand = self.parse_unary()?;
            let span = start.merge(operand.span());
            return Ok(Expr::Unary {
                op: UnaryOp::BitNot,
                expr: Box::new(operand),
                span,
            });
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_primary()?;

        loop {
            if self.match_token(&TokenKind::Dot) {
                let member = self.expect_ident()?;
                if self.match_token(&TokenKind::LParen) {
                    let args = self.parse_arg_list()?;
                    let end = self.expect(&TokenKind::RParen)?.span;
                    let span = expr.span().merge(end);
                    expr = Expr::MethodCall {
                        receiver: Box::new(expr),
                        method: member,
                        args,
                        span,
                    };
                } else {
                    let span = expr.span().merge(self.previous_span());
                    expr = Expr::FieldAccess {
                        receiver: Box::new(expr),
                        field: member,
                        span,
                    };
                }
            } else if self.match_token(&TokenKind::LBracket) {
                let index = self.parse_expr()?;
                let end = self.expect(&TokenKind::RBracket)?.span;
                let span = expr.span().merge(end);
                expr = Expr::Index {
                    receiver: Box::new(expr),
                    index: Box::new(index),
                    span,
                };
            } else if self.match_token(&TokenKind::Question) {
                let span = expr.span().merge(self.previous_span());
                expr = Expr::TryOp {
                    expr: Box::new(expr),
                    span,
                };
            } else if self.match_token(&TokenKind::KwAs) {
                let target_type = self.parse_type_expr()?;
                let span = expr.span().merge(self.previous_span());
                expr = Expr::Cast {
                    expr: Box::new(expr),
                    target_type,
                    span,
                };
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        let start = self.current_span();

        // Expressão condicional inline determinística: `if cond { a } else { b }`
        if self.match_token(&TokenKind::KwIf) {
            let cond = self.parse_expr_no_struct()?;
            self.expect(&TokenKind::LBrace)?;
            let then_branch = self.parse_expr()?;
            self.expect(&TokenKind::RBrace)?;
            self.expect(&TokenKind::KwElse)?;
            let else_branch = if self.check(&TokenKind::KwIf) {
                self.parse_primary()?
            } else {
                self.expect(&TokenKind::LBrace)?;
                let eb = self.parse_expr()?;
                self.expect(&TokenKind::RBrace)?;
                eb
            };
            let end = self.previous_span();
            return Ok(Expr::IfExpr {
                cond: Box::new(cond),
                then_branch: Box::new(then_branch),
                else_branch: Box::new(else_branch),
                span: start.merge(end),
            });
        }

        // Closure curta `|_| ...` para `.map_err(|_| ...)`
        if self.match_token(&TokenKind::Pipe) {
            let _ = self.expect_ident();
            self.expect(&TokenKind::Pipe)?;
            let body = self.parse_expr()?;
            let span = start.merge(body.span());
            return Ok(Expr::ErrorMapClosure {
                body: Box::new(body),
                span,
            });
        }

        if self.match_token(&TokenKind::LBracket) {
            let prev_dis = self.disallow_struct_lit;
            self.disallow_struct_lit = false;
            let mut elements = Vec::new();
            if !self.check(&TokenKind::RBracket) && !self.is_at_end() {
                let first = self.parse_expr()?;
                if self.match_token(&TokenKind::Semicolon) {
                    let count = match self.peek_kind() {
                        Some(TokenKind::IntLit(n)) | Some(TokenKind::HexLit(n)) => {
                            let v = (*n).max(0) as usize;
                            self.advance();
                            v
                        }
                        _ => {
                            self.disallow_struct_lit = prev_dis;
                            return Err(ParseError {
                                message: format!(
                                    "Expected array repeat length integer, found '{}'",
                                    self.peek_lexeme()
                                ),
                                span: self.current_span(),
                            });
                        }
                    };
                    self.disallow_struct_lit = prev_dis;
                    self.expect(&TokenKind::RBracket)?;
                    let end = self.previous_span();
                    return Ok(Expr::ArrayLit {
                        elements: vec![first; count],
                        span: start.merge(end),
                    });
                }
                elements.push(first);
                while self.match_token(&TokenKind::Comma) {
                    if self.check(&TokenKind::RBracket) {
                        break;
                    }
                    elements.push(self.parse_expr()?);
                }
            }
            self.disallow_struct_lit = prev_dis;
            self.expect(&TokenKind::RBracket)?;
            let end = self.previous_span();
            return Ok(Expr::ArrayLit {
                elements,
                span: start.merge(end),
            });
        }
        if self.match_token(&TokenKind::LParen) {
            let prev_dis = self.disallow_struct_lit;
            self.disallow_struct_lit = false;
            let inner = self.parse_expr()?;
            self.disallow_struct_lit = prev_dis;
            self.expect(&TokenKind::RParen)?;
            return Ok(inner);
        }

        let tok = self.advance().cloned().ok_or_else(|| ParseError {
            message: "Unexpected EOF in expression".to_string(),
            span: start,
        })?;

        match &tok.kind {
            TokenKind::IntLit(v) | TokenKind::HexLit(v) => Ok(Expr::IntLit(*v, tok.span)),
            TokenKind::FloatLit(v) => Ok(Expr::FloatLit(*v, tok.span)),
            TokenKind::BoolLit(b) => Ok(Expr::BoolLit(*b, tok.span)),
            TokenKind::StringLit(s) => Ok(Expr::StringLit(s.clone(), tok.span)),
            TokenKind::KwOk | TokenKind::KwErr | TokenKind::KwSome => {
                let variant = tok.lexeme.clone();
                self.expect(&TokenKind::LParen)?;
                let payload = self.parse_expr()?;
                let end = self.expect(&TokenKind::RParen)?.span;
                Ok(Expr::EnumConstructor {
                    enum_name: None,
                    variant,
                    payload: Some(Box::new(payload)),
                    span: tok.span.merge(end),
                })
            }
            TokenKind::KwNone => Ok(Expr::EnumConstructor {
                enum_name: None,
                variant: "None".to_string(),
                payload: None,
                span: tok.span,
            }),
            TokenKind::TyVec2
            | TokenKind::TyVec3
            | TokenKind::TyVec4
            | TokenKind::TyQuat => {
                let ns = tok.lexeme.clone();
                if self.match_token(&TokenKind::ColonColon) {
                    let member = self.expect_ident()?;
                    if self.match_token(&TokenKind::LParen) {
                        let args = self.parse_arg_list()?;
                        let end = self.expect(&TokenKind::RParen)?.span;
                        Ok(Expr::StaticCall {
                            namespace: ns,
                            func: member,
                            args,
                            span: tok.span.merge(end),
                        })
                    } else if !self.disallow_struct_lit && self.looks_like_struct_init() {
                        self.advance();
                        let mut fields = Vec::new();
                        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
                            let f_name = self.expect_ident()?;
                            self.expect(&TokenKind::Colon)?;
                            let f_expr = self.parse_expr()?;
                            fields.push((f_name, f_expr));
                            let _ = self.match_token(&TokenKind::Comma);
                        }
                        let end = self.expect(&TokenKind::RBrace)?.span;
                        Ok(Expr::StructInit {
                            struct_name: member,
                            fields,
                            span: tok.span.merge(end),
                        })
                    } else {
                        Ok(Expr::EnumConstructor {
                            enum_name: Some(ns),
                            variant: member,
                            payload: None,
                            span: tok.span.merge(self.previous_span()),
                        })
                    }
                } else {
                    self.expect(&TokenKind::LParen)?;
                    let args = self.parse_arg_list()?;
                    let end = self.expect(&TokenKind::RParen)?.span;
                    Ok(Expr::StaticCall {
                        namespace: "self".to_string(),
                        func: ns,
                        args,
                        span: tok.span.merge(end),
                    })
                }
            }
            TokenKind::TyHandle => {
                self.expect(&TokenKind::ColonColon)?;
                let func = self.expect_ident()?;
                self.expect(&TokenKind::LParen)?;
                let args = self.parse_arg_list()?;
                let end = self.expect(&TokenKind::RParen)?.span;
                Ok(Expr::StaticCall {
                    namespace: "Handle".to_string(),
                    func,
                    args,
                    span: tok.span.merge(end),
                })
            }
            TokenKind::TyStr64 | TokenKind::TyStr128 | TokenKind::TyStr256 => {
                let ns = match tok.kind {
                    TokenKind::TyStr64 => "str64",
                    TokenKind::TyStr128 => "str128",
                    _ => "str256",
                };
                self.expect(&TokenKind::ColonColon)?;
                let func = self.expect_ident()?;
                self.expect(&TokenKind::LParen)?;
                let args = self.parse_arg_list()?;
                let end = self.expect(&TokenKind::RParen)?.span;
                Ok(Expr::StaticCall {
                    namespace: ns.to_string(),
                    func,
                    args,
                    span: tok.span.merge(end),
                })
            }
            TokenKind::TyMat3 | TokenKind::TyMat4 => {
                let ns = tok.lexeme.clone();
                self.expect(&TokenKind::ColonColon)?;
                let member = self.expect_ident()?;
                if self.match_token(&TokenKind::LParen) {
                    let args = self.parse_arg_list()?;
                    let end = self.expect(&TokenKind::RParen)?.span;
                    Ok(Expr::StaticCall {
                        namespace: ns,
                        func: member,
                        args,
                        span: tok.span.merge(end),
                    })
                } else if !self.disallow_struct_lit && self.looks_like_struct_init() {
                    self.advance();
                    let mut fields = Vec::new();
                    while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
                        let f_name = self.expect_ident()?;
                        self.expect(&TokenKind::Colon)?;
                        let f_expr = self.parse_expr()?;
                        fields.push((f_name, f_expr));
                        let _ = self.match_token(&TokenKind::Comma);
                    }
                    let end = self.expect(&TokenKind::RBrace)?.span;
                    Ok(Expr::StructInit {
                        struct_name: member,
                        fields,
                        span: tok.span.merge(end),
                    })
                } else {
                    Ok(Expr::EnumConstructor {
                        enum_name: Some(ns),
                        variant: member,
                        payload: None,
                        span: tok.span.merge(self.previous_span()),
                    })
                }
            }
            TokenKind::Ident(ident_str) => {
                let name = ident_str.clone();
                // Chamada ou Enum `Namespace::member`
                if self.match_token(&TokenKind::ColonColon) {
                    let member = self.expect_ident()?;
                    if self.match_token(&TokenKind::LParen) {
                        let args = self.parse_arg_list()?;
                        let end = self.expect(&TokenKind::RParen)?.span;
                        Ok(Expr::StaticCall {
                            namespace: name,
                            func: member,
                            args,
                            span: tok.span.merge(end),
                        })
                    } else if !self.disallow_struct_lit && self.looks_like_struct_init() {
                        self.advance();
                        let mut fields = Vec::new();
                        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
                            let f_name = self.expect_ident()?;
                            self.expect(&TokenKind::Colon)?;
                            let f_expr = self.parse_expr()?;
                            fields.push((f_name, f_expr));
                            let _ = self.match_token(&TokenKind::Comma);
                        }
                        let end = self.expect(&TokenKind::RBrace)?.span;
                        Ok(Expr::StructInit {
                            struct_name: member,
                            fields,
                            span: tok.span.merge(end),
                        })
                    } else {
                        Ok(Expr::EnumConstructor {
                            enum_name: Some(name),
                            variant: member,
                            payload: None,
                            span: tok.span.merge(self.previous_span()),
                        })
                    }
                } else if !self.disallow_struct_lit
                    && self.looks_like_struct_init()
                    && name
                        .chars()
                        .next()
                        .map(|c| c.is_uppercase())
                        .unwrap_or(false)
                {
                    // Inicialização de Struct `StructName { field: val, ... }`
                    self.advance(); // consume '{'
                    let mut fields = Vec::new();
                    while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
                        let f_name = self.expect_ident()?;
                        self.expect(&TokenKind::Colon)?;
                        let f_expr = self.parse_expr()?;
                        fields.push((f_name, f_expr));
                        let _ = self.match_token(&TokenKind::Comma);
                    }
                    let end = self.expect(&TokenKind::RBrace)?.span;
                    Ok(Expr::StructInit {
                        struct_name: name,
                        fields,
                        span: tok.span.merge(end),
                    })
                } else if self.match_token(&TokenKind::LParen) {
                    let args = self.parse_arg_list()?;
                    let end = self.expect(&TokenKind::RParen)?.span;
                    Ok(Expr::StaticCall {
                        namespace: "self".to_string(),
                        func: name,
                        args,
                        span: tok.span.merge(end),
                    })
                } else {
                    Ok(Expr::Ident(name, tok.span))
                }
            }
            _ => Err(ParseError {
                message: format!("Unexpected token '{}' in expression", tok.lexeme),
                span: tok.span,
            }),
        }
    }

    fn parse_arg_list(&mut self) -> Result<Vec<Expr>, ParseError> {
        let mut args = Vec::new();
        while !self.check(&TokenKind::RParen) && !self.is_at_end() {
            let _explicit_inout = self.match_token(&TokenKind::KwInout);
            args.push(self.parse_expr()?);
            let _ = self.match_token(&TokenKind::Comma);
        }
        Ok(args)
    }

    // Utilitários de navegação de Tokens
    fn is_at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn peek_kind(&self) -> Option<&TokenKind> {
        self.peek().map(|t| &t.kind)
    }

    fn peek_lexeme(&self) -> String {
        self.peek()
            .map(|t| t.lexeme.clone())
            .unwrap_or_else(|| "<EOF>".to_string())
    }

    fn current_span(&self) -> Span {
        self.peek()
            .map(|t| t.span)
            .or_else(|| self.tokens.last().map(|t| t.span))
            .unwrap_or_default()
    }

    fn previous_span(&self) -> Span {
        if self.pos > 0 {
            self.tokens[self.pos - 1].span
        } else {
            self.current_span()
        }
    }

    fn advance(&mut self) -> Option<&Token> {
        if !self.is_at_end() {
            self.pos += 1;
            Some(&self.tokens[self.pos - 1])
        } else {
            None
        }
    }

    fn check(&self, kind: &TokenKind) -> bool {
        match (self.peek_kind(), kind) {
            (Some(a), b) => std::mem::discriminant(a) == std::mem::discriminant(b),
            _ => false,
        }
    }

    fn match_token(&mut self, kind: &TokenKind) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn looks_like_struct_init(&self) -> bool {
        if !matches!(
            self.tokens.get(self.pos).map(|t| &t.kind),
            Some(TokenKind::LBrace)
        ) {
            return false;
        }
        match self.tokens.get(self.pos + 1).map(|t| &t.kind) {
            Some(TokenKind::RBrace) => true,
            Some(TokenKind::Ident(_)) => matches!(
                self.tokens.get(self.pos + 2).map(|t| &t.kind),
                Some(TokenKind::Colon)
            ),
            _ => false,
        }
    }

    fn expect(&mut self, kind: &TokenKind) -> Result<Token, ParseError> {
        if self.check(kind) {
            Ok(self.advance().unwrap().clone())
        } else {
            Err(ParseError {
                message: format!("Expected {:?}, found '{}'", kind, self.peek_lexeme()),
                span: self.current_span(),
            })
        }
    }

    fn expect_ident(&mut self) -> Result<String, ParseError> {
        match self.peek_kind() {
            Some(TokenKind::Ident(s)) => {
                let res = s.clone();
                self.advance();
                Ok(res)
            }
            _ => Err(ParseError {
                message: format!("Expected identifier, found '{}'", self.peek_lexeme()),
                span: self.current_span(),
            }),
        }
    }

    fn expect_path_segment(&mut self) -> Result<String, ParseError> {
        match self.peek_kind() {
            Some(TokenKind::Ident(s)) => {
                let res = s.clone();
                self.advance();
                Ok(res)
            }
            Some(
                TokenKind::TyVec2
                | TokenKind::TyVec3
                | TokenKind::TyVec4
                | TokenKind::TyQuat
                | TokenKind::TyMat3
                | TokenKind::TyMat4
                | TokenKind::TyStr64
                | TokenKind::TyStr128
                | TokenKind::TyStr256
                | TokenKind::TyBool
                | TokenKind::TyStr,
            ) => {
                let res = self.advance().unwrap().lexeme.clone();
                Ok(res)
            }
            _ => Err(ParseError {
                message: format!("Expected identifier, found '{}'", self.peek_lexeme()),
                span: self.current_span(),
            }),
        }
    }

    fn expect_string_lit(&mut self) -> Result<String, ParseError> {
        match self.peek_kind() {
            Some(TokenKind::StringLit(s)) => {
                let res = s.clone();
                self.advance();
                Ok(res)
            }
            _ => Err(ParseError {
                message: format!("Expected string literal, found '{}'", self.peek_lexeme()),
                span: self.current_span(),
            }),
        }
    }

    fn expect_bool_lit(&mut self) -> Result<bool, ParseError> {
        match self.peek_kind() {
            Some(TokenKind::BoolLit(b)) => {
                let res = *b;
                self.advance();
                Ok(res)
            }
            _ => Err(ParseError {
                message: format!("Expected boolean literal, found '{}'", self.peek_lexeme()),
                span: self.current_span(),
            }),
        }
    }
}

