# Release Notes — RustTS `v2.0.0` (`rtsc v2.0.0-rts`)

**Data de Lançamento:** 14 de Novembro de 2026  
**Tag Oficial:** `v2.0.0`  
**Versão do Compilador:** `rtsc 2.0.0-rts`  
**Status da Suíte de Testes:** **36/36 testes unitários aprovados (`cargo test`)**  
**Progresso da Régua de Engenharia:** **Ciclo 1 (`100%`) + Ciclo 2 (`100%` — A Próxima Fronteira Concluída)**

---

## Destaques da Versão `v2.0.0` (`rtsc v2.0.0-rts`)

A versão **`v2.0.0`** consolida a conclusão integral do **Ciclo 2 ("A Próxima Fronteira")**, transformando o compilador `rtsc` em uma infraestrutura multi-backend 100% autocontida (zero dependências estáticas C++ quebradiças, compilando do zero via `cargo install --path .` em ~30 segundos) capaz de emitir diretamente **MLIR SSA**, **LLVM IR nativo (`.ll`) sem passar por C99**, **Shaders GPU Compute (WebGPU WGSL e Vulkan/DX12 SPIR-V 1.5)** e **Sandboxes WebAssembly/WASI com *Fuel Metering* determinístico**.

---

### 1. Fase 6 (`v1.6.0-rts`) — Dialeto Intermediário MLIR SSA (`--target=mlir`)
- **Emissão Direta de MLIR (`src/codegen_mlir.rs`):** Lowering da AST tipada da RustTS para os dialetos `builtin`, `func`, `scf`, `memref`, `arith` e `vector`.
- **Metadados de Capacidades e Aliasing:** Colunas SoA (`MutColumn<T>` e `ReadColumn<T>`) preservam atributos `{rts.cap = "...", llvm.noalias, llvm.align = 64 : i64}` e loops `for i in 0..count` geram `scf.for` com `{rts.simd_tile = 256 : i32, rts.vectorize_width = 8 : i32}`.

---

### 2. Fase 7 (`v1.7.0-rts`) — Backend Direto LLVM IR (`.ll`) & Compilação Nativa Sem C99 (`--target=llvm-ir` e `rtsc run --llvm`)
- **Gerador LLVM IR SSA Tipado (`src/codegen_llvm.rs`):** Emite módulos `.ll` completos com `target datalayout`, `target triple` nativo, ponteiros opacos (`ptr noalias nocapture noundef align 64`), descritores SoA (`%Comp_soa_column_t`), `Option<T>` / `Result<T, E>` (`{ i1, T }`) e remoção *zero-overhead* de tokens `cap` em declarações e chamadas FFI (`extern "C" fn`).
- **Fast-Math & Intrínsecos LLVM:** Emite `fadd fast`, `fsub fast`, `fmul fast`, `fdiv fast`, `fcmp fast`, intrínsecos `@llvm.fmuladd.f32`, `@llvm.sqrt.f32`, `@llvm.sin.f32`, `@llvm.cos.f32`, `@llvm.fabs.f32` e metadados `!llvm.loop` (`vectorize.enable = true`, `vectorize.width = 8`).
- **Execução Direta Sem C99:** `rtsc build app.rts --target llvm-ir --bin` e `rtsc run app.rts --llvm` invocam `clang -x ir -O3 -mavx2 -mfma` diretamente sobre o `.ll` gerado.

---

### 3. Fase 8 (`v1.8.0-rts`) — GPU Compute (`@gpu_kernel` & `@shader_shared`) $\rightarrow$ WebGPU WGSL & Vulkan/DX12 SPIR-V 1.5
- **Verificador Semântico Estático de GPU (`RTS-E0601` a `RTS-E0603` em `src/sema.rs`):**
  - `RTS-E0601 (GpuForbiddenCapability)`: Bloqueia capacidades de CPU/Host/IO (`StdIO`, `I2CBus`, `PWMPin`, `SqlRead`, `NetSocket`) e chamadas FFI/`io::*` dentro de `@gpu_kernel`.
  - `RTS-E0602 (GpuLayoutAlignmentViolation)`: Garante alinhamento `std430`/`std140` em structs `@shader_shared`.
  - `RTS-E0603 (GpuRecursionOrDynamicEscape)`: Proíbe recursão direta ou mútua em kernels de GPU.
- **Emissor WebGPU WGSL (`--target=wgsl` em `src/codegen_gpu.rs`):** Gera `@compute @workgroup_size(64, 1, 1)` e mapeia automaticamente `ReadColumn<T>`, `MutColumn<T>` e `TimeStep` para `@group(0) @binding(N) var<storage, read / read_write>` e `var<uniform>`.
- **Emissor SPIR-V 1.5 (`--target=spirv` / `--target=spv`):** Gera tanto assembly textual auditável (`.spvasm`: `OpCapability Shader`, `OpMemoryModel Logical GLSL450`, `OpEntryPoint GLCompute`) quanto bytecode binário de words de 32 bits little-endian (`.spv`, magic `0x07230203`, versão `0x00010500`, generator `"RTSC"` `0x52545343`).

---

### 4. Fase 9 (`v1.9.0-rts`) — Sandbox WASI com *Fuel Metering* Determinístico (`--target=wasi` / `--target=wasm` e `--fuel-limit <N>`)
- **Verificador de Contratos de Sandbox (`RTS-E0701` em `src/sema.rs`):** Exige fronteira `@sealed_sig` em funções exportadas por módulos `sandbox.*` / `wasi.*` / `plugin.*` e bloqueia capacidades físicas de hardware.
- **Instrumentação Automática de Combustível (`src/codegen_wasi.rs`):** Injeta `(global $__rts_fuel (mut i64) ...)`, exporta `rts_set_fuel`, `rts_get_fuel` e `rts_arena_reset`, e insere checagem determinística `$O(1)$` no prólogo de cada função e no *back-edge* de cada loop (`while`/`for`), disparando `unreachable` se `$__rts_fuel < 0`.
- **Empacotador Binário `.wasm` com Lowering Real da AST (`emit_wasm_binary`):** Emite binários WebAssembly 1.0 completos (cabeçalho `\0asm` + versão `1` + seções LEB128) codificando as assinaturas reais de parâmetros/retorno na *Type Section*, variáveis locais na *Code Section* e opcodes reais de aritmética (`f32`/`f64`/`i32`/`i64`), controle de fluxo estruturado (`block`/`loop`/`br_if`/`if`/`else`) e *Fuel Metering* em cada loop.
- **Imutabilidade Estrita de Variáveis Locais (`let` vs `let mut` — `RTS-E0405`):** O verificador semântico (`src/sema.rs`) rastreia `immutable_locals` e bloqueia em tempo de compilação qualquer tentativa de reatribuir variáveis locais declaradas sem `mut`.

---

### 5. Fase 10 (`v2.0.0-rts`) — Showcase Heterogêneo `examples/kanori_gpu_wasi_sim/` & Extensão `v2.0.0`
- Novo showcase multi-arquivo integrando CPU (`--target=llvm-ir`), GPU Compute (`--target=wgsl` / `--target=spirv`) e Sandbox WASI (`--target=wasi --fuel-limit 100000`).
- Pacote oficial `editors/rustts-lang-2.0.0.vsix` atualizado com snippet `gpukernel` (e remoção do pacote legado `1.4.0`).

---

## Comandos Rápidos (`rtsc v2.0.0-rts`)

```powershell
# 1. Instalar o compilador v2.0.0-rts globalmente
cargo install --path .

# 2. Executar toda a suíte de 36 testes unitários
cargo test

# 3. Executar o showcase heterogêneo v2.0.0 diretamente via LLVM IR (sem C99)
rtsc run examples/kanori_gpu_wasi_sim/src/main.rts --llvm

# 4. Emitir WebGPU WGSL e Vulkan SPIR-V (.spvasm e .spv binário)
rtsc build examples/kanori_gpu_wasi_sim/src/gpu_particles.rts --target wgsl -o build/gpu_particles.wgsl
rtsc build examples/kanori_gpu_wasi_sim/src/gpu_particles.rts --target spirv -o build/gpu_particles.spvasm
rtsc build examples/kanori_gpu_wasi_sim/src/gpu_particles.rts --target spv -o build/gpu_particles.spv

# 5. Emitir Sandbox WASI com Fuel Metering (.wat e .wasm binário)
rtsc build examples/kanori_gpu_wasi_sim/src/sandbox_agent.rts --target wasi --fuel-limit 50000 -o build/sandbox_agent.wat
rtsc build examples/kanori_gpu_wasi_sim/src/sandbox_agent.rts --target wasm --fuel-limit 50000 -o build/sandbox_agent.wasm
```
