use crate::ast::*;

/// Gerador da Ponte de Contrato TypeScript (`rtsc build --emit-ts-sdk` — v1.7.0-rts).
/// 1. Preserva inteiros de 64 bits (`u64`, `i64`, `usize`) como `string` decimal na serialização JSON
///    para impedir perda de precisão acima de 2^53 - 1 (IEEE-754 Double).
/// 2. Exporta `RtsResult<T, E>` como Discriminated Union sem `throw` para funções que retornam `Result<T, E>`.
pub struct TsSdkCodegen;

impl TsSdkCodegen {
    pub fn emit_typescript_sdk(module: &Module) -> String {
        let mut ts = String::new();

        ts.push_str("// ==========================================================================\n");
        ts.push_str("// Generated deterministically by rtsc 1.7.0-rts --emit-ts-sdk\n");
        ts.push_str(&format!("// Source Backend Module: {}\n", module.path.join(".")));
        ts.push_str("// Architecture Rule: Tier-1 RustTS Backend (RequestArena) -> Typed TS SDK\n");
        ts.push_str("// Safety Guarantee: u64/i64 mapped to string (IEEE-754 53-bit overflow proof)\n");
        ts.push_str("// ==========================================================================\n\n");

        ts.push_str("export type RtsResult<T, E> =\n  | { readonly ok: true; readonly value: T }\n  | { readonly ok: false; readonly error: E };\n\n");

        // 1. Exporta todas as Structs como interfaces TypeScript imutáveis (`readonly`)
        for item in &module.items {
            if let Item::Struct(s) = item {
                ts.push_str(&format!("export interface {} {{\n", s.name));
                for f in &s.fields {
                    ts.push_str(&format!(
                        "  readonly {}: {};\n",
                        f.name,
                        Self::map_ts_type(&f.ty)
                    ));
                }
                ts.push_str("}\n\n");
            }
        }

        // 2. Exporta todas as Enums como Discriminated Unions TypeScript
        for item in &module.items {
            if let Item::Enum(e) = item {
                let variants: Vec<String> = e
                    .variants
                    .iter()
                    .map(|v| match &v.payload {
                        Some(ty) => format!(
                            "{{ readonly tag: \"{}\"; readonly value: {} }}",
                            v.name,
                            Self::map_ts_type(ty)
                        ),
                        None => format!("{{ readonly tag: \"{}\" }}", v.name),
                    })
                    .collect();
                ts.push_str(&format!(
                    "export type {} =\n  | {};\n\n",
                    e.name,
                    variants.join("\n  | ")
                ));
            }
        }

        // 3. Exporta o Cliente HTTP Tipado para cada função anotada com `@route(method, path)`
        ts.push_str("export class RustTSClient {\n");
        ts.push_str("  constructor(private readonly baseUrl: string) {}\n\n");

        for item in &module.items {
            if let Item::Function(f) = item {
                let route_attr = f.attributes.iter().find_map(|a| match a {
                    Attribute::Route { method, path } => Some((method.clone(), path.clone())),
                    _ => None,
                });

                if let Some((method, path)) = route_attr {
                    let non_cap_params: Vec<&FnParam> =
                        f.params.iter().filter(|p| !p.is_cap).collect();
                    let ts_params: Vec<String> = non_cap_params
                        .iter()
                        .map(|p| format!("{}: {}", p.name, Self::map_ts_type(&p.ty)))
                        .collect();

                    let (ret_ts, is_result_contract) = match &f.return_type {
                        Some(TypeExpr::Result(ok_ty, err_ty)) => (
                            format!(
                                "RtsResult<{}, {}>",
                                Self::map_ts_type(ok_ty),
                                Self::map_ts_type(err_ty)
                            ),
                            Some(Self::map_ts_type(ok_ty)),
                        ),
                        Some(other) => (Self::map_ts_type(other), None),
                        None => ("void".to_string(), None),
                    };

                    ts.push_str(&format!(
                        "  /**\n   * Route: {} {}\n   * Backed by RustTS O(1) RequestArena\n   */\n",
                        method, path
                    ));
                    ts.push_str(&format!(
                        "  public async {}({}): Promise<{}> {{\n",
                        f.name,
                        ts_params.join(", "),
                        ret_ts
                    ));

                    let mut interpolated_path = path.clone();
                    for p in &non_cap_params {
                        let placeholder = format!(":{}", p.name);
                        if interpolated_path.contains(&placeholder) {
                            interpolated_path = interpolated_path.replace(
                                &placeholder,
                                &format!("${{encodeURIComponent(String({}))}}", p.name),
                            );
                        }
                    }

                    ts.push_str(&format!(
                        "    const res = await fetch(`${{this.baseUrl}}{}`, {{\n      method: \"{}\",\n      headers: {{ \"Accept\": \"application/json\" }},\n    }});\n",
                        interpolated_path, method
                    ));
                    if let Some(ok_ts) = is_result_contract {
                        ts.push_str("    if (!res.ok) {\n      return { ok: false, error: (await res.json()) };\n    }\n");
                        ts.push_str(&format!(
                            "    return {{ ok: true, value: (await res.json()) as {} }};\n",
                            ok_ts
                        ));
                    } else {
                        ts.push_str(&format!("    return (await res.json()) as {};\n", ret_ts));
                    }
                    ts.push_str("  }\n");
                }
            }
        }

        ts.push_str("}\n");
        ts
    }

    fn map_ts_type(ty: &TypeExpr) -> String {
        match ty {
            TypeExpr::Primitive(p) => match p {
                PrimitiveType::I8
                | PrimitiveType::I16
                | PrimitiveType::I32
                | PrimitiveType::U8
                | PrimitiveType::U16
                | PrimitiveType::U32
                | PrimitiveType::F32
                | PrimitiveType::F64
                | PrimitiveType::Fix32 => "number".to_string(),
                // Inteiros de 64 bits mapeados como string decimal para evitar perda de precisão (> 2^53 - 1) no JSON.parse
                PrimitiveType::I64 | PrimitiveType::U64 | PrimitiveType::Usize => {
                    "string".to_string()
                }
                PrimitiveType::Bool => "boolean".to_string(),
                PrimitiveType::Str => "string".to_string(),
            },
            TypeExpr::Option(inner) => format!("{} | null", Self::map_ts_type(inner)),
            TypeExpr::Named { name, generics } if name == "Json" && generics.len() == 1 => {
                Self::map_ts_type(&generics[0])
            }
            TypeExpr::Named { name, .. } => name.clone(),
            _ => "unknown".to_string(),
        }
    }
}
