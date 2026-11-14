# RustTS (`rtsc v1.4.0-rts`) — 100% Production Roadmap Release

Compilador de sistemas e motores de jogos determinístico projetado para **Humanos e Agentes de IA**, unindo **Segurança Baseada em Capacidades (`cap` / `@sealed_sig`)**, **Arenas Escopadas $O(1)$ (`FrameArena` / `ReadFrameArena`)** e **Vetorização Nativa AVX2 SoA (`0.758 ns/projétil`)**.

---

## Instalação Rápida (1 Linha — Sem necessidade de Rust/Cargo)

### Windows (PowerShell)
```powershell
irm https://raw.githubusercontent.com/LuizCorrea-Dev/RustTS/main/scripts/install.ps1 | iex
```

### Linux / macOS
```bash
curl -fsSL https://raw.githubusercontent.com/LuizCorrea-Dev/RustTS/main/scripts/install.sh | bash
```

---

## O Que Há de Novo na `v1.4.0-rts` (100% do Roadmap de Produção)

1. **CLI Autônoma (`rtsc init`, `rtsc run`, `--bin`, `-o`, `--out-dir`):**
   * Crie projetos prontos com `rtsc init meu_jogo` e compile + execute nativamente em um único comando com `rtsc run src/main.rts`.
   * Remoção transparente de UTF-8 BOM no Lexer e escrita direta em UTF-8 puro.

2. **Resolução Multi-Arquivo Real em Disco (`import` + `RTS-E0102`):**
   * Carregamento recursivo de módulos `.rts`, detecção de ciclos, mesclagem topológica para inlining total em C99 e verificação inter-módulos de tipos (`RTS-E0301`) e capacidades transitivas (`RTS-E0403`).

3. **FFI C-ABI com Guarda de Capacidades Zero-Overhead (`extern "C" fn`):**
   * Chame bibliotecas C nativas (`libc`, Raylib, SDL3, Win32, OpenGL) com `extern "C" fn`. Parâmetros `cap` impõem verificação estática de segurança em tempo de compilação e são removidos automaticamente da assinatura C99 gerada.
   * Suporte às flags de linkagem `-l <lib>`, `-L <dir>` e `-I <dir>` diretamente em `rtsc build` e `rtsc run`.

4. **Linguagem Completa (`Option<T>`, `[T; N]`, `break`/`continue`, `impl Struct`, `core.math`):**
   * Lowering C99 sem ponteiros nulos para `Option<T>` (`rts_option_<T>_t`), arrays fixos na stack `[T; N]`, métodos nativos via `impl Struct` e biblioteca matemática `<math.h>` completa (`clamp`, `min`, `max`, `abs`, `sqrt`, `sin`, `cos`, `atan2`, `floor`, `ceil`, `lerp`).

5. **Source-Level Debugging (`#line` + `-g` / `--debug`) & Exportação de Header C/C++ (`--emit-header`):**
   * Emissão de diretivas `#line <linha> "<modulo>.rts"` no C99 gerado: depure diretamente o código `.rts` no Visual Studio, VS Code (`codelldb`/`cppvsdbg`), GDB ou LLDB usando `rtsc run arquivo.rts -g`.
   * Gere cabeçalhos `.h` (`#pragma once` + `extern "C"`) com `rtsc build arquivo.rts --emit-header` para integração imediata com engines C/C++.

6. **Extensão Oficial VS Code & Antigravity IDE + Showcase KANORI:**
   * **Pacote VSIX pronto para VS Code e Antigravity IDE:** [`editors/rustts-lang-1.4.0.vsix`](./editors/rustts-lang-1.4.0.vsix) (instale via `Ctrl + Shift + P` → `Extensions: Install from VSIX...`, sem dar duplo-clique pelo Windows Explorer para não invocar o `VSIXInstaller.exe` do Visual Studio C++).
   * **Código-fonte da extensão, TextMate e snippets (`comp`, `sys`, `externc`):** [`editors/vscode-rustts/`](./editors/vscode-rustts/README.md) (`Ctrl + Shift + P` → `Developer: Install Extension from Location...`).
   * Projeto multi-arquivo completo em [`examples/kanori_zone_sim/`](./examples/kanori_zone_sim/src/main.rts).

---

## Telemetria de Performance e Garantias Estáticas (`22/22` Testes Unitários)

| Métrica / Garantia | Resultado Homologado (`Clang 22.1.3 -O3 -mavx2 -mfma`) |
| --- | --- |
| **Simulação Balística 10.000 Entidades (SoA 64B)** | **`7.581 µs/frame`** (`0.758 ns/projétil` — Produtor) / **`8.398 µs/frame`** (Produtor + Consumidor) |
| **Verificação Estática (`rtsc check`)** | `RTS-E0101`..`RTS-E0900` (`TypeMismatch`, `NonExhaustiveMatch`, `CapabilityViolation`, `ArenaLifetimeEscape`) |
| **Targets Suportados** | `native` (C99/AVX2), `unreal` (UE5 UHT `.h`/`.cpp`), `unity` (C-ABI + C# `NativeArray`), `ts-sdk` |

---

## Binários Pré-Compilados Nesta Release

* **`rtsc-windows-x64.exe`** — Windows 10/11 (x86_64 MSVC)
* **`rtsc-linux-x64`** — Linux (x86_64 GNU)
* **`rtsc-macos-arm64`** — macOS Apple Silicon (M1/M2/M3/ARM64)
