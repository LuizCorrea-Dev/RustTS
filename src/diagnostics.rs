use crate::lexer::Span;
use serde::{Deserialize, Serialize};

pub const COMPILER_VERSION: &str = "2.2.0-rts";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticCategory {
    SyntaxError,
    ModuleResolutionError,
    CapabilityViolation,
    TransitiveCapabilityViolation,
    CapabilityForgeryAttempt,
    ArenaLifetimeEscape,
    TypeMismatch,
    NonExhaustiveMatch,
    TierZeroForbiddenDomain,
    AliasingViolation,
    GpuForbiddenCapability,
    GpuLayoutAlignmentViolation,
    GpuRecursionOrDynamicEscape,
    WasiSandboxContractViolation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DeterministicFix {
    RemoveUnauthorizedCall {
        action: String,
        line_to_remove: usize,
        security_policy: String,
    },
    RemoveArenaEscapeAssignment {
        action: String,
        line_to_remove: usize,
        expected_arena_rank: String,
    },
    AddParameterCapabilityOpenContract {
        action: String,
        replacement_signature: String,
        contract_mode: String,
    },
}

impl DeterministicFix {
    /// Em contratos selados (`@sealed_sig`, padrão da linguagem), a assinatura da função é a fronteira
    /// de segurança do arquiteto. O compilador PROÍBE sugerir escalação de privilégio para a IA.
    pub fn remove_unauthorized_call_sealed(line_to_remove: usize) -> Self {
        Self::RemoveUnauthorizedCall {
            action: "remove_unauthorized_call".to_string(),
            line_to_remove,
            security_policy:
                "SealedSignatureBoundary: AI agent is forbidden from mutating function signature to escalate privileges."
                    .to_string(),
        }
    }

    /// Apenas quando o arquiteto marca explicitamente `@open_sig`, o compilador permite sugerir evolução da assinatura.
    pub fn add_cap_open_contract(replacement_signature: String) -> Self {
        Self::AddParameterCapabilityOpenContract {
            action: "add_parameter_capability".to_string(),
            replacement_signature,
            contract_mode: "@open_sig (explicitly unlocked by architect)".to_string(),
        }
    }

    pub fn remove_arena_escape(line_to_remove: usize, expected_arena_rank: String) -> Self {
        Self::RemoveArenaEscapeAssignment {
            action: "remove_arena_lifetime_escape".to_string(),
            line_to_remove,
            expected_arena_rank,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticEntry {
    pub error_code: String,
    pub category: DiagnosticCategory,
    pub file: String,
    pub span: Span,
    pub ast_node: String,
    pub message: String,
    pub deterministic_fixes: Vec<DeterministicFix>,
}

/// Payload raiz do protocolo `rtsc check --json-diagnostics` para realimentação de LLMs em <15ms.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompilerDiagnosticsReport {
    pub status: String,
    pub compiler_version: String,
    pub diagnostics: Vec<DiagnosticEntry>,
}

impl CompilerDiagnosticsReport {
    pub fn ok() -> Self {
        Self {
            status: "ok".to_string(),
            compiler_version: COMPILER_VERSION.to_string(),
            diagnostics: Vec::new(),
        }
    }

    pub fn from_diagnostics(diagnostics: Vec<DiagnosticEntry>) -> Self {
        let status = if diagnostics.is_empty() {
            "ok".to_string()
        } else {
            "error".to_string()
        };
        Self {
            status,
            compiler_version: COMPILER_VERSION.to_string(),
            diagnostics,
        }
    }

    /// Serializa o relatório de diagnóstico no schema JSON exato para agentes de IA.
    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// Emite diagnóstico formatado e limpo para o desenvolvedor humano no terminal.
    pub fn to_human_terminal(&self) -> String {
        if self.diagnostics.is_empty() {
            return format!(
                "[rtsc {}] OK: 0 capability violations, 0 transitive leaks, 0 capability forgeries, 0 arena escapes.",
                self.compiler_version
            );
        }

        let mut out = String::new();
        for diag in &self.diagnostics {
            out.push_str(&format!(
                "error[{}]: {:?} in {}:{}:{}\n",
                diag.error_code,
                diag.category,
                diag.file,
                diag.span.line_start,
                diag.span.col_start
            ));
            out.push_str(&format!("  --> AST Node: {}\n", diag.ast_node));
            out.push_str(&format!("  | {}\n", diag.message));
            for fix in &diag.deterministic_fixes {
                match fix {
                    DeterministicFix::RemoveUnauthorizedCall {
                        line_to_remove,
                        security_policy,
                        ..
                    } => {
                        out.push_str(&format!(
                            "  = help [deterministic_fix]: remove unauthorized call at line {} ({})\n",
                            line_to_remove, security_policy
                        ));
                    }
                    DeterministicFix::RemoveArenaEscapeAssignment {
                        line_to_remove,
                        expected_arena_rank,
                        ..
                    } => {
                        out.push_str(&format!(
                            "  = help [deterministic_fix]: remove escaping handle assignment at line {} (requires {})\n",
                            line_to_remove, expected_arena_rank
                        ));
                    }
                    DeterministicFix::AddParameterCapabilityOpenContract {
                        replacement_signature,
                        ..
                    } => {
                        out.push_str(&format!(
                            "  = help [@open_sig fix]: update signature to:\n    {}\n",
                            replacement_signature
                        ));
                    }
                }
            }
            out.push('\n');
        }
        out
    }
}
