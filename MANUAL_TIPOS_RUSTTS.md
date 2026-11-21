<p align="center">
  <img src="./rustts_language_logo.png" alt="RustTS Logo" width="220">
</p>

# Manual Oficial de Tipos e Referência da Linguagem — RustTS (`rtsc v2.3.0-rts`)

Este manual documenta todas as categorias de tipos, aliases ergonômicos (TypeScript / C / C# / Unreal Engine 5), estruturas de memória, tokens de capacidade (`cap`), atributos declarativos e intrínsecos da linguagem **RustTS (`.rts`)** na versão **`v2.3.0-rts`**.

---

## 1. Tipos Primitivos Escalares e Aliases Ergonômicos

A RustTS opera com tipos escalares de largura fixa e sem coerção implícita oculta. Você pode declarar tipos usando a nomenclatura explícita de bits (estilo Rust) ou a família completa de aliases ergonômicos (estilo TypeScript, C, C# e Unreal Engine 5), que compilam para os mesmos nós canônicos na AST com **custo zero**.

### 1.1. Tabela Completa de Inteiros, Ponto Flutuante, Ponto Fixo, Booleano e Texto

| Tipo Canônico | Aliases TS / C / C# / UE5 | C99 Nativo | Unreal C++ | TypeScript SDK |
| :--- | :--- | :--- | :--- | :--- |
| `i8` | `int8`, `sbyte` | `int8_t` | `int32` | `number` |
| `i16` | `int16`, `short` | `int16_t` | `int32` | `number` |
| `i32` | `int`, `int32` | `int32_t` | `int32` | `number` |
| `i64` | `int64`, `long` | `int64_t` | `int64` | `string` |
| `u8` | `byte`, `uint8` | `uint8_t` | `uint8` | `number` |
| `u16` | `uint16`, `ushort` | `uint16_t` | `uint16` | `number` |
| `u32` | `uint`, `uint32` | `uint32_t` | `uint32` | `number` |
| `u64` | `uint64`, `ulong` | `uint64_t` | `uint64` | `string` |
| `usize` | `size_t`, `uintptr` | `size_t` | `int32` | `string` |
| `f32` | `number`, `float`, `float32` | `float` | `float` | `number` |
| `f64` | `double`, `float64` | `double` | `double` | `number` |
| `fix32` | `fixed32` | `int32_t` | `int32` | `number` |
| `bool` | `boolean` | `bool` | `bool` | `boolean` |
| `str` | `string` | `const char*` | `FString` | `string` |

* **Proteção de 64 bits no TypeScript SDK:** Os tipos `i64`, `u64` e `usize` são exportados como `string` no cliente TypeScript (`--emit-ts-sdk`) para impedir perda de precisão acima de $2^{53} - 1$ no padrão IEEE-754 do JavaScript.
* **Ponto Fixo Determinístico (`fix32` / `fixed32`):** Aritmética determinística de 32 bits representada como `int32_t` no C99 para simulações de rede (*lockstep*) que exigem reprodutibilidade bit a bit entre CPUs distintas.
* **Inferência Automática em Variáveis Locais (`let`):** Ao omitir `: Tipo` em declarações `let`, literais inteiros positivos são inferidos como `u32` (`uint32_t`), inteiros negativos como `i32` (`int32_t`), decimais como `f32` (`float`), booleanos como `bool` e literais de texto como `str` (`const char*`).

```rust
module manual.escalares_completos;
import core.io;

pub fn demonstrar_todos_os_aliases(
    pino: uint8,
    leitura_adc: uint16,
    cliques: uint32,
    steam_id: uint64
) -> int32 {
    // 1. Inteiros com sinal (Aliases C / Unreal / C#)
    let delta_curto: int8 = -10;
    let offset_mapa: int16 = -512;
    let saldo_conta: int = -42;
    let timestamp_epoch: int64 = 1700000000;

    // 2. Índices de memória, ponto flutuante e ponto fixo
    let indice_buffer: size_t = 0;
    let velocidade_som: number = 343.2;
    let precisao_orbital: double = 0.000000125;
    let fisica_rede: fixed32 = 1024;

    // 3. Booleano, texto e inferência automática no let
    let sensor_online: boolean = true;
    let mensagem: string = "Telemetria Validada: ";
    let soma_inferida = (leitura_adc as float32) + velocidade_som + (precisao_orbital as float32);

    if sensor_online && pino > 0 && indice_buffer == 0 && steam_id > 0 {
        io::print_f32(mensagem, soma_inferida);
        return saldo_conta + (delta_curto as int32) + (offset_mapa as int32) + (cliques as int32) + (timestamp_epoch as int32) + fisica_rede;
    }
    return 0;
}
```

---

## 2. Tipos Matemáticos Nativos SIMD-Ready

Os tipos vetoriais e matriciais fazem parte do núcleo léxico da linguagem, sem necessidade de importar pacotes externos.

| Tipo RustTS | Dimensão | C99 Nativo | Unreal Engine 5 | Unity C# |
| --- | --- | --- | --- | --- |
| `vec2` | 2 floats | `rts_vec2_t` | `FVector2D` | `UnityEngine.Vector2` |
| `vec3` | 3 floats | `rts_vec3_t` | `FVector` | `UnityEngine.Vector3` |
| `vec4` | 4 floats | `rts_vec4_t` | `FVector4` | `UnityEngine.Vector4` |
| `quat` | 4 floats | `rts_quat_t` | `FQuat` | `UnityEngine.Quaternion` |
| `mat3` | 9 floats | `float[9]` | `FVector` | `IntPtr` |
| `mat4` | 16 floats | `float[16]` | `FMatrix` | `IntPtr` |

* **Scalar Flattening SoA Automático:** Quando `vec2`, `vec3`, `vec4` ou `quat` são declarados dentro de uma `@component struct`, o compilador desdobra automaticamente cada componente (`.x`, `.y`, `.z`, `.w`) em ponteiros escalares independentes (`float* RTS_RESTRICT position_x`, `position_y`, `position_z`) alinhados em 64 bytes para vetorização AVX2/FMA3 sem desperdício de linha de cache L1.

```rust
module manual.matematica_simd;

@component
struct Transform {
    position: vec3,
    rotation: quat,
}

@component
struct Velocity {
    linear: vec3,
}

@system(stage = "FixedUpdate")
pub fn integrar_cinematica(
    transforms: cap MutColumn<Transform>,
    velocities: cap ReadColumn<Velocity>,
    time: cap TimeStep
) {
    let dt = time.fixed_delta;
    let total = transforms.len();

    for i in 0..total {
        let vel = velocities[i].linear;
        transforms[i].position = transforms[i].position + (vel * dt);

        if transforms[i].position.y < 0.0 {
            transforms[i].position.y = 0.0;
        }
    }
}
```

---

## 3. Coleções Estáticas de Tamanho Fixo (`[T; N]`)

Para garantir determinismo de tempo real (Zero Heap Allocation), a linguagem oferece arrays estáticos contíguos `[T; N]` alocados diretamente na stack ou dentro de structs.

```rust
module manual.arrays_fixos;

struct FiltroFIR {
    coeficientes: [number; 4],
}

pub fn processar_amostras() -> number {
    let filtro = FiltroFIR {
        coeficientes: [0.25, 0.25, 0.25, 0.25],
    };
    let leituras = [12.0, 18.5, 95.0, 40.0];
    let mut soma = 0.0;
    let mut i: usize = 0;

    while i < 4 {
        let valor = leituras[i] * filtro.coeficientes[i];
        i = i + 1;
        if valor < 5.0 {
            continue;
        }
        soma = soma + valor;
        if soma > 100.0 {
            break;
        }
    }
    return soma;
}
```

---

## 4. Tipos Algébricos Seguros (`Option<T>`, `Result<T, E>`, `Handle<T>`, `Json<T>`)

A RustTS substitui ponteiros nulos (`null`/`nullptr`), exceções (`try`/`catch`/`throw`) e anotações complexas de tempo de vida (`'a`) por quatro tipos genéricos verificados estaticamente:

| Tipo Genérico | Construtores e Operadores | Representação C99 Nativa | Objetivo Arquitetural |
| --- | --- | --- | --- |
| `Option<T>` | `Some(v)`, `None`, `match` | `rts_option_<T>_t` | Substitui `null` e `undefined` sem ponteiros |
| `Result<T, E>` | `Ok(v)`, `Err(e)`, `?`, `.map_err`, `match` | `rts_result_<T>_<E>_t` | Substitui `throw` com propagação explícita |
| `Handle<T>` | `Handle::null()`, `.index`, `.generation` | `rts_handle_t` (64 bits) | Substitui ponteiros brutos e lifetimes `'a` |
| `Json<T>` | Retorno de funções `@route` | `<T>_t` | Serialização tipada para TypeScript SDK |

```rust
module manual.algebricos;
import core.hardware;

enum ErroSensor {
    Offline(byte),
    Critico(number),
}

pub fn buscar_offset(calibrado: boolean) -> Option<number> {
    if calibrado {
        return Some(1.25);
    }
    return None;
}

pub fn ler_sensor(i2c: cap I2CBus<0x48>, calibrado: boolean) -> Result<number, ErroSensor> {
    let bruto = i2c.read_register_u16(0x00)
        .map_err(|_| ErroSensor::Offline(0x48))?;

    let base = (bruto as number) * 0.0625;
    let opt = buscar_offset(calibrado);
    match opt {
        Some(offset) => {
            return Ok(base + offset);
        }
        None => {
            return Ok(base);
        }
    }
}
```

---

## 5. Tipos Nominais (`struct`, `enum`, `impl`) e Ponte FFI (`extern "C" fn`)

```rust
module manual.nominais_e_ffi;
import core.math;

extern "C" fn puts(msg: string, console: cap StdIO) -> int;

enum ModoOperacao {
    Eco,
    Turbo(number),
}

struct Motor {
    rpm_base: number,
}

impl Motor {
    pub fn calcular_rpm(self: Motor, modo: ModoOperacao) -> number {
        match modo {
            ModoOperacao::Eco => {
                return self.rpm_base * 0.8;
            }
            ModoOperacao::Turbo(fator) => {
                return math::clamp(self.rpm_base * fator, 1000.0, 9000.0);
            }
        }
    }
}
```

---

## 6. Tokens de Capacidades Lineares (`cap` — Zero Ambient Authority)

Nenhuma função pode acessar hardware, rede, banco de dados, relógio ou arenas sem receber o respectivo token `cap` na assinatura (`RTS-E0402` / `RTS-E0403`). Tokens `cap` só podem ser instanciados no entrypoint raiz `pub fn main()` (`RTS-E0404`).

### 6.1. Hardware, Barramentos Físicos e Relógios

* `cap I2CBus<Addr>`, `cap I2CRead<Addr>`, `cap I2CPort<Addr>` (`.read_register_u16`, `.read_register_u8`, `.write_register_u16`, `.write_register_u8`)
* `cap PWMPin<Chan>`, `cap PWMWrite<Chan>` (`.set_duty`, `.set_frequency`)
* `cap GPIOPin<Pin>`, `cap SPIBus`, `cap DMAChannel`, `cap Interrupt`
* `cap MonotonicClock` (`SystemClock::now()`, `.ticks_ns`), `cap TimeStep` (`.fixed_delta`)

### 6.2. Arenas Escopadas $O(1)$ e Hierarquia Anti-Escape (`RTS-E0501`)

* **Rank 3 (Permanente):** `cap StaticArena`
* **Rank 2 (Fase / Sessão / Coluna ECS):** `cap LevelArena`, `cap SessionArena`, `cap MutColumn<T>`, `cap ReadColumn<T>`, `cap Mut<T>`, `cap Read<T>`
* **Rank 1 (Efêmero — Reset $O(1)$ a cada quadro/request):** `cap FrameArena<T>`, `cap RequestArena`, `cap TaskArena`, `cap ReadFrameArena<T>`, `cap ReadRequestArena<T>`, `cap ReadTaskArena<T>`

### 6.3. Rede, Banco de Dados, GPU e Sistema

* `cap NetSocket`, `cap TcpSocket`, `cap UdpTick`, `cap HttpRoute`
* `cap SqlRead<Tabela>`, `cap SqlWrite<Tabela>`
* `cap GpuQueue`, `cap WebGpuDevice`, `cap CanvasContext`, `cap AudioBuffer`, `cap StdIO`

---

## 7. Atributos Declarativos e Intrínsecos (`core.math` / `core.io`)

* **Atributos Fixos:** `@component`, `@system(stage = "...")`, `@parallel_system(stage = "...")`, `@sealed_sig`, `@open_sig`, `@repr_c`, `@unreal_actor(replicate = bool, category = "...")`, `@unreal_impl(Struct)`, `@uproperty(...)`, `@route("METHOD", "/path")`, `@gpu_kernel`, `@vertex_shader`, `@fragment_shader`, `@shader_shared`.
* **Matemática (`core.math`):** `math::clamp`, `math::lerp`, `math::min`, `math::max`, `math::abs`, `math::sqrt`, `math::sin`, `math::cos`, `math::atan2`, `math::floor`, `math::ceil`.
* **Saída Determinística (`core.io`):** `io::println(str)`, `io::print_f32(str, f32)`, `io::print_i32(str, i32)`, `io::print_u32(str, u32)`.
* **Domínios Web Tradicionais Banidos (`RTS-E0900`):** `DOM`, `DOMNode`, `Document`, `HtmlElement`, `CssStyle`, `VirtualDom`, `ReactComponent`, `WindowDom`.
