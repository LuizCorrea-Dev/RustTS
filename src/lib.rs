pub mod ast;
pub mod codegen_native;
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
}
