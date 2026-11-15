pub mod ast;
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

    candidates.into_iter().find(|p| p.is_file())
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

    merged_items.extend(module.items.clone());
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
        assert_eq!(diagnostics::COMPILER_VERSION, "2.1.0-rts");
        let ok_report = diagnostics::CompilerDiagnosticsReport::ok();
        assert_eq!(ok_report.compiler_version, "2.1.0-rts");
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
}
