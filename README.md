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

### 1. Motores de Jogos: ECS Nativo, `FrameArena` e Vetorização SIMD
```rust
struct Transform {
    x: f32,
    y: f32,
    z: f32,
}

struct Velocity {
    dx: f32,
    dy: f32,
    dz: f32,
}

struct CollisionEvent {
    entity_id: u32,
    impact_force: f32,
}

// O compilador garante ausência de aliasing entre 'pos' e 'vel',
// vetorizando o loop em instruções AVX2/NEON automaticamente.
fn update_physics_system(
    pos: cap MutColumn<Transform>,
    vel: cap ReadColumn<Velocity>,
    events: cap FrameArena<CollisionEvent>,
    dt: f32
) -> Result<u32, EngineError> {
    let count: usize = pos.len();
    let mut hits: u32 = 0;

    for i in 0..count {
        pos[i].x = pos[i].x + (vel[i].dx * dt);
        pos[i].y = pos[i].y + (vel[i].dy * dt);
        pos[i].z = pos[i].z + (vel[i].dz * dt);

        if pos[i].y < 0.0 {
            pos[i].y = 0.0;
            // Alocação O(1) na arena do frame atual (limpa ao final dos 16.6ms)
            events.push(CollisionEvent {
                entity_id: i as u32,
                impact_force: vel[i].dy,
            })?;
            hits = hits + 1;
        }
    }

    return Ok(hits);
}
```

### 2. Hardware e Embarcados: Controle Físico Blindado por Capacidades
```rust
enum ThermalError {
    BusTimeout,
    SensorDisconnected,
}

// Se uma IA tentar acessar rede, disco ou outro pino de hardware dentro desta função,
// a compilação é abortada imediatamente no estágio de Capability Checking.
fn regulate_cooling_unit(
    sensor: cap I2CRead<0x48>,
    fan_pwm: cap PWMWrite<Pin4>
) -> Result<u16, ThermalError> {
    let raw_temp: u16 = sensor.read_u16().map_err(ThermalError::BusTimeout)?;

    if raw_temp > 85 {
        fan_pwm.set_duty_cycle(100)?;
    } else if raw_temp > 60 {
        fan_pwm.set_duty_cycle(50)?;
    } else {
        fan_pwm.set_duty_cycle(0)?;
    }

    return Ok(raw_temp);
}
```

### 3. Servidores e APIs: `RequestArena` e Segurança de Banco de Dados
```rust
struct UserProfileDTO {
    id: u64,
    username: str,
    active: bool,
}

// A capacidade 'SqlRead' impede fisicamente qualquer operação de escrita (INSERT/DELETE/DROP).
// Toda memória alocada em 'req_arena' é reciclada em O(1) assim que a resposta HTTP é enviada.
@route("GET", "/api/v1/users/:id")
fn get_user_profile(
    user_id: u64,
    db: cap SqlRead<UsersTable>,
    req_arena: cap RequestArena
) -> Result<Json<UserProfileDTO>, HttpError> {
    let row = db.find_by_id(user_id, req_arena)
        .ok_or(HttpError::NotFound)?;

    let dto = req_arena.alloc(UserProfileDTO {
        id: row.id,
        username: row.username,
        active: row.is_active,
    })?;

    return Ok(Json(dto));
}
```

---

## Instalação Rápida (`v1.4.0-rts`)

**Via código-fonte (`v1.4.0-rts` — recomendado, requer Rust/Cargo):**
```bash
git clone https://github.com/LuizCorrea-Dev/RustTS.git
cd RustTS
cargo install --path .
rtsc --version
# rtsc 1.4.0-rts
```

**Instaladores pré-compilados (Release GitHub [`v1.0.0`](https://github.com/LuizCorrea-Dev/RustTS/releases/tag/v1.0.0)):**

**Windows (PowerShell):**
```powershell
irm https://raw.githubusercontent.com/LuizCorrea-Dev/RustTS/main/scripts/install.ps1 | iex
```

**Linux / macOS:**
```bash
curl -fsSL https://raw.githubusercontent.com/LuizCorrea-Dev/RustTS/main/scripts/install.sh | bash
```

Consulte o **[Guia Completo de Instalação e Uso (`INSTALACAO_E_USO.md`)](./INSTALACAO_E_USO.md)** para detalhes da toolchain, compilação AVX2/Clang e tutorial passo a passo.

---

## Interface de Linha de Comando (`rtsc 1.4.0-rts`)

O compilador `rtsc` centraliza criação de projetos, verificação, compilação nativa e depuração:

```bash
# 1. Criar novo projeto autônomo com `pub fn main()` e `rts.toml`
rtsc init meu_projeto

# 2. Compilar via LLVM/Clang e executar diretamente (use `-g` para símbolos `#line` + `-O0`)
rtsc run examples/kanori_zone_sim/src/main.rts
rtsc run examples/kanori_zone_sim/src/main.rts -g

# 3. Validar código e projetos multi-arquivo com saída humana ou JSON (< 15ms)
rtsc check examples/thermal_control.rts
rtsc check examples/thermal_control.rts --json-diagnostics

# 4. Gerar C99 AVX2/SoA, header C/C++ (`--emit-header`) ou binário (`--bin`), com linkagem externa (`-l/-L/-I`)
rtsc build examples/kanori_zone_sim/src/main.rts --out-dir build/kanori --emit-header
rtsc build src/main.rts --bin -o build/app.exe -l raylib -L ./lib -I ./include

# 5. Transpilar módulos Unreal Engine 5 (.h/.cpp UHT) e Unity (C-ABI + NativeArray<T>)
rtsc build examples/combat_unreal.rts --target=unreal --out-dir build/unreal
rtsc build examples/engine_physics.rts --target=unity --out-dir build/unity

# 6. Gerar SDK TypeScript tipado (Next.js/React) e gramática GBNF para LLMs
rtsc build examples/telemetry_server.rts --emit-ts-sdk -o build/telemetry_client_sdk.ts
rtsc --export-gbnf > rustts.gbnf
```

---

## Exemplo de Diagnóstico JSON para Auto-Correção de IA

Quando um agente de IA tenta violar uma regra de memória ou permissão de hardware, `rtsc check --json-diagnostics` emite:

```json
{
  "status": "error",
  "error_code": "RTS-E0402",
  "category": "CapabilityViolation",
  "location": {
    "file": "src/server.rts",
    "line": 18,
    "column": 5,
    "ast_node": "MethodCall(db.delete_by_id)"
  },
  "message": "Function 'get_user_profile' attempted a write operation ('delete_by_id') on resource 'UsersTable', but only possesses read capability 'cap SqlRead<UsersTable>'.",
  "suggested_fix": {
    "action": "remove_call_or_upgrade_cap",
    "signature_patch": "db: cap SqlWrite<UsersTable>"
  }
}
```

---

## Estrutura do Repositório e Documentação

* [`INSTALACAO_E_USO.md`](./INSTALACAO_E_USO.md) — **Guia Oficial de Instalação e Uso (`v1.4.0-rts`):** Instalação, build via Cargo, manual da CLI `rtsc`, exemplos e solução de problemas.
* [`INICIANDO_UM_PROJETO.md`](./INICIANDO_UM_PROJETO.md) — **Tutorial: Iniciando um Projeto com RustTS (`v1.4.0-rts`):** Fluxo com `rtsc init`, `pub fn main()`, `rtsc run [-g]`, projetos multi-arquivo (`import`), FFI e exportação `--emit-header`.
* [`Guia_Interativo_e_Documentacao_RustTS.html`](./Guia_Interativo_e_Documentacao_RustTS.html) — **Guia Interativo & Documentação HTML:** Dashboard interativo com documentação completa da linguagem e Cheat Sheet (`v1.4.0-rts`).
* [`CONTRIBUTING.md`](./CONTRIBUTING.md) — **Guia de Contribuição:** Mapa do compilador (`src/`), padrões de código, suíte `cargo test` e checklist para Pull Requests.
* [`devlog.md`](./devlog.md) — **Devlog & Cronologia (`v0.1.0-rts` → `v1.4.0-rts`):** Histórico técnico completo de versões, auditorias, suíte `cargo test` (`22/22`) e benchmarks AVX2/SoA.
* [`RustTS_guia.md`](./RustTS_guia.md) — **Matriz de Escopo Operacional e Domínios:** Regras de aplicação Tier 1 / Tier 2 / Tier 0 e integração com TypeScript SDK.
* [`editors/vscode-rustts/`](./editors/vscode-rustts/package.json) — **Extensão Oficial VS Code:** Syntax highlighting TextMate e configuração de linguagem para arquivos `.rts`.
* [`examples/kanori_zone_sim/`](./examples/kanori_zone_sim/src/main.rts) — **Showcase Multi-Arquivo KANORI:** Projeto integrado com `import`, `impl Struct`, `core.math`, `Option<T>`, `extern "C"` e `rtsc run -g`.
* [`agenteRTS.md`](./agenteRTS.md) — **System Prompt do Agente Especialista:** Configuração de contexto para transformar LLMs em engenheiros de compiladores (`COMPILER-DEV`) e programadores nativos RustTS (`RTS-CODER`).
---

## Roadmap de Implementação (`rtsc`)

- [x] **Fase 1 (`v0.1.0` → `v1.0.0-rts`):** Front-end (`logos` + parser LL), GBNF, `TypeChecker` (`RTS-E0301`), `match` exaustivo (`RTS-E0302`), capacidades (`RTS-E0402`–`E0405`), arenas $O(1)$ (`RTS-E0501`) e codegen C99 AVX2/SoA, Unreal UHT, Unity C-ABI e TS SDK.
- [x] **Fase 2 (`v1.1.0-rts`):** Autonomia da CLI (`rtsc init`, `rtsc run`, `-o`, `--out-dir`, `--bin`), remoção de UTF-8 BOM, `RTS_UNUSED`, `io::*` e entrypoint nativo `pub fn main()`.
- [x] **Fase 3 (`v1.2.0-rts`):** Projetos multi-arquivo (`import` recursivo com detecção de ciclos e `RTS-E0102`), forward declarations C99, `Option<T>`, arrays `[T; N]` e `break`/`continue`.
- [x] **Fase 4 (`v1.3.0-rts`):** FFI `extern "C" fn` blindado por `cap` com remoção zero-overhead, blocos `impl Struct`, biblioteca matemática `core.math` e flags `-l`, `-L`, `-I`.
- [x] **Fase 5 (`v1.4.0-rts`):** Diretivas `#line` + flag `-g`/`--debug`, exportação de header C/C++ (`--emit-header`), extensão VS Code e showcase multi-arquivo `examples/kanori_zone_sim/` (`cargo test` **22/22 PASS**).
- [ ] **Próxima Fronteira (`v2.0+`):** Backend direto MLIR/LLVM, SPIR-V para shaders GPU e sandbox WASI com *Fuel Metering*.

---

## Licença

Distribuído sob a licença MIT / Apache 2.0. Consulte o arquivo `LICENSE` para mais detalhes.