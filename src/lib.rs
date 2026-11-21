pub mod ast;
pub mod bake_mesh;
pub mod bake_textures;
pub mod codegen_godot;
pub mod codegen_gpu;
pub mod codegen_llvm;
pub mod codegen_mlir;
pub mod codegen_native;
pub mod codegen_wasi;
pub mod codegen_ts_sdk;
pub mod codegen_unity;
pub mod codegen_unreal;
pub mod diagnostics;
pub mod gbnf_exporter;
pub mod lexer;
pub mod parser;
pub mod sema;

use ast::{Item, Module};
use diagnostics::{CompilerDiagnosticsReport, DiagnosticCategory, DiagnosticEntry};
use lexer::{tokenize, Span};
use parser::Parser;
use sema::SemanticAnalyzer;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

fn parse_module_only(source: &str, file_path: &str) -> Result<Module, Vec<DiagnosticEntry>> {
    let tokens = tokenize(source).map_err(|lex_errs| {
        lex_errs
            .into_iter()
            .map(|e| DiagnosticEntry {
                error_code: "RTS-E0101".to_string(),
                category: DiagnosticCategory::SyntaxError,
                file: file_path.to_string(),
                span: e.span,
                ast_node: "LexerToken".to_string(),
                message: e.message,
                deterministic_fixes: vec![],
            })
            .collect::<Vec<_>>()
    })?;

    let mut parser = Parser::new(tokens);
    parser.parse_module().map_err(|parse_err| {
        vec![DiagnosticEntry {
            error_code: "RTS-E0201".to_string(),
            category: DiagnosticCategory::SyntaxError,
            file: file_path.to_string(),
            span: parse_err.span,
            ast_node: "ParserNode".to_string(),
            message: parse_err.message,
            deterministic_fixes: vec![],
        }]
    })
}

/// Pipeline completo de validação (`rtsc check`) de um módulo em string (single-file / in-memory)
pub fn compile_and_check(source: &str, file_path: &str) -> (Option<Module>, CompilerDiagnosticsReport) {
    let module = match parse_module_only(source, file_path) {
        Ok(m) => m,
        Err(entries) => return (None, CompilerDiagnosticsReport::from_diagnostics(entries)),
    };

    let analyzer = SemanticAnalyzer::new(file_path);
    let report = analyzer.analyze_module(&module);
    (Some(module), report)
}

pub fn read_static_arena_mb_from_rts_toml(source_file: &Path) -> usize {
    let mut dir = source_file.parent();
    while let Some(d) = dir {
        let toml_path = d.join("rts.toml");
        if toml_path.exists() {
            if let Ok(content) = fs::read_to_string(&toml_path) {
                let mut in_memory = false;
                for raw_line in content.lines() {
                    let line = raw_line.split('#').next().unwrap_or("").trim();
                    if line.starts_with('[') && line.ends_with(']') {
                        in_memory = line.trim_matches(&['[', ']'][..]).trim() == "memory";
                        continue;
                    }
                    if in_memory || line.starts_with("static_arena_mb") {
                        if let Some((k, v)) = line.split_once('=') {
                            if k.trim() == "static_arena_mb" {
                                if let Ok(mb) = v.trim().parse::<usize>() {
                                    return mb.clamp(1, 512);
                                }
                            }
                        }
                    }
                }
            }
            break;
        }
        dir = d.parent();
    }
    16
}

fn resolve_import_path(entry_dir: &Path, current_file: &Path, imp_path: &[String]) -> Option<PathBuf> {
    if imp_path.is_empty() {
        return None;
    }
    let rel_nested = PathBuf::from(format!("{}.rts", imp_path.join("/")));
    let short_file = imp_path.last().map(|s| PathBuf::from(format!("{}.rts", s)));
    let current_dir = current_file.parent().unwrap_or(entry_dir);

    let mut candidates = Vec::new();
    candidates.push(current_dir.join(&rel_nested));
    candidates.push(entry_dir.join(&rel_nested));
    if let Some(short) = &short_file {
        candidates.push(current_dir.join(short));
        candidates.push(entry_dir.join(short));
    }
    if let Some(entry_parent) = entry_dir.parent() {
        candidates.push(entry_parent.join(&rel_nested));
        candidates.push(entry_parent.join("src").join(&rel_nested));
    }
    let mut anc = current_dir;
    for _ in 0..4 {
        if let Some(parent) = anc.parent() {
            candidates.push(parent.join(&rel_nested));
            candidates.push(parent.join("src").join(&rel_nested));
            anc = parent;
        } else {
            break;
        }
    }

    candidates.into_iter().find(|p| p.is_file())
}

fn rewrite_item_const_idents(
    item: &mut Item,
    renames: &std::collections::HashMap<String, String>,
) {
    match item {
        Item::Const(c) => {
            if let Some(new_name) = renames.get(&c.name) {
                c.name = new_name.clone();
            }
            rewrite_expr_const_idents(&mut c.value, renames);
        }
        Item::Function(f) => {
            for s in &mut f.body {
                rewrite_stmt_const_idents(s, renames);
            }
        }
        _ => {}
    }
}

fn rewrite_stmt_const_idents(
    stmt: &mut ast::Stmt,
    renames: &std::collections::HashMap<String, String>,
) {
    match stmt {
        ast::Stmt::Let { value, .. } | ast::Stmt::ExprStmt { expr: value, .. } => {
            rewrite_expr_const_idents(value, renames);
        }
        ast::Stmt::Assign { target, value, .. } => {
            rewrite_expr_const_idents(target, renames);
            rewrite_expr_const_idents(value, renames);
        }
        ast::Stmt::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            rewrite_expr_const_idents(condition, renames);
            for s in then_branch {
                rewrite_stmt_const_idents(s, renames);
            }
            if let Some(eb) = else_branch {
                for s in eb {
                    rewrite_stmt_const_idents(s, renames);
                }
            }
        }
        ast::Stmt::While { condition, body, .. } => {
            rewrite_expr_const_idents(condition, renames);
            for s in body {
                rewrite_stmt_const_idents(s, renames);
            }
        }
        ast::Stmt::ForRange { start, end, body, .. } => {
            rewrite_expr_const_idents(start, renames);
            rewrite_expr_const_idents(end, renames);
            for s in body {
                rewrite_stmt_const_idents(s, renames);
            }
        }
        ast::Stmt::Match { expr, arms, .. } => {
            rewrite_expr_const_idents(expr, renames);
            for arm in arms {
                for s in &mut arm.body {
                    rewrite_stmt_const_idents(s, renames);
                }
            }
        }
        ast::Stmt::Return { value: Some(v), .. } => {
            rewrite_expr_const_idents(v, renames);
        }
        _ => {}
    }
}

fn rewrite_expr_const_idents(
    expr: &mut ast::Expr,
    renames: &std::collections::HashMap<String, String>,
) {
    match expr {
        ast::Expr::Ident(name, _) => {
            if let Some(new_name) = renames.get(name) {
                *name = new_name.clone();
            }
        }
        ast::Expr::Unary { expr: inner, .. }
        | ast::Expr::Cast { expr: inner, .. }
        | ast::Expr::TryOp { expr: inner, .. }
        | ast::Expr::ErrorMapClosure { body: inner, .. } => {
            rewrite_expr_const_idents(inner, renames);
        }
        ast::Expr::Binary { left, right, .. } => {
            rewrite_expr_const_idents(left, renames);
            rewrite_expr_const_idents(right, renames);
        }
        ast::Expr::FieldAccess { receiver, .. } => {
            rewrite_expr_const_idents(receiver, renames);
        }
        ast::Expr::Index { receiver, index, .. } => {
            rewrite_expr_const_idents(receiver, renames);
            rewrite_expr_const_idents(index, renames);
        }
        ast::Expr::MethodCall { receiver, args, .. } => {
            rewrite_expr_const_idents(receiver, renames);
            for a in args {
                rewrite_expr_const_idents(a, renames);
            }
        }
        ast::Expr::StaticCall { args, .. } => {
            for a in args {
                rewrite_expr_const_idents(a, renames);
            }
        }
        ast::Expr::StructInit { fields, .. } => {
            for (_, v) in fields {
                rewrite_expr_const_idents(v, renames);
            }
        }
        ast::Expr::EnumConstructor { payload: Some(p), .. } => {
            rewrite_expr_const_idents(p, renames);
        }
        ast::Expr::ArrayLit { elements, .. } => {
            for el in elements {
                rewrite_expr_const_idents(el, renames);
            }
        }
        ast::Expr::IfExpr {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            rewrite_expr_const_idents(cond, renames);
            rewrite_expr_const_idents(then_branch, renames);
            rewrite_expr_const_idents(else_branch, renames);
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn load_module_recursive(
    entry_dir: &Path,
    file_path: &Path,
    visited: &mut HashSet<PathBuf>,
    visiting: &mut HashSet<PathBuf>,
    merged_items: &mut Vec<Item>,
    entry_module_out: &mut Option<Module>,
    is_entry: bool,
    errors: &mut Vec<DiagnosticEntry>,
) {
    let canon = fs::canonicalize(file_path).unwrap_or_else(|_| file_path.to_path_buf());
    if visited.contains(&canon) || visiting.contains(&canon) {
        return;
    }
    visiting.insert(canon.clone());

    let file_str = file_path.to_string_lossy().replace('\\', "/");
    let source = match fs::read_to_string(file_path) {
        Ok(s) => s,
        Err(err) => {
            errors.push(DiagnosticEntry {
                error_code: "RTS-E0102".to_string(),
                category: DiagnosticCategory::ModuleResolutionError,
                file: file_str,
                span: Span::new(1, 1, 1, 1, 0, 0),
                ast_node: "ModuleFile".to_string(),
                message: format!("Failed to read module '{}': {}", file_path.display(), err),
                deterministic_fixes: vec![],
            });
            visiting.remove(&canon);
            return;
        }
    };

    let module = match parse_module_only(&source, &file_str) {
        Ok(m) => m,
        Err(mut parse_errs) => {
            errors.append(&mut parse_errs);
            visiting.remove(&canon);
            return;
        }
    };

    for imp in &module.imports {
        if imp.first().map(|s| s.as_str()) == Some("core") {
            continue;
        }
        match resolve_import_path(entry_dir, file_path, imp) {
            Some(dep_path) => {
                load_module_recursive(
                    entry_dir,
                    &dep_path,
                    visited,
                    visiting,
                    merged_items,
                    entry_module_out,
                    false,
                    errors,
                );
            }
            None => {
                errors.push(DiagnosticEntry {
                    error_code: "RTS-E0102".to_string(),
                    category: DiagnosticCategory::ModuleResolutionError,
                    file: file_str.clone(),
                    span: Span::new(1, 1, 1, 1, 0, 0),
                    ast_node: format!("ImportDecl({})", imp.join(".")),
                    message: format!(
                        "Imported module '{}' could not be resolved on disk (expected '{}.rts' relative to '{}' or '{}').",
                        imp.join("."),
                        imp.join("/"),
                        file_path.parent().unwrap_or(entry_dir).display(),
                        entry_dir.display()
                    ),
                    deterministic_fixes: vec![],
                });
            }
        }
    }

    visiting.remove(&canon);
    visited.insert(canon);

    let mod_short = module
        .path
        .last()
        .cloned()
        .unwrap_or_else(|| "root".to_string());
    let mut local_const_renames: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();

    for item in &module.items {
        if let Item::Const(c) = item {
            let already_exists = merged_items.iter().any(|existing| {
                matches!(existing, Item::Const(ec) if ec.name == c.name)
            });
            if already_exists {
                let mangled = format!("{}__{}", mod_short, c.name);
                local_const_renames.insert(c.name.clone(), mangled);
            }
        }
    }

    for mut item in module.items.clone() {
        if let Item::Function(f) = &mut item {
            f.source_file = Some(file_str.clone());
        }
        if !local_const_renames.is_empty() {
            rewrite_item_const_idents(&mut item, &local_const_renames);
        }
        if let Item::Const(c) = &item {
            let qualified_alias = format!("{}__{}", mod_short, c.name);
            let alias_exists = merged_items.iter().any(|existing| {
                matches!(existing, Item::Const(ec) if ec.name == qualified_alias)
            });
            if !alias_exists && c.name != qualified_alias {
                let mut aliased = c.clone();
                aliased.name = qualified_alias;
                merged_items.push(Item::Const(aliased));
            }
        }
        merged_items.push(item);
    }

    if is_entry {
        *entry_module_out = Some(module);
    }
}

/// Pipeline multi-arquivo real em disco (`v1.2.0-rts`): resolve `import ...;` recursivamente,
/// mescla ASTs em ordem topológica para inlining/codegen C99 e valida capacidades cross-module.
pub fn compile_and_check_project(entry_file: &Path) -> (Option<Module>, CompilerDiagnosticsReport) {
    let entry_str = entry_file.to_string_lossy().replace('\\', "/");
    let entry_dir = entry_file.parent().unwrap_or_else(|| Path::new("."));
    let mut visited = HashSet::new();
    let mut visiting = HashSet::new();
    let mut merged_items = Vec::new();
    let mut entry_module_opt = None;
    let mut errors = Vec::new();

    load_module_recursive(
        entry_dir,
        entry_file,
        &mut visited,
        &mut visiting,
        &mut merged_items,
        &mut entry_module_opt,
        true,
        &mut errors,
    );

    if !errors.is_empty() {
        return (None, CompilerDiagnosticsReport::from_diagnostics(errors));
    }

    let mut merged_module = match entry_module_opt {
        Some(m) => m,
        None => {
            return (None, CompilerDiagnosticsReport::from_diagnostics(errors));
        }
    };
    merged_module.items = merged_items;

    let analyzer = SemanticAnalyzer::new(&entry_str);
    let report = analyzer.analyze_module(&merged_module);
    (Some(merged_module), report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::DeterministicFix;

    #[test]
    fn test_valid_hardware_and_physics_rts_passes_capability_checker() {
        let valid_rts = r#"
module drone.thermal_control;

struct SensorConfig {
    target_temp_celsius: f32,
    max_pwm_duty: u8,
}

enum HardwareFault {
    I2CTimeout(u8),
    SensorDisconnected,
    OverheatCritical(f32),
}

pub fn regulate_motor_cooling(
    config: SensorConfig,
    i2c: cap I2CBus<0x48>,
    pwm: cap PWMPin<4>,
    clock: cap MonotonicClock
) -> Result<f32, HardwareFault> {
    let raw_data: u16 = i2c.read_register_u16(0x00)?;

    if raw_data == 0xFFFF {
        return Err(HardwareFault::SensorDisconnected);
    }

    let current_temp: f32 = (raw_data as f32) * 0.0625;
    if current_temp >= 105.0 {
        pwm.set_duty(100);
        return Err(HardwareFault::OverheatCritical(current_temp));
    }

    pwm.set_duty(25);
    return Ok(current_temp);
}
"#;

        let (module_opt, report) =
            compile_and_check(valid_rts, "src/drone/thermal_control.rts");
        assert!(module_opt.is_some());
        assert_eq!(report.status, "ok");
        assert!(report.diagnostics.is_empty());
    }

    #[test]
    fn test_rts_e0402_blocks_privilege_escalation_in_sealed_contract() {
        let unauthorized_rts = r#"
module drone.thermal_control;

@sealed_sig
pub fn regulate_motor_cooling(
    i2c: cap I2CBus<0x48>,
    pwm: cap PWMPin<4>
) -> f32 {
    let current_temp: f32 = 42.0;
    NetworkSocket::send("192.168.1.100:9000", current_temp);
    return current_temp;
}
"#;

        let (_module_opt, report) =
            compile_and_check(unauthorized_rts, "src/drone/thermal_control.rts");
        assert_eq!(report.status, "error");
        assert_eq!(report.diagnostics.len(), 1);

        let diag = &report.diagnostics[0];
        assert_eq!(diag.error_code, "RTS-E0402");
        assert_eq!(diag.category, DiagnosticCategory::CapabilityViolation);
        // Garante que NÃO há sugestão de escalação de privilégio (add_parameter_capability) em contrato selado
        assert_eq!(diag.deterministic_fixes.len(), 1);
        match &diag.deterministic_fixes[0] {
            DeterministicFix::RemoveUnauthorizedCall { action, .. } => {
                assert_eq!(action, "remove_unauthorized_call");
            }
            _ => panic!("Sealed contract must NEVER suggest privilege escalation!"),
        }
    }

    #[test]
    fn test_rts_e0403_transitive_call_graph_capability_violation() {
        let transitive_leak_rts = r#"
module drone.telemetry;

fn registrar_log_remoto(val: f32, net: cap NetSocket) {
    NetworkSocket::send("10.0.0.1:8080", val);
}

pub fn regulate_motor_cooling(
    i2c: cap I2CBus<0x48>,
    pwm: cap PWMPin<4>
) -> f32 {
    let temp: f32 = 55.0;
    // Chamada transitiva ilegal: regulate_motor_cooling não possui 'cap NetSocket' exigido por registrar_log_remoto
    registrar_log_remoto(temp);
    return temp;
}
"#;

        let (_module_opt, report) =
            compile_and_check(transitive_leak_rts, "src/drone/telemetry.rts");
        assert_eq!(report.status, "error");
        assert_eq!(report.diagnostics[0].error_code, "RTS-E0403");
        assert_eq!(
            report.diagnostics[0].category,
            DiagnosticCategory::TransitiveCapabilityViolation
        );
    }

    #[test]
    fn test_rts_e0404_blocks_capability_forgery_attempt() {
        let forgery_rts = r#"
module security.forgery;

pub fn hijack_bus() {
    let fake_net: cap NetSocket = NetSocket::forge();
}
"#;

        let (_module_opt, report) = compile_and_check(forgery_rts, "src/security/forgery.rts");
        assert_eq!(report.status, "error");
        assert!(report.diagnostics.iter().any(|d| d.error_code == "RTS-E0404"
            && d.category == DiagnosticCategory::CapabilityForgeryAttempt));
    }

    #[test]
    fn test_rts_e0501_blocks_frame_arena_escape_into_level_arena() {
        let arena_escape_rts = r#"
module engine.memory_safety;

pub fn update_scene(
    level_cache: cap LevelArena,
    frame_scratch: cap FrameArena<TempNode>
) {
    let ephemeral_handle: Handle<TempNode> = frame_scratch.alloc_handle(1);
    // VIOLAÇÃO RTS-E0501: Salvar Handle da FrameArena (Rank 1, reseta em 16.6ms) na LevelArena (Rank 2, persistente)
    level_cache.store(ephemeral_handle);
}
"#;

        let (_module_opt, report) =
            compile_and_check(arena_escape_rts, "src/engine/memory_safety.rts");
        assert_eq!(report.status, "error");
        assert_eq!(report.diagnostics.len(), 1);
        assert_eq!(report.diagnostics[0].error_code, "RTS-E0501");
        assert_eq!(
            report.diagnostics[0].category,
            DiagnosticCategory::ArenaLifetimeEscape
        );
    }

    #[test]
    fn test_rts_e0501_blocks_read_frame_arena_index_escape() {
        let read_arena_escape_rts = r#"
module engine.consumer_escape;

pub fn leak_collision_event(
    level_cache: cap LevelArena,
    events: cap ReadFrameArena<CollisionEvent>
) {
    let ev: CollisionEvent = events[0];
    // VIOLAÇÃO RTS-E0501: Salvar elemento/handle lido via indexação de ReadFrameArena (Rank 1) na LevelArena (Rank 2)
    level_cache.store(ev);
    level_cache.store(events[0].entity);
}
"#;

        let (_module_opt, report) =
            compile_and_check(read_arena_escape_rts, "src/engine/consumer_escape.rts");
        assert_eq!(report.status, "error");
        assert_eq!(report.diagnostics.len(), 2);
        assert!(report
            .diagnostics
            .iter()
            .all(|d| d.error_code == "RTS-E0501"
                && d.category == DiagnosticCategory::ArenaLifetimeEscape));
    }

    #[test]
    fn test_native_codegen_v080_thermal_control_try_op_and_enum_payload() {
        let thermal_rts = include_str!("../examples/thermal_control.rts");
        let (module_opt, report) =
            compile_and_check(thermal_rts, "examples/thermal_control.rts");
        assert_eq!(report.status, "ok");
        let c99 = codegen_native::NativeCodegen::emit_c99(module_opt.as_ref().unwrap());
        assert!(c99.contains("rts_hw_u16_result_t _rts_try_raw_data = rts_i2c_read_register_u16(i2c, (uint8_t)(0u));"));
        assert!(c99.contains("uint16_t raw_data = _rts_try_raw_data.val.ok;"));
        assert!(c99.contains(".tag = HardwareFault_I2CTimeout, .data.I2CTimeout_val = 72u"));
        assert!(c99.contains(".tag = HardwareFault_OverheatCritical, .data.OverheatCritical_val = current_temp"));
        assert!(c99.contains("rts_pwm_set_duty(pwm, (uint8_t)(100u));"));
    }

    #[test]
    fn test_rts_e0301_type_checker_rejects_incompatible_types() {
        let bad_types_rts = r#"
module test.type_checker;

pub fn broken_types() {
    let bad: f32 = true;
    let bad_math: f32 = 10.0 + "string";
}
"#;
        let (_module_opt, report) = compile_and_check(bad_types_rts, "src/test/type_checker.rts");
        assert_eq!(report.status, "error");
        assert!(report.diagnostics.len() >= 2);
        assert!(report
            .diagnostics
            .iter()
            .all(|d| d.error_code == "RTS-E0301"
                && d.category == DiagnosticCategory::TypeMismatch));
    }

    #[test]
    fn test_rts_e0302_match_exhaustiveness_blocks_missing_enum_variant() {
        let non_exhaustive_rts = r#"
module drone.exhaustiveness;

enum HardwareFault {
    I2CTimeout(u8),
    SensorDisconnected,
    OverheatCritical(f32),
}

pub fn handle_fault(fault: HardwareFault) -> u8 {
    match fault {
        HardwareFault::I2CTimeout(addr) => {
            return addr;
        },
        HardwareFault::SensorDisconnected => {
            return 1;
        },
    }
}
"#;
        let (_module_opt, report) =
            compile_and_check(non_exhaustive_rts, "src/drone/exhaustiveness.rts");
        assert_eq!(report.status, "error");
        assert_eq!(report.diagnostics.len(), 1);
        assert_eq!(report.diagnostics[0].error_code, "RTS-E0302");
        assert_eq!(
            report.diagnostics[0].category,
            DiagnosticCategory::NonExhaustiveMatch
        );
        assert!(report.diagnostics[0]
            .message
            .contains("HardwareFault::OverheatCritical"));
    }

    #[test]
    fn test_native_codegen_v090_exhaustive_match_lowers_to_c99_switch() {
        let thermal_rts = include_str!("../examples/thermal_control.rts");
        let (module_opt, report) =
            compile_and_check(thermal_rts, "examples/thermal_control.rts");
        assert_eq!(report.status, "ok");
        let c99 = codegen_native::NativeCodegen::emit_c99(module_opt.as_ref().unwrap());
        assert!(c99.contains("switch (_rts_match_1.tag)"));
        assert!(c99.contains("case HardwareFault_I2CTimeout:"));
        assert!(c99.contains("const uint8_t addr = _rts_match_1.data.I2CTimeout_val;"));
        assert!(c99.contains("case HardwareFault_SensorDisconnected:"));
        assert!(c99.contains("case HardwareFault_OverheatCritical:"));
        assert!(c99.contains("const float temp = _rts_match_1.data.OverheatCritical_val;"));
    }

    #[test]
    fn test_rts_e0301_return_and_struct_field_type_mismatch() {
        let bad_rts = r#"
module test.return_and_struct_types;

struct SensorConfig {
    target_temp_celsius: f32,
    max_pwm_duty: u8,
}

struct Transform {
    px: f32,
}

struct Velocity {
    vx: f32,
}

enum HardwareFault {
    SensorDisconnected,
}

pub fn bad_types(h_vel: Handle<Velocity>) -> Result<f32, HardwareFault> {
    let cfg = SensorConfig {
        target_temp_celsius: "not_a_float",
        unknown_field: 10,
    };
    let h_trans: Handle<Transform> = h_vel;
    return Ok("wrong_ok_payload");
}
"#;
        let (_module_opt, report) =
            compile_and_check(bad_rts, "src/test/return_and_struct_types.rts");
        assert_eq!(report.status, "error");
        // 1: target_temp_celsius type mismatch, 2: unknown_field, 3: Handle<Transform> = Handle<Velocity>, 4: return Ok("wrong_ok_payload")
        assert_eq!(report.diagnostics.len(), 4);
        for diag in &report.diagnostics {
            assert_eq!(diag.error_code, "RTS-E0301");
            assert_eq!(diag.category, DiagnosticCategory::TypeMismatch);
        }
    }

    #[test]
    fn test_rts_e0302_match_exhaustiveness_on_result_and_option() {
        let bad_match_rts = r#"
module test.result_option_exhaustiveness;

enum HardwareFault {
    SensorDisconnected,
}

pub fn check_matches(res: Result<f32, HardwareFault>, opt: Option<u32>) -> f32 {
    match res {
        Ok(val) => {
            return val;
        },
    }
    match opt {
        Some(v) => {
            return v as f32;
        },
    }
    return 0.0;
}
"#;
        let (_module_opt, report) =
            compile_and_check(bad_match_rts, "src/test/result_option_exhaustiveness.rts");
        assert_eq!(report.status, "error");
        assert_eq!(report.diagnostics.len(), 2);
        assert_eq!(report.diagnostics[0].error_code, "RTS-E0302");
        assert_eq!(
            report.diagnostics[0].category,
            DiagnosticCategory::NonExhaustiveMatch
        );
        assert!(report.diagnostics[0].message.contains("Err(_)"));
        assert_eq!(report.diagnostics[1].error_code, "RTS-E0302");
        assert!(report.diagnostics[1].message.contains("None"));
    }

    #[test]
    fn test_v100_while_loop_and_imports_and_result_match_codegen() {
        let v100_rts = r#"
module drone.v100_release;
import core.hardware;
import core.math;

enum HardwareFault {
    I2CTimeout(u8),
    SensorDisconnected,
}

pub fn read_sensor(raw: u16) -> Result<f32, HardwareFault> {
    if raw == 0xFFFF {
        return Err(HardwareFault::SensorDisconnected);
    }
    return Ok((raw as f32) * 0.0625);
}

pub fn supervise_sensor(max_retries: u32, raw: u16) -> Result<f32, HardwareFault> {
    let mut attempt: u32 = 0;
    while attempt < max_retries {
        attempt = attempt + 1;
    }
    let val: f32 = self::read_sensor(raw)?;
    let probe: Result<f32, HardwareFault> = self::read_sensor(raw);
    match probe {
        Ok(temp) => {
            return Ok(temp + val);
        },
        Err(fault) => {
            return Err(fault);
        },
    }
}
"#;
        let (module_opt, report) = compile_and_check(v100_rts, "src/drone/v100_release.rts");
        assert_eq!(report.status, "ok");
        let module = module_opt.unwrap();
        assert_eq!(module.imports.len(), 2);
        assert_eq!(module.imports[0], vec!["core", "hardware"]);
        assert_eq!(module.imports[1], vec!["core", "math"]);

        let c99 = codegen_native::NativeCodegen::emit_c99(&module);
        assert!(c99.contains("while (attempt < max_retries)"));
        assert!(c99.contains("rts_result_float_HardwareFault_t_t _rts_try_val = drone_v100_release_read_sensor(raw);"));
        assert!(c99.contains("if (_rts_match_1.is_ok)"));
        assert!(c99.contains("const float temp = _rts_match_1.val.ok;"));
        assert!(c99.contains("const HardwareFault_t fault = _rts_match_1.val.err;"));
    }

    #[test]
    fn test_v110_utf8_bom_stripping_in_lexer() {
        let bom_rts = "\u{feff}module test.bom;\npub fn ok_fn() -> u32 { return 42; }\n";
        let (module_opt, report) = compile_and_check(bom_rts, "src/test/bom.rts");
        assert!(module_opt.is_some());
        assert_eq!(report.status, "ok");
    }

    #[test]
    fn test_v110_autonomous_main_entrypoint_and_root_capabilities() {
        let standalone_rts = r#"
module app.standalone;
import core.hardware;
import core.math;

struct Limites {
    max_temp: f32,
}

enum FalhaSensor {
    Offline(u8),
    Critico(f32),
}

pub fn monitorar(
    lim: Limites,
    i2c: cap I2CBus<0x48>,
    cooler: cap PWMPin<4>
) -> Result<f32, FalhaSensor> {
    let bruto: u16 = i2c.read_register_u16(0x00)
        .map_err(|_| FalhaSensor::Offline(0x48))?;
    let temp: f32 = (bruto as f32) * 0.0625;
    if temp > lim.max_temp {
        cooler.set_duty(100);
        return Err(FalhaSensor::Critico(temp));
    }
    cooler.set_duty(50);
    return Ok(temp);
}

pub fn main() -> i32 {
    let lim: Limites = Limites { max_temp: 85.0 };
    let i2c: cap I2CBus<0x48> = I2CBus::new(0x48);
    let cooler: cap PWMPin<4> = PWMPin::new(4);
    let res: Result<f32, FalhaSensor> = self::monitorar(lim, i2c, cooler);
    match res {
        Ok(t) => {
            io::print_f32("Temp: ", t);
            return 0;
        },
        Err(_) => {
            io::println("Fault!");
            return 1;
        },
    }
}
"#;
        let (module_opt, report) = compile_and_check(standalone_rts, "src/app/main.rts");
        assert_eq!(report.status, "ok", "Diagnostics: {:?}", report.diagnostics);
        let c99 = codegen_native::NativeCodegen::emit_c99(module_opt.as_ref().unwrap());
        assert!(c99.contains("rts_i2c_bus_t _i2c_storage = { .device_address = 72u"));
        assert!(c99.contains("rts_pwm_pin_t _cooler_storage = { .channel = 4u"));
        assert!(c99.contains("int main(void) {\n    return (int)app_standalone_main();\n}"));
    }

    #[test]
    fn test_v120_option_codegen_and_break_continue_and_fixed_arrays() {
        let rts = r#"
module app.v120_features;
import core.io;

struct Filtro {
    pesos: [f32; 3],
}

fn buscar_leitura(ativa: bool) -> Option<f32> {
    if ativa {
        return Some(42.5);
    }
    return None;
}

pub fn main() -> i32 {
    let f: Filtro = Filtro { pesos: [1.0, 2.0, 3.0] };
    let local_arr: [f32; 2] = [f.pesos[0], f.pesos[2]];
    let mut i: u32 = 0;
    let mut acc: f32 = 0.0;
    while i < 5 {
        i = i + 1;
        if i == 2 {
            continue;
        }
        if i == 4 {
            break;
        }
        acc = acc + local_arr[0];
    }

    let opt: Option<f32> = self::buscar_leitura(true);
    match opt {
        Some(v) => {
            io::print_f32("Valor=", v + acc);
            return 0;
        },
        None => {
            io::println("Vazio");
            return 1;
        },
    }
}
"#;
        let (module_opt, report) = compile_and_check(rts, "src/v120_features.rts");
        assert_eq!(report.status, "ok", "Diagnostics: {:?}", report.diagnostics);
        let c99 = codegen_native::NativeCodegen::emit_c99(module_opt.as_ref().unwrap());
        assert!(c99.contains("rts_option_float_t;"), "C99 missing Option typedef:\n{}", c99);
        assert!(c99.contains("float pesos[3];"), "C99 missing struct array field:\n{}", c99);
        assert!(c99.contains("float local_arr[2] ="), "C99 missing local array:\n{}", c99);
        assert!(c99.contains("break;"), "C99 missing break");
        assert!(c99.contains("continue;"), "C99 missing continue");
        assert!(c99.contains(".is_some = true"), "C99 missing Some constructor");
        assert!(c99.contains(".is_some = false"), "C99 missing None constructor");
        assert!(c99.contains("if (_rts_match_1.is_some)"), "C99 missing Option match");
    }

    #[test]
    fn test_v120_multi_file_import_resolution_and_cross_module_capabilities() {
        let base = std::env::temp_dir().join("rustts_v120_multifile_test");
        let _ = fs::remove_dir_all(&base);
        let src_dir = base.join("src");
        let sensores_dir = src_dir.join("sensores");
        fs::create_dir_all(&sensores_dir).unwrap();

        fs::write(
            sensores_dir.join("termico.rts"),
            r#"
module sensores.termico;
import core.hardware;

pub fn ler_celsius(i2c: cap I2CBus<0x48>) -> Result<f32, u8> {
    let raw: u16 = i2c.read_register_u16(0x00)?;
    return Ok((raw as f32) * 0.0625);
}
"#,
        )
        .unwrap();

        let main_ok = src_dir.join("main.rts");
        fs::write(
            &main_ok,
            r#"
module app.main;
import core.hardware;
import core.io;
import sensores.termico;

pub fn executar(i2c: cap I2CBus<0x48>) -> Result<f32, u8> {
    let t: f32 = termico::ler_celsius(i2c)?;
    return Ok(t);
}

pub fn main() -> i32 {
    let i2c: cap I2CBus<0x48> = I2CBus::new(0x48);
    let r: Result<f32, u8> = self::executar(i2c);
    match r {
        Ok(v) => {
            io::print_f32("T=", v);
            return 0;
        },
        Err(_) => {
            return 1;
        },
    }
}
"#,
        )
        .unwrap();

        let (mod_ok, rep_ok) = compile_and_check_project(&main_ok);
        assert_eq!(rep_ok.status, "ok", "Diagnostics: {:?}", rep_ok.diagnostics);
        let c99 = codegen_native::NativeCodegen::emit_c99(mod_ok.as_ref().unwrap());
        assert!(c99.contains("app_main_ler_celsius(i2c)"), "Cross-module call prefix mismatch:\n{}", c99);

        // Now omit `i2c` cap in caller signature and verify cross-module RTS-E0403!
        let main_bad = src_dir.join("main_bad.rts");
        fs::write(
            &main_bad,
            r#"
module app.main_bad;
import sensores.termico;

pub fn violacao_cruzada() -> Result<f32, u8> {
    let i2c: cap I2CBus<0x48> = I2CBus::new(0x48);
    let t: f32 = termico::ler_celsius(i2c)?;
    return Ok(t);
}
"#,
        )
        .unwrap();

        let (_mod_bad, rep_bad) = compile_and_check_project(&main_bad);
        assert_eq!(rep_bad.status, "error");
        assert!(
            rep_bad
                .diagnostics
                .iter()
                .any(|d| d.error_code == "RTS-E0403" || d.error_code == "RTS-E0404"),
            "Expected cross-module capability violation, got: {:?}",
            rep_bad.diagnostics
        );

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_rts_e0102_missing_imported_module_on_disk() {
        let base = std::env::temp_dir().join("rustts_v120_missing_import");
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        let entry = base.join("main.rts");
        fs::write(
            &entry,
            r#"
module app.missing_test;
import sensores.inexistente;

pub fn main() -> i32 {
    return 0;
}
"#,
        )
        .unwrap();

        let (_mod_opt, report) = compile_and_check_project(&entry);
        assert_eq!(report.status, "error");
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.error_code == "RTS-E0102"
                    && d.category == DiagnosticCategory::ModuleResolutionError),
            "Expected RTS-E0102, got: {:?}",
            report.diagnostics
        );

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_v130_extern_c_ffi_capability_guard_and_zero_overhead_stripping() {
        let valid_ffi_rts = r#"
module app.v130_ffi;

extern "C" fn puts(s: str, console: cap GPIOPin<1>) -> i32;
extern "C" fn abs(x: i32) -> i32;

fn saudar(pin: cap GPIOPin<1>) -> i32 {
    let rc: i32 = puts("hello", pin);
    let val: i32 = abs(-42);
    return rc + val;
}
"#;
        let (mod_ok, rep_ok) = compile_and_check(valid_ffi_rts, "src/v130_ffi.rts");
        assert_eq!(rep_ok.status, "ok", "Diagnostics: {:?}", rep_ok.diagnostics);
        let c99 = codegen_native::NativeCodegen::emit_c99(mod_ok.as_ref().unwrap());
        assert!(
            c99.contains("extern int32_t puts(const char* s);"),
            "FFI prototype should strip phantom cap param:\n{}",
            c99
        );
        assert!(
            c99.contains("puts(\"hello\")"),
            "FFI call should strip phantom cap argument:\n{}",
            c99
        );
        assert!(c99.contains("abs((-42u))") || c99.contains("abs("));

        let invalid_ffi_rts = r#"
module app.v130_ffi_bad;

extern "C" fn puts(s: str, console: cap GPIOPin<1>) -> i32;

fn violar(fake_pin: i32) -> i32 {
    let dummy: cap GPIOPin<1> = fake_pin;
    return puts("nope", dummy);
}
"#;
        let (_mod_bad, rep_bad) = compile_and_check(invalid_ffi_rts, "src/v130_ffi_bad.rts");
        assert_eq!(rep_bad.status, "error");
        assert!(
            rep_bad
                .diagnostics
                .iter()
                .any(|d| d.error_code == "RTS-E0403"),
            "Expected RTS-E0403 for unauthorized FFI call, got: {:?}",
            rep_bad.diagnostics
        );
    }

    #[test]
    fn test_v130_struct_impl_methods_and_core_math_stdlib() {
        let rts = r#"
module app.v130;
import core.math;

struct Oscilador {
    freq: f32,
    amp: f32,
}

impl Oscilador {
    pub fn amostrar(self: Oscilador, t: f32) -> f32 {
        return math::lerp(0.0, self.amp, math::abs(math::sin(t * self.freq)));
    }
}

pub fn main() -> i32 {
    let osc: Oscilador = Oscilador { freq: 2.0, amp: 10.0 };
    let v: f32 = osc.amostrar(0.5);
    if v > 0.0 {
        return 0;
    }
    return 1;
}
"#;
        let (mod_ok, report) = compile_and_check(rts, "src/v130_impl.rts");
        assert_eq!(report.status, "ok", "Diagnostics: {:?}", report.diagnostics);
        let c99 = codegen_native::NativeCodegen::emit_c99(mod_ok.as_ref().unwrap());
        assert!(c99.contains("math_lerp(0.0f"), "Missing math_lerp:\n{}", c99);
        assert!(c99.contains("math_abs(math_sin("), "Missing math_abs/math_sin:\n{}", c99);
        assert!(
            c99.contains("app_v130_amostrar(osc, 0.5f)"),
            "Missing impl method call lowering:\n{}",
            c99
        );
    }

    #[test]
    fn test_v140_line_directives_and_c_header_generation() {
        let rts = r#"
module app.v140;

pub fn somar_seguro(a: f32, b: f32) -> f32 {
    let soma: f32 = a + b;
    return soma;
}

fn privada(x: i32) -> i32 {
    return x * 2;
}
"#;
        let (mod_ok, report) = compile_and_check(rts, "src/v140_debug.rts");
        assert_eq!(report.status, "ok", "Diagnostics: {:?}", report.diagnostics);
        let module = mod_ok.as_ref().unwrap();

        let c99 = codegen_native::NativeCodegen::emit_c99(module);
        assert!(
            c99.contains("#line ") && c99.contains("app/v140.rts\""),
            "Missing #line source-level debug directives:\n{}",
            c99
        );

        let header = codegen_native::NativeCodegen::emit_c_header(module);
        assert!(header.contains("#pragma once"), "Missing #pragma once");
        assert!(header.contains("extern \"C\""), "Missing extern \"C\"");
        assert!(
            header.contains("float app_v140_somar_seguro(float a, float b);"),
            "Missing public prototype in C header:\n{}",
            header
        );
        assert!(
            !header.contains("privada"),
            "Private function leaked into public C header:\n{}",
            header
        );
    }

    #[test]
    fn test_v140_vscode_grammar_and_kanori_multi_file_showcase() {
        let grammar_path = Path::new("editors/vscode-rustts/syntaxes/rustts.tmLanguage.json");
        let pkg_path = Path::new("editors/vscode-rustts/package.json");
        assert!(grammar_path.exists(), "VS Code TextMate grammar missing");
        assert!(pkg_path.exists(), "VS Code package.json missing");
        let grammar_str = fs::read_to_string(grammar_path).unwrap();
        assert!(grammar_str.contains("source.rustts"));
        assert!(grammar_str.contains("@[a-zA-Z_]"));

        let kanori_main = Path::new("examples/kanori_zone_sim/src/main.rts");
        assert!(kanori_main.exists(), "KANORI showcase main.rts missing");
        let (mod_ok, report) = compile_and_check_project(kanori_main);
        assert_eq!(
            report.status, "ok",
            "KANORI multi-file showcase diagnostics: {:?}",
            report.diagnostics
        );
        let c99 = codegen_native::NativeCodegen::emit_c99(mod_ok.as_ref().unwrap());
        assert!(c99.contains("kanori_avaliar_setor"));
        assert!(c99.contains("math_sqrt"));
        assert!(c99.contains("int main(void)"));
    }

    #[test]
    fn test_v150_typescript_ergonomics_type_aliases_and_let_inference() {
        let src = r#"
module teste.ergonomia_ts;
import core.io;

@component
struct Telemetria {
    id: uint,
    status: byte,
    ativo: boolean,
    temperatura: number,
    rotulo: string,
}

impl Telemetria {
    @sealed_sig
    pub fn criar(id: int, temp: float, rotulo: string) -> Telemetria {
        return Telemetria {
            id: 42,
            status: 1,
            ativo: true,
            temperatura: temp,
            rotulo: rotulo,
        };
    }

    @sealed_sig
    pub fn ler_temperatura(self: Telemetria) -> number {
        return self.temperatura;
    }
}

pub fn demonstrar_todos_os_aliases(
    pino: uint8,
    leitura_adc: uint16,
    cliques: uint32,
    steam_id: uint64,
    io: cap StdIO
) -> int32 {
    let delta_curto: int8 = -10;
    let offset_mapa: int16 = -512;
    let saldo_conta: int32 = -42;
    let timestamp_epoch: int64 = 1700000000;

    let alt_sbyte: sbyte = -1;
    let alt_short: short = -2;
    let alt_ushort: ushort = 100;
    let alt_long: long = 1000;
    let alt_ulong: ulong = 2000;
    let alt_uintptr: uintptr = 0;

    let indice_buffer: size_t = 0;
    let velocidade_ar: float32 = 343.2;
    let precisao_orbital: float64 = 0.000000125;
    let alt_double: double = 1.5;
    let fisica_rede: fixed32 = 1024;

    let sensor_online: boolean = true;
    let mensagem: string = "Telemetria Validada: ";

    if sensor_online && pino > 0 && indice_buffer == 0 && steam_id > 0 && alt_uintptr == 0 && alt_ulong > 0 {
        let total = (leitura_adc as float32) + velocidade_ar + (precisao_orbital as float32) + (alt_double as float32);
        io::print_f32(mensagem, total);
        return saldo_conta + (delta_curto as int32) + (offset_mapa as int32) + (cliques as int32) + (timestamp_epoch as int32) + (alt_sbyte as int32) + (alt_short as int32) + (alt_ushort as int32) + (alt_long as int32) + fisica_rede;
    }
    return 0;
}

@sealed_sig
pub fn main() -> int {
    // Inferência completa no `let` (estilo TypeScript) sem anotação `: Tipo`
    let contador = 10;
    let delta_negativo = -5;
    let ganho = 2.5;
    let ligado = true;
    let nome = "KANORI-NODE-01";
    let pos = vec3(1.0, 2.0, 3.0);
    let pesos = [1.0, 2.0, 3.0, 4.0];
    let tele = Telemetria::criar(contador, ganho, nome);
    let temp_atual = tele.ler_temperatura();
    let id_sensor = tele.id;

    if ligado && id_sensor > 0 && temp_atual > 0.0 && pos.x > 0.0 && pesos[0] > 0.0 && delta_negativo < 0 {
        return 0;
    }
    return 1;
}
"#;
        let (mod_ok, report) = compile_and_check(src, "src/v150_ergonomics.rts");
        assert_eq!(
            report.status, "ok",
            "v1.5.0 TypeScript ergonomics diagnostics: {:?}",
            report.diagnostics
        );
        let c99 = codegen_native::NativeCodegen::emit_c99(mod_ok.as_ref().unwrap());
        assert!(c99.contains("uint32_t contador = 10u;"));
        assert!(c99.contains("int32_t delta_negativo = (-5);"));
        assert!(c99.contains("float ganho = 2.5f;"));
        assert!(c99.contains("bool ligado = true;"));
        assert!(c99.contains("const char* nome = \"KANORI-NODE-01\";"));
        assert!(c99.contains("rts_vec3_t pos = (rts_vec3_t){ 1.0f, 2.0f, 3.0f };"));
        assert!(c99.contains("float pesos[4] = { 1.0f, 2.0f, 3.0f, 4.0f };"));
        assert!(c99.contains("Telemetria_t tele = teste_ergonomia_ts_criar("));
        assert!(c99.contains("float temp_atual = teste_ergonomia_ts_ler_temperatura(tele);"));
        assert!(c99.contains("uint32_t id_sensor = tele.id;"));
        assert!(c99.contains("int8_t delta_curto = (-10);"));
        assert!(c99.contains("int16_t offset_mapa = (-512);"));
        assert!(c99.contains("int64_t timestamp_epoch = 1700000000u;"));
        assert!(c99.contains("size_t indice_buffer = 0u;"));
        assert!(c99.contains("double precisao_orbital = "));
    }

    #[test]
    fn test_v160_mlir_ssa_codegen_soa_scf_and_capabilities() {
        let src = r#"
module sim.mlir_physics;

@component
struct Position {
    px: f32,
    py: f32,
}

@component
struct Velocity {
    vx: f32,
    vy: f32,
}

struct HitEvent {
    entity_idx: u32,
    impact_speed: f32,
}

@system(stage = "FixedUpdate")
pub fn step_physics(
    pos: cap MutColumn<Position>,
    vel: cap ReadColumn<Velocity>,
    hits: cap FrameArena<HitEvent>,
    time: cap TimeStep
) {
    let dt = time.fixed_delta;
    let count = pos.len();

    for i in 0..count {
        let ny = pos[i].py + (vel[i].vy * dt);
        pos[i].py = ny;
        if ny < 0.0 {
            hits.push(HitEvent { entity_idx: i as u32, impact_speed: vel[i].vy });
        }
    }
}
"#;
        let (mod_ok, report) = compile_and_check(src, "src/v160_mlir_physics.rts");
        assert_eq!(
            report.status, "ok",
            "v1.6.0 MLIR physics diagnostics: {:?}",
            report.diagnostics
        );
        let mlir = codegen_mlir::MlirCodegen::emit_mlir(mod_ok.as_ref().unwrap());
        assert!(mlir.contains("module @sim_mlir_physics attributes"));
        assert!(mlir.contains("rts.soa_alignment = 64 : i64"));
        assert!(mlir.contains(
            "%pos: memref<?xf32, strided<[1]>> {rts.cap = \"MutColumn<Position>\", rts.access = \"read_write\", llvm.noalias, llvm.align = 64 : i64}"
        ));
        assert!(mlir.contains(
            "%vel: memref<?xf32, strided<[1]>> {rts.cap = \"ReadColumn<Velocity>\", rts.access = \"read_only\", llvm.noalias, llvm.align = 64 : i64}"
        ));
        assert!(mlir.contains("rts.arena_rank = 1 : i32"));
        assert!(mlir.contains("scf.for %i ="));
        assert!(mlir.contains("{rts.simd_tile = 256 : i32, rts.vectorize_width = 8 : i32}"));
        assert!(mlir.contains("memref.load %pos[%i] {rts.field = \"py\", rts.aligned = 64 : i64}"));
        assert!(mlir.contains("arith.mulf"));
        assert!(mlir.contains("arith.addf"));
        assert!(mlir.contains("memref.store"));
        assert!(mlir.contains("scf.if"));
        assert!(mlir.contains("\"rts.arena_push\""));
    }

    #[test]
    fn test_v160_mlir_hardware_and_ffi_capabilities() {
        let src = r#"
module hw.mlir_driver;
import core.hardware;

extern "C" fn puts(msg: string, console: cap StdIO) -> int;

pub fn monitor_bus(i2c: cap I2CBus<0x48>, pwm: cap PWMPin<0>, console: cap StdIO) -> int {
    puts("Iniciando leitura MLIR I2C", console);
    let mut tentativas: int = 0;
    while tentativas < 3 {
        pwm.set_duty(80);
        tentativas = tentativas + 1;
    }
    return tentativas;
}
"#;
        let (mod_ok, report) = compile_and_check(src, "src/v160_mlir_hw.rts");
        assert_eq!(
            report.status, "ok",
            "v1.6.0 MLIR hardware diagnostics: {:?}",
            report.diagnostics
        );
        let mlir = codegen_mlir::MlirCodegen::emit_mlir(mod_ok.as_ref().unwrap());
        assert!(mlir.contains(
            "func.func private @puts(!rts.str) -> i32 attributes { llvm.emit_c_interface, rts.ffi_abi = \"C\", rts.required_caps = [\"StdIO\"] }"
        ));
        assert!(mlir.contains("func.call @puts("));
        assert!(mlir.contains("scf.while : () -> ()"));
        assert!(mlir.contains("scf.condition("));
        assert!(mlir.contains("\"rts.hw_call\"(%pwm"));
        assert!(mlir.contains("arith.addi"));
    }

    #[test]
    fn test_v170_llvm_ir_direct_codegen_soa_and_fast_math_vectorization() {
        let src = r#"
@component
struct Transform {
    pos_x: f32,
    pos_y: f32,
    vel_x: f32,
    vel_y: f32
}

@parallel_system
fn integrate_velocity(
    t: cap MutColumn<Transform>,
    dt: cap TimeStep
) {
    let dt_val: f32 = dt.fixed_delta;
    let count: usize = t.len();
    for i in 0..count {
        t[i].pos_x = math::lerp(t[i].pos_x, t[i].vel_x, dt_val);
        t[i].pos_y = t[i].pos_y + t[i].vel_y * dt_val;
    }
}
"#;
        let (module_opt, report) = compile_and_check(src, "test_v170_soa.rts");
        assert_eq!(report.status, "ok", "{}", report.to_human_terminal());
        let module = module_opt.expect("Valid module AST");
        let ir = codegen_llvm::LlvmIrCodegen::emit_llvm_ir(&module);
        assert!(ir.contains("target datalayout ="));
        assert!(ir.contains("target triple ="));
        assert!(ir.contains("%Transform_t = type { float, float, float, float }"));
        assert!(ir.contains("%Transform_soa_column_t = type { ptr, ptr, ptr, ptr, ptr, i64, i64 }"));
        assert!(ir.contains("define dso_local void @root_integrate_velocity("));
        assert!(ir.contains("ptr noalias nocapture noundef align 64 %arg_t"));
        assert!(ir.contains("call fast float @llvm.fmuladd.f32("));
        assert!(ir.contains("fmul fast float"));
        assert!(ir.contains("fadd fast float"));
        assert!(ir.contains("!llvm.loop !0"));
        assert!(ir.contains("!\"llvm.loop.vectorize.enable\", i1 true"));
        assert!(ir.contains("!\"llvm.loop.vectorize.width\", i32 8"));
    }

    #[test]
    fn test_v170_llvm_ir_ffi_intrinsics_and_executable_main() {
        let src = r#"
extern "C" fn puts(msg: string, io: cap StdIO) -> int;

fn somar_quadrado(x: float, bias: float) -> float {
    let r = math::sqrt(x * x) + bias;
    return r;
}

fn main(io: cap StdIO) -> i32 {
    let v = somar_quadrado(9.0, 2.5);
    if v > 10.0 {
        let _rc = puts("RustTS v1.7.0 Direct LLVM IR Backend OK!", io);
    }
    return 0;
}
"#;
        let (module_opt, report) = compile_and_check(src, "test_v170_main.rts");
        assert_eq!(report.status, "ok", "{}", report.to_human_terminal());
        let module = module_opt.expect("Valid module AST");
        let ir = codegen_llvm::LlvmIrCodegen::emit_llvm_ir(&module);
        assert!(ir.contains("declare i32 @puts(ptr)"));
        assert!(ir.contains("define dso_local float @root_somar_quadrado(float %arg_x, float %arg_bias)"));
        assert!(ir.contains("call fast float @llvm.sqrt.f32("));
        assert!(ir.contains("define dso_local i32 @main()"));
        assert!(ir.contains("call i32 @puts(ptr @.str.4)"));
    }

    #[test]
    fn test_v180_gpu_sema_blocks_forbidden_cap_invalid_shader_layout_and_recursion() {
        let src = r#"
@shader_shared
struct BadGpuUniform {
    label: string,
    prec: f64
}

@gpu_kernel
fn bad_kernel(
    io: cap StdIO,
    x: f32
) -> f32 {
    io::println("forbidden host call on GPU");
    return bad_kernel(io, x - 1.0);
}
"#;
        let (_module_opt, report) = compile_and_check(src, "test_v180_gpu_sema.rts");
        assert_eq!(report.status, "error");
        let codes: Vec<&str> = report
            .diagnostics
            .iter()
            .map(|d| d.error_code.as_str())
            .collect();
        assert!(
            codes.contains(&"RTS-E0601"),
            "Expected RTS-E0601 (GpuForbiddenCapability), got {:?}",
            codes
        );
        assert!(
            codes.contains(&"RTS-E0602"),
            "Expected RTS-E0602 (GpuLayoutAlignmentViolation), got {:?}",
            codes
        );
        assert!(
            codes.contains(&"RTS-E0603"),
            "Expected RTS-E0603 (GpuRecursionOrDynamicEscape), got {:?}",
            codes
        );
    }

    #[test]
    fn test_v180_wgsl_compute_shader_codegen_storage_buffers_and_workgroup() {
        let src = r#"
@shader_shared
struct Particle {
    pos_x: f32,
    pos_y: f32,
    vel_x: f32,
    vel_y: f32
}

@gpu_kernel
pub fn simulate_particles(
    particles: cap MutColumn<Particle>,
    dt: cap TimeStep
) {
    let dt_val: f32 = dt.fixed_delta;
    let count: usize = particles.len();
    for i in 0..count {
        particles[i].pos_x = math::lerp(particles[i].pos_x, particles[i].vel_x, dt_val);
        particles[i].pos_y = particles[i].pos_y + particles[i].vel_y * dt_val;
    }
}
"#;
        let (module_opt, report) = compile_and_check(src, "test_v180_wgsl.rts");
        assert_eq!(report.status, "ok", "{}", report.to_human_terminal());
        let module = module_opt.expect("Valid module AST");
        let wgsl = codegen_gpu::GpuCodegen::emit_wgsl(&module);
        assert!(wgsl.contains("struct Particle {"));
        assert!(wgsl.contains("@group(0) @binding(0) var<storage, read_write> particles: array<Particle>;"));
        assert!(wgsl.contains("@group(0) @binding(1) var<uniform> dt: RtsTimeStep;"));
        assert!(wgsl.contains("@compute @workgroup_size(64, 1, 1)"));
        assert!(wgsl.contains("fn simulate_particles(@builtin(global_invocation_id) global_id: vec3<u32>)"));
        assert!(wgsl.contains("arrayLength(&particles)"));
        assert!(wgsl.contains("let i: u32 = global_id.x;"));
        assert!(wgsl.contains("mix(particles[i].pos_x, particles[i].vel_x, dt_val)"));
    }

    #[test]
    fn test_v180_spirv_asm_and_binary_magic_word_codegen() {
        let src = r#"
@shader_shared
struct Particle {
    pos_x: f32,
    pos_y: f32,
    vel_x: f32,
    vel_y: f32
}

@gpu_kernel
pub fn simulate_particles(
    particles: cap MutColumn<Particle>,
    dt: cap TimeStep
) {
    let dt_val: f32 = dt.fixed_delta;
    let count: usize = particles.len();
    for i in 0..count {
        particles[i].pos_x = math::lerp(particles[i].pos_x, particles[i].vel_x, dt_val);
        particles[i].pos_y = particles[i].pos_y + particles[i].vel_y * dt_val;
    }
}
"#;
        let (module_opt, report) = compile_and_check(src, "test_v180_spirv.rts");
        assert_eq!(report.status, "ok", "{}", report.to_human_terminal());
        let module = module_opt.expect("Valid module AST");

        let spvasm = codegen_gpu::GpuCodegen::emit_spirv_asm(&module);
        assert!(spvasm.contains("OpCapability Shader"));
        assert!(spvasm.contains("OpMemoryModel Logical GLSL450"));
        assert!(spvasm.contains("OpEntryPoint GLCompute %simulate_particles \"simulate_particles\" %gl_GlobalInvocationID"));
        assert!(spvasm.contains("OpExecutionMode %simulate_particles LocalSize 64 1 1"));
        assert!(spvasm.contains("OpDecorate %var_particles DescriptorSet 0"));
        assert!(spvasm.contains("OpDecorate %var_particles Binding 0"));
        assert!(spvasm.contains("OpExtInst %float %glsl450 FMix"));
        assert!(spvasm.contains("OpFMul %float"));
        assert!(spvasm.contains("OpFAdd %float"));

        let spv_bin = codegen_gpu::GpuCodegen::emit_spirv_binary(&module);
        assert!(spv_bin.len() >= 20 && spv_bin.len() % 4 == 0);
        let magic = u32::from_le_bytes([spv_bin[0], spv_bin[1], spv_bin[2], spv_bin[3]]);
        let version = u32::from_le_bytes([spv_bin[4], spv_bin[5], spv_bin[6], spv_bin[7]]);
        let generator = u32::from_le_bytes([spv_bin[8], spv_bin[9], spv_bin[10], spv_bin[11]]);
        assert_eq!(magic, 0x0723_0203);
        assert_eq!(version, 0x0001_0500);
        assert_eq!(generator, 0x5254_5343);
    }

    #[test]
    fn test_v210_graphics_vertex_and_fragment_shader_spirv_and_wgsl() {
        let src = r#"
module gpu.graphics;

struct Vec2 { x: float, y: float }
struct Vec4 { x: float, y: float, z: float, w: float }

@vertex_shader
pub fn vs_main(pos: Vec2) -> Vec4 {
    return Vec4 { x: pos.x, y: pos.y, z: 0.0, w: 1.0 };
}

@fragment_shader
pub fn fs_main(uv: Vec2) -> Vec4 {
    return Vec4 { x: uv.x, y: uv.y, z: 0.5, w: 1.0 };
}
"#;
        let (module_opt, report) = compile_and_check(src, "test_v210_graphics.rts");
        assert_eq!(report.status, "ok", "{}", report.to_human_terminal());
        let module = module_opt.unwrap();

        let wgsl = codegen_gpu::GpuCodegen::emit_wgsl(&module);
        assert!(wgsl.contains("@vertex"));
        assert!(wgsl.contains("fn vs_main("));
        assert!(wgsl.contains("@fragment"));
        assert!(wgsl.contains("fn fs_main("));

        let vs_spv = codegen_gpu::GpuCodegen::emit_spirv_vertex(&module);
        assert!(vs_spv.len() >= 20 && vs_spv.len() % 4 == 0);
        assert_eq!(
            u32::from_le_bytes([vs_spv[0], vs_spv[1], vs_spv[2], vs_spv[3]]),
            0x0723_0203
        );

        // Regressao Bug 2: parametro `pos: Vec4` (TypeExpr::Named("Vec4")) em @vertex_shader
        // deve emitir OpTypePointer Input %vec4, nunca escalar float.
        let src_vec4 = r#"
module gpu.graphics_vec4;
struct Vec4 { x: float, y: float, z: float, w: float }
@vertex_shader
pub fn vs_main(pos: Vec4) -> Vec4 {
    return Vec4 { x: pos.x, y: pos.y, z: pos.z, w: pos.w };
}
"#;
        let (m4_opt, rep4) = compile_and_check(src_vec4, "test_v210_vec4.rts");
        assert_eq!(rep4.status, "ok", "{}", rep4.to_human_terminal());
        let vs4_spv = codegen_gpu::GpuCodegen::emit_spirv_vertex(&m4_opt.unwrap());
        let words: Vec<u32> = vs4_spv
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        // Verifica que existe OpTypeVector %float 4 (opcode 23, word_count 4) e OpTypePointer Input %v4 (opcode 32, storage 1)
        let mut i = 5usize;
        let mut ty_v4_id = 0u32;
        let mut ptr_in_v4_id = 0u32;
        let mut var_in_uses_v4 = false;
        while i < words.len() {
            let wc = (words[i] >> 16) as usize;
            let op = (words[i] & 0xFFFF) as u16;
            if wc == 0 || i + wc > words.len() {
                break;
            }
            if op == 23 && wc == 4 && words[i + 3] == 4 {
                ty_v4_id = words[i + 1];
            } else if op == 32 && wc == 4 && words[i + 2] == 1 && words[i + 3] == ty_v4_id {
                ptr_in_v4_id = words[i + 1];
            } else if op == 59 && wc == 4 && words[i + 3] == 1 && words[i + 1] == ptr_in_v4_id {
                var_in_uses_v4 = true;
            }
            i += wc;
        }
        assert!(
            var_in_uses_v4,
            "Input variable for Named Vec4 must use OpTypePointer Input vec4"
        );

        let fs_spv = codegen_gpu::GpuCodegen::emit_spirv_fragment(&module);
        assert!(fs_spv.len() >= 20 && fs_spv.len() % 4 == 0);
        assert_eq!(
            u32::from_le_bytes([fs_spv[0], fs_spv[1], fs_spv[2], fs_spv[3]]),
            0x0723_0203
        );
    }

    #[test]
    fn test_v190_wasi_sema_rts_e0701_blocks_open_sig_and_hardware_caps_in_sandbox() {
        let src = r#"
module sandbox.plugin;

@open_sig
pub fn untrusted_plugin_entry(
    i2c: cap I2CBus<0x48>,
    x: f32
) -> f32 {
    return x + 1.0;
}
"#;
        let (_module_opt, report) = compile_and_check(src, "test_v190_wasi_sema.rts");
        assert_eq!(report.status, "error");
        let e0701_count = report
            .diagnostics
            .iter()
            .filter(|d| d.error_code == "RTS-E0701")
            .count();
        assert!(
            e0701_count >= 2,
            "Expected at least 2 RTS-E0701 diagnostics (@open_sig + I2CBus in sandbox), got {:?}",
            report.diagnostics
        );
    }

    #[test]
    fn test_v190_wasi_wat_codegen_fuel_metering_on_fn_and_loops() {
        let src = r#"
module sandbox.rules;

@sealed_sig
pub fn avaliar_regra_mod(
    io: cap StdIO,
    iteracoes: i32,
    ganho: f32
) -> f32 {
    let mut acc: f32 = 0.0;
    let mut i: i32 = 0;
    while i < iteracoes {
        acc = acc + ganho;
        i = i + 1;
    }
    return acc;
}
"#;
        let (module_opt, report) = compile_and_check(src, "test_v190_wat.rts");
        assert_eq!(report.status, "ok", "{}", report.to_human_terminal());
        let module = module_opt.expect("Valid module AST");
        let wat = codegen_wasi::WasiCodegen::emit_wat(&module, 50_000);
        assert!(wat.contains("(import \"wasi_snapshot_preview1\" \"fd_write\""));
        assert!(!wat.contains("clock_time_get"));
        assert!(wat.contains("(global $__rts_fuel (mut i64) (i64.const 50000))"));
        assert!(wat.contains("(func $rts_set_fuel (export \"rts_set_fuel\")"));
        assert!(wat.contains("(func $rts_get_fuel (export \"rts_get_fuel\")"));
        assert!(wat.contains("(func $avaliar_regra_mod (export \"avaliar_regra_mod\")"));
        assert!(wat.contains("(global.set $__rts_fuel (i64.sub (global.get $__rts_fuel)"));
        assert!(wat.contains("(if (i64.lt_s (global.get $__rts_fuel) (i64.const 0))"));
        assert!(wat.contains("(then (unreachable))"));
    }

    #[test]
    fn test_v190_wasm_binary_magic_header_and_leb128_sections() {
        let src = r#"
module sandbox.sim;

@sealed_sig
pub fn step_agent(clock: cap MonotonicClock, dose_inicial: f32, ganho: f32, ticks: i32) -> i32 {
    let mut acumulado: f32 = dose_inicial;
    let mut passo: i32 = 0;
    while passo < ticks {
        acumulado = acumulado + ganho;
        passo = passo + 1;
        if acumulado > 300.0 {
            return 1;
        }
    }
    return 0;
}
"#;
        let (module_opt, report) = compile_and_check(src, "test_v190_wasm.rts");
        assert_eq!(report.status, "ok", "{}", report.to_human_terminal());
        let module = module_opt.expect("Valid module AST");
        let wasm = codegen_wasi::WasiCodegen::emit_wasm_binary(&module, 100_000);
        assert!(wasm.len() >= 100);
        assert_eq!(&wasm[0..4], &[0x00, 0x61, 0x73, 0x6D]); // "\0asm"
        assert_eq!(&wasm[4..8], &[0x01, 0x00, 0x00, 0x00]); // version 1
        // Verifica assinatura binária real na Type Section: func (f32, f32, i32) -> (i32)
        let expected_sig = [0x60, 0x03, 0x7D, 0x7D, 0x7F, 0x01, 0x7F];
        assert!(
            wasm.windows(expected_sig.len())
                .any(|w| w == expected_sig),
            "Type Section do .wasm deve conter a assinatura real (f32, f32, i32) -> i32"
        );
        // Verifica opcodes reais do loop e aritmética f32/i32 na Code Section
        assert!(wasm.contains(&0x92)); // f32.add
        assert!(wasm.contains(&0x5E)); // f32.gt
        assert!(wasm.contains(&0x48)); // i32.lt_s
    }

    #[test]
    fn test_v200_kanori_heterogeneous_cpu_llvm_gpu_wgsl_spirv_and_wasi_showcase() {
        let entry_path = Path::new("examples/kanori_gpu_wasi_sim/src/main.rts");
        let (module_opt, report) = compile_and_check_project(entry_path);
        assert_eq!(report.status, "ok", "{}", report.to_human_terminal());
        let module = module_opt.expect("Valid heterogeneous KANORI v2.0.0 module");

        // 1. Native C99 + AVX2
        let c99 = codegen_native::NativeCodegen::emit_c99(&module);
        assert!(c99.contains("kanori_heterogeneous_calcular_dose_particula"));
        assert!(c99.contains("kanori_heterogeneous_decidir_evacuacao_agente"));

        // 2. MLIR SSA Dialect
        let mlir = codegen_mlir::MlirCodegen::emit_mlir(&module);
        assert!(mlir.contains("func.func public @kanori_heterogeneous_calcular_dose_particula"));
        assert!(mlir.contains("func.func public @kanori_heterogeneous_step_radiation_field"));

        // 3. Direct LLVM IR (.ll)
        let llvm_ir = codegen_llvm::LlvmIrCodegen::emit_llvm_ir(&module);
        assert!(llvm_ir.contains("%RadiationParticle_t = type { float, float, float, float, float }"));
        assert!(llvm_ir.contains("define dso_local float @kanori_heterogeneous_calcular_dose_particula"));
        assert!(llvm_ir.contains("define dso_local i32 @main()"));

        // 4. WebGPU WGSL Compute Shader
        let wgsl = codegen_gpu::GpuCodegen::emit_wgsl(&module);
        assert!(wgsl.contains("@compute @workgroup_size(64, 1, 1)"));
        assert!(wgsl.contains("fn step_radiation_field(@builtin(global_invocation_id) global_id: vec3<u32>)"));
        assert!(wgsl.contains("var<storage, read_write> particles: array<RadiationParticle>;"));

        // 5. Vulkan / DX12 SPIR-V 1.5 (.spvasm & .spv binary)
        let spvasm = codegen_gpu::GpuCodegen::emit_spirv_asm(&module);
        assert!(spvasm.contains("OpEntryPoint GLCompute %step_radiation_field"));
        let spv_bin = codegen_gpu::GpuCodegen::emit_spirv_binary(&module);
        assert_eq!(&spv_bin[0..4], &0x0723_0203u32.to_le_bytes());

        // 6. WASI Deterministic Fuel-Metered Sandbox (.wat & .wasm binary)
        let wat = codegen_wasi::WasiCodegen::emit_wat(&module, 75_000);
        assert!(wat.contains("(global $__rts_fuel (mut i64) (i64.const 75000))"));
        assert!(wat.contains("(func $decidir_evacuacao_agente (export \"decidir_evacuacao_agente\")"));
        let wasm_bin = codegen_wasi::WasiCodegen::emit_wasm_binary(&module, 75_000);
        assert_eq!(&wasm_bin[0..4], &[0x00, 0x61, 0x73, 0x6D]);
        let expected_agent_sig = [0x60, 0x03, 0x7D, 0x7D, 0x7F, 0x01, 0x7F];
        assert!(
            wasm_bin
                .windows(expected_agent_sig.len())
                .any(|w| w == expected_agent_sig),
            "O binário .wasm de kanori_gpu_wasi_sim deve codificar (f32, f32, i32) -> i32 para decidir_evacuacao_agente"
        );
    }

    #[test]
    fn test_v200_immutable_let_reassignment_blocked_by_sema() {
        let src = r#"
module safety.immutability;

@sealed_sig
pub fn tentar_mutar_let_imutavel(dose_inicial: f32) -> f32 {
    let acumulado: f32 = dose_inicial;
    acumulado = acumulado + 10.0;
    return acumulado;
}
"#;
        let (_module_opt, report) = compile_and_check(src, "test_immutable_let.rts");
        assert_eq!(report.status, "error");
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.error_code == "RTS-E0405" && d.message.contains("Cannot reassign immutable local variable 'acumulado'")),
            "Esperado erro RTS-E0405 ao reatribuir variável local declarada sem 'mut'"
        );
    }

    #[test]
    fn test_v200_compiler_version_and_full_diagnostic_taxonomy_homologation() {
        assert_eq!(diagnostics::COMPILER_VERSION, "2.3.0-rts");
        let ok_report = diagnostics::CompilerDiagnosticsReport::ok();
        assert_eq!(ok_report.compiler_version, "2.3.0-rts");
        assert_eq!(ok_report.status, "ok");
    }

    #[test]
    fn test_v210_fase1_const_bitwise_compound_ifexpr_and_dual_codegen() {
        let src = r#"
module systems.framebuffer_core;

const MAX_NATIVE_WIDTH: i32 = 1920;
pub const PIXEL_COUNT: i32 = MAX_NATIVE_WIDTH * 1080;
const COLOR_MASK: u32 = (1 << 8) | 0x0F;
const PALETTE: [u32; 4] = [0xFF0000FF, 0x00FF00FF, 0x0000FFFF, 0xFFFFFFFF];

@sealed_sig
pub fn mix_pixel(raw: u32, idx: i32, boost: bool) -> u32 {
    let mut acc: u32 = raw & COLOR_MASK;
    acc |= (PALETTE[idx % 4] >> 24) & 0xFF;
    acc ^= ~0x0F;
    acc <<= 1;
    acc += 4;
    acc -= 2;
    let delta: u32 = if boost { acc + 10 } else if idx > 1 { acc } else { 0 };
    return delta;
}
"#;
        let (module_opt, report) = compile_and_check(src, "test_v210_fase1.rts");
        assert_eq!(
            report.status, "ok",
            "Diagnósticos inesperados: {:?}",
            report.diagnostics
        );
        let module = module_opt.expect("AST válida");

        // 1. Native C99 backend
        let c99 = codegen_native::NativeCodegen::emit_c99(&module);
        assert!(c99.contains("RTS_UNUSED static const int32_t MAX_NATIVE_WIDTH = 1920u;"));
        assert!(c99.contains("RTS_UNUSED static const int32_t PIXEL_COUNT = 2073600u;"));
        assert!(c99.contains("RTS_UNUSED static const uint32_t COLOR_MASK = 271u;"));
        assert!(c99.contains("RTS_UNUSED static const uint32_t PALETTE[4] = { 4278190335u, 16711935u, 65535u, 4294967295u };"));
        assert!(c99.contains("(~15u)"));
        assert!(c99.contains("? ("));

        // 2. Direct LLVM IR backend
        let llvm_ir = codegen_llvm::LlvmIrCodegen::emit_llvm_ir(&module);
        assert!(llvm_ir.contains("@PALETTE = internal constant [4 x i32]"));
        assert!(llvm_ir.contains("getelementptr inbounds [4 x i32], ptr @PALETTE"));
        assert!(llvm_ir.contains("xor i32"));
        assert!(llvm_ir.contains("lshr i32"));
        assert!(llvm_ir.contains("shl i32"));
        assert!(llvm_ir.contains("ifexpr.then."));
        assert!(llvm_ir.contains("ifexpr.else."));

        // 3. Formal GBNF includes Fase 1 productions
        let gbnf = gbnf_exporter::export_rustts_gbnf();
        assert!(gbnf.contains("const-decl"));
        assert!(gbnf.contains("if-expr"));
        assert!(gbnf.contains("\"<<=\""));
        assert!(gbnf.contains("\"|=\""));
    }

    #[test]
    fn test_v210_fase1_const_immutability_and_bitwise_type_guards() {
        // 1. Reatribuir `const` deve emitir RTS-E0405
        let src_const_mut = r#"
module safety.const_mut;

const LIMIT: i32 = 64;

@sealed_sig
pub fn fail_reassign_const() -> i32 {
    LIMIT += 1;
    return LIMIT;
}
"#;
        let (_m1, rep1) = compile_and_check(src_const_mut, "test_const_mut.rts");
        assert_eq!(rep1.status, "error");
        assert!(
            rep1.diagnostics.iter().any(|d| d.error_code == "RTS-E0405"),
            "Esperado RTS-E0405 ao mutar const LIMIT: {:?}",
            rep1.diagnostics
        );

        // 2. Operação bitwise com float deve emitir RTS-E0301
        let src_float_bit = r#"
module safety.float_bitwise;

@sealed_sig
pub fn fail_float_shift(x: f32) -> f32 {
    let y: f32 = x << 2;
    return y;
}
"#;
        let (_m2, rep2) = compile_and_check(src_float_bit, "test_float_bit.rts");
        assert_eq!(rep2.status, "error");
        assert!(
            rep2.diagnostics.iter().any(|d| d.error_code == "RTS-E0301"),
            "Esperado RTS-E0301 ao aplicar '<<' em f32: {:?}",
            rep2.diagnostics
        );

        // 3. Bitwise NOT `~` em float deve emitir RTS-E0301
        let src_float_not = r#"
module safety.float_bitnot;

@sealed_sig
pub fn fail_float_bitnot(x: f32) -> f32 {
    return ~x;
}
"#;
        let (_m3, rep3) = compile_and_check(src_float_not, "test_float_bitnot.rts");
        assert_eq!(rep3.status, "error");
        assert!(
            rep3.diagnostics.iter().any(|d| d.error_code == "RTS-E0301"),
            "Esperado RTS-E0301 ao aplicar '~' em f32: {:?}",
            rep3.diagnostics
        );
    }

    #[test]
    fn test_v210_fase1_ifexpr_type_guards_and_nested_generics_parsing() {
        // 1. Nested generics `Option<Handle<Entity>>` must not collide with `>>` token
        let src_ok = r#"
module parser.nested_generics;

struct Entity {
    id: u32,
}

@sealed_sig
pub fn check_entity(ent: Option<Handle<Entity>>, mask: u32) -> u32 {
    let high: u32 = mask >> 8;
    let out: u32 = if high > 0 { high } else { 0 };
    return out;
}
"#;
        let (_m_ok, rep_ok) = compile_and_check(src_ok, "test_nested_generics.rts");
        assert_eq!(
            rep_ok.status, "ok",
            "Generics aninhados Option<Handle<Entity>> falharam: {:?}",
            rep_ok.diagnostics
        );

        // 2. IfExpr com condição não-booleana deve emitir RTS-E0301
        let src_bad_cond = r#"
module safety.ifexpr_cond;

@sealed_sig
pub fn bad_cond(x: i32) -> i32 {
    return if x { 1 } else { 2 };
}
"#;
        let (_m_bad1, rep_bad1) = compile_and_check(src_bad_cond, "test_ifexpr_cond.rts");
        assert_eq!(rep_bad1.status, "error");
        assert!(
            rep_bad1.diagnostics.iter().any(|d| d.error_code == "RTS-E0301"),
            "Esperado RTS-E0301 para condição inteira em IfExpr"
        );

        // 3. IfExpr com ramos incompatíveis deve emitir RTS-E0301
        let src_bad_branches = r#"
module safety.ifexpr_branches;

@sealed_sig
pub fn bad_branches(flag: bool) -> i32 {
    return if flag { 1 } else { false };
}
"#;
        let (_m_bad2, rep_bad2) = compile_and_check(src_bad_branches, "test_ifexpr_branches.rts");
        assert_eq!(rep_bad2.status, "error");
        assert!(
            rep_bad2.diagnostics.iter().any(|d| d.error_code == "RTS-E0301"),
            "Esperado RTS-E0301 para ramos incompatíveis em IfExpr"
        );
    }

    #[test]
    fn test_37_bitwise_modulo_and_compound_assign_codegen() {
        let source = r#"
module engine.color;
import core.math;

@sealed_sig
pub fn scale_argb(col: uint, factor: float, green_tint: boolean) -> uint {
    let f = math::clamp(factor, 0.0, 1.3);
    let g_mult = if green_tint { f * 1.45 } else { f };
    let b_mult = if green_tint { f * 0.65 } else { f };

    let mut mask: uint = 0xFF000000;
    let r = (math::clamp((((col >> 16) & 255) as float) * f, 0.0, 255.0) as uint) << 16;
    let g = (math::clamp((((col >> 8) & 255) as float) * g_mult, 0.0, 255.0) as uint) << 8;
    let b = (math::clamp(((col & 255) as float) * b_mult, 0.0, 255.0) as uint) % 256;

    mask |= r;
    mask |= g;
    mask |= b;
    return mask;
}
"#;

        let (ast_opt, report) = compile_and_check(source, "test_37_scale_argb.rts");
        assert_eq!(
            report.status, "ok",
            "Falha ao validar bitwise, % e if-expr: {:?}",
            report.diagnostics
        );
        let ast = ast_opt.expect("AST válida");

        // 1. Verifica emissão no backend C99 nativo
        let c99_code = codegen_native::NativeCodegen::emit_c99(&ast);
        assert!(c99_code.contains(">> 16u"));
        assert!(c99_code.contains("<< 16u"));
        assert!(c99_code.contains("% 256u"));
        assert!(c99_code.contains("?"));

        // 2. Verifica emissão direta no backend LLVM IR (Zero C)
        let llvm_ir = codegen_llvm::LlvmIrCodegen::emit_llvm_ir(&ast);
        assert!(llvm_ir.contains("lshr i32"));
        assert!(llvm_ir.contains("shl i32"));
        assert!(llvm_ir.contains("and i32"));
        assert!(llvm_ir.contains("or i32"));
        assert!(llvm_ir.contains("urem i32"));
        assert!(llvm_ir.contains("select i1"));
    }

    #[test]
    fn test_38_module_level_const_scalar_and_fixed_array() {
        let source = r#"
module game.constants;

const MAX_NATIVE_WIDTH: usize = 3840;
const KRASTOV_SAVE_MAGIC: uint = 0x5A505356;
const MOD_COSTS: [uint; 4] = [100, 250, 500, 1000];

@sealed_sig
pub fn verificar_cabecalho(magic: uint, nivel: usize) -> uint {
    if magic == KRASTOV_SAVE_MAGIC && MAX_NATIVE_WIDTH == 3840 {
        return MOD_COSTS[nivel];
    }
    return 0;
}
"#;

        let (ast_opt, report) = compile_and_check(source, "test_38_constants.rts");
        assert_eq!(
            report.status, "ok",
            "Constantes de módulo devem compilar sem erro: {:?}",
            report.diagnostics
        );
        let ast = ast_opt.expect("AST válida");
        let llvm_ir = codegen_llvm::LlvmIrCodegen::emit_llvm_ir(&ast);
        assert!(llvm_ir.contains("@MAX_NATIVE_WIDTH = internal constant i64 3840"));
        assert!(llvm_ir.contains("@KRASTOV_SAVE_MAGIC = internal constant i32 1515213654"));
        assert!(llvm_ir.contains("@MOD_COSTS = internal constant [4 x i32]"));
    }

    #[test]
    fn test_39_const_mutation_triggers_rts_e0405() {
        let source = r#"
module game.illegal_const_mut;

const MAX_PROJECTILES: uint = 32;

@sealed_sig
pub fn tentar_alterar_constante() -> uint {
    MAX_PROJECTILES += 8;
    return MAX_PROJECTILES;
}
"#;

        let (_ast_opt, report) = compile_and_check(source, "test_39_const_mut.rts");
        assert_eq!(report.status, "error");
        let err = report
            .diagnostics
            .iter()
            .find(|d| d.error_code == "RTS-E0405")
            .expect("Mutação de const deve falhar com RTS-E0405");
        assert_eq!(err.category, DiagnosticCategory::AliasingViolation);
    }

    #[test]
    fn test_40_bitwise_on_float_triggers_rts_e0301() {
        let source = r#"
module game.illegal_float_bits;

@sealed_sig
pub fn shift_invalido(x: float) -> float {
    let invalido = x << 2.0;
    return invalido;
}
"#;

        let (_ast_opt, report) = compile_and_check(source, "test_40_float_bits.rts");
        assert_eq!(report.status, "error");
        let err = report
            .diagnostics
            .iter()
            .find(|d| d.error_code == "RTS-E0301")
            .expect("Bitwise em float deve falhar com RTS-E0301");
        assert_eq!(err.category, DiagnosticCategory::TypeMismatch);
    }

    #[test]
    fn test_41_if_expr_branch_type_mismatch_triggers_rts_e0301() {
        let source = r#"
module game.illegal_if_expr;

@sealed_sig
pub fn ramos_incompativeis(ativo: boolean) -> uint {
    let valor = if ativo { 100 } else { "ERRO_TIPO" };
    return valor;
}
"#;

        let (_ast_opt, report) = compile_and_check(source, "test_41_if_expr.rts");
        assert_eq!(report.status, "error");
        assert!(
            report.diagnostics.iter().any(|d| d.error_code == "RTS-E0301"),
            "Ramos de tipos distintos no if-expr devem falhar com RTS-E0301"
        );
    }

    #[test]
    fn test_42_gbnf_grammar_includes_const_bitwise_and_if_expr() {
        let grammar = gbnf_exporter::export_rustts_gbnf();
        assert!(grammar.contains("const-decl"));
        assert!(grammar.contains("if-expr"));
        assert!(grammar.contains("\"<<\""));
        assert!(grammar.contains("\">>\""));
        assert!(grammar.contains("\"|=\""));
        assert!(grammar.contains("\"&=\""));
    }

    #[test]
    fn test_43_inout_param_mutation_and_dual_codegen() {
        let source = r#"
module zone.operator_sim;
import core.math;

struct StalkerOperator {
    hp: float,
    gas_filter_pct: float,
    rad_msv: float,
}

@sealed_sig
pub fn forward_filter_tick(stalker: inout StalkerOperator) {
    stalker.gas_filter_pct = math::clamp(stalker.gas_filter_pct - 0.5, 0.0, 100.0);
}

@sealed_sig
pub fn step_operator(stalker: inout StalkerOperator, target_dmg: inout int, incoming_rad: float) {
    forward_filter_tick(inout stalker);
    stalker.hp = math::clamp(stalker.hp - (target_dmg as float), 0.0, 100.0);
    stalker.rad_msv += incoming_rad;
    target_dmg = 0;
}

@sealed_sig
pub fn main() -> int {
    let mut s = StalkerOperator { hp: 100.0, gas_filter_pct: 50.0, rad_msv: 0.0 };
    let mut hits: int = 3;
    step_operator(inout s, inout hits, 12.5);
    return hits;
}
"#;

        let (ast_opt, report) = compile_and_check(source, "test_43_inout.rts");
        assert_eq!(
            report.status, "ok",
            "inout válido deve compilar sem erros: {:?}",
            report.diagnostics
        );
        let ast = ast_opt.expect("AST válida");

        let c99_code = codegen_native::NativeCodegen::emit_c99(&ast);
        assert!(c99_code.contains("RTS_RESTRICT stalker"));
        assert!(c99_code.contains("RTS_RESTRICT target_dmg"));
        assert!(c99_code.contains("(*stalker).gas_filter_pct"));
        assert!(c99_code.contains("(*target_dmg) = 0u"));
        assert!(c99_code.contains("&(s)"));

        let llvm_ir = codegen_llvm::LlvmIrCodegen::emit_llvm_ir(&ast);
        assert!(llvm_ir.contains("ptr noalias noundef %arg_stalker"));
        assert!(llvm_ir.contains("ptr noalias noundef %arg_target_dmg"));
    }

    #[test]
    fn test_44_mut_slice_and_read_slice_downgrade_codegen() {
        let source = r#"
module engine.framebuffer;
import core.math;

@sealed_sig
pub fn draw_rect_fast(fb: cap MutSlice<uint>, bw: usize, x0: usize, y0: usize, w: usize, h: usize, color: uint) {
    let x1 = x0 + w;
    let y1 = y0 + h;
    for y in y0..y1 {
        let row_offset = y * bw;
        for x in x0..x1 {
            fb[row_offset + x] = color;
        }
    }
}

@sealed_sig
pub fn ler_celula_mapa(grid: cap ReadSlice<byte>, idx: usize) -> byte {
    return grid[idx];
}

@sealed_sig
pub fn render_and_clear(arena: cap StaticArena, grid: cap MutSlice<byte>) -> int {
    let fb: cap MutSlice<uint> = arena.alloc_slice(1024);
    fb.fill(0);
    draw_rect_fast(fb, 32, 0, 0, 8, 8, 0xFF00FF00);
    let _cell = ler_celula_mapa(grid, 0);
    return (fb.len() as int);
}
"#;

        let (ast_opt, report) = compile_and_check(source, "test_44_slices.rts");
        assert_eq!(
            report.status, "ok",
            "MutSlice/ReadSlice e downgrade seguro devem compilar: {:?}",
            report.diagnostics
        );
        let ast = ast_opt.expect("AST válida");

        let c99_code = codegen_native::NativeCodegen::emit_c99(&ast);
        assert!(c99_code.contains("rts_slice_t fb"));
        assert!(c99_code.contains("const rts_slice_t grid"));
        assert!(c99_code.contains("RTS_SLICE_AT(uint32_t, fb,"));
        assert!(c99_code.contains("RTS_SLICE_FILL(uint32_t, fb,"));
        assert!(c99_code.contains("RTS_ARENA_ALLOC_SLICE(uint32_t,"));
        assert!(c99_code.contains("fb.len"));

        let llvm_ir = codegen_llvm::LlvmIrCodegen::emit_llvm_ir(&ast);
        assert!(llvm_ir.contains("%rts_slice_t = type { ptr, i64 }"));
        assert!(llvm_ir.contains("declare ptr @calloc(i64, i64)"));
        assert!(llvm_ir.contains("extractvalue %rts_slice_t"));
        assert!(llvm_ir.contains("getelementptr inbounds i32, ptr"));

        let grammar = gbnf_exporter::export_rustts_gbnf();
        assert!(grammar.contains("inout"));
        assert!(grammar.contains("MutSlice"));
        assert!(grammar.contains("ReadSlice"));
    }

    #[test]
    fn test_45_inout_rvalue_and_readonly_blocked_rts_e0405() {
        let source = r#"
module zone.illegal_inout;

@sealed_sig
pub fn step_val(x: inout int) {
    x += 1;
}

@sealed_sig
pub fn bad_caller() {
    let fixo: int = 10;
    step_val(inout fixo);
    step_val(42);
}
"#;

        let (_ast_opt, report) = compile_and_check(source, "test_45_illegal_inout.rts");
        assert_eq!(report.status, "error");
        let e0405_count = report
            .diagnostics
            .iter()
            .filter(|d| d.error_code == "RTS-E0405" && d.category == DiagnosticCategory::AliasingViolation)
            .count();
        assert!(
            e0405_count >= 2,
            "Binding imutável e rvalue literal em inout devem disparar RTS-E0405, obtido: {:?}",
            report.diagnostics
        );
    }

    #[test]
    fn test_46_aliasing_same_binding_twice_blocked_rts_e0405() {
        let source = r#"
module zone.illegal_aliasing;

@sealed_sig
pub fn swap_ints(a: inout int, b: inout int) {
    let tmp = a;
    a = b;
    b = tmp;
}

@sealed_sig
pub fn blend_buffers(dst: cap MutSlice<uint>, src: cap MutSlice<uint>) {
    dst[0] = src[0];
}

@sealed_sig
pub fn bad_caller(buf: cap MutSlice<uint>) {
    let mut v: int = 7;
    swap_ints(inout v, inout v);
    blend_buffers(buf, buf);
}
"#;

        let (_ast_opt, report) = compile_and_check(source, "test_46_aliasing.rts");
        assert_eq!(report.status, "error");
        let e0405_count = report
            .diagnostics
            .iter()
            .filter(|d| d.error_code == "RTS-E0405" && d.category == DiagnosticCategory::AliasingViolation)
            .count();
        assert!(
            e0405_count >= 2,
            "Aliasing O(1) de inout e MutSlice na mesma call deve disparar RTS-E0405, obtido: {:?}",
            report.diagnostics
        );
    }

    #[test]
    fn test_47_read_slice_mutation_and_escalation_blocked_rts_e0402() {
        let source = r#"
module engine.illegal_slice_write;

@sealed_sig
pub fn clear_mut(buf: cap MutSlice<byte>) {
    buf[0] = 0;
}

@sealed_sig
pub fn tentar_corromper(grid: cap ReadSlice<byte>) {
    grid[0] = 9;
    clear_mut(grid);
}
"#;

        let (_ast_opt, report) = compile_and_check(source, "test_47_read_slice.rts");
        assert_eq!(report.status, "error");
        let e0402_count = report
            .diagnostics
            .iter()
            .filter(|d| d.error_code == "RTS-E0402" && d.category == DiagnosticCategory::CapabilityViolation)
            .count();
        assert!(
            e0402_count >= 2,
            "Mutação e escalonamento de ReadSlice devem falhar com RTS-E0402, obtido: {:?}",
            report.diagnostics
        );
    }

    #[test]
    fn test_48_slice_in_struct_and_return_blocked_rts_e0501() {
        let source = r#"
module engine.illegal_slice_escape;

struct MaliciousHolder {
    leaked: cap MutSlice<uint>,
}

@sealed_sig
pub fn leak_slice(arena: cap StaticArena) -> cap MutSlice<uint> {
    let buf: cap MutSlice<uint> = arena.alloc_slice(64);
    return buf;
}
"#;

        let (_ast_opt, report) = compile_and_check(source, "test_48_slice_escape.rts");
        assert_eq!(report.status, "error");
        let e0501_count = report
            .diagnostics
            .iter()
            .filter(|d| {
                d.error_code == "RTS-E0501"
                    && d.category == DiagnosticCategory::ArenaLifetimeEscape
            })
            .count();
        assert!(
            e0501_count >= 2,
            "MutSlice em campo de struct e em return devem disparar RTS-E0501, obtido: {:?}",
            report.diagnostics
        );
    }

    #[test]
    fn test_49_fixed_string_hud_formatting_c99_and_llvm() {
        let source = r#"
module engine.hud_text;
import core.io;

@sealed_sig
pub fn format_hud(rad: float, wave: uint, delta: int, magic: uint) -> usize {
    let mut hud: str256 = str256::new();
    let tag: str128 = str128::from("KANORI-");
    hud.clear();
    hud.push_str(tag.as_str());
    hud.push_str("W:");
    hud.push_u32(wave);
    hud.push_str(" D:");
    hud.push_i32(delta);
    hud.push_str(" R:");
    hud.push_f32(rad, 2);
    hud.push_str(" ");
    hud.push_hex(magic);
    io::println(hud.as_str());
    return hud.len() + hud.capacity();
}
"#;

        let (ast_opt, report) = compile_and_check(source, "test_49_hud_text.rts");
        assert_eq!(
            report.status, "ok",
            "Falha ao compilar str256/str128 HUD formatting: {:?}",
            report.diagnostics
        );
        let module = ast_opt.unwrap();

        let c99 = codegen_native::NativeCodegen::emit_c99(&module);
        assert!(c99.contains("rts_str256_t"));
        assert!(c99.contains("rts_str128_from("));
        assert!(c99.contains("rts_fixed_str_clear("));
        assert!(c99.contains("rts_fixed_str_push_cstr("));
        assert!(c99.contains("rts_fixed_str_push_u32("));
        assert!(c99.contains("rts_fixed_str_push_i32("));
        assert!(c99.contains("rts_fixed_str_push_f32("));
        assert!(c99.contains("rts_fixed_str_push_hex("));

        let llvm = codegen_llvm::LlvmIrCodegen::emit_llvm_ir(&module);
        assert!(llvm.contains("%rts_str256_t = type { i32, [256 x i8] }"));
        assert!(llvm.contains("@__rts_fixed_str_clear"));
        assert!(llvm.contains("@__rts_fixed_str_push_cstr"));
        assert!(llvm.contains("@__rts_fixed_str_push_f32"));
        assert!(llvm.contains("@.str.fs_hex"));
    }

    #[test]
    fn test_50_repr_c_nested_structs_and_fixed_strings_abi() {
        let source = r#"
module engine.save_abi;

@repr_c
struct BunkerHubState {
    black_market_credits: int,
    status_note: str64,
}

@repr_c
struct BunkerSaveData {
    magic: uint,
    bunker: BunkerHubState,
    inventory: [int; 8],
}

@sealed_sig
pub fn patch_save(save: inout BunkerSaveData, creditos: int) {
    save.magic = 0x5A4F4E45;
    save.bunker.black_market_credits = creditos;
    save.bunker.status_note.clear();
    save.bunker.status_note.push_str("SYNC:");
    save.bunker.status_note.push_i32(creditos);
}
"#;

        let (ast_opt, report) = compile_and_check(source, "test_50_repr_c_abi.rts");
        assert_eq!(
            report.status, "ok",
            "@repr_c aninhado com str64 deveria validar sem erros: {:?}",
            report.diagnostics
        );
        let module = ast_opt.unwrap();

        let c99 = codegen_native::NativeCodegen::emit_c99(&module);
        assert!(c99.contains("BunkerHubState_t"));
        assert!(c99.contains("BunkerSaveData_t"));
        assert!(c99.contains("rts_str64_t status_note;"));

        let llvm = codegen_llvm::LlvmIrCodegen::emit_llvm_ir(&module);
        assert!(llvm.contains("%BunkerHubState_t = type"));
        assert!(llvm.contains("%BunkerSaveData_t = type"));
        assert!(llvm.contains("%rts_str64_t"));
    }

    #[test]
    fn test_51_repr_c_rejects_non_pod_and_cap_fields_rts_e0602() {
        let source = r#"
module engine.bad_repr_c;

struct UnsafeHolder {
    text_ptr: str,
}

@repr_c
struct BadSaveWithPtr {
    raw_msg: str,
    nested: UnsafeHolder,
}

@repr_c
struct BadSaveWithHandle {
    owner: Handle<UnsafeHolder>,
}
"#;

        let (_ast_opt, report) = compile_and_check(source, "test_51_bad_repr_c.rts");
        assert_eq!(report.status, "error");
        let e0602_count = report
            .diagnostics
            .iter()
            .filter(|d| {
                d.error_code == "RTS-E0602"
                    && d.category == DiagnosticCategory::GpuLayoutAlignmentViolation
            })
            .count();
        assert!(
            e0602_count >= 3,
            "Campos str, struct nao-@repr_c e Handle dentro de @repr_c devem disparar RTS-E0602, obtido: {:?}",
            report.diagnostics
        );
    }

    #[test]
    fn test_52_immutable_fixed_string_mutation_blocked_rts_e0405() {
        let source = r#"
module engine.immutable_fixed_str;

@sealed_sig
pub fn try_mutate() {
    let banner: str64 = str64::from("SEALED");
    banner.push_str("_EDIT");
    banner.clear();
}
"#;

        let (_ast_opt, report) = compile_and_check(source, "test_52_immutable_str.rts");
        assert_eq!(report.status, "error");
        let e0405_count = report
            .diagnostics
            .iter()
            .filter(|d| {
                d.error_code == "RTS-E0405"
                    && d.category == DiagnosticCategory::AliasingViolation
            })
            .count();
        assert!(
            e0405_count >= 2,
            "Mutacao em str64 sem 'let mut' deve disparar RTS-E0405, obtido: {:?}",
            report.diagnostics
        );
    }

    #[test]
    fn test_53_fixed_string_inout_param_and_accessors() {
        let source = r#"
module engine.fixed_str_inout;

@sealed_sig
pub fn append_badge(buf: inout str64, sector: uint) -> usize {
    buf.push_str("-SEC");
    buf.push_u32(sector);
    return buf.len();
}

@sealed_sig
pub fn test_caller() -> usize {
    let mut tag: str64 = str64::from("ZONE");
    let used: usize = append_badge(inout tag, 42);
    return used + tag.capacity();
}
"#;

        let (ast_opt, report) = compile_and_check(source, "test_53_fixed_str_inout.rts");
        assert_eq!(
            report.status, "ok",
            "inout str64 deveria compilar sem erros: {:?}",
            report.diagnostics
        );
        let module = ast_opt.unwrap();
        let c99 = codegen_native::NativeCodegen::emit_c99(&module);
        assert!(c99.contains("rts_str64_t* RTS_RESTRICT buf"));
        let llvm = codegen_llvm::LlvmIrCodegen::emit_llvm_ir(&module);
        assert!(llvm.contains("ptr noalias noundef %arg_buf"));
    }

    #[test]
    fn test_54_gbnf_grammar_includes_fase3_repr_c_and_fixed_strings() {
        let gbnf = gbnf_exporter::export_rustts_gbnf();
        assert!(gbnf.contains("@repr_c"));
        assert!(gbnf.contains("\"str64\""));
        assert!(gbnf.contains("\"str128\""));
        assert!(gbnf.contains("\"str256\""));
    }

    #[test]
    fn test_55_hal_window_and_input_capabilities_codegen() {
        let source = r#"
module test.hal_window;

@sealed_sig
pub fn render_step(
    win: cap WindowHost,
    inp: cap InputDevice,
    clk: cap MonotonicClock,
    fb: cap MutSlice<uint>
) -> bool {
    if !win.poll_events() {
        return false;
    }
    inp.poll();
    fb.fill(0xFF050505);
    win.present_rgba(fb);
    let mut hud: str64 = str64::from("ZONE OK");
    win.draw_text(16, 16, 0xFFFFFFFF, hud.as_str());
    let _pad: bool = inp.gamepad_connected();
    clk.tick_frame_fps(60);
    let _dt: float = clk.last_frame_ms();
    return win.width() > 0;
}

pub fn main(arena: cap StaticArena) -> int {
    let win = WindowHost::new();
    let inp = InputDevice::new();
    let clk = MonotonicClock::new();
    let fb: cap MutSlice<uint> = arena.alloc_slice(6400);
    if win.open_headless(80, 80, 3) {
        let _ok = render_step(win, inp, clk, fb);
        win.close();
    }
    return 0;
}
"#;

        let (ast_opt, report) = compile_and_check(source, "test_55_hal_window.rts");
        assert_eq!(
            report.status, "ok",
            "HAL window/input deveria compilar sem erros: {:?}",
            report.diagnostics
        );
        let checked = ast_opt.unwrap();
        let c99 = codegen_native::NativeCodegen::emit_c99(&checked);
        assert!(c99.contains("rts_window_open_headless"));
        assert!(c99.contains("rts_window_present_rgba"));
        assert!(c99.contains("rts_input_poll"));
        assert!(c99.contains("rts_clock_tick_frame_fps"));

        let llvm = codegen_llvm::LlvmIrCodegen::emit_llvm_ir(&checked);
        assert!(llvm.contains("define internal i1 @__rts_window_open_headless"));
        assert!(llvm.contains("define internal void @__rts_window_present_rgba"));
        assert!(llvm.contains("define internal void @__rts_input_poll"));
        assert!(llvm.contains("define internal float @__rts_clock_tick_frame_fps"));
    }

    #[test]
    fn test_56_hal_filesystem_and_audio_repr_c_roundtrip() {
        let source = r#"
module test.hal_fs_audio;

@repr_c
struct BunkerSave {
    magic: uint,
    hp: int,
    rad: float,
}

@sealed_sig
pub fn save_and_beep(fs: cap FileSystem, audio: cap AudioDevice, save_data: BunkerSave) -> bool {
    let _dir = fs.create_dir("scratch_test");
    let _txt = fs.write_text("scratch_test/status.txt", "KRASTOV");
    if !fs.write_struct("scratch_test/save.bin", save_data) {
        return false;
    }
    let mut loaded: BunkerSave = BunkerSave { magic: 0, hp: 0, rad: 0.0 };
    let ok = fs.read_struct("scratch_test/save.bin", loaded);
    let _wav = audio.write_synth_wav("scratch_test/geiger.wav", 22050, 440, 1);
    return ok && (loaded.magic == save_data.magic);
}

pub fn main() -> int {
    let fs = FileSystem::new();
    let audio = AudioDevice::new();
    let data = BunkerSave { magic: 0x4B524153, hp: 100, rad: 0.25 };
    let _ok = save_and_beep(fs, audio, data);
    return 0;
}
"#;

        let (ast_opt, report) = compile_and_check(source, "test_56_hal_fs_audio.rts");
        assert_eq!(
            report.status, "ok",
            "HAL filesystem/audio deveria compilar sem erros: {:?}",
            report.diagnostics
        );
        let checked = ast_opt.unwrap();
        let c99 = codegen_native::NativeCodegen::emit_c99(&checked);
        assert!(c99.contains("rts_fs_write_bytes"));
        assert!(c99.contains("rts_fs_read_bytes"));
        assert!(c99.contains("rts_audio_write_synth_wav"));

        let llvm = codegen_llvm::LlvmIrCodegen::emit_llvm_ir(&checked);
        assert!(llvm.contains("define internal i1 @__rts_fs_write_bytes"));
        assert!(llvm.contains("define internal i1 @__rts_fs_read_bytes"));
        assert!(llvm.contains("define internal i1 @__rts_audio_write_synth_wav"));
    }

    #[test]
    fn test_57_hal_filesystem_rejects_non_repr_c_and_immutable_target() {
        let source_non_repr_c = r#"
module test.hal_fs_bad_struct;

struct UnsafeSave {
    hp: int,
}

@sealed_sig
pub fn try_write(fs: cap FileSystem, s: UnsafeSave) -> bool {
    return fs.write_struct("bad.bin", s);
}
"#;
        let (_ast1, report1) = compile_and_check(source_non_repr_c, "test_57a.rts");
        assert_eq!(report1.status, "error");
        assert!(
            report1.diagnostics.iter().any(|d| {
                d.error_code == "RTS-E0602"
                    && d.category == DiagnosticCategory::GpuLayoutAlignmentViolation
            }),
            "Esperado RTS-E0602 GpuLayoutAlignmentViolation para struct sem @repr_c, obtido: {:?}",
            report1.diagnostics
        );

        let source_immutable_read = r#"
module test.hal_fs_immut_read;

@repr_c
struct GoodSave {
    hp: int,
}

@sealed_sig
pub fn try_read(fs: cap FileSystem) -> bool {
    let target = GoodSave { hp: 0 };
    return fs.read_struct("good.bin", target);
}
"#;
        let (_ast2, report2) = compile_and_check(source_immutable_read, "test_57b.rts");
        assert_eq!(report2.status, "error");
        assert!(
            report2.diagnostics.iter().any(|d| {
                d.error_code == "RTS-E0405"
                    && d.category == DiagnosticCategory::AliasingViolation
            }),
            "Esperado RTS-E0405 AliasingViolation para alvo imutavel em read_struct, obtido: {:?}",
            report2.diagnostics
        );
    }

    #[test]
    fn test_58_hal_capability_forgery_outside_main_blocked() {
        let source = r#"
module test.hal_forgery;

@sealed_sig
pub fn sneak_fs() -> bool {
    let fs = FileSystem::new();
    return fs.write_text("pwned.txt", "oops");
}
"#;
        let (_ast, report) = compile_and_check(source, "test_58_hal_forgery.rts");
        assert_eq!(report.status, "error");
        assert!(
            report.diagnostics.iter().any(|d| {
                d.error_code == "RTS-E0404"
                    && d.category == DiagnosticCategory::CapabilityForgeryAttempt
            }),
            "Esperado RTS-E0404 CapabilityForgeryAttempt fora de main, obtido: {:?}",
            report.diagnostics
        );
    }

    #[test]
    fn test_59_hal_capability_mismatch_and_undeclared_blocked() {
        let source_mismatch = r#"
module test.hal_mismatch;

@sealed_sig
pub fn needs_fs(fs: cap FileSystem) -> bool {
    return fs.write_text("ok.txt", "ok");
}

pub fn main() -> int {
    let win = WindowHost::new();
    let _bad = needs_fs(win);
    return 0;
}
"#;
        let (_ast, report) = compile_and_check(source_mismatch, "test_59_hal_mismatch.rts");
        assert_eq!(report.status, "error");
        assert!(
            report.diagnostics.iter().any(|d| {
                d.error_code == "RTS-E0402"
                    && d.category == DiagnosticCategory::CapabilityViolation
            }),
            "Esperado RTS-E0402 CapabilityViolation ao passar WindowHost para cap FileSystem, obtido: {:?}",
            report.diagnostics
        );
    }

    #[test]
    fn test_60_gbnf_grammar_includes_fase4_hal_domains() {
        let gbnf = gbnf_exporter::export_rustts_gbnf();
        for domain in [
            "\"WindowHost\"",
            "\"InputDevice\"",
            "\"FileSystem\"",
            "\"AudioDevice\"",
            "\"MonotonicClock\"",
        ] {
            assert!(
                gbnf.contains(domain),
                "Gramatica GBNF deveria conter o dominio HAL {}",
                domain
            );
        }
    }

    #[test]
    fn test_61_window_host_auto_smoke_test_detection_dual_codegen() {
        let source = r#"
module engine.smoke_guard;

@sealed_sig
pub fn iniciar_motor_seguro(
    win: cap WindowHost,
    audio: cap AudioDevice,
    clk: cap MonotonicClock
) -> int {
    if !win.open_fullscreen("KRASTOV: Zone Protocol", 1920, 1080) {
        return 1;
    }
    if !win.is_smoke_test() {
        audio.play_wav_async("assets/audio/extraction_alarm.wav");
    }
    let mut frames: int = 0;
    while win.poll_events() {
        let _dt: float = clk.tick_frame_fps(60);
        frames += 1;
    }
    win.close();
    return frames;
}

pub fn main() -> int {
    let win = WindowHost::new();
    let audio = AudioDevice::new();
    let clk = MonotonicClock::new();
    return iniciar_motor_seguro(win, audio, clk);
}
"#;

        let (ast_opt, report) = compile_and_check(source, "test_61_smoke_guard.rts");
        assert_eq!(
            report.status, "ok",
            "win.is_smoke_test() deve compilar sem erro: {:?}",
            report.diagnostics
        );
        let checked = ast_opt.unwrap();

        let c99 = codegen_native::NativeCodegen::emit_c99(&checked);
        assert!(c99.contains("rts_window_is_smoke_test"));
        assert!(c99.contains("--smoke-test"));

        let llvm = codegen_llvm::LlvmIrCodegen::emit_llvm_ir(&checked);
        assert!(llvm.contains("@__rts_window_is_smoke_test"));
        assert!(llvm.contains("--smoke-test"));
    }

    #[test]
    fn test_62_v300_pub_struct_array_repeat_const_header_and_extern_inout() {
        let source = r#"
module engine.v300_lang_features;
import core.math;

pub const BUCKET_CAP: usize = 16;

@repr_c
pub struct ArenaBox {
    pub data: [float; 16],
    pub count: usize,
}

@sealed_sig
pub extern "C" fn bridge_fill_out(seed: u64, out_val: inout u64) -> u64;

@sealed_sig
pub fn step_box(box_ref: inout ArenaBox, angle: float) -> float {
    let mut i: usize = 0;
    while i < BUCKET_CAP {
        box_ref.data[i] = math::tan(angle);
        i += 1;
    }
    let mut out_h: u64 = 0;
    let res = bridge_fill_out(42, inout out_h);
    let _zero = ArenaBox { data: [0.0; 16], count: 0 };
    return box_ref.data[0] + ((res + out_h) as float);
}
"#;

        let (ast_opt, report) = compile_and_check(source, "test_62_v300.rts");
        assert_eq!(
            report.status, "ok",
            "Novos recursos v3.0.0-PRO deveriam compilar sem erro: {:?}",
            report.diagnostics
        );
        let checked = ast_opt.unwrap();
        let c99 = codegen_native::NativeCodegen::emit_c99(&checked);
        assert!(c99.contains("math_tan"));
        assert!(c99.contains("uint64_t* RTS_RESTRICT out_val"));
        assert!(c99.contains("bridge_fill_out(") && c99.contains("&(out_h))"));

        let llvm = codegen_llvm::LlvmIrCodegen::emit_llvm_ir(&checked);
        assert!(llvm.contains("declare i64 @bridge_fill_out(i64, ptr)"));
        assert!(llvm.contains("@tanf"));
    }

    #[test]
    fn test_63_v300_engine_main_project_compiles_dual_backend() {
        let entry = Path::new("src/main.rts");
        assert!(entry.exists(), "src/main.rts deveria existir");
        let (ast_opt, report) = compile_and_check_project(&entry);
        assert_eq!(
            report.status, "ok",
            "Projeto src/main.rts deveria validar sem erros: {:?}",
            report.diagnostics
        );
        let checked = ast_opt.unwrap();
        let c99 = codegen_native::NativeCodegen::emit_c99(&checked);
        assert!(c99.contains("rhi_device_init"));
        assert!(c99.contains("camera_build_uniform"));
        let llvm = codegen_llvm::LlvmIrCodegen::emit_llvm_ir(&checked);
        assert!(llvm.contains("define dso_local i32 @main("));
        assert!(llvm.contains("rtsc_vk_alloc_vram_arena"));
        assert!(llvm.contains("llvm.memcpy.p0.p0.i64"));
    }

    #[test]
    fn test_64_core_mem_intrinsics_and_zero_c_rhi_dual_codegen() {
        let source = r#"
module test.core_mem_rhi;
import core.mem;

@repr_c
pub struct Packet {
    pub id: u64,
    pub val: float,
}

@sealed_sig
pub fn transfer_packet(buf: cap MutSlice<Packet>, src: Packet) -> u64 {
    let raw = mem::alloc_aligned(64, 256);
    mem::zero(raw, 64);
    mem::copy(raw, mem::addr_of(src), 16);
    let base = mem::slice_ptr(buf);
    mem::copy(base, raw, 16);
    mem::free_aligned(raw);
    return base;
}
"#;
        let (ast_opt, report) = compile_and_check(source, "test_64_core_mem.rts");
        assert_eq!(
            report.status, "ok",
            "core.mem deveria compilar sem erros: {:?}",
            report.diagnostics
        );
        let checked = ast_opt.unwrap();
        let c99 = codegen_native::NativeCodegen::emit_c99(&checked);
        assert!(c99.contains("rtsc_mem_alloc_aligned"));
        assert!(c99.contains("rtsc_mem_copy"));
        assert!(c99.contains("rtsc_mem_zero"));
        assert!(c99.contains("rtsc_mem_free_aligned"));
        assert!(c99.contains(".data))"));

        let llvm = codegen_llvm::LlvmIrCodegen::emit_llvm_ir(&checked);
        assert!(llvm.contains("call void @llvm.memcpy.p0.p0.i64"));
        assert!(llvm.contains("call void @llvm.memset.p0.i64"));
        assert!(llvm.contains("call void @_aligned_free(ptr"));
        assert!(llvm.contains("ptrtoint ptr"));
    }

    #[test]
    fn test_65_v300_map_pak_persistence_roundtrip_dual_codegen() {
        let entry = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/village_editor.rts");
        let (ast_opt, report) = compile_and_check_project(&entry);
        assert_eq!(
            report.status, "ok",
            "Projeto examples/village_editor.rts com pak_map deveria validar sem erros: {:?}",
            report.diagnostics
        );
        let checked = ast_opt.unwrap();
        let c99 = codegen_native::NativeCodegen::emit_c99(&checked);
        assert!(c99.contains("pak_save_scene_file"));
        assert!(c99.contains("pak_load_scene_file"));
        assert!(c99.contains("rts_fs_write_bytes"));
        assert!(c99.contains("rts_fs_read_raw"));

        let llvm = codegen_llvm::LlvmIrCodegen::emit_llvm_ir(&checked);
        assert!(llvm.contains("pak_save_scene_file"));
        assert!(llvm.contains("pak_load_scene_file"));
        assert!(llvm.contains("@__rts_fs_write_bytes"));
        assert!(llvm.contains("@__rts_fs_read_raw"));
    }

    #[test]
    fn test_66_v310_engine_2d_3d_and_shader_examples_dual_codegen() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));

        // 1. Engine 2D (SpriteBatch2D + Texturas Reais + MODE_ORTHO_2D)
        let demo_2d = root.join("examples/demo_2d.rts");
        let (ast_2d, rep_2d) = compile_and_check_project(&demo_2d);
        assert_eq!(rep_2d.status, "ok", "demo_2d.rts failed: {:?}", rep_2d.diagnostics);
        let checked_2d = ast_2d.unwrap();
        let c99_2d = codegen_native::NativeCodegen::emit_c99(&checked_2d);
        assert!(c99_2d.contains("sprite2d_draw"));
        assert!(c99_2d.contains("sprite2d_flush"));
        assert!(c99_2d.contains("rtsc_vk_create_texture_rgba_ptr"));
        assert!(c99_2d.contains("rtsc_vk_cmd_bind_texture"));
        assert!(c99_2d.contains("pass.ubo_off != UINT64_MAX"));
        assert!(!c99_2d.contains("pass.ubo_off != 0ull"));
        let llvm_2d = codegen_llvm::LlvmIrCodegen::emit_llvm_ir(&checked_2d);
        assert!(llvm_2d.contains("sprite2d_draw"));
        assert!(llvm_2d.contains("@rtsc_vk_cmd_bind_texture"));

        // 2. Engine 3D desacoplada
        let demo_3d = root.join("examples/demo_3d.rts");
        let (ast_3d, rep_3d) = compile_and_check_project(&demo_3d);
        assert_eq!(rep_3d.status, "ok", "demo_3d.rts failed: {:?}", rep_3d.diagnostics);
        let checked_3d = ast_3d.unwrap();
        let c99_3d = codegen_native::NativeCodegen::emit_c99(&checked_3d);
        assert!(c99_3d.contains("mesh_write_box_rotated"));
        let llvm_3d = codegen_llvm::LlvmIrCodegen::emit_llvm_ir(&checked_3d);
        assert!(llvm_3d.contains("mesh_write_box_rotated"));

        // 3. Pipeline dinamico SPIR-V + regressao de runtime C99 (sem VKAPI_PTR/rtsc_vk_load_dev)
        let demo_shader = root.join("examples/demo_shader.rts");
        let (ast_sh, rep_sh) = compile_and_check_project(&demo_shader);
        assert_eq!(rep_sh.status, "ok", "demo_shader.rts failed: {:?}", rep_sh.diagnostics);
        let checked_sh = ast_sh.unwrap();
        let c99_sh = codegen_native::NativeCodegen::emit_c99(&checked_sh);
        assert!(c99_sh.contains("rtsc_vk_create_pipeline_from_spirv"));
        assert!(c99_sh.contains("LoadLibraryA(\"vulkan-1.dll\")"));
        assert!(c99_sh.contains("custom_pipes"));
        assert!(!c99_sh.contains("VKAPI_PTR"));
        assert!(!c99_sh.contains("rtsc_vk_load_dev"));
        let llvm_sh = codegen_llvm::LlvmIrCodegen::emit_llvm_ir(&checked_sh);
        assert!(llvm_sh.contains("@rtsc_vk_create_pipeline_from_spirv"));
    }

    #[test]
    fn test_67_v310_engine_mesh_load_obj_and_gbnf_shader_attrs() {
        let gbnf = gbnf_exporter::export_rustts_gbnf();
        assert!(gbnf.contains("@vertex_shader"));
        assert!(gbnf.contains("@fragment_shader"));

        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let tmp_dir = root.join("build");
        let _ = std::fs::create_dir_all(&tmp_dir);
        let probe = tmp_dir.join("_test_mesh_obj_probe.rts");
        let src = r#"
module test.mesh_obj_probe;
import core.mem;
import engine.mesh;

pub fn main(arena: cap StaticArena) -> int {
    let verts: cap MutSlice<mesh::Vertex3D> = arena.alloc_slice(64);
    let inds: cap MutSlice<u32> = arena.alloc_slice(64);
    let _packed = mesh::mesh_load_obj("nonexistent.obj", mem::slice_ptr(verts), 0, 64, mem::slice_ptr(inds), 0, 64);
    return 0;
}
"#;
        std::fs::write(&probe, src).unwrap();
        let (ast_opt, report) = compile_and_check_project(&probe);
        let _ = std::fs::remove_file(&probe);
        assert_eq!(report.status, "ok", "mesh_load_obj probe failed: {:?}", report.diagnostics);
        let checked = ast_opt.unwrap();
        let c99 = codegen_native::NativeCodegen::emit_c99(&checked);
        assert!(c99.contains("rtsc_mesh_load_obj"));
        let llvm = codegen_llvm::LlvmIrCodegen::emit_llvm_ir(&checked);
        assert!(llvm.contains("mesh_load_obj"));
    }

    #[test]
    fn test_68_v310_graphics_shader_push_constant_mvp_and_sampler2d_spirv() {
        let src = r#"
module gpu.pbr_custom;

struct Mat4 { m: [float; 16] }
struct Vec2 { x: float, y: float }
struct Vec4 { x: float, y: float, z: float, w: float }
struct Sampler2D { handle: u64 }

fn mul(m: Mat4, v: Vec4) -> Vec4 {
    return v;
}

fn sample(tex: Sampler2D, uv: Vec2) -> Vec4 {
    return Vec4 { x: uv.x, y: uv.y, z: 0.0, w: 1.0 };
}

@vertex_shader
pub fn vs_main(pos: Vec4, mvp: Mat4) -> Vec4 {
    return mul(mvp, pos);
}

@fragment_shader
pub fn fs_main(uv: Vec2, tex: Sampler2D) -> Vec4 {
    return sample(tex, uv);
}
"#;
        let (ast_opt, rep) = compile_and_check(src, "test_68_shader.rts");
        assert_eq!(rep.status, "ok", "{}", rep.to_human_terminal());
        let module = ast_opt.unwrap();

        let vs_spv = codegen_gpu::GpuCodegen::emit_spirv_vertex(&module);
        let vs_words: Vec<u32> = vs_spv
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        let has_mat4 = vs_words.windows(4).any(|w| (w[0] & 0xFFFF) == 24 && w[3] == 4); // OpTypeMatrix 4
        let has_mat_mul = vs_words.windows(5).any(|w| (w[0] & 0xFFFF) == 145); // OpMatrixTimesVector
        assert!(has_mat4, "Vertex shader should emit OpTypeMatrix for Mat4 push constant");
        assert!(has_mat_mul, "Vertex shader should emit OpMatrixTimesVector for mul(mvp, pos)");

        let fs_spv = codegen_gpu::GpuCodegen::emit_spirv_fragment(&module);
        let fs_words: Vec<u32> = fs_spv
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        let has_sampled_img = fs_words.windows(3).any(|w| (w[0] & 0xFFFF) == 26); // OpTypeSampledImage
        let has_img_sample = fs_words.windows(5).any(|w| (w[0] & 0xFFFF) == 87); // OpImageSampleImplicitLod
        assert!(has_sampled_img, "Fragment shader should emit OpTypeSampledImage for Sampler2D");
        assert!(has_img_sample, "Fragment shader should emit OpImageSampleImplicitLod for sample()");
    }

    #[test]
    fn test_69_v310_engine_r3d_audio_input_png_and_multitex_dual_codegen() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let tmp_dir = root.join("build");
        let _ = std::fs::create_dir_all(&tmp_dir);
        let probe = tmp_dir.join("_test_engine_v31_probe.rts");
        let src = r#"
module test.engine_v31_probe;
import core.mem;
import engine.platform.window;
import engine.audio;
import engine.gpu.rhi;
import engine.gpu.vram_arena;
import engine.renderer_2d;
import engine.renderer_3d;
import engine.ui.imgui_rts;
import engine.math.vec3;
import engine.math.vec4;

pub fn main(arena: cap StaticArena) -> int {
    let t0 = window::rtsc_time_now_sec();
    let _k = window::KEY_SPACE_BIT | window::KEY_ESC_BIT | window::KEY_ENTER_BIT;
    let _aud = audio::audio_play_wav("sfx.wav", false);
    audio::audio_stop_all();
    let mut dev = rhi::rhi_device_init(rhi::BACKEND_VULKAN, 0, 1280, 720, 1024 * 1024, true);
    let mut cmd = rhi::rhi_begin_frame(inout dev);
    let mut b2d = renderer_2d::sprite2d_init(16);
    let v2: cap MutSlice<imgui_rts::UiVertex> = arena.alloc_slice(64);
    let i2: cap MutSlice<u32> = arena.alloc_slice(96);
    let ubo = vram_arena::vram_arena_alloc_aligned(inout dev.arena, 256, vram_arena::GPU_CBUFFER_ALIGNMENT);
    renderer_2d::sprite2d_begin(inout b2d, 1);
    renderer_2d::sprite2d_draw_rect(inout b2d, v2, i2, 0.0, 0.0, 1.0, 1.0, 0xFFFFFFFF);
    renderer_2d::sprite2d_set_texture(inout b2d, inout dev, cmd, v2, i2, ubo, 2);
    renderer_2d::sprite2d_flush(inout b2d, inout dev, cmd, v2, i2, ubo);
    let mut r3d = renderer_3d::r3d_init(64, 96);
    renderer_3d::r3d_begin(inout r3d);
    let _ok = rhi::rhi_end_frame_and_present(dev, inout cmd);
    return if t0 >= 0.0 { 0 } else { 1 };
}
"#;
        std::fs::write(&probe, src).unwrap();
        let (ast_opt, rep) = compile_and_check_project(&probe);
        let _ = std::fs::remove_file(&probe);
        assert_eq!(rep.status, "ok", "v31 probe failed: {:?}", rep.diagnostics);
        let checked = ast_opt.unwrap();
        let c99 = codegen_native::NativeCodegen::emit_c99(&checked);
        assert!(c99.contains("rtsc_time_now_sec"));
        assert!(c99.contains("rtsc_audio_play_wav"));
        assert!(c99.contains("rtsc_decode_png_rgba"));
        assert!(c99.contains("sprite2d_set_texture"));
        assert!(c99.contains("r3d_begin"));
        let llvm = codegen_llvm::LlvmIrCodegen::emit_llvm_ir(&checked);
        assert!(llvm.contains("@rtsc_time_now_sec"));
        assert!(llvm.contains("@rtsc_audio_play_wav"));
        assert!(!llvm.contains("declare float @rtsc_time_now_sec"));
        assert!(!llvm.contains("declare i1 @rtsc_audio_play_wav"));
        assert!(!llvm.contains("ret float 0.016"));
        assert!(llvm.contains("sprite2d_set_texture"));
    }

    #[test]
    fn test_70_v300_version_and_docs_consistency() {
        assert_eq!(diagnostics::COMPILER_VERSION, "2.3.0-rts");
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let doc_files = [
            "README.md",
            "INSTALACAO_E_USO.md",
            "MANUAL_TIPOS_RUSTTS.md",
            "manual_e_matriz_de_regras_da_linguagem_rustts.md",
            "CONTRIBUTING.md",
            "devlog.md",
            "Guia_Interativo_e_Documentacao_RustTS.html",
        ];
        for rel in doc_files {
            let text = std::fs::read_to_string(root.join(rel))
                .unwrap_or_else(|e| panic!("Falha ao ler {}: {}", rel, e));
            assert!(
                text.contains("2.3.0-rts"),
                "Documento '{}' deveria referenciar a versao unificada 2.3.0-rts",
                rel
            );
        }
        let roadmap_rel = "roadmap_mestre_e_arquitetura_rustts_v3_0_0_pro.md";
        if root.join(roadmap_rel).exists() {
            let text = std::fs::read_to_string(root.join(roadmap_rel)).unwrap();
            assert!(
                text.contains("2.3.0-rts"),
                "Documento '{}' deveria referenciar a versao unificada 2.3.0-rts",
                roadmap_rel
            );
        }
        for rel in [
            "README.md",
            "INSTALACAO_E_USO.md",
            "Guia_Interativo_e_Documentacao_RustTS.html",
        ] {
            let text = std::fs::read_to_string(root.join(rel)).unwrap();
            assert!(
                text.contains("demo_2d.rts") && text.contains("spirv-vert"),
                "Guia '{}' deveria documentar demo_2d.rts e --target spirv-vert",
                rel
            );
        }
        let cargo_toml = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
        assert!(
            cargo_toml.contains("version = \"2.3.0-rts\""),
            "Cargo.toml deveria estar na versao 2.3.0-rts"
        );
        for rel in ["README.md", "INSTALACAO_E_USO.md"] {
            let text = std::fs::read_to_string(root.join(rel)).unwrap();
            assert!(
                text.contains("bake-mesh")
                    && text.contains("bake-textures")
                    && text.contains("--verify-parity")
                    && text.contains("combat_godot.rts")
                    && text.contains("--target=godot"),
                "Guia '{}' deveria documentar bake-mesh, bake-textures, --verify-parity e backend Godot 4",
                rel
            );
        }
    }

    struct TempDirGuard(PathBuf);
    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    fn find_test_clang() -> Option<PathBuf> {
        if std::process::Command::new("clang").arg("--version").output().is_ok() {
            return Some(PathBuf::from("clang"));
        }
        let candidates = [
            r"C:\Program Files\Microsoft Visual Studio\18\Community\VC\Tools\Llvm\x64\bin\clang.exe",
            r"C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\Llvm\x64\bin\clang.exe",
            r"C:\Program Files\LLVM\bin\clang.exe",
        ];
        candidates.into_iter().map(PathBuf::from).find(|p| p.exists())
    }

    fn compile_and_run_dual(
        rts_path: &Path,
        work_dir: &Path,
        tag: &str,
    ) -> (String, String, Option<(String, String)>) {
        let arena_mb = crate::read_static_arena_mb_from_rts_toml(rts_path);
        let (ast_opt, rep) = compile_and_check_project(rts_path);
        assert_eq!(rep.status, "ok", "[{}] check failed:\n{}", tag, rep.to_human_terminal());
        let checked = ast_opt.expect("AST after check");

        let c99 = codegen_native::NativeCodegen::emit_c99_with_arena_mb(&checked, arena_mb);
        let llvm = codegen_llvm::LlvmIrCodegen::emit_llvm_ir_with_arena_mb(&checked, arena_mb);

        #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
        let run_outputs = if let Some(clang) = find_test_clang() {
            let c_src = work_dir.join(format!("{}.c", tag));
            let ll_src = work_dir.join(format!("{}.ll", tag));
            let c_exe = work_dir.join(format!("{}_c.exe", tag));
            let ll_exe = work_dir.join(format!("{}_ll.exe", tag));
            std::fs::write(&c_src, c99.as_bytes()).unwrap();
            std::fs::write(&ll_src, llvm.as_bytes()).unwrap();

            let mut c_cmd = std::process::Command::new(&clang);
            c_cmd.args([
                "-std=c11",
                "-O3",
                "-mavx2",
                "-mfma",
                "-Wall",
                "-Wextra",
                "-Werror",
            ])
            .arg(&c_src)
            .arg("-o")
            .arg(&c_exe);
            c_cmd.args(["-luser32", "-lgdi32", "-lwinmm"]);
            let c_comp = c_cmd.output().unwrap();
            assert!(
                c_comp.status.success(),
                "[{}] clang C99 failed:\n{}",
                tag,
                String::from_utf8_lossy(&c_comp.stderr)
            );

            let mut ll_cmd = std::process::Command::new(&clang);
            ll_cmd
                .args(["-x", "ir", "-O3", "-mavx2", "-mfma", "-Wno-override-module"])
                .arg(&ll_src)
                .arg("-o")
                .arg(&ll_exe);
            ll_cmd.args(["-luser32", "-lgdi32", "-lwinmm"]);
            let ll_comp = ll_cmd.output().unwrap();
            assert!(
                ll_comp.status.success(),
                "[{}] clang LLVM IR failed:\n{}",
                tag,
                String::from_utf8_lossy(&ll_comp.stderr)
            );

            let c_run = std::process::Command::new(&c_exe)
                .current_dir(work_dir)
                .env("RTS_SMOKE_TEST", "1")
                .output()
                .unwrap();
            assert!(
                c_run.status.success(),
                "[{}] C99 binary exited non-zero ({:?}): stdout={} stderr={}",
                tag,
                c_run.status.code(),
                String::from_utf8_lossy(&c_run.stdout),
                String::from_utf8_lossy(&c_run.stderr)
            );

            let ll_run = std::process::Command::new(&ll_exe)
                .current_dir(work_dir)
                .env("RTS_SMOKE_TEST", "1")
                .output()
                .unwrap();
            assert!(
                ll_run.status.success(),
                "[{}] LLVM binary exited non-zero ({:?}): stdout={} stderr={}",
                tag,
                ll_run.status.code(),
                String::from_utf8_lossy(&ll_run.stdout),
                String::from_utf8_lossy(&ll_run.stderr)
            );

            Some((
                String::from_utf8_lossy(&c_run.stdout).replace("\r\n", "\n"),
                String::from_utf8_lossy(&ll_run.stdout).replace("\r\n", "\n"),
            ))
        } else {
            None
        };
        #[cfg(not(all(target_os = "windows", target_arch = "x86_64")))]
        let run_outputs: Option<(String, String)> = {
            let _ = (work_dir, tag);
            None
        };

        (c99, llvm, run_outputs)
    }

    fn crc32_ieee(data: &[u8]) -> u32 {
        let mut crc = 0xFFFF_FFFFu32;
        for &b in data {
            crc ^= b as u32;
            for _ in 0..8 {
                let mask = (crc & 1).wrapping_neg();
                crc = (crc >> 1) ^ (0xEDB8_8320u32 & mask);
            }
        }
        !crc
    }

    fn adler32(data: &[u8]) -> u32 {
        let mut s1 = 1u32;
        let mut s2 = 0u32;
        for &b in data {
            s1 = (s1 + b as u32) % 65521;
            s2 = (s2 + s1) % 65521;
        }
        (s2 << 16) | s1
    }

    fn png_chunk(ctype: &[u8; 4], cdata: &[u8], out: &mut Vec<u8>) {
        out.extend_from_slice(&(cdata.len() as u32).to_be_bytes());
        out.extend_from_slice(ctype);
        out.extend_from_slice(cdata);
        let mut crc_in = Vec::with_capacity(4 + cdata.len());
        crc_in.extend_from_slice(ctype);
        crc_in.extend_from_slice(cdata);
        out.extend_from_slice(&crc32_ieee(&crc_in).to_be_bytes());
    }

    fn make_valid_rgba_png(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
        let mut png = vec![137, 80, 78, 71, 13, 10, 26, 10];
        let mut ihdr = Vec::with_capacity(13);
        ihdr.extend_from_slice(&w.to_be_bytes());
        ihdr.extend_from_slice(&h.to_be_bytes());
        ihdr.push(8); // bit_depth
        ihdr.push(6); // RGBA
        ihdr.push(0);
        ihdr.push(0);
        ihdr.push(0);
        png_chunk(b"IHDR", &ihdr, &mut png);

        let stride = (w as usize) * 4;
        let mut raw_scan = Vec::with_capacity((stride + 1) * h as usize);
        for y in 0..h as usize {
            raw_scan.push(0); // filter None
            raw_scan.extend_from_slice(&rgba[y * stride..(y + 1) * stride]);
        }
        let mut zlib = vec![0x78, 0x01, 0x01]; // zlib + final stored block
        let len = raw_scan.len() as u16;
        zlib.extend_from_slice(&len.to_le_bytes());
        zlib.extend_from_slice(&(!len).to_le_bytes());
        zlib.extend_from_slice(&raw_scan);
        zlib.extend_from_slice(&adler32(&raw_scan).to_be_bytes());
        png_chunk(b"IDAT", &zlib, &mut png);
        png_chunk(b"IEND", &[], &mut png);
        png
    }

    fn make_synthetic_fbx_7400() -> Vec<u8> {
        fn push_node(out: &mut Vec<u8>, base_off: usize, name: &str, props: &[u8], children: &[u8]) {
            let start = out.len();
            out.extend_from_slice(&[0u8; 12]);
            out.push(name.len() as u8);
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(props);
            out.extend_from_slice(children);
            if !children.is_empty() {
                out.extend_from_slice(&[0u8; 13]);
            }
            let end_off = (base_off + out.len()) as u32;
            let num_props = if props.is_empty() { 0u32 } else { 1u32 };
            let prop_len = props.len() as u32;
            out[start..start + 4].copy_from_slice(&end_off.to_le_bytes());
            out[start + 4..start + 8].copy_from_slice(&num_props.to_le_bytes());
            out[start + 8..start + 12].copy_from_slice(&prop_len.to_le_bytes());
        }
        fn f64_arr_prop(vals: &[f64]) -> Vec<u8> {
            let mut p = vec![b'd'];
            p.extend_from_slice(&(vals.len() as u32).to_le_bytes());
            p.extend_from_slice(&0u32.to_le_bytes());
            p.extend_from_slice(&((vals.len() * 8) as u32).to_le_bytes());
            for v in vals {
                p.extend_from_slice(&v.to_le_bytes());
            }
            p
        }
        fn i32_arr_prop(vals: &[i32]) -> Vec<u8> {
            let mut p = vec![b'i'];
            p.extend_from_slice(&(vals.len() as u32).to_le_bytes());
            p.extend_from_slice(&0u32.to_le_bytes());
            p.extend_from_slice(&((vals.len() * 4) as u32).to_le_bytes());
            for v in vals {
                p.extend_from_slice(&v.to_le_bytes());
            }
            p
        }

        let mut fbx = Vec::new();
        fbx.extend_from_slice(b"Kaydara FBX Binary  \x00\x1a\x00");
        fbx.extend_from_slice(&7400u32.to_le_bytes());

        // Build from inside out so end_offsets match absolute file offsets
        let objects_header_len = 13 + "Objects".len();
        let geom_header_len = 13 + "Geometry".len();
        let geom_base = fbx.len() + objects_header_len;
        let mut geom_children = Vec::new();
        let pos_p = f64_arr_prop(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.5, 0.0]);
        push_node(&mut geom_children, geom_base + geom_header_len, "Vertices", &pos_p, &[]);
        let poly_p = i32_arr_prop(&[0, 1, !2]);
        push_node(
            &mut geom_children,
            geom_base + geom_header_len,
            "PolygonVertexIndex",
            &poly_p,
            &[],
        );

        let norm_hdr_len = 13 + "LayerElementNormal".len();
        let mut norm_children = Vec::new();
        let norm_p = f64_arr_prop(&[0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0]);
        push_node(
            &mut norm_children,
            geom_base + geom_header_len + geom_children.len() + norm_hdr_len,
            "Normals",
            &norm_p,
            &[],
        );
        push_node(
            &mut geom_children,
            geom_base + geom_header_len,
            "LayerElementNormal",
            &[],
            &norm_children,
        );

        let uv_hdr_len = 13 + "LayerElementUV".len();
        let mut uv_children = Vec::new();
        let uv_p = f64_arr_prop(&[0.0, 0.0, 1.0, 0.0, 0.0, 1.0]);
        push_node(
            &mut uv_children,
            geom_base + geom_header_len + geom_children.len() + uv_hdr_len,
            "UV",
            &uv_p,
            &[],
        );
        push_node(
            &mut geom_children,
            geom_base + geom_header_len,
            "LayerElementUV",
            &[],
            &uv_children,
        );

        let mut objects_children = Vec::new();
        push_node(&mut objects_children, geom_base, "Geometry", &[], &geom_children);

        let top_base = fbx.len();
        push_node(&mut fbx, top_base, "Objects", &[], &objects_children);
        fbx.extend_from_slice(&[0u8; 13]);
        fbx
    }

    #[test]
    fn test_71_p0_parser_view_struct_sign_compare_and_llvm_bool_if_parity() {
        let dir = std::env::temp_dir().join("rtsc_test_71_parity");
        let _ = std::fs::remove_dir_all(&dir);
        let src_dir = dir.join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        let _guard = TempDirGuard(dir.clone());

        let sub_rts = src_dir.join("sub_mod.rts");
        std::fs::write(
            &sub_rts,
            r#"
module sub_mod;

pub const TARGET_OK: i32 = 42;

pub @view struct EntityView {
    pub pos_x: cap MutSlice<f32>,
    pub count: usize,
}

@repr_c
pub struct Enemy {
    pub hp: i32,
}

pub fn step_entities(view: EntityView) {
    let mut i: usize = 0;
    while i < view.pos_x.len() && i < view.count {
        view.pos_x[i] = view.pos_x[i] + 2.5;
        i = i + 1;
    }
}

pub fn check_flag(v: i32) -> bool {
    return if v == TARGET_OK { true } else { false };
}
"#,
        )
        .unwrap();

        let main_rts = src_dir.join("main.rts");
        std::fs::write(
            &main_rts,
            r#"
module test_p0_main;
import core.io;
import sub_mod;

pub fn main(arena: cap StaticArena) -> int {
    let px: cap MutSlice<f32> = arena.alloc_slice(4);
    px.fill(1.0);
    let view = sub_mod::EntityView { pos_x: px, count: 4 };
    sub_mod::step_entities(view);

    let enemies: cap MutSlice<sub_mod::Enemy> = arena.alloc_slice(2);
    enemies[0].hp = 99;
    enemies[1].hp = 42;

    let mut cur: i32 = enemies[1].hp;
    let mut const_hit: i32 = 0;
    if cur == sub_mod::TARGET_OK {
        const_hit = 1;
    }

    let s: i32 = -5;
    let u: usize = 3;
    let mut sign_ok: i32 = 0;
    if s < 0 && u > 1 && s < u {
        sign_ok = 1;
    }

    let flag_a: bool = sub_mod::check_flag(cur);
    let flag_b: bool = if const_hit > 0 { flag_a } else { false };

    let ok_val: i32 = if flag_b { 1 } else { 0 };
    let total: i32 = enemies[0].hp + ok_val + const_hit + sign_ok + (view.pos_x[0] as i32);
    io::print_i32("TOTAL=", total);
    return 0;
}
"#,
        )
        .unwrap();

        let (c99, _llvm, run_outs) = compile_and_run_dual(&main_rts, &dir, "t71");
        assert!(c99.contains("sub_mod.rts"), "Expected sub_mod.rts in #line directives");
        assert!(c99.contains("main.rts"), "Expected main.rts in #line directives");
        if let Some((c_out, ll_out)) = run_outs {
            assert_eq!(c_out, ll_out, "C99 and --llvm outputs differ!");
            assert!(c_out.contains("TOTAL=105"), "Unexpected output: {}", c_out);
        }

        let bad_view = r#"
module test.bad_view;
@view
struct MyView {
    buf: cap MutSlice<u32>,
}
struct NestedBad {
    inner: MyView,
}
pub fn leak_view(v: MyView) -> MyView {
    return v;
}
"#;
        let (_ast, rep) = compile_and_check(bad_view, "bad_view.rts");
        assert_eq!(rep.status, "error");
        let e0501 = rep.diagnostics.iter().filter(|d| d.error_code == "RTS-E0501").count();
        assert!(e0501 >= 2, "Expected RTS-E0501 on nested and returned @view, got: {:?}", rep.diagnostics);
    }

    #[test]
    fn test_72_p0_window_input_fs_png_and_static_arena_config() {
        let dir = std::env::temp_dir().join("rtsc_test_72_p0_hal");
        let _ = std::fs::remove_dir_all(&dir);
        let src_dir = dir.join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        let _guard = TempDirGuard(dir.clone());

        std::fs::write(
            dir.join("rts.toml"),
            "[package]\nname = \"t72\"\nversion = \"2.3.0-rts\"\n\n[memory]\nstatic_arena_mb = 24\n",
        )
        .unwrap();
        let main_rts = src_dir.join("main.rts");
        assert_eq!(crate::read_static_arena_mb_from_rts_toml(&main_rts), 24);

        let rgba = [
            255u8, 10, 20, 255, // (0,0) -> 0xFFFF0A14
            0, 255, 0, 128,     // (1,0) -> 0x8000FF00
            10, 20, 255, 255,   // (0,1) -> 0xFF0A14FF
            200, 150, 100, 255, // (1,1) -> 0xFFC89664
        ];
        std::fs::write(dir.join("probe.png"), make_valid_rgba_png(2, 2, &rgba)).unwrap();

        std::fs::write(
            &main_rts,
            r#"
module test_p0_hal;
import core.io;
import core.mem;
import core.math;

pub fn main(
    arena: cap StaticArena,
    win: cap WindowHost,
    inp: cap InputDevice,
    fs: cap FileSystem
) -> int {
    let cap_b: usize = arena.capacity_bytes();
    let scratch: cap MutSlice<u8> = arena.alloc_slice(256);
    let used_b: usize = arena.used_bytes();

    let _okw: bool = win.open_headless(320, 240, 2);
    win.set_cursor_locked(true);
    win.set_cursor_visible(false);
    let fb: cap MutSlice<u32> = arena.alloc_slice(4);
    fb.fill(0xFF112233);
    win.draw_rect(0, 0, 10, 10, 0xFF00FF00);
    win.draw_text_scaled(4, 4, 2, 0xFFFFFFFF, "HUD");
    win.draw_blit_rgba(0, 0, 2, 2, fb);
    let _p: bool = win.poll_events();

    inp.poll();
    let _dx: i32 = inp.mouse_dx();
    let _dy: i32 = inp.mouse_dy();
    let _wh: i32 = inp.mouse_wheel_delta();
    let _rd: bool = inp.mouse_right_down();
    let _rc: bool = inp.mouse_right_clicked();

    let w_txt: bool = fs.write_text("note.txt", "hello-v221");
    let ex: bool = fs.exists("note.txt");
    let fsz: u64 = fs.file_size("note.txt");
    let rd_n: usize = fs.read_text("note.txt", scratch);

    let bin_w: cap MutSlice<u8> = arena.alloc_slice(4);
    bin_w[0] = 77;
    bin_w[1] = 88;
    bin_w[2] = 99;
    bin_w[3] = 111;
    let _bw: bool = fs.write_raw_slice("bin.dat", bin_w);
    let bin_r: cap MutSlice<u8> = arena.alloc_slice(4);
    let br_n: usize = fs.read_raw_slice("bin.dat", bin_r);
    let list_n: usize = fs.list_dir(".", scratch);

    let img_px: cap MutSlice<u32> = arena.alloc_slice(4);
    let img_dims: cap MutSlice<u32> = arena.alloc_slice(2);
    let png_ok: bool = fs.load_image_rgba("probe.png", img_px, img_dims);

    let f_val: f32 = mem::f32_from_bits(0x3F800000);
    let bits: u32 = mem::f32_to_bits(f_val);
    let m_val: f32 = math::asin(0.5) + math::acos(0.5) + math::atan(1.0) + math::exp(0.0) + math::log(1.0) + math::pow(2.0, 3.0);

    win.close();

    if cap_b >= 25165824 && used_b >= 256 && w_txt && ex && fsz == 10 && rd_n == 10
       && br_n == 4 && bin_r[2] == 99 && list_n > 0 && png_ok
       && img_dims[0] == 2 && img_dims[1] == 2
       && img_px[0] == 0xFFFF0A14 && img_px[1] == 0x8000FF00
       && img_px[2] == 0xFF0A14FF && img_px[3] == 0xFFC89664
       && bits == 0x3F800000 && m_val > 9.0 {
        io::println("OK_P0");
        return 0;
    }
    return 1;
}
"#,
        )
        .unwrap();

        let (_c99, _llvm, run_outs) = compile_and_run_dual(&main_rts, &dir, "t72");
        if let Some((c_out, ll_out)) = run_outs {
            assert_eq!(c_out, ll_out, "C99 and --llvm outputs differ!");
            assert!(c_out.contains("OK_P0"), "Expected OK_P0, got:\n{}", c_out);
        }
    }

    #[test]
    fn test_73_p1_audio_mixer_mic_cut_network_and_pbr_rhi() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let dir = root.join("build").join("_rtsc_test_73");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let _guard = TempDirGuard(dir.clone());

        let main_rts = dir.join("main.rts");
        std::fs::write(
            &main_rts,
            r#"
module test_p1_main;
import core.io;
import engine.gpu.rhi;
import engine.gpu.vram_arena;

pub fn main(
    arena: cap StaticArena,
    audio: cap AudioDevice,
    mic: cap AudioInput,
    net: cap NetworkSocket
) -> int {
    let wav_ok: bool = audio.write_synth_wav("sfx_t73.wav", 44100, 1024, 1);
    let mix_ok: bool = audio.mixer_init(44100);
    let ld_ok: bool = audio.mixer_load_wav(0, "sfx_t73.wav");
    let ch: i32 = audio.mixer_play(0, 0.85, 0.2, 2200.0, false);
    audio.mixer_set_channel(ch, 0.9, -0.3, 1800.0);
    let st_buf: cap MutSlice<i16> = arena.alloc_slice(64);
    let mixed_n: usize = audio.mixer_mix_frame(st_buf);
    audio.mixer_stop_channel(ch);

    let mut non_zero_pcm: bool = false;
    let mut k: usize = 0;
    while k < mixed_n {
        if st_buf[k] != 0 {
            non_zero_pcm = true;
        }
        k = k + 1;
    }

    let mic_ok: bool = mic.open(16000);
    let pcm_buf: cap MutSlice<i16> = arena.alloc_slice(8);
    pcm_buf.fill(1234);
    mic.set_muted(true);
    let muted_chk: bool = mic.is_muted();
    let mic_rd: usize = mic.read_pcm(pcm_buf);
    mic.close();

    let mut port_a: u16 = 45320;
    let mut s_rx: i32 = -1;
    while port_a < 45340 && s_rx < 0 {
        s_rx = net.udp_bind(port_a);
        if s_rx < 0 { port_a = port_a + 1; }
    }
    let mut port_b: u16 = port_a + 1;
    let mut s_tx: i32 = -1;
    while port_b < 45350 && s_tx < 0 {
        s_tx = net.udp_bind(port_b);
        if s_tx < 0 { port_b = port_b + 1; }
    }

    let pkt_tx: cap MutSlice<u8> = arena.alloc_slice(4);
    pkt_tx[0] = 70;
    pkt_tx[1] = 79;
    pkt_tx[2] = 68;
    pkt_tx[3] = 86;
    let sent_n: i32 = net.udp_send_to(s_tx, "127.0.0.1", port_a, pkt_tx);

    let pkt_rx: cap MutSlice<u8> = arena.alloc_slice(16);
    let sender_ip: cap MutSlice<u8> = arena.alloc_slice(32);
    let mut recv_n: i32 = -1;
    let mut tries: u32 = 0;
    while tries < 200 && recv_n <= 0 {
        recv_n = net.udp_recv_from(s_rx, pkt_rx, sender_ip);
        tries = tries + 1;
    }
    net.close_socket(s_rx);
    net.close_socket(s_tx);

    let mut dev = rhi::rhi_device_init(rhi::BACKEND_VULKAN, 0, 320, 240, 1048576, false);
    let mut cmd = rhi::rhi_begin_frame(inout dev);
    let ubo = vram_arena::vram_arena_alloc_aligned(inout dev.arena, 256, vram_arena::GPU_CBUFFER_ALIGNMENT);
    rhi::rhi_cmd_bind_pipeline(cmd, rhi::PIPELINE_PBR_3D);
    rhi::rhi_cmd_bind_pbr_textures(cmd, 1, 2, 3, 4);
    rhi::rhi_cmd_bind_lights(cmd, ubo);
    rhi::rhi_cmd_bind_bones(cmd, ubo);
    rhi::rhi_cmd_draw_indexed_instanced(cmd, 36, 1, 0, 0, 0);
    let _pbr_ok = rhi::rhi_end_frame_and_present(dev, inout cmd);

    if wav_ok && mix_ok && ld_ok && ch >= 0 && mixed_n == 64 && non_zero_pcm
       && mic_ok && muted_chk && mic_rd == 0 && pcm_buf[0] == 0 && pcm_buf[7] == 0
       && s_rx >= 0 && s_tx >= 0 && sent_n == 4 && recv_n == 4
       && pkt_rx[0] == 70 && pkt_rx[1] == 79 && pkt_rx[2] == 68 && pkt_rx[3] == 86 {
        io::println("OK_P1");
        return 0;
    }
    return 1;
}
"#,
        )
        .unwrap();

        let (c99, llvm, run_outs) = compile_and_run_dual(&main_rts, &dir, "t73");
        assert!(c99.contains("rtsc_vk_cmd_bind_pbr_textures"));
        assert!(llvm.contains("@rtsc_vk_cmd_bind_pbr_textures"));
        if let Some((c_out, ll_out)) = run_outs {
            assert!(c_out.contains("OK_P1"), "Expected OK_P1 in C99, got:\n{}", c_out);
            assert!(ll_out.contains("OK_P1"), "Expected OK_P1 in --llvm, got:\n{}", ll_out);
        }

        let cap_viol = r#"
module test.p1_cap_sec;
pub fn illegal_calls(arena: cap StaticArena, wrong_mic: cap AudioDevice, wrong_net: cap FileSystem) {
    let forged_mic = AudioInput::new();
    let forged_net = NetworkSocket::new();
    let pcm: cap MutSlice<i16> = arena.alloc_slice(4);
    let _r = wrong_mic.read_pcm(pcm);
    let _s = wrong_net.udp_bind(9999);
    let _u = forged_mic;
    let _v = forged_net;
}
"#;
        let (_ast, rep) = compile_and_check(cap_viol, "p1_cap_sec.rts");
        assert_eq!(rep.status, "error");
        let e0404 = rep.diagnostics.iter().filter(|d| d.error_code == "RTS-E0404").count();
        let e0402 = rep.diagnostics.iter().filter(|d| d.error_code == "RTS-E0402").count();
        assert!(e0404 >= 2, "Forgery AudioInput::new/NetworkSocket::new should trigger RTS-E0404: {:?}", rep.diagnostics);
        assert!(e0402 >= 2, "Wrong caps for AudioInput/NetworkSocket methods should trigger RTS-E0402: {:?}", rep.diagnostics);
    }

    #[test]
    fn test_74_stdlib_modules_check_and_run() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let dir = root.join("build").join("_rtsc_test_74");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let _guard = TempDirGuard(dir.clone());

        let main_rts = dir.join("main.rts");
        std::fs::write(
            &main_rts,
            r#"
module test_stdlib_main;
import core.io;
import core.mem;
import engine.json;
import engine.collections;
import engine.threads;
import engine.zpmesh;
import engine.diagnostics;

fn write_u32_le(buf: cap MutSlice<u8>, off: usize, v: u32) {
    buf[off + 0] = (v & 0xFF) as u8;
    buf[off + 1] = ((v >> 8) & 0xFF) as u8;
    buf[off + 2] = ((v >> 16) & 0xFF) as u8;
    buf[off + 3] = ((v >> 24) & 0xFF) as u8;
}

pub fn main(arena: cap StaticArena, fs: cap FileSystem) -> int {
    // 1. engine.json
    let mut jw = json::json_writer_new();
    json::json_push_i32(inout jw, "credits", 750);
    json::json_push_bool(inout jw, "alive", true);
    json::json_push_str(inout jw, "zone", "ALPHA");
    let finished: str256 = json::json_finish(inout jw);
    let jbuf: cap MutSlice<u8> = arena.alloc_slice(256);
    let mut ji: usize = 0;
    let jlen: usize = finished.len();
    let jstr_p: str = finished.as_str();
    while ji < jlen {
        jbuf[ji] = mem::str_byte(jstr_p, ji);
        ji = ji + 1;
    }
    let parsed_credits: i32 = json::json_get_i32(jbuf, jlen, "credits", -1);
    let parsed_alive: bool = json::json_get_bool(jbuf, jlen, "alive", false);

    // 2. engine.collections
    let v_mem: cap MutSlice<u32> = arena.alloc_slice(8);
    let mut vec = collections::ArenaVecU32 { data: v_mem, len: 0, capacity: 8 };
    let _p1 = collections::vec_u32_push(inout vec, 100);
    let _p2 = collections::vec_u32_push(inout vec, 250);
    let v_pop: u32 = collections::vec_u32_pop(inout vec, 0);

    let q_mem: cap MutSlice<u32> = arena.alloc_slice(4);
    let mut q = collections::ArenaQueueU32 { data: q_mem, head: 0, tail: 0, count: 0, capacity: 4 };
    let _qp = collections::queue_u32_push(inout q, 42);
    let q_pop: u32 = collections::queue_u32_pop(inout q, 0);

    let mk: cap MutSlice<u32> = arena.alloc_slice(8);
    let mv: cap MutSlice<u32> = arena.alloc_slice(8);
    let mu: cap MutSlice<u8> = arena.alloc_slice(8);
    let mut mp = collections::ArenaMapU32 { keys: mk, vals: mv, used: mu, capacity: 8 };
    collections::map_u32_init(inout mp);
    let _mi = collections::map_u32_insert(inout mp, 77, 999);
    let mp_val: u32 = collections::map_u32_get(mp, 77, 0);

    // 3. engine.threads
    let wc: u32 = threads::parallel_worker_count(2);
    let rng = threads::parallel_chunk_range(100, 0, 2);

    // 4. engine.zpmesh
    let zp: cap MutSlice<u8> = arena.alloc_slice(128);
    write_u32_le(zp, 0, zpmesh::MAGIC_ZPM2);
    write_u32_le(zp, 4, 2);
    write_u32_le(zp, 12, 1);
    write_u32_le(zp, 52, (zpmesh::zpmesh_f32_to_q16(1.5)) as u32);
    let zh = zpmesh::zpmesh_parse_v2_header(zp, 128);
    let zv = zpmesh::zpmesh_read_vertex_v2(zp, 128, 0);

    // 5. engine.diagnostics
    let d_none = diagnostics::error_registrar_se_houver(fs, diagnostics::error_none(), "logs/no_err.json");
    let no_err_ex: bool = fs.exists("logs/no_err.json");
    let bad = diagnostics::DiagErrorInfo {
        code: diagnostics::ERR_ASSET,
        stage: str64::from("load"),
        message: str128::from("missing"),
        detail: str128::from("wall"),
    };
    let d_err = diagnostics::error_registrar_se_houver(fs, bad, "logs/err.json");
    let err_ex: bool = fs.exists("logs/err.json");
    let err_buf: cap MutSlice<u8> = arena.alloc_slice(256);
    let err_len: usize = fs.read_text("logs/err.json", err_buf);
    let parsed_code: i32 = json::json_get_i32(err_buf, err_len, "code", -1);

    // 6. Regressao de sequencias de escape em literais de string (C99 vs --llvm)
    let esc_s: str = "\"a\nb\tc\\d\"";
    let esc_ok: bool = mem::str_len(esc_s) == 9
        && mem::str_byte(esc_s, 0) == 34
        && mem::str_byte(esc_s, 2) == 10
        && mem::str_byte(esc_s, 4) == 9
        && mem::str_byte(esc_s, 6) == 92
        && mem::str_byte(esc_s, 8) == 34;

    if parsed_credits == 750 && parsed_alive
       && v_pop == 250 && q_pop == 42 && mp_val == 999
       && wc >= 1 && rng.start == 0 && rng.end == 50
       && zh.magic == zpmesh::MAGIC_ZPM2 && zh.vertex_count == 1 && zv.pos.x > 1.4 && zv.pos.x < 1.6
       && !d_none && !no_err_ex && d_err && err_ex && parsed_code == diagnostics::ERR_ASSET
       && esc_ok {
        io::println("OK_STDLIB");
        return 0;
    }
    return 1;
}
"#,
        )
        .unwrap();

        let (_c99, _llvm, run_outs) = compile_and_run_dual(&main_rts, &dir, "t74");
        if let Some((c_out, ll_out)) = run_outs {
            let c_clean: String = c_out
                .lines()
                .filter(|l| !l.starts_with("[MULTICORE] "))
                .map(|l| format!("{}\n", l))
                .collect();
            assert_eq!(c_clean, ll_out, "C99 and --llvm outputs differ!");
            assert!(
                ll_out.contains("OK_STDLIB"),
                "Expected OK_STDLIB, got c_out:\n{}\nll_out:\n{}",
                c_out,
                ll_out
            );
        }
    }

    #[test]
    fn test_75_native_tools_bake_mesh_and_bake_textures() {
        let dir = std::env::temp_dir().join("rtsc_test_75_tools");
        let _ = std::fs::remove_dir_all(&dir);
        let tex_sub = dir.join("textures").join("props");
        std::fs::create_dir_all(&tex_sub).unwrap();
        let _guard = TempDirGuard(dir.clone());

        // 1. inflate_zlib stored block check
        let payload = b"RUSTTS_ZLIB";
        let mut z = vec![0x78, 0x01, 0x01];
        z.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(payload.len() as u16)).to_le_bytes());
        z.extend_from_slice(payload);
        z.extend_from_slice(&adler32(payload).to_be_bytes());
        assert_eq!(crate::bake_mesh::inflate_zlib(&z).unwrap(), payload);

        // 2. FBX 7400 -> .zpmesh v2
        let fbx_path = dir.join("tri.fbx");
        let zp_path = dir.join("tri.zpmesh");
        std::fs::write(&fbx_path, make_synthetic_fbx_7400()).unwrap();
        let m_rep = crate::bake_mesh::bake_fbx_to_zpmesh(&fbx_path, &zp_path).unwrap();
        assert_eq!(m_rep.vertices, 3);
        assert_eq!(m_rep.triangles, 1);

        let zp_bytes = std::fs::read(&zp_path).unwrap();
        assert!(zp_bytes.len() >= 52 + 3 * 60 + 3 * 4);
        let magic = u32::from_le_bytes(zp_bytes[0..4].try_into().unwrap());
        let ver = u32::from_le_bytes(zp_bytes[4..8].try_into().unwrap());
        let vcnt = u32::from_le_bytes(zp_bytes[12..16].try_into().unwrap());
        let icnt = u32::from_le_bytes(zp_bytes[16..20].try_into().unwrap());
        let v1_x_q16 = i32::from_le_bytes(zp_bytes[52 + 60..52 + 64].try_into().unwrap());
        assert_eq!(magic, crate::bake_mesh::MAGIC_ZPM2);
        assert_eq!(ver, 2);
        assert_eq!(vcnt, 3);
        assert_eq!(icnt, 3);
        assert_eq!(v1_x_q16, 65536);

        // 3. Recursive bake_textures_dir (Color + AmbientOcclusion)
        // Color = (200, 100, 50, 255), AO = 0 => 0.35 * Color = (70, 35, 18, 255) = 0xFF462312
        let color_px = [200u8, 100, 50, 255, 200, 100, 50, 255, 200, 100, 50, 255, 200, 100, 50, 255];
        let ao_px = [0u8, 0, 0, 255, 255, 255, 255, 255, 0, 0, 0, 255, 255, 255, 255, 255];
        std::fs::write(
            tex_sub.join("wall_Color.png"),
            make_valid_rgba_png(2, 2, &color_px),
        )
        .unwrap();
        std::fs::write(
            tex_sub.join("wall_AmbientOcclusion.png"),
            make_valid_rgba_png(2, 2, &ao_px),
        )
        .unwrap();

        let baked_rts = dir.join("baked_tex.rts");
        let t_rep = crate::bake_textures::bake_textures_dir(&dir.join("textures"), &baked_rts).unwrap();
        assert_eq!(t_rep.textures_baked, 1);
        assert_eq!(t_rep.pixels_total, 4);
        let rts_text = std::fs::read_to_string(&baked_rts).unwrap();
        assert!(rts_text.contains("WALL_W: u32 = 2"));
        assert!(rts_text.contains("0xFF462312"), "Expected AO=0 shaded pixel in:\n{}", rts_text);
        assert!(rts_text.contains("0xFFC86432"), "Expected AO=255 pixel in:\n{}", rts_text);
    }

    #[test]
    fn test_76_godot_gdextension_and_custom_module_codegen() {
        // 1. GBNF formal grammar includes Godot 4 attributes
        let gbnf = crate::gbnf_exporter::export_rustts_gbnf();
        assert!(gbnf.contains("@godot_node"), "GBNF missing @godot_node");
        assert!(gbnf.contains("@godot_impl"), "GBNF missing @godot_impl");
        assert!(gbnf.contains("@gdexport"), "GBNF missing @gdexport");

        // 2. Compile & validate examples/combat_godot.rts
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let godot_example = root.join("examples").join("combat_godot.rts");
        let (mod_opt, rep) = compile_and_check_project(&godot_example);
        assert_eq!(
            rep.status, "ok",
            "combat_godot.rts check failed: {:?}",
            rep.diagnostics
        );
        let module = mod_opt.expect("Valid AST from combat_godot.rts");

        // 3. Semantic capability check: mutating self under cap Read<KinematicMover> triggers RTS-E0402
        let bad_godot = r#"
module test.godot_sec;
@godot_node(base = "CharacterBody3D", category = "Movement")
struct GuardNode {
    @gdexport
    speed: f32,
}
@godot_impl(GuardNode)
pub fn illegal_write(self: cap Read<GuardNode>, delta: f32) {
    self.speed = delta;
}
"#;
        let (_bad_ast, bad_rep) = compile_and_check(bad_godot, "bad_godot.rts");
        assert_eq!(bad_rep.status, "error");
        assert!(
            bad_rep.diagnostics.iter().any(|d| d.error_code == "RTS-E0402"),
            "Expected RTS-E0402 when mutating Read<GuardNode>, got: {:?}",
            bad_rep.diagnostics
        );

        // 4. GDExtension output (--target=godot)
        let ext = crate::codegen_godot::GodotCodegen::emit_godot_extension(&module);
        assert_eq!(ext.classes.len(), 1);
        let cls = &ext.classes[0];
        assert_eq!(cls.class_name, "KinematicMover");
        assert!(cls.header_code.contains("GDCLASS(KinematicMover, CharacterBody3D);"));
        assert!(cls.header_code.contains("<godot_cpp/classes/character_body3d.hpp>"));
        assert!(cls.header_code.contains("compute_effective_speed(float friction) const;"));
        assert!(cls.header_code.contains("void apply_impulse(float boost);"));
        assert!(cls.source_code.contains("ClassDB::bind_method(D_METHOD(\"compute_effective_speed\", \"friction\"), &KinematicMover::compute_effective_speed);"));
        assert!(cls.source_code.contains("ADD_PROPERTY(PropertyInfo(Variant::FLOAT, \"base_speed\", PROPERTY_HINT_RANGE, \"1.0,20.0,0.5\"), \"set_base_speed\", \"get_base_speed\");"));
        assert!(cls.source_code.contains("Math::clamp("));
        assert!(ext.register_types_cpp.contains("GDREGISTER_CLASS(KinematicMover);"));
        assert!(ext.register_types_cpp.contains("<gdextension_interface.h>"));
        assert!(ext.register_types_cpp.contains("godot::GDExtensionBinding::InitObject"));
        assert!(ext.sconstruct.contains("godot-cpp/SConstruct"));
        assert!(ext.sconstruct.contains("SharedLibrary"));
        assert!(ext.gdextension_manifest.contains("compatibility_minimum = \"4.2\""));
        assert!(ext.gdextension_manifest.contains("entry_symbol = \"game_godot_movement_library_init\""));

        // 5. Custom Engine Module output (--target=godot-module)
        let gmod = crate::codegen_godot::GodotCodegen::emit_godot_module(&module);
        assert_eq!(gmod.classes.len(), 1);
        assert!(gmod.classes[0].header_code.contains("\"scene/3d/physics/character_body_3d.h\""));
        assert!(gmod.register_types_h.contains("\"modules/register_module_types.h\""));
        assert!(gmod.register_types_cpp.contains("ClassDB::register_class<KinematicMover>();"));
        assert!(gmod.scsub.contains("add_source_files"));
        assert!(gmod.config_py.contains("def can_build("));

        // 6. Fallback RefCounted static helper generation when module has no @godot_node
        let util_src = r#"
module demo.helpers;
pub fn triple_val(x: i32) -> i32 {
    return x * 3;
}
"#;
        let (util_ast, util_rep) = compile_and_check(util_src, "helpers.rts");
        assert_eq!(util_rep.status, "ok");
        let util_ext = crate::codegen_godot::GodotCodegen::emit_godot_extension(&util_ast.unwrap());
        assert_eq!(util_ext.classes.len(), 1);
        assert!(util_ext.classes[0].header_code.contains(": public RefCounted"));
        assert!(util_ext.classes[0].header_code.contains("static int32_t triple_val(int32_t x);"));
        assert!(util_ext.classes[0].source_code.contains("ClassDB::bind_static_method(\"DemoHelpersExtension\", D_METHOD(\"triple_val\", \"x\"), &DemoHelpersExtension::triple_val);"));
    }
}
