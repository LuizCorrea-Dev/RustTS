<p align="center">
  <img src="./rustts_language_logo.png" alt="RustTS Logo" width="220">
</p>

# Manual Oficial de Tipos, Capacidades e Memória — RustTS (`rtsc v1.5.0-rts`)

**Autor:** Luiz Correa (`Rootszera`) / KANORI  
**Versão do Compilador:** `rtsc 1.5.0-rts`  
**Alvos Suportados:** Nativo C99/SIMD (`--target=native`), Unreal Engine 5 (`--target=unreal`), Unity (`--target=unity`) e TypeScript SDK (`--emit-ts-sdk`)

Este manual documenta **100% dos tipos, aliases ergonômicos, estruturas de memória, tokens de capacidade (`cap`), atributos e intrínsecos** da linguagem **RustTS (`.rts`)**, organizados por categoria com exemplos práticos e tabelas de tradução multi-alvo.

---

## Sumário por Categoria

1. [Tipos Primitivos Escalares e Família Completa de Aliases (TS / C / C# / Unreal)](#1-tipos-primitivos-escalares-e-família-completa-de-aliases-ts--c--c--unreal)
2. [Inferência Automática de Tipos em Variáveis Locais (`let`)](#2-inferência-automática-de-tipos-em-variáveis-locais-let)
3. [Tipos Matemáticos Nativos SIMD-Ready (`vec2`..`mat4`)](#3-tipos-matemáticos-nativos-simd-ready-vec2mat4)
4. [Coleções Estáticas de Tamanho Fixo (`[T; N]`)](#4-coleções-estáticas-de-tamanho-fixo-t-n)
5. [Tipos Algébricos Seguros (`Option<T>`, `Result<T, E>`, `Handle<T>`, `Json<T>`)](#5-tipos-algébricos-seguros-optiont-resultt-e-handlet-jsont)
6. [Tipos Nominais (`struct`, `@component struct`, `enum`, `impl`)](#6-tipos-nominais-struct-component-struct-enum-impl)
7. [Ponte FFI C-ABI com Guarda de Capacidade (`extern "C" fn`)](#7-ponte-ffi-c-abi-com-guarda-de-capacidade-extern-c-fn)
8. [Tokens de Capacidades Lineares (`cap` — Zero Ambient Authority)](#8-tokens-de-capacidades-lineares-cap--zero-ambient-authority)
9. [Atributos Declarativos do Compilador (`@component`..`@route`)](#9-atributos-declarativos-do-compilador-componentroute)
10. [Biblioteca Padrão Intrínseca (`core.math` e `core.io`)](#10-biblioteca-padrão-intrínseca-coremath-e-coreio)
11. [Domínios Banidos por Design (`TierZeroForbiddenDomain` — `RTS-E0900`)](#11-domínios-banidos-por-design-tierzeroforbiddendomain--rts-e0900)

---

## 1. Tipos Primitivos Escalares e Família Completa de Aliases (TS / C / C# / Unreal)

A RustTS opera com tipos escalares de largura fixa em bits e proíbe coerções implícitas ocultas (`RTS-E0301`). Para acelerar a escrita sem perder determinismo binário, o analisador léxico (`src/lexer.rs`) reconhece tanto a **nomenclatura canônica em bits (estilo Rust)** quanto a **família completa de aliases (estilo TypeScript, C99, C# e Unreal Engine 5)**.

> **Garantia de Custo Zero:** Todos os aliases na tabela abaixo mapeiam diretamente no autômato léxico para o mesmo nó `TypeExpr::Primitive` da AST, gerando exatamente o mesmo código de máquina.

### 1.1. Tabela Completa de Inteiros com Sinal, Sem Sinal, Ponto Flutuante, Ponto Fixo, Booleano e Texto

| Tipo Canônico | Aliases TS / C / C# / UE5 | Bits | C99 Nativo | Unreal C++ | Unity C# | TypeScript SDK | Faixa de Valores / Uso Principal |
| :--- | :--- | :---: | :--- | :--- | :--- | :--- | :--- |
| **`i8`** | **`int8`**, **`sbyte`** | $8$ | `int8_t` | `int32` | `sbyte` | `number` | $-128 \dots 127$ (deltas curtos com sinal) |
| **`i16`** | **`int16`**, **`short`** | $16$ | `int16_t` | `int32` | `short` | `number` | $-32.768 \dots 32.767$ (áudio PCM, offsets) |
| **`i32`** | **`int`**, **`int32`** | $32$ | `int32_t` | `int32` | `int` | `number` | $-2{,}14 \times 10^9 \dots 2{,}14 \times 10^9$ (inteiro com sinal padrão) |
| **`i64`** | **`int64`**, **`long`** | $64$ | `int64_t` | `int64` | `long` | `string`* | Timestamps com sinal e deltas de alta escala |
| **`u8`** | **`byte`**, **`uint8`** | $8$ | `uint8_t` | `uint8` | `byte` | `number` | $0 \dots 255$ (registradores I2C, PWM duty, bytes) |
| **`u16`** | **`uint16`**, **`ushort`** | $16$ | `uint16_t` | `uint16` | `ushort` | `number` | $0 \dots 65.535$ (conversores ADC 12/16-bit, portas) |
| **`u32`** | **`uint`**, **`uint32`** | $32$ | `uint32_t` | `uint32` | `uint` | `number` | $0 \dots 4{,}29 \times 10^9$ (contadores, IDs, máscaras de bits) |
| **`u64`** | **`uint64`**, **`ulong`** | $64$ | `uint64_t` | `uint64` | `ulong` | `string`* | $0 \dots 1{,}84 \times 10^{19}$ (relógios em nanossegundos, SteamID) |
| **`usize`** | **`size_t`**, **`uintptr`** | $64$ | `size_t` | `int32` | `nuint` | `string`* | Índices de memória, `.len()`, iteradores `for i in 0..n` |
| **`f32`** | **`number`**, **`float`**, **`float32`** | $32$ | `float` | `float` | `float` | `number` | IEEE-754 32-bit (**8 lanes por registrador AVX2**) |
| **`f64`** | **`double`**, **`float64`** | $64$ | `double` | `double` | `double` | `number` | IEEE-754 64-bit (coordenadas globais / orbitais) |
| **`fix32`** | **`fixed32`** | $32$ | `int32_t` | `int32` | `int` | `number` | Ponto fixo determinístico Q16.16 (lockstep / rollback netcode) |
| **`bool`** | **`boolean`** | $8$ | `bool` | `bool` | `bool` | `boolean` | Valor lógico estrito (`true` ou `false`) |
| **`str`** | **`string`** | Ptr | `const char*` | `FString` | `IntPtr` | `string` | Literal de texto UTF-8 imutável |

*\*Nota sobre `i64`, `u64` e `usize` no TypeScript SDK:* O gerador `--emit-ts-sdk` converte inteiros de 64 bits para `string` decimal na interface TypeScript para impedir corrupção silenciosa de dados acima de $2^{53} - 1$ (`Number.MAX_SAFE_INTEGER` do IEEE-754 Double no JavaScript).

### Exemplo Prático — Todos os 14 Tipos Escalares e Aliases em Uso

```rust
module manual.escalares_completos;
import core.io;

pub fn demonstrar_todos_os_aliases(
    pino_gpio: uint8,
    leitura_adc: uint16,
    cliques_botao: uint32,
    id_sessao: uint64
) -> int32 {
    // 1. Inteiros com sinal usando aliases C / Unreal / C#
    let ajuste_fino: int8 = -10;
    let offset_setor: int16 = -512;
    let saldo_energia: int32 = -42;
    let tempo_acumulado: int64 = 1700000000;

    // 2. Equivalentes usando aliases estilo C# / TypeScript
    let byte_status: byte = 1;
    let porta_udp: ushort = 8080;
    let contador: uint = cliques_botao + 1;
    let hash_global: ulong = id_sessao;

    // 3. Tamanho de ponteiro, Ponto Flutuante e Ponto Fixo
    let indice_atual: size_t = 0;
    let velocidade_som: float32 = 343.2;
    let fator_escala: number = 1.25;
    let precisao_orbital: float64 = 0.000000125;
    let trava_deterministica: fixed32 = 1024;

    // 4. Booleano e Texto (Estilo TypeScript)
    let sistema_online: boolean = true;
    let prefixo_log: string = "[TELEMETRIA] Energia Calculada: ";

    if sistema_online && pino_gpio > 0 && byte_status == 1 && indice_atual == 0 && hash_global > 0 {
        let energia: number = ((leitura_adc as number) * fator_escala) + velocidade_som + (precisao_orbital as number);
        io::print_f32(prefixo_log, energia);

        return saldo_energia
            + (ajuste_fino as int32)
            + (offset_setor as int32)
            + (porta_udp as int32)
            + (contador as int32)
            + (tempo_acumulado as int32)
            + trava_deterministica;
    }
    return 0;
}
```

---

## 2. Inferência Automática de Tipos em Variáveis Locais (`let`)

Assim como no TypeScript, você **não precisa anotar o tipo (`: Tipo`) em variáveis locais** dentro das funções quando o tipo puder ser deduzido da expressão à direita do `=`.

### Regras de Inferência Canônica (`sema.rs` + `codegen_native.rs`)

| Expressão Inicializadora | Tipo Inferido na AST | Tipo C99 Emitido |
| :--- | :--- | :--- |
| `let a = 10;` ou `let a = 0x48;` | `u32` (`uint`) | `uint32_t` |
| `let b = -42;` | `i32` (`int`) | `int32_t` |
| `let c = 85.5;` | `f32` (`number`) | `float` |
| `let d = true;` ou `let d = !flag;` | `bool` (`boolean`) | `bool` |
| `let e = "KANORI";` | `str` (`string`) | `const char*` |
| `let f = [1.0, 2.0, 3.0];` | `[f32; 3]` | `float f[3]` |
| `let g = Config { max: 10.0 };` | `Config` | `Config_t` |
| `let h = math::sqrt(x);` | `f32` (`number`) | `float` |
| `let i = col.len();` | `usize` (`size_t`) | `size_t` |
| `let j = col.handle_at(0);` | `Handle<Entity>` | `rts_handle_t` |

```rust
module manual.inferencia_let;
import core.io;
import core.math;

struct Configuracao {
    limite: number,
}

pub fn calcular_rapido() -> int {
    let mut cliques = 0;               // Inferido: uint (uint32_t)
    let ajuste = -3;                   // Inferido: int (int32_t)
    let temperatura_atual = 72.5;      // Inferido: number (float)
    let ativo = true;                  // Inferido: boolean (bool)
    let aviso = "Temperatura final: "; // Inferido: string (const char*)
    let pesos = [0.5, 1.0, 1.5];       // Inferido: [f32; 3] (float[3])
    let cfg = Configuracao {           // Inferido: Configuracao (Configuracao_t)
        limite: 100.0,
    };

    if ativo && temperatura_atual < cfg.limite {
        cliques = cliques + 1;
        let corrigida = math::clamp(temperatura_atual * pesos[1], 0.0, cfg.limite);
        io::print_f32(aviso, corrigida);
        return (cliques as int) + ajuste;
    }
    return 0;
}
```

---

## 3. Tipos Matemáticos Nativos SIMD-Ready (`vec2`..`mat4`)

Os tipos vetoriais e matriciais são nativos da linguagem e desenhados para **desdobramento escalar automático (*Scalar Flattening SoA*)** em colunas de componentes ECS.

| Tipo RustTS | Componentes | C99 Local (Stack) | C99 em `@component` (SoA 64B) | Unreal Engine 5 | Unity C# |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **`vec2`** | `.x`, `.y` (`f32`) | `rts_vec2_t` | `float* <campo>_x`, `_y` | `FVector2D` | `UnityEngine.Vector2` |
| **`vec3`** | `.x`, `.y`, `.z` (`f32`) | `rts_vec3_t` | `float* <campo>_x`, `_y`, `_z` | `FVector` | `UnityEngine.Vector3` |
| **`vec4`** | `.x`, `.y`, `.z`, `.w` (`f32`) | `rts_vec4_t` | `float* <campo>_x`, `_y`, `_z`, `_w` | `FVector4` | `UnityEngine.Vector4` |
| **`quat`** | `.x`, `.y`, `.z`, `.w` (`f32`) | `rts_quat_t` | `float* <campo>_x`, `_y`, `_z`, `_w` | `FQuat` | `UnityEngine.Quaternion` |
| **`mat3`** | $3 \times 3$ (`f32`) | `float[9]` | `float[9]* <campo>` | `FVector` | `IntPtr` |
| **`mat4`** | $4 \times 4$ (`f32`) | `float[16]` | `float[16]* <campo>` | `FMatrix` | `IntPtr` |

### Exemplo Prático — Vetores SIMD e Desdobramento SoA

```rust
module manual.vetores_simd;

@component
struct Transformada {
    posicao: vec3,
    rotacao: quat,
    escala_uv: vec2,
    cor_emissiva: vec4,
}

@component
struct Movimento {
    velocidade: vec3,
}

@system(stage = "FixedUpdate")
pub fn atualizar_particulas(
    trans: cap MutColumn<Transformada>,
    mov: cap ReadColumn<Movimento>,
    tempo: cap TimeStep
) {
    let dt = tempo.fixed_delta;
    let total = trans.len();

    for i in 0..total {
        // Operação vetorial: o compilador desdobra automaticamente em 3 instruções FMA AVX2 (.x, .y, .z)
        let v: vec3 = mov[i].velocidade;
        trans[i].posicao = trans[i].posicao + (v * dt);

        // Acesso direto a um eixo escalar específico
        if trans[i].posicao.y < 0.0 {
            trans[i].posicao.y = 0.0;
        }
    }
}
```

---

## 4. Coleções Estáticas de Tamanho Fixo (`[T; N]`)

Arrays de tamanho fixo `[Tipo; N]` garantem alocação contígua na stack ou dentro de uma `struct`, sem fragmentação de memória e sem ponteiros indiretos.

- **Declaração de Tipo:** `[f32; 4]`, `[uint8; 16]`, `[int32; 8]`.
- **Inicialização Literal:** `[10.0, 20.0, 30.0, 40.0]`.
- **Indexação:** `meu_array[indice]` (onde `indice` é um inteiro).

```rust
module manual.colecoes_fixas;

struct CurvaBalistica {
    distancias: [number; 4],
    queda_gravidade: [number; 4],
}

pub fn calcular_queda_total() -> number {
    let curva = CurvaBalistica {
        distancias: [100.0, 200.0, 300.0, 400.0],
        queda_gravidade: [0.05, 0.22, 0.55, 1.10],
    };

    let multiplicadores = [1.0, 1.0, 1.2, 1.5];
    let mut soma_queda = 0.0;
    let mut idx: size_t = 0;

    while idx < 4 {
        let q = curva.queda_gravidade[idx] * multiplicadores[idx];
        idx = idx + 1;
        if q < 0.1 {
            continue;
        }
        soma_queda = soma_queda + q;
        if soma_queda > 2.0 {
            break;
        }
    }
    return soma_queda;
}
```

---

## 5. Tipos Algébricos Seguros (`Option<T>`, `Result<T, E>`, `Handle<T>`, `Json<T>`)

A RustTS substitui mecanismos inseguros de C/C++ e TypeScript por quatro tipos genéricos verificados estaticamente:

### 5.1. `Option<T>` — Substituto Seguro para `null` / `nullptr` / `undefined`
- **Construtores:** `Some(valor)` e `None`.
- **Tradução C99:** `typedef struct { bool is_some; <T> val; } rts_option_<T>_t;` (zero ponteiros nulos).
- **Regra Semântica (`RTS-E0302`):** Todo `match` sobre um `Option<T>` é obrigado a tratar os dois braços (`Some(v)` e `None`, ou `_`).

### 5.2. `Result<T, E>` — Substituto Seguro para `try / catch / throw`
- **Construtores e Operadores:**
  - `Ok(valor)` e `Err(erro)`.
  - Operador de propagação imediata `?` (retorna o erro automaticamente se a operação falhar).
  - Transformador de erro `.map_err(|_| MeuErro::Variante(...))?`.
- **Tradução C99:** `typedef struct { bool is_ok; union { <T> ok; <E> err; } val; } rts_result_<T>_<E>_t;`.

### 5.3. `Handle<T>` — Identificador Geracional de 64 Bits (Substitui Ponteiros e `'a`)
- **Estrutura C99 (`rts_handle_t`):** `.index` (`uint32_t`) + `.generation` (`uint32_t`).
- **Construtor Nulo Seguro:** `Handle::null()` (retorna `{ .index = 0xFFFFFFFFu, .generation = 0u }`).
- **Extração de Coluna SoA:** `coluna.handle_at(i)`.
- **Proteção Contra *Use-After-Free*:** Toda escrita indireta em coluna SoA usando um índice vindo de `handle.index` recebe automaticamente a guarda `if ((idx < col->count) && (col_handles[idx].generation == handle.generation))`.

### 5.4. `Json<T>` — Envelope de Contrato para APIs HTTP (`--emit-ts-sdk`)
- Usado no retorno de funções `@route` (ex: `Result<Json<PerfilDTO>, ErroAPI>`) para instruir o compilador a gerar o cliente HTTP tipado em TypeScript (`RtsResult<PerfilDTO, ErroAPI>`).

### Exemplo Prático — `Option<T>`, `Result<T, E>`, `?`, `Handle<T>` e `Json<T>`

```rust
module manual.tipos_algebricos;
import core.hardware;

struct TelemetriaDTO {
    entidade_id: uint32,
    temperatura_c: number,
}

enum FalhaHardware {
    I2CDesconectado(byte),
    LeituraForaDaFaixa(number),
}

pub fn calibrar_offset(ativo: boolean) -> Option<number> {
    if ativo {
        return Some(1.5);
    }
    return None;
}

@route("GET", "/api/v1/telemetria/:id")
pub fn ler_telemetria_segura(
    id: uint32,
    i2c: cap I2CBus<0x48>,
    req_arena: cap RequestArena
) -> Result<Json<TelemetriaDTO>, FalhaHardware> {
    // 1. Propagação de erro com '?' e mapeamento '.map_err'
    let bruto: uint16 = i2c.read_register_u16(0x00)
        .map_err(|_| FalhaHardware::I2CDesconectado(0x48))?;

    let mut temp = (bruto as number) * 0.0625;

    // 2. Desestruturação exaustiva de Option<number>
    let offset_opt: Option<number> = calibrar_offset(true);
    match offset_opt {
        Some(ajuste) => {
            temp = temp + ajuste;
        }
        None => {}
    }

    if temp > 120.0 {
        return Err(FalhaHardware::LeituraForaDaFaixa(temp));
    }

    // 3. Handle<T> geracional de 64 bits
    let h_nulo: Handle<TelemetriaDTO> = Handle::null();
    if h_nulo.generation == 0 {
        let payload = TelemetriaDTO {
            entidade_id: id,
            temperatura_c: temp,
        };
        return Ok(payload);
    }

    return Err(FalhaHardware::I2CDesconectado(0x48));
}
```

---

## 6. Tipos Nominais (`struct`, `@component struct`, `enum`, `impl`)

### 6.1. `struct` vs. `@component struct`
- **`struct Nome`:** Estrutura de dados padrão passada por valor (copiada na stack ou retornada em funções).
- **`@component struct Nome`:** Componente ECS. Além da struct normal `Nome_t`, o compilador gera o descritor de coluna `Nome_soa_column_t` alinhado em 64 bytes (`RTS_ALIGN64`) com ponteiros `__restrict__` separados para cada campo escalar.

### 6.2. `enum Nome` (Tagged Unions com Payload)
Permite variantes simples (tags puras) e variantes que carregam um dado tipado (`Variante(Tipo)`). Em C99, compila para uma `struct` contendo o discriminador `tag` e uma `union data`.

### 6.3. `impl Nome` (Métodos Nativos Associados)
Agrupa funções que operam sobre uma `struct`. Quando o primeiro parâmetro é `self: Nome`, o método pode ser chamado com a sintaxe de ponto `instancia.metodo(args...)`.

```rust
module manual.tipos_nominais;
import core.math;

enum ModoOperacao {
    Eco,
    Turbo(number),
    Emergencia(byte),
}

struct ControladorMotor {
    rpm_base: number,
    rpm_maximo: number,
}

impl ControladorMotor {
    pub fn calcular_rpm(self: ControladorMotor, modo: ModoOperacao) -> number {
        match modo {
            ModoOperacao::Eco => {
                return self.rpm_base * 0.75;
            }
            ModoOperacao::Turbo(multiplicador) => {
                return math::clamp(self.rpm_base * multiplicador, 0.0, self.rpm_maximo);
            }
            ModoOperacao::Emergencia(_) => {
                return 0.0;
            }
        }
    }
}

pub fn testar_motor() -> number {
    let ctrl = ControladorMotor {
        rpm_base: 3000.0,
        rpm_maximo: 8000.0,
    };
    let modo_atual = ModoOperacao::Turbo(2.2);
    return ctrl.calcular_rpm(modo_atual);
}
```

---

## 7. Ponte FFI C-ABI com Guarda de Capacidade (`extern "C" fn`)

Para invocar bibliotecas C do sistema operacional ou de motores gráficos (`libc`, `user32.dll`, `Raylib`, `SDL3`, `OpenGL`, `WinSock`) sem abrir brechas de segurança:

1. Declare a assinatura com `extern "C" fn nome_funcao_c(...);`.
2. Adicione um parâmetro `cap Dominio` na declaração para exigir que qualquer função `.rts` que chame essa API possua a capacidade correspondente (`RTS-E0403`).
3. **Zero Overhead na ABI C99:** O gerador `codegen_native.rs` remove automaticamente todos os parâmetros e argumentos `cap` ao emitir o protótipo `extern` e a chamada em C99, mantendo compatibilidade binária total com qualquer `.dll`, `.so` ou `.lib`.

```rust
module manual.ffi_c_abi;

// O parâmetro 'ui: cap GpuQueue' exige permissão estática no .rts,
// mas no C99 gera exatamente: extern int32_t MessageBoxA(size_t hwnd, const char* text, const char* caption, uint32_t utype);
extern "C" fn MessageBoxA(
    hwnd: size_t,
    text: string,
    caption: string,
    utype: uint32,
    ui: cap GpuQueue
) -> int32;

pub fn exibir_alerta(ui: cap GpuQueue) -> int32 {
    return MessageBoxA(0, "Sistema RustTS Operacional!", "KANORI FFI", 64, ui);
}
```

---

## 8. Tokens de Capacidades Lineares (`cap` — Zero Ambient Authority)

Todo efeito colateral externo na RustTS é governado pelo modificador `cap`.
- **Proibição de Forja (`RTS-E0404`):** Apenas `pub fn main()` pode instanciar capacidades iniciais com `Dominio::new(...)`. Qualquer outra função que tente criar uma variável local `cap` é bloqueada pelo compilador.
- **Contratos Selados por Padrão (`@sealed_sig`):** Quando uma função tenta usar um recurso sem a capacidade na assinatura, o compilador emite `RTS-E0402` / `RTS-E0403` e instrui o agente de IA a remover a chamada ilegal (nunca a escalar privilégios).

### 8.1. Catálogo Completo de Todos os Tokens `cap` da Linguagem

| Domínio / Token `cap` | Argumento Genérico | Rank de Arena | Categoria e Operações Autorizadas |
| :--- | :--- | :---: | :--- |
| **`cap I2CBus<0xAddr>`** | Endereço Hex/Int | — | Barramento I2C (`.read_register_u16`, `.read_register_u8`, `.write_register_*`) |
| **`cap I2CRead<0xAddr>`** | Endereço Hex/Int | — | Leitura dedicada de barramento I2C |
| **`cap I2CPort<0xAddr>`** | Endereço Hex/Int | — | Porta física I2C |
| **`cap PWMPin<Canal>`** | Canal Int (ex: `4`) | — | Controle de pino PWM (`.set_duty(byte)`, `.set_frequency(uint)`) |
| **`cap PWMWrite<Canal>`**| Canal Int | — | Escrita em canal PWM |
| **`cap GPIOPin<Pino>`** | Pino Int (ex: `1`) | — | Pino digital (`GPIOPin::set_high`) e guarda de periféricos |
| **`cap SPIBus`** | Opcional | — | Barramento serial SPI (`SPIBus::transfer`) |
| **`cap DMAChannel`** | Canal Int | — | Transferência direta de memória (DMA) |
| **`cap Interrupt`** | IRQ Int | — | Rotinas de tratamento de interrupção de hardware |
| **`cap MonotonicClock`** | Nenhum | — | Relógio monotônico (`SystemClock::now()`, `.ticks_ns`) |
| **`cap TimeStep`** | Nenhum | — | Delta de tempo fixo da simulação (`.fixed_delta`) |
| **`cap MutColumn<T>`** | `@component` `T` | **Rank 2** | Escrita e leitura exclusiva em coluna SoA (`col[i]`, `.len()`, `.handle_at(i)`) |
| **`cap ReadColumn<T>`** | `@component` `T` | **Rank 2** | Leitura imutável de coluna SoA (bloqueia escrita com `RTS-E0402`) |
| **`cap Mut<T>`** | `struct` / Actor `T` | **Rank 2** | Mutação do receptor `self` em métodos `@unreal_impl` |
| **`cap Read<T>`** | `struct` / Actor `T` | **Rank 2** | Acesso somente-leitura (`const` C++) em métodos `@unreal_impl` |
| **`cap StaticArena`** | Opcional | **Rank 3** | Arena global permanente durante toda a execução (`.store`, `.alloc_handle`) |
| **`cap LevelArena`** | Opcional | **Rank 2** | Arena persistente durante uma fase/cena (`.store`, `.insert`, `.save_handle`) |
| **`cap SessionArena`** | Opcional | **Rank 2** | Arena persistente durante a sessão de rede do jogador |
| **`cap FrameArena<T>`** | Evento/Struct `T` | **Rank 1** | Arena efêmera de quadro (16,6 ms), reset $O(1)$, produtora (`.push(ev)`) |
| **`cap RequestArena`** | Opcional | **Rank 1** | Arena efêmera por requisição HTTP `@route`, reset $O(1)$ |
| **`cap TaskArena`** | Opcional | **Rank 1** | Arena efêmera de tarefa assíncrona / job |
| **`cap ReadFrameArena<T>`**| Evento/Struct `T` | **Rank 1** | Consumidora imutável da `FrameArena<T>` no estágio seguinte (`ev[k]`, `.len()`) |
| **`cap ReadRequestArena<T>`**| Struct `T` | **Rank 1** | Consumidora imutável de `RequestArena` |
| **`cap ReadTaskArena<T>`** | Struct `T` | **Rank 1** | Consumidora imutável de `TaskArena` |
| **`cap NetSocket`** | Nenhum | — | Socket de rede (`NetworkSocket::send`, `.send`, `.recv`, `.broadcast`) |
| **`cap TcpSocket`** | Nenhum | — | Conexão TCP (`TcpSocket::connect`) |
| **`cap UdpTick`** | Nenhum | — | Pacotes de sincronia rápida UDP (`UdpSocket::send_to`) |
| **`cap HttpRoute`** | Nenhum | — | Requisições HTTP (`Http::get`) |
| **`cap SqlRead<Tabela>`** | Tipo da Tabela | — | Consultas SQL somente-leitura (`.query`, `.select`, `.find_by_id`, `.fetch_one`) |
| **`cap SqlWrite<Tabela>`**| Tipo da Tabela | — | Escrita SQL (`.insert`, `.update`, `.delete_by_id`, `.drop`) + leitura |
| **`cap GpuQueue`** | Nenhum | — | Submissão de comandos de GPU (`Gpu::submit`) e janelas nativas |
| **`cap WebGpuDevice`** | Nenhum | — | Pipeline de computação e gráficos WebGPU |
| **`cap CanvasContext`** | Nenhum | — | Contexto de pintura 2D |
| **`cap AudioBuffer`** | Nenhum | — | Buffer de síntese de áudio DSP |
| **`cap StdIO`** | Nenhum | — | Autoridade de saída padrão de console para chamadas FFI (`puts`, `printf`) |

### Regra de Ouro das Arenas (`RTS-E0501: ArenaLifetimeEscape`)
Um `Handle<T>` ou dado alocado em uma arena de **Rank menor (mais efêmera)** jamais pode ser salvo dentro de uma estrutura ou arena de **Rank maior (mais duradoura)**:
$$\text{Rank}(\text{StaticArena}) = 3 > \text{Rank}(\text{LevelArena} / \text{MutColumn}) = 2 > \text{Rank}(\text{FrameArena} / \text{RequestArena}) = 1$$

---

## 9. Atributos Declarativos do Compilador (`@component`..`@route`)

| Atributo | Sintaxe Exata | Efeito na Compilação |
| :--- | :--- | :--- |
| **`@component`** | `@component struct Nome { ... }` | Gera layout SoA de 64 bytes com *Scalar Flattening* para AVX2 |
| **`@system`** | `@system(stage = "FixedUpdate")` | Gera loop ECS Strip-Mined em tiles de 256 elementos (100% L1d Cache Hit) |
| **`@parallel_system`** | `@parallel_system(stage = "Update")` | Emite `_tile_batch` local na stack e commit atômico lock-free (`CAS`) |
| **`@sealed_sig`** | `@sealed_sig pub fn nome(...)` | Fronteira selada: bloqueia sugestões de escalação de privilégio para IAs |
| **`@open_sig`** | `@open_sig pub fn nome(...)` | Permite ao compilador sugerir a assinatura com a nova `cap` durante prototipação |
| **`@unreal_actor`** | `@unreal_actor(replicate = true, category = "Combat")` | Gera `.h` e `.cpp` de `AActor` compatíveis com o Unreal Header Tool (UHT) |
| **`@unreal_impl`** | `@unreal_impl(NomeStruct)` | Exporta função como método `UFUNCTION(BlueprintCallable)` na Unreal Engine 5 |
| **`@uproperty`** | `@uproperty(edit_anywhere, blueprint_read_only, replicated)` | Exporta campo da struct como `UPROPERTY(...)` com replicação de rede `DOREPLIFETIME` |
| **`@route`** | `@route("POST", "/api/v1/item/:id")` | Gera método assíncrono tipado na classe `RustTSClient` via `--emit-ts-sdk` |
| **`@gpu_kernel`** | `@gpu_kernel pub fn shader(...)` | Marca kernel de computação unificado CPU/GPU |
| **`@shader_shared`** | `@shader_shared struct Uniforms` | Garante alinhamento idêntico de bytes entre C99 e buffers de GPU |

---

## 10. Biblioteca Padrão Intrínseca (`core.math` e `core.io`)

### 10.1. Módulo `core.math` (`import core.math;`)

| Função | Assinatura | Descrição |
| :--- | :--- | :--- |
| `math::clamp(v, min, max)` | `(number, number, number) -> number` | Restringe `v` ao intervalo `[min, max]` |
| `math::lerp(a, b, t)` | `(number, number, number) -> number` | Interpolação linear $a + (b - a) \times t$ |
| `math::min(a, b)` | `(number, number) -> number` | Menor valor entre `a` e `b` |
| `math::max(a, b)` | `(number, number) -> number` | Maior valor entre `a` e `b` |
| `math::abs(x)` | `(number) -> number` | Valor absoluto (`fabsf`) |
| `math::sqrt(x)` | `(number) -> number` | Raiz quadrada (`sqrtf`) |
| `math::sin(rad)` | `(number) -> number` | Seno trigonométrico em radianos (`sinf`) |
| `math::cos(rad)` | `(number) -> number` | Cosseno trigonométrico em radianos (`cosf`) |
| `math::atan2(y, x)` | `(number, number) -> number` | Arco-tangente de dois argumentos (`atan2f`) |
| `math::floor(x)` | `(number) -> number` | Arredondamento para baixo (`floorf`) |
| `math::ceil(x)` | `(number) -> number` | Arredondamento para cima (`ceilf`) |

### 10.2. Módulo `core.io` (`import core.io;`)

| Função | Assinatura | Descrição |
| :--- | :--- | :--- |
| `io::println(msg)` | `(string) -> void` | Imprime linha de texto no console |
| `io::print_f32(rotulo, val)` | `(string, number) -> void` | Imprime rótulo seguido de `float` formatado (`%.2f`) |
| `io::print_i32(rotulo, val)` | `(string, int) -> void` | Imprime rótulo seguido de inteiro com sinal (`%d`) |
| `io::print_u32(rotulo, val)` | `(string, uint) -> void` | Imprime rótulo seguido de inteiro sem sinal (`%u`) |

---

## 11. Domínios Banidos por Design (`TierZeroForbiddenDomain` — `RTS-E0900`)

Para impedir acoplamento entre lógica de sistemas determinísticos e motores de layout de navegadores, os seguintes nomes de tipos, capacidades ou namespaces disparam erro fatal de compilação **`RTS-E0900`**:
- `DOM`, `DOMNode`, `Document`, `HtmlElement`, `CssStyle`, `VirtualDom`, `ReactComponent`, `WindowDom`.
- **Solução Canônica:** Programe a regra de negócio e persistência em `.rts` usando `@route` e exporte o SDK para o seu frontend Next.js / React usando `rtsc build servidor.rts --emit-ts-sdk -o client.ts`.