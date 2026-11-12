# Guia Oficial de Instalação e Uso — RustTS (`rtsc v1.0.0-rts`)

Bem-vindo ao **RustTS (`.rts`)**, o compilador determinístico de sistemas, motores em tempo real e controle físico de hardware baseado em **Capacidades (`cap`)**, **Arenas Escopadas ($O(1)$)** e geração nativa **SoA/AVX2**.

Este guia é destinado a **usuários e desenvolvedores** que desejam instalar e utilizar o `rtsc` a partir do GitHub — sem precisar configurar nenhuma cadeia de compilação complexa para começar.

---

## 1. Instalação Rápida (Binários Pré-Compilados)

Os executáveis oficiais da release **`v1.0.0`** estão publicados na aba [GitHub Releases](https://github.com/LuizCorrea-Dev/RustTS/releases/tag/v1.0.0) e possuem **zero dependências externas** para executar análise semântica e transpilações.

### Windows (PowerShell)
Abra o PowerShell e execute:
```powershell
irm https://raw.githubusercontent.com/LuizCorrea-Dev/RustTS/main/scripts/install.ps1 | iex
```
O instalador baixa automaticamente `rtsc-windows-x64.exe` para `%USERPROFILE%\.rtsc\bin\rtsc.exe` e adiciona essa pasta ao `PATH` do usuário.

> **Dica:** Caso o terminal atual ainda não reconheça o comando `rtsc` imediatamente após instalar, feche e reabra o PowerShell.

### Linux e macOS (Bash / Zsh)
Execute no terminal:
```bash
curl -fsSL https://raw.githubusercontent.com/LuizCorrea-Dev/RustTS/main/scripts/install.sh | bash
```
O script instala o binário em `~/.rtsc/bin/rtsc` (`rtsc-linux-x64` ou `rtsc-macos-arm64`) com permissão de execução. Adicione ao seu perfil (`~/.bashrc` ou `~/.zshrc`):
```bash
export PATH="$HOME/.rtsc/bin:$PATH"
```

### Download Manual (GitHub Releases)
Você também pode baixar o binário diretamente da página [Releases `v1.0.0`](https://github.com/LuizCorrea-Dev/RustTS/releases/tag/v1.0.0):
- **Windows x64:** `rtsc-windows-x64.exe`
- **Linux x64:** `rtsc-linux-x64`
- **macOS (Apple Silicon ARM64):** `rtsc-macos-arm64`

### Validar a Instalação
```bash
rtsc --version
```
Saída esperada:
```text
rtsc 1.0.0-rts
```

---

## 2. Compilar a Partir do Código-Fonte (Opcional)

Se você clonou o repositório para estudar o compilador, contribuir ou gerar binários para outra plataforma, será necessário ter o toolchain **Rust (`cargo` / `rustc`)** instalado (ex.: via [rustup.rs](https://rustup.rs) ou `winget install Rustlang.Rustup` no Windows):

```bash
git clone https://github.com/LuizCorrea-Dev/RustTS.git
cd RustTS

# Executa os 13 testes unitários do compilador (TypeChecker, Match, Capacidades, Arenas e Codegen)
cargo test

# Compila o executável otimizado (LTO fat, opt-level 3)
cargo build --release

# Instala globalmente em ~/.cargo/bin
cargo install --path .
```

### Requisito Opcional para Compilação de Código Nativo (`.c` → Binário AVX2)
* Para rodar `rtsc check`, `--export-gbnf`, `--emit-ts-sdk`, `--target=unreal` e `--target=unity`, o `rtsc` **não precisa de nenhum compilador externo**.
* Para compilar o código **C99** emitido por `--target=native` em um executável `.exe`/ELF ou DLL com vetorização **AVX2/FMA3**, utilize **LLVM/Clang** (ou GCC compatível):
  * **Windows:** `winget install LLVM.LLVM` (ou LLVM incluso no Visual Studio Build Tools)
  * **Linux (Debian/Ubuntu):** `sudo apt install clang`
  * **macOS:** `xcode-select --install`

---

## 3. Manual de Comandos do Compilador (`rtsc`)

### 3.1. Validar Contratos, Tipos, Arenas e Capacidades (`rtsc check`)
Executa verificação estática completa (códigos `RTS-E0301` a `RTS-E0900`):

```bash
# Saída legível para humanos no terminal:
rtsc check examples/thermal_control.rts

# Saída estruturada em JSON para pipelines de auto-correção por IA:
rtsc check examples/thermal_control.rts --json-diagnostics
```
Saída esperada em arquivos válidos:
```text
[rtsc 1.0.0-rts] OK: 0 capability violations, 0 transitive leaks, 0 capability forgeries, 0 arena escapes.
```

### 3.2. Gerar Código Nativo C99 + AVX2/SoA (`--target=native`)
Transpila o módulo `.rts` para C99 com descritores SoA alinhados em cache line (`64 bytes`), *Strip-Mining* em L1d e laços vetorizados:

```bash
rtsc build examples/ballistics_10k_benchmark.rts --target=native > ballistics_gen.c
```

### 3.3. Gerar Atores C++ para Unreal Engine 5 (`--target=unreal`)
Emite cabeçalho `.h` e implementação `.cpp` compatíveis com UHT (*Unreal Header Tool*):

```bash
rtsc build examples/combat_unreal.rts --target=unreal > combat_unreal_ue5.cpp
```

### 3.4. Gerar Plugin Rust C-ABI + Wrapper C# Zero-Copy para Unity (`--target=unity`)
Emite a ponte FFI `extern "C"` e o wrapper C# operando diretamente sobre `NativeArray<T>` sem Garbage Collector:

```bash
rtsc build examples/engine_physics.rts --target=unity > engine_physics_unity.txt
```

### 3.5. Gerar Cliente TypeScript Tipado para Frontend (`--emit-ts-sdk`)
Gera contratos TypeScript seguros (`u64`/`i64` mapeados para `string` contra truncamento IEEE-754 e retornos `RtsResult<T, E>` sem `throw`):

```bash
rtsc build examples/telemetry_server.rts --emit-ts-sdk > telemetry_client_sdk.ts
```

### 3.6. Exportar Gramática Formal GBNF (`--export-gbnf`)
Exporta a gramática completa da versão `v1.0.0-rts` para *Constrained Decoding* em motores LLM (`llama.cpp`, `vLLM`):

```bash
rtsc --export-gbnf > rustts.gbnf
```

---

## 4. Executando os Exemplos e Benchmarks Inclusos

Os exemplos oficiais ficam na pasta `examples/` do repositório:

| Arquivo `.rts` | Domínio / Recurso Demonstrado |
| :--- | :--- |
| `examples/thermal_control.rts` | Hardware embarcado (`I2CBus`, `PWMPin`, `TryOp ?`, `match` exaustivo, `while`) |
| `examples/ballistics_10k_benchmark.rts` | Simulação ECS de 10.000 projéteis com `FrameArena`, `ReadFrameArena` e Strip-Mining L1d |
| `examples/engine_physics.rts` | Desdobramento vetorial `vec3` em colunas SoA + detecção de colisão |
| `examples/combat_unreal.rts` | Gameplay / replicação direcionada ao backend Unreal Engine 5 |
| `examples/telemetry_server.rts` | API de servidor com `SqlRead<T>`, `RequestArena` e geração `--emit-ts-sdk` |

Para validar todos os exemplos via CLI:
```bash
rtsc check examples/thermal_control.rts
rtsc check examples/ballistics_10k_benchmark.rts
rtsc check examples/engine_physics.rts
rtsc check examples/combat_unreal.rts
rtsc check examples/telemetry_server.rts
```

Para compilar e executar o benchmark de balística de 10.000 projéteis com **Clang** (x86_64 AVX2 + FMA3):

```bash
clang -std=c11 -O3 -mavx2 -mfma -Wall -Wextra -Werror examples/ballistics_10k_benchmark.c -o ballistics_bench
./ballistics_bench
```

Da mesma forma, os harnesses de física e controle térmico podem ser compilados e executados diretamente:
```bash
clang -std=c11 -O3 -mavx2 -mfma -Wall -Wextra -Werror examples/engine_physics_benchmark.c -o engine_physics_bench
./engine_physics_bench

clang -std=c11 -O3 -mavx2 -mfma -Wall -Wextra -Werror examples/thermal_control_benchmark.c -o thermal_control_bench
./thermal_control_bench
```

---

## 5. Criando Seu Primeiro Projeto em RustTS

1. Crie um arquivo chamado `meu_sensor.rts`:

```rust
module fabrica.meu_sensor;
import core.hardware;
import core.math;

struct Limites {
    max_temp: f32,
}

enum FalhaSensor {
    Offline(u8),
    Critico(f32),
}

pub fn monitorar(
    lim: Limites,
    i2c: cap I2CBus<0x48>,
    cooler: cap PWMPin<4>
) -> Result<f32, FalhaSensor> {
    let bruto: u16 = i2c.read_register_u16(0x00)
        .map_err(|_| FalhaSensor::Offline(0x48))?;

    let temp: f32 = (bruto as f32) * 0.0625;
    if temp > lim.max_temp {
        cooler.set_duty(100);
        return Err(FalhaSensor::Critico(temp));
    }

    let duty: u8 = math::clamp(temp, 20.0, 80.0) as u8;
    cooler.set_duty(duty);
    return Ok(temp);
}
```

2. Verifique estaticamente tipos, capacidades e escopos de memória:
```bash
rtsc check meu_sensor.rts
```
Saída esperada:
```text
[rtsc 1.0.0-rts] OK: 0 capability violations, 0 transitive leaks, 0 capability forgeries, 0 arena escapes.
```

3. Gere o código C99 determinístico:
```bash
rtsc build meu_sensor.rts --target=native > meu_sensor.c
```

---

## 6. Solução Rápida de Problemas

- **`rtsc` não encontrado no Windows após rodar `install.ps1`:**
  Feche e abra uma nova janela do PowerShell para recarregar o `PATH`, ou adicione manualmente `%USERPROFILE%\.rtsc\bin` às Variáveis de Ambiente do Usuário.
- **Erro de ExecutionPolicy no PowerShell ao rodar script local:**
  O comando remoto `irm ... | iex` roda direto em memória; caso execute um arquivo `.ps1` baixado manualmente, rode antes na sessão:
  `Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass`.
- **`rtsc` não encontrado no Linux/macOS após `install.sh`:**
  Garanta que `~/.rtsc/bin` esteja no seu `PATH` (`export PATH="$HOME/.rtsc/bin:$PATH"`) e recarregue o shell (`source ~/.bashrc` ou `source ~/.zshrc`).
- **`Invalid lexical token` na linha 1:1 ao criar arquivos pelo Windows PowerShell:**
  Salve arquivos `.rts` em **UTF-8 sem BOM** (`UTF-8 without BOM`). No PowerShell 5.1, `Set-Content -Encoding UTF8` adiciona um BOM (`U+FEFF`) no início do arquivo; prefira editores como VS Code (UTF-8) ou `Out-File` / `[System.IO.File]::WriteAllText(..., [System.Text.UTF8Encoding]::new($false))`.
