use crate::ast::*;

/// Par de arquivos `.h` e `.cpp` gerados deterministicamente para o Unreal Build Tool (UBT) / Unreal Header Tool (UHT).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnrealCppOutput {
    pub header_filename: String,
    pub header_code: String,
    pub source_filename: String,
    pub source_code: String,
}

/// Transpilador para Unreal Engine 5 (`rtsc --target unreal` — v2.0.0-rts).
/// Converte um único arquivo `.rts` contendo `@unreal_actor` e `@unreal_impl` em `.h` e `.cpp`
/// com `UCLASS()`, `GENERATED_BODY()`, `UPROPERTY()`, `UFUNCTION()`, `GetLifetimeReplicatedProps`
/// e lowering determinístico completo da AST para C++20 da Unreal (`FMath`, `this->`, etc.).
pub struct UnrealCodegen;

impl UnrealCodegen {
    pub fn emit_unreal_actor(module: &Module) -> Vec<UnrealCppOutput> {
        let mut outputs = Vec::new();

        for item in &module.items {
            if let Item::Struct(s) = item {
                let actor_attr = s.attributes.iter().find_map(|a| match a {
                    Attribute::UnrealActor {
                        replicate,
                        category,
                    } => Some((*replicate, category.clone())),
                    _ => None,
                });

                if let Some((replicate, category)) = actor_attr {
                    let class_name = format!("A{}", s.name);
                    let header_filename = format!("{}.h", class_name);
                    let source_filename = format!("{}.cpp", class_name);

                    // Coleta todos os métodos `@unreal_impl(StructName)` associados a este Actor
                    let methods: Vec<&FnDecl> = module
                        .items
                        .iter()
                        .filter_map(|it| match it {
                            Item::Function(f) => {
                                let matches_target = f.attributes.iter().any(|attr| {
                                    matches!(
                                        attr,
                                        Attribute::UnrealImpl { target_struct } if target_struct == &s.name
                                    )
                                });
                                if matches_target {
                                    Some(f)
                                } else {
                                    None
                                }
                            }
                            _ => None,
                        })
                        .collect();

                    let mut h = String::new();
                    h.push_str("// Generated deterministically by rtsc 2.0.0-rts --target=unreal\n");
                    h.push_str("#pragma once\n\n");
                    h.push_str("#include \"CoreMinimal.h\"\n");
                    h.push_str("#include \"GameFramework/Actor.h\"\n");
                    h.push_str("#include \"Net/UnrealNetwork.h\"\n");
                    h.push_str(&format!("#include \"{}.generated.h\"\n\n", s.name));
                    h.push_str(&format!(
                        "UCLASS(BlueprintType, Blueprintable, Category=\"{}\")\n",
                        category
                    ));
                    h.push_str(&format!("class {} : public AActor\n{{\n", class_name));
                    h.push_str("    GENERATED_BODY()\n\npublic:\n");
                    h.push_str(&format!("    {}();\n\n", class_name));

                    let mut replicated_fields = Vec::new();

                    for field in &s.fields {
                        let uprop_flags = field.attributes.iter().find_map(|a| match a {
                            Attribute::UProperty { flags } => Some(flags.clone()),
                            _ => None,
                        });

                        if let Some(flags) = uprop_flags {
                            let mut ue_specifiers = Vec::new();
                            for fl in &flags {
                                match fl.as_str() {
                                    "edit_anywhere" => ue_specifiers.push("EditAnywhere"),
                                    "blueprint_read_only" => {
                                        ue_specifiers.push("BlueprintReadOnly")
                                    }
                                    "blueprint_read_write" => {
                                        ue_specifiers.push("BlueprintReadWrite")
                                    }
                                    "replicated" => {
                                        ue_specifiers.push("Replicated");
                                        replicated_fields.push(field.name.clone());
                                    }
                                    _ => {}
                                }
                            }
                            h.push_str(&format!(
                                "    UPROPERTY({}, Category=\"{}\")\n",
                                ue_specifiers.join(", "),
                                category
                            ));
                        }

                        h.push_str(&format!(
                            "    {} {};\n\n",
                            Self::map_unreal_type(&field.ty),
                            field.name
                        ));
                    }

                    if replicate || !replicated_fields.is_empty() {
                        h.push_str("    virtual void GetLifetimeReplicatedProps(TArray<FLifetimeProperty>& OutLifetimeProps) const override;\n\n");
                    }

                    for m in &methods {
                        let ret_ty = m
                            .return_type
                            .as_ref()
                            .map(Self::map_unreal_type)
                            .unwrap_or_else(|| "void".to_string());
                        let is_const_method = Self::is_readonly_self_method(m);
                        let const_qual = if is_const_method { " const" } else { "" };
                        let params_cpp: Vec<String> = m
                            .params
                            .iter()
                            .filter(|p| p.name != "self")
                            .map(|p| format!("{} {}", Self::map_unreal_type(&p.ty), p.name))
                            .collect();
                        h.push_str(&format!(
                            "    UFUNCTION(BlueprintCallable, Category=\"{}\")\n",
                            category
                        ));
                        h.push_str(&format!(
                            "    {} {}({}){};\n\n",
                            ret_ty,
                            m.name,
                            params_cpp.join(", "),
                            const_qual
                        ));
                    }

                    h.push_str("};\n");

                    // Geração do `.cpp` com lowering real da AST
                    let mut cpp = String::new();
                    cpp.push_str("// Generated deterministically by rtsc 2.0.0-rts --target=unreal\n");
                    cpp.push_str(&format!("#include \"{}\"\n", header_filename));
                    cpp.push_str("#include \"Math/UnrealMathUtility.h\"\n\n");
                    cpp.push_str(&format!("{}::{}()\n{{\n", class_name, class_name));
                    cpp.push_str("    PrimaryActorTick.bCanEverTick = false;\n");
                    if replicate {
                        cpp.push_str("    bReplicates = true;\n");
                    }
                    cpp.push_str("}\n\n");

                    if replicate || !replicated_fields.is_empty() {
                        cpp.push_str(&format!(
                            "void {}::GetLifetimeReplicatedProps(TArray<FLifetimeProperty>& OutLifetimeProps) const\n{{\n",
                            class_name
                        ));
                        cpp.push_str("    Super::GetLifetimeReplicatedProps(OutLifetimeProps);\n");
                        for rf in &replicated_fields {
                            cpp.push_str(&format!(
                                "    DOREPLIFETIME({}, {});\n",
                                class_name, rf
                            ));
                        }
                        cpp.push_str("}\n\n");
                    }

                    for m in &methods {
                        let ret_ty = m
                            .return_type
                            .as_ref()
                            .map(Self::map_unreal_type)
                            .unwrap_or_else(|| "void".to_string());
                        let is_const_method = Self::is_readonly_self_method(m);
                        let const_qual = if is_const_method { " const" } else { "" };
                        let params_cpp: Vec<String> = m
                            .params
                            .iter()
                            .filter(|p| p.name != "self")
                            .map(|p| format!("{} {}", Self::map_unreal_type(&p.ty), p.name))
                            .collect();
                        cpp.push_str(&format!(
                            "{} {}::{}({}){}\n{{\n",
                            ret_ty,
                            class_name,
                            m.name,
                            params_cpp.join(", "),
                            const_qual
                        ));
                        cpp.push_str("    // Capability-checked deterministic body lowered from RustTS AST\n");
                        for stmt in &m.body {
                            Self::lower_unreal_stmt(&mut cpp, stmt, 1);
                        }
                        cpp.push_str("}\n");
                    }

                    outputs.push(UnrealCppOutput {
                        header_filename,
                        header_code: h,
                        source_filename,
                        source_code: cpp,
                    });
                }
            }
        }

        outputs
    }

    fn is_readonly_self_method(m: &FnDecl) -> bool {
        for p in &m.params {
            if p.name == "self" {
                if let Some(cap) = &p.capability {
                    if cap.kind == CapabilityKind::MutResource {
                        return false;
                    }
                }
            }
        }
        true
    }

    fn lower_unreal_stmt(out: &mut String, stmt: &Stmt, indent: usize) {
        let pad = "    ".repeat(indent);
        match stmt {
            Stmt::Let {
                name,
                is_mut,
                ty,
                value,
                ..
            } => {
                let const_kw = if *is_mut { "" } else { "const " };
                let cpp_ty = ty
                    .as_ref()
                    .map(Self::map_unreal_type)
                    .unwrap_or_else(|| "auto".to_string());
                let val_cpp = Self::lower_unreal_expr(value);
                out.push_str(&format!("{}{}{} {} = {};\n", pad, const_kw, cpp_ty, name, val_cpp));
            }
            Stmt::Assign { target, value, .. } => {
                let lhs = Self::lower_unreal_expr(target);
                let rhs = Self::lower_unreal_expr(value);
                out.push_str(&format!("{}{} = {};\n", pad, lhs, rhs));
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                let cond = Self::lower_unreal_expr(condition);
                out.push_str(&format!("{}if ({})\n{}{{\n", pad, cond, pad));
                for s in then_branch {
                    Self::lower_unreal_stmt(out, s, indent + 1);
                }
                if let Some(eb) = else_branch {
                    out.push_str(&format!("{}}}\n{}else\n{}{{\n", pad, pad, pad));
                    for s in eb {
                        Self::lower_unreal_stmt(out, s, indent + 1);
                    }
                }
                out.push_str(&format!("{}}}\n", pad));
            }
            Stmt::Match { expr, arms, .. } => {
                let scrutinee = Self::lower_unreal_expr(expr);
                out.push_str(&format!("{}switch ({})\n{}{{\n", pad, scrutinee, pad));
                for arm in arms {
                    match &arm.pattern {
                        MatchPattern::Variant {
                            enum_name, variant, ..
                        } => {
                            let case_lbl = match enum_name {
                                Some(en) => format!("{}::{}", en, variant),
                                None => variant.clone(),
                            };
                            out.push_str(&format!("{}case {}:\n{}{{\n", pad, case_lbl, pad));
                            for s in &arm.body {
                                Self::lower_unreal_stmt(out, s, indent + 1);
                            }
                            out.push_str(&format!("{}    break;\n{}}}\n", pad, pad));
                        }
                        MatchPattern::Wildcard => {
                            out.push_str(&format!("{}default:\n{}{{\n", pad, pad));
                            for s in &arm.body {
                                Self::lower_unreal_stmt(out, s, indent + 1);
                            }
                            out.push_str(&format!("{}    break;\n{}}}\n", pad, pad));
                        }
                    }
                }
                out.push_str(&format!("{}}}\n", pad));
            }
            Stmt::While {
                condition, body, ..
            } => {
                let cond = Self::lower_unreal_expr(condition);
                out.push_str(&format!("{}while ({})\n{}{{\n", pad, cond, pad));
                for s in body {
                    Self::lower_unreal_stmt(out, s, indent + 1);
                }
                out.push_str(&format!("{}}}\n", pad));
            }
            Stmt::Break { .. } => out.push_str(&format!("{}break;\n", pad)),
            Stmt::Continue { .. } => out.push_str(&format!("{}continue;\n", pad)),
            Stmt::ForRange {
                iterator,
                start,
                end,
                body,
                ..
            } => {
                let s_cpp = Self::lower_unreal_expr(start);
                let e_cpp = Self::lower_unreal_expr(end);
                out.push_str(&format!(
                    "{}for (int32 {} = {}; {} < {}; ++{})\n{}{{\n",
                    pad, iterator, s_cpp, iterator, e_cpp, iterator, pad
                ));
                for s in body {
                    Self::lower_unreal_stmt(out, s, indent + 1);
                }
                out.push_str(&format!("{}}}\n", pad));
            }
            Stmt::Return { value, .. } => match value {
                Some(v) => {
                    out.push_str(&format!("{}return {};\n", pad, Self::lower_unreal_expr(v)));
                }
                None => {
                    out.push_str(&format!("{}return;\n", pad));
                }
            },
            Stmt::ExprStmt { expr, .. } => {
                out.push_str(&format!("{}{};\n", pad, Self::lower_unreal_expr(expr)));
            }
        }
    }

    fn lower_unreal_expr(expr: &Expr) -> String {
        match expr {
            Expr::IntLit(v, _) => format!("{}", v),
            Expr::FloatLit(v, _) => {
                let s = format!("{}", v);
                if s.contains('.') {
                    format!("{}f", s)
                } else {
                    format!("{}.0f", s)
                }
            }
            Expr::BoolLit(b, _) => b.to_string(),
            Expr::StringLit(s, _) => format!("TEXT(\"{}\")", s),
            Expr::Ident(name, _) => {
                if name == "self" {
                    "this".to_string()
                } else {
                    name.clone()
                }
            }
            Expr::Unary { op, expr: inner, .. } => {
                let op_str = match op {
                    UnaryOp::Neg => "-",
                    UnaryOp::Not => "!",
                };
                format!("({}{})", op_str, Self::lower_unreal_expr(inner))
            }
            Expr::FieldAccess {
                receiver, field, ..
            } => {
                if let Expr::Ident(rname, _) = receiver.as_ref() {
                    if rname == "self" {
                        return format!("this->{}", field);
                    }
                }
                format!("{}.{}", Self::lower_unreal_expr(receiver), field)
            }
            Expr::StaticCall {
                namespace,
                func,
                args,
                ..
            } => {
                let args_cpp: Vec<String> = args.iter().map(Self::lower_unreal_expr).collect();
                if namespace == "math" {
                    let ue_fn = match func.as_str() {
                        "clamp" => "FMath::Clamp",
                        "min" => "FMath::Min",
                        "max" => "FMath::Max",
                        "abs" => "FMath::Abs",
                        "sqrt" => "FMath::Sqrt",
                        "lerp" => "FMath::Lerp",
                        other => other,
                    };
                    format!("{}({})", ue_fn, args_cpp.join(", "))
                } else {
                    format!("{}::{}({})", namespace, func, args_cpp.join(", "))
                }
            }
            Expr::MethodCall {
                receiver,
                method,
                args,
                ..
            } => {
                let rec = Self::lower_unreal_expr(receiver);
                let args_cpp: Vec<String> = args.iter().map(Self::lower_unreal_expr).collect();
                if rec == "this" {
                    format!("this->{}({})", method, args_cpp.join(", "))
                } else {
                    format!("{}.{}({})", rec, method, args_cpp.join(", "))
                }
            }
            Expr::Binary {
                left, op, right, ..
            } => {
                let l = Self::lower_unreal_expr(left);
                let r = Self::lower_unreal_expr(right);
                let op_str = match op {
                    BinaryOp::Add => "+",
                    BinaryOp::Sub => "-",
                    BinaryOp::Mul => "*",
                    BinaryOp::Div => "/",
                    BinaryOp::Eq => "==",
                    BinaryOp::NotEq => "!=",
                    BinaryOp::Lt => "<",
                    BinaryOp::LtEq => "<=",
                    BinaryOp::Gt => ">",
                    BinaryOp::GtEq => ">=",
                    BinaryOp::And => "&&",
                    BinaryOp::Or => "||",
                };
                format!("({} {} {})", l, op_str, r)
            }
            Expr::Cast {
                expr: inner,
                target_type,
                ..
            } => {
                format!(
                    "static_cast<{}>({})",
                    Self::map_unreal_type(target_type),
                    Self::lower_unreal_expr(inner)
                )
            }
            Expr::Index {
                receiver, index, ..
            } => {
                format!(
                    "{}[{}]",
                    Self::lower_unreal_expr(receiver),
                    Self::lower_unreal_expr(index)
                )
            }
            Expr::StructInit {
                struct_name,
                fields,
                ..
            } => {
                let vals: Vec<String> = fields
                    .iter()
                    .map(|(_, v)| Self::lower_unreal_expr(v))
                    .collect();
                format!("{}({})", struct_name, vals.join(", "))
            }
            Expr::EnumConstructor {
                enum_name, variant, ..
            } => match enum_name {
                Some(en) => format!("{}::{}", en, variant),
                None => variant.clone(),
            },
            Expr::TryOp { expr: inner, .. } | Expr::ErrorMapClosure { body: inner, .. } => {
                Self::lower_unreal_expr(inner)
            }
            Expr::ArrayLit { elements, .. } => {
                let elems: Vec<String> = elements.iter().map(Self::lower_unreal_expr).collect();
                format!("{{ {} }}", elems.join(", "))
            }
        }
    }

    fn map_unreal_type(ty: &TypeExpr) -> String {
        match ty {
            TypeExpr::Primitive(p) => match p {
                PrimitiveType::F32 => "float".to_string(),
                PrimitiveType::F64 => "double".to_string(),
                PrimitiveType::I32 => "int32".to_string(),
                PrimitiveType::I64 => "int64".to_string(),
                PrimitiveType::U8 => "uint8".to_string(),
                PrimitiveType::U16 => "uint16".to_string(),
                PrimitiveType::U32 => "uint32".to_string(),
                PrimitiveType::U64 => "uint64".to_string(),
                PrimitiveType::Bool => "bool".to_string(),
                PrimitiveType::Str => "FString".to_string(),
                _ => "int32".to_string(),
            },
            TypeExpr::Math(m) => match m {
                MathType::Vec2 => "FVector2D".to_string(),
                MathType::Vec3 => "FVector".to_string(),
                MathType::Vec4 => "FVector4".to_string(),
                MathType::Quat => "FQuat".to_string(),
                MathType::Mat4 => "FMatrix".to_string(),
                _ => "FVector".to_string(),
            },
            TypeExpr::Named { name, .. } => name.clone(),
            _ => "void*".to_string(),
        }
    }
}
