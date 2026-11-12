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

## Instalação Rápida (`v1.0.0-rts`)

**Windows (PowerShell):**
```powershell
irm https://raw.githubusercontent.com/LuizCorrea-Dev/RustTS/main/scripts/install.ps1 | iex
```

**Linux / macOS:**
```bash
curl -fsSL https://raw.githubusercontent.com/LuizCorrea-Dev/RustTS/main/scripts/install.sh | bash
```

Consulte o **[Guia Completo de Instalação e Uso (`INSTALACAO_E_USO.md`)](./INSTALACAO_E_USO.md)** para download manual em [Releases](https://github.com/LuizCorrea-Dev/RustTS/releases/tag/v1.0.0), compilação a partir do código-fonte, exemplos de benchmark e tutorial passo a passo.

---

## Interface de Linha de Comando (`rtsc`)

O compilador `rtsc` centraliza a validação, compilação cruzada e integração com motores e agentes de IA:

```bash
# 1. Validar código com saída humana ou JSON estruturado (< 15ms)
rtsc check examples/thermal_control.rts
rtsc check examples/thermal_control.rts --json-diagnostics

# 2. Gerar código C99 nativo (SoA + Strip-Mining L1d + AVX2/FMA3)
rtsc build examples/ballistics_10k_benchmark.rts --target=native > ballistics_gen.c

# 3. Transpilar módulo de Gameplay/GAS para Unreal Engine 5 (.h e .cpp UHT)
rtsc build examples/combat_unreal.rts --target=unreal > combat_unreal_ue5.cpp

# 4. Gerar Plugin C-ABI + Wrapper C# Zero-Copy (NativeArray<T>) para Unity
rtsc build examples/engine_physics.rts --target=unity > engine_physics_unity.txt

# 5. Gerar SDK TypeScript tipado (Next.js/React) a partir de servidor .rts
rtsc build examples/telemetry_server.rts --emit-ts-sdk > telemetry_client_sdk.ts

# 6. Exportar a gramática formal GBNF para restringir geração de tokens em LLMs locais
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

* [`INSTALACAO_E_USO.md`](./INSTALACAO_E_USO.md) — **Guia Oficial de Instalação e Uso:** Instalação em 1 linha (`install.ps1` / `install.sh`), binários de release, manual da CLI `rtsc` e tutorial passo a passo.
* [`devlog.md`](./devlog.md) — **Devlog & Cronologia (`v0.1.0-rts` → `v1.0.0-rts`):** Histórico técnico completo de versões, auditorias, suíte `cargo test` (`13/13`) e benchmarks AVX2/SoA.
* [`RustTS_guia.md`](./RustTS_guia.md) — **Constituição e Matriz de Regras da Linguagem:** Especificação completa de sintaxe, memória, capacidades, proibições e governança de evolução.
* [`agenteRTS.md`](./agenteRTS.md) — **System Prompt do Agente Especialista:** Configuração de contexto para transformar LLMs em engenheiros de compiladores (`COMPILER-DEV`) e programadores nativos RustTS (`RTS-CODER`).
---

## Roadmap de Implementação (`rtsc`)

- [x] **Fase 0:** Especificação formal da arquitetura, regras de escopo, sistema de Arenas e capacidades (`cap`).
- [x] **Fase 1 (Front-End Core):** Lexer (`logos`), Parser recursivo, AST estrita e exportador de gramática GBNF (`rtsc --export-gbnf`).
- [x] **Fase 2 (Análise Semântica):** *TypeChecker* (`RTS-E0301`), `match` exaustivo (`RTS-E0302`), *Capability Checker* (`RTS-E0402`–`E0405`), validador de escopo de Arenas (`Static`, `Level`, `Frame`, `Request`) e `--json-diagnostics`.
- [x] **Fase 3 (Backends `v1.0.0-rts`):** Gerador C99 SIMD/SoA (`64B`, Strip-Mining L1d AVX2/FMA3), gerador Unreal Engine UHT (`.h`/`.cpp`), gerador Unity C-ABI + `NativeArray<T>` e gerador `--emit-ts-sdk`.
- [ ] **Fase 4 (Próxima Fronteira):** Integração direta MLIR/LLVM, SPIR-V para shaders GPU e sandbox WASI com *Fuel Metering*.

---

## Licença

Distribuído sob a licença MIT / Apache 2.0. Consulte o arquivo `LICENSE` para mais detalhes.