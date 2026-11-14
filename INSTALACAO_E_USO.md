<p align="center">
  <img src="./rustts_language_logo.png" alt="RustTS Logo" width="220">
</p>

# Guia Oficial de Instalação e Uso — RustTS (`rtsc v1.5.0-rts`)

Bem-vindo ao **RustTS (`.rts`)**, o compilador determinístico de sistemas, motores em tempo real e controle físico de hardware baseado em **Capacidades (`cap`)**, **Arenas Escopadas ($O(1)$)** e geração nativa **SoA/AVX2**.

Este guia cobre instalação via instaladores pré-compilados (`v1.5.0`), compilação a partir do código-fonte, manual completo da CLI `rtsc`, depuração source-level `#line` e execução dos exemplos inclusos.

---

## 1. Instalação

### A. Instaladores Pré-Compilados (GitHub Release [`v1.5.0`](https://github.com/LuizCorrea-Dev/RustTS/releases/tag/v1.5.0))

Os binários pré-compilados da release `v1.5.0` possuem **zero dependências externas** para análise semântica e transpilações:

**Windows (PowerShell):**
```powershell
irm https://raw.githubusercontent.com/LuizCorrea-Dev/RustTS/main/scripts/install.ps1 | iex
```

**Linux e macOS (Bash / Zsh):**
```bash
curl -fsSL https://raw.githubusercontent.com/LuizCorrea-Dev/RustTS/main/scripts/install.sh | bash
# Adicione ao perfil se necessário:
export PATH="$HOME/.rtsc/bin:$PATH"
```

### B. Compilando a Partir do Código-Fonte (`v1.5.0-rts`)

Com o toolchain **Rust (`cargo` / `rustc`)** instalado (via [rustup.rs](https://rustup.rs) ou `winget install Rustlang.Rustup` no Windows):

```bash
git clone https://github.com/LuizCorrea-Dev/RustTS.git
cd RustTS

# Executa os 23 testes unitários do compilador
cargo test

# Compila o executável otimizado (LTO fat, opt-level 3)
cargo build --release

# Instala globalmente em ~/.cargo/bin (ou %USERPROFILE%\.cargo\bin)
cargo install --path .
```

### Validar a Instalação
```bash
rtsc --version
# Saída esperada: rtsc 1.5.0-rts
```

### Requisito Opcional para Execução Nativa (`rtsc run` e `--bin`)
- Para rodar `rtsc check`, `--export-gbnf`, `--emit-ts-sdk`, `--emit-header`, `--target=unreal` e `--target=unity`, o `rtsc` **não precisa de compilador C externo**.
- Para compilar e executar código nativo AVX2/FMA3 diretamente via `rtsc run` ou `rtsc build --bin`, utilize **LLVM/Clang** (localizado automaticamente no `PATH` ou no diretório LLVM do Visual Studio no Windows):
  - **Windows:** `winget install LLVM.LLVM` (ou componente LLVM do Visual Studio 2022/2026)
  - **Linux (Debian/Ubuntu):** `sudo apt install clang`
  - **macOS:** `xcode-select --install`

---

## 2. Manual de Comandos do Compilador (`rtsc 1.5.0-rts`)

### 2.1. Criar Novo Projeto (`rtsc init`)
Gera `rts.toml`, `.gitignore`, `src/main.rts` (com `pub fn main() -> i32`) e `build/`:
```bash
rtsc init meu_projeto
```

### 2.2. Validar Contratos, Tipos, Arenas e Múltiplos Módulos (`rtsc check`)
Executa verificação estática completa (`RTS-E0101` a `RTS-E0900`), incluindo resolução recursiva de `import` em disco (`RTS-E0102`):
```bash
rtsc check examples/thermal_control.rts
rtsc check examples/kanori_zone_sim/src/main.rts --json-diagnostics
```
Saída esperada:
```text
[rtsc 1.5.0-rts] OK: 0 capability violations, 0 transitive leaks, 0 capability forgeries, 0 arena escapes.
```

### 2.3. Compilar e Executar Imediatamente (`rtsc run`)
Transpila o projeto `.rts` (incluindo módulos importados) para C99 em `build/`, invoca o Clang e executa o binário nativo:
```bash
# Release otimizado (-O3 -mavx2 -mfma -Wall -Wextra -Werror)
rtsc run examples/kanori_zone_sim/src/main.rts

# Modo debug (-g -O0) com mapeamento `#line` para GDB / LLDB / Visual Studio
rtsc run examples/kanori_zone_sim/src/main.rts -g

# Linkando bibliotecas C externas (-l, -L, -I)
rtsc run src/main.rts -l raylib -L ./libs -I ./include
```

### 2.4. Gerar Código Nativo C99, Header `.h` e Binários (`rtsc build --target=native`)
```bash
# Emitir .c e header ABI C/C++ (.h com #pragma once e extern "C") em diretório dedicado
rtsc build examples/kanori_zone_sim/src/main.rts --out-dir build/kanori --emit-header

# Compilar diretamente para executável AVX2
rtsc build src/main.rts --bin -o build/app.exe
```

### 2.5. Gerar Atores C++ para Unreal Engine 5 (`--target=unreal`)
Emite pares `A<Actor>.h` e `A<Actor>.cpp` separados e compatíveis com UHT (*Unreal Header Tool*):
```bash
rtsc build examples/combat_unreal.rts --target=unreal --out-dir build/unreal
```

### 2.6. Gerar Plugin Rust C-ABI + Wrapper C# Zero-Copy para Unity (`--target=unity`)
Emite `<modulo>_unity_plugin.rs` (ex.: `engine_physics_unity_plugin.rs`) e `RtscNativeBridge.cs` operando sobre `NativeArray<T>`:
```bash
rtsc build examples/engine_physics.rts --target=unity --out-dir build/unity
```

### 2.7. Gerar Cliente TypeScript Tipado (`--emit-ts-sdk`)
```bash
rtsc build examples/telemetry_server.rts --emit-ts-sdk -o build/telemetry_client_sdk.ts
```

### 2.8. Exportar Gramática Formal GBNF (`--export-gbnf`)
Exporta a gramática completa da versão `v1.5.0-rts` para *Constrained Decoding* em motores LLM (`llama.cpp`, `vLLM`):
```powershell
# Windows (PowerShell — garante gravação em UTF-8 sem BOM)
rtsc --export-gbnf | Set-Content -Path rustts.gbnf -Encoding utf8
```
```bash
# Linux / macOS
rtsc --export-gbnf > rustts.gbnf
```

---

## 3. Executando os Exemplos e Showcase Inclusos

| Arquivo `.rts` | Domínio / Recurso Demonstrado |
| :--- | :--- |
| `examples/kanori_zone_sim/src/main.rts` | Showcase multi-arquivo `v1.5.0-rts` (`import`, `impl Struct`, `core.math`, `Option<f32>`, `extern "C" fn`, `[f32; 4]`, `while` + `continue`/`break`, `#line -g`) |
| `examples/thermal_control.rts` | Hardware embarcado (`I2CBus`, `PWMPin`, `TryOp ?`, `match` exaustivo, `while`) |
| `examples/ballistics_10k_benchmark.rts` | Simulação ECS de 10.000 projéteis com `FrameArena`, `ReadFrameArena` e Strip-Mining L1d |
| `examples/engine_physics.rts` | Desdobramento vetorial `vec3` em colunas SoA + detecção de colisão |
| `examples/combat_unreal.rts` | Gameplay / replicação direcionada ao backend Unreal Engine 5 |
| `examples/telemetry_server.rts` | API de servidor com `SqlRead<T>`, `RequestArena` e geração `--emit-ts-sdk` |

Execução do showcase multi-arquivo KANORI e checagem de exemplos:
```bash
rtsc run examples/kanori_zone_sim/src/main.rts -g
rtsc check examples/thermal_control.rts
rtsc check examples/ballistics_10k_benchmark.rts
rtsc check examples/engine_physics.rts
rtsc check examples/combat_unreal.rts
rtsc check examples/telemetry_server.rts
```

Compilação e execução manual dos harnesses de benchmark C99 com Clang (`-O3 -mavx2 -mfma`):
```bash
mkdir -p build
clang -std=c11 -O3 -mavx2 -mfma -Wall -Wextra -Werror examples/ballistics_10k_benchmark.c -o build/ballistics_bench.exe
clang -std=c11 -O3 -mavx2 -mfma -Wall -Wextra -Werror examples/engine_physics_benchmark.c -o build/engine_physics_bench.exe
clang -std=c11 -O3 -mavx2 -mfma -Wall -Wextra -Werror examples/thermal_control_benchmark.c -o build/thermal_control_bench.exe
```

---

## 4. Extensão para VS Code e Antigravity IDE (`v1.5.0`)

A extensão oficial fornece realce semântico de sintaxe (`cap`, `@component`, `@system`, `@sealed_sig`, `?`, tipos vetoriais e capacidades), configuração de linguagem e snippets (`comp`, `sys`, `externc`). Consulte também o guia dedicado em [`editors/vscode-rustts/README.md`](./editors/vscode-rustts/README.md).

- **Pacote VSIX pronto para VS Code e Antigravity IDE:** [`editors/rustts-lang-1.5.0.vsix`](./editors/rustts-lang-1.5.0.vsix) (instale via `Ctrl + Shift + P` → `Extensions: Install from VSIX...` → selecione `editors/rustts-lang-1.5.0.vsix` → `Developer: Reload Window`).
- **Código-fonte da extensão e snippets:** [`editors/vscode-rustts/`](./editors/vscode-rustts/package.json) (carregue via `Ctrl + Shift + P` → `Developer: Install Extension from Location...`).

> **Atenção (Windows):** **NÃO dê duplo-clique** no arquivo `.vsix` pelo Windows Explorer. O Windows associa `.vsix` ao `VSIXInstaller.exe` do *Microsoft Visual Studio Build Tools* (C++/C#), exibindo o erro `MissingPackagePartException`. Instale sempre por dentro do **VS Code** ou **Antigravity IDE**, ou via terminal:

**Instalação via PowerShell (VS Code + Antigravity IDE):**
```powershell
foreach ($base in @("$env:USERPROFILE\.antigravity\extensions", "$env:USERPROFILE\.vscode\extensions")) {
    if (Test-Path (Split-Path $base -Parent)) {
        $dest = "$base\rootszera.rustts-lang-1.5.0"
        Remove-Item $dest -Recurse -Force -ErrorAction SilentlyContinue
        New-Item -ItemType Directory -Force -Path $dest | Out-Null
        Copy-Item -Path "editors\vscode-rustts\*" -Destination $dest -Recurse -Force
    }
}
```

**Instalação via Bash/Zsh (Linux / macOS):**
```bash
mkdir -p ~/.vscode/extensions/rootszera.rustts-lang-1.5.0 && cp -r editors/vscode-rustts/* ~/.vscode/extensions/rootszera.rustts-lang-1.5.0/
mkdir -p ~/.antigravity/extensions/rootszera.rustts-lang-1.5.0 && cp -r editors/vscode-rustts/* ~/.antigravity/extensions/rootszera.rustts-lang-1.5.0/
```

---

## 5. Solução Rápida de Problemas

- **`rtsc` não encontrado no Windows após rodar `install.ps1` ou `cargo install`:**
  Feche e reabra o terminal para recarregar o `PATH`, ou garanta que `%USERPROFILE%\.cargo\bin` (ou `%USERPROFILE%\.rtsc\bin`) esteja nas Variáveis de Ambiente do Usuário.
- **Erro de ExecutionPolicy no PowerShell ao rodar script local:**
  Caso execute um arquivo `.ps1` baixado manualmente, rode antes na sessão: `Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass`.
- **`rtsc` não encontrado no Linux/macOS após `install.sh`:**
  Garanta que `~/.rtsc/bin` esteja no `PATH` (`export PATH="$HOME/.rtsc/bin:$PATH"`) e recarregue o shell.
- **Arquivos salvos com UTF-8 BOM no Windows:**
  A partir da `v1.1.0-rts`, o `rtsc` remove o BOM `\u{FEFF}` automaticamente no lexer. Ainda assim, recomenda-se usar **UTF-8 sem BOM** para compatibilidade com ferramentas externas.
