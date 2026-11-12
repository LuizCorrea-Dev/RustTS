# Guia de Contribuição — RustTS (`rtsc`)

Obrigado pelo interesse em contribuir com o **RustTS (`rtsc`)**! Este repositório reúne o compilador de sistemas determinístico em Rust e seus backends para **C99 SIMD/SoA**, **Unreal Engine 5**, **Unity** e **TypeScript SDK**.

---

## 1. Princípios Inegociáveis do Projeto

Antes de propor alterações na linguagem ou no compilador, tenha em mente os pilares que definem a RustTS:

1. **Zero Autoridade Ambiental (`@sealed_sig`):** Nenhum sistema pode acessar hardware, disco, rede ou mutar estado global sem receber explicitamente sua capacidade (`cap`) na assinatura.
2. **Hierarquia Estrita de Arenas ($O(1)$ / Zero GC):** Sem `malloc`/`free` manuais ou *Garbage Collector*. Fluxos de tempo de vida respeitam `Static > Level/Column > Frame/Request` (`RTS-E0501`).
3. **Sincronia Gramatical (Parser ↔ GBNF):** Qualquer adição ou mudança na sintaxe `.rts` precisa atualizar simultaneamente o lexer/parser e o exportador formal (`src/gbnf_exporter.rs`) para manter geração livre de erros em LLMs.
4. **Código Nativo Sem Warnings (`-Wall -Wextra -Werror`):** Todo C99 emitido por `--target=native` deve compilar limpo em Clang/GCC com `-std=c11 -O3 -mavx2 -mfma -Wall -Wextra -Werror`.
5. **Diagnósticos Acionáveis (`--json-diagnostics`):** Novos erros estáticos devem emitir códigos `RTS-EXXXX` claros com nó da AST e sugestão determinística de correção.

---

## 2. Mapa do Código-Fonte (`src/`)

| Arquivo | Responsabilidade |
| :--- | :--- |
| [`src/lexer.rs`](./src/lexer.rs) | Tokenização léxica via `logos` |
| [`src/ast.rs`](./src/ast.rs) | Definições da AST (`Module`, `Stmt`, `Expr`, `TypeExpr`, `CapabilityDecl`, etc.) |
| [`src/parser.rs`](./src/parser.rs) | Parser recursivo para `.rts` |
| [`src/gbnf_exporter.rs`](./src/gbnf_exporter.rs) | Exportação da gramática formal `.gbnf` (`rtsc --export-gbnf`) |
| [`src/sema.rs`](./src/sema.rs) | *TypeChecker* (`RTS-E0301`), `match` exaustivo (`RTS-E0302`), capacidades (`RTS-E0402`–`E0405`), arenas (`RTS-E0501`) e regras Tier 1/Tier 0 (`RTS-E0900`) |
| [`src/diagnostics.rs`](./src/diagnostics.rs) | Formatação de relatórios para terminal humano e `--json-diagnostics` |
| [`src/codegen_native.rs`](./src/codegen_native.rs) | Backend C99 (`SoA 64B`, *Strip-Mining L1d*, unrolling vetorial, `TryOp ?`, `switch`, acoplamento `ReadFrameArena`) |
| [`src/codegen_unreal.rs`](./src/codegen_unreal.rs) | Emissor C++ UHT (`.h` / `.cpp`) para Unreal Engine 5 |
| [`src/codegen_unity.rs`](./src/codegen_unity.rs) | Emissor FFI Rust (`extern "C"`) + wrapper C# `NativeArray<T>` para Unity |
| [`src/codegen_ts_sdk.rs`](./src/codegen_ts_sdk.rs) | Emissor TypeScript SDK (`RtsResult<T, ApiError>`, `u64`/`i64` → `string`) |
| [`src/main.rs`](./src/main.rs) | CLI `rtsc` (`check`, `build`, `--export-gbnf`) |
| [`src/lib.rs`](./src/lib.rs) | Pipeline integrado `compile_and_check` e suíte de testes unitários |

---

## 3. Configurando o Ambiente Local

### Pré-requisitos
- **Rust (`rustc` / `cargo`):** `edition = "2021"` (testado com `rustc 1.99.1` via [rustup.rs](https://rustup.rs)).
- **LLVM / Clang** (recomendado se for alterar `codegen_native.rs` ou benchmarks C99).

### Clone e Validação Inicial
```bash
git clone https://github.com/<SEU_USUARIO>/RustTS.git
cd RustTS

cargo test
cargo build --release
```

---

## 4. Fluxo de Contribuição (Fork & Pull Request)

1. Faça um **Fork** do repositório [`LuizCorrea-Dev/RustTS`](https://github.com/LuizCorrea-Dev/RustTS) e crie uma branch descritiva a partir da `main`:
   ```bash
   git checkout -b feat/sua-melhoria
   ```
2. Siga o padrão **Conventional Commits** nas mensagens:
   - `feat(sema): ...` — nova regra estática ou recurso de linguagem
   - `fix(codegen_native): ...` — correção em geração C99/SIMD
   - `perf(codegen_native): ...` — otimização de latência/cache (anexe números medidos antes/depois)
   - `test(...): ...` — novos testes em `src/lib.rs` ou `.rts`
   - `docs(...): ...` — documentação e exemplos
3. **Salve arquivos `.rts` e `.c` em UTF-8 sem BOM** (atenção especial no Windows PowerShell).

---

## 5. Checklist Obrigatório Antes de Enviar o PR

Execute os comandos abaixo após sua edição final e garanta 100% de aprovação:

1. **Suíte unitária do compilador:**
   ```bash
   cargo test
   ```
2. **Validação estática em todos os exemplos oficiais (`0` violações):**
   ```bash
   cargo run -- check examples/thermal_control.rts
   cargo run -- check examples/ballistics_10k_benchmark.rts
   cargo run -- check examples/engine_physics.rts
   cargo run -- check examples/combat_unreal.rts
   cargo run -- check examples/telemetry_server.rts
   ```
3. **Exportação dos backends auxiliares:**
   ```bash
   cargo run -- --export-gbnf > /dev/null
   cargo run -- build examples/combat_unreal.rts --target=unreal > /dev/null
   cargo run -- build examples/engine_physics.rts --target=unity > /dev/null
   cargo run -- build examples/telemetry_server.rts --emit-ts-sdk > /dev/null
   ```
   *(No Windows PowerShell, use `| Out-Null` em vez de `> /dev/null`.)*
4. **Validação C99 / AVX2 (obrigatória ao alterar `codegen_native.rs` ou `examples/*.c`):**
   ```bash
   clang -std=c11 -O3 -mavx2 -mfma -Wall -Wextra -Werror examples/thermal_control_benchmark.c -o thermal_check
   clang -std=c11 -O3 -mavx2 -mfma -Wall -Wextra -Werror examples/engine_physics_benchmark.c -o physics_check
   clang -std=c11 -O3 -mavx2 -mfma -Wall -Wextra -Werror examples/ballistics_10k_benchmark.c -o ballistics_check
   ```
5. **Inclua testes de regressão em `src/lib.rs`** cobrindo novos diagnósticos `RTS-EXXXX` ou novas construções de lowering.

---

## 6. Reportando Bugs ou Sugerindo Recursos

Ao abrir uma **Issue** em [LuizCorrea-Dev/RustTS/issues](https://github.com/LuizCorrea-Dev/RustTS/issues), inclua:
- Saída de `rtsc --version` e seu sistema operacional;
- Trecho mínimo em `.rts` que reproduz o problema;
- Saída de `rtsc check arquivo.rts --json-diagnostics` (ou do erro do compilador C/Clang se ocorrer no código gerado).

