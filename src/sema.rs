use crate::ast::*;
use crate::diagnostics::{
    CompilerDiagnosticsReport, DeterministicFix, DiagnosticCategory, DiagnosticEntry,
};
use crate::lexer::Span;
use std::collections::{HashMap, HashSet};

/// Tabela de Capacidades Ativas, Tipos Locais e Escopos de Arena no contexto de uma função `.rts`
#[derive(Debug, Clone, Default)]
pub struct FunctionCapabilityEnv {
    /// Mapeia o nome do parâmetro (ex: `i2c`, `db`, `transforms`, `frame_arena`) para sua `CapabilitySpec`
    pub param_caps: HashMap<String, CapabilitySpec>,
    /// Lista ordenada de capacidades concedidas na assinatura da função
    pub granted_specs: Vec<(String, CapabilitySpec)>,
    /// Rastreia o Rank de Arena (`ArenaScope`) de variáveis locais (ex: `let h = frame_arena.alloc(...)` -> Rank 1)
    pub local_var_arena_scope: HashMap<String, (ArenaScope, String)>,
    /// Rastreia o tipo estático (`TypeExpr`) de parâmetros e variáveis locais (`v1.0.0-rts` TypeChecker)
    pub local_var_types: HashMap<String, TypeExpr>,
    /// Variáveis locais declaradas sem `mut` (`let x = ...`), proibidas de sofrerem reatribuição
    pub immutable_locals: HashSet<String>,
    /// Ligações mutáveis elegíveis para passagem como `inout` (`let mut x` ou parâmetro `inout`)
    pub mutable_bindings: HashSet<String>,
}

impl FunctionCapabilityEnv {
    pub fn from_fn_decl(func: &FnDecl) -> Self {
        let mut param_caps = HashMap::new();
        let mut granted_specs = Vec::new();
        let mut local_var_arena_scope = HashMap::new();
        let mut local_var_types = HashMap::new();
        let immutable_locals = HashSet::new();
        let mut mutable_bindings = HashSet::new();

        for param in &func.params {
            local_var_types.insert(param.name.clone(), param.ty.clone());
            if param.is_inout {
                mutable_bindings.insert(param.name.clone());
            }
            if param.is_cap {
                let spec_opt = param.capability.clone().or_else(|| match &param.ty {
                    TypeExpr::Capability(s) => Some(s.clone()),
                    _ => None,
                });
                if let Some(spec) = spec_opt {
                    if let Some(scope) = Self::capability_arena_scope(&spec) {
                        local_var_arena_scope
                            .insert(param.name.clone(), (scope, spec.to_string()));
                    }
                    param_caps.insert(param.name.clone(), spec.clone());
                    granted_specs.push((param.name.clone(), spec));
                }
            }
        }

        Self {
            param_caps,
            granted_specs,
            local_var_arena_scope,
            local_var_types,
            immutable_locals,
            mutable_bindings,
        }
    }

    /// Determina o `ArenaScope` (Rank 1, 2 ou 3) associado a uma capacidade de Arena ou Coluna ECS.
    /// - `StaticArena` -> Rank 3
    /// - `LevelArena`, `SessionArena`, `MutColumn<T>`, `ReadColumn<T>`, `Mut<T>` -> Rank 2 (Persistente entre quadros)
    /// - `FrameArena<T>`, `RequestArena`, `TaskArena` -> Rank 1 (Efêmera: zerada em O(1) a cada 16.6ms / fim de request)
    pub fn capability_arena_scope(spec: &CapabilitySpec) -> Option<ArenaScope> {
        match spec.kind {
            CapabilityKind::ScopedArena(scope) | CapabilityKind::ReadScopedArena(scope) => {
                Some(scope)
            }
            CapabilityKind::MutColumn
            | CapabilityKind::ReadColumn
            | CapabilityKind::MutResource
            | CapabilityKind::ReadResource => Some(ArenaScope::LevelOrSession),
            _ => None,
        }
    }

    pub fn satisfies_capability(&self, required: &CapabilitySpec) -> bool {
        self.granted_specs.iter().any(|(_, granted)| {
            if granted.domain_name == required.domain_name {
                match (&granted.generic_arg, &required.generic_arg) {
                    (Some(g), Some(r)) => g == r,
                    (Some(_), None) | (None, None) => true,
                    (None, Some(_)) => false,
                }
            } else if matches!(
                (&granted.kind, &required.kind),
                (
                    CapabilityKind::MutSlice,
                    CapabilityKind::ReadSlice
                ) | (
                    CapabilityKind::MutColumn,
                    CapabilityKind::ReadColumn
                )
            ) {
                granted.generic_arg == required.generic_arg
            } else if matches!(
                (&granted.kind, &required.kind),
                (
                    CapabilityKind::ScopedArena(s1),
                    CapabilityKind::ReadScopedArena(s2)
                ) if s1 == s2
            ) {
                match (&granted.generic_arg, &required.generic_arg) {
                    (Some(g), Some(r)) => g == r,
                    (Some(_), None) | (None, None) => true,
                    (None, Some(_)) => false,
                }
            } else {
                granted.kind == required.kind
                    && granted.kind != CapabilityKind::Custom
                    && required.generic_arg.is_none()
            }
        })
    }
}

/// Assinatura simbólica de uma função (intrínseca do `core.rts` ou definida pelo usuário no módulo)
#[derive(Debug, Clone)]
pub struct FunctionSymbol {
    pub qualified_name: String,
    /// Conjunto de capacidades exigidas diretamente ou transitivamente ($\text{Caps}(f)$)
    pub required_caps: Vec<CapabilitySpec>,
    /// Caminho da cadeia de chamadas para diagnóstico de violação transitiva (`RTS-E0403`)
    pub cap_provenance: HashMap<String, String>,
}

/// Tabela de Símbolos Global do Módulo + Prelude Intrínseco (`core.rts`)
pub struct SymbolTable {
    pub functions: HashMap<String, FunctionSymbol>,
    pub structs: HashMap<String, HashMap<String, TypeExpr>>,
    pub struct_decls: HashMap<String, StructDecl>,
    pub enums: HashMap<String, Vec<(String, Option<TypeExpr>)>>,
    pub consts: HashMap<String, TypeExpr>,
    pub fn_return_types: HashMap<String, Option<TypeExpr>>,
    pub fn_param_types: HashMap<String, Vec<TypeExpr>>,
    pub fn_params: HashMap<String, Vec<FnParam>>,
    pub extern_fns: HashSet<String>,
    pub call_edges: HashMap<String, Vec<String>>,
}

impl SymbolTable {
    /// Carrega as declarações intrínsecas de `core.rts` (eliminando mocks ad-hoc) e indexa o módulo atual.
    pub fn build_for_module(module: &Module) -> Self {
        let mut functions = HashMap::new();
        let mut structs: HashMap<String, HashMap<String, TypeExpr>> = HashMap::new();
        let mut enums: HashMap<String, Vec<(String, Option<TypeExpr>)>> = HashMap::new();
        let mut consts: HashMap<String, TypeExpr> = HashMap::new();
        let mut fn_return_types: HashMap<String, Option<TypeExpr>> = HashMap::new();
        let mut fn_param_types: HashMap<String, Vec<TypeExpr>> = HashMap::new();
        let mut fn_params: HashMap<String, Vec<FnParam>> = HashMap::new();

        // Intrínsecos matemáticos determinísticos do Prelude (`core.math` -> `<math.h>`)
        let f32_ty = TypeExpr::Primitive(PrimitiveType::F32);
        let u32_ty = TypeExpr::Primitive(PrimitiveType::U32);
        for unary_math in [
            "abs", "sqrt", "sin", "cos", "tan", "asin", "acos", "atan", "exp", "log", "floor",
            "ceil",
        ] {
            let key = format!("math::{}", unary_math);
            fn_return_types.insert(key.clone(), Some(f32_ty.clone()));
            fn_param_types.insert(key, vec![f32_ty.clone()]);
        }
        for binary_math in ["min", "max", "atan2", "pow"] {
            let key = format!("math::{}", binary_math);
            fn_return_types.insert(key.clone(), Some(f32_ty.clone()));
            fn_param_types.insert(key, vec![f32_ty.clone(), f32_ty.clone()]);
        }
        for ternary_math in ["clamp", "lerp"] {
            let key = format!("math::{}", ternary_math);
            fn_return_types.insert(key.clone(), Some(f32_ty.clone()));
            fn_param_types.insert(key, vec![f32_ty.clone(), f32_ty.clone(), f32_ty.clone()]);
        }
        fn_return_types.insert("math::f32_from_bits".to_string(), Some(f32_ty.clone()));
        fn_param_types.insert("math::f32_from_bits".to_string(), vec![u32_ty.clone()]);
        fn_return_types.insert("math::f32_to_bits".to_string(), Some(u32_ty.clone()));
        fn_param_types.insert("math::f32_to_bits".to_string(), vec![f32_ty.clone()]);

        // Intrínsecos de I/O determinístico para CLI e telemetria (`io::println`, `io::print_f32`, `io::print_i32`, `io::print_u32`)
        fn_return_types.insert("io::println".to_string(), None);
        fn_param_types.insert(
            "io::println".to_string(),
            vec![TypeExpr::Primitive(PrimitiveType::Str)],
        );
        fn_return_types.insert("io::print_f32".to_string(), None);
        fn_param_types.insert(
            "io::print_f32".to_string(),
            vec![
                TypeExpr::Primitive(PrimitiveType::Str),
                TypeExpr::Primitive(PrimitiveType::F32),
            ],
        );
        fn_return_types.insert("io::print_i32".to_string(), None);
        fn_param_types.insert(
            "io::print_i32".to_string(),
            vec![
                TypeExpr::Primitive(PrimitiveType::Str),
                TypeExpr::Primitive(PrimitiveType::I32),
            ],
        );
        fn_return_types.insert("io::print_u32".to_string(), None);
        fn_param_types.insert(
            "io::print_u32".to_string(),
            vec![
                TypeExpr::Primitive(PrimitiveType::Str),
                TypeExpr::Primitive(PrimitiveType::U32),
            ],
        );

        for (ns, pty) in [
            ("str64", PrimitiveType::Str64),
            ("str128", PrimitiveType::Str128),
            ("str256", PrimitiveType::Str256),
        ] {
            let ret = Some(TypeExpr::Primitive(pty));
            let new_k = format!("{}::new", ns);
            fn_return_types.insert(new_k.clone(), ret.clone());
            fn_param_types.insert(new_k, vec![]);

            let from_k = format!("{}::from", ns);
            fn_return_types.insert(from_k.clone(), ret);
            fn_param_types.insert(from_k, vec![TypeExpr::Primitive(PrimitiveType::Str)]);
        }

        // 1. Prelude Intrínseco (`core.rts`): Assinaturas formais dos subsistemas de hardware/SO/rede
        let intrinsics: Vec<(&str, &str, Option<CapGenericArg>)> = vec![
            ("NetworkSocket::send", "NetSocket", None),
            ("TcpSocket::connect", "TcpSocket", None),
            ("UdpSocket::send_to", "UdpTick", None),
            ("Http::get", "HttpRoute", None),
            (
                "I2CBus::read_register_u16",
                "I2CBus",
                Some(CapGenericArg::HardwareAddress(0x48)),
            ),
            ("SPIBus::transfer", "SPIBus", None),
            (
                "GPIOPin::set_high",
                "GPIOPin",
                Some(CapGenericArg::HardwareAddress(0)),
            ),
            (
                "PWMPin::set_duty",
                "PWMPin",
                Some(CapGenericArg::HardwareAddress(4)),
            ),
            ("SystemClock::now", "MonotonicClock", None),
            ("Gpu::submit", "GpuQueue", None),
        ];

        for (qualified, domain, arg) in intrinsics {
            let cap = CapabilitySpec::from_name_and_arg(domain, arg);
            let mut prov = HashMap::new();
            prov.insert(cap.to_string(), qualified.to_string());
            functions.insert(
                qualified.to_string(),
                FunctionSymbol {
                    qualified_name: qualified.to_string(),
                    required_caps: vec![cap],
                    cap_provenance: prov,
                },
            );
        }

        // 2. Registra todas as structs, enums e funções declaradas no módulo e suas capacidades explícitas
        let mut call_edges: HashMap<String, Vec<String>> = HashMap::new();
        let mut extern_fns: HashSet<String> = HashSet::new();
        let mut struct_decls: HashMap<String, StructDecl> = HashMap::new();

        for item in &module.items {
            match item {
                Item::Const(c) => {
                    consts.insert(c.name.clone(), c.ty.clone());
                }
                Item::Struct(s) => {
                    let mut field_map = HashMap::new();
                    for f in &s.fields {
                        field_map.insert(f.name.clone(), f.ty.clone());
                    }
                    structs.insert(s.name.clone(), field_map);
                    struct_decls.insert(s.name.clone(), s.clone());
                }
                Item::Enum(e) => {
                    let variants = e
                        .variants
                        .iter()
                        .map(|v| (v.name.clone(), v.payload.clone()))
                        .collect();
                    enums.insert(e.name.clone(), variants);
                }
                Item::Function(f) => {
                    fn_return_types.insert(f.name.clone(), f.return_type.clone());
                    fn_param_types.insert(
                        f.name.clone(),
                        f.params.iter().map(|p| p.ty.clone()).collect(),
                    );
                    fn_params.insert(f.name.clone(), f.params.clone());
                    let mut req_caps = Vec::new();
                    let mut prov = HashMap::new();
                    for p in &f.params {
                        if p.is_cap {
                            if let Some(c) = &p.capability {
                                req_caps.push(c.clone());
                                prov.insert(c.to_string(), f.name.clone());
                            }
                        }
                    }

                    // Coleta também chamadas diretas a intrínsecos no corpo de `f` para que funções auxiliares
                    // (mesmo que tenham omitido `cap` em suas próprias assinaturas) propaguem a exigência no Call Graph!
                    let mut callees = Vec::new();
                    Self::collect_callees_in_stmts(&f.body, &mut callees);

                    for callee in &callees {
                        if let Some(intrinsic_sym) = functions.get(callee) {
                            for icap in &intrinsic_sym.required_caps {
                                if !req_caps.contains(icap) {
                                    req_caps.push(icap.clone());
                                    prov.insert(
                                        icap.to_string(),
                                        format!("{} -> {}", f.name, intrinsic_sym.qualified_name),
                                    );
                                }
                            }
                        }
                    }

                    call_edges.insert(f.name.clone(), callees);
                    functions.insert(
                        f.name.clone(),
                        FunctionSymbol {
                            qualified_name: f.name.clone(),
                            required_caps: req_caps,
                            cap_provenance: prov,
                        },
                    );
                }
                Item::ExternFn(ext) => {
                    extern_fns.insert(ext.name.clone());
                    fn_return_types.insert(ext.name.clone(), ext.return_type.clone());
                    fn_param_types.insert(
                        ext.name.clone(),
                        ext.params.iter().map(|p| p.ty.clone()).collect(),
                    );
                    fn_params.insert(ext.name.clone(), ext.params.clone());
                    let mut req_caps = Vec::new();
                    let mut prov = HashMap::new();
                    for p in &ext.params {
                        if p.is_cap {
                            if let Some(c) = &p.capability {
                                req_caps.push(c.clone());
                                prov.insert(c.to_string(), format!("extern fn {}", ext.name));
                            }
                        }
                    }
                    functions.insert(
                        ext.name.clone(),
                        FunctionSymbol {
                            qualified_name: ext.name.clone(),
                            required_caps: req_caps,
                            cap_provenance: prov,
                        },
                    );
                }
            }
        }

        // 3. Fecho Transitivo no Grafo de Chamadas (Ponto Fixo): Caps(Caller) >= Caps(Callee)
        let fn_names: Vec<String> = call_edges.keys().cloned().collect();
        let mut changed = true;
        let mut iterations = 0;
        while changed && iterations < 16 {
            changed = false;
            iterations += 1;
            for caller in &fn_names {
                if let Some(callees) = call_edges.get(caller) {
                    let mut inherited: Vec<(CapabilitySpec, String)> = Vec::new();
                    for callee in callees {
                        let normalized = callee.strip_prefix("self::").unwrap_or(callee);
                        let resolved_sym = functions.get(normalized).or_else(|| {
                            normalized.split_once("::").and_then(|(ns, short)| {
                                if !matches!(
                                    ns,
                                    "io" | "math"
                                        | "core"
                                        | "time"
                                        | "physics"
                                        | "net"
                                        | "fs"
                                        | "I2CBus"
                                        | "PWMPin"
                                        | "MonotonicClock"
                                        | "NetSocket"
                                        | "FileSystem"
                                        | "Handle"
                                ) {
                                    functions.get(short)
                                } else {
                                    None
                                }
                            })
                        });
                        if let Some(callee_sym) = resolved_sym {
                            for c in &callee_sym.required_caps {
                                let chain = callee_sym
                                    .cap_provenance
                                    .get(&c.to_string())
                                    .cloned()
                                    .unwrap_or_else(|| callee_sym.qualified_name.clone());
                                inherited.push((c.clone(), format!("{} -> {}", caller, chain)));
                            }
                        }
                    }
                    if let Some(caller_sym) = functions.get_mut(caller) {
                        for (cap, chain) in inherited {
                            if !caller_sym.required_caps.contains(&cap) {
                                caller_sym.required_caps.push(cap.clone());
                                caller_sym.cap_provenance.insert(cap.to_string(), chain);
                                changed = true;
                            }
                        }
                    }
                }
            }
        }

        Self {
            functions,
            structs,
            struct_decls,
            enums,
            consts,
            fn_return_types,
            fn_param_types,
            fn_params,
            extern_fns,
            call_edges,
        }
    }

    fn collect_callees_in_stmts(stmts: &[Stmt], out: &mut Vec<String>) {
        for stmt in stmts {
            match stmt {
                Stmt::Let { value, .. } | Stmt::ExprStmt { expr: value, .. } => {
                    Self::collect_callees_in_expr(value, out);
                }
                Stmt::Assign { target, value, .. } => {
                    Self::collect_callees_in_expr(target, out);
                    Self::collect_callees_in_expr(value, out);
                }
                Stmt::If {
                    condition,
                    then_branch,
                    else_branch,
                    ..
                } => {
                    Self::collect_callees_in_expr(condition, out);
                    Self::collect_callees_in_stmts(then_branch, out);
                    if let Some(eb) = else_branch {
                        Self::collect_callees_in_stmts(eb, out);
                    }
                }
                Stmt::Match { expr, arms, .. } => {
                    Self::collect_callees_in_expr(expr, out);
                    for arm in arms {
                        Self::collect_callees_in_stmts(&arm.body, out);
                    }
                }
                Stmt::ForRange {
                    start, end, body, ..
                } => {
                    Self::collect_callees_in_expr(start, out);
                    Self::collect_callees_in_expr(end, out);
                    Self::collect_callees_in_stmts(body, out);
                }
                Stmt::While {
                    condition, body, ..
                } => {
                    Self::collect_callees_in_expr(condition, out);
                    Self::collect_callees_in_stmts(body, out);
                }
                Stmt::Return { value: Some(v), .. } => {
                    Self::collect_callees_in_expr(v, out);
                }
                Stmt::Return { value: None, .. }
                | Stmt::Break { .. }
                | Stmt::Continue { .. } => {}
            }
        }
    }

    fn collect_callees_in_expr(expr: &Expr, out: &mut Vec<String>) {
        match expr {
            Expr::StaticCall {
                namespace,
                func,
                args,
                ..
            } => {
                if namespace == "self" {
                    out.push(func.clone());
                } else {
                    out.push(format!("{}::{}", namespace, func));
                }
                for a in args {
                    Self::collect_callees_in_expr(a, out);
                }
            }
            Expr::MethodCall {
                receiver,
                method,
                args,
                ..
            } => {
                out.push(method.clone());
                Self::collect_callees_in_expr(receiver, out);
                for a in args {
                    Self::collect_callees_in_expr(a, out);
                }
            }
            Expr::FieldAccess { receiver, .. } => Self::collect_callees_in_expr(receiver, out),
            Expr::Index {
                receiver, index, ..
            } => {
                Self::collect_callees_in_expr(receiver, out);
                Self::collect_callees_in_expr(index, out);
            }
            Expr::StructInit { fields, .. } => {
                for (_, fe) in fields {
                    Self::collect_callees_in_expr(fe, out);
                }
            }
            Expr::EnumConstructor {
                payload: Some(p), ..
            } => Self::collect_callees_in_expr(p, out),
            Expr::Binary { left, right, .. } => {
                Self::collect_callees_in_expr(left, out);
                Self::collect_callees_in_expr(right, out);
            }
            Expr::Unary { expr: inner, .. }
            | Expr::Cast { expr: inner, .. }
            | Expr::TryOp { expr: inner, .. }
            | Expr::ErrorMapClosure { body: inner, .. } => {
                Self::collect_callees_in_expr(inner, out);
            }
            Expr::ArrayLit { elements, .. } => {
                for el in elements {
                    Self::collect_callees_in_expr(el, out);
                }
            }
            Expr::IfExpr {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                Self::collect_callees_in_expr(cond, out);
                Self::collect_callees_in_expr(then_branch, out);
                Self::collect_callees_in_expr(else_branch, out);
            }
            _ => {}
        }
    }
}

pub struct SemanticAnalyzer {
    file_path: String,
    diagnostics: Vec<DiagnosticEntry>,
}

impl SemanticAnalyzer {
    pub fn new(file_path: impl Into<String>) -> Self {
        Self {
            file_path: file_path.into(),
            diagnostics: Vec::new(),
        }
    }

    pub fn analyze_module(mut self, module: &Module) -> CompilerDiagnosticsReport {
        let sym_table = SymbolTable::build_for_module(module);
        let is_wasi_sandbox_mod = module.path.iter().any(|seg| {
            matches!(
                seg.as_str(),
                "sandbox" | "wasi" | "plugin" | "mod_sandbox"
            )
        });

        for item in &module.items {
            match item {
                Item::Const(c) => {
                    let empty_env = FunctionCapabilityEnv::default();
                    if let Some(actual_ty) = self.infer_expr_type(&empty_env, &sym_table, &c.value)
                    {
                        if !Self::types_compatible(&c.ty, &actual_ty) {
                            self.diagnostics.push(DiagnosticEntry {
                                error_code: "RTS-E0301".to_string(),
                                category: DiagnosticCategory::TypeMismatch,
                                file: self.file_path.clone(),
                                span: c.span,
                                ast_node: format!("ConstDecl({}: {})", c.name, c.ty),
                                message: format!(
                                    "Type mismatch in module const '{}': declared as '{}', but initializer has incompatible type '{}'.",
                                    c.name, c.ty, actual_ty
                                ),
                                deterministic_fixes: vec![],
                            });
                        }
                    }
                }
                Item::Struct(s) => self.check_struct(s, &sym_table),
                Item::Enum(_e) => {}
                Item::Function(f) => {
                    if is_wasi_sandbox_mod {
                        self.check_wasi_sandbox_constraints(f);
                    }
                    self.check_function(f, &sym_table);
                }
                Item::ExternFn(_) => {}
            }
        }

        CompilerDiagnosticsReport::from_diagnostics(self.diagnostics)
    }

    pub fn is_wasi_allowed_capability(cap: &CapabilitySpec) -> bool {
        matches!(
            cap.domain_name.as_str(),
            "StdIO"
                | "RequestArena"
                | "FrameArena"
                | "ReadRequestArena"
                | "ReadFrameArena"
                | "MonotonicClock"
                | "TimeStep"
                | "ReadColumn"
                | "MutColumn"
        ) || matches!(
            cap.kind,
            CapabilityKind::Clock
                | CapabilityKind::ReadColumn
                | CapabilityKind::MutColumn
                | CapabilityKind::ScopedArena(ArenaScope::FrameOrRequest)
                | CapabilityKind::ReadScopedArena(ArenaScope::FrameOrRequest)
        )
    }

    pub fn check_wasi_sandbox_constraints(&mut self, func: &FnDecl) {
        // 1. RTS-E0701: Funções exportadas pela sandbox (`pub fn`) devem manter fronteira `@sealed_sig` (proíbe `@open_sig`)
        if func.is_pub && !func.is_sealed_contract() {
            self.diagnostics.push(DiagnosticEntry {
                error_code: "RTS-E0701".to_string(),
                category: DiagnosticCategory::WasiSandboxContractViolation,
                file: self.file_path.clone(),
                span: func.span,
                ast_node: format!("WasiSandboxExport({})", func.name),
                message: format!(
                    "WASI sandbox exported function 'pub fn {}' cannot be marked '@open_sig'. All WASI sandbox exports must enforce an immutable '@sealed_sig' capability boundary.",
                    func.name
                ),
                deterministic_fixes: vec![],
            });
        }

        // 2. RTS-E0701: Proíbe capacidades de barramento físico direto ou privilégio irrestrito dentro da sandbox WASI
        for param in &func.params {
            if let Some(cap) = &param.capability {
                if !Self::is_wasi_allowed_capability(cap) {
                    self.diagnostics.push(DiagnosticEntry {
                        error_code: "RTS-E0701".to_string(),
                        category: DiagnosticCategory::WasiSandboxContractViolation,
                        file: self.file_path.clone(),
                        span: param.span,
                        ast_node: format!("WasiSandboxParam({}: cap {})", param.name, cap),
                        message: format!(
                            "WASI sandbox function '{}' requests forbidden physical/host capability 'cap {}'. Only virtualized WASI capabilities (StdIO, RequestArena, FrameArena, MonotonicClock, TimeStep) are permitted inside a fuel-metered sandbox.",
                            func.name, cap
                        ),
                        deterministic_fixes: vec![],
                    });
                }
            }
        }
    }

    fn is_tier_zero_forbidden(name: &str) -> bool {
        matches!(
            name,
            "DOM"
                | "DOMNode"
                | "Document"
                | "HtmlElement"
                | "CssStyle"
                | "VirtualDom"
                | "ReactComponent"
                | "WindowDom"
        )
    }

    /// Verifica se um nome de tipo é um Token de Capacidade Linear/Não-Forjável (`RTS-E0404`)
    fn is_unforgeable_capability_domain(name: &str) -> bool {
        matches!(
            name,
            "I2CBus"
                | "I2CRead"
                | "I2CPort"
                | "PWMPin"
                | "PWMWrite"
                | "GPIOPin"
                | "SPIBus"
                | "DMAChannel"
                | "Interrupt"
                | "MonotonicClock"
                | "TimeStep"
                | "ReadColumn"
                | "MutColumn"
                | "ReadSlice"
                | "MutSlice"
                | "StaticArena"
                | "LevelArena"
                | "SessionArena"
                | "FrameArena"
                | "RequestArena"
                | "TaskArena"
                | "ReadFrameArena"
                | "ReadRequestArena"
                | "ReadTaskArena"
                | "NetSocket"
                | "TcpSocket"
                | "UdpTick"
                | "HttpRoute"
                | "SqlRead"
                | "SqlWrite"
                | "GpuQueue"
                | "WebGpuDevice"
                | "WindowHost"
                | "InputDevice"
                | "FileSystem"
                | "AudioDevice"
                | "AudioInput"
                | "NetworkSocket"
        )
    }

    fn type_contains_view_or_slice(
        ty: &TypeExpr,
        sym_table: &SymbolTable,
        visiting: &mut HashSet<String>,
    ) -> bool {
        if ty.is_slice_capability() {
            return true;
        }
        match ty {
            TypeExpr::Named { name, .. } => {
                if !visiting.insert(name.clone()) {
                    return false;
                }
                let is_view = sym_table.struct_decls.get(name).map_or(false, |d| {
                    d.is_view()
                        || d.fields.iter().any(|f| {
                            Self::type_contains_view_or_slice(&f.ty, sym_table, visiting)
                        })
                });
                visiting.remove(name);
                is_view
            }
            TypeExpr::Array(inner, _) | TypeExpr::Option(inner) => {
                Self::type_contains_view_or_slice(inner, sym_table, visiting)
            }
            _ => false,
        }
    }

    fn build_fixes_for_violation(
        func: &FnDecl,
        suggested_param: &str,
        required_cap: &CapabilitySpec,
        line: usize,
    ) -> Vec<DeterministicFix> {
        if func.is_sealed_contract() {
            // Fronteira de Segurança Selada (@sealed_sig por padrão): NUNCA sugere escalação de privilégio!
            vec![DeterministicFix::remove_unauthorized_call_sealed(line)]
        } else {
            // Apenas quando o arquiteto marcou explicitamente @open_sig
            let replacement_sig = func.format_signature(Some((suggested_param, required_cap)));
            vec![
                DeterministicFix::add_cap_open_contract(replacement_sig),
                DeterministicFix::remove_unauthorized_call_sealed(line),
            ]
        }
    }

    fn is_gpu_shader_shared_type(ty: &TypeExpr) -> bool {
        match ty {
            TypeExpr::Primitive(
                PrimitiveType::F32 | PrimitiveType::I32 | PrimitiveType::U32 | PrimitiveType::Fix32,
            ) => true,
            TypeExpr::Math(
                MathType::Vec2
                | MathType::Vec3
                | MathType::Vec4
                | MathType::Quat
                | MathType::Mat3
                | MathType::Mat4,
            ) => true,
            TypeExpr::Array(inner, _) => Self::is_gpu_shader_shared_type(inner),
            _ => false,
        }
    }

    fn is_gpu_allowed_capability(cap: &CapabilitySpec) -> bool {
        matches!(
            cap.domain_name.as_str(),
            "ReadColumn"
                | "MutColumn"
                | "TimeStep"
                | "GpuQueue"
                | "WebGpuDevice"
                | "FrameArena"
                | "ReadFrameArena"
        ) || matches!(
            cap.kind,
            CapabilityKind::ReadColumn
                | CapabilityKind::MutColumn
                | CapabilityKind::Clock
                | CapabilityKind::DeviceQueue
                | CapabilityKind::ScopedArena(ArenaScope::FrameOrRequest)
                | CapabilityKind::ReadScopedArena(ArenaScope::FrameOrRequest)
        )
    }

    fn is_type_repr_c_safe(
        ty: &TypeExpr,
        sym_table: &SymbolTable,
        visiting: &mut HashSet<String>,
    ) -> bool {
        match ty {
            TypeExpr::Primitive(p) => matches!(
                p,
                PrimitiveType::I8
                    | PrimitiveType::I16
                    | PrimitiveType::I32
                    | PrimitiveType::I64
                    | PrimitiveType::U8
                    | PrimitiveType::U16
                    | PrimitiveType::U32
                    | PrimitiveType::U64
                    | PrimitiveType::Usize
                    | PrimitiveType::F32
                    | PrimitiveType::F64
                    | PrimitiveType::Fix32
                    | PrimitiveType::Bool
                    | PrimitiveType::Str64
                    | PrimitiveType::Str128
                    | PrimitiveType::Str256
            ),
            TypeExpr::Math(_) => true,
            TypeExpr::Array(inner, _) => Self::is_type_repr_c_safe(inner, sym_table, visiting),
            TypeExpr::Named { name, generics } if generics.is_empty() => {
                if !visiting.insert(name.clone()) {
                    return false;
                }
                let ok = sym_table
                    .struct_decls
                    .get(name)
                    .map(|st| {
                        (st.is_repr_c()
                            || st
                                .attributes
                                .iter()
                                .any(|a| matches!(a, Attribute::ShaderShared)))
                            && st.fields.iter().all(|f| {
                                Self::is_type_repr_c_safe(&f.ty, sym_table, visiting)
                            })
                    })
                    .unwrap_or(false);
                visiting.remove(name);
                ok
            }
            _ => false,
        }
    }

    fn check_struct(&mut self, s: &StructDecl, sym_table: &SymbolTable) {
        if Self::is_tier_zero_forbidden(&s.name) {
            self.diagnostics.push(DiagnosticEntry {
                error_code: "RTS-E0900".to_string(),
                category: DiagnosticCategory::TierZeroForbiddenDomain,
                file: self.file_path.clone(),
                span: s.span,
                ast_node: format!("StructDecl({})", s.name),
                message: format!(
                    "Struct '{}' attempts to model Tier-0 forbidden Traditional Web DOM/HTML/CSS state. Use '--emit-ts-sdk' and delegate UI to TypeScript/Next.js.",
                    s.name
                ),
                deterministic_fixes: vec![],
            });
        }

        // RTS-E0501: Bloqueia MutSlice<T> / ReadSlice<T> (ou `@view struct` aninhado) dentro de struct normal.
        // Structs anotadas com `@view` agrupam slices emprestadas localmente (visões SoA) sem escape via `return`.
        if !s.is_view() {
            for field in &s.fields {
                if field.ty.is_slice_capability()
                    || Self::type_contains_view_or_slice(&field.ty, sym_table, &mut HashSet::new())
                {
                    self.diagnostics.push(DiagnosticEntry {
                        error_code: "RTS-E0501".to_string(),
                        category: DiagnosticCategory::ArenaLifetimeEscape,
                        file: self.file_path.clone(),
                        span: field.span,
                        ast_node: format!("StructField({}.{})", s.name, field.name),
                        message: format!(
                            "Capability slices ('MutSlice<T>' / 'ReadSlice<T>') and '@view' structs cannot escape function scope via struct field '{}.{}' (prevents dangling pointers; annotate '{}' with '@view').",
                            s.name, field.name, s.name
                        ),
                        deterministic_fixes: vec![],
                    });
                }
            }
        }

        // RTS-E0602: Verifica alinhamento e tipos std430/std140 em `@shader_shared struct`
        if s.attributes.iter().any(|a| matches!(a, Attribute::ShaderShared)) {
            for field in &s.fields {
                if !Self::is_gpu_shader_shared_type(&field.ty) {
                    self.diagnostics.push(DiagnosticEntry {
                        error_code: "RTS-E0602".to_string(),
                        category: DiagnosticCategory::GpuLayoutAlignmentViolation,
                        file: self.file_path.clone(),
                        span: field.span,
                        ast_node: format!("ShaderSharedField({}.{})", s.name, field.name),
                        message: format!(
                            "Field '{}.{}' in '@shader_shared' struct has type '{}', which violates GPU std430/std140 layout alignment rules. Only 32-bit scalars (f32, i32, u32), SIMD vectors/matrices (vec2..vec4, quat, mat3, mat4), and fixed-size arrays [T; N] are permitted on GPU.",
                            s.name, field.name, field.ty
                        ),
                        deterministic_fixes: vec![],
                    });
                }
            }
        }

        // RTS-E0602: Verifica layout binário estrito ABI C em `@repr_c struct` (proíbe `str`, `cap`, `Handle`, etc.)
        if s.is_repr_c() {
            for field in &s.fields {
                let mut visiting = HashSet::new();
                visiting.insert(s.name.clone());
                if !Self::is_type_repr_c_safe(&field.ty, sym_table, &mut visiting) {
                    self.diagnostics.push(DiagnosticEntry {
                        error_code: "RTS-E0602".to_string(),
                        category: DiagnosticCategory::GpuLayoutAlignmentViolation,
                        file: self.file_path.clone(),
                        span: field.span,
                        ast_node: format!("ReprCField({}.{})", s.name, field.name),
                        message: format!(
                            "@repr_c struct '{}' field '{}' has non-POD/pointer type '{}'. Only fixed-width scalars, fixed arrays [T; N], str64/str128/str256, and nested @repr_c structs are allowed.",
                            s.name, field.name, field.ty
                        ),
                        deterministic_fixes: vec![],
                    });
                }
            }
        }
    }

    fn check_gpu_kernel_constraints(&mut self, func: &FnDecl, sym_table: &SymbolTable) {
        // 1. RTS-E0601: Bloqueia capacidades de CPU/IO/Hardware em `@gpu_kernel`
        for param in &func.params {
            if let Some(cap) = &param.capability {
                if !Self::is_gpu_allowed_capability(cap) {
                    self.diagnostics.push(DiagnosticEntry {
                        error_code: "RTS-E0601".to_string(),
                        category: DiagnosticCategory::GpuForbiddenCapability,
                        file: self.file_path.clone(),
                        span: param.span,
                        ast_node: format!("GpuKernelParam({}: cap {})", param.name, cap),
                        message: format!(
                            "GPU kernel '{}' cannot request host/hardware capability 'cap {}'. Only GPU storage/uniform capabilities (ReadColumn, MutColumn, TimeStep, GpuQueue, WebGpuDevice) are permitted in '@gpu_kernel'.",
                            func.name, cap
                        ),
                        deterministic_fixes: vec![],
                    });
                }
            }
        }

        // 2. Coleta chamadas diretas no corpo do kernel para verificar RTS-E0601 (FFI / Host I/O) e RTS-E0603 (Recursão)
        let mut direct_callees = Vec::new();
        SymbolTable::collect_callees_in_stmts(&func.body, &mut direct_callees);

        for raw_callee in &direct_callees {
            let short = raw_callee.strip_prefix("self::").unwrap_or(raw_callee);
            if sym_table.extern_fns.contains(short) || raw_callee.starts_with("io::") {
                self.diagnostics.push(DiagnosticEntry {
                    error_code: "RTS-E0601".to_string(),
                    category: DiagnosticCategory::GpuForbiddenCapability,
                    file: self.file_path.clone(),
                    span: func.span,
                    ast_node: format!("GpuKernelHostCall({} -> {})", func.name, raw_callee),
                    message: format!(
                        "GPU kernel '{}' invokes host/FFI routine '{}', which is forbidden in SPIR-V/WGSL compute shaders.",
                        func.name, raw_callee
                    ),
                    deterministic_fixes: vec![],
                });
            }
        }

        // 3. RTS-E0603: Detecta recursão direta ou mútua no grafo de chamadas a partir de `func.name`
        let mut visited = HashSet::new();
        let mut queue: Vec<String> = direct_callees
            .iter()
            .map(|c| c.strip_prefix("self::").unwrap_or(c).to_string())
            .collect();
        let mut is_recursive = false;
        while let Some(curr) = queue.pop() {
            if curr == func.name {
                is_recursive = true;
                break;
            }
            if visited.insert(curr.clone()) {
                if let Some(next_callees) = sym_table.call_edges.get(&curr) {
                    for nc in next_callees {
                        let norm = nc.strip_prefix("self::").unwrap_or(nc).to_string();
                        queue.push(norm);
                    }
                }
            }
        }

        if is_recursive {
            self.diagnostics.push(DiagnosticEntry {
                error_code: "RTS-E0603".to_string(),
                category: DiagnosticCategory::GpuRecursionOrDynamicEscape,
                file: self.file_path.clone(),
                span: func.span,
                ast_node: format!("GpuKernelRecursion({})", func.name),
                message: format!(
                    "GPU kernel '{}' exhibits direct or mutual recursion, which is strictly forbidden by SPIR-V (OpCapability Shader) and WebGPU WGSL execution models.",
                    func.name
                ),
                deterministic_fixes: vec![],
            });
        }
    }

    fn check_function(&mut self, func: &FnDecl, sym_table: &SymbolTable) {
        let mut env = FunctionCapabilityEnv::from_fn_decl(func);

        if func
            .return_type
            .as_ref()
            .map(|t| Self::type_contains_view_or_slice(t, sym_table, &mut HashSet::new()))
            .unwrap_or(false)
        {
            self.diagnostics.push(DiagnosticEntry {
                error_code: "RTS-E0501".to_string(),
                category: DiagnosticCategory::ArenaLifetimeEscape,
                file: self.file_path.clone(),
                span: func.span,
                ast_node: format!("FnReturn({})", func.name),
                message: format!(
                    "Function '{}' cannot return capability slice ('MutSlice<T>' / 'ReadSlice<T>') or '@view' struct; slice capabilities must remain scoped to caller lifetimes.",
                    func.name
                ),
                deterministic_fixes: vec![],
            });
        }

        if func.attributes.iter().any(|a| {
            matches!(
                a,
                Attribute::GpuKernel | Attribute::VertexShader | Attribute::FragmentShader
            )
        }) {
            self.check_gpu_kernel_constraints(func, sym_table);
        }

        // 1. Verifica capacidades Tier 0 Banidas (DOM/HTML)
        for param in &func.params {
            if let Some(cap) = &param.capability {
                if Self::is_tier_zero_forbidden(&cap.domain_name) {
                    self.diagnostics.push(DiagnosticEntry {
                        error_code: "RTS-E0900".to_string(),
                        category: DiagnosticCategory::TierZeroForbiddenDomain,
                        file: self.file_path.clone(),
                        span: param.span,
                        ast_node: format!("FnParam({}: cap {})", param.name, cap),
                        message: format!(
                            "Capability 'cap {}' is strictly banned in RustTS (Tier-0 Traditional Web DOM).",
                            cap
                        ),
                        deterministic_fixes: vec![],
                    });
                }
            }
        }

        // 2. Verifica ausência de Aliasing entre colunas ECS (`MutColumn<T>` duplicado ou `MutColumn<T>` + `ReadColumn<T>`)
        self.check_ecs_column_aliasing(func);

        // 3. Percorre todas as instruções do corpo validando:
        //    - RTS-E0402 (Violação Direta de Capacidade)
        //    - RTS-E0403 (Violação Transitiva no Call Graph: Caps(Callee) <= Caps(Caller))
        //    - RTS-E0404 (Tentativa de Falsificação/Forja de Token de Capacidade)
        //    - RTS-E0501 (Fuga de Tempo de Vida entre Arenas: Rank(Origem) < Rank(Destino))
        for stmt in &func.body {
            self.check_stmt(func, &mut env, sym_table, stmt);
        }
    }

    fn check_ecs_column_aliasing(&mut self, func: &FnDecl) {
        let mut seen_columns: HashMap<String, (bool, Span)> = HashMap::new();
        let mut seen_arenas: HashMap<String, (bool, Span)> = HashMap::new();

        for param in &func.params {
            if let Some(spec) = &param.capability {
                match spec.kind {
                    CapabilityKind::MutColumn | CapabilityKind::ReadColumn => {
                        let is_mut = spec.kind == CapabilityKind::MutColumn;
                        if let Some(CapGenericArg::TypeName(comp_type)) = &spec.generic_arg {
                            if let Some((prev_mut, _)) = seen_columns.get(comp_type) {
                                if *prev_mut || is_mut {
                                    self.diagnostics.push(DiagnosticEntry {
                                        error_code: "RTS-E0405".to_string(),
                                        category: DiagnosticCategory::AliasingViolation,
                                        file: self.file_path.clone(),
                                        span: param.span,
                                        ast_node: format!("FnParam({}: cap {})", param.name, spec),
                                        message: format!(
                                            "Function '{}' declares conflicting ECS column capabilities for component '{}'. Parallel SIMD scheduling requires exclusive MutColumn access.",
                                            func.name, comp_type
                                        ),
                                        deterministic_fixes: vec![],
                                    });
                                }
                            } else {
                                seen_columns.insert(comp_type.clone(), (is_mut, param.span));
                            }
                        }
                    }
                    CapabilityKind::ScopedArena(_) | CapabilityKind::ReadScopedArena(_) => {
                        let is_mut = matches!(spec.kind, CapabilityKind::ScopedArena(_));
                        if let Some(CapGenericArg::TypeName(ev_type)) = &spec.generic_arg {
                            if let Some((prev_mut, _)) = seen_arenas.get(ev_type) {
                                if *prev_mut || is_mut {
                                    self.diagnostics.push(DiagnosticEntry {
                                        error_code: "RTS-E0405".to_string(),
                                        category: DiagnosticCategory::AliasingViolation,
                                        file: self.file_path.clone(),
                                        span: param.span,
                                        ast_node: format!("FnParam({}: cap {})", param.name, spec),
                                        message: format!(
                                            "Function '{}' declares conflicting FrameArena capabilities ('FrameArena<{}>' and 'ReadFrameArena<{}>') in the same system stage. Producer and Consumer systems must be separated into sequential stages.",
                                            func.name, ev_type, ev_type
                                        ),
                                        deterministic_fixes: vec![],
                                    });
                                }
                            } else {
                                seen_arenas.insert(ev_type.clone(), (is_mut, param.span));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn check_stmt(
        &mut self,
        func: &FnDecl,
        env: &mut FunctionCapabilityEnv,
        sym_table: &SymbolTable,
        stmt: &Stmt,
    ) {
        match stmt {
            Stmt::Let {
                name,
                is_mut,
                ty,
                value,
                span,
            } => {
                if *is_mut {
                    env.immutable_locals.remove(name);
                    env.mutable_bindings.insert(name.clone());
                } else {
                    env.immutable_locals.insert(name.clone());
                    env.mutable_bindings.remove(name);
                }

                // RTS-E0404: Bloqueia declaração de variável local com tipo `cap ...` forjado fora do entrypoint `main`,
                // exceto derivação autorizada `let buf: cap MutSlice<T> = arena.alloc_slice(N);`.
                if let Some(TypeExpr::Capability(forged_cap)) = ty {
                    let is_authorized_slice_alloc = forged_cap.kind == CapabilityKind::MutSlice
                        && matches!(
                            value,
                            Expr::MethodCall { method, .. } if method == "alloc_slice"
                        );
                    if func.name != "main" && !is_authorized_slice_alloc {
                        self.diagnostics.push(DiagnosticEntry {
                            error_code: "RTS-E0404".to_string(),
                            category: DiagnosticCategory::CapabilityForgeryAttempt,
                            file: self.file_path.clone(),
                            span: *span,
                            ast_node: format!("LetStmt({}: cap {})", name, forged_cap),
                            message: format!(
                                "Function '{}' attempted to forge capability token 'cap {}' in local variable '{}'. Capability tokens are linear/unforgeable and can only be instantiated inside the root 'main' entrypoint.",
                                func.name, forged_cap, name
                            ),
                            deterministic_fixes: vec![
                                DeterministicFix::remove_unauthorized_call_sealed(span.line_start),
                            ],
                        });
                    } else {
                        // Entrypoint `main` ou derivação via `arena.alloc_slice(N)` concede capacidade escopada
                        if let Some(scope) = FunctionCapabilityEnv::capability_arena_scope(forged_cap)
                        {
                            env.local_var_arena_scope
                                .insert(name.clone(), (scope, forged_cap.to_string()));
                        }
                        env.param_caps.insert(name.clone(), forged_cap.clone());
                        env.granted_specs.push((name.clone(), forged_cap.clone()));
                    }
                }

                self.check_expr(func, env, sym_table, value);

                let inferred_ty = self.infer_expr_type(env, sym_table, value);
                if let (Some(declared_ty), Some(actual_ty)) = (ty, &inferred_ty) {
                    if !matches!(declared_ty, TypeExpr::Capability(_))
                        && !Self::types_compatible(declared_ty, actual_ty)
                    {
                        self.diagnostics.push(DiagnosticEntry {
                            error_code: "RTS-E0301".to_string(),
                            category: DiagnosticCategory::TypeMismatch,
                            file: self.file_path.clone(),
                            span: *span,
                            ast_node: format!("LetStmt({}: {})", name, declared_ty),
                            message: format!(
                                "Type mismatch in '{}': local variable '{}' is declared as '{}', but initializer expression has incompatible type '{}'.",
                                func.name, name, declared_ty, actual_ty
                            ),
                            deterministic_fixes: vec![],
                        });
                    }
                }

                if let Some(var_ty) = ty.clone().or(inferred_ty) {
                    if ty.is_none() && func.name == "main" {
                        if let TypeExpr::Capability(cap_spec) = &var_ty {
                            if let Some(scope) = FunctionCapabilityEnv::capability_arena_scope(cap_spec)
                            {
                                env.local_var_arena_scope
                                    .insert(name.clone(), (scope, cap_spec.to_string()));
                            }
                            env.param_caps.insert(name.clone(), cap_spec.clone());
                            env.granted_specs.push((name.clone(), cap_spec.clone()));
                        }
                    }
                    env.local_var_types.insert(name.clone(), var_ty);
                }

                // Rastreia a Arena de origem se a expressão alocar ou extrair um Handle/referência de uma Arena
                if let Some((origin_scope, origin_label)) = self.infer_expr_arena_scope(env, value)
                {
                    env.local_var_arena_scope
                        .insert(name.clone(), (origin_scope, origin_label));
                }
            }
            Stmt::Assign {
                target,
                value,
                span,
            } => {
                self.check_mutation_target(func, env, sym_table, target, *span);
                self.check_arena_escape_on_assign(func, env, target, value, *span);
                self.check_expr(func, env, sym_table, target);
                self.check_expr(func, env, sym_table, value);
                if let (Some(lhs_ty), Some(rhs_ty)) = (
                    self.infer_expr_type(env, sym_table, target),
                    self.infer_expr_type(env, sym_table, value),
                ) {
                    if !Self::types_compatible(&lhs_ty, &rhs_ty) {
                        self.diagnostics.push(DiagnosticEntry {
                            error_code: "RTS-E0301".to_string(),
                            category: DiagnosticCategory::TypeMismatch,
                            file: self.file_path.clone(),
                            span: *span,
                            ast_node: format!("AssignStmt({} = {})", lhs_ty, rhs_ty),
                            message: format!(
                                "Type mismatch in assignment inside '{}': target has type '{}', but assigned value has type '{}'.",
                                func.name, lhs_ty, rhs_ty
                            ),
                            deterministic_fixes: vec![],
                        });
                    }
                }
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.check_expr(func, env, sym_table, condition);
                for s in then_branch {
                    self.check_stmt(func, env, sym_table, s);
                }
                if let Some(else_stmts) = else_branch {
                    for s in else_stmts {
                        self.check_stmt(func, env, sym_table, s);
                    }
                }
            }
            Stmt::Match { expr, arms, span } => {
                self.check_expr(func, env, sym_table, expr);
                let scrutinee_ty = self.infer_expr_type(env, sym_table, expr);
                let enum_name_opt = match &scrutinee_ty {
                    Some(TypeExpr::Named { name, .. }) if sym_table.enums.contains_key(name) => {
                        Some(name.clone())
                    }
                    _ => arms.iter().find_map(|a| match &a.pattern {
                        MatchPattern::Variant {
                            enum_name: Some(en),
                            ..
                        } => Some(en.clone()),
                        _ => None,
                    }),
                };

                let has_wildcard = arms
                    .iter()
                    .any(|a| matches!(a.pattern, MatchPattern::Wildcard));
                let covered: HashSet<&str> = arms
                    .iter()
                    .filter_map(|a| match &a.pattern {
                        MatchPattern::Variant { variant, .. } => Some(variant.as_str()),
                        MatchPattern::Wildcard => None,
                    })
                    .collect();

                if let Some(TypeExpr::Result(_, _)) = &scrutinee_ty {
                    if !has_wildcard {
                        let mut missing = Vec::new();
                        if !covered.contains("Ok") {
                            missing.push("Ok(_)".to_string());
                        }
                        if !covered.contains("Err") {
                            missing.push("Err(_)".to_string());
                        }
                        if !missing.is_empty() {
                            self.diagnostics.push(DiagnosticEntry {
                                error_code: "RTS-E0302".to_string(),
                                category: DiagnosticCategory::NonExhaustiveMatch,
                                file: self.file_path.clone(),
                                span: *span,
                                ast_node: "MatchStmt(Result)".to_string(),
                                message: format!(
                                    "Non-exhaustive match in '{}': Result variants not covered: [{}]. Both 'Ok' and 'Err' (or '_') must be handled.",
                                    func.name,
                                    missing.join(", ")
                                ),
                                deterministic_fixes: vec![],
                            });
                        }
                    }
                } else if let Some(TypeExpr::Option(_)) = &scrutinee_ty {
                    if !has_wildcard {
                        let mut missing = Vec::new();
                        if !covered.contains("Some") {
                            missing.push("Some(_)".to_string());
                        }
                        if !covered.contains("None") {
                            missing.push("None".to_string());
                        }
                        if !missing.is_empty() {
                            self.diagnostics.push(DiagnosticEntry {
                                error_code: "RTS-E0302".to_string(),
                                category: DiagnosticCategory::NonExhaustiveMatch,
                                file: self.file_path.clone(),
                                span: *span,
                                ast_node: "MatchStmt(Option)".to_string(),
                                message: format!(
                                    "Non-exhaustive match in '{}': Option variants not covered: [{}]. Both 'Some' and 'None' (or '_') must be handled.",
                                    func.name,
                                    missing.join(", ")
                                ),
                                deterministic_fixes: vec![],
                            });
                        }
                    }
                } else if let Some(enum_name) = &enum_name_opt {
                    if let Some(declared_variants) = sym_table.enums.get(enum_name) {
                        if !has_wildcard {
                            let missing: Vec<String> = declared_variants
                                .iter()
                                .filter(|(vname, _)| !covered.contains(vname.as_str()))
                                .map(|(vname, _)| format!("{}::{}", enum_name, vname))
                                .collect();
                            if !missing.is_empty() {
                                self.diagnostics.push(DiagnosticEntry {
                                    error_code: "RTS-E0302".to_string(),
                                    category: DiagnosticCategory::NonExhaustiveMatch,
                                    file: self.file_path.clone(),
                                    span: *span,
                                    ast_node: format!("MatchStmt({})", enum_name),
                                    message: format!(
                                        "Non-exhaustive match in '{}': enum '{}' variants not covered: [{}]. Add explicit arms for all variants or a '_' wildcard arm.",
                                        func.name,
                                        enum_name,
                                        missing.join(", ")
                                    ),
                                    deterministic_fixes: vec![],
                                });
                            }
                        }
                    }
                }

                for arm in arms {
                    let mut arm_env = env.clone();
                    if let MatchPattern::Variant {
                        enum_name: pat_en,
                        variant,
                        binding: Some(bind_name),
                    } = &arm.pattern
                    {
                        match &scrutinee_ty {
                            Some(TypeExpr::Result(ok_ty, err_ty)) => {
                                if variant == "Ok" {
                                    arm_env
                                        .local_var_types
                                        .insert(bind_name.clone(), (**ok_ty).clone());
                                } else if variant == "Err" {
                                    arm_env
                                        .local_var_types
                                        .insert(bind_name.clone(), (**err_ty).clone());
                                }
                            }
                            Some(TypeExpr::Option(inner_ty)) => {
                                if variant == "Some" {
                                    arm_env
                                        .local_var_types
                                        .insert(bind_name.clone(), (**inner_ty).clone());
                                }
                            }
                            _ => {
                                let resolved_en = pat_en.as_ref().or(enum_name_opt.as_ref());
                                if let Some(en) = resolved_en {
                                    if let Some(variants) = sym_table.enums.get(en) {
                                        if let Some((_, Some(payload_ty))) =
                                            variants.iter().find(|(v, _)| v == variant)
                                        {
                                            arm_env
                                                .local_var_types
                                                .insert(bind_name.clone(), payload_ty.clone());
                                        }
                                    }
                                }
                            }
                        }
                    }
                    for s in &arm.body {
                        self.check_stmt(func, &mut arm_env, sym_table, s);
                    }
                }
            }
            Stmt::ForRange {
                iterator,
                start,
                end,
                body,
                ..
            } => {
                self.check_expr(func, env, sym_table, start);
                self.check_expr(func, env, sym_table, end);
                env.local_var_types.insert(
                    iterator.clone(),
                    TypeExpr::Primitive(PrimitiveType::Usize),
                );
                for s in body {
                    self.check_stmt(func, env, sym_table, s);
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                self.check_expr(func, env, sym_table, condition);
                for s in body {
                    self.check_stmt(func, env, sym_table, s);
                }
            }
            Stmt::Break { .. } | Stmt::Continue { .. } => {}
            Stmt::Return { value, span } => {
                if let Some(expr) = value {
                    self.check_expr(func, env, sym_table, expr);
                    if let Some(expected_ret) = &func.return_type {
                        if let Some(actual_ret) = self.infer_expr_type(env, sym_table, expr) {
                            if !Self::types_compatible(expected_ret, &actual_ret) {
                                self.diagnostics.push(DiagnosticEntry {
                                    error_code: "RTS-E0301".to_string(),
                                    category: DiagnosticCategory::TypeMismatch,
                                    file: self.file_path.clone(),
                                    span: *span,
                                    ast_node: format!("ReturnStmt({})", func.name),
                                    message: format!(
                                        "Return type mismatch in '{}': function signature declares return type '{}', but returned expression has type '{}'.",
                                        func.name, expected_ret, actual_ret
                                    ),
                                    deterministic_fixes: vec![],
                                });
                            }
                        }
                    }
                } else if let Some(expected_ret) = &func.return_type {
                    self.diagnostics.push(DiagnosticEntry {
                        error_code: "RTS-E0301".to_string(),
                        category: DiagnosticCategory::TypeMismatch,
                        file: self.file_path.clone(),
                        span: *span,
                        ast_node: format!("ReturnStmt({})", func.name),
                        message: format!(
                            "Return type mismatch in '{}': function expects return value of type '{}', but found empty 'return;'.",
                            func.name, expected_ret
                        ),
                        deterministic_fixes: vec![],
                    });
                }
            }
            Stmt::ExprStmt { expr, .. } => {
                self.check_expr(func, env, sym_table, expr);
            }
        }
    }

    /// Inferência de Escopo de Arena (`ArenaScope`) de uma expressão:
    /// Ex: `frame_arena.alloc(...)` ou `frame_arena.alloc_handle(...)` -> `FrameOrRequest (Rank = 1)`
    fn infer_expr_arena_scope(
        &self,
        env: &FunctionCapabilityEnv,
        expr: &Expr,
    ) -> Option<(ArenaScope, String)> {
        match expr {
            Expr::Ident(name, _) => env.local_var_arena_scope.get(name).cloned(),
            Expr::MethodCall {
                receiver,
                method,
                args,
                ..
            } => {
                if let Some(root) = Self::extract_root_receiver(receiver) {
                    if let Some((scope, label)) = env.local_var_arena_scope.get(&root) {
                        if matches!(
                            method.as_str(),
                            "alloc" | "alloc_handle" | "push_handle" | "spawn_temp" | "handle_at"
                        ) {
                            return Some((*scope, label.clone()));
                        }
                    }
                }
                for a in args {
                    if let Some(found) = self.infer_expr_arena_scope(env, a) {
                        return Some(found);
                    }
                }
                None
            }
            Expr::Index { receiver, .. } | Expr::FieldAccess { receiver, .. } => {
                self.infer_expr_arena_scope(env, receiver)
            }
            Expr::TryOp { expr: inner, .. }
            | Expr::Cast { expr: inner, .. }
            | Expr::Unary { expr: inner, .. } => {
                self.infer_expr_arena_scope(env, inner)
            }
            Expr::StructInit { fields, .. } => {
                // O escopo de uma struct composta é limitado pela menor Arena (mais efêmera) de seus campos
                let mut min_scope: Option<(ArenaScope, String)> = None;
                for (_, field_expr) in fields {
                    if let Some((s, lbl)) = self.infer_expr_arena_scope(env, field_expr) {
                        match &min_scope {
                            Some((curr, _)) if s.rank() < curr.rank() => {
                                min_scope = Some((s, lbl));
                            }
                            None => {
                                min_scope = Some((s, lbl));
                            }
                            _ => {}
                        }
                    }
                }
                min_scope
            }
            Expr::ArrayLit { elements, .. } => {
                let mut min_scope: Option<(ArenaScope, String)> = None;
                for el in elements {
                    if let Some((s, lbl)) = self.infer_expr_arena_scope(env, el) {
                        match &min_scope {
                            Some((curr, _)) if s.rank() < curr.rank() => min_scope = Some((s, lbl)),
                            None => min_scope = Some((s, lbl)),
                            _ => {}
                        }
                    }
                }
                min_scope
            }
            _ => None,
        }
    }

    /// Regra Matemática `RTS-E0501` (`ArenaLifetimeEscape`):
    /// Bloqueia atribuição de `Handle<T, ArenaOrigem>` em destino de `ArenaDestino` se `Rank(ArenaOrigem) < Rank(ArenaDestino)`,
    /// onde `StaticArena(3) > LevelArena/MutColumn(2) > FrameArena/RequestArena(1)`.
    fn check_arena_escape_on_assign(
        &mut self,
        func: &FnDecl,
        env: &FunctionCapabilityEnv,
        target: &Expr,
        value: &Expr,
        span: Span,
    ) {
        if let Some(target_root) = Self::extract_root_receiver(target) {
            if let Some((dest_scope, dest_label)) = env.local_var_arena_scope.get(&target_root) {
                if let Some((origin_scope, origin_label)) = self.infer_expr_arena_scope(env, value)
                {
                    if origin_scope.rank() < dest_scope.rank() {
                        self.diagnostics.push(DiagnosticEntry {
                            error_code: "RTS-E0501".to_string(),
                            category: DiagnosticCategory::ArenaLifetimeEscape,
                            file: self.file_path.clone(),
                            span,
                            ast_node: format!("ArenaEscapeAssign({} <- {})", target_root, origin_label),
                            message: format!(
                                "Arena Lifetime Escape in '{}': cannot store handle/data allocated in ephemeral '{}' ({}) into longer-lived '{}' ({}). Rank({}) = {} < Rank({}) = {}, which would cause Use-After-Reset on the next frame/request.",
                                func.name,
                                origin_label,
                                origin_scope.label(),
                                target_root,
                                dest_scope.label(),
                                origin_label,
                                origin_scope.rank(),
                                dest_label,
                                dest_scope.rank()
                            ),
                            deterministic_fixes: vec![DeterministicFix::remove_arena_escape(
                                span.line_start,
                                dest_scope.label().to_string(),
                            )],
                        });
                    }
                }
            }
        }
    }

    fn check_mutation_target(
        &mut self,
        func: &FnDecl,
        env: &FunctionCapabilityEnv,
        sym_table: &SymbolTable,
        target: &Expr,
        span: Span,
    ) {
        let index_recv_cap = if let Expr::Index { receiver, .. } = target {
            if let Some(TypeExpr::Capability(c)) = self.infer_expr_type(env, sym_table, receiver) {
                Some(c)
            } else {
                None
            }
        } else {
            None
        };
        if let Some(root_ident) = Self::extract_root_receiver(target) {
            if sym_table.consts.contains_key(&root_ident) {
                self.diagnostics.push(DiagnosticEntry {
                    error_code: "RTS-E0405".to_string(),
                    category: DiagnosticCategory::AliasingViolation,
                    file: self.file_path.clone(),
                    span,
                    ast_node: format!("AssignConst({})", root_ident),
                    message: format!(
                        "Cannot reassign immutable module constant '{}' in function '{}'. Constants declared with 'const' are strictly read-only.",
                        root_ident, func.name
                    ),
                    deterministic_fixes: vec![],
                });
            } else if env.immutable_locals.contains(&root_ident) {
                let is_mut_cap_element_write = !matches!(target, Expr::Ident(_, _))
                    && (index_recv_cap.as_ref().map_or(false, |c| {
                        matches!(c.kind, CapabilityKind::MutSlice | CapabilityKind::MutColumn)
                    }) || env.param_caps.get(&root_ident).map_or(false, |c| {
                        matches!(c.kind, CapabilityKind::MutSlice | CapabilityKind::MutColumn)
                    }));
                if !is_mut_cap_element_write {
                    self.diagnostics.push(DiagnosticEntry {
                        error_code: "RTS-E0405".to_string(),
                        category: DiagnosticCategory::AliasingViolation,
                        file: self.file_path.clone(),
                        span,
                        ast_node: format!("AssignExpr({})", root_ident),
                        message: format!(
                            "Cannot reassign immutable local variable '{}' in function '{}'. Declare it as 'let mut {}' to allow mutation.",
                            root_ident, func.name, root_ident
                        ),
                        deterministic_fixes: vec![],
                    });
                }
            }
            if let Some(cap_spec) = index_recv_cap
                .as_ref()
                .or_else(|| env.param_caps.get(&root_ident))
            {
                if matches!(
                    cap_spec.kind,
                    CapabilityKind::ReadColumn
                        | CapabilityKind::ReadSlice
                        | CapabilityKind::ReadResource
                        | CapabilityKind::ReadScopedArena(_)
                ) {
                    self.diagnostics.push(DiagnosticEntry {
                        error_code: "RTS-E0402".to_string(),
                        category: DiagnosticCategory::CapabilityViolation,
                        file: self.file_path.clone(),
                        span,
                        ast_node: format!("AssignExpr({})", root_ident),
                        message: format!(
                            "Function '{}' attempted to mutate '{}' which only holds read-only capability 'cap {}'.",
                            func.name, root_ident, cap_spec
                        ),
                        deterministic_fixes: vec![
                            DeterministicFix::remove_unauthorized_call_sealed(span.line_start),
                        ],
                    });
                }
            }
        }
    }

    fn extract_root_receiver(expr: &Expr) -> Option<String> {
        match expr {
            Expr::Ident(name, _) => Some(name.clone()),
            Expr::FieldAccess { receiver, .. } => Self::extract_root_receiver(receiver),
            Expr::Index { receiver, .. } => Self::extract_root_receiver(receiver),
            _ => None,
        }
    }

    fn check_expr(
        &mut self,
        func: &FnDecl,
        env: &FunctionCapabilityEnv,
        sym_table: &SymbolTable,
        expr: &Expr,
    ) {
        match expr {
            Expr::StructInit {
                struct_name,
                fields,
                span,
            } => {
                // RTS-E0404: Bloqueia tentativa de instanciar um Token de Capacidade via StructInit literal
                if Self::is_unforgeable_capability_domain(struct_name) && func.name != "main" {
                    self.diagnostics.push(DiagnosticEntry {
                        error_code: "RTS-E0404".to_string(),
                        category: DiagnosticCategory::CapabilityForgeryAttempt,
                        file: self.file_path.clone(),
                        span: *span,
                        ast_node: format!("StructInit({})", struct_name),
                        message: format!(
                            "Function '{}' attempted to instantiate capability token '{}' via struct literal. Capability tokens are linear/unforgeable and cannot be forged outside the runtime entrypoint.",
                            func.name, struct_name
                        ),
                        deterministic_fixes: vec![
                            DeterministicFix::remove_unauthorized_call_sealed(span.line_start),
                        ],
                    });
                }

                if let Some(declared_fields) = sym_table.structs.get(struct_name) {
                    for (field_name, field_expr) in fields {
                        match declared_fields.get(field_name) {
                            Some(expected_field_ty) => {
                                if let Some(actual_field_ty) =
                                    self.infer_expr_type(env, sym_table, field_expr)
                                {
                                    if !Self::types_compatible(expected_field_ty, &actual_field_ty)
                                    {
                                        self.diagnostics.push(DiagnosticEntry {
                                            error_code: "RTS-E0301".to_string(),
                                            category: DiagnosticCategory::TypeMismatch,
                                            file: self.file_path.clone(),
                                            span: *span,
                                            ast_node: format!(
                                                "StructInitField({}.{})",
                                                struct_name, field_name
                                            ),
                                            message: format!(
                                                "Type mismatch in struct literal '{}' inside '{}': field '{}' expects '{}', but found '{}'.",
                                                struct_name,
                                                func.name,
                                                field_name,
                                                expected_field_ty,
                                                actual_field_ty
                                            ),
                                            deterministic_fixes: vec![],
                                        });
                                    }
                                }
                            }
                            None => {
                                self.diagnostics.push(DiagnosticEntry {
                                    error_code: "RTS-E0301".to_string(),
                                    category: DiagnosticCategory::TypeMismatch,
                                    file: self.file_path.clone(),
                                    span: *span,
                                    ast_node: format!(
                                        "StructInitField({}.{})",
                                        struct_name, field_name
                                    ),
                                    message: format!(
                                        "Unknown field '{}' in struct literal '{}' inside '{}'.",
                                        field_name, struct_name, func.name
                                    ),
                                    deterministic_fixes: vec![],
                                });
                            }
                        }
                    }
                }

                for (_, field_expr) in fields {
                    self.check_expr(func, env, sym_table, field_expr);
                }
            }

            Expr::StaticCall {
                namespace,
                func: called_fn,
                args,
                span,
            } => {
                // 1. Verifica Tier 0 Banido (DOM/HTML)
                if Self::is_tier_zero_forbidden(namespace) {
                    self.diagnostics.push(DiagnosticEntry {
                        error_code: "RTS-E0900".to_string(),
                        category: DiagnosticCategory::TierZeroForbiddenDomain,
                        file: self.file_path.clone(),
                        span: *span,
                        ast_node: format!("CallExpr({}::{})", namespace, called_fn),
                        message: format!(
                            "Function '{}' invoked forbidden Tier-0 DOM operation '{}::{}'. Traditional Web DOM is strictly banned in RustTS.",
                            func.name, namespace, called_fn
                        ),
                        deterministic_fixes: vec![
                            DeterministicFix::remove_unauthorized_call_sealed(span.line_start),
                        ],
                    });
                }
                // 2. RTS-E0404: Verifica se é uma tentativa de forjar uma capacidade via `I2CBus::new()`, `NetSocket::forge()`, etc.
                else if Self::is_unforgeable_capability_domain(namespace)
                    && matches!(
                        called_fn.as_str(),
                        "new" | "create" | "forge" | "unsafe_acquire" | "open_ambient"
                    )
                    && func.name != "main"
                {
                    self.diagnostics.push(DiagnosticEntry {
                        error_code: "RTS-E0404".to_string(),
                        category: DiagnosticCategory::CapabilityForgeryAttempt,
                        file: self.file_path.clone(),
                        span: *span,
                        ast_node: format!("CallExpr({}::{})", namespace, called_fn),
                        message: format!(
                            "Function '{}' attempted to forge capability token via '{}::{}'. Capabilities are unforgeable linear tokens and must be injected via function parameters.",
                            func.name, namespace, called_fn
                        ),
                        deterministic_fixes: vec![
                            DeterministicFix::remove_unauthorized_call_sealed(span.line_start),
                        ],
                    });
                }
                if namespace == "mem" || namespace == "core.mem" {
                    for a in args {
                        self.check_expr(func, env, sym_table, a);
                    }
                    let expected_arity = match called_fn.as_str() {
                        "addr_of" | "slice_ptr" | "free_aligned" | "str_len" | "f32_from_bits"
                        | "f32_to_bits" => Some(1usize),
                        "zero" | "alloc_aligned" | "str_byte" => Some(2usize),
                        "copy" => Some(3usize),
                        _ => None,
                    };
                    match expected_arity {
                        Some(exp) if args.len() != exp => {
                            self.diagnostics.push(DiagnosticEntry {
                                error_code: "RTS-E0301".to_string(),
                                category: DiagnosticCategory::TypeMismatch,
                                file: self.file_path.clone(),
                                span: *span,
                                ast_node: format!("MemIntrinsic({}::{})", namespace, called_fn),
                                message: format!(
                                    "Intrinsic 'mem::{}()' in '{}' expected {} argument(s), found {}.",
                                    called_fn, func.name, exp, args.len()
                                ),
                                deterministic_fixes: vec![],
                            });
                        }
                        None => {
                            self.diagnostics.push(DiagnosticEntry {
                                error_code: "RTS-E0301".to_string(),
                                category: DiagnosticCategory::TypeMismatch,
                                file: self.file_path.clone(),
                                span: *span,
                                ast_node: format!("MemIntrinsic({}::{})", namespace, called_fn),
                                message: format!(
                                    "Unknown 'core.mem' intrinsic 'mem::{}()' in '{}'.",
                                    called_fn, func.name
                                ),
                                deterministic_fixes: vec![],
                            });
                        }
                        _ => {}
                    }
                }
                // 3. Resolução Simbólica e Propagação Transitiva no Call Graph (`SymbolTable`)
                else {
                    let qualified_key = if namespace == "self" {
                        called_fn.clone()
                    } else {
                        format!("{}::{}", namespace, called_fn)
                    };
                    let is_builtin_ns = matches!(
                        namespace.as_str(),
                        "io" | "math" | "core" | "mem" | "time" | "physics" | "net" | "fs"
                    );
                    let lookup_key = if sym_table.functions.contains_key(&qualified_key)
                        || is_builtin_ns
                        || namespace == "self"
                    {
                        qualified_key
                    } else if sym_table.functions.contains_key(called_fn) {
                        called_fn.clone()
                    } else {
                        qualified_key
                    };

                    let mut escalated_slice_caps: Vec<CapabilitySpec> = Vec::new();
                    if let Some(callee_params) = sym_table.fn_params.get(&lookup_key) {
                        self.verify_call_inout_and_slice_aliasing(
                            func,
                            env,
                            callee_params,
                            args,
                            *span,
                            &mut escalated_slice_caps,
                        );
                    }
                    if let Some(callee_sym) = sym_table.functions.get(&lookup_key) {
                        for req_cap in &callee_sym.required_caps {
                            if escalated_slice_caps.contains(req_cap) {
                                continue;
                            }
                            if !env.satisfies_capability(req_cap) {
                                let is_interprocedural = namespace == "self"
                                    || (!is_builtin_ns
                                        && sym_table.functions.contains_key(called_fn));
                                let (code, category, ast_label, msg) = if is_interprocedural {
                                    let chain = callee_sym
                                        .cap_provenance
                                        .get(&req_cap.to_string())
                                        .cloned()
                                        .unwrap_or_else(|| called_fn.clone());
                                    (
                                        "RTS-E0403".to_string(),
                                        DiagnosticCategory::TransitiveCapabilityViolation,
                                        format!("CallExpr({})", called_fn),
                                        format!(
                                            "Transitive capability violation in '{}': calling '{}' requires capability 'cap {}' (call chain: {} -> {}), which is not granted in '{}'s signature (Caps(callee) is not a subset of Caps(caller)).",
                                            func.name, called_fn, req_cap, func.name, chain, func.name
                                        ),
                                    )
                                } else {
                                    (
                                        "RTS-E0402".to_string(),
                                        DiagnosticCategory::CapabilityViolation,
                                        format!("CallExpr({}::{})", namespace, called_fn),
                                        format!(
                                            "Function '{}' attempted to invoke '{}::{}' without declaring capability 'cap {}' in its signature.",
                                            func.name, namespace, called_fn, req_cap
                                        ),
                                    )
                                };

                                let fixes = Self::build_fixes_for_violation(
                                    func,
                                    "cap_token",
                                    req_cap,
                                    span.line_start,
                                );
                                self.diagnostics.push(DiagnosticEntry {
                                    error_code: code,
                                    category,
                                    file: self.file_path.clone(),
                                    span: *span,
                                    ast_node: ast_label,
                                    message: msg,
                                    deterministic_fixes: fixes,
                                });
                            }
                        }
                    }
                    if let Some(param_tys) = sym_table.fn_param_types.get(&lookup_key) {
                        for (idx, (arg_expr, expected_ty)) in
                            args.iter().zip(param_tys.iter()).enumerate()
                        {
                            if let Some(actual_ty) = self.infer_expr_type(env, sym_table, arg_expr)
                            {
                                let both_caps = matches!(
                                    (expected_ty, &actual_ty),
                                    (TypeExpr::Capability(_), TypeExpr::Capability(_))
                                );
                                if !both_caps && !Self::types_compatible(expected_ty, &actual_ty) {
                                    self.diagnostics.push(DiagnosticEntry {
                                        error_code: "RTS-E0301".to_string(),
                                        category: DiagnosticCategory::TypeMismatch,
                                        file: self.file_path.clone(),
                                        span: *span,
                                        ast_node: format!("CallArg({}[{}])", lookup_key, idx),
                                        message: format!(
                                            "Type mismatch in call to '{}' inside '{}': argument #{} expected '{}', but found '{}'.",
                                            lookup_key,
                                            func.name,
                                            idx + 1,
                                            expected_ty,
                                            actual_ty
                                        ),
                                        deterministic_fixes: vec![],
                                    });
                                }
                            }
                        }
                    }
                }

                for arg in args {
                    self.check_expr(func, env, sym_table, arg);
                }
            }

            Expr::MethodCall {
                receiver,
                method,
                args,
                span,
            } => {
                self.check_expr(func, env, sym_table, receiver);
                self.check_hal_method_call(func, env, sym_table, receiver, method, args, *span);

                if let Some(TypeExpr::Primitive(rec_prim)) =
                    self.infer_expr_type(env, sym_table, receiver)
                {
                    if rec_prim.fixed_str_capacity().is_some() {
                        let expected_args: Option<Vec<TypeExpr>> = match method.as_str() {
                            "clear" => Some(vec![]),
                            "push_str" => Some(vec![TypeExpr::Primitive(PrimitiveType::Str)]),
                            "push_u32" | "push_hex" => {
                                Some(vec![TypeExpr::Primitive(PrimitiveType::U32)])
                            }
                            "push_i32" => Some(vec![TypeExpr::Primitive(PrimitiveType::I32)]),
                            "push_f32" => Some(vec![
                                TypeExpr::Primitive(PrimitiveType::F32),
                                TypeExpr::Primitive(PrimitiveType::U32),
                            ]),
                            _ => None,
                        };
                        if let Some(exp_tys) = expected_args {
                            if let Some(root_ident) = Self::extract_root_receiver(receiver) {
                                if sym_table.consts.contains_key(&root_ident)
                                    || env.immutable_locals.contains(&root_ident)
                                    || !env.mutable_bindings.contains(&root_ident)
                                {
                                    self.diagnostics.push(DiagnosticEntry {
                                        error_code: "RTS-E0405".to_string(),
                                        category: DiagnosticCategory::AliasingViolation,
                                        file: self.file_path.clone(),
                                        span: *span,
                                        ast_node: format!(
                                            "FixedStringMut({}.{})",
                                            root_ident, method
                                        ),
                                        message: format!(
                                            "Cannot mutate fixed string '{}' via '.{}()' in function '{}'. Declare it with 'let mut' or pass as 'inout'.",
                                            root_ident, method, func.name
                                        ),
                                        deterministic_fixes: vec![],
                                    });
                                }
                            }
                            if args.len() != exp_tys.len() {
                                self.diagnostics.push(DiagnosticEntry {
                                    error_code: "RTS-E0301".to_string(),
                                    category: DiagnosticCategory::TypeMismatch,
                                    file: self.file_path.clone(),
                                    span: *span,
                                    ast_node: format!("FixedStringArity({})", method),
                                    message: format!(
                                        "Fixed string method '.{}()' expects {} argument(s), got {}.",
                                        method,
                                        exp_tys.len(),
                                        args.len()
                                    ),
                                    deterministic_fixes: vec![],
                                });
                            } else {
                                for (idx, (a, exp)) in args.iter().zip(exp_tys.iter()).enumerate() {
                                    if let Some(actual) = self.infer_expr_type(env, sym_table, a) {
                                        if !Self::types_compatible(exp, &actual) {
                                            self.diagnostics.push(DiagnosticEntry {
                                                error_code: "RTS-E0301".to_string(),
                                                category: DiagnosticCategory::TypeMismatch,
                                                file: self.file_path.clone(),
                                                span: *span,
                                                ast_node: format!(
                                                    "FixedStringArg({}[{}])",
                                                    method, idx
                                                ),
                                                message: format!(
                                                    "Type mismatch in call to '.{}()' inside '{}': argument #{} expected '{}', but found '{}'.",
                                                    method, func.name, idx + 1, exp, actual
                                                ),
                                                deterministic_fixes: vec![],
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if let Some(root_ident) = Self::extract_root_receiver(receiver) {
                    // Verifica RTS-E0501 (ArenaLifetimeEscape) em métodos `.push(...)`, `.store(...)`, `.insert(...)`
                    // quando uma Arena superior (`LevelArena` Rank=2 ou `StaticArena` Rank=3) recebe dados de uma Arena inferior (`FrameArena` Rank=1)
                    if let Some((dest_scope, dest_label)) = env.local_var_arena_scope.get(&root_ident)
                    {
                        if matches!(method.as_str(), "push" | "store" | "insert" | "save_handle") {
                            for arg in args {
                                if let Some((origin_scope, origin_label)) =
                                    self.infer_expr_arena_scope(env, arg)
                                {
                                    if origin_scope.rank() < dest_scope.rank() {
                                        self.diagnostics.push(DiagnosticEntry {
                                            error_code: "RTS-E0501".to_string(),
                                            category: DiagnosticCategory::ArenaLifetimeEscape,
                                            file: self.file_path.clone(),
                                            span: *span,
                                            ast_node: format!(
                                                "MethodCall({}.{})",
                                                root_ident, method
                                            ),
                                            message: format!(
                                                "Arena Lifetime Escape in '{}': cannot push/store handle from ephemeral '{}' ({}) into persistent '{}' ({}). Rank({}) = {} < Rank({}) = {}.",
                                                func.name,
                                                origin_label,
                                                origin_scope.label(),
                                                root_ident,
                                                dest_scope.label(),
                                                origin_label,
                                                origin_scope.rank(),
                                                dest_label,
                                                dest_scope.rank()
                                            ),
                                            deterministic_fixes: vec![
                                                DeterministicFix::remove_arena_escape(
                                                    span.line_start,
                                                    dest_scope.label().to_string(),
                                                ),
                                            ],
                                        });
                                    }
                                }
                            }
                        }
                    }

                    // Validação Estrita de Capacidade no Receptor de `MethodCall` (Fecha Brecha de `.metodo()` sem `cap`)
                    if let Some(req_cap) = Self::required_capability_for_method(method) {
                        let inferred_cap = match self.infer_expr_type(env, sym_table, receiver) {
                            Some(TypeExpr::Capability(c)) => Some(c),
                            _ => env.param_caps.get(&root_ident).cloned(),
                        };
                        match inferred_cap {
                            Some(cap_spec) => {
                                let close_ok = method == "close"
                                    && matches!(
                                        cap_spec.kind,
                                        CapabilityKind::WindowHost
                                            | CapabilityKind::AudioInput
                                            | CapabilityKind::AudioDevice
                                            | CapabilityKind::Network
                                    );
                                if cap_spec.kind == CapabilityKind::SqlRead
                                    && req_cap.kind == CapabilityKind::SqlWrite
                                {
                                    let table_name = match &cap_spec.generic_arg {
                                        Some(CapGenericArg::TypeName(t)) => t.clone(),
                                        _ => "Table".to_string(),
                                    };
                                    self.diagnostics.push(DiagnosticEntry {
                                        error_code: "RTS-E0402".to_string(),
                                        category: DiagnosticCategory::CapabilityViolation,
                                        file: self.file_path.clone(),
                                        span: *span,
                                        ast_node: format!("MethodCall({}.{})", root_ident, method),
                                        message: format!(
                                            "Function '{}' attempted a write operation ('{}') on resource '{}', but only possesses read capability 'cap {}'.",
                                            func.name, method, table_name, cap_spec
                                        ),
                                        deterministic_fixes: vec![
                                            DeterministicFix::remove_unauthorized_call_sealed(
                                                span.line_start,
                                            ),
                                        ],
                                    });
                                } else if !close_ok
                                    && !Self::receiver_cap_matches_method(
                                        &cap_spec.kind,
                                        &req_cap.kind,
                                    )
                                {
                                    let fixes = Self::build_fixes_for_violation(
                                        func,
                                        &root_ident,
                                        &req_cap,
                                        span.line_start,
                                    );
                                    self.diagnostics.push(DiagnosticEntry {
                                        error_code: "RTS-E0402".to_string(),
                                        category: DiagnosticCategory::CapabilityViolation,
                                        file: self.file_path.clone(),
                                        span: *span,
                                        ast_node: format!("MethodCall({}.{})", root_ident, method),
                                        message: format!(
                                            "Function '{}' invoked capability method '{}.{}()' on 'cap {}', which requires capability 'cap {}'.",
                                            func.name, root_ident, method, cap_spec, req_cap
                                        ),
                                        deterministic_fixes: fixes,
                                    });
                                }
                            }
                            None => {
                                let fixes = Self::build_fixes_for_violation(
                                    func,
                                    &root_ident,
                                    &req_cap,
                                    span.line_start,
                                );
                                self.diagnostics.push(DiagnosticEntry {
                                    error_code: "RTS-E0402".to_string(),
                                    category: DiagnosticCategory::CapabilityViolation,
                                    file: self.file_path.clone(),
                                    span: *span,
                                    ast_node: format!("MethodCall({}.{})", root_ident, method),
                                    message: format!(
                                        "Function '{}' invoked privileged capability method '{}.{}()' on non-capability receiver '{}'. Hardware, Arena, Network, and SQL methods require an explicit 'cap {}' token parameter.",
                                        func.name, root_ident, method, root_ident, req_cap
                                    ),
                                    deterministic_fixes: fixes,
                                });
                            }
                        }
                    }
                }

                if let Some(callee_sym) = sym_table.functions.get(method) {
                    for req_cap in &callee_sym.required_caps {
                        if !env.satisfies_capability(req_cap) {
                            let chain = callee_sym
                                .cap_provenance
                                .get(&req_cap.to_string())
                                .cloned()
                                .unwrap_or_else(|| method.clone());
                            let fixes = Self::build_fixes_for_violation(
                                func,
                                "cap_token",
                                req_cap,
                                span.line_start,
                            );
                            self.diagnostics.push(DiagnosticEntry {
                                error_code: "RTS-E0403".to_string(),
                                category: DiagnosticCategory::TransitiveCapabilityViolation,
                                file: self.file_path.clone(),
                                span: *span,
                                ast_node: format!("MethodCall({})", method),
                                message: format!(
                                    "Transitive capability violation in '{}': calling method '{}' requires capability 'cap {}' (call chain: {} -> {}), which is not granted in '{}'s signature.",
                                    func.name, method, req_cap, func.name, chain, func.name
                                ),
                                deterministic_fixes: fixes,
                            });
                        }
                    }
                }
                if let Some(param_tys) = sym_table.fn_param_types.get(method) {
                    for (idx, (arg_expr, expected_ty)) in
                        args.iter().zip(param_tys.iter().skip(1)).enumerate()
                    {
                        if let Some(actual_ty) = self.infer_expr_type(env, sym_table, arg_expr) {
                            if !Self::types_compatible(expected_ty, &actual_ty) {
                                self.diagnostics.push(DiagnosticEntry {
                                    error_code: "RTS-E0301".to_string(),
                                    category: DiagnosticCategory::TypeMismatch,
                                    file: self.file_path.clone(),
                                    span: *span,
                                    ast_node: format!("MethodArg({}[{}])", method, idx),
                                    message: format!(
                                        "Type mismatch in method call '{}' inside '{}': argument #{} expected '{}', but found '{}'.",
                                        method, func.name, idx + 1, expected_ty, actual_ty
                                    ),
                                    deterministic_fixes: vec![],
                                });
                            }
                        }
                    }
                }

                for arg in args {
                    self.check_expr(func, env, sym_table, arg);
                }
            }

            Expr::FieldAccess { receiver, .. } => {
                self.check_expr(func, env, sym_table, receiver);
            }
            Expr::Index {
                receiver, index, ..
            } => {
                self.check_expr(func, env, sym_table, receiver);
                self.check_expr(func, env, sym_table, index);
            }
            Expr::EnumConstructor { payload, .. } => {
                if let Some(inner) = payload {
                    self.check_expr(func, env, sym_table, inner);
                }
            }
            Expr::Binary {
                left,
                op,
                right,
                span,
            } => {
                self.check_expr(func, env, sym_table, left);
                self.check_expr(func, env, sym_table, right);
                if let (Some(lt), Some(rt)) = (
                    self.infer_expr_type(env, sym_table, left),
                    self.infer_expr_type(env, sym_table, right),
                ) {
                    if !Self::binary_operands_valid(*op, &lt, &rt) {
                        self.diagnostics.push(DiagnosticEntry {
                            error_code: "RTS-E0301".to_string(),
                            category: DiagnosticCategory::TypeMismatch,
                            file: self.file_path.clone(),
                            span: *span,
                            ast_node: format!("BinaryExpr({} {:?} {})", lt, op, rt),
                            message: format!(
                                "Invalid binary operation in '{}': cannot apply operator '{:?}' between '{}' and '{}'.",
                                func.name, op, lt, rt
                            ),
                            deterministic_fixes: vec![],
                        });
                    }
                }
            }
            Expr::Unary {
                op: UnaryOp::BitNot,
                expr: inner,
                span,
            } => {
                self.check_expr(func, env, sym_table, inner);
                if let Some(in_ty) = self.infer_expr_type(env, sym_table, inner) {
                    let is_int = matches!(&in_ty, TypeExpr::Primitive(p) if Self::is_integer_primitive(p));
                    if !is_int {
                        self.diagnostics.push(DiagnosticEntry {
                            error_code: "RTS-E0301".to_string(),
                            category: DiagnosticCategory::TypeMismatch,
                            file: self.file_path.clone(),
                            span: *span,
                            ast_node: format!("UnaryExpr(~{})", in_ty),
                            message: format!(
                                "Bitwise NOT '~' in '{}' requires an integer scalar operand, found '{}'.",
                                func.name, in_ty
                            ),
                            deterministic_fixes: vec![],
                        });
                    }
                }
            }
            Expr::Unary { expr: inner, .. }
            | Expr::Cast { expr: inner, .. }
            | Expr::TryOp { expr: inner, .. }
            | Expr::ErrorMapClosure { body: inner, .. } => {
                self.check_expr(func, env, sym_table, inner);
            }
            Expr::IfExpr {
                cond,
                then_branch,
                else_branch,
                span,
            } => {
                self.check_expr(func, env, sym_table, cond);
                self.check_expr(func, env, sym_table, then_branch);
                self.check_expr(func, env, sym_table, else_branch);
                if let Some(cond_ty) = self.infer_expr_type(env, sym_table, cond) {
                    if !Self::types_compatible(&TypeExpr::Primitive(PrimitiveType::Bool), &cond_ty)
                    {
                        self.diagnostics.push(DiagnosticEntry {
                            error_code: "RTS-E0301".to_string(),
                            category: DiagnosticCategory::TypeMismatch,
                            file: self.file_path.clone(),
                            span: *span,
                            ast_node: format!("IfExprCond({})", cond_ty),
                            message: format!(
                                "Inline 'if' expression condition in '{}' must be 'bool', found '{}'.",
                                func.name, cond_ty
                            ),
                            deterministic_fixes: vec![],
                        });
                    }
                }
                if let (Some(tt), Some(et)) = (
                    self.infer_expr_type(env, sym_table, then_branch),
                    self.infer_expr_type(env, sym_table, else_branch),
                ) {
                    if !Self::types_compatible(&tt, &et) {
                        self.diagnostics.push(DiagnosticEntry {
                            error_code: "RTS-E0301".to_string(),
                            category: DiagnosticCategory::TypeMismatch,
                            file: self.file_path.clone(),
                            span: *span,
                            ast_node: format!("IfExpr({} vs {})", tt, et),
                            message: format!(
                                "Inline 'if' expression branch types mismatch in '{}': 'then' produces '{}', but 'else' produces '{}'.",
                                func.name, tt, et
                            ),
                            deterministic_fixes: vec![],
                        });
                    }
                }
            }
            Expr::ArrayLit { elements, span } => {
                for el in elements {
                    self.check_expr(func, env, sym_table, el);
                }
                if elements.len() > 1 {
                    if let Some(first_ty) = self.infer_expr_type(env, sym_table, &elements[0]) {
                        for el in &elements[1..] {
                            if let Some(el_ty) = self.infer_expr_type(env, sym_table, el) {
                                if !Self::types_compatible(&first_ty, &el_ty) {
                                    self.diagnostics.push(DiagnosticEntry {
                                        error_code: "RTS-E0301".to_string(),
                                        category: DiagnosticCategory::TypeMismatch,
                                        file: self.file_path.clone(),
                                        span: *span,
                                        ast_node: format!("ArrayLit({} vs {})", first_ty, el_ty),
                                        message: format!(
                                            "Homogeneous array literal elements required in '{}': expected '{}', found '{}'.",
                                            func.name, first_ty, el_ty
                                        ),
                                        deterministic_fixes: vec![],
                                    });
                                }
                            }
                        }
                    }
                }
            }
            Expr::IntLit(_, _)
            | Expr::FloatLit(_, _)
            | Expr::BoolLit(_, _)
            | Expr::StringLit(_, _)
            | Expr::Ident(_, _) => {}
        }
    }

    fn is_integer_primitive(p: &PrimitiveType) -> bool {
        matches!(
            p,
            PrimitiveType::I8
                | PrimitiveType::I16
                | PrimitiveType::I32
                | PrimitiveType::I64
                | PrimitiveType::U8
                | PrimitiveType::U16
                | PrimitiveType::U32
                | PrimitiveType::U64
                | PrimitiveType::Usize
                | PrimitiveType::Fix32
        )
    }

    fn is_float_primitive(p: &PrimitiveType) -> bool {
        matches!(p, PrimitiveType::F32 | PrimitiveType::F64)
    }

    fn is_numeric_type(ty: &TypeExpr) -> bool {
        match ty {
            TypeExpr::Primitive(p) => Self::is_integer_primitive(p) || Self::is_float_primitive(p),
            TypeExpr::Math(_) => true,
            _ => false,
        }
    }

    pub fn types_compatible(expected: &TypeExpr, actual: &TypeExpr) -> bool {
        if expected == actual {
            return true;
        }
        match (expected, actual) {
            (TypeExpr::Named { name: n1, .. }, _) if n1 == "_Any" => true,
            (_, TypeExpr::Named { name: n2, .. }) if n2 == "_Any" => true,
            (TypeExpr::Primitive(p1), TypeExpr::Primitive(p2)) => {
                (Self::is_integer_primitive(p1) && Self::is_integer_primitive(p2))
                    || (Self::is_float_primitive(p1) && Self::is_float_primitive(p2))
            }
            (TypeExpr::Handle(a), TypeExpr::Handle(b)) => {
                let is_wildcard = |t: &TypeExpr| {
                    matches!(
                        t,
                        TypeExpr::Named { name, .. }
                            if name == "Entity" || name == "Component" || name == "_Any"
                    )
                };
                is_wildcard(a) || is_wildcard(b) || Self::types_compatible(a, b)
            }
            (TypeExpr::Named { name, generics }, other)
                if name == "Json" && generics.len() == 1 =>
            {
                Self::types_compatible(&generics[0], other)
            }
            (other, TypeExpr::Named { name, generics })
                if name == "Json" && generics.len() == 1 =>
            {
                Self::types_compatible(other, &generics[0])
            }
            (TypeExpr::Array(a, n1), TypeExpr::Array(b, n2)) => {
                n1 == n2 && Self::types_compatible(a, b)
            }
            (TypeExpr::Option(a), TypeExpr::Option(b)) => Self::types_compatible(a, b),
            (TypeExpr::Result(ok1, err1), TypeExpr::Result(ok2, err2)) => {
                Self::types_compatible(ok1, ok2) && Self::types_compatible(err1, err2)
            }
            (TypeExpr::Capability(c1), TypeExpr::Capability(c2)) => {
                let kinds_ok = c1.kind == c2.kind
                    || (c1.kind == CapabilityKind::ReadSlice
                        && c2.kind == CapabilityKind::MutSlice);
                kinds_ok
                    && (c1.generic_arg == c2.generic_arg
                        || c1.generic_arg.is_none()
                        || c2.generic_arg.is_none())
            }
            _ => false,
        }
    }

    fn binary_operands_valid(op: BinaryOp, lt: &TypeExpr, rt: &TypeExpr) -> bool {
        match op {
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div => {
                if matches!(
                    (lt, rt),
                    (
                        TypeExpr::Math(_),
                        TypeExpr::Primitive(PrimitiveType::F32 | PrimitiveType::F64)
                    ) | (
                        TypeExpr::Primitive(PrimitiveType::F32 | PrimitiveType::F64),
                        TypeExpr::Math(_)
                    )
                ) {
                    return true;
                }
                Self::is_numeric_type(lt)
                    && Self::is_numeric_type(rt)
                    && Self::types_compatible(lt, rt)
            }
            BinaryOp::Rem => {
                Self::is_numeric_type(lt)
                    && Self::is_numeric_type(rt)
                    && Self::types_compatible(lt, rt)
            }
            BinaryOp::BitAnd
            | BinaryOp::BitOr
            | BinaryOp::BitXor
            | BinaryOp::Shl
            | BinaryOp::Shr => {
                let l_int = matches!(lt, TypeExpr::Primitive(p) if Self::is_integer_primitive(p));
                let r_int = matches!(rt, TypeExpr::Primitive(p) if Self::is_integer_primitive(p));
                l_int && r_int && Self::types_compatible(lt, rt)
            }
            BinaryOp::And | BinaryOp::Or => {
                matches!(
                    (lt, rt),
                    (
                        TypeExpr::Primitive(PrimitiveType::Bool),
                        TypeExpr::Primitive(PrimitiveType::Bool)
                    )
                )
            }
            BinaryOp::Eq
            | BinaryOp::NotEq
            | BinaryOp::Lt
            | BinaryOp::LtEq
            | BinaryOp::Gt
            | BinaryOp::GtEq => Self::types_compatible(lt, rt),
        }
    }

    pub fn infer_expr_type(
        &self,
        env: &FunctionCapabilityEnv,
        sym_table: &SymbolTable,
        expr: &Expr,
    ) -> Option<TypeExpr> {
        match expr {
            Expr::IntLit(_, _) => Some(TypeExpr::Primitive(PrimitiveType::U32)),
            Expr::FloatLit(_, _) => Some(TypeExpr::Primitive(PrimitiveType::F32)),
            Expr::BoolLit(_, _) => Some(TypeExpr::Primitive(PrimitiveType::Bool)),
            Expr::StringLit(_, _) => Some(TypeExpr::Primitive(PrimitiveType::Str)),
            Expr::Ident(name, _) => env
                .local_var_types
                .get(name)
                .cloned()
                .or_else(|| sym_table.consts.get(name).cloned()),
            Expr::Cast { target_type, .. } => Some(target_type.clone()),
            Expr::Unary { op, expr: inner, .. } => match op {
                UnaryOp::Not => Some(TypeExpr::Primitive(PrimitiveType::Bool)),
                UnaryOp::BitNot => self.infer_expr_type(env, sym_table, inner),
                UnaryOp::Neg => {
                    if matches!(inner.as_ref(), Expr::IntLit(_, _)) {
                        Some(TypeExpr::Primitive(PrimitiveType::I32))
                    } else {
                        self.infer_expr_type(env, sym_table, inner)
                    }
                }
            },
            Expr::TryOp { expr: inner, .. } => {
                let core = match inner.as_ref() {
                    Expr::MethodCall {
                        receiver, method, ..
                    } if method == "map_err" => receiver.as_ref(),
                    other => other,
                };
                match self.infer_expr_type(env, sym_table, core) {
                    Some(TypeExpr::Result(ok_ty, _)) => Some(*ok_ty),
                    other => other,
                }
            }
            Expr::Index { receiver, .. } => {
                match self.infer_expr_type(env, sym_table, receiver) {
                    Some(TypeExpr::Array(inner, _)) => return Some(*inner),
                    Some(TypeExpr::Capability(cap)) => {
                        if let Some(elem_ty) = cap.slice_element_type() {
                            return Some(elem_ty);
                        }
                    }
                    _ => {}
                }
                if let Some(root) = Self::extract_root_receiver(receiver) {
                    if let Some(cap) = env.param_caps.get(&root) {
                        if let Some(elem_ty) = cap.slice_element_type() {
                            return Some(elem_ty);
                        }
                    }
                }
                None
            }
            Expr::FieldAccess {
                receiver, field, ..
            } => {
                if field == "fixed_delta" {
                    return Some(TypeExpr::Primitive(PrimitiveType::F32));
                }
                if field == "index" || field == "generation" {
                    return Some(TypeExpr::Primitive(PrimitiveType::U32));
                }
                if let Some(rec_ty) = self.infer_expr_type(env, sym_table, receiver) {
                    match rec_ty {
                        TypeExpr::Named { name, .. } => {
                            if let Some(fields) = sym_table.structs.get(&name) {
                                return fields.get(field).cloned();
                            }
                        }
                        TypeExpr::Math(
                            MathType::Vec2 | MathType::Vec3 | MathType::Vec4 | MathType::Quat,
                        ) if matches!(field.as_str(), "x" | "y" | "z" | "w") => {
                            return Some(TypeExpr::Primitive(PrimitiveType::F32));
                        }
                        _ => {}
                    }
                }
                None
            }
            Expr::MethodCall {
                receiver, method, ..
            } => {
                if let Some(TypeExpr::Primitive(rec_prim)) =
                    self.infer_expr_type(env, sym_table, receiver)
                {
                    if rec_prim.fixed_str_capacity().is_some() {
                        match method.as_str() {
                            "len" | "capacity" => {
                                return Some(TypeExpr::Primitive(PrimitiveType::Usize));
                            }
                            "as_str" => {
                                return Some(TypeExpr::Primitive(PrimitiveType::Str));
                            }
                            "clear" | "push_str" | "push_u32" | "push_i32" | "push_hex"
                            | "push_f32" => return None,
                            _ => {}
                        }
                    }
                }
                if let Some(TypeExpr::Capability(cap)) =
                    self.infer_expr_type(env, sym_table, receiver)
                {
                    if let Some(hal_ret) = Self::hal_method_return_type(&cap.kind, method) {
                        return hal_ret;
                    }
                }
                match method.as_str() {
                "len" => Some(TypeExpr::Primitive(PrimitiveType::Usize)),
                "handle_at" | "alloc_handle" | "push_handle" => Some(TypeExpr::Handle(Box::new(
                    TypeExpr::Named {
                        name: "Entity".to_string(),
                        generics: vec![],
                    },
                ))),
                "read_register_u16" => Some(TypeExpr::Result(
                    Box::new(TypeExpr::Primitive(PrimitiveType::U16)),
                    Box::new(TypeExpr::Primitive(PrimitiveType::U8)),
                )),
                "read_register_u8" => Some(TypeExpr::Result(
                    Box::new(TypeExpr::Primitive(PrimitiveType::U8)),
                    Box::new(TypeExpr::Primitive(PrimitiveType::U8)),
                )),
                    _ => sym_table.fn_return_types.get(method).cloned().flatten(),
                }
            }
            Expr::StaticCall {
                namespace,
                func: called_fn,
                args,
                ..
            } => {
                if Self::is_unforgeable_capability_domain(namespace) && called_fn == "new" {
                    let generic_arg = args.first().and_then(|a| match a {
                        Expr::IntLit(v, _) => Some(CapGenericArg::HardwareAddress(*v)),
                        Expr::Ident(tname, _) => Some(CapGenericArg::TypeName(tname.clone())),
                        _ => None,
                    });
                    return Some(TypeExpr::Capability(CapabilitySpec::from_name_and_arg(
                        namespace,
                        generic_arg,
                    )));
                }
                if sym_table.enums.contains_key(namespace) {
                    return Some(TypeExpr::Named {
                        name: namespace.clone(),
                        generics: vec![],
                    });
                }
                if namespace == "Handle" && called_fn == "null" {
                    return Some(TypeExpr::Handle(Box::new(TypeExpr::Named {
                        name: "Entity".to_string(),
                        generics: vec![],
                    })));
                }
                if namespace == "self" {
                    match called_fn.as_str() {
                        "vec2" => return Some(TypeExpr::Math(MathType::Vec2)),
                        "vec3" => return Some(TypeExpr::Math(MathType::Vec3)),
                        "vec4" => return Some(TypeExpr::Math(MathType::Vec4)),
                        "quat" => return Some(TypeExpr::Math(MathType::Quat)),
                        _ => {}
                    }
                }
                if namespace == "mem" || namespace == "core.mem" {
                    return match called_fn.as_str() {
                        "addr_of" | "slice_ptr" | "alloc_aligned" => {
                            Some(TypeExpr::Primitive(PrimitiveType::U64))
                        }
                        "f32_from_bits" => Some(TypeExpr::Primitive(PrimitiveType::F32)),
                        "f32_to_bits" => Some(TypeExpr::Primitive(PrimitiveType::U32)),
                        "str_len" => Some(TypeExpr::Primitive(PrimitiveType::Usize)),
                        "str_byte" => Some(TypeExpr::Primitive(PrimitiveType::U8)),
                        _ => None,
                    };
                }
                let qualified_key = if namespace == "self" {
                    called_fn.clone()
                } else {
                    format!("{}::{}", namespace, called_fn)
                };
                let is_builtin_ns = matches!(
                    namespace.as_str(),
                    "io" | "math" | "core" | "mem" | "time" | "physics" | "net" | "fs"
                );
                let lookup_key = if sym_table.fn_return_types.contains_key(&qualified_key)
                    || is_builtin_ns
                    || namespace == "self"
                {
                    qualified_key
                } else if sym_table.fn_return_types.contains_key(called_fn) {
                    called_fn.clone()
                } else {
                    qualified_key
                };
                sym_table.fn_return_types.get(&lookup_key).cloned().flatten()
            }
            Expr::StructInit { struct_name, .. } => Some(TypeExpr::Named {
                name: struct_name.clone(),
                generics: vec![],
            }),
            Expr::EnumConstructor {
                enum_name,
                variant,
                payload,
                ..
            } => {
                if let Some(en) = enum_name {
                    if payload.is_none() && !sym_table.enums.contains_key(en) {
                        let qualified_const = format!("{}__{}", en, variant);
                        if let Some(c_ty) = sym_table
                            .consts
                            .get(&qualified_const)
                            .or_else(|| sym_table.consts.get(variant))
                        {
                            return Some(c_ty.clone());
                        }
                    }
                    return Some(TypeExpr::Named {
                        name: en.clone(),
                        generics: vec![],
                    });
                }
                let any_ty = TypeExpr::Named {
                    name: "_Any".to_string(),
                    generics: vec![],
                };
                match variant.as_str() {
                    "Ok" => {
                        let ok_ty = payload
                            .as_ref()
                            .and_then(|p| self.infer_expr_type(env, sym_table, p))
                            .unwrap_or_else(|| any_ty.clone());
                        Some(TypeExpr::Result(Box::new(ok_ty), Box::new(any_ty)))
                    }
                    "Err" => {
                        let err_ty = payload
                            .as_ref()
                            .and_then(|p| self.infer_expr_type(env, sym_table, p))
                            .unwrap_or_else(|| any_ty.clone());
                        Some(TypeExpr::Result(Box::new(any_ty), Box::new(err_ty)))
                    }
                    "Some" => {
                        let inner_ty = payload
                            .as_ref()
                            .and_then(|p| self.infer_expr_type(env, sym_table, p))
                            .unwrap_or(any_ty);
                        Some(TypeExpr::Option(Box::new(inner_ty)))
                    }
                    "None" => Some(TypeExpr::Option(Box::new(any_ty))),
                    _ => None,
                }
            }
            Expr::Binary {
                left, op, right, ..
            } => match op {
                BinaryOp::Eq
                | BinaryOp::NotEq
                | BinaryOp::Lt
                | BinaryOp::LtEq
                | BinaryOp::Gt
                | BinaryOp::GtEq
                | BinaryOp::And
                | BinaryOp::Or => Some(TypeExpr::Primitive(PrimitiveType::Bool)),
                BinaryOp::Add
                | BinaryOp::Sub
                | BinaryOp::Mul
                | BinaryOp::Div
                | BinaryOp::Rem
                | BinaryOp::BitAnd
                | BinaryOp::BitOr
                | BinaryOp::BitXor
                | BinaryOp::Shl
                | BinaryOp::Shr => {
                    let lt = self.infer_expr_type(env, sym_table, left);
                    let rt = self.infer_expr_type(env, sym_table, right);
                    match (&lt, &rt) {
                        (Some(TypeExpr::Math(m)), _) | (_, Some(TypeExpr::Math(m))) => {
                            Some(TypeExpr::Math(m.clone()))
                        }
                        _ => lt.or(rt),
                    }
                }
            },
            Expr::IfExpr {
                then_branch,
                else_branch,
                ..
            } => self
                .infer_expr_type(env, sym_table, then_branch)
                .or_else(|| self.infer_expr_type(env, sym_table, else_branch)),
            Expr::ArrayLit { elements, .. } => {
                let elem_ty = elements
                    .first()
                    .and_then(|e| self.infer_expr_type(env, sym_table, e))
                    .unwrap_or(TypeExpr::Primitive(PrimitiveType::F32));
                Some(TypeExpr::Array(Box::new(elem_ty), elements.len()))
            }
            _ => None,
        }
    }

    fn required_capability_for_method(method: &str) -> Option<CapabilitySpec> {
        match method {
            "read_register_u16" | "read_register_u8" | "write_register_u16"
            | "write_register_u8" => Some(CapabilitySpec::from_name_and_arg(
                "I2CBus",
                Some(CapGenericArg::HardwareAddress(0x48)),
            )),
            "set_duty" | "set_frequency" => Some(CapabilitySpec::from_name_and_arg(
                "PWMPin",
                Some(CapGenericArg::HardwareAddress(4)),
            )),
            "push" | "alloc" | "alloc_handle" | "push_handle" | "spawn_temp" | "alloc_slice" => {
                Some(CapabilitySpec::from_name_and_arg(
                    "FrameArena",
                    Some(CapGenericArg::TypeName("Event".to_string())),
                ))
            }
            "used_bytes" | "capacity_bytes" => Some(CapabilitySpec::from_name_and_arg(
                "ReadFrameArena",
                Some(CapGenericArg::TypeName("Event".to_string())),
            )),
            "fill" => Some(CapabilitySpec::from_name_and_arg(
                "MutSlice",
                Some(CapGenericArg::TypeName("T".to_string())),
            )),
            "send" | "recv" | "broadcast" | "connect" | "udp_bind" | "udp_send_to"
            | "udp_recv_from" | "tcp_connect" | "tcp_listen" | "tcp_accept" | "tcp_send"
            | "tcp_recv" | "close_socket" => Some(CapabilitySpec::from_name_and_arg("NetSocket", None)),
            "query" | "select" | "find_by_id" | "fetch_one" => {
                Some(CapabilitySpec::from_name_and_arg(
                    "SqlRead",
                    Some(CapGenericArg::TypeName("Table".to_string())),
                ))
            }
            "delete_by_id" | "delete" | "insert" | "update" | "drop" | "execute_write" => {
                Some(CapabilitySpec::from_name_and_arg(
                    "SqlWrite",
                    Some(CapGenericArg::TypeName("Table".to_string())),
                ))
            }
            "handle_at" => Some(CapabilitySpec::from_name_and_arg(
                "ReadColumn",
                Some(CapGenericArg::TypeName("Component".to_string())),
            )),
            "open_fullscreen"
            | "open_windowed"
            | "open_headless"
            | "set_fullscreen"
            | "set_cursor_locked"
            | "set_cursor_visible"
            | "width"
            | "height"
            | "poll_events"
            | "present_rgba"
            | "draw_text"
            | "draw_rect"
            | "draw_text_scaled"
            | "draw_blit_rgba"
            | "close"
            | "is_smoke_test" => Some(CapabilitySpec::from_name_and_arg("WindowHost", None)),
            "poll"
            | "is_key_down"
            | "was_key_pressed"
            | "mouse_x"
            | "mouse_y"
            | "mouse_dx"
            | "mouse_dy"
            | "mouse_wheel_delta"
            | "mouse_left_down"
            | "mouse_clicked"
            | "mouse_right_down"
            | "mouse_right_clicked"
            | "gamepad_connected"
            | "gamepad_move_x"
            | "gamepad_move_y"
            | "gamepad_aim_x"
            | "gamepad_aim_y"
            | "gamepad_aim_active"
            | "gamepad_fire_down"
            | "was_gamepad_pressed"
            | "set_vibration" => Some(CapabilitySpec::from_name_and_arg("InputDevice", None)),
            "create_dir"
            | "write_text"
            | "write_struct"
            | "read_struct"
            | "write_raw"
            | "read_raw"
            | "exists"
            | "file_size"
            | "read_text"
            | "read_raw_slice"
            | "write_raw_slice"
            | "list_dir"
            | "load_image_rgba" => Some(CapabilitySpec::from_name_and_arg("FileSystem", None)),
            "write_synth_wav"
            | "play_wav_async"
            | "mixer_init"
            | "mixer_load_wav"
            | "mixer_play"
            | "mixer_set_channel"
            | "mixer_stop_channel"
            | "mixer_mix_frame" => Some(CapabilitySpec::from_name_and_arg("AudioDevice", None)),
            "open" | "set_muted" | "is_muted" | "read_pcm" => {
                Some(CapabilitySpec::from_name_and_arg("AudioInput", None))
            }
            "tick_frame_fps" | "last_frame_ms" => {
                Some(CapabilitySpec::from_name_and_arg("MonotonicClock", None))
            }
            _ => None,
        }
    }

    fn receiver_cap_matches_method(
        receiver_kind: &CapabilityKind,
        required_kind: &CapabilityKind,
    ) -> bool {
        match (receiver_kind, required_kind) {
            (CapabilityKind::ScopedArena(_), CapabilityKind::ScopedArena(_)) => true,
            (CapabilityKind::ScopedArena(_), CapabilityKind::ReadScopedArena(_)) => true,
            (CapabilityKind::ReadScopedArena(_), CapabilityKind::ReadScopedArena(_)) => true,
            (CapabilityKind::SqlWrite, CapabilityKind::SqlRead) => true,
            (CapabilityKind::MutColumn, CapabilityKind::ReadColumn) => true,
            (CapabilityKind::MutSlice, CapabilityKind::ReadSlice) => true,
            (a, b) => a == b,
        }
    }

    fn verify_call_inout_and_slice_aliasing(
        &mut self,
        caller: &FnDecl,
        env: &FunctionCapabilityEnv,
        callee_params: &[FnParam],
        call_args: &[Expr],
        span: Span,
        escalated_slice_caps: &mut Vec<CapabilitySpec>,
    ) {
        let mut all_roots: Vec<(String, bool)> = Vec::new();

        for (param, arg) in callee_params.iter().zip(call_args.iter()) {
            let is_mut_slice = param.is_cap
                && matches!(
                    param.capability.as_ref().map(|c| &c.kind),
                    Some(CapabilityKind::MutSlice | CapabilityKind::MutColumn)
                );
            let is_read_slice = param.is_cap
                && matches!(
                    param.capability.as_ref().map(|c| &c.kind),
                    Some(CapabilityKind::ReadSlice | CapabilityKind::ReadColumn)
                );

            if param.is_inout {
                if let Some(root) = Self::extract_root_receiver(arg) {
                    if !env.mutable_bindings.contains(&root) {
                        self.diagnostics.push(DiagnosticEntry {
                            error_code: "RTS-E0405".to_string(),
                            category: DiagnosticCategory::AliasingViolation,
                            file: self.file_path.clone(),
                            span,
                            ast_node: format!("InoutArg({}: {})", param.name, root),
                            message: format!(
                                "Cannot pass immutable binding '{}' to 'inout' parameter '{}' inside '{}'. Declare it as 'let mut' or 'inout'.",
                                root, param.name, caller.name
                            ),
                            deterministic_fixes: vec![],
                        });
                    }
                    all_roots.push((root, true));
                } else {
                    self.diagnostics.push(DiagnosticEntry {
                        error_code: "RTS-E0405".to_string(),
                        category: DiagnosticCategory::AliasingViolation,
                        file: self.file_path.clone(),
                        span,
                        ast_node: format!("InoutArg({})", param.name),
                        message: format!(
                            "Argument for 'inout' parameter '{}' inside '{}' must be a mutable variable ('let mut' or 'inout').",
                            param.name, caller.name
                        ),
                        deterministic_fixes: vec![],
                    });
                }
            } else if param.is_cap {
                if let (Some(req_cap), Some(root)) =
                    (&param.capability, Self::extract_root_receiver(arg))
                {
                    if let Some(arg_cap) = env.param_caps.get(&root) {
                        if is_mut_slice
                            && matches!(
                                arg_cap.kind,
                                CapabilityKind::ReadSlice | CapabilityKind::ReadColumn
                            )
                        {
                            escalated_slice_caps.push(req_cap.clone());
                            self.diagnostics.push(DiagnosticEntry {
                                error_code: "RTS-E0402".to_string(),
                                category: DiagnosticCategory::CapabilityViolation,
                                file: self.file_path.clone(),
                                span,
                                ast_node: format!("SliceEscalation({} -> {})", root, param.name),
                                message: format!(
                                    "Cannot escalate read-only capability '{}' to mutable capability for parameter '{}' inside '{}'.",
                                    root, param.name, caller.name
                                ),
                                deterministic_fixes: vec![
                                    DeterministicFix::remove_unauthorized_call_sealed(span.line_start),
                                ],
                            });
                        } else if !Self::types_compatible(
                            &TypeExpr::Capability(req_cap.clone()),
                            &TypeExpr::Capability(arg_cap.clone()),
                        ) {
                            escalated_slice_caps.push(req_cap.clone());
                            self.diagnostics.push(DiagnosticEntry {
                                error_code: "RTS-E0402".to_string(),
                                category: DiagnosticCategory::CapabilityViolation,
                                file: self.file_path.clone(),
                                span,
                                ast_node: format!(
                                    "CapArgMismatch({}: cap {} -> cap {})",
                                    param.name, arg_cap, req_cap
                                ),
                                message: format!(
                                    "Capability mismatch in '{}': argument '{}' has 'cap {}', but parameter '{}' requires 'cap {}'.",
                                    caller.name, root, arg_cap, param.name, req_cap
                                ),
                                deterministic_fixes: Self::build_fixes_for_violation(
                                    caller,
                                    &param.name,
                                    req_cap,
                                    span.line_start,
                                ),
                            });
                        }
                    }
                    if is_mut_slice || is_read_slice {
                        all_roots.push((root, is_mut_slice));
                    }
                }
            }
        }

        for i in 0..all_roots.len() {
            for j in (i + 1)..all_roots.len() {
                if all_roots[i].0 == all_roots[j].0 && (all_roots[i].1 || all_roots[j].1) {
                    self.diagnostics.push(DiagnosticEntry {
                        error_code: "RTS-E0405".to_string(),
                        category: DiagnosticCategory::AliasingViolation,
                        file: self.file_path.clone(),
                        span,
                        ast_node: format!("AliasingCall({})", all_roots[i].0),
                        message: format!(
                            "Aliasing violation in '{}': variable '{}' cannot be borrowed more than once when mutated in the same call.",
                            caller.name, all_roots[i].0
                        ),
                        deterministic_fixes: vec![],
                    });
                    return;
                }
            }
        }
    }

    fn hal_method_return_type(
        kind: &CapabilityKind,
        method: &str,
    ) -> Option<Option<TypeExpr>> {
        let b = Some(Some(TypeExpr::Primitive(PrimitiveType::Bool)));
        let i = Some(Some(TypeExpr::Primitive(PrimitiveType::I32)));
        let u64_t = Some(Some(TypeExpr::Primitive(PrimitiveType::U64)));
        let usize_t = Some(Some(TypeExpr::Primitive(PrimitiveType::Usize)));
        let f = Some(Some(TypeExpr::Primitive(PrimitiveType::F32)));
        let v = Some(None);
        match (kind, method) {
            (
                CapabilityKind::ScopedArena(_) | CapabilityKind::ReadScopedArena(_),
                "used_bytes" | "capacity_bytes",
            ) => usize_t,
            (
                CapabilityKind::WindowHost,
                "open_fullscreen"
                | "open_windowed"
                | "open_headless"
                | "poll_events"
                | "is_smoke_test",
            ) => b,
            (CapabilityKind::WindowHost, "width" | "height") => i,
            (
                CapabilityKind::WindowHost,
                "set_fullscreen"
                | "set_cursor_locked"
                | "set_cursor_visible"
                | "present_rgba"
                | "draw_text"
                | "draw_rect"
                | "draw_text_scaled"
                | "draw_blit_rgba"
                | "close",
            ) => v,
            (
                CapabilityKind::InputDevice,
                "is_key_down"
                | "was_key_pressed"
                | "mouse_left_down"
                | "mouse_clicked"
                | "mouse_right_down"
                | "mouse_right_clicked"
                | "gamepad_connected"
                | "gamepad_aim_active"
                | "gamepad_fire_down"
                | "was_gamepad_pressed",
            ) => b,
            (
                CapabilityKind::InputDevice,
                "mouse_x" | "mouse_y" | "mouse_dx" | "mouse_dy" | "mouse_wheel_delta",
            ) => i,
            (
                CapabilityKind::InputDevice,
                "gamepad_move_x" | "gamepad_move_y" | "gamepad_aim_x" | "gamepad_aim_y",
            ) => f,
            (CapabilityKind::InputDevice, "poll" | "set_vibration") => v,
            (
                CapabilityKind::FileSystem,
                "create_dir"
                | "write_text"
                | "write_struct"
                | "read_struct"
                | "write_raw"
                | "exists"
                | "write_raw_slice"
                | "load_image_rgba",
            ) => b,
            (CapabilityKind::FileSystem, "read_raw" | "file_size") => u64_t,
            (CapabilityKind::FileSystem, "read_text" | "read_raw_slice" | "list_dir") => usize_t,
            (
                CapabilityKind::AudioDevice,
                "write_synth_wav" | "mixer_init" | "mixer_load_wav",
            ) => b,
            (CapabilityKind::AudioDevice, "mixer_play") => i,
            (CapabilityKind::AudioDevice, "mixer_mix_frame") => usize_t,
            (
                CapabilityKind::AudioDevice,
                "play_wav_async" | "mixer_set_channel" | "mixer_stop_channel",
            ) => v,
            (CapabilityKind::AudioInput, "open" | "is_muted") => b,
            (CapabilityKind::AudioInput, "read_pcm") => usize_t,
            (CapabilityKind::AudioInput, "set_muted" | "close") => v,
            (
                CapabilityKind::Network,
                "udp_bind"
                | "udp_send_to"
                | "udp_recv_from"
                | "tcp_connect"
                | "tcp_listen"
                | "tcp_accept"
                | "tcp_send"
                | "tcp_recv",
            ) => i,
            (CapabilityKind::Network, "close_socket") => v,
            (CapabilityKind::Clock, "tick_frame_fps" | "last_frame_ms") => f,
            _ => None,
        }
    }

    fn check_hal_method_call(
        &mut self,
        func: &FnDecl,
        env: &FunctionCapabilityEnv,
        sym_table: &SymbolTable,
        receiver: &Expr,
        method: &str,
        args: &[Expr],
        span: Span,
    ) {
        let Some(TypeExpr::Capability(rec_cap)) = self.infer_expr_type(env, sym_table, receiver)
        else {
            return;
        };
        let str_ty = TypeExpr::Primitive(PrimitiveType::Str);
        let bool_ty = TypeExpr::Primitive(PrimitiveType::Bool);
        let i16_ty = TypeExpr::Primitive(PrimitiveType::I16);
        let i32_ty = TypeExpr::Primitive(PrimitiveType::I32);
        let u8_ty = TypeExpr::Primitive(PrimitiveType::U8);
        let u16_ty = TypeExpr::Primitive(PrimitiveType::U16);
        let u32_ty = TypeExpr::Primitive(PrimitiveType::U32);
        let f32_ty = TypeExpr::Primitive(PrimitiveType::F32);
        let slice_u32_ty = TypeExpr::Capability(CapabilitySpec::from_name_and_arg(
            "ReadSlice",
            Some(CapGenericArg::ElementType(Box::new(u32_ty.clone()))),
        ));
        let mut_slice_u32_ty = TypeExpr::Capability(CapabilitySpec::from_name_and_arg(
            "MutSlice",
            Some(CapGenericArg::ElementType(Box::new(u32_ty.clone()))),
        ));
        let slice_u8_ty = TypeExpr::Capability(CapabilitySpec::from_name_and_arg(
            "ReadSlice",
            Some(CapGenericArg::ElementType(Box::new(u8_ty.clone()))),
        ));
        let mut_slice_u8_ty = TypeExpr::Capability(CapabilitySpec::from_name_and_arg(
            "MutSlice",
            Some(CapGenericArg::ElementType(Box::new(u8_ty))),
        ));
        let mut_slice_i16_ty = TypeExpr::Capability(CapabilitySpec::from_name_and_arg(
            "MutSlice",
            Some(CapGenericArg::ElementType(Box::new(i16_ty))),
        ));

        let expected: Option<Vec<TypeExpr>> = match (&rec_cap.kind, method) {
            (
                CapabilityKind::ScopedArena(_) | CapabilityKind::ReadScopedArena(_),
                "used_bytes" | "capacity_bytes",
            ) => Some(vec![]),
            (CapabilityKind::WindowHost, "open_fullscreen" | "open_windowed") => {
                Some(vec![str_ty.clone(), i32_ty.clone(), i32_ty.clone()])
            }
            (CapabilityKind::WindowHost, "open_headless") => {
                Some(vec![i32_ty.clone(), i32_ty.clone(), u32_ty.clone()])
            }
            (
                CapabilityKind::WindowHost,
                "set_fullscreen" | "set_cursor_locked" | "set_cursor_visible",
            ) => Some(vec![bool_ty.clone()]),
            (
                CapabilityKind::WindowHost,
                "width" | "height" | "poll_events" | "close" | "is_smoke_test",
            ) => Some(vec![]),
            (CapabilityKind::WindowHost, "present_rgba") => Some(vec![slice_u32_ty.clone()]),
            (CapabilityKind::WindowHost, "draw_text") => {
                Some(vec![i32_ty.clone(), i32_ty.clone(), u32_ty.clone(), str_ty.clone()])
            }
            (CapabilityKind::WindowHost, "draw_rect") => Some(vec![
                i32_ty.clone(),
                i32_ty.clone(),
                i32_ty.clone(),
                i32_ty.clone(),
                u32_ty.clone(),
            ]),
            (CapabilityKind::WindowHost, "draw_text_scaled") => Some(vec![
                i32_ty.clone(),
                i32_ty.clone(),
                i32_ty.clone(),
                u32_ty.clone(),
                str_ty.clone(),
            ]),
            (CapabilityKind::WindowHost, "draw_blit_rgba") => Some(vec![
                i32_ty.clone(),
                i32_ty.clone(),
                i32_ty.clone(),
                i32_ty.clone(),
                slice_u32_ty,
            ]),
            (
                CapabilityKind::InputDevice,
                "poll"
                | "mouse_x"
                | "mouse_y"
                | "mouse_dx"
                | "mouse_dy"
                | "mouse_wheel_delta"
                | "mouse_left_down"
                | "mouse_clicked"
                | "mouse_right_down"
                | "mouse_right_clicked"
                | "gamepad_connected"
                | "gamepad_move_x"
                | "gamepad_move_y"
                | "gamepad_aim_x"
                | "gamepad_aim_y"
                | "gamepad_aim_active"
                | "gamepad_fire_down",
            ) => Some(vec![]),
            (
                CapabilityKind::InputDevice,
                "is_key_down" | "was_key_pressed" | "was_gamepad_pressed",
            ) => Some(vec![u32_ty.clone()]),
            (CapabilityKind::InputDevice, "set_vibration") => Some(vec![f32_ty.clone()]),
            (CapabilityKind::FileSystem, "create_dir" | "exists" | "file_size") => {
                Some(vec![str_ty.clone()])
            }
            (CapabilityKind::FileSystem, "write_text") => {
                Some(vec![str_ty.clone(), str_ty.clone()])
            }
            (CapabilityKind::FileSystem, "read_text" | "read_raw_slice" | "list_dir") => {
                Some(vec![str_ty.clone(), mut_slice_u8_ty.clone()])
            }
            (CapabilityKind::FileSystem, "write_raw_slice") => {
                Some(vec![str_ty.clone(), slice_u8_ty.clone()])
            }
            (CapabilityKind::FileSystem, "load_image_rgba") => Some(vec![
                str_ty.clone(),
                mut_slice_u32_ty.clone(),
                mut_slice_u32_ty,
            ]),
            (CapabilityKind::FileSystem, "write_raw" | "read_raw") => {
                let u64_ty = TypeExpr::Primitive(PrimitiveType::U64);
                Some(vec![str_ty.clone(), u64_ty.clone(), u64_ty])
            }
            (CapabilityKind::FileSystem, "write_struct" | "read_struct") => {
                if args.len() != 2 {
                    self.diagnostics.push(DiagnosticEntry {
                        error_code: "RTS-E0301".to_string(),
                        category: DiagnosticCategory::TypeMismatch,
                        file: self.file_path.clone(),
                        span,
                        ast_node: format!("HalFsStructArity({})", method),
                        message: format!(
                            "Method '.{}()' in '{}' expected 2 arguments (path, struct), found {}.",
                            method,
                            func.name,
                            args.len()
                        ),
                        deterministic_fixes: vec![],
                    });
                    return;
                }
                if let Some(p_ty) = self.infer_expr_type(env, sym_table, &args[0]) {
                    if !Self::types_compatible(&str_ty, &p_ty) {
                        self.diagnostics.push(DiagnosticEntry {
                            error_code: "RTS-E0301".to_string(),
                            category: DiagnosticCategory::TypeMismatch,
                            file: self.file_path.clone(),
                            span,
                            ast_node: format!("HalFsPath({})", method),
                            message: format!(
                                "Path argument #1 to '.{}()' in '{}' expected 'str', found '{}'.",
                                method, func.name, p_ty
                            ),
                            deterministic_fixes: vec![],
                        });
                    }
                }
                if method == "read_struct" {
                    let is_mut_arg = Self::extract_root_receiver(&args[1])
                        .map_or(false, |r| env.mutable_bindings.contains(&r));
                    if !is_mut_arg {
                        self.diagnostics.push(DiagnosticEntry {
                            error_code: "RTS-E0405".to_string(),
                            category: DiagnosticCategory::AliasingViolation,
                            file: self.file_path.clone(),
                            span,
                            ast_node: "HalFsReadStructMut".to_string(),
                            message: format!(
                                "Destination struct passed to 'fs.read_struct()' in '{}' must be mutable ('let mut' or 'inout').",
                                func.name
                            ),
                            deterministic_fixes: vec![],
                        });
                    }
                }
                if let Some(s_ty) = self.infer_expr_type(env, sym_table, &args[1]) {
                    let is_repr_c = match &s_ty {
                        TypeExpr::Named { name, .. } => sym_table
                            .struct_decls
                            .get(name)
                            .map_or(false, |d| d.is_repr_c()),
                        _ => false,
                    };
                    if !is_repr_c {
                        self.diagnostics.push(DiagnosticEntry {
                            error_code: "RTS-E0602".to_string(),
                            category: DiagnosticCategory::GpuLayoutAlignmentViolation,
                            file: self.file_path.clone(),
                            span,
                            ast_node: format!("HalFsReprC({}, {})", method, s_ty),
                            message: format!(
                                "Binary FileSystem I/O '.{}()' in '{}' requires a '@repr_c' struct with deterministic ABI layout, but found '{}'.",
                                method, func.name, s_ty
                            ),
                            deterministic_fixes: vec![],
                        });
                    }
                }
                return;
            }
            (CapabilityKind::AudioDevice, "write_synth_wav") => Some(vec![
                str_ty.clone(),
                u32_ty.clone(),
                u32_ty.clone(),
                i32_ty.clone(),
            ]),
            (CapabilityKind::AudioDevice, "play_wav_async") => Some(vec![str_ty.clone()]),
            (CapabilityKind::AudioDevice, "mixer_init") => Some(vec![u32_ty.clone()]),
            (CapabilityKind::AudioDevice, "mixer_load_wav") => {
                Some(vec![u32_ty.clone(), str_ty.clone()])
            }
            (CapabilityKind::AudioDevice, "mixer_play") => Some(vec![
                u32_ty.clone(),
                f32_ty.clone(),
                f32_ty.clone(),
                f32_ty.clone(),
                bool_ty.clone(),
            ]),
            (CapabilityKind::AudioDevice, "mixer_set_channel") => Some(vec![
                i32_ty.clone(),
                f32_ty.clone(),
                f32_ty.clone(),
                f32_ty.clone(),
            ]),
            (CapabilityKind::AudioDevice, "mixer_stop_channel") => Some(vec![i32_ty.clone()]),
            (CapabilityKind::AudioDevice, "mixer_mix_frame") => Some(vec![mut_slice_i16_ty.clone()]),
            (CapabilityKind::AudioInput, "open") => Some(vec![u32_ty.clone()]),
            (CapabilityKind::AudioInput, "set_muted") => Some(vec![bool_ty]),
            (CapabilityKind::AudioInput, "is_muted" | "close") => Some(vec![]),
            (CapabilityKind::AudioInput, "read_pcm") => Some(vec![mut_slice_i16_ty]),
            (CapabilityKind::Network, "udp_bind" | "tcp_listen") => Some(vec![u16_ty.clone()]),
            (CapabilityKind::Network, "udp_send_to") => {
                Some(vec![i32_ty.clone(), str_ty.clone(), u16_ty.clone(), slice_u8_ty.clone()])
            }
            (CapabilityKind::Network, "udp_recv_from") => {
                Some(vec![i32_ty.clone(), mut_slice_u8_ty.clone(), mut_slice_u8_ty.clone()])
            }
            (CapabilityKind::Network, "tcp_connect") => Some(vec![str_ty, u16_ty]),
            (CapabilityKind::Network, "tcp_accept" | "close_socket") => Some(vec![i32_ty.clone()]),
            (CapabilityKind::Network, "tcp_send") => Some(vec![i32_ty.clone(), slice_u8_ty]),
            (CapabilityKind::Network, "tcp_recv") => Some(vec![i32_ty, mut_slice_u8_ty]),
            (CapabilityKind::Clock, "tick_frame_fps") => Some(vec![u32_ty]),
            (CapabilityKind::Clock, "last_frame_ms") => Some(vec![]),
            _ => None,
        };

        if let Some(exp_tys) = expected {
            if args.len() != exp_tys.len() {
                self.diagnostics.push(DiagnosticEntry {
                    error_code: "RTS-E0301".to_string(),
                    category: DiagnosticCategory::TypeMismatch,
                    file: self.file_path.clone(),
                    span,
                    ast_node: format!("HalArity({})", method),
                    message: format!(
                        "HAL method '.{}()' in '{}' expected {} argument(s), found {}.",
                        method,
                        func.name,
                        exp_tys.len(),
                        args.len()
                    ),
                    deterministic_fixes: vec![],
                });
                return;
            }
            for (idx, (a, exp)) in args.iter().zip(exp_tys.iter()).enumerate() {
                if let Some(actual) = self.infer_expr_type(env, sym_table, a) {
                    if !Self::types_compatible(exp, &actual) {
                        self.diagnostics.push(DiagnosticEntry {
                            error_code: "RTS-E0301".to_string(),
                            category: DiagnosticCategory::TypeMismatch,
                            file: self.file_path.clone(),
                            span,
                            ast_node: format!("HalArg({}[{}])", method, idx),
                            message: format!(
                                "Type mismatch in HAL method '.{}()' inside '{}': argument #{} expected '{}', found '{}'.",
                                method, func.name, idx + 1, exp, actual
                            ),
                            deterministic_fixes: vec![],
                        });
                    }
                }
            }
        }
    }
}
