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

use ast::Module;
use diagnostics::{CompilerDiagnosticsReport, DiagnosticCategory, DiagnosticEntry};
use lexer::tokenize;
use parser::Parser;
use sema::SemanticAnalyzer;

/// Pipeline completo de validação (`rtsc check`) de um arquivo `.rts`
pub fn compile_and_check(source: &str, file_path: &str) -> (Option<Module>, CompilerDiagnosticsReport) {
    let tokens = match tokenize(source) {
        Ok(t) => t,
        Err(lex_errs) => {
            let entries = lex_errs
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
                .collect();
            return (None, CompilerDiagnosticsReport::from_diagnostics(entries));
        }
    };

    let mut parser = Parser::new(tokens);
    let module = match parser.parse_module() {
        Ok(m) => m,
        Err(parse_err) => {
            let entry = DiagnosticEntry {
                error_code: "RTS-E0201".to_string(),
                category: DiagnosticCategory::SyntaxError,
                file: file_path.to_string(),
                span: parse_err.span,
                ast_node: "ParserNode".to_string(),
                message: parse_err.message,
                deterministic_fixes: vec![],
            };
            return (
                None,
                CompilerDiagnosticsReport::from_diagnostics(vec![entry]),
            );
        }
    };

    let analyzer = SemanticAnalyzer::new(file_path);
    let report = analyzer.analyze_module(&module);
    (Some(module), report)
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
}
