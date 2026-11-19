# RustTS (`.rts`) — Matriz de Regras, Especificação Arquitetural e Guia Mestre (`v2.2.0-rts`)

## 1. Visão Geral e Filosofia Central

A **RustTS** é uma linguagem de programação de sistemas, jogos e controle de hardware projetada sob um paradigma inédito: **Simbiose Cognitiva entre Inteligência Artificial (LLMs) e Desenvolvedores Humanos**.

Enquanto linguagens tradicionais foram desenhadas exclusivamente para a ergonomia de digitação humana (criando dezenas de atalhos sintáticos, ambiguidades contextuais e abstrações implícitas), a RustTS é desenhada para **maximizar a precisão estatística de modelos de linguagem (Gemini, Claude, GPT, DeepSeek, Qwen)** e **garantir segurança mecânica absoluta no acesso a hardware e motores de tempo real**.

### Os 4 Pilares Inegociáveis
1. **Sintaxe Zero-Novelty (Fusão Canônica Rust + TypeScript):** Aproveita os trilhões de tokens já presentes no pré-treinamento dos LLMs atuais, eliminando a necessidade de ensinar uma estética nova ao modelo.
2. **Isolamento Contextual Total (Context-Local Reasoning):** Toda função ou sistema declara explicitamente na sua assinatura exatamente quais dados lê, quais dados modifica e quais recursos de hardware/SO utiliza. Nem o humano nem a IA precisam ler outros arquivos para entender os efeitos colaterais de uma função.
3. **Performance de Tempo Real Previsível ($O(1)$ Memory Management):** Focada em motores de jogos (60–240 FPS) e sistemas embarcados/bare-metal, substituindo tanto o *Garbage Collector* quanto a complexidade de *lifetimes* aninhados por **Arenas de Memória Escopadas** e **Índices Geracionais**.
4. **Feedback Nativo para Máquinas e Humanos:** O compilador (`rtsc`) exporta sua gramática em formato **GBNF** (para *Constrained Decoding* em LLMs) e emite diagnósticos de erro em **JSON AST estruturado** para auto-correção instantânea em menos de $15\text{ ms}$.

---

## 2. O Que a RustTS DEVE Ter (Requisitos Obrigatórios e Justificativas)

### 2.1. Regra da Forma Canônica Única (*One Canonical Way*)
* **O que é:** Para cada operação lógica (declarar estrutura, iterar coleção, tratar erro, exportar função), existe **exatamente uma** construção sintática válida na gramática.
* **Por que deve ter:** Em linguagens como C++ ou JavaScript, existem 5 a 8 maneiras de escrever o mesmo loop ou declarar um tipo. Isso divide a distribuição de probabilidade dos tokens nos LLMs, aumentando a entropia e a chance de alucinação. Uma forma canônica única concentra a atenção do modelo em $100\%$ na lógica do problema.

### 2.2. Segurança Baseada em Capacidades (`cap`) no Sistema de Tipos
* **O que é:** Nenhuma função possui "autoridade ambiental". Para ler um sensor I2C, escrever em um pino GPIO, abrir um socket de rede, alocar memória ou modificar uma coluna de entidades em um jogo, a função **deve** receber um token de capacidade `cap` nos parâmetros.
* **Por que deve ter:**
  1. **Segurança de Hardware:** Impede matematicamente que um código gerado por IA acesse endereços de memória, arquivos ou barramentos fora do escopo autorizado.
  2. **Multithreading Automático em Jogos:** Como o compilador sabe quais funções têm `cap Read<T>` e `cap Mut<T>`, ele agenda a execução paralela em múltiplos núcleos da CPU com garantia matemática de zero *Data Races*.

### 2.3. Gerenciamento de Memória por Arenas Escopadas e Handles Geracionais
* **O que é:** Toda alocação dinâmica ocorre dentro de uma Arena explícita passada via capacidade (`StaticArena`, `LevelArena`, `FrameArena`, `TaskArena`). Referências duradouras entre objetos de jogo usam `Handle<T>` (índice de 32 bits + contador de geração de 32 bits) em vez de ponteiros brutos.
* **Por que deve ter:**
  * Elimina pausas de *Garbage Collector* (fatal para jogos na Unity/Unreal).
  * Elimina o problema de *borrow checker fighting* com *lifetimes* complexos (`<'a, 'b>`) do Rust tradicional, onde LLMs entram em loops infinitos de erro ao tentar criar grafos de cena ou listas duplamente encadeadas.
  * `FrameArena` permite alocar memória livremente durante o cálculo de um frame e limpar tudo no final do frame em tempo constante $O(1)$ apenas zerando o ponteiro de topo.

### 2.4. Data-Oriented Design (DOD) e ECS como Cidadãos de Primeira Classe
* **O que é:** Suporte nativo na linguagem para `component` (dados puros alinhados em *Structure of Arrays* - SoA) e `system` (funções que operam em lote sobre colunas contíguas de componentes).
* **Por que deve ter:** Garante localidade máxima de Cache L1/L2 da CPU e permite que o compilador aplique vetorização **SIMD (AVX2, AVX-512, ARM NEON)** automaticamente sem que a IA ou o Dev precisem escrever intrínsecos de processador.

### 2.5. Unificação CPU-GPU (`@gpu_kernel` e `@shader_ shared`)
* **O que é:** Estruturas de dados e funções matemáticas marcadas com `@gpu_kernel` são compiladas diretamente pelo backend para **SPIR-V (Vulkan)**, **DXIL (DirectX 12)** ou **WGSL**, compartilhando exatamente a mesma definição de `struct` do código da CPU.
* **Por que deve ter:** Elimina um dos erros mais comuns entre IAs e programadores gráficos: desalinhamento de bytes (`std140`/`std430`) e dessincronização entre structs em C++/Rust e shaders em HLSL/GLSL.

### 2.6. Arquitetura Multi-Target (Transpilação e Compilação Nativa)
O compilador `rtsc` deve suportar 4 alvos de saída (`--target`):
1. **`native` (C99 / Rust / LLVM):** Gera código nativo de altíssima performance com C ABI limpa para engines próprias do zero, sistemas operacionais e microcontroladores.
2. **`unreal` (C++20 + UHT/GAS Generator):** Lê um único arquivo `.rts` e emite automaticamente o par `.h` e `.cpp` com todas as macros `UCLASS()`, `UPROPERTY()`, `UFUNCTION()`, métodos de replicação de rede (`GetLifetimeReplicatedProps`) e boilerplate do *Gameplay Ability System (GAS)*.
3. **`unity` (C-ABI Native Plugin + C# Burst Wrapper):** Compila o núcleo pesado para biblioteca nativa (`.dll`/`.so`) e gera o wrapper C# com `NativeArray` zero-copy.
4. **`wasm-sandbox` (WASI Component Model + Fuel Metering):** Compila para WebAssembly isolado com limite determinístico de ciclos de CPU (*fuel*) para mods, hot-reload em tempo real e testes seguros de código gerado por IA.

### 2.7. Interface Nativa para LLMs (`--export-gbnf` e `--json-diagnostics`)
* **O que é:**
  * `rtsc --export-gbnf`: Gera a gramática formal completa para motores de inferência restringirem os logits dos tokens da IA.
  * `rtsc check --json-diagnostics`: Retorna erros semânticos em um schema JSON compacto contendo o ID do nó da AST, a regra violada e o patch exato sugerido.
* **Por que deve ter:** Reduz a taxa de erro sintático da IA para $0\%$ e reduz o consumo de tokens no loop de auto-correção em mais de $80\%$.

---

## 3. O Que a RustTS NÃO DEVE Ter (Proibições Estritas e Justificativas)

| Recurso Proibido | Por que é Estritamente Proibido na RustTS | Alternativa na RustTS |
| :--- | :--- | :--- |
| **Herança de Classes (`extends`, OOP Clássica) e Polimorfismo por Vtable** | Fragmenta a memória no heap (causando *cache misses* severos em jogos) e obriga a IA a carregar dezenas de arquivos de classes-pai na janela de contexto para entender um método. | Composição estrita via `struct`, `enum` (Tagged Unions) e `trait` (Interfaces estáticas resolvidas em compilação). |
| **Garbage Collector (Rastreamento em Runtime)** | Causa picos imprevisíveis de latência (*frame drops*) em jogos e impede o uso da linguagem em drivers, kernels e microcontroladores. | Arenas Escopadas (`FrameArena`, `LevelArena`) + Contagem de escopo determinística. |
| **Anotações de *Lifetimes* Explícitos (`'a`, `'b`)** | É a principal barreira cognitiva do Rust tradicional para humanos e a maior causa de falhas de compilação em código gerado por LLMs. | Referências restritas a parâmetros de escopo local + `Handle<T>` para referências persistentes. |
| **Macros Procedurais Arbitrárias e Templates C++ (SFINAE)** | Ocultam o fluxo real de execução e geram erros de compilação indecifráveis que quebram a capacidade de raciocínio local da IA. | Atributos declarativos fixos do compilador (`@derive`, `@unreal_actor`, `@gpu_kernel`) e Generics com restrições explícitas de Trait. |
| **`null`, `undefined`, `NaN` Silencioso e Exceções (`try/catch/throw`)** | Criam fluxos de controle invisíveis na assinatura da função e falhas catastróficas em tempo de execução. | Tipos algébricos `Option<T>` e `Result<T, E>` com propagação explícita via operador `?`. |
| **Variáveis Globais Mutáveis e Singletons Implícitos** | Violam o isolamento de capacidades, quebram o determinismo de replays em jogos e causam *race conditions* em multithreading. | Estado injetado explicitamente via parâmetros `cap`. |
| **Coerção Implícita de Tipos e Sobrecarga de Operadores** | Gera ambiguidade sobre o custo computacional de cada linha (ex: um sinal `+` escondendo uma alocação de memória pesada). | Conversões numéricas explícitas (`as f32`) e operadores restritos a tipos primitivos e vetores matemáticos nativos (`vec2`, `vec3`, `vec4`, `mat4`). |
| **Separadores de Cabeçalho e Implementação (`.h` / `.cpp`)** | Duplica informação, gasta o dobro de tokens da IA e gera erros constantes de dessincronização de assinaturas. | Módulo único `.rts` (o compilador gera `.h`/`.cpp` apenas como artefato invisível se o alvo for a Unreal Engine). |

---

## 4. Especificação da Sintaxe Canônica (`.rts`)

A sintaxe combina a legibilidade imediata das anotações de tipo do **TypeScript** com a semântica de valores, `struct`, `enum` e `match` do **Rust**.

### 4.1. Tipos Primitivos e Vetoriais Nativos (SIMD-Ready)
* **Inteiros:** `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64`, `usize`
* **Ponto Flutuante e Ponto Fixo (Determinístico para Física/Rede):** `f32`, `f64`, `fix32`
* **Lógico e Texto:** `bool`, `str` (fatia imutável UTF-8)
* **Matemática de Jogos/Hardware (Nativos na Gramática):** `vec2`, `vec3`, `vec4`, `quat`, `mat3`, `mat4`

### 4.2. Exemplo Canônico 1: Controle de Hardware e Sistemas Embarcados
```rust
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

// A assinatura declara TODAS as capacidades físicas necessárias.
// É impossível esta função acessar rede, disco ou outros pinos.
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
        let duty: u8 = math::clamp((delta * 5.0) as u8, 20, config.max_pwm_duty);
        pwm.set_duty(duty);
    } else {
        pwm.set_duty(0);
    }

    return Ok(current_temp);
}
```

### 4.3. Exemplo Canônico 2: Engine de Jogos do Zero (ECS + FrameArena + SIMD)
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

// O compilador agenda este sistema em paralelo e vetoriza o loop via SIMD
// porque ReadColumn e MutColumn garantem memória contígua sem aliasing.
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

        // Se bater no chão (y < 0.0), registra evento na FrameArena (alocação O(1) sem GC)
        if transforms[i].position.y < 0.0 {
            transforms[i].position.y = 0.0;
            events.push(CollisionEvent {
                entity_a: transforms.handle_at(i),
                entity_b: Handle::null(),
                impact_point: transforms[i].position,
            });
        }
    }
}
```

### 4.4. Exemplo Canônico 3: Integração com Unreal Engine 5 (`--target unreal`)
```rust
module game.combat;

// O compilador rtsc converte este bloco único em AWeaponProjectile.h e AWeaponProjectile.cpp
// gerando UCLASS(), UPROPERTY(), UFUNCTION() e GetLifetimeReplicatedProps automaticamente.
@unreal_actor(replicate = true, category = "Combat")
struct WeaponProjectile {
    @uproperty(edit_anywhere, blueprint_read_only)
    base_damage: f32,

    @uproperty(replicated)
    current_speed: f32,
}

@unreal_impl(WeaponProjectile)
pub fn compute_impact_damage(
    self: cap Read<WeaponProjectile>,
    armor_rating: f32
) -> f32 {
    let mitigation: f32 = math::clamp(1.0 - (armor_rating * 0.01), 0.1, 1.0);
    return self.base_damage * mitigation;
}
```

---

## 5. Formato do Protocolo de Diagnóstico JSON para IAs

Quando invocado com `rtsc check --json-diagnostics`, qualquer erro de compilação deve seguir estritamente este schema para realimentação imediata do agente de IA:

```json
{
  "status": "error",
  "compiler_version": "0.1.0-rts",
  "diagnostics": [
    {
      "error_code": "RTS-E0402",
      "category": "CapabilityViolation",
      "file": "src/drone/thermal_control.rts",
      "span": { "line_start": 28, "col_start": 5, "line_end": 28, "col_end": 24 },
      "ast_node": "CallExpr(NetworkSocket::send)",
      "message": "Function 'regulate_motor_cooling' attempted to invoke 'NetworkSocket::send' without declaring capability 'cap NetSocket' in its signature.",
      "deterministic_fixes": [
        {
          "action": "add_parameter_capability",
          "replacement_signature": "pub fn regulate_motor_cooling(config: SensorConfig, i2c: cap I2CBus<0x48>, pwm: cap PWMPin<4>, clock: cap MonotonicClock, net: cap NetSocket) -> Result<f32, HardwareFault>"
        },
        {
          "action": "remove_unauthorized_call",
          "line_to_remove": 28
        }
      ]
    }
  ]
}
```

---

## 6. Regras de Governança e Evolução da Linguagem

Para garantir que a RustTS nunca se degrade em uma linguagem inchada como C++ ao longo do tempo, toda nova funcionalidade proposta deve passar pelos **4 Filtros de Evolução**:

1. **Filtro da Entropia de Tokens (LLM Benchmark):**
   Uma mudança na sintaxe só é aprovada se testes empíricos com pelo menos 3 famílias de modelos (ex: Gemini, Claude, DeepSeek/Qwen) demonstrarem que a geração *zero-shot* tem taxa de acerto superior a $95\%$ e consome igual ou menor quantidade de tokens que a versão anterior.
2. **Filtro da Ortogonalidade:**
   Se uma nova funcionalidade puder ser expressa combinando `struct`, `enum`, `cap` e funções normais sem perda de performance de máquina, ela **não** deve ser adicionada à gramática da linguagem.
3. **Filtro do Orçamento de Frame ($0\text{ Overhead}$):**
   Nenhum recurso novo pode introduzir alocações ocultas no heap, verificações dinâmicas não solicitadas no *Hot Path* ou perda de alinhamento SIMD.
4. **Filtro da Transpilação Limpa:**
   Qualquer construção na RustTS deve ter um mapeamento direto, legível e determinístico para C99, Rust e C++20 (Unreal), garantindo que a linguagem nunca fique presa em um único ecossistema de hardware.

---

## 7. Roteiro Técnico de Implementação do Compilador (`rtsc`)

* **Etapa 1 (Fundação e Parser):** Escrever o Lexer (`logos`) e o Parser (`chumsky` / Recursive Descent) em Rust, gerando a AST tipada e o exportador `--export-gbnf`.
* **Etapa 2 (Análise Semântica e `cap` Checker):** Implementar a tabela de símbolos, verificador de tipos, verificador de escopo de Arenas e validador estático de capacidades com saída `--json-diagnostics`.
* **Etapa 3 (Transpilador Bootstrap):** Implementar os geradores de código (`codegen`) emitindo **Rust nativo** e **C99** limpos, permitindo compilar binários reais desde a primeira semana usando `rustc` ou `clang` como backend.
* **Etapa 4 (Pontes para Motores de Jogos):** Implementar os emissores especializados `--target unreal` (gerador `.h`/`.cpp` UHT) e `--target unity` (C ABI + C# wrapper).
* **Etapa 5 (Runtime Sandbox, Shaders Gráficos e Engine Vulkan 2D/3D):** Integrar WASI (`--target wasm`), compilação de `@gpu_kernel`, `@vertex_shader` e `@fragment_shader` para SPIR-V 1.5 / WGSL (`PushConstant Mat4`, `Sampler2D`), e runtime gráfico Vulkan 2D/3D (`Renderer3D`, `SpriteBatch2D`, `@repr_c`, `@sealed_sig`).