<p align="center">
  <img src="./rustts_language_logo.png" alt="RustTS Logo" width="220">
</p>

# Como Iniciar um Projeto com RustTS (`v1.5.0-rts`)

Na versão **`v1.5.0-rts`**, a RustTS une a **ergonomia e fluidez do TypeScript e C** com a **segurança determinística e performance AVX2 do Rust e C99**:
- **Inferência de Tipos Completa no `let`:** Você não precisa anotar `: f32` ou `: u32` em toda variável local (`let velocidade = 12.5;`, `let contador = 0;`, `let ativo = true;`, `let pos = vec3(1.0, 2.0, 3.0);`).
- **Dois Estilos de Tipos de Largura Fixa (Zero Ambiguidade):** Você pode usar os nomes amigáveis do TypeScript/C (`int`, `uint`, `byte`, `float`, `number`, `boolean`, `string`) **ou** os tipos explícitos de hardware (`i8..i64`, `u8..u64`, `f32`, `f64`, `bool`, `str`). Ambos compilam para exatamente os mesmos registradores C99 (`int32_t`, `uint32_t`, `uint8_t`, `float`, `bool`, `const char*`) com **zero overhead**.

---

## Tabela Rápida de Tipos: Estilo TypeScript/C vs. Estilo Hardware

Use o estilo que for mais confortável para a sua equipe (ou misture ambos livremente no mesmo arquivo):

| O que você quer representar | Estilo Amigável (TypeScript / C) | Estilo Explícito (Rust / Hardware) | Tipo C99 Gerado (`codegen_native`) | Exemplo de Inferência Automática (`let`) |
| :--- | :--- | :--- | :--- | :--- |
| **Inteiro com sinal (32-bit)** | `int` | `i32` | `int32_t` | `let delta = -10;` |
| **Inteiro sem sinal (32-bit)** | `uint` | `u32` | `uint32_t` | `let id = 42;` |
| **Byte / Registrador (8-bit)** | `byte` | `u8` | `uint8_t` | `let reg: byte = 0x48;` |
| **Ponto Flutuante SIMD (32-bit)** | `float` ou `number` | `f32` | `float` | `let temp = 36.5;` |
| **Ponto Flutuante Duplo (64-bit)** | `f64` | `f64` | `double` | `let preciso: f64 = 3.1415926535;` |
| **Booleano Lógico** | `boolean` | `bool` | `bool` | `let online = true;` |
| **Texto Imutável** | `string` | `str` | `const char*` | `let tag = "SENSOR-01";` |
| **Vetores e Quaternions SIMD** | `vec2`, `vec3`, `vec4`, `quat` | `vec2`, `vec3`, `vec4`, `quat` | `rts_vec3_t` (ou lanes SoA) | `let pos = vec3(1.0, 2.0, 3.0);` |
| **Array de Tamanho Fixo na Stack** | `[float; 4]` ou `[number; 4]` | `[f32; 4]` | `float nome[4]` | `let pesos = [1.0, 2.0, 3.0, 4.0];` |

---

## Passo 1 — Verifique o Compilador e Instale a Extensão da IDE

1. Confirme no terminal que o compilador `rtsc` está atualizado:
   ```bash
   rtsc --version
   # Saída esperada: rtsc 1.5.0-rts
   ```
   *(Se precisar instalar ou atualizar, veja [INSTALACAO_E_USO.md](./INSTALACAO_E_USO.md)).*

2. **Ative o Realce de Sintaxe e Snippets no VS Code ou Antigravity IDE:**
   - **Pacote VSIX pronto para VS Code e Antigravity IDE:** [`editors/rustts-lang-1.4.0.vsix`](./editors/rustts-lang-1.4.0.vsix) — abra o editor e pressione **`Ctrl + Shift + P`** → **`Extensions: Install from VSIX...`** (ou **`Developer: Install Extension from Location...`** selecionando a pasta [`editors/vscode-rustts`](./editors/vscode-rustts)).
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
├── rts.toml          # Manifesto do projeto (compiler = "1.5.0-rts", target = "native")
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
Transpila todos os arquivos `.rts` para C99 SIMD, invoca o `clang -O3 -mavx2 -mfma` automaticamente e executa o binário:
```bash
rtsc run src/main.rts
```

### 3. Depuração Source-Level linha a linha (`rtsc run -g`)
Injeta diretivas `#line <linha> "<arquivo>.rts"` no C99 e compila com `-g -O0`, permitindo colocar breakpoints diretamente nos arquivos `.rts` via LLDB, GDB ou VS Code:
```bash
rtsc run src/main.rts -g
```

---

## Passo 5 — Compilar para Motores de Jogos, C/C++ e Web SDK (`rtsc build`)

O mesmo código `.rts` pode ser exportado para qualquer alvo suportado pelo compilador:

```bash
# 1. Binário Nativo Stand-Alone (.exe / ELF) + Código C99:
rtsc build src/main.rts --bin --out-dir build

# 2. Biblioteca Nativa + Header C/C++ (.h) para linkar com C, C++ ou Rust:
rtsc build src/main.rts --emit-header --out-dir build

# 3. Linkagem direta com bibliotecas C externas (ex: Raylib, SDL2, OpenGL):
rtsc run src/main.rts -l raylib -l opengl32 -l gdi32 -l winmm

# 4. Unreal Engine 5 (gera A<Actor>.h e A<Actor>.cpp com macros UCLASS/UPROPERTY):
rtsc build examples/combat_unreal.rts --target unreal --out-dir build/unreal

# 5. Unity Engine (gera plugin C-ABI em Rust + wrapper C# zero-copy com NativeArray<T>):
rtsc build examples/engine_physics.rts --target unity --out-dir build/unity

# 6. TypeScript SDK (gera cliente tipado com proteção de 64 bits e RtsResult<T, E>):
rtsc build examples/telemetry_server.rts --emit-ts-sdk -o build/telemetry_client_sdk.ts
```

---

## Próximos Passos e Exemplos Prontos no Repositório

Explore a pasta [`examples/`](./examples/) para ver projetos reais em funcionamento:
- [`examples/kanori_zone_sim/src/main.rts`](./examples/kanori_zone_sim/src/main.rts) — Simulação multi-arquivo de zonas térmicas e risco ecológico (**KANORI**).
- [`examples/ballistics_10k_benchmark.rts`](./examples/ballistics_10k_benchmark.rts) — Simulação física SIMD AVX2 de 10.000 projéteis a **$7,58\text{ }\mu\text{s/frame}$**.
- [`examples/thermal_control.rts`](./examples/thermal_control.rts) — Controle de hardware embarcados (`I2CBus<0x48>`, `PWMPin<0>`, `MonotonicClock`) com falha segura.
