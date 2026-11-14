<p align="center">
  <img src="./rustts_language_logo.png" alt="RustTS Logo" width="220">
</p>

# RustTS (`.rts`) — Deterministic Systems & Engine Language for Humans and AI

[![Compiler: rtsc](https://img.shields.io/badge/compiler-rtsc%20(Rust%2FMLIR)-orange.svg)](#)
[![Targets: LLVM | WASI | Unreal | Unity | WebGPU](https://img.shields.io/badge/targets-LLVM%20%7C%20WASI%20%7C%20UE5%20%7C%20Unity%20%7C%20WebGPU-blue.svg)](#)
[![Memory: Scoped Arenas (Zero GC)](https://img.shields.io/badge/memory-Scoped%20Arenas%20(Zero%20GC)-success.svg)](#)
[![AI Native: GBNF + JSON AST Diagnostics](https://img.shields.io/badge/AI--Native-GBNF%20%2B%20JSON%20Diagnostics-purple.svg)](#)

**RustTS** é uma linguagem de programação de sistemas, motores de jogos e servidores de alta performance projetada sob uma premissa de engenharia: **permitir que desenvolvedores humanos e modelos de Inteligência Artificial (Gemini, Claude, GPT, DeepSeek, Qwen) escrevam código de baixo nível na mesma língua, com zero alucinação sintática, tempo de quadro previsível e segurança física de hardware.**

---

## O Problema que a RustTS Resolve

Linguagens tradicionais falham quando colocadas no fluxo de trabalho entre um engenheiro de sistemas e uma IA generativa:
1. **C e C++** oferecem controle total do hardware, mas dão autoridade ambiental irrestrita ao código: um único ponteiro inválido ou macro mal gerada pela IA causa *Segmentation Fault*, corrupção silenciosa de memória ou falha em drivers físicos.
2. **Rust** resolve a segurança de memória, mas impõe um *Borrow Checker* com anotações de *lifetimes* complexas e macros procedurais que fazem LLMs e humanos perderem horas em ciclos de compilação lentos, especialmente em grafos de cena de jogos e arquiteturas ECS.
3. **Linguagens com Garbage Collector (C#, Go, Python, JS)** sofrem com pausas não-determinísticas de coleta de lixo, layout de memória fragmentado (*cache misses* constantes em L1/L2) e alto consumo de recursos em servidores e motores de tempo real.

A **RustTS** elimina esses gargalos unindo a legibilidade direta do **TypeScript** com a semântica de sistemas do **Rust**, substituindo o *Garbage Collector* e os *lifetimes* complexos por **Arenas de Memória Escopadas ($O(1)$)** e **Segurança Baseada em Capacidades (`cap`)**.

---

## Os 4 Pilares Arquiteturais

### 1. Sintaxe Canônica "Zero-Novelty" + Restrição GBNF
* **Sem Invenções Exóticas:** A gramática da RustTS é um subconjunto estrito de TypeScript e Rust (`fn`, `let`, `struct`, `enum`, tipos após `:`). Como os LLMs já possuem trilhões de tokens de treinamento nessas duas linguagens, qualquer modelo programa em RustTS em modo *zero-shot* sem precisar da documentação no prompt.
* **Forma Canônica Única:** Existe apenas uma forma válida de declarar estruturas, iterar sobre coleções e tratar erros (`Result<T, E>`). Sem sobrecarga de operadores, sem herança de classes, sem macros obscuras.
* **Exportação Nativa GBNF:** O compilador `rtsc` exporta sua gramática formal em formato `.gbnf` para motores de inferência (`llama.cpp`, `vLLM`, *Structured Outputs*), travando a geração de tokens da IA para tornar erros de sintaxe matematicamente impossíveis.

### 2. Segurança e Paralelismo por Capacidades (`cap`)
* **Zero Autoridade Ambiental:** Nenhuma função em RustTS pode acessar disco, rede, banco de dados, pinos de hardware ou colunas de memória global a menos que receba explicitamente o token de capacidade (`cap`) em sua assinatura.
* **Multithreading Automático Livre de Locks:** Como toda função declara estaticamente quais fatias de memória lê (`cap ReadColumn<T>`) e quais altera (`cap MutColumn<T>`), o compilador `rtsc` constrói o grafo de dependências e paraleliza a execução em múltiplos núcleos da CPU com garantia matemática contra *Data Races*.

### 3. Memória Determinística via Arenas Escopadas (Zero GC)
Proíbe-se o uso de `malloc`/`free` manuais ou *Garbage Collection*. Toda alocação dinâmica ocorre em arenas lineares atreladas ao ciclo de vida da operação, liberadas em tempo constante $O(1)$ apenas zerando o ponteiro de deslocamento (`offset = 0`):
* `StaticArena`: Memória permanente alocada na inicialização do sistema ou firmware.
* `LevelArena` / `SessionArena`: Memória persistente durante uma fase de jogo ou sessão de conexão.
* `FrameArena`: Memória temporária de um único quadro de renderização/física (ex: 16,6ms a 60 FPS), zerada automaticamente ao fim do frame.
* `RequestArena`: Memória dedicada a uma única requisição HTTP/gRPC/WebSocket, limpa instantaneamente após o envio da resposta.

### 4. Compilador Orientado a Auto-Correção (`--json-diagnostics`)
Em vez de emitir apenas textos longos para humanos no terminal, o compilador `rtsc` possui um modo nativo para agentes de IA que retorna erros semânticos e violações de capacidade em um payload JSON enxuto (menos de 15ms), contendo o nó exato da AST e o *patch* determinístico de correção.

---

## Matriz de Escopo Operacional (Onde Usar e Onde NÃO Usar)

A RustTS **não é um "canivete suíço"**. Ela foi construída para domínios onde performance previsível, controle de memória e segurança de execução são inegociáveis.

| Classificação | Domínio de Aplicação | Modelo de Memória | Capacidades Típicas (`cap`) | Alvo de Compilação (`rtsc`) | Status |
|---|---|---|---|---|---|
| **Tier 1 (Núcleo)** | **Motores de Jogos (2D / 2.5D / 3D) e Simulação** | `StaticArena`, `LevelArena`, `FrameArena` + ECS (`SoA`) | `MutColumn<T>`, `ReadColumn<T>`, `GpuQueue`, `AudioBuffer` | Nativo (LLVM / SIMD + SPIR-V / DXIL) ou Transpilação Unreal/Unity | **Suporte Nativo** |
| **Tier 1 (Núcleo)** | **Hardware, Embarcados e Sistemas Bare-Metal** | `StaticArena` + Mapeamento Direto (MMIO) | `I2CPort`, `SPIBus`, `GPIOPin`, `DMAChannel`, `Interrupt` | Nativo Bare-Metal (ARM / RISC-V / x86 via LLVM + C ABI) | **Suporte Nativo** |
| **Tier 1 (Núcleo)** | **Servidores de Alta Performance, Multiplayer e APIs** | `StaticArena`, `SessionArena`, `RequestArena` | `HttpRoute`, `TcpSocket`, `UdpTick`, `SqlRead<T>`, `SqlWrite<T>` | Nativo (Linux/Windows Server) ou Sandbox WASI Isolada | **Suporte Nativo** |
| **Tier 2 (Exceção)** | **Frontend Gráfico no Navegador (WebGPU / WebGL)** | `StaticArena`, `FrameArena` (Memória Linear WASM) | `CanvasContext`, `WebGpuDevice`, `InputState` | `wasm32-unknown-wasi` + Bindings Diretos WebGPU / WebGL2 | **Exceção Estrita** |
| **Tier 0 (Banido)** | **Frontend Web Tradicional (DOM, HTML, CSS, UI)** | *Incompatível (Exige GC e Árvore DOM Mutável)* | *Nenhuma (Proibido na linguagem)* | *Rejeitado pelo Compilador* | **Estritamente Proibido** |

> **Regra de Arquitetura Web:** Nunca utilize RustTS para criar interfaces HTML/CSS/DOM. Construa o Backend/API em **RustTS** e utilize a flag `--emit-ts-sdk` do compilador para gerar automaticamente o cliente TypeScript tipado para consumo em **Next.js / React + Tailwind CSS**.

---

## Exemplos de Código (`.rts`)

### 1. Motores de Jogos: ECS Nativo, `FrameArena` e Vetorização SIMD (`examples/engine_physics.rts`)
```rust
module engine.physics;

@component
struct Transform {
    position: vec3,
    rotation: quat,
    scale: vec3,
}

@component
struct Velocity {
    linear: vec3,
    damping: f32,
}

struct CollisionEvent {
    entity_a: Handle<Entity>,
    entity_b: Handle<Entity>,
    impact_point: vec3,
}

// O compilador garante ausência de aliasing entre 'transforms' e 'velocities',
// vetorizando o loop em instruções AVX2/NEON ou LLVM IR automaticamente.
@system(stage = "FixedUpdate")
pub fn integrate_kinematics(
    transforms: cap MutColumn<Transform>,
    velocities: cap ReadColumn<Velocity>,
    events: cap FrameArena<CollisionEvent>,
    time: cap TimeStep
) {
    let dt: f32 = time.fixed_delta;
    let count: usize = transforms.len();

    for i in 0..count {
        let vel: vec3 = velocities[i].linear;
        transforms[i].position = transforms[i].position + (vel * dt);

        if transforms[i].position.y < 0.0 {
            transforms[i].position.y = 0.0;
            // Alocação O(1) na arena do frame atual (reciclada a cada 16.6ms; proibida de escapar via RTS-E0501)
            events.push(CollisionEvent {
                entity_a: transforms.handle_at(i),
                entity_b: Handle::null(),
                impact_point: transforms[i].position,
            });
        }
    }
}
```

### 2. Hardware e Embarcados: Controle Físico Blindado por Capacidades (`examples/thermal_control.rts`)
```rust
module drone.thermal_control;
import core.hardware;
import core.math;

struct SensorConfig {
    target_temp_celsius: f32,
    max_pwm_duty: u8,
}

enum HardwareFault {
    I2CTimeout(u8),
    SensorDisconnected,
    OverheatCritical(f32),
}

// Se uma IA tentar acessar rede, disco ou outro pino de hardware dentro desta função,
// a compilação é abortada imediatamente no estágio de Capability Checking (RTS-E0402).
pub fn regulate_motor_cooling(
    config: SensorConfig,
    i2c: cap I2CBus<0x48>,
    pwm: cap PWMPin<4>,
    clock: cap MonotonicClock
) -> Result<f32, HardwareFault> {
    let raw_data: u16 = i2c.read_register_u16(0x00)
        .map_err(|_| HardwareFault::I2CTimeout(0x48))?;

    if raw_data == 0xFFFF {
        return Err(HardwareFault::SensorDisconnected);
    }

    let current_temp: f32 = (raw_data as f32) * 0.0625;

    if current_temp >= 105.0 {
        pwm.set_duty(100);
        return Err(HardwareFault::OverheatCritical(current_temp));
    }

    if current_temp > config.target_temp_celsius {
        let delta: f32 = current_temp - config.target_temp_celsius;
        let duty: u8 = math::clamp(delta * 5.0, 20.0, config.max_pwm_duty as f32) as u8;
        pwm.set_duty(duty);
    } else {
        pwm.set_duty(0);
    }

    return Ok(current_temp);
}
```

### 3. Compute Shaders GPU (`@gpu_kernel`) e Sandbox WASI com *Fuel Metering* (`examples/kanori_gpu_wasi_sim/`)
```rust
module kanori.gpu_particles;

@shader_shared
struct RadiationParticle {
    pos_x: f32,
    pos_y: f32,
    vel_x: f32,
    vel_y: f32,
    flux_sv: f32
}

// Compila nativamente para WebGPU (--target wgsl) e Vulkan/DX12 SPIR-V 1.5 (--target spirv / spv)
@gpu_kernel
pub fn step_radiation_field(
    particles: cap MutColumn<RadiationParticle>,
    dt: cap TimeStep
) {
    let dt_val: f32 = dt.fixed_delta;
    let count: usize = particles.len();
    for i in 0..count {
        particles[i].pos_x = particles[i].pos_x + particles[i].vel_x * dt_val;
        particles[i].pos_y = particles[i].pos_y + particles[i].vel_y * dt_val;
        particles[i].flux_sv = math::lerp(particles[i].flux_sv, 0.0, dt_val * 0.05);
    }
}

// Compila para WebAssembly/WASI (--target wasi / wasm --fuel-limit 75000) com Fuel Metering determinístico
@sealed_sig
pub fn decidir_evacuacao_agente(
    dose_inicial: float,
    incremento_por_tick: float,
    ticks_simulacao: int
) -> int {
    let mut acumulado: float = dose_inicial;
    let mut passo: int = 0;
    while passo < ticks_simulacao {
        acumulado = acumulado + incremento_por_tick;
        passo = passo + 1;
        if acumulado > 300.0 {
            return 1;
        }
    }
    return 0;
}
```

---

## Instalação Rápida (`v2.0.0-rts`)

**Via código-fonte (`v2.0.0-rts` — recomendado, requer Rust/Cargo):**
```bash
git clone https://github.com/LuizCorrea-Dev/RustTS.git
cd RustTS
cargo install --path .
rtsc --version
# rtsc 2.0.0-rts
```

**Instaladores pré-compilados (Release GitHub [`v2.0.0`](https://github.com/LuizCorrea-Dev/RustTS/releases/tag/v2.0.0)):**

**Windows (PowerShell):**
```powershell
irm https://raw.githubusercontent.com/LuizCorrea-Dev/RustTS/main/scripts/install.ps1 | iex
```

**Linux / macOS:**
```bash
curl -fsSL https://raw.githubusercontent.com/LuizCorrea-Dev/RustTS/main/scripts/install.sh | bash
```

Consulte o **[Guia Completo de Instalação e Uso (`INSTALACAO_E_USO.md`)](./INSTALACAO_E_USO.md)** para detalhes da toolchain, compilação AVX2/Clang/LLVM, shaders GPU (WGSL/SPIR-V), sandbox WASI e tutorial passo a passo.

---

## Interface de Linha de Comando (`rtsc 2.0.0-rts`)

O compilador `rtsc` centraliza criação de projetos, verificação semântica e compilação heterogênea (CPU, GPU e WASI):

```bash
# 1. Criar novo projeto autônomo com `pub fn main()` e `rts.toml`
rtsc init meu_projeto

# 2. Compilar e executar diretamente (via C99/Clang ou direto via LLVM IR com `--llvm`, ou debug `-g`)
rtsc run examples/kanori_gpu_wasi_sim/src/main.rts --llvm
rtsc run examples/kanori_zone_sim/src/main.rts -g

# 3. Validar código e projetos multi-arquivo com saída humana ou JSON (< 15ms)
rtsc check examples/kanori_gpu_wasi_sim/src/main.rts
rtsc check examples/thermal_control.rts --json-diagnostics

# 4. Backends CPU: C99 AVX2/SoA, MLIR SSA (`.mlir`) e LLVM IR Direto (`.ll` / `--bin`)
rtsc build examples/kanori_zone_sim/src/main.rts --out-dir build/kanori --emit-header
rtsc build examples/kanori_gpu_wasi_sim/src/main.rts --target mlir -o build/kanori.mlir
rtsc build examples/kanori_gpu_wasi_sim/src/main.rts --target llvm-ir --bin -o build/kanori_llvm.exe

# 5. Backends GPU Compute (`@gpu_kernel`): WebGPU (`.wgsl`) e Vulkan/DX12 SPIR-V 1.5 (`.spvasm` / `.spv`)
rtsc build examples/kanori_gpu_wasi_sim/src/gpu_particles.rts --target wgsl -o build/gpu_particles.wgsl
rtsc build examples/kanori_gpu_wasi_sim/src/gpu_particles.rts --target spirv -o build/gpu_particles.spvasm
rtsc build examples/kanori_gpu_wasi_sim/src/gpu_particles.rts --target spv -o build/gpu_particles.spv

# 6. Backend Sandbox WASI Determinístico com Fuel Metering (`.wat` e binário `.wasm`)
rtsc build examples/kanori_gpu_wasi_sim/src/sandbox_agent.rts --target wasi --fuel-limit 75000 -o build/agent.wat
rtsc build examples/kanori_gpu_wasi_sim/src/sandbox_agent.rts --target wasm --fuel-limit 75000 -o build/agent.wasm

# 7. Transpilar módulos Unreal Engine 5 (.h/.cpp UHT), Unity (C-ABI), TypeScript SDK e gramática GBNF
rtsc build examples/combat_unreal.rts --target=unreal --out-dir build/unreal
rtsc build examples/engine_physics.rts --target=unity --out-dir build/unity
rtsc build examples/telemetry_server.rts --emit-ts-sdk -o build/telemetry_client_sdk.ts
rtsc --export-gbnf > rustts.gbnf
```

---

## Exemplo de Diagnóstico JSON para Auto-Correção de IA

Quando um agente de IA tenta violar uma regra de memória ou permissão de hardware, `rtsc check --json-diagnostics` emite o schema `CompilerDiagnosticsReport`:

```json
{
  "compiler_version": "2.0.0-rts",
  "status": "error",
  "diagnostics": [
    {
      "error_code": "RTS-E0402",
      "category": "CapabilityViolation",
      "file": "src/server.rts",
      "span": {
        "line_start": 18,
        "col_start": 5,
        "line_end": 18,
        "col_end": 28
      },
      "ast_node": "MethodCall(db.delete_by_id)",
      "message": "Function 'get_user_profile' attempted to invoke 'db.delete_by_id' which requires capability 'cap SqlWrite<UsersTable>', but only holds 'cap SqlRead<UsersTable>'.",
      "deterministic_fixes": [
        {
          "strategy": "remove_unauthorized_call",
          "rationale": "Function is marked @sealed_sig; privilege escalation is forbidden.",
          "replacement_snippet": "// REMOVED BY RTSC GUARD",
          "target_line": 18
        }
      ]
    }
  ]
}
```

---

## Estrutura do Repositório e Documentação

* [`INSTALACAO_E_USO.md`](./INSTALACAO_E_USO.md) — **Guia Oficial de Instalação e Uso (`v2.0.0-rts`):** Instalação, build via Cargo, manual da CLI `rtsc`, backends MLIR/LLVM/SPIR-V/WGSL/WASI, exemplos e solução de problemas.
* [`INICIANDO_UM_PROJETO.md`](./INICIANDO_UM_PROJETO.md) — **Tutorial: Iniciando um Projeto com RustTS (`v2.0.0-rts`):** Ergonomia TypeScript/C, `rtsc init`, `pub fn main()`, `rtsc run [--llvm] [-g]`, `@gpu_kernel` (SPIR-V/WGSL), sandbox WASI (`--fuel-limit`), `import`, FFI e `--emit-header`.
* [`MANUAL_TIPOS_RUSTTS.md`](./MANUAL_TIPOS_RUSTTS.md) — **Manual Oficial de Tipos, Capacidades e Memória (`v2.0.0-rts`):** Referência completa dos 14 tipos escalares e seus aliases (TS / C / C# / Unreal Engine 5), inferência `let`, tipos SIMD, coleções `[T; N]`, tipos algébricos e tokens `cap`.
* [`Guia_Interativo_e_Documentacao_RustTS.html`](./Guia_Interativo_e_Documentacao_RustTS.html) — **Guia Interativo & Documentação HTML:** Dashboard interativo com documentação completa da linguagem e Cheat Sheet (`v2.0.0-rts`).
* [`CONTRIBUTING.md`](./CONTRIBUTING.md) — **Guia de Contribuição:** Mapa do compilador (`src/`), padrões de código, suíte `cargo test` e checklist para Pull Requests.
* [`devlog.md`](./devlog.md) — **Devlog & Cronologia (`v0.1.0-rts` → `v2.0.0-rts`):** Histórico técnico completo de versões, auditorias, suíte `cargo test` (`36/36`) e benchmarks AVX2/SoA.
* [`RustTS_guia.md`](./RustTS_guia.md) — **Matriz de Escopo Operacional e Domínios:** Regras de aplicação Tier 1 / Tier 2 / Tier 0 e integração com TypeScript SDK.
* [`editors/vscode-rustts/`](./editors/vscode-rustts/package.json) — **Extensão Oficial VS Code / Antigravity IDE (`v2.0.0`):** Syntax highlighting TextMate, snippets (incluindo `gpukernel`) e pacote `editors/rustts-lang-2.0.0.vsix` para arquivos `.rts`.
* [`examples/kanori_zone_sim/`](./examples/kanori_zone_sim/src/main.rts) — **Showcase Multi-Arquivo KANORI (`v1.5.0`):** Projeto integrado com `import`, `impl Struct`, `core.math`, `Option<T>`, `extern "C"` e `rtsc run -g`.
* [`examples/kanori_gpu_wasi_sim/`](./examples/kanori_gpu_wasi_sim/src/main.rts) — **Showcase Heterogêneo KANORI (`v2.0.0`):** Pipeline unificado CPU (`--target llvm-ir`) + GPU Compute (`@gpu_kernel` $\rightarrow$ `spirv`/`wgsl`) + Sandbox WASI com *Fuel Metering* (`--target wasi --fuel-limit 100000`).
* [`agenteRTS.md`](./agenteRTS.md) — **System Prompt do Agente Especialista:** Configuração de contexto para transformar LLMs em engenheiros de compiladores (`COMPILER-DEV`) e programadores nativos RustTS (`RTS-CODER`).
---

## Roadmap de Implementação (`rtsc`)

- [x] **Fase 1 (`v0.1.0` → `v1.0.0-rts`):** Front-end (`logos` + parser LL), GBNF, `TypeChecker` (`RTS-E0301`), `match` exaustivo (`RTS-E0302`), capacidades (`RTS-E0402`–`E0405`), arenas $O(1)$ (`RTS-E0501`) e codegen C99 AVX2/SoA, Unreal UHT, Unity C-ABI e TS SDK.
- [x] **Fase 2 (`v1.1.0-rts`):** Autonomia da CLI (`rtsc init`, `rtsc run`, `-o`, `--out-dir`, `--bin`), remoção de UTF-8 BOM, `RTS_UNUSED`, `io::*` e entrypoint nativo `pub fn main()`.
- [x] **Fase 3 (`v1.2.0-rts`):** Projetos multi-arquivo (`import` recursivo com detecção de ciclos e `RTS-E0102`), forward declarations C99, `Option<T>`, arrays `[T; N]` e `break`/`continue`.
- [x] **Fase 4 (`v1.3.0-rts`):** FFI `extern "C" fn` blindado por `cap` com remoção zero-overhead, blocos `impl Struct`, biblioteca matemática `core.math` e flags `-l`, `-L`, `-I`.
- [x] **Fase 5 (`v1.4.0-rts`):** Diretivas `#line` + flag `-g`/`--debug`, exportação de header C/C++ (`--emit-header`), extensão VS Code e showcase multi-arquivo `examples/kanori_zone_sim/`.
- [x] **Fase 5.1 (`v1.5.0-rts`):** Ergonomia TypeScript/C com inferência completa de tipos no `let` e família completa de aliases para os 14 tipos escalares (`int8..int64`, `uint8..uint64`, `int`, `uint`, `byte`, `float`, `double`, `number`, `boolean`, `string`).
- [x] **Fase 6 (`v1.6.0-rts`):** Backend intermediário MLIR SSA (`--target=mlir`: dialetos `func`, `scf`, `memref`, `arith`, `vector` com metadados `rts.cap` e `llvm.noalias`).
- [x] **Fase 7 (`v1.7.0-rts`):** Backend direto LLVM IR (`.ll`) e compilação nativa sem C99 (`--target=llvm-ir --bin` e `rtsc run --llvm` via `clang -x ir -O3 -mavx2 -mfma`).
- [x] **Fase 8 (`v1.8.0-rts`):** Backend GPU Compute (`@gpu_kernel` e `@shader_shared`) compilando diretamente para WebGPU (`--target=wgsl`) e Vulkan/DX12 SPIR-V 1.5 (`--target=spirv` `.spvasm` e binário `.spv` magic `0x07230203`), com verificação estática `RTS-E0601`–`E0603`.
- [x] **Fase 9 & 10 (`v1.9.0-rts` → `v2.0.0-rts`):** Sandbox WASI determinística (`.wat` e binário `.wasm` magic `\0asm` com lowering real da AST) com *Fuel Metering* automático (`--fuel-limit <N>` + `RTS-E0701`), validação de imutabilidade local `let` vs `let mut` (`RTS-E0405`) e showcase heterogêneo `examples/kanori_gpu_wasi_sim/` (`cargo test` **36/36 PASS**).

---

## Licença

Distribuído sob a licença MIT / Apache 2.0. Consulte o arquivo `LICENSE` para mais detalhes.