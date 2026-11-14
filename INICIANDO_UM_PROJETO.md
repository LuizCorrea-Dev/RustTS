<p align="center">
  <img src="./rustts_language_logo.png" alt="RustTS Logo" width="220">
</p>

# Como Iniciar um Projeto com RustTS (`v1.4.0-rts`)

Na versão **`v1.4.0-rts`**, o compilador `rtsc` opera como um toolchain autônomo de sistemas e motores de jogos: inicialização de projetos (`rtsc init`), resolução recursiva multi-arquivo em disco (`import`), compilação direta via LLVM/Clang com vetorização AVX2 ou depuração source-level `#line` (`rtsc run [-g]`), blocos de métodos nativos (`impl Struct`), FFI C-ABI protegido por capacidades (`extern "C" fn`), exportação de cabeçalhos C/C++ (`--emit-header`) e geração de código para **C99 Nativo**, **Unreal Engine 5**, **Unity** e **TypeScript SDK**.

## Passo 1 — Verifique a Instalação do Compilador e Extensão VS Code

1. Confirme no terminal que o binário `rtsc` está disponível no `PATH`:
   ```bash
   rtsc --version
   # Saída esperada: rtsc 1.4.0-rts
   ```
   *(Caso ainda não tenha instalado o compilador, consulte [INSTALACAO_E_USO.md](./INSTALACAO_E_USO.md)).*

2. **Ative o Syntax Highlighting Oficial no VS Code:**
   Copie a extensão oficial `editors/vscode-rustts` para a pasta de extensões do seu usuário e recarregue a janela do VS Code (`Ctrl+Shift+P` → `Developer: Reload Window`):
   ```powershell
   # Windows (PowerShell)
   Copy-Item -Path "editors\vscode-rustts" -Destination "$env:USERPROFILE\.vscode\extensions\rustts-lang-1.4.0" -Recurse -Force
   ```
   ```bash
   # Linux / macOS
   cp -r editors/vscode-rustts ~/.vscode/extensions/rustts-lang-1.4.0
   ```

## Passo 2 — Crie um Projeto Autônomo (`rtsc init`)

Use o subcomando `rtsc init` para gerar a estrutura padrão do projeto:

```bash
rtsc init meu_projeto
cd meu_projeto
```

Estrutura criada automaticamente em UTF-8 sem BOM:

```text
meu_projeto/
├── rts.toml          # Manifesto do pacote (compiler = "1.4.0-rts", target = "native")
├── .gitignore        # Ignora /build/ e artefatos nativos (*.exe, *.pdb, *.obj, *.o)
├── src/
│   └── main.rts      # Entrypoint nativo com `pub fn main() -> i32`
└── build/            # Código C99, headers `.h` e executáveis gerados pelo rtsc
```

> **Nota sobre codificação no Windows:** o lexer do `rtsc` remove de forma transparente o UTF-8 BOM (`\u{FEFF}`) na leitura de arquivos `.rts`, e todos os arquivos gerados por `-o` / `--out-dir` são gravados em UTF-8 puro.

## Passo 3 — Organize Múltiplos Módulos (`import`, `impl` e `extern "C" fn`)

O `rtsc` resolve automaticamente dependências declaradas com `import`, mesclando as ASTs em ordem topológica e validando tipos (`RTS-E0301`) e capacidades transitivas (`RTS-E0403`) entre todos os arquivos do projeto.

### 3.1. Crie o módulo de domínio em `src/meu_sensor.rts`

```rust
module meu_sensor;
import core.hardware;
import core.math;

struct Limites {
    max_temp: f32,
    fator_cooler: f32,
}

enum FalhaSensor {
    Offline(u8),
    Critico(f32),
}

impl Limites {
    pub fn calcular_duty(self: Limites, temp: f32) -> u8 {
        let bruto: f32 = temp * self.fator_cooler;
        return math::clamp(bruto, 20.0, 80.0) as u8;
    }
}

@sealed_sig
pub fn monitorar(
    lim: Limites,
    i2c: cap I2CBus<0x48>,
    cooler: cap PWMPin<4>
) -> Result<f32, FalhaSensor> {
    let reg: u16 = i2c.read_register_u16(0x00)
        .map_err(|_| FalhaSensor::Offline(0x48))?;

    let temp: f32 = (reg as f32) * 0.0625;
    if temp > lim.max_temp {
        cooler.set_duty(100);
        return Err(FalhaSensor::Critico(temp));
    }

    let duty: u8 = lim.calcular_duty(temp);
    cooler.set_duty(duty);
    return Ok(temp);
}
```

### 3.2. Conecte no entrypoint `src/main.rts` (com FFI C-ABI protegido por `cap`)

Parâmetros `cap` declarados em funções `extern "C" fn` exigem comprovação estática de autoridade em tempo de compilação e são removidos automaticamente na assinatura C99 final (zero overhead na ABI C):

```rust
module app;
import core.hardware;
import core.io;
import meu_sensor;

extern "C" fn puts(msg: str, console: cap StdIO) -> i32;

pub fn main() -> i32 {
    let console: cap StdIO = StdIO::new();
    let i2c: cap I2CBus<0x48> = I2CBus::new(0x48);
    let cooler: cap PWMPin<4> = PWMPin::new(4);

    puts("=== [RustTS v1.4.0-rts] Inicializando Telemetria ===", console);

    let lim: Limites = Limites {
        max_temp: 85.0,
        fator_cooler: 0.75,
    };

    let res: Result<f32, FalhaSensor> = monitorar(lim, i2c, cooler);
    match res {
        Ok(temp) => {
            io::print_f32("[OK] Temperatura Nominal (C): ", temp);
            return 0;
        }
        Err(_) => {
            puts("[ALERTA] Falha de barramento I2C ou superaquecimento!", console);
            return 1;
        }
    }
}
```

## Passo 4 — Verifique, Compile e Depure (`rtsc check` / `rtsc run`)

### 4.1. Verificação Estática Multi-Arquivo (`< 15 ms`)
Valida sintaxe, tipos, exaustividade de `match` (`RTS-E0302`), ausência de vazamento de arenas (`RTS-E0501`) e grafo transitivo de capacidades (`RTS-E0402`..`RTS-E0404`):

```bash
rtsc check src/main.rts
```
Saída esperada:
```text
[rtsc 1.4.0-rts] OK: 0 capability violations, 0 transitive leaks, 0 capability forgeries, 0 arena escapes.
```
*(Adicione `--json-diagnostics` para emitir relatórios JSON estruturados para agentes de IA).*

### 4.2. Execução Direta Otimizada (`-O3 -mavx2 -mfma`)
Transpila todos os módulos importados para `build/main.c`, invoca o LLVM/Clang com `-std=c11 -O3 -mavx2 -mfma -Wall -Wextra -Werror` e executa o binário nativo imediatamente:

```bash
rtsc run src/main.rts
```

### 4.3. Depuração Source-Level Direto no `.rts` (`-g` / `--debug`)
Compila com `-g -O0` e mapeia cada instrução C99 de volta para a linha exata do arquivo `.rts` via diretivas `#line`:

```bash
rtsc run src/main.rts -g
```
*(Você pode abrir `build/main.exe` no debugger do Visual Studio, VS Code `cppvsdbg`/`codelldb`, GDB ou LLDB e posicionar breakpoints diretamente nas linhas de `src/main.rts`).*

## Passo 5 — Exportação de Artefatos e Integração com Engines / Hosts Externos

Para integrar módulos `.rts` em motores C/C++ próprios, Unreal Engine 5, Unity ou frontends Next.js/React, use `rtsc build`:

### 5.1. Engine Própria C/C++ (`--emit-header` + Linkagem Externa `-l / -L / -I`)
Gera `build/main.c` e o cabeçalho ABI `build/main.h` (com `#pragma once` e `extern "C"` contendo apenas as funções `pub fn` do módulo):

```bash
# Gera build/main.c e build/main.h
rtsc build src/main.rts --out-dir build --emit-header

# Compila um executável linkando contra bibliotecas C externas (ex: Raylib / SDL3 / OpenGL)
rtsc build src/main.rts --bin -o build/game.exe -l raylib -l opengl32 -L ./libs -I ./include
```

### 5.2. Unreal Engine 5 (`--target=unreal`)
Emite os arquivos `A<Actor>.h` e `A<Actor>.cpp` separados e prontos para o Unreal Header Tool (UHT), incluindo `UCLASS()`, `UPROPERTY()`, `UFUNCTION()`, `DOREPLIFETIME` e mapeamento matemático para `FMath::*`:

```bash
rtsc build examples/combat_unreal.rts --target=unreal --out-dir Source/MeuJogoUE5
```

### 5.3. Unity Engine (`--target=unity`)
Emite o código FFI Rust (`<modulo>_unity_plugin.rs`) e o wrapper C# zero-copy (`RtscNativeBridge.cs` com `NativeArray<T>` e `NativeArrayUnsafeUtility`):

```bash
rtsc build examples/engine_physics.rts --target=unity --out-dir build/unity
```
*(Compile o arquivo `build/unity/engine_physics_unity_plugin.rs` como uma biblioteca dinâmica Rust `cdylib` chamada `rtsc_native_plugin.dll` / `.so` / `.dylib` e copie a DLL junto com `RtscNativeBridge.cs` para `Assets/Plugins/RustTS/` no seu projeto Unity).*

### 5.4. Frontend Web TypeScript / Next.js (`--emit-ts-sdk`)
Gera um cliente HTTP TypeScript estritamente tipado (`RtsResult<T, E>` sem `throw` e inteiros de 64 bits mapeados como `string` para evitar perda de precisão IEEE-754):

```bash
rtsc build examples/telemetry_server.rts --emit-ts-sdk -o frontend/src/sdk/telemetry_client.ts
```

### 5.5. Gramática Formal GBNF para LLMs Locais (`--export-gbnf`)
Exporta a gramática formal `1.4.0-rts` para *Constrained Decoding* (`0%` de erro sintático em `llama.cpp` / `vLLM`):

```powershell
# Windows (PowerShell — garante gravação em UTF-8 sem BOM)
rtsc --export-gbnf | Set-Content -Path build/rustts.gbnf -Encoding utf8
```
```bash
# Linux / macOS
rtsc --export-gbnf > build/rustts.gbnf
```

## Passo 6 — Projetos e Showcases de Referência (`examples/`)

| Domínio | Arquivos de Referência | Comando de Teste / Execução |
| --- | --- | --- |
| **Showcase Multi-Arquivo KANORI (`v1.4.0-rts`)** | [`examples/kanori_zone_sim/src/main.rts`](./examples/kanori_zone_sim/src/main.rts)<br>[`examples/kanori_zone_sim/src/anomaly_math.rts`](./examples/kanori_zone_sim/src/anomaly_math.rts) | `rtsc run examples/kanori_zone_sim/src/main.rts -g` |
| **Controle de Hardware (`I2C`/`PWM`) & `match`** | [`examples/thermal_control.rts`](./examples/thermal_control.rts)<br>[`examples/thermal_control_benchmark.c`](./examples/thermal_control_benchmark.c) | `rtsc check examples/thermal_control.rts` |
| **Balística SIMD AVX2 (`10.000` projéteis em `7.58 µs`)** | [`examples/ballistics_10k_benchmark.rts`](./examples/ballistics_10k_benchmark.rts)<br>[`examples/ballistics_10k_benchmark.c`](./examples/ballistics_10k_benchmark.c) | `rtsc check examples/ballistics_10k_benchmark.rts` |
| **Física ECS SoA 64B (`vec3` Unrolling)** | [`examples/engine_physics.rts`](./examples/engine_physics.rts)<br>[`examples/engine_physics_benchmark.c`](./examples/engine_physics_benchmark.c) | `rtsc check examples/engine_physics.rts` |
| **Ator Replicado Unreal Engine 5** | [`examples/combat_unreal.rts`](./examples/combat_unreal.rts) | `rtsc build examples/combat_unreal.rts --target=unreal --out-dir build/ue5` |
| **Servidor Telemetria + SDK TypeScript** | [`examples/telemetry_server.rts`](./examples/telemetry_server.rts)<br>[`examples/telemetry_client_sdk.ts`](./examples/telemetry_client_sdk.ts) | `rtsc build examples/telemetry_server.rts --emit-ts-sdk -o build/client.ts` |
