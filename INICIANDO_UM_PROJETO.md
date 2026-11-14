<p align="center">
  <img src="./rustts_language_logo.png" alt="RustTS Logo" width="220">
</p>

# Como Iniciar um Projeto com RustTS (`v2.0.0-rts`)

Na versão **`v2.0.0-rts`**, a RustTS une a **ergonomia e fluidez do TypeScript e C** com a **segurança determinística e performance AVX2 do Rust e C99**:
- **Inferência de Tipos Completa no `let`:** Você não precisa anotar `: f32` ou `: u32` em toda variável local (`let velocidade = 12.5;`, `let contador = 0;`, `let ativo = true;`, `let pos = vec3(1.0, 2.0, 3.0);`).
- **Dois Estilos de Tipos de Largura Fixa (Zero Ambiguidade):** Você pode usar os nomes amigáveis do TypeScript/C (`int`, `uint`, `byte`, `float`, `number`, `boolean`, `string`) **ou** os tipos explícitos de hardware (`i8..i64`, `u8..u64`, `f32`, `f64`, `bool`, `str`). Ambos compilam para exatamente os mesmos registradores C99 (`int32_t`, `uint32_t`, `uint8_t`, `float`, `bool`, `const char*`) com **zero overhead**.

---

## Tabela Rápida de Tipos: Todos os 14 Tipos Escalares com Aliases TS / C / C# / Unreal

Use o estilo que for mais confortável para a sua equipe (ou misture ambos livremente no mesmo arquivo — todos compilam com custo zero na AST via DFA do lexer):

| Tipo Canônico | Aliases TS / C / C# / UE5 | C99 Nativo (`codegen_native`) | Unreal C++ (`codegen_unreal`) | TypeScript SDK (`codegen_ts_sdk`) | Exemplo de Inferência ou Declaração (`let`) |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **`i8`** | `int8`, `sbyte` | `int8_t` | `int32` | `number` | `let delta_curto: int8 = -10;` |
| **`i16`** | `int16`, `short` | `int16_t` | `int32` | `number` | `let offset_mapa: int16 = -512;` |
| **`i32`** | `int`, `int32` | `int32_t` | `int32` | `number` | `let delta = -10;` |
| **`i64`** | `int64`, `long` | `int64_t` | `int64` | `string` | `let timestamp_epoch: int64 = 1700000000;` |
| **`u8`** | `byte`, `uint8` | `uint8_t` | `uint8` | `number` | `let reg: byte = 0x48;` |
| **`u16`** | `uint16`, `ushort` | `uint16_t` | `uint16` | `number` | `let adc: uint16 = 4095;` |
| **`u32`** | `uint`, `uint32` | `uint32_t` | `uint32` | `number` | `let id = 42;` |
| **`u64`** | `uint64`, `ulong` | `uint64_t` | `uint64` | `string` | `let steam_id: uint64 = 76561198000000000;` |
| **`usize`** | `size_t`, `uintptr` | `size_t` | `int32` | `string` | `let indice_buffer: size_t = 0;` |
| **`f32`** | `number`, `float`, `float32` | `float` | `float` | `number` | `let temp = 36.5;` |
| **`f64`** | `double`, `float64` | `double` | `double` | `number` | `let preciso: double = 3.1415926535;` |
| **`fix32`** | `fixed32` | `int32_t` | `int32` | `number` | `let fisica_rede: fixed32 = 1024;` |
| **`bool`** | `boolean` | `bool` | `bool` | `boolean` | `let online = true;` |
| **`str`** | `string` | `const char*` | `FString` | `string` | `let tag = "SENSOR-01";` |
| **Vetores SIMD** | `vec2`, `vec3`, `vec4`, `quat` | `rts_vec3_t` (ou lanes SoA) | `FVector` / `FQuat` | `readonly number[]` | `let pos = vec3(1.0, 2.0, 3.0);` |
| **Array Fixo** | `[float; 4]`, `[number; 4]` | `float nome[4]` | `float[4]` | `readonly number[]` | `let pesos = [1.0, 2.0, 3.0, 4.0];` |

### Exemplo Prático Completo com Todos os Aliases Escalares

```rust
module manual.escalares_completos;
import core.io;

pub fn demonstrar_todos_os_aliases(
    pino: uint8,
    leitura_adc: uint16,
    cliques: uint32,
    steam_id: uint64,
    io: cap StdIO
) -> int32 {
    // 1. Inteiros com sinal (Aliases C / Unreal / C#)
    let delta_curto: int8 = -10;
    let offset_mapa: int16 = -512;
    let saldo_conta: int32 = -42;
    let timestamp_epoch: int64 = 1700000000;

    // 2. Inteiros de ponteiro, ponto flutuante e ponto fixo
    let indice_buffer: size_t = 0;
    let velocidade_ar: float32 = 343.2;
    let precisao_orbital: float64 = 0.000000125;
    let fisica_rede: fixed32 = 1024;

    // 3. Booleano e Texto (Aliases TypeScript)
    let sensor_online: boolean = true;
    let mensagem: string = "Telemetria Validada: ";

    if sensor_online && pino > 0 && indice_buffer == 0 && steam_id > 0 {
        let total = (leitura_adc as float32) + velocidade_ar + (precisao_orbital as float32);
        io::print_f32(mensagem, total);
        return saldo_conta + (delta_curto as int32) + (offset_mapa as int32) + (cliques as int32) + (timestamp_epoch as int32) + fisica_rede;
    }
    return 0;
}
```

---

## Passo 1 — Verifique o Compilador e Instale a Extensão da IDE

1. Confirme no terminal que o compilador `rtsc` está atualizado:
   ```bash
   rtsc --version
   # Saída esperada: rtsc 2.0.0-rts
   ```
   *(Se precisar instalar ou atualizar, veja [INSTALACAO_E_USO.md](./INSTALACAO_E_USO.md)).*

2. **Ative o Realce de Sintaxe e Snippets no VS Code ou Antigravity IDE:**
   - **Pacote VSIX pronto para VS Code e Antigravity IDE:** [`editors/rustts-lang-2.0.0.vsix`](./editors/rustts-lang-2.0.0.vsix) — abra o editor e pressione **`Ctrl + Shift + P`** → **`Extensions: Install from VSIX...`** (ou **`Developer: Install Extension from Location...`** selecionando a pasta [`editors/vscode-rustts`](./editors/vscode-rustts)).
   - *(**Importante:** não dê duplo-clique no arquivo `.vsix` pelo Windows Explorer para evitar que o instalador do Visual Studio C++ intercepte o pacote).*

---

## Passo 2 — Crie um Novo Projeto (`rtsc init`)

No terminal, crie a estrutura completa do projeto com um único comando:

```bash
rtsc init meu_jogo
cd meu_jogo
```

Isso gera automaticamente (em UTF-8 puro sem BOM):

```text
meu_jogo/
├── rts.toml          # Manifesto do projeto (compiler = "2.0.0-rts", target = "native")
├── .gitignore        # Ignora /build/ e binários nativos (*.exe, *.pdb, *.obj, *.o)
├── src/
│   └── main.rts      # Ponto de entrada executável com `pub fn main() -> int`
└── build/            # Código C99, headers `.h` e executáveis gerados pelo rtsc
```

---

## Passo 3 — Escrevendo Código Fluido com `let`, `impl`, `import` e `cap`

Em `RustTS`, você só precisa declarar tipos nas **assinaturas de funções (`fn`)** e nas **definições de estruturas (`struct`)** — garantindo que os contratos de memória e capacidades (`@sealed_sig`) permaneçam 100% explícitos e auditáveis para humanos e IA. Dentro do corpo das funções, o `let` deduce tudo automaticamente.

### 3.1. Crie o módulo `src/telemetria.rts`

```rust
module telemetria;
import core.math;

@component
struct Sensor {
    id: uint,
    canal: byte,
    ativo: boolean,
    leitura_atual: number,
    nome: string,
}

impl Sensor {
    @sealed_sig
    pub fn novo(id: uint, canal: byte, leitura: number, nome: string) -> Sensor {
        return Sensor {
            id: id,
            canal: canal,
            ativo: true,
            leitura_atual: leitura,
            nome: nome,
        };
    }

    @sealed_sig
    pub fn temperatura_compensada(self: Sensor, offset: float) -> number {
        // Inferência automática no `let`: sem precisar repetir `: float`
        let ajustada = self.leitura_atual + offset;
        return math::clamp(ajustada, 0.0, 120.0);
    }
}
```

### 3.2. Importe e execute em `src/main.rts`

```rust
module main;
import core.io;
import telemetria;

@sealed_sig
pub fn main(io: cap StdIO) -> int {
    // Inferência completa estilo TypeScript:
    let s1 = Sensor::novo(101, 0x48, 78.5, "TERMO-CORE-01");
    let offset_ambiente = -2.5;
    let temp_final = s1.temperatura_compensada(offset_ambiente);
    let historico = [72.0, 74.5, 76.0, temp_final];

    io::println(s1.nome);
    io::print_f32("Temperatura compensada (C): ", historico[3]);
    return 0;
}
```

---

## Passo 4 — Validar, Executar e Depurar (`rtsc check` e `rtsc run`)

### 1. Validação Estática Instantânea (`< 15 ms`)
Verifica tipos, exaustividade de `match`, escopo de arenas e capacidades transitivas em todos os módulos importados:
```bash
rtsc check src/main.rts
```
*(Para integração com agentes de IA e auto-correção determinística, adicione `--json-diagnostics`).*

### 2. Compilar e Executar Nativamente (`rtsc run`)
Transpila todos os arquivos `.rts` (via C99 SIMD ou diretamente via LLVM IR com `--llvm`), invoca o `clang -O3 -mavx2 -mfma` automaticamente e executa o binário:
```bash
rtsc run src/main.rts
rtsc run src/main.rts --llvm
```

### 3. Depuração Source-Level linha a linha (`rtsc run -g`)
Injeta diretivas `#line <linha> "<arquivo>.rts"` no C99 e compila com `-g -O0`, permitindo colocar breakpoints diretamente nos arquivos `.rts` via LLDB, GDB ou VS Code:
```bash
rtsc run src/main.rts -g
```

---

## Passo 5 — Compilar para CPU (LLVM/MLIR/C99), GPU (WGSL/SPIR-V), Sandbox WASI, Motores de Jogos e Web SDK (`rtsc build`)

O mesmo código `.rts` pode ser exportado para qualquer alvo suportado pelo compilador:

```bash
# 1. Binário Nativo Stand-Alone (.exe / ELF) via C99 ou Direto via LLVM IR (.ll):
rtsc build src/main.rts --bin --out-dir build
rtsc build src/main.rts --target llvm-ir --bin -o build/app_llvm.exe
rtsc build src/main.rts --target mlir -o build/app.mlir

# 2. Biblioteca Nativa + Header C/C++ (.h) para linkar com C, C++ ou Rust:
rtsc build src/main.rts --emit-header --out-dir build

# 3. Shaders GPU Compute (@gpu_kernel): WebGPU (.wgsl) e Vulkan/DX12 SPIR-V 1.5 (.spvasm / .spv):
rtsc build examples/kanori_gpu_wasi_sim/src/gpu_particles.rts --target wgsl -o build/gpu_particles.wgsl
rtsc build examples/kanori_gpu_wasi_sim/src/gpu_particles.rts --target spirv -o build/gpu_particles.spvasm
rtsc build examples/kanori_gpu_wasi_sim/src/gpu_particles.rts --target spv -o build/gpu_particles.spv

# 4. Sandbox WASI Determinística com Fuel Metering (.wat e binário .wasm):
rtsc build examples/kanori_gpu_wasi_sim/src/sandbox_agent.rts --target wasi --fuel-limit 75000 -o build/agent.wat
rtsc build examples/kanori_gpu_wasi_sim/src/sandbox_agent.rts --target wasm --fuel-limit 75000 -o build/agent.wasm

# 5. Linkagem direta com bibliotecas C externas (ex: Raylib, SDL2, OpenGL):
rtsc run src/main.rts -l raylib -l opengl32 -l gdi32 -l winmm

# 6. Unreal Engine 5 (gera A<Actor>.h e A<Actor>.cpp com macros UCLASS/UPROPERTY):
rtsc build examples/combat_unreal.rts --target unreal --out-dir build/unreal

# 7. Unity Engine (gera plugin C-ABI em Rust + wrapper C# zero-copy com NativeArray<T>):
rtsc build examples/engine_physics.rts --target unity --out-dir build/unity

# 8. TypeScript SDK (gera cliente tipado com proteção de 64 bits e RtsResult<T, E>):
rtsc build examples/telemetry_server.rts --emit-ts-sdk -o build/telemetry_client_sdk.ts
```

---

## Próximos Passos e Exemplos Prontos no Repositório

Explore a pasta [`examples/`](./examples/) para ver projetos reais em funcionamento:
- [`examples/kanori_gpu_wasi_sim/src/main.rts`](./examples/kanori_gpu_wasi_sim/src/main.rts) — **Showcase Heterogêneo `v2.0.0-rts`** unificando CPU (`--llvm`), GPU Compute (`@gpu_kernel` $\rightarrow$ `wgsl`/`spirv`) e Sandbox WASI (`--fuel-limit`).
- [`examples/kanori_zone_sim/src/main.rts`](./examples/kanori_zone_sim/src/main.rts) — Simulação multi-arquivo de zonas térmicas e risco ecológico (**KANORI**).
- [`examples/ballistics_10k_benchmark.rts`](./examples/ballistics_10k_benchmark.rts) — Simulação física SIMD AVX2 de 10.000 projéteis a **$7,58\text{ }\mu\text{s/frame}$**.
- [`examples/thermal_control.rts`](./examples/thermal_control.rts) — Controle de hardware embarcados (`I2CBus<0x48>`, `PWMPin<0>`, `MonotonicClock`) com falha segura.
