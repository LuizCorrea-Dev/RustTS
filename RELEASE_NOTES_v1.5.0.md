# RustTS (`rtsc v1.5.0-rts`) — TypeScript/C/C#/UE5 Type Ergonomics, Full `let` Inference & Official Language Manual

Compilador determinístico de sistemas, motores de jogos e controle físico de hardware projetado para **Humanos e Agentes de IA**, unindo **Ergonomia TypeScript/C/C#/UE5**, **Segurança Baseada em Capacidades (`cap` / `@sealed_sig`)**, **Arenas Escopadas $O(1)$ (`FrameArena` / `ReadFrameArena`)** e **Vetorização Nativa AVX2 SoA (`0.758 ns/projétil`)**.

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

## O Que Há de Novo na `v1.5.0-rts`

1. **Família Completa de Aliases Escalares (TypeScript / C / C# / Unreal Engine 5 — Custo Zero na AST):**
   * Todos os **14 tipos escalares** da RustTS agora aceitam seus aliases consagrados de mercado diretamente no DFA do lexer (`src/lexer.rs`), compilando para os mesmos nós canônicos `TypeExpr::Primitive` sem overhead:
     * **Com Sinal:** `i8` (`int8`, `sbyte`), `i16` (`int16`, `short`), `i32` (`int`, `int32`), `i64` (`int64`, `long`)
     * **Sem Sinal:** `u8` (`byte`, `uint8`), `u16` (`uint16`, `ushort`), `u32` (`uint`, `uint32`), `u64` (`uint64`, `ulong`), `usize` (`size_t`, `uintptr`)
     * **Ponto Flutuante e Fixo:** `f32` (`number`, `float`, `float32`), `f64` (`double`, `float64`), `fix32` (`fixed32`)
     * **Booleano e Texto:** `bool` (`boolean`), `str` (`string`)

2. **Inferência Completa de Tipos em Variáveis Locais (`let` sem `: Tipo`):**
   * Implementado `NativeCodegen::infer_c_type_from_expr` (`src/codegen_native.rs`) e inferência de literais inteiros negativos (`-5` → `i32` / `int32_t`) e construtores vetoriais (`vec2`, `vec3`, `vec4`, `quat`) em `src/sema.rs` e `src/parser.rs`.
   * Permite escrever código fluido estilo TypeScript dentro de funções (`let contador = 10;`, `let delta = -5;`, `let ganho = 2.5;`, `let pos = vec3(1.0, 2.0, 3.0);`, `let pesos = [1.0, 2.0, 3.0, 4.0];`, `let tele = Telemetria::criar(...);`) mantendo contratos `@sealed_sig` 100% explícitos nas assinaturas.

3. **Manual Oficial de Tipos e Guias Atualizados (`v1.5.0-rts`):**
   * Novo **[`MANUAL_TIPOS_RUSTTS.md`](./MANUAL_TIPOS_RUSTTS.md)** cobrindo todas as categorias de tipos, tabelas comparativas multi-alvo (C99, Unreal C++, Unity C#, TypeScript SDK), capacidades `cap` e exemplos compiláveis.
   * Guias **[`INICIANDO_UM_PROJETO.md`](./INICIANDO_UM_PROJETO.md)**, **[`INSTALACAO_E_USO.md`](./INSTALACAO_E_USO.md)** e **[`devlog.md`](./devlog.md)** atualizados para a `v1.5.0-rts`.

4. **Extensão Oficial VS Code & Antigravity IDE (`v1.5.0`):**
   * Pacote instalável **[`editors/rustts-lang-1.5.0.vsix`](./editors/rustts-lang-1.5.0.vsix)** e gramática TextMate atualizados com realce semântico para todos os novos aliases de tipos escalares.

---

## Telemetria de Performance e Garantias Estáticas (`23/23` Testes Unitários)

| Métrica / Garantia | Resultado Homologado (`Clang 22.1.3 -O3 -mavx2 -mfma`) |
| --- | --- |
| **Suíte de Testes do Compilador (`cargo test`)** | **`23/23 PASS`** (incluindo `test_v150_typescript_ergonomics_type_aliases_and_let_inference`) |
| **Simulação Balística 10.000 Entidades (SoA 64B)** | **`7.581 µs/frame`** (`0.758 ns/projétil` — Produtor) / **`8.398 µs/frame`** (Produtor + Consumidor) |
| **Verificação Estática (`rtsc check`)** | `RTS-E0101`..`RTS-E0900` (`TypeMismatch`, `NonExhaustiveMatch`, `CapabilityViolation`, `ArenaLifetimeEscape`) |
| **Targets Suportados** | `native` (C99/AVX2), `unreal` (UE5 UHT `.h`/`.cpp`), `unity` (C-ABI + C# `NativeArray`), `ts-sdk` |

---

## Binários Pré-Compilados Nesta Release

* **`rtsc-windows-x64.exe`** — Windows 10/11 (x86_64 MSVC)
* **`rtsc-linux-x64`** — Linux (x86_64 GNU)
* **`rtsc-macos-arm64`** — macOS Apple Silicon (M1/M2/M3/ARM64)
