<p align="center">
  <img src="./rustts_language_logo.png" alt="RustTS Logo" width="220">
</p>

# Guia Oficial de Instalação e Uso — RustTS (`rtsc v2.2.0-rts`)

Bem-vindo ao **RustTS (`.rts`)**, o compilador determinístico de sistemas, motores em tempo real e controle físico de hardware baseado em **Capacidades (`cap`)**, **Arenas Escopadas ($O(1)$)** e geração nativa **SoA/AVX2**.

Este guia cobre instalação via instaladores pré-compilados (`v2.0.0`), compilação a partir do código-fonte, manual completo da CLI `rtsc` (CPU LLVM/C99, GPU SPIR-V/WGSL e Sandbox WASI), depuração source-level `#line` e execução dos exemplos inclusos.

---

## 1. Instalação

### A. Instaladores Pré-Compilados (GitHub Release [`v2.0.0`](https://github.com/LuizCorrea-Dev/RustTS/releases/tag/v2.0.0))

Os binários pré-compilados da release `v2.0.0` possuem **zero dependências externas** para análise semântica e transpilações:

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

### B. Compilando a Partir do Código-Fonte (`v2.2.0-rts`)

Com o toolchain **Rust (`cargo` / `rustc`)** instalado (via [rustup.rs](https://rustup.rs) ou `winget install Rustlang.Rustup` no Windows):

```bash
git clone https://github.com/LuizCorrea-Dev/RustTS.git
cd RustTS

# Executa os 36 testes unitários do compilador
cargo test

# Compila o executável otimizado (LTO fat, opt-level 3)
cargo build --release

# Instala globalmente em ~/.cargo/bin (ou %USERPROFILE%\.cargo\bin)
cargo install --path .
```

### Validar a Instalação
```bash
rtsc --version
# Saída esperada: rtsc 2.2.0-rts
```

### Requisito Opcional para Execução Nativa (`rtsc run` e `--bin`)
- Para rodar `rtsc check`, `--export-gbnf`, `--emit-ts-sdk`, `--emit-header`, `--target=unreal` e `--target=unity`, o `rtsc` **não precisa de compilador C externo**.
- Para compilar e executar código nativo AVX2/FMA3 diretamente via `rtsc run` ou `rtsc build --bin`, utilize **LLVM/Clang** (localizado automaticamente no `PATH` ou no diretório LLVM do Visual Studio no Windows):
  - **Windows:** `winget install LLVM.LLVM` (ou componente LLVM do Visual Studio 2022/2026)
  - **Linux (Debian/Ubuntu):** `sudo apt install clang`
  - **macOS:** `xcode-select --install`

---

## 2. Manual de Comandos do Compilador (`rtsc 2.2.0-rts`)

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
[rtsc 2.2.0-rts] OK: 0 capability violations, 0 transitive leaks, 0 capability forgeries, 0 arena escapes.
```

### 2.3. Compilar e Executar Imediatamente (`rtsc run`)
Transpila o projeto `.rts` (via C99 ou diretamente via LLVM IR com `--llvm`), invoca o Clang/LLVM e executa o binário nativo:
```bash
# Release otimizado (-O3 -mavx2 -mfma -Wall -Wextra -Werror)
rtsc run examples/kanori_zone_sim/src/main.rts

# Pipeline direto LLVM IR (.ll -> executável nativo sem passar por C99)
rtsc run examples/kanori_gpu_wasi_sim/src/main.rts --llvm

# Modo debug (-g -O0) com mapeamento `#line` para GDB / LLDB / Visual Studio
rtsc run examples/kanori_zone_sim/src/main.rts -g

# Linkando bibliotecas C externas (-l, -L, -I)
rtsc run src/main.rts -l raylib -L ./libs -I ./include
```

### 2.4. Gerar Código Nativo C99, Header `.h`, MLIR SSA (`.mlir`) e LLVM IR Direto (`.ll`)
```bash
# Emitir .c e header ABI C/C++ (.h com #pragma once e extern "C") em diretório dedicado
rtsc build examples/kanori_zone_sim/src/main.rts --out-dir build/kanori --emit-header

# Emitir dialeto intermediário MLIR SSA (func, scf, memref, arith, vector)
rtsc build examples/kanori_gpu_wasi_sim/src/main.rts --target mlir -o build/kanori.mlir

# Emitir LLVM IR textual (.ll) ou compilar direto de LLVM IR para binário nativo (--bin)
rtsc build examples/kanori_gpu_wasi_sim/src/main.rts --target llvm-ir -o build/kanori.ll
rtsc build examples/kanori_gpu_wasi_sim/src/main.rts --target llvm-ir --bin -o build/kanori_llvm.exe
```

### 2.5. Gerar Compute Shaders GPU: WebGPU (`--target=wgsl`) e Vulkan/DX12 (`--target=spirv` / `spv`)
Compila kernels anotados com `@gpu_kernel` (validados contra `RTS-E0601`–`E0603`):
```bash
# WebGPU Shading Language (.wgsl)
rtsc build examples/kanori_gpu_wasi_sim/src/gpu_particles.rts --target wgsl -o build/gpu_particles.wgsl

# Vulkan / DirectX 12 SPIR-V 1.5 Assembly (.spvasm) e Binário (.spv magic 0x07230203)
rtsc build examples/kanori_gpu_wasi_sim/src/gpu_particles.rts --target spirv -o build/gpu_particles.spvasm
# Binário Vulkan/DX12 SPIR-V 1.5 (.spv, magic 0x07230203) para Compute e Shaders Gráficos
rtsc build examples/kanori_gpu_wasi_sim/src/gpu_particles.rts --target spv -o build/gpu_particles.spv
rtsc build examples/shaders/custom_tri.rts --target spirv-vert --bin -o build/custom_vs.spv
rtsc build examples/shaders/custom_tri.rts --target spirv-frag --bin -o build/custom_fs.spv
```

### 2.6. Gerar Sandbox WASI Determinística com *Fuel Metering* (`--target=wasi` / `wasm`)
Compila módulos isolados de IA/plugins com injeção automática de `$__rts_fuel` e bloqueio estático de hardware bruto (`RTS-E0701`):
```bash
# WebAssembly Text (.wat) e Binário WebAssembly 1.0 (.wasm magic \0asm) com assinaturas e opcodes reais da AST
rtsc build examples/kanori_gpu_wasi_sim/src/sandbox_agent.rts --target wasi --fuel-limit 75000 -o build/agent.wat
rtsc build examples/kanori_gpu_wasi_sim/src/sandbox_agent.rts --target wasm --fuel-limit 75000 -o build/agent.wasm
```

### 2.7. Gerar Atores C++ para Unreal Engine 5 (`--target=unreal`)
Emite pares `A<Actor>.h` e `A<Actor>.cpp` separados e compatíveis com UHT (*Unreal Header Tool*):
```bash
rtsc build examples/combat_unreal.rts --target=unreal --out-dir build/unreal
```

### 2.8. Gerar Plugin Rust C-ABI + Wrapper C# Zero-Copy para Unity (`--target=unity`)
Emite `<modulo>_unity_plugin.rs` (ex.: `engine_physics_unity_plugin.rs`) e `RtscNativeBridge.cs` operando sobre `NativeArray<T>`:
```bash
rtsc build examples/engine_physics.rts --target=unity --out-dir build/unity
```

### 2.9. Gerar Cliente TypeScript Tipado (`--emit-ts-sdk`) e Gramática Formal GBNF (`--export-gbnf`)
```bash
rtsc build examples/telemetry_server.rts --emit-ts-sdk -o build/telemetry_client_sdk.ts
rtsc --export-gbnf > rustts.gbnf
```

---

## 3. Executando os Exemplos e Showcases Inclusos

| Arquivo `.rts` | Domínio / Recurso Demonstrado |
| :--- | :--- |
| `examples/demo_2d.rts` | Motor 2D Vulkan (`v2.2.0-rts`): câmera ortográfica `MODE_ORTHO_2D`, `SpriteBatch2D`, texturas RGBA e animação (`sprite_anim`) |
| `examples/demo_3d.rts` | Motor 3D Vulkan (`v2.2.0-rts`): câmera orbital, `Renderer3D` (`r3d_add_box`, `r3d_flush`) e delta-time `rtsc_time_now_sec()` |
| `examples/demo_shader.rts` | Shaders gráficos em `.rts` (`@vertex_shader`/`@fragment_shader` -> `.spv`) carregados via `rhi_create_graphics_pipeline` |
| `examples/village_editor.rts` | Editor visual 3D completo com instancing, loader `.OBJ` e persistência binária `.pak` |
| `examples/kanori_gpu_wasi_sim/src/main.rts` | Showcase Heterogêneo `v2.2.0-rts`: CPU (`--target llvm-ir`) + GPU Compute (`@gpu_kernel` $\rightarrow$ `wgsl`/`spirv`) + Sandbox WASI (`--target wasi --fuel-limit 75000`) |
| `examples/kanori_zone_sim/src/main.rts` | Showcase multi-arquivo (`import`, `impl Struct`, `core.math`, `Option<f32>`, `extern "C" fn`, `[f32; 4]`, `while` + `continue`/`break`, `#line -g`) |
| `examples/thermal_control.rts` | Hardware embarcado (`I2CBus`, `PWMPin`, `TryOp ?`, `match` exaustivo, `while`) |
| `examples/ballistics_10k_benchmark.rts` | Simulação ECS de 10.000 projéteis com `FrameArena`, `ReadFrameArena` e Strip-Mining L1d |
| `examples/engine_physics.rts` | Desdobramento vetorial `vec3` em colunas SoA + detecção de colisão |
| `examples/combat_unreal.rts` | Gameplay / replicação direcionada ao backend Unreal Engine 5 |
| `examples/telemetry_server.rts` | API de servidor com `SqlRead<T>`, `RequestArena` e geração `--emit-ts-sdk` |

Execução dos showcases KANORI e checagem de exemplos:
```bash
rtsc run examples/kanori_gpu_wasi_sim/src/main.rts --llvm
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

## 4. Extensão para VS Code e Antigravity IDE (`v2.0.0`)

A extensão oficial fornece realce semântico de sintaxe (`cap`, `@component`, `@system`, `@sealed_sig`, `@gpu_kernel`, `@shader_shared`, `?`, tipos vetoriais e capacidades), configuração de linguagem e snippets (`comp`, `sys`, `externc`, `gpukernel`). Consulte também o guia dedicado em [`editors/vscode-rustts/README.md`](./editors/vscode-rustts/README.md).

- **Pacote VSIX pronto para VS Code e Antigravity IDE:** [`editors/rustts-lang-2.0.0.vsix`](./editors/rustts-lang-2.0.0.vsix) (instale via `Ctrl + Shift + P` → `Extensions: Install from VSIX...` → selecione `editors/rustts-lang-2.0.0.vsix` → `Developer: Reload Window`).
- **Código-fonte da extensão e snippets:** [`editors/vscode-rustts/`](./editors/vscode-rustts/package.json) (carregue via `Ctrl + Shift + P` → `Developer: Install Extension from Location...`).

> **Atenção (Windows):** **NÃO dê duplo-clique** no arquivo `.vsix` pelo Windows Explorer. O Windows associa `.vsix` ao `VSIXInstaller.exe` do *Microsoft Visual Studio Build Tools* (C++/C#), exibindo o erro `MissingPackagePartException`. Instale sempre por dentro do **VS Code** ou **Antigravity IDE**, ou via terminal:

**Instalação via PowerShell (VS Code + Antigravity IDE):**
```powershell
foreach ($base in @("$env:USERPROFILE\.antigravity\extensions", "$env:USERPROFILE\.vscode\extensions")) {
    if (Test-Path (Split-Path $base -Parent)) {
        $dest = "$base\rootszera.rustts-lang-2.0.0"
        Remove-Item $dest -Recurse -Force -ErrorAction SilentlyContinue
        New-Item -ItemType Directory -Force -Path $dest | Out-Null
        Copy-Item -Path "editors\vscode-rustts\*" -Destination $dest -Recurse -Force
    }
}
```

**Instalação via Bash/Zsh (Linux / macOS):**
```bash
mkdir -p ~/.vscode/extensions/rootszera.rustts-lang-2.0.0 && cp -r editors/vscode-rustts/* ~/.vscode/extensions/rootszera.rustts-lang-2.0.0/
mkdir -p ~/.antigravity/extensions/rootszera.rustts-lang-2.0.0 && cp -r editors/vscode-rustts/* ~/.antigravity/extensions/rootszera.rustts-lang-2.0.0/
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
