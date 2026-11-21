use crate::ast::*;
use std::collections::{HashMap, HashSet};

/// Gerador Direto de GPU Compute & Graphics Shaders (`rtsc v2.3.0-rts`):
///   - `--target=wgsl`: Emite módulos WebGPU Shading Language (`.wgsl`) com `@compute @workgroup_size(64, 1, 1)`
///     e buffers `@group(0) @binding(N) var<storage, read / read_write>`.
///   - `--target=spirv`: Emite módulos Vulkan / DirectX 12 SPIR-V 1.5 tanto em formato textual auditável (`.spvasm`)
///     quanto em bytecode binário completo de words de 32 bits little-endian (`.spv`, magic `0x07230203`).
pub struct GpuCodegen;

pub const SPIRV_MAGIC: u32 = 0x0723_0203;
pub const SPIRV_VERSION_1_5: u32 = 0x0001_0500;
pub const SPIRV_GENERATOR_RTSC: u32 = 0x5254_5343; // "RTSC"

#[derive(Default, Clone)]
struct WgslCtx {
    storage_buffers: HashSet<String>,
    uniform_time_vars: HashSet<String>,
    kernel_parallel_index: Option<String>,
}

/// Montador binário determinístico SPIR-V 1.5 (organiza as seções exigidas pela especificação Khronos 2.4)
#[derive(Default)]
struct SpvBinaryBuilder {
    capabilities: Vec<u32>,
    ext_inst_imports: Vec<u32>,
    memory_model: Vec<u32>,
    entry_points: Vec<u32>,
    exec_modes: Vec<u32>,
    names: Vec<u32>,
    decorations: Vec<u32>,
    types_consts_globals: Vec<u32>,
    functions: Vec<u32>,
    next_id: u32,
    uint_consts: HashMap<u32, u32>,
    int_consts: HashMap<i32, u32>,
    float_consts: HashMap<u32, u32>,
}

impl SpvBinaryBuilder {
    fn new() -> Self {
        Self {
            next_id: 1,
            ..Default::default()
        }
    }

    fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn push_inst(section: &mut Vec<u32>, opcode: u16, operands: &[u32]) {
        let word_count = (1 + operands.len()) as u32;
        section.push((word_count << 16) | (opcode as u32));
        section.extend_from_slice(operands);
    }

    fn emit_name(&mut self, target_id: u32, name: &str) {
        let mut ops = vec![target_id];
        ops.extend(GpuCodegen::encode_spirv_string(name));
        Self::push_inst(&mut self.names, 5, &ops); // OpName = 5
    }

    fn emit_member_name(&mut self, struct_id: u32, member_idx: u32, name: &str) {
        let mut ops = vec![struct_id, member_idx];
        ops.extend(GpuCodegen::encode_spirv_string(name));
        Self::push_inst(&mut self.names, 6, &ops); // OpMemberName = 6
    }

    fn emit_decorate(&mut self, target_id: u32, decoration: u32, literals: &[u32]) {
        let mut ops = vec![target_id, decoration];
        ops.extend_from_slice(literals);
        Self::push_inst(&mut self.decorations, 71, &ops); // OpDecorate = 71
    }

    fn emit_member_decorate(
        &mut self,
        struct_id: u32,
        member_idx: u32,
        decoration: u32,
        literals: &[u32],
    ) {
        let mut ops = vec![struct_id, member_idx, decoration];
        ops.extend_from_slice(literals);
        Self::push_inst(&mut self.decorations, 72, &ops); // OpMemberDecorate = 72
    }

    fn get_or_emit_uint_const(&mut self, ty_uint: u32, val: u32) -> u32 {
        if let Some(&id) = self.uint_consts.get(&val) {
            return id;
        }
        let id = self.alloc_id();
        Self::push_inst(&mut self.types_consts_globals, 43, &[ty_uint, id, val]); // OpConstant = 43
        self.uint_consts.insert(val, id);
        id
    }

    fn get_or_emit_int_const(&mut self, ty_int: u32, val: i32) -> u32 {
        if let Some(&id) = self.int_consts.get(&val) {
            return id;
        }
        let id = self.alloc_id();
        Self::push_inst(
            &mut self.types_consts_globals,
            43,
            &[ty_int, id, val as u32],
        );
        self.int_consts.insert(val, id);
        id
    }

    fn get_or_emit_float_const(&mut self, ty_float: u32, val: f32) -> u32 {
        let bits = val.to_bits();
        if let Some(&id) = self.float_consts.get(&bits) {
            return id;
        }
        let id = self.alloc_id();
        Self::push_inst(&mut self.types_consts_globals, 43, &[ty_float, id, bits]);
        self.float_consts.insert(bits, id);
        id
    }

    fn finish(self) -> Vec<u8> {
        let total_words = 5
            + self.capabilities.len()
            + self.ext_inst_imports.len()
            + self.memory_model.len()
            + self.entry_points.len()
            + self.exec_modes.len()
            + self.names.len()
            + self.decorations.len()
            + self.types_consts_globals.len()
            + self.functions.len();

        let mut words = Vec::with_capacity(total_words);
        words.push(SPIRV_MAGIC);
        words.push(SPIRV_VERSION_1_5);
        words.push(SPIRV_GENERATOR_RTSC);
        words.push(self.next_id);
        words.push(0);

        words.extend_from_slice(&self.capabilities);
        words.extend_from_slice(&self.ext_inst_imports);
        words.extend_from_slice(&self.memory_model);
        words.extend_from_slice(&self.entry_points);
        words.extend_from_slice(&self.exec_modes);
        words.extend_from_slice(&self.names);
        words.extend_from_slice(&self.decorations);
        words.extend_from_slice(&self.types_consts_globals);
        words.extend_from_slice(&self.functions);

        let mut bytes = Vec::with_capacity(words.len() * 4);
        for w in words {
            bytes.extend_from_slice(&w.to_le_bytes());
        }
        bytes
    }
}

impl GpuCodegen {
    // =========================================================================
    // 1. EMISSOR WEBGPU WGSL (`--target=wgsl`)
    // =========================================================================

    pub fn emit_wgsl(module: &Module) -> String {
        let mut out = String::new();
        let mod_name = if module.path.is_empty() {
            "root".to_string()
        } else {
            module.path.join(".")
        };

        out.push_str("// ==========================================================================\n");
        out.push_str("// Generated by rtsc 2.3.0-rts --target=wgsl (WebGPU Shader Backend)\n");
        out.push_str(&format!("// Source Module: {}\n", mod_name));
        out.push_str("// Guarantees:\n");
        out.push_str("//   1. Static std430/std140 Layout Verification (RTS-E0602)\n");
        out.push_str("//   2. Zero Host/IO Capability Escalation & Recursion Freedom (RTS-E0601/E0603)\n");
        out.push_str("//   3. Automatic Capability-to-Binding Lowering (@group(0) @binding(N))\n");
        out.push_str("// ==========================================================================\n\n");

        out.push_str("struct RtsTimeStep {\n");
        out.push_str("    fixed_delta: f32,\n");
        out.push_str("    _pad0: f32,\n");
        out.push_str("    _pad1: f32,\n");
        out.push_str("    _pad2: f32,\n");
        out.push_str("};\n\n");

        for item in &module.items {
            if let Item::Struct(s) = item {
                let is_shared = s
                    .attributes
                    .iter()
                    .any(|a| matches!(a, Attribute::ShaderShared));
                let is_comp = s
                    .attributes
                    .iter()
                    .any(|a| matches!(a, Attribute::Component));
                if is_shared {
                    out.push_str("// @shader_shared (std430 GPU layout verified)\n");
                } else if is_comp {
                    out.push_str("// @component (GPU Storage Buffer element)\n");
                }
                out.push_str(&format!("struct {} {{\n", s.name));
                for f in &s.fields {
                    out.push_str(&format!(
                        "    {}: {},\n",
                        f.name,
                        Self::map_wgsl_type(&f.ty)
                    ));
                }
                out.push_str("};\n\n");
            }
        }

        let mut binding_idx = 0u32;
        let mut emitted_bindings = HashSet::new();
        let mut global_storage_buffers = HashSet::new();
        let mut global_time_vars = HashSet::new();

        for item in &module.items {
            if let Item::Function(f) = item {
                if Self::is_gpu_entry_fn(f) {
                    for p in &f.params {
                        if let Some(cap) = &p.capability {
                            if emitted_bindings.contains(&p.name) {
                                continue;
                            }
                            match (&cap.kind, &cap.generic_arg) {
                                (CapabilityKind::ReadColumn, Some(CapGenericArg::TypeName(t)))
                                | (
                                    CapabilityKind::ReadScopedArena(_),
                                    Some(CapGenericArg::TypeName(t)),
                                ) => {
                                    out.push_str(&format!(
                                        "@group(0) @binding({}) var<storage, read> {}: array<{}>;\n",
                                        binding_idx, p.name, t
                                    ));
                                    emitted_bindings.insert(p.name.clone());
                                    global_storage_buffers.insert(p.name.clone());
                                    binding_idx += 1;
                                }
                                (CapabilityKind::MutColumn, Some(CapGenericArg::TypeName(t)))
                                | (
                                    CapabilityKind::ScopedArena(_),
                                    Some(CapGenericArg::TypeName(t)),
                                ) => {
                                    out.push_str(&format!(
                                        "@group(0) @binding({}) var<storage, read_write> {}: array<{}>;\n",
                                        binding_idx, p.name, t
                                    ));
                                    emitted_bindings.insert(p.name.clone());
                                    global_storage_buffers.insert(p.name.clone());
                                    binding_idx += 1;
                                }
                                (CapabilityKind::Clock, _) => {
                                    out.push_str(&format!(
                                        "@group(0) @binding({}) var<uniform> {}: RtsTimeStep;\n",
                                        binding_idx, p.name
                                    ));
                                    emitted_bindings.insert(p.name.clone());
                                    global_time_vars.insert(p.name.clone());
                                    binding_idx += 1;
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
        if !emitted_bindings.is_empty() {
            out.push('\n');
        }

        for item in &module.items {
            if let Item::Function(f) = item {
                if f.name == "main" && !Self::is_gpu_entry_fn(f) {
                    continue;
                }
                let mut ctx = WgslCtx {
                    storage_buffers: global_storage_buffers.clone(),
                    uniform_time_vars: global_time_vars.clone(),
                    kernel_parallel_index: None,
                };

                if Self::is_vertex_shader_fn(f) {
                    let mut params_str = Vec::new();
                    for (loc, p) in f.params.iter().enumerate() {
                        if !p.is_cap {
                            params_str.push(format!(
                                "@location({}) {}: {}",
                                loc,
                                p.name,
                                Self::map_wgsl_type(&p.ty)
                            ));
                        }
                    }
                    out.push_str("@vertex\n");
                    out.push_str(&format!(
                        "fn {}({}) -> @builtin(position) vec4<f32> {{\n",
                        f.name,
                        params_str.join(", ")
                    ));
                    for stmt in &f.body {
                        Self::lower_wgsl_stmt(&mut out, stmt, &mut ctx, 1, false);
                    }
                    out.push_str("}\n\n");
                } else if Self::is_fragment_shader_fn(f) {
                    let mut params_str = Vec::new();
                    for (loc, p) in f.params.iter().enumerate() {
                        if !p.is_cap {
                            params_str.push(format!(
                                "@location({}) {}: {}",
                                loc,
                                p.name,
                                Self::map_wgsl_type(&p.ty)
                            ));
                        }
                    }
                    out.push_str("@fragment\n");
                    out.push_str(&format!(
                        "fn {}({}) -> @location(0) vec4<f32> {{\n",
                        f.name,
                        params_str.join(", ")
                    ));
                    for stmt in &f.body {
                        Self::lower_wgsl_stmt(&mut out, stmt, &mut ctx, 1, false);
                    }
                    out.push_str("}\n\n");
                } else if Self::is_gpu_entry_fn(f) {
                    out.push_str("@compute @workgroup_size(64, 1, 1)\n");
                    out.push_str(&format!(
                        "fn {}(@builtin(global_invocation_id) global_id: vec3<u32>) {{\n",
                        f.name
                    ));
                    for stmt in &f.body {
                        Self::lower_wgsl_stmt(&mut out, stmt, &mut ctx, 1, true);
                    }
                    out.push_str("}\n\n");
                } else {
                    let mut params_str = Vec::new();
                    for p in &f.params {
                        if !p.is_cap {
                            params_str.push(format!("{}: {}", p.name, Self::map_wgsl_type(&p.ty)));
                        }
                    }
                    let ret_str = f
                        .return_type
                        .as_ref()
                        .map(|t| format!(" -> {}", Self::map_wgsl_type(t)))
                        .unwrap_or_default();
                    out.push_str(&format!(
                        "fn {}({}){} {{\n",
                        f.name,
                        params_str.join(", "),
                        ret_str
                    ));
                    for stmt in &f.body {
                        Self::lower_wgsl_stmt(&mut out, stmt, &mut ctx, 1, false);
                    }
                    out.push_str("}\n\n");
                }
            }
        }

        out
    }

    fn is_gpu_entry_fn(f: &FnDecl) -> bool {
        f.attributes.iter().any(|a| {
            matches!(
                a,
                Attribute::GpuKernel
                    | Attribute::ParallelSystem { .. }
                    | Attribute::System { .. }
            )
        })
    }

    fn is_vertex_shader_fn(f: &FnDecl) -> bool {
        f.attributes
            .iter()
            .any(|a| matches!(a, Attribute::VertexShader))
    }

    fn is_fragment_shader_fn(f: &FnDecl) -> bool {
        f.attributes
            .iter()
            .any(|a| matches!(a, Attribute::FragmentShader))
    }

    fn lower_wgsl_stmt(
        out: &mut String,
        stmt: &Stmt,
        ctx: &mut WgslCtx,
        indent: usize,
        is_kernel_top_level: bool,
    ) {
        let pad = "    ".repeat(indent);
        match stmt {
            Stmt::Let {
                name, ty, value, ..
            } => {
                if let Some(TypeExpr::Capability(_)) = ty {
                    return;
                }
                let val_str = Self::lower_wgsl_expr(value, ctx);
                if let Some(t) = ty {
                    out.push_str(&format!(
                        "{}var {}: {} = {};\n",
                        pad,
                        name,
                        Self::map_wgsl_type(t),
                        val_str
                    ));
                } else {
                    out.push_str(&format!("{}var {} = {};\n", pad, name, val_str));
                }
            }

            Stmt::Assign { target, value, .. } => {
                let lhs = Self::lower_wgsl_expr(target, ctx);
                let rhs = Self::lower_wgsl_expr(value, ctx);
                out.push_str(&format!("{}{} = {};\n", pad, lhs, rhs));
            }

            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                let cond = Self::lower_wgsl_expr(condition, ctx);
                out.push_str(&format!("{}if ({}) {{\n", pad, cond));
                for s in then_branch {
                    Self::lower_wgsl_stmt(out, s, ctx, indent + 1, false);
                }
                if let Some(eb) = else_branch {
                    out.push_str(&format!("{}}} else {{\n", pad));
                    for s in eb {
                        Self::lower_wgsl_stmt(out, s, ctx, indent + 1, false);
                    }
                }
                out.push_str(&format!("{}}}\n", pad));
            }

            Stmt::While {
                condition, body, ..
            } => {
                let cond = Self::lower_wgsl_expr(condition, ctx);
                out.push_str(&format!("{}while ({}) {{\n", pad, cond));
                for s in body {
                    Self::lower_wgsl_stmt(out, s, ctx, indent + 1, false);
                }
                out.push_str(&format!("{}}}\n", pad));
            }

            Stmt::ForRange {
                iterator,
                start,
                end,
                body,
                ..
            } => {
                let end_s = Self::lower_wgsl_expr(end, ctx);
                if is_kernel_top_level && ctx.kernel_parallel_index.is_none() {
                    ctx.kernel_parallel_index = Some(iterator.clone());
                    out.push_str(&format!(
                        "{}let {}: u32 = global_id.x;\n",
                        pad, iterator
                    ));
                    out.push_str(&format!(
                        "{}if ({} >= u32({})) {{\n{}    return;\n{}}}\n",
                        pad, iterator, end_s, pad, pad
                    ));
                    for s in body {
                        Self::lower_wgsl_stmt(out, s, ctx, indent, false);
                    }
                } else {
                    let start_s = Self::lower_wgsl_expr(start, ctx);
                    out.push_str(&format!(
                        "{}for (var {}: u32 = u32({}); {} < u32({}); {} = {} + 1u) {{\n",
                        pad, iterator, start_s, iterator, end_s, iterator, iterator
                    ));
                    for s in body {
                        Self::lower_wgsl_stmt(out, s, ctx, indent + 1, false);
                    }
                    out.push_str(&format!("{}}}\n", pad));
                }
            }

            Stmt::Break { .. } => {
                out.push_str(&format!("{}break;\n", pad));
            }

            Stmt::Continue { .. } => {
                out.push_str(&format!("{}continue;\n", pad));
            }

            Stmt::Return { value, .. } => {
                if let Some(v) = value {
                    let vs = Self::lower_wgsl_expr(v, ctx);
                    out.push_str(&format!("{}return {};\n", pad, vs));
                } else {
                    out.push_str(&format!("{}return;\n", pad));
                }
            }

            Stmt::Match { expr, arms, .. } => {
                let scrut = Self::lower_wgsl_expr(expr, ctx);
                if let Some(first) = arms.first() {
                    if let MatchPattern::Variant {
                        binding: Some(b), ..
                    } = &first.pattern
                    {
                        out.push_str(&format!("{}let {} = {};\n", pad, b, scrut));
                    }
                    for s in &first.body {
                        Self::lower_wgsl_stmt(out, s, ctx, indent, false);
                    }
                }
            }

            Stmt::ExprStmt { expr, .. } => {
                let e = Self::lower_wgsl_expr(expr, ctx);
                out.push_str(&format!("{}{};\n", pad, e));
            }
        }
    }

    fn lower_wgsl_expr(expr: &Expr, ctx: &WgslCtx) -> String {
        match expr {
            Expr::IntLit(v, _) => v.to_string(),
            Expr::FloatLit(v, _) => {
                if v.fract() == 0.0 {
                    format!("{:.1}", v)
                } else {
                    v.to_string()
                }
            }
            Expr::BoolLit(b, _) => b.to_string(),
            Expr::StringLit(s, _) => format!("\"{}\"", s),
            Expr::Ident(name, _) => name.clone(),
            Expr::Unary { op, expr: inner, .. } => {
                let inner_s = Self::lower_wgsl_expr(inner, ctx);
                match op {
                    UnaryOp::Neg => format!("-({})", inner_s),
                    UnaryOp::Not => format!("!({})", inner_s),
                    UnaryOp::BitNot => format!("~({})", inner_s),
                }
            }
            Expr::Binary {
                left, op, right, ..
            } => {
                let l = Self::lower_wgsl_expr(left, ctx);
                let r = Self::lower_wgsl_expr(right, ctx);
                let op_str = match op {
                    BinaryOp::Add => "+",
                    BinaryOp::Sub => "-",
                    BinaryOp::Mul => "*",
                    BinaryOp::Div => "/",
                    BinaryOp::Rem => "%",
                    BinaryOp::BitAnd => "&",
                    BinaryOp::BitOr => "|",
                    BinaryOp::BitXor => "^",
                    BinaryOp::Shl => "<<",
                    BinaryOp::Shr => ">>",
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
            Expr::FieldAccess {
                receiver, field, ..
            } => {
                let rec = Self::lower_wgsl_expr(receiver, ctx);
                format!("{}.{}", rec, field)
            }
            Expr::Index {
                receiver, index, ..
            } => {
                let rec = Self::lower_wgsl_expr(receiver, ctx);
                let idx = Self::lower_wgsl_expr(index, ctx);
                format!("{}[{}]", rec, idx)
            }
            Expr::MethodCall {
                receiver,
                method,
                args,
                ..
            } => {
                let rec = Self::lower_wgsl_expr(receiver, ctx);
                if method == "len" && ctx.storage_buffers.contains(&rec) {
                    return format!("arrayLength(&{})", rec);
                }
                if method == "value" && ctx.uniform_time_vars.contains(&rec) {
                    return format!("{}.fixed_delta", rec);
                }
                let mut call_args = vec![rec];
                for a in args {
                    call_args.push(Self::lower_wgsl_expr(a, ctx));
                }
                format!("{}({})", method, call_args.join(", "))
            }
            Expr::StaticCall {
                namespace,
                func,
                args,
                ..
            } => {
                let arg_strs: Vec<String> =
                    args.iter().map(|a| Self::lower_wgsl_expr(a, ctx)).collect();
                if namespace == "math" {
                    let wgsl_fn = match func.as_str() {
                        "lerp" => "mix",
                        other => other,
                    };
                    return format!("{}({})", wgsl_fn, arg_strs.join(", "));
                }
                if namespace == "self"
                    && matches!(func.as_str(), "vec2" | "vec3" | "vec4" | "quat")
                {
                    let wgsl_ty = match func.as_str() {
                        "vec2" => "vec2<f32>",
                        "vec3" => "vec3<f32>",
                        _ => "vec4<f32>",
                    };
                    return format!("{}({})", wgsl_ty, arg_strs.join(", "));
                }
                if namespace == "self"
                    && matches!(func.as_str(), "fma" | "sqrt" | "sin" | "cos" | "abs" | "clamp")
                {
                    return format!("{}({})", func, arg_strs.join(", "));
                }
                format!("{}({})", func, arg_strs.join(", "))
            }
            Expr::StructInit {
                struct_name,
                fields,
                ..
            } => {
                let f_vals: Vec<String> = fields
                    .iter()
                    .map(|(_, e)| Self::lower_wgsl_expr(e, ctx))
                    .collect();
                format!("{}({})", struct_name, f_vals.join(", "))
            }
            Expr::Cast {
                expr: inner,
                target_type,
                ..
            } => {
                let in_s = Self::lower_wgsl_expr(inner, ctx);
                let ty_s = Self::map_wgsl_type(target_type);
                format!("{}({})", ty_s, in_s)
            }
            Expr::ArrayLit { elements, .. } => {
                let el_s: Vec<String> = elements
                    .iter()
                    .map(|e| Self::lower_wgsl_expr(e, ctx))
                    .collect();
                format!("array({})", el_s.join(", "))
            }
            Expr::EnumConstructor { payload, .. } => {
                if let Some(p) = payload {
                    Self::lower_wgsl_expr(p, ctx)
                } else {
                    "0".to_string()
                }
            }
            Expr::TryOp { expr: inner, .. } | Expr::ErrorMapClosure { body: inner, .. } => {
                Self::lower_wgsl_expr(inner, ctx)
            }
            Expr::IfExpr {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                let c = Self::lower_wgsl_expr(cond, ctx);
                let t = Self::lower_wgsl_expr(then_branch, ctx);
                let e = Self::lower_wgsl_expr(else_branch, ctx);
                format!("select({}, {}, {})", e, t, c)
            }
        }
    }

    pub fn map_wgsl_type(ty: &TypeExpr) -> String {
        match ty {
            TypeExpr::Primitive(p) => match p {
                PrimitiveType::I8
                | PrimitiveType::I16
                | PrimitiveType::I32
                | PrimitiveType::I64
                | PrimitiveType::Fix32 => "i32".to_string(),
                PrimitiveType::U8
                | PrimitiveType::U16
                | PrimitiveType::U32
                | PrimitiveType::U64
                | PrimitiveType::Usize => "u32".to_string(),
                PrimitiveType::F32 | PrimitiveType::F64 => "f32".to_string(),
                PrimitiveType::Bool => "bool".to_string(),
                PrimitiveType::Str
                | PrimitiveType::Str64
                | PrimitiveType::Str128
                | PrimitiveType::Str256 => "u32".to_string(),
            },
            TypeExpr::Math(m) => match m {
                MathType::Vec2 => "vec2<f32>".to_string(),
                MathType::Vec3 => "vec3<f32>".to_string(),
                MathType::Vec4 | MathType::Quat => "vec4<f32>".to_string(),
                MathType::Mat3 => "mat3x3<f32>".to_string(),
                MathType::Mat4 => "mat4x4<f32>".to_string(),
            },
            TypeExpr::Array(inner, len) => {
                format!("array<{}, {}>", Self::map_wgsl_type(inner), len)
            }
            TypeExpr::Handle(_) => "u32".to_string(),
            TypeExpr::Option(inner) | TypeExpr::Result(inner, _) => Self::map_wgsl_type(inner),
            TypeExpr::Capability(_) => "u32".to_string(),
            TypeExpr::Named { name, .. } => name.clone(),
        }
    }

    // =========================================================================
    // 2. EMISSOR VULKAN / DX12 SPIR-V 1.5 (`.spvasm` & `.spv` Binário Real)
    // =========================================================================

    pub fn emit_spirv_asm(module: &Module) -> String {
        let mut header = String::new();
        let mut entry_points = String::new();
        let mut exec_modes = String::new();
        let mut names = String::new();
        let mut decorations = String::new();
        let mut types_consts = String::new();
        let mut globals = String::new();
        let mut functions = String::new();

        types_consts.push_str("%void = OpTypeVoid\n");
        types_consts.push_str("%bool = OpTypeBool\n");
        types_consts.push_str("%int = OpTypeInt 32 1\n");
        types_consts.push_str("%uint = OpTypeInt 32 0\n");
        types_consts.push_str("%float = OpTypeFloat 32\n");
        types_consts.push_str("%v2float = OpTypeVector %float 2\n");
        types_consts.push_str("%v3float = OpTypeVector %float 3\n");
        types_consts.push_str("%v4float = OpTypeVector %float 4\n");
        types_consts.push_str("%v3uint = OpTypeVector %uint 3\n");
        types_consts.push_str("%fn_void = OpTypeFunction %void\n");
        types_consts.push_str("%ptr_input_v3uint = OpTypePointer Input %v3uint\n");
        types_consts.push_str("%ptr_input_uint = OpTypePointer Input %uint\n");
        types_consts.push_str("%ptr_sb_float = OpTypePointer StorageBuffer %float\n");
        types_consts.push_str("%ptr_ub_float = OpTypePointer Uniform %float\n");
        types_consts.push_str("%uint_0 = OpConstant %uint 0\n");
        types_consts.push_str("%uint_1 = OpConstant %uint 1\n");
        types_consts.push_str("%float_0 = OpConstant %float 0.0\n");

        let mut emitted_uint_consts: HashSet<u32> = HashSet::from([0, 1]);
        let mut emitted_float_consts: HashMap<u32, String> = HashMap::new();
        emitted_float_consts.insert(0.0f32.to_bits(), "%float_0".to_string());

        decorations.push_str("OpDecorate %gl_GlobalInvocationID BuiltIn GlobalInvocationId\n");
        globals.push_str(
            "%gl_GlobalInvocationID = OpVariable %ptr_input_v3uint Input\n",
        );

        let mut struct_field_map: HashMap<(String, String), (usize, String)> = HashMap::new();
        for item in &module.items {
            if let Item::Struct(s) = item {
                names.push_str(&format!("OpName %{}_t \"{}\"\n", s.name, s.name));
                let mut f_types = Vec::new();
                let mut byte_offset = 0u32;
                for (idx, f) in s.fields.iter().enumerate() {
                    let spv_ty = Self::map_spirv_type(&f.ty);
                    f_types.push(spv_ty.clone());
                    struct_field_map.insert((s.name.clone(), f.name.clone()), (idx, spv_ty));
                    if emitted_uint_consts.insert(idx as u32) {
                        types_consts.push_str(&format!(
                            "%uint_{} = OpConstant %uint {}\n",
                            idx, idx
                        ));
                    }
                    names.push_str(&format!(
                        "OpMemberName %{}_t {} \"{}\"\n",
                        s.name, idx, f.name
                    ));
                    decorations.push_str(&format!(
                        "OpMemberDecorate %{}_t {} Offset {}\n",
                        s.name, idx, byte_offset
                    ));
                    byte_offset += Self::spirv_type_size_bytes(&f.ty);
                }
                let stride = byte_offset.max(4);
                types_consts.push_str(&format!(
                    "%{}_t = OpTypeStruct {}\n",
                    s.name,
                    f_types.join(" ")
                ));
                types_consts.push_str(&format!(
                    "%_runtimearr_{}_t = OpTypeRuntimeArray %{}_t\n",
                    s.name, s.name
                ));
                decorations.push_str(&format!(
                    "OpDecorate %_runtimearr_{}_t ArrayStride {}\n",
                    s.name, stride
                ));
                types_consts.push_str(&format!(
                    "%BufferBlock_{} = OpTypeStruct %_runtimearr_{}_t\n",
                    s.name, s.name
                ));
                decorations.push_str(&format!("OpDecorate %BufferBlock_{} Block\n", s.name));
                decorations.push_str(&format!(
                    "OpMemberDecorate %BufferBlock_{} 0 Offset 0\n",
                    s.name
                ));
                types_consts.push_str(&format!(
                    "%ptr_sb_block_{} = OpTypePointer StorageBuffer %BufferBlock_{}\n",
                    s.name, s.name
                ));
            }
        }

        types_consts.push_str("%RtsTimeStep_t = OpTypeStruct %float\n");
        decorations.push_str("OpDecorate %RtsTimeStep_t Block\n");
        decorations.push_str("OpMemberDecorate %RtsTimeStep_t 0 Offset 0\n");
        types_consts.push_str("%ptr_ub_RtsTimeStep = OpTypePointer Uniform %RtsTimeStep_t\n");

        let mut binding_idx = 0u32;
        let mut emitted_vars = HashSet::new();
        let mut reg_counter = 100usize;

        for item in &module.items {
            if let Item::Function(f) = item {
                if !Self::is_gpu_entry_fn(f) {
                    continue;
                }
                entry_points.push_str(&format!(
                    "OpEntryPoint GLCompute %{} \"{}\" %gl_GlobalInvocationID\n",
                    f.name, f.name
                ));
                exec_modes.push_str(&format!(
                    "OpExecutionMode %{} LocalSize 64 1 1\n",
                    f.name
                ));
                names.push_str(&format!("OpName %{} \"{}\"\n", f.name, f.name));

                let mut col_types: HashMap<String, String> = HashMap::new();
                for p in &f.params {
                    if let Some(cap) = &p.capability {
                        match (&cap.kind, &cap.generic_arg) {
                            (CapabilityKind::ReadColumn, Some(CapGenericArg::TypeName(t)))
                            | (CapabilityKind::MutColumn, Some(CapGenericArg::TypeName(t))) => {
                                col_types.insert(p.name.clone(), t.clone());
                                if !emitted_vars.contains(&p.name) {
                                    decorations.push_str(&format!(
                                        "OpDecorate %var_{} DescriptorSet 0\n",
                                        p.name
                                    ));
                                    decorations.push_str(&format!(
                                        "OpDecorate %var_{} Binding {}\n",
                                        p.name, binding_idx
                                    ));
                                    if cap.kind == CapabilityKind::ReadColumn {
                                        decorations.push_str(&format!(
                                            "OpDecorate %var_{} NonWritable\n",
                                            p.name
                                        ));
                                    }
                                    globals.push_str(&format!(
                                        "%var_{} = OpVariable %ptr_sb_block_{} StorageBuffer\n",
                                        p.name, t
                                    ));
                                    emitted_vars.insert(p.name.clone());
                                    binding_idx += 1;
                                }
                            }
                            (CapabilityKind::Clock, _) => {
                                if !emitted_vars.contains(&p.name) {
                                    decorations.push_str(&format!(
                                        "OpDecorate %var_{} DescriptorSet 0\n",
                                        p.name
                                    ));
                                    decorations.push_str(&format!(
                                        "OpDecorate %var_{} Binding {}\n",
                                        p.name, binding_idx
                                    ));
                                    globals.push_str(&format!(
                                        "%var_{} = OpVariable %ptr_ub_RtsTimeStep Uniform\n",
                                        p.name
                                    ));
                                    emitted_vars.insert(p.name.clone());
                                    binding_idx += 1;
                                }
                            }
                            _ => {}
                        }
                    }
                }

                functions.push_str(&format!(
                    "%{} = OpFunction %void None %fn_void\n%entry_{} = OpLabel\n",
                    f.name, f.name
                ));
                functions.push_str(
                    "  %gid_ptr = OpAccessChain %ptr_input_uint %gl_GlobalInvocationID %uint_0\n",
                );
                functions.push_str("  %gid_x = OpLoad %uint %gid_ptr\n");

                let mut local_regs: HashMap<String, String> = HashMap::new();
                for stmt in &f.body {
                    Self::lower_spirv_stmt(
                        &mut functions,
                        &mut types_consts,
                        stmt,
                        &mut reg_counter,
                        &mut local_regs,
                        &col_types,
                        &struct_field_map,
                        &mut emitted_uint_consts,
                        &mut emitted_float_consts,
                    );
                }

                functions.push_str("  OpReturn\nOpFunctionEnd\n\n");
            }
        }

        header.push_str("; SPIR-V\n");
        header.push_str("; Version: 1.5 (0x00010500)\n");
        header.push_str("; Generator: RustTS Deterministic GPU Compiler (rtsc 2.3.0-rts); 0x52545343\n");
        header.push_str(&format!("; Bound: {}\n", reg_counter + 32));
        header.push_str("; Schema: 0\n");
        header.push_str("OpCapability Shader\n");
        header.push_str("%glsl450 = OpExtInstImport \"GLSL.std.450\"\n");
        header.push_str("OpMemoryModel Logical GLSL450\n\n");

        format!(
            "{}{}\n{}\n{}\n{}\n{}\n{}\n{}",
            header,
            entry_points,
            exec_modes,
            names,
            decorations,
            types_consts,
            globals,
            functions
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn lower_spirv_stmt(
        out: &mut String,
        types_consts: &mut String,
        stmt: &Stmt,
        reg_counter: &mut usize,
        local_regs: &mut HashMap<String, String>,
        col_types: &HashMap<String, String>,
        struct_field_map: &HashMap<(String, String), (usize, String)>,
        emitted_uint_consts: &mut HashSet<u32>,
        emitted_float_consts: &mut HashMap<u32, String>,
    ) {
        match stmt {
            Stmt::Let { name, value, .. } => {
                let r = Self::lower_spirv_expr(
                    out,
                    types_consts,
                    value,
                    reg_counter,
                    local_regs,
                    col_types,
                    struct_field_map,
                    emitted_uint_consts,
                    emitted_float_consts,
                );
                local_regs.insert(name.clone(), r);
            }
            Stmt::Assign { target, value, .. } => {
                let val_r = Self::lower_spirv_expr(
                    out,
                    types_consts,
                    value,
                    reg_counter,
                    local_regs,
                    col_types,
                    struct_field_map,
                    emitted_uint_consts,
                    emitted_float_consts,
                );
                if let Expr::FieldAccess {
                    receiver, field, ..
                } = target
                {
                    if let Expr::Index {
                        receiver: col_expr, ..
                    } = receiver.as_ref()
                    {
                        if let Expr::Ident(col_name, _) = col_expr.as_ref() {
                            if let Some(comp_name) = col_types.get(col_name) {
                                let f_idx = struct_field_map
                                    .get(&(comp_name.clone(), field.clone()))
                                    .map(|(i, _)| *i)
                                    .unwrap_or(0);
                                let ptr_r = Self::next_spv_reg(reg_counter);
                                out.push_str(&format!(
                                    "  {} = OpAccessChain %ptr_sb_float %var_{} %uint_0 %gid_x %uint_{}\n",
                                    ptr_r, col_name, f_idx
                                ));
                                out.push_str(&format!("  OpStore {} {}\n", ptr_r, val_r));
                            }
                        }
                    }
                }
            }
            Stmt::ForRange {
                iterator, body, ..
            } => {
                local_regs.insert(iterator.clone(), "%gid_x".to_string());
                for s in body {
                    Self::lower_spirv_stmt(
                        out,
                        types_consts,
                        s,
                        reg_counter,
                        local_regs,
                        col_types,
                        struct_field_map,
                        emitted_uint_consts,
                        emitted_float_consts,
                    );
                }
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                let cond_r = Self::lower_spirv_expr(
                    out,
                    types_consts,
                    condition,
                    reg_counter,
                    local_regs,
                    col_types,
                    struct_field_map,
                    emitted_uint_consts,
                    emitted_float_consts,
                );
                let then_lbl = Self::next_spv_reg(reg_counter);
                let merge_lbl = Self::next_spv_reg(reg_counter);
                if let Some(eb) = else_branch {
                    let else_lbl = Self::next_spv_reg(reg_counter);
                    out.push_str(&format!("  OpSelectionMerge {} None\n", merge_lbl));
                    out.push_str(&format!(
                        "  OpBranchConditional {} {} {}\n",
                        cond_r, then_lbl, else_lbl
                    ));
                    out.push_str(&format!("{} = OpLabel\n", then_lbl));
                    for s in then_branch {
                        Self::lower_spirv_stmt(
                            out,
                            types_consts,
                            s,
                            reg_counter,
                            local_regs,
                            col_types,
                            struct_field_map,
                            emitted_uint_consts,
                            emitted_float_consts,
                        );
                    }
                    out.push_str(&format!("  OpBranch {}\n", merge_lbl));
                    out.push_str(&format!("{} = OpLabel\n", else_lbl));
                    for s in eb {
                        Self::lower_spirv_stmt(
                            out,
                            types_consts,
                            s,
                            reg_counter,
                            local_regs,
                            col_types,
                            struct_field_map,
                            emitted_uint_consts,
                            emitted_float_consts,
                        );
                    }
                    out.push_str(&format!("  OpBranch {}\n", merge_lbl));
                    out.push_str(&format!("{} = OpLabel\n", merge_lbl));
                } else {
                    out.push_str(&format!("  OpSelectionMerge {} None\n", merge_lbl));
                    out.push_str(&format!(
                        "  OpBranchConditional {} {} {}\n",
                        cond_r, then_lbl, merge_lbl
                    ));
                    out.push_str(&format!("{} = OpLabel\n", then_lbl));
                    for s in then_branch {
                        Self::lower_spirv_stmt(
                            out,
                            types_consts,
                            s,
                            reg_counter,
                            local_regs,
                            col_types,
                            struct_field_map,
                            emitted_uint_consts,
                            emitted_float_consts,
                        );
                    }
                    out.push_str(&format!("  OpBranch {}\n", merge_lbl));
                    out.push_str(&format!("{} = OpLabel\n", merge_lbl));
                }
            }
            _ => {}
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn lower_spirv_expr(
        out: &mut String,
        types_consts: &mut String,
        expr: &Expr,
        reg_counter: &mut usize,
        local_regs: &HashMap<String, String>,
        col_types: &HashMap<String, String>,
        struct_field_map: &HashMap<(String, String), (usize, String)>,
        emitted_uint_consts: &mut HashSet<u32>,
        emitted_float_consts: &mut HashMap<u32, String>,
    ) -> String {
        match expr {
            Expr::Ident(name, _) => local_regs
                .get(name)
                .cloned()
                .unwrap_or_else(|| "%float_0".to_string()),
            Expr::FloatLit(v, _) => {
                let f = *v as f32;
                let bits = f.to_bits();
                if let Some(existing) = emitted_float_consts.get(&bits) {
                    return existing.clone();
                }
                let cname = format!("%float_c{:08X}", bits);
                let formatted = if f.fract() == 0.0 {
                    format!("{:.1}", f)
                } else {
                    format!("{}", f)
                };
                types_consts.push_str(&format!("{} = OpConstant %float {}\n", cname, formatted));
                emitted_float_consts.insert(bits, cname.clone());
                cname
            }
            Expr::IntLit(v, _) => {
                let u = (*v).max(0) as u32;
                if emitted_uint_consts.insert(u) {
                    types_consts.push_str(&format!("%uint_{} = OpConstant %uint {}\n", u, u));
                }
                format!("%uint_{}", u)
            }
            Expr::FieldAccess {
                receiver, field, ..
            } => {
                if field == "fixed_delta" {
                    if let Expr::Ident(t_name, _) = receiver.as_ref() {
                        let ptr_r = Self::next_spv_reg(reg_counter);
                        out.push_str(&format!(
                            "  {} = OpAccessChain %ptr_ub_float %var_{} %uint_0\n",
                            ptr_r, t_name
                        ));
                        let val_r = Self::next_spv_reg(reg_counter);
                        out.push_str(&format!("  {} = OpLoad %float {}\n", val_r, ptr_r));
                        return val_r;
                    }
                }
                if let Expr::Index {
                    receiver: col_expr, ..
                } = receiver.as_ref()
                {
                    if let Expr::Ident(col_name, _) = col_expr.as_ref() {
                        if let (Some(comp_name), Some(_)) =
                            (col_types.get(col_name), Some(0))
                        {
                            let f_idx = struct_field_map
                                .get(&(comp_name.clone(), field.clone()))
                                .map(|(i, _)| *i)
                                .unwrap_or(0);
                            let ptr_r = Self::next_spv_reg(reg_counter);
                            out.push_str(&format!(
                                "  {} = OpAccessChain %ptr_sb_float %var_{} %uint_0 %gid_x %uint_{}\n",
                                ptr_r, col_name, f_idx
                            ));
                            let val_r = Self::next_spv_reg(reg_counter);
                            out.push_str(&format!("  {} = OpLoad %float {}\n", val_r, ptr_r));
                            return val_r;
                        }
                    }
                }
                "%float_0".to_string()
            }
            Expr::Binary {
                left, op, right, ..
            } => {
                let l = Self::lower_spirv_expr(
                    out,
                    types_consts,
                    left,
                    reg_counter,
                    local_regs,
                    col_types,
                    struct_field_map,
                    emitted_uint_consts,
                    emitted_float_consts,
                );
                let r = Self::lower_spirv_expr(
                    out,
                    types_consts,
                    right,
                    reg_counter,
                    local_regs,
                    col_types,
                    struct_field_map,
                    emitted_uint_consts,
                    emitted_float_consts,
                );
                let dst = Self::next_spv_reg(reg_counter);
                let spv_op = match op {
                    BinaryOp::Add => "OpFAdd %float",
                    BinaryOp::Sub => "OpFSub %float",
                    BinaryOp::Mul => "OpFMul %float",
                    BinaryOp::Div => "OpFDiv %float",
                    BinaryOp::Rem => "OpSRem %int",
                    BinaryOp::BitAnd => "OpBitwiseAnd %int",
                    BinaryOp::BitOr => "OpBitwiseOr %int",
                    BinaryOp::BitXor => "OpBitwiseXor %int",
                    BinaryOp::Shl => "OpShiftLeftLogical %int",
                    BinaryOp::Shr => "OpShiftRightArithmetic %int",
                    BinaryOp::Gt => "OpFOrdGreaterThan %bool",
                    BinaryOp::GtEq => "OpFOrdGreaterThanEqual %bool",
                    BinaryOp::LtEq => "OpFOrdLessThanEqual %bool",
                    BinaryOp::Eq => "OpFOrdEqual %bool",
                    BinaryOp::NotEq => "OpFOrdNotEqual %bool",
                    _ => "OpFOrdLessThan %bool",
                };
                out.push_str(&format!("  {} = {} {} {}\n", dst, spv_op, l, r));
                dst
            }
            Expr::StaticCall {
                namespace,
                func,
                args,
                ..
            } => {
                let mut arg_regs = Vec::new();
                for a in args {
                    arg_regs.push(Self::lower_spirv_expr(
                        out,
                        types_consts,
                        a,
                        reg_counter,
                        local_regs,
                        col_types,
                        struct_field_map,
                        emitted_uint_consts,
                        emitted_float_consts,
                    ));
                }
                if namespace == "math" {
                    let ext_op = match func.as_str() {
                        "sqrt" => "Sqrt",
                        "sin" => "Sin",
                        "cos" => "Cos",
                        "abs" => "FAbs",
                        "clamp" => "FClamp",
                        "lerp" => "FMix",
                        _ => "Fma",
                    };
                    let dst = Self::next_spv_reg(reg_counter);
                    out.push_str(&format!(
                        "  {} = OpExtInst %float %glsl450 {} {}\n",
                        dst,
                        ext_op,
                        arg_regs.join(" ")
                    ));
                    return dst;
                }
                "%float_0".to_string()
            }
            _ => "%float_0".to_string(),
        }
    }

    fn next_spv_reg(reg_counter: &mut usize) -> String {
        let id = *reg_counter;
        *reg_counter += 1;
        format!("%r{}", id)
    }

    fn map_spirv_type(ty: &TypeExpr) -> String {
        match ty {
            TypeExpr::Primitive(PrimitiveType::I32 | PrimitiveType::Fix32) => "%int".to_string(),
            TypeExpr::Primitive(PrimitiveType::U32) => "%uint".to_string(),
            TypeExpr::Math(MathType::Vec2) => "%v2float".to_string(),
            TypeExpr::Math(MathType::Vec3) => "%v3float".to_string(),
            TypeExpr::Math(MathType::Vec4 | MathType::Quat) => "%v4float".to_string(),
            _ => "%float".to_string(),
        }
    }

    fn spirv_type_size_bytes(ty: &TypeExpr) -> u32 {
        match ty {
            TypeExpr::Math(MathType::Vec2) => 8,
            TypeExpr::Math(MathType::Vec3) => 12,
            TypeExpr::Math(MathType::Vec4 | MathType::Quat) => 16,
            TypeExpr::Math(MathType::Mat3) => 36,
            TypeExpr::Math(MathType::Mat4) => 64,
            TypeExpr::Array(inner, len) => Self::spirv_type_size_bytes(inner) * (*len as u32),
            _ => 4,
        }
    }

    /// Compila a AST completa do módulo diretamente em um binário SPIR-V 1.5 (`.spv`).
    pub fn emit_spirv_binary(module: &Module) -> Vec<u8> {
        let mut b = SpvBinaryBuilder::new();

        // 1. Capabilities, ExtInstImport & MemoryModel
        SpvBinaryBuilder::push_inst(&mut b.capabilities, 17, &[1]); // OpCapability Shader (1)
        let id_glsl450 = b.alloc_id();
        let mut ext_ops = vec![id_glsl450];
        ext_ops.extend(Self::encode_spirv_string("GLSL.std.450"));
        SpvBinaryBuilder::push_inst(&mut b.ext_inst_imports, 11, &ext_ops); // OpExtInstImport
        SpvBinaryBuilder::push_inst(&mut b.memory_model, 14, &[0, 1]); // OpMemoryModel Logical(0) GLSL450(1)

        // 2. Tipos Base
        let ty_void = b.alloc_id();
        SpvBinaryBuilder::push_inst(&mut b.types_consts_globals, 19, &[ty_void]); // OpTypeVoid
        let ty_bool = b.alloc_id();
        SpvBinaryBuilder::push_inst(&mut b.types_consts_globals, 20, &[ty_bool]); // OpTypeBool
        let ty_int = b.alloc_id();
        SpvBinaryBuilder::push_inst(&mut b.types_consts_globals, 21, &[ty_int, 32, 1]); // OpTypeInt 32 1
        let ty_uint = b.alloc_id();
        SpvBinaryBuilder::push_inst(&mut b.types_consts_globals, 21, &[ty_uint, 32, 0]); // OpTypeInt 32 0
        let ty_float = b.alloc_id();
        SpvBinaryBuilder::push_inst(&mut b.types_consts_globals, 22, &[ty_float, 32]); // OpTypeFloat 32
        let ty_v2float = b.alloc_id();
        SpvBinaryBuilder::push_inst(&mut b.types_consts_globals, 23, &[ty_v2float, ty_float, 2]);
        let ty_v3float = b.alloc_id();
        SpvBinaryBuilder::push_inst(&mut b.types_consts_globals, 23, &[ty_v3float, ty_float, 3]);
        let ty_v4float = b.alloc_id();
        SpvBinaryBuilder::push_inst(&mut b.types_consts_globals, 23, &[ty_v4float, ty_float, 4]);
        let ty_v3uint = b.alloc_id();
        SpvBinaryBuilder::push_inst(&mut b.types_consts_globals, 23, &[ty_v3uint, ty_uint, 3]);
        let ty_fn_void = b.alloc_id();
        SpvBinaryBuilder::push_inst(&mut b.types_consts_globals, 33, &[ty_fn_void, ty_void]); // OpTypeFunction

        // Storage Classes: Input = 1, Uniform = 2, StorageBuffer = 12
        let ptr_input_v3uint = b.alloc_id();
        SpvBinaryBuilder::push_inst(
            &mut b.types_consts_globals,
            32,
            &[ptr_input_v3uint, 1, ty_v3uint],
        );
        let ptr_input_uint = b.alloc_id();
        SpvBinaryBuilder::push_inst(
            &mut b.types_consts_globals,
            32,
            &[ptr_input_uint, 1, ty_uint],
        );
        let ptr_sb_float = b.alloc_id();
        SpvBinaryBuilder::push_inst(
            &mut b.types_consts_globals,
            32,
            &[ptr_sb_float, 12, ty_float],
        );
        let ptr_ub_float = b.alloc_id();
        SpvBinaryBuilder::push_inst(
            &mut b.types_consts_globals,
            32,
            &[ptr_ub_float, 2, ty_float],
        );

        let const_uint_0 = b.get_or_emit_uint_const(ty_uint, 0);
        let _const_uint_1 = b.get_or_emit_uint_const(ty_uint, 1);
        let const_float_0 = b.get_or_emit_float_const(ty_float, 0.0);

        // BuiltIn GlobalInvocationId (BuiltIn = 11, GlobalInvocationId = 28)
        let var_global_id = b.alloc_id();
        b.emit_decorate(var_global_id, 11, &[28]);
        SpvBinaryBuilder::push_inst(
            &mut b.types_consts_globals,
            59,
            &[ptr_input_v3uint, var_global_id, 1],
        );

        // 3. Structs e StorageBuffer Blocks (Block = 2, ArrayStride = 6, Offset = 35)
        let mut struct_ptr_sb_map: HashMap<String, u32> = HashMap::new();
        let mut struct_field_idx_map: HashMap<(String, String), u32> = HashMap::new();

        for item in &module.items {
            if let Item::Struct(s) = item {
                let struct_id = b.alloc_id();
                b.emit_name(struct_id, &s.name);
                let mut field_type_ids = vec![struct_id];
                let mut byte_offset = 0u32;

                for (idx, f) in s.fields.iter().enumerate() {
                    let f_ty_id = match &f.ty {
                        TypeExpr::Primitive(PrimitiveType::I32 | PrimitiveType::Fix32) => ty_int,
                        TypeExpr::Primitive(PrimitiveType::U32) => ty_uint,
                        TypeExpr::Math(MathType::Vec2) => ty_v2float,
                        TypeExpr::Math(MathType::Vec3) => ty_v3float,
                        TypeExpr::Math(MathType::Vec4 | MathType::Quat) => ty_v4float,
                        _ => ty_float,
                    };
                    field_type_ids.push(f_ty_id);
                    struct_field_idx_map.insert((s.name.clone(), f.name.clone()), idx as u32);
                    b.get_or_emit_uint_const(ty_uint, idx as u32);
                    b.emit_member_name(struct_id, idx as u32, &f.name);
                    b.emit_member_decorate(struct_id, idx as u32, 35, &[byte_offset]);
                    byte_offset += Self::spirv_type_size_bytes(&f.ty);
                }

                let stride = byte_offset.max(4);
                SpvBinaryBuilder::push_inst(&mut b.types_consts_globals, 30, &field_type_ids); // OpTypeStruct

                let rt_arr_id = b.alloc_id();
                SpvBinaryBuilder::push_inst(
                    &mut b.types_consts_globals,
                    29,
                    &[rt_arr_id, struct_id],
                ); // OpTypeRuntimeArray
                b.emit_decorate(rt_arr_id, 6, &[stride]); // ArrayStride = 6

                let block_id = b.alloc_id();
                SpvBinaryBuilder::push_inst(&mut b.types_consts_globals, 30, &[block_id, rt_arr_id]);
                b.emit_decorate(block_id, 2, &[]); // Block = 2
                b.emit_member_decorate(block_id, 0, 35, &[0]); // Offset 0

                let ptr_sb_block = b.alloc_id();
                SpvBinaryBuilder::push_inst(
                    &mut b.types_consts_globals,
                    32,
                    &[ptr_sb_block, 12, block_id],
                );
                struct_ptr_sb_map.insert(s.name.clone(), ptr_sb_block);
            }
        }

        // Uniform TimeStep Block
        let ty_timestep = b.alloc_id();
        SpvBinaryBuilder::push_inst(&mut b.types_consts_globals, 30, &[ty_timestep, ty_float]);
        b.emit_decorate(ty_timestep, 2, &[]);
        b.emit_member_decorate(ty_timestep, 0, 35, &[0]);
        let ptr_ub_timestep = b.alloc_id();
        SpvBinaryBuilder::push_inst(
            &mut b.types_consts_globals,
            32,
            &[ptr_ub_timestep, 2, ty_timestep],
        );

        // 4. Funções Kernel / Sistemas GPU
        let mut binding_idx = 0u32;
        let mut var_ids: HashMap<String, u32> = HashMap::new();
        let mut emitted_any_entry = false;

        for item in &module.items {
            if let Item::Function(f) = item {
                if !Self::is_gpu_entry_fn(f) {
                    continue;
                }
                emitted_any_entry = true;
                let fn_id = b.alloc_id();
                b.emit_name(fn_id, &f.name);

                let mut ep_ops = vec![5, fn_id]; // ExecutionModel GLCompute = 5
                ep_ops.extend(Self::encode_spirv_string(&f.name));
                ep_ops.push(var_global_id);
                SpvBinaryBuilder::push_inst(&mut b.entry_points, 15, &ep_ops); // OpEntryPoint = 15
                SpvBinaryBuilder::push_inst(&mut b.exec_modes, 16, &[fn_id, 17, 64, 1, 1]); // LocalSize = 17

                let mut col_types: HashMap<String, String> = HashMap::new();
                for p in &f.params {
                    if let Some(cap) = &p.capability {
                        match (&cap.kind, &cap.generic_arg) {
                            (CapabilityKind::ReadColumn, Some(CapGenericArg::TypeName(t)))
                            | (CapabilityKind::MutColumn, Some(CapGenericArg::TypeName(t))) => {
                                col_types.insert(p.name.clone(), t.clone());
                                if !var_ids.contains_key(&p.name) {
                                    if let Some(&ptr_sb_ty) = struct_ptr_sb_map.get(t) {
                                        let v_id = b.alloc_id();
                                        b.emit_decorate(v_id, 34, &[0]); // DescriptorSet = 34
                                        b.emit_decorate(v_id, 33, &[binding_idx]); // Binding = 33
                                        if cap.kind == CapabilityKind::ReadColumn {
                                            b.emit_decorate(v_id, 24, &[]); // NonWritable = 24
                                        }
                                        SpvBinaryBuilder::push_inst(
                                            &mut b.types_consts_globals,
                                            59,
                                            &[ptr_sb_ty, v_id, 12],
                                        );
                                        var_ids.insert(p.name.clone(), v_id);
                                        binding_idx += 1;
                                    }
                                }
                            }
                            (CapabilityKind::Clock, _) => {
                                if !var_ids.contains_key(&p.name) {
                                    let v_id = b.alloc_id();
                                    b.emit_decorate(v_id, 34, &[0]);
                                    b.emit_decorate(v_id, 33, &[binding_idx]);
                                    SpvBinaryBuilder::push_inst(
                                        &mut b.types_consts_globals,
                                        59,
                                        &[ptr_ub_timestep, v_id, 2],
                                    );
                                    var_ids.insert(p.name.clone(), v_id);
                                    binding_idx += 1;
                                }
                            }
                            _ => {}
                        }
                    }
                }

                // Corpo da função
                SpvBinaryBuilder::push_inst(&mut b.functions, 54, &[ty_void, fn_id, 0, ty_fn_void]);
                let entry_lbl = b.alloc_id();
                SpvBinaryBuilder::push_inst(&mut b.functions, 248, &[entry_lbl]);

                let gid_ptr = b.alloc_id();
                SpvBinaryBuilder::push_inst(
                    &mut b.functions,
                    65,
                    &[ptr_input_uint, gid_ptr, var_global_id, const_uint_0],
                );
                let gid_x = b.alloc_id();
                SpvBinaryBuilder::push_inst(&mut b.functions, 61, &[ty_uint, gid_x, gid_ptr]);

                let mut local_ids: HashMap<String, u32> = HashMap::new();
                for stmt in &f.body {
                    Self::lower_spirv_bin_stmt(
                        &mut b,
                        stmt,
                        gid_x,
                        ty_float,
                        ty_int,
                        ty_uint,
                        ty_bool,
                        ptr_sb_float,
                        ptr_ub_float,
                        const_uint_0,
                        const_float_0,
                        id_glsl450,
                        &mut local_ids,
                        &var_ids,
                        &col_types,
                        &struct_field_idx_map,
                    );
                }

                SpvBinaryBuilder::push_inst(&mut b.functions, 253, &[]); // OpReturn
                SpvBinaryBuilder::push_inst(&mut b.functions, 56, &[]); // OpFunctionEnd
            }
        }

        if !emitted_any_entry {
            let has_vert = module.items.iter().any(|it| {
                matches!(it, Item::Function(f) if Self::is_vertex_shader_fn(f))
            });
            if has_vert {
                return Self::emit_spirv_vertex(module);
            }
            let has_frag = module.items.iter().any(|it| {
                matches!(it, Item::Function(f) if Self::is_fragment_shader_fn(f))
            });
            if has_frag {
                return Self::emit_spirv_fragment(module);
            }
            let fn_id = b.alloc_id();
            let mut ep_ops = vec![5, fn_id];
            ep_ops.extend(Self::encode_spirv_string("main_kernel"));
            ep_ops.push(var_global_id);
            SpvBinaryBuilder::push_inst(&mut b.entry_points, 15, &ep_ops);
            SpvBinaryBuilder::push_inst(&mut b.exec_modes, 16, &[fn_id, 17, 64, 1, 1]);
            SpvBinaryBuilder::push_inst(&mut b.functions, 54, &[ty_void, fn_id, 0, ty_fn_void]);
            let lbl = b.alloc_id();
            SpvBinaryBuilder::push_inst(&mut b.functions, 248, &[lbl]);
            SpvBinaryBuilder::push_inst(&mut b.functions, 253, &[]);
            SpvBinaryBuilder::push_inst(&mut b.functions, 56, &[]);
        }

        b.finish()
    }

    #[allow(clippy::too_many_arguments)]
    fn lower_spirv_bin_stmt(
        b: &mut SpvBinaryBuilder,
        stmt: &Stmt,
        gid_x: u32,
        ty_float: u32,
        ty_int: u32,
        ty_uint: u32,
        ty_bool: u32,
        ptr_sb_float: u32,
        ptr_ub_float: u32,
        const_uint_0: u32,
        const_float_0: u32,
        id_glsl450: u32,
        local_ids: &mut HashMap<String, u32>,
        var_ids: &HashMap<String, u32>,
        col_types: &HashMap<String, String>,
        struct_field_idx_map: &HashMap<(String, String), u32>,
    ) {
        match stmt {
            Stmt::Let { name, value, .. } => {
                let val_id = Self::lower_spirv_bin_expr(
                    b,
                    value,
                    gid_x,
                    ty_float,
                    ty_int,
                    ty_uint,
                    ty_bool,
                    ptr_sb_float,
                    ptr_ub_float,
                    const_uint_0,
                    const_float_0,
                    id_glsl450,
                    local_ids,
                    var_ids,
                    col_types,
                    struct_field_idx_map,
                );
                local_ids.insert(name.clone(), val_id);
            }
            Stmt::Assign { target, value, .. } => {
                let val_id = Self::lower_spirv_bin_expr(
                    b,
                    value,
                    gid_x,
                    ty_float,
                    ty_int,
                    ty_uint,
                    ty_bool,
                    ptr_sb_float,
                    ptr_ub_float,
                    const_uint_0,
                    const_float_0,
                    id_glsl450,
                    local_ids,
                    var_ids,
                    col_types,
                    struct_field_idx_map,
                );
                if let Expr::FieldAccess {
                    receiver, field, ..
                } = target
                {
                    if let Expr::Index {
                        receiver: col_expr, ..
                    } = receiver.as_ref()
                    {
                        if let Expr::Ident(col_name, _) = col_expr.as_ref() {
                            if let (Some(comp_name), Some(&var_id)) =
                                (col_types.get(col_name), var_ids.get(col_name))
                            {
                                let f_idx = struct_field_idx_map
                                    .get(&(comp_name.clone(), field.clone()))
                                    .copied()
                                    .unwrap_or(0);
                                let const_f_idx = b.get_or_emit_uint_const(ty_uint, f_idx);
                                let ptr_id = b.alloc_id();
                                SpvBinaryBuilder::push_inst(
                                    &mut b.functions,
                                    65,
                                    &[
                                        ptr_sb_float,
                                        ptr_id,
                                        var_id,
                                        const_uint_0,
                                        gid_x,
                                        const_f_idx,
                                    ],
                                );
                                SpvBinaryBuilder::push_inst(&mut b.functions, 62, &[ptr_id, val_id]);
                            }
                        }
                    }
                }
            }
            Stmt::ForRange {
                iterator, body, ..
            } => {
                local_ids.insert(iterator.clone(), gid_x);
                for s in body {
                    Self::lower_spirv_bin_stmt(
                        b,
                        s,
                        gid_x,
                        ty_float,
                        ty_int,
                        ty_uint,
                        ty_bool,
                        ptr_sb_float,
                        ptr_ub_float,
                        const_uint_0,
                        const_float_0,
                        id_glsl450,
                        local_ids,
                        var_ids,
                        col_types,
                        struct_field_idx_map,
                    );
                }
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                let cond_id = Self::lower_spirv_bin_expr(
                    b,
                    condition,
                    gid_x,
                    ty_float,
                    ty_int,
                    ty_uint,
                    ty_bool,
                    ptr_sb_float,
                    ptr_ub_float,
                    const_uint_0,
                    const_float_0,
                    id_glsl450,
                    local_ids,
                    var_ids,
                    col_types,
                    struct_field_idx_map,
                );
                let then_lbl = b.alloc_id();
                let merge_lbl = b.alloc_id();
                if let Some(eb) = else_branch {
                    let else_lbl = b.alloc_id();
                    SpvBinaryBuilder::push_inst(&mut b.functions, 247, &[merge_lbl, 0]); // OpSelectionMerge
                    SpvBinaryBuilder::push_inst(
                        &mut b.functions,
                        250,
                        &[cond_id, then_lbl, else_lbl],
                    ); // OpBranchConditional
                    SpvBinaryBuilder::push_inst(&mut b.functions, 248, &[then_lbl]);
                    for s in then_branch {
                        Self::lower_spirv_bin_stmt(
                            b,
                            s,
                            gid_x,
                            ty_float,
                            ty_int,
                            ty_uint,
                            ty_bool,
                            ptr_sb_float,
                            ptr_ub_float,
                            const_uint_0,
                            const_float_0,
                            id_glsl450,
                            local_ids,
                            var_ids,
                            col_types,
                            struct_field_idx_map,
                        );
                    }
                    SpvBinaryBuilder::push_inst(&mut b.functions, 249, &[merge_lbl]); // OpBranch
                    SpvBinaryBuilder::push_inst(&mut b.functions, 248, &[else_lbl]);
                    for s in eb {
                        Self::lower_spirv_bin_stmt(
                            b,
                            s,
                            gid_x,
                            ty_float,
                            ty_int,
                            ty_uint,
                            ty_bool,
                            ptr_sb_float,
                            ptr_ub_float,
                            const_uint_0,
                            const_float_0,
                            id_glsl450,
                            local_ids,
                            var_ids,
                            col_types,
                            struct_field_idx_map,
                        );
                    }
                    SpvBinaryBuilder::push_inst(&mut b.functions, 249, &[merge_lbl]);
                    SpvBinaryBuilder::push_inst(&mut b.functions, 248, &[merge_lbl]);
                } else {
                    SpvBinaryBuilder::push_inst(&mut b.functions, 247, &[merge_lbl, 0]);
                    SpvBinaryBuilder::push_inst(
                        &mut b.functions,
                        250,
                        &[cond_id, then_lbl, merge_lbl],
                    );
                    SpvBinaryBuilder::push_inst(&mut b.functions, 248, &[then_lbl]);
                    for s in then_branch {
                        Self::lower_spirv_bin_stmt(
                            b,
                            s,
                            gid_x,
                            ty_float,
                            ty_int,
                            ty_uint,
                            ty_bool,
                            ptr_sb_float,
                            ptr_ub_float,
                            const_uint_0,
                            const_float_0,
                            id_glsl450,
                            local_ids,
                            var_ids,
                            col_types,
                            struct_field_idx_map,
                        );
                    }
                    SpvBinaryBuilder::push_inst(&mut b.functions, 249, &[merge_lbl]);
                    SpvBinaryBuilder::push_inst(&mut b.functions, 248, &[merge_lbl]);
                }
            }
            _ => {}
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn lower_spirv_bin_expr(
        b: &mut SpvBinaryBuilder,
        expr: &Expr,
        gid_x: u32,
        ty_float: u32,
        ty_int: u32,
        ty_uint: u32,
        ty_bool: u32,
        ptr_sb_float: u32,
        ptr_ub_float: u32,
        const_uint_0: u32,
        const_float_0: u32,
        id_glsl450: u32,
        local_ids: &HashMap<String, u32>,
        var_ids: &HashMap<String, u32>,
        col_types: &HashMap<String, String>,
        struct_field_idx_map: &HashMap<(String, String), u32>,
    ) -> u32 {
        match expr {
            Expr::Ident(name, _) => local_ids.get(name).copied().unwrap_or(const_float_0),
            Expr::FloatLit(v, _) => b.get_or_emit_float_const(ty_float, *v as f32),
            Expr::IntLit(v, _) => b.get_or_emit_uint_const(ty_uint, *v as u32),
            Expr::FieldAccess {
                receiver, field, ..
            } => {
                if field == "fixed_delta" {
                    if let Expr::Ident(t_name, _) = receiver.as_ref() {
                        if let Some(&var_id) = var_ids.get(t_name) {
                            let ptr_id = b.alloc_id();
                            SpvBinaryBuilder::push_inst(
                                &mut b.functions,
                                65,
                                &[ptr_ub_float, ptr_id, var_id, const_uint_0],
                            );
                            let val_id = b.alloc_id();
                            SpvBinaryBuilder::push_inst(
                                &mut b.functions,
                                61,
                                &[ty_float, val_id, ptr_id],
                            );
                            return val_id;
                        }
                    }
                }
                if let Expr::Index {
                    receiver: col_expr, ..
                } = receiver.as_ref()
                {
                    if let Expr::Ident(col_name, _) = col_expr.as_ref() {
                        if let (Some(comp_name), Some(&var_id)) =
                            (col_types.get(col_name), var_ids.get(col_name))
                        {
                            let f_idx = struct_field_idx_map
                                .get(&(comp_name.clone(), field.clone()))
                                .copied()
                                .unwrap_or(0);
                            let const_f_idx = b.get_or_emit_uint_const(ty_uint, f_idx);
                            let ptr_id = b.alloc_id();
                            SpvBinaryBuilder::push_inst(
                                &mut b.functions,
                                65,
                                &[
                                    ptr_sb_float,
                                    ptr_id,
                                    var_id,
                                    const_uint_0,
                                    gid_x,
                                    const_f_idx,
                                ],
                            );
                            let val_id = b.alloc_id();
                            SpvBinaryBuilder::push_inst(
                                &mut b.functions,
                                61,
                                &[ty_float, val_id, ptr_id],
                            );
                            return val_id;
                        }
                    }
                }
                const_float_0
            }
            Expr::Binary {
                left, op, right, ..
            } => {
                let l = Self::lower_spirv_bin_expr(
                    b,
                    left,
                    gid_x,
                    ty_float,
                    ty_int,
                    ty_uint,
                    ty_bool,
                    ptr_sb_float,
                    ptr_ub_float,
                    const_uint_0,
                    const_float_0,
                    id_glsl450,
                    local_ids,
                    var_ids,
                    col_types,
                    struct_field_idx_map,
                );
                let r = Self::lower_spirv_bin_expr(
                    b,
                    right,
                    gid_x,
                    ty_float,
                    ty_int,
                    ty_uint,
                    ty_bool,
                    ptr_sb_float,
                    ptr_ub_float,
                    const_uint_0,
                    const_float_0,
                    id_glsl450,
                    local_ids,
                    var_ids,
                    col_types,
                    struct_field_idx_map,
                );
                let dst = b.alloc_id();
                let (opcode, res_ty) = match op {
                    BinaryOp::Add => (129u16, ty_float), // OpFAdd
                    BinaryOp::Sub => (131u16, ty_float), // OpFSub
                    BinaryOp::Mul => (133u16, ty_float), // OpFMul
                    BinaryOp::Div => (136u16, ty_float), // OpFDiv
                    BinaryOp::Rem => (140u16, ty_int),   // OpSRem
                    BinaryOp::BitAnd => (199u16, ty_int), // OpBitwiseAnd
                    BinaryOp::BitOr => (197u16, ty_int),  // OpBitwiseOr
                    BinaryOp::BitXor => (198u16, ty_int), // OpBitwiseXor
                    BinaryOp::Shl => (196u16, ty_int),    // OpShiftLeftLogical
                    BinaryOp::Shr => (195u16, ty_int),    // OpShiftRightArithmetic
                    BinaryOp::Gt => (186u16, ty_bool),    // OpFOrdGreaterThan
                    BinaryOp::GtEq => (188u16, ty_bool),  // OpFOrdGreaterThanEqual
                    BinaryOp::LtEq => (185u16, ty_bool),  // OpFOrdLessThanEqual
                    BinaryOp::Eq => (180u16, ty_bool),    // OpFOrdEqual
                    BinaryOp::NotEq => (182u16, ty_bool), // OpFOrdNotEqual
                    _ => (184u16, ty_bool),               // OpFOrdLessThan
                };
                SpvBinaryBuilder::push_inst(&mut b.functions, opcode, &[res_ty, dst, l, r]);
                dst
            }
            Expr::StaticCall {
                namespace,
                func,
                args,
                ..
            } => {
                let mut arg_ids = Vec::new();
                for a in args {
                    arg_ids.push(Self::lower_spirv_bin_expr(
                        b,
                        a,
                        gid_x,
                        ty_float,
                        ty_int,
                        ty_uint,
                        ty_bool,
                        ptr_sb_float,
                        ptr_ub_float,
                        const_uint_0,
                        const_float_0,
                        id_glsl450,
                        local_ids,
                        var_ids,
                        col_types,
                        struct_field_idx_map,
                    ));
                }
                if namespace == "math" {
                    let ext_inst_num: u32 = match func.as_str() {
                        "abs" => 4,    // FAbs
                        "sin" => 13,   // Sin
                        "cos" => 14,   // Cos
                        "sqrt" => 31,  // Sqrt
                        "clamp" => 43, // FClamp
                        "lerp" => 46,  // FMix
                        _ => 50,       // Fma
                    };
                    let dst = b.alloc_id();
                    let mut ops = vec![ty_float, dst, id_glsl450, ext_inst_num];
                    ops.extend_from_slice(&arg_ids);
                    SpvBinaryBuilder::push_inst(&mut b.functions, 12, &ops); // OpExtInst = 12
                    return dst;
                }
                const_float_0
            }
            _ => const_float_0,
        }
    }

    /// Compila uma função `@vertex_shader` do módulo para binário SPIR-V 1.5 (ExecutionModel Vertex).
    pub fn emit_spirv_vertex(module: &Module) -> Vec<u8> {
        Self::emit_spirv_graphics_stage(module, true)
    }

    /// Compila uma função `@fragment_shader` do módulo para binário SPIR-V 1.5 (ExecutionModel Fragment).
    pub fn emit_spirv_fragment(module: &Module) -> Vec<u8> {
        Self::emit_spirv_graphics_stage(module, false)
    }

    fn emit_spirv_graphics_stage(module: &Module, is_vertex: bool) -> Vec<u8> {
        let mut b = SpvBinaryBuilder::new();
        SpvBinaryBuilder::push_inst(&mut b.capabilities, 17, &[1]); // OpCapability Shader
        let id_glsl450 = b.alloc_id();
        let mut ext_ops = vec![id_glsl450];
        ext_ops.extend(Self::encode_spirv_string("GLSL.std.450"));
        SpvBinaryBuilder::push_inst(&mut b.ext_inst_imports, 11, &ext_ops);
        SpvBinaryBuilder::push_inst(&mut b.memory_model, 14, &[0, 1]); // Logical GLSL450

        let ty_void = b.alloc_id();
        SpvBinaryBuilder::push_inst(&mut b.types_consts_globals, 19, &[ty_void]);
        let ty_bool = b.alloc_id();
        SpvBinaryBuilder::push_inst(&mut b.types_consts_globals, 20, &[ty_bool]);
        let ty_int = b.alloc_id();
        SpvBinaryBuilder::push_inst(&mut b.types_consts_globals, 21, &[ty_int, 32, 1]);
        let ty_uint = b.alloc_id();
        SpvBinaryBuilder::push_inst(&mut b.types_consts_globals, 21, &[ty_uint, 32, 0]);
        let ty_float = b.alloc_id();
        SpvBinaryBuilder::push_inst(&mut b.types_consts_globals, 22, &[ty_float, 32]);
        let ty_v2float = b.alloc_id();
        SpvBinaryBuilder::push_inst(&mut b.types_consts_globals, 23, &[ty_v2float, ty_float, 2]);
        let ty_v3float = b.alloc_id();
        SpvBinaryBuilder::push_inst(&mut b.types_consts_globals, 23, &[ty_v3float, ty_float, 3]);
        let ty_v4float = b.alloc_id();
        SpvBinaryBuilder::push_inst(&mut b.types_consts_globals, 23, &[ty_v4float, ty_float, 4]);
        let ty_fn_void = b.alloc_id();
        SpvBinaryBuilder::push_inst(&mut b.types_consts_globals, 33, &[ty_fn_void, ty_void]);

        let ptr_in_float = b.alloc_id();
        SpvBinaryBuilder::push_inst(&mut b.types_consts_globals, 32, &[ptr_in_float, 1, ty_float]);
        let ptr_in_v2 = b.alloc_id();
        SpvBinaryBuilder::push_inst(&mut b.types_consts_globals, 32, &[ptr_in_v2, 1, ty_v2float]);
        let ptr_in_v3 = b.alloc_id();
        SpvBinaryBuilder::push_inst(&mut b.types_consts_globals, 32, &[ptr_in_v3, 1, ty_v3float]);
        let ptr_in_v4 = b.alloc_id();
        SpvBinaryBuilder::push_inst(&mut b.types_consts_globals, 32, &[ptr_in_v4, 1, ty_v4float]);
        let ptr_out_v4 = b.alloc_id();
        SpvBinaryBuilder::push_inst(&mut b.types_consts_globals, 32, &[ptr_out_v4, 3, ty_v4float]);

        let const_float_0 = b.get_or_emit_float_const(ty_float, 0.0);
        let const_float_1 = b.get_or_emit_float_const(ty_float, 1.0);
        let default_v4 = b.alloc_id();
        SpvBinaryBuilder::push_inst(
            &mut b.types_consts_globals,
            44, // OpConstantComposite
            &[ty_v4float, default_v4, const_float_0, const_float_0, const_float_0, const_float_1],
        );

        let target_fn = module.items.iter().find_map(|it| match it {
            Item::Function(f)
                if (is_vertex && Self::is_vertex_shader_fn(f))
                    || (!is_vertex && Self::is_fragment_shader_fn(f)) =>
            {
                Some(f)
            }
            _ => None,
        });

        let out_var = b.alloc_id();
        if is_vertex {
            b.emit_decorate(out_var, 11, &[0]); // BuiltIn Position = 0
        } else {
            b.emit_decorate(out_var, 30, &[0]); // Location = 0
        }
        SpvBinaryBuilder::push_inst(&mut b.types_consts_globals, 59, &[ptr_out_v4, out_var, 3]);

        let fn_id = b.alloc_id();
        let entry_name = target_fn.map(|f| f.name.as_str()).unwrap_or("main");
        b.emit_name(fn_id, entry_name);

        let mut interface_ids = vec![out_var];
        let mut param_loads: Vec<(String, u32, u32)> = Vec::new();
        let mut pc_loads: Vec<(String, u32, u32, u32)> = Vec::new(); // (name, ty_mat4, ptr_pc_mat4, var_pc)
        let mut sampler_loads: Vec<(String, u32, u32)> = Vec::new(); // (name, ty_sampled_img, var_samp)
        let mut next_loc: u32 = 0;

        if let Some(f) = target_fn {
            for p in &f.params {
                if p.is_cap {
                    continue;
                }
                let is_mat4 = matches!(p.ty, TypeExpr::Math(MathType::Mat4))
                    || matches!(&p.ty, TypeExpr::Named { name, .. } if name == "Mat4" || name.ends_with("::Mat4"));
                let is_sampler = matches!(&p.ty, TypeExpr::Named { name, .. } if name == "Sampler2D" || name == "Texture2D");

                if is_mat4 {
                    let ty_mat4 = b.alloc_id();
                    SpvBinaryBuilder::push_inst(&mut b.types_consts_globals, 24, &[ty_mat4, ty_v4float, 4]); // OpTypeMatrix
                    let ty_pc_block = b.alloc_id();
                    SpvBinaryBuilder::push_inst(&mut b.types_consts_globals, 30, &[ty_pc_block, ty_mat4]); // OpTypeStruct
                    b.emit_decorate(ty_pc_block, 2, &[]); // Block
                    b.emit_member_decorate(ty_pc_block, 0, 5, &[]); // ColMajor
                    b.emit_member_decorate(ty_pc_block, 0, 35, &[0]); // Offset 0
                    b.emit_member_decorate(ty_pc_block, 0, 7, &[16]); // MatrixStride 16
                    let ptr_pc_block = b.alloc_id();
                    SpvBinaryBuilder::push_inst(&mut b.types_consts_globals, 32, &[ptr_pc_block, 9, ty_pc_block]); // StorageClass PushConstant = 9
                    let ptr_pc_mat4 = b.alloc_id();
                    SpvBinaryBuilder::push_inst(&mut b.types_consts_globals, 32, &[ptr_pc_mat4, 9, ty_mat4]);
                    let var_pc = b.alloc_id();
                    SpvBinaryBuilder::push_inst(&mut b.types_consts_globals, 59, &[ptr_pc_block, var_pc, 9]);
                    pc_loads.push((p.name.clone(), ty_mat4, ptr_pc_mat4, var_pc));
                    continue;
                }

                if is_sampler {
                    let ty_img = b.alloc_id();
                    SpvBinaryBuilder::push_inst(&mut b.types_consts_globals, 25, &[ty_img, ty_float, 1, 0, 0, 0, 1, 0]); // OpTypeImage 2D
                    let ty_sampled_img = b.alloc_id();
                    SpvBinaryBuilder::push_inst(&mut b.types_consts_globals, 26, &[ty_sampled_img, ty_img]); // OpTypeSampledImage
                    let ptr_uc_samp = b.alloc_id();
                    SpvBinaryBuilder::push_inst(&mut b.types_consts_globals, 32, &[ptr_uc_samp, 0, ty_sampled_img]); // UniformConstant = 0
                    let var_samp = b.alloc_id();
                    b.emit_decorate(var_samp, 34, &[0]); // DescriptorSet 0
                    b.emit_decorate(var_samp, 33, &[0]); // Binding 0
                    SpvBinaryBuilder::push_inst(&mut b.types_consts_globals, 59, &[ptr_uc_samp, var_samp, 0]);
                    sampler_loads.push((p.name.clone(), ty_sampled_img, var_samp));
                    continue;
                }

                let (val_ty, ptr_ty) = match &p.ty {
                    TypeExpr::Math(MathType::Vec2) => (ty_v2float, ptr_in_v2),
                    TypeExpr::Math(MathType::Vec3) => (ty_v3float, ptr_in_v3),
                    TypeExpr::Math(MathType::Vec4 | MathType::Quat) => (ty_v4float, ptr_in_v4),
                    TypeExpr::Named { name: n, .. } if n == "Vec2" || n.ends_with("::Vec2") => {
                        (ty_v2float, ptr_in_v2)
                    }
                    TypeExpr::Named { name: n, .. } if n == "Vec3" || n.ends_with("::Vec3") => {
                        (ty_v3float, ptr_in_v3)
                    }
                    TypeExpr::Named { name: n, .. }
                        if n == "Vec4"
                            || n == "Quat"
                            || n.ends_with("::Vec4")
                            || n.ends_with("::Quat") =>
                    {
                        (ty_v4float, ptr_in_v4)
                    }
                    _ => (ty_float, ptr_in_float),
                };
                let in_var = b.alloc_id();
                b.emit_decorate(in_var, 30, &[next_loc]); // Location
                next_loc += 1;
                b.emit_name(in_var, &p.name);
                SpvBinaryBuilder::push_inst(&mut b.types_consts_globals, 59, &[ptr_ty, in_var, 1]);
                interface_ids.push(in_var);
                param_loads.push((p.name.clone(), val_ty, in_var));
            }
        }

        let exec_model = if is_vertex { 0u32 } else { 4u32 }; // Vertex = 0, Fragment = 4
        let mut ep_ops = vec![exec_model, fn_id];
        ep_ops.extend(Self::encode_spirv_string(entry_name));
        ep_ops.extend_from_slice(&interface_ids);
        SpvBinaryBuilder::push_inst(&mut b.entry_points, 15, &ep_ops);
        if !is_vertex {
            SpvBinaryBuilder::push_inst(&mut b.exec_modes, 16, &[fn_id, 7]); // OriginUpperLeft = 7
        }

        SpvBinaryBuilder::push_inst(&mut b.functions, 54, &[ty_void, fn_id, 0, ty_fn_void]);
        let entry_lbl = b.alloc_id();
        SpvBinaryBuilder::push_inst(&mut b.functions, 248, &[entry_lbl]);

        let mut local_ids: HashMap<String, u32> = HashMap::new();
        for (name, val_ty, in_var) in param_loads {
            let loaded = b.alloc_id();
            SpvBinaryBuilder::push_inst(&mut b.functions, 61, &[val_ty, loaded, in_var]);
            local_ids.insert(name, loaded);
        }
        let const_int_0 = b.get_or_emit_int_const(ty_int, 0);
        for (name, ty_mat4, ptr_pc_mat4, var_pc) in pc_loads {
            let ac = b.alloc_id();
            SpvBinaryBuilder::push_inst(&mut b.functions, 65, &[ptr_pc_mat4, ac, var_pc, const_int_0]);
            let loaded = b.alloc_id();
            SpvBinaryBuilder::push_inst(&mut b.functions, 61, &[ty_mat4, loaded, ac]);
            local_ids.insert(name, loaded);
        }
        for (name, ty_sampled_img, var_samp) in sampler_loads {
            let loaded = b.alloc_id();
            SpvBinaryBuilder::push_inst(&mut b.functions, 61, &[ty_sampled_img, loaded, var_samp]);
            local_ids.insert(name, loaded);
        }

        let mut ret_val_id = default_v4;
        if let Some(f) = target_fn {
            let dummy_sb_map: HashMap<String, u32> = HashMap::new();
            let dummy_col_types: HashMap<String, String> = HashMap::new();
            let dummy_field_map: HashMap<(String, String), u32> = HashMap::new();
            for stmt in &f.body {
                match stmt {
                    Stmt::Return {
                        value: Some(expr), ..
                    } => {
                        ret_val_id = Self::lower_spirv_graphics_expr(
                            &mut b,
                            expr,
                            ty_float,
                            ty_v4float,
                            ty_int,
                            ty_uint,
                            ty_bool,
                            const_float_0,
                            const_float_1,
                            id_glsl450,
                            &local_ids,
                        );
                    }
                    Stmt::Let { name, value, .. } => {
                        let val_id = Self::lower_spirv_graphics_expr(
                            &mut b,
                            value,
                            ty_float,
                            ty_v4float,
                            ty_int,
                            ty_uint,
                            ty_bool,
                            const_float_0,
                            const_float_1,
                            id_glsl450,
                            &local_ids,
                        );
                        local_ids.insert(name.clone(), val_id);
                    }
                    other => {
                        Self::lower_spirv_bin_stmt(
                            &mut b,
                            other,
                            0,
                            ty_float,
                            ty_int,
                            ty_uint,
                            ty_bool,
                            0,
                            0,
                            0,
                            const_float_0,
                            id_glsl450,
                            &mut local_ids,
                            &dummy_sb_map,
                            &dummy_col_types,
                            &dummy_field_map,
                        );
                    }
                }
            }
        }

        SpvBinaryBuilder::push_inst(&mut b.functions, 62, &[out_var, ret_val_id]); // OpStore
        SpvBinaryBuilder::push_inst(&mut b.functions, 253, &[]); // OpReturn
        SpvBinaryBuilder::push_inst(&mut b.functions, 56, &[]); // OpFunctionEnd
        b.finish()
    }

    #[allow(clippy::too_many_arguments)]
    fn lower_spirv_graphics_expr(
        b: &mut SpvBinaryBuilder,
        expr: &Expr,
        ty_float: u32,
        ty_v4float: u32,
        ty_int: u32,
        ty_uint: u32,
        ty_bool: u32,
        const_float_0: u32,
        const_float_1: u32,
        id_glsl450: u32,
        local_ids: &HashMap<String, u32>,
    ) -> u32 {
        match expr {
            Expr::Ident(name, _) => *local_ids.get(name).unwrap_or(&const_float_0),
            Expr::FloatLit(v, _) => b.get_or_emit_float_const(ty_float, *v as f32),
            Expr::IntLit(v, _) => b.get_or_emit_float_const(ty_float, *v as f32),
            Expr::StructInit { fields, .. } if fields.len() == 4 => {
                let mut comps = [const_float_0; 4];
                for (i, (_, fexpr)) in fields.iter().enumerate() {
                    comps[i] = Self::lower_spirv_graphics_expr(
                        b, fexpr, ty_float, ty_v4float, ty_int, ty_uint, ty_bool,
                        const_float_0, const_float_1, id_glsl450, local_ids,
                    );
                }
                let dst = b.alloc_id();
                SpvBinaryBuilder::push_inst(
                    &mut b.functions,
                    80, // OpCompositeConstruct
                    &[ty_v4float, dst, comps[0], comps[1], comps[2], comps[3]],
                );
                dst
            }
            Expr::StaticCall {
                namespace,
                func,
                args,
                ..
            } if (namespace == "vec4" && func == "vec4_new" && args.len() == 4) => {
                let mut comps = [const_float_0; 4];
                for (i, a) in args.iter().enumerate() {
                    comps[i] = Self::lower_spirv_graphics_expr(
                        b, a, ty_float, ty_v4float, ty_int, ty_uint, ty_bool,
                        const_float_0, const_float_1, id_glsl450, local_ids,
                    );
                }
                let dst = b.alloc_id();
                SpvBinaryBuilder::push_inst(
                    &mut b.functions,
                    80, // OpCompositeConstruct
                    &[ty_v4float, dst, comps[0], comps[1], comps[2], comps[3]],
                );
                dst
            }
            Expr::StaticCall { func, args, .. }
                if args.len() == 2
                    && (func == "sample"
                        || func == "texture_sample"
                        || func == "mul"
                        || func == "mat4_mul_vec4") =>
            {
                let c_name = func.as_str();
                let a0 = Self::lower_spirv_graphics_expr(
                    b, &args[0], ty_float, ty_v4float, ty_int, ty_uint, ty_bool,
                    const_float_0, const_float_1, id_glsl450, local_ids,
                );
                let a1 = Self::lower_spirv_graphics_expr(
                    b, &args[1], ty_float, ty_v4float, ty_int, ty_uint, ty_bool,
                    const_float_0, const_float_1, id_glsl450, local_ids,
                );
                let dst = b.alloc_id();
                if c_name == "sample" || c_name == "texture_sample" {
                    SpvBinaryBuilder::push_inst(&mut b.functions, 87, &[ty_v4float, dst, a0, a1]); // OpImageSampleImplicitLod
                    dst
                } else if c_name == "mul" || c_name == "mat4_mul_vec4" {
                    SpvBinaryBuilder::push_inst(&mut b.functions, 145, &[ty_v4float, dst, a0, a1]); // OpMatrixTimesVector
                    dst
                } else {
                    const_float_0
                }
            }
            Expr::FieldAccess {
                receiver, field, ..
            } => {
                let obj_id = Self::lower_spirv_graphics_expr(
                    b, receiver, ty_float, ty_v4float, ty_int, ty_uint, ty_bool,
                    const_float_0, const_float_1, id_glsl450, local_ids,
                );
                let idx: u32 = match field.as_str() {
                    "x" | "r" => 0,
                    "y" | "g" => 1,
                    "z" | "b" => 2,
                    "w" | "a" => 3,
                    _ => 0,
                };
                let dst = b.alloc_id();
                SpvBinaryBuilder::push_inst(&mut b.functions, 81, &[ty_float, dst, obj_id, idx]); // OpCompositeExtract
                dst
            }
            _ => {
                let dummy_sb: HashMap<String, u32> = HashMap::new();
                let dummy_col: HashMap<String, String> = HashMap::new();
                let dummy_fld: HashMap<(String, String), u32> = HashMap::new();
                let dummy_u0 = b.get_or_emit_uint_const(ty_uint, 0);
                let mut locals_clone = local_ids.clone();
                Self::lower_spirv_bin_expr(
                    b, expr, 0, ty_float, ty_int, ty_uint, ty_bool, 0, 0, dummy_u0,
                    const_float_0, id_glsl450, &mut locals_clone, &dummy_sb, &dummy_col, &dummy_fld,
                )
            }
        }
    }

    fn encode_spirv_string(s: &str) -> Vec<u32> {
        let mut bytes = s.as_bytes().to_vec();
        bytes.push(0); // Null terminator
        while bytes.len() % 4 != 0 {
            bytes.push(0);
        }
        bytes
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect()
    }
}
