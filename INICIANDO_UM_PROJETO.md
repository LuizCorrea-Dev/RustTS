<p align="center">
  <img src="./rustts_language_logo.png" alt="RustTS Logo" width="220">
</p>

# Como Iniciar um Projeto com RustTS (`v1.4.0-rts`)

Na versão **`v1.4.0-rts`**, o compilador `rtsc` oferece fluxo completo e autônomo: criação de projetos (`rtsc init`), resolução multi-arquivo em disco (`import`), compilação direta via LLVM/Clang com vetorização AVX2 ou símbolos `#line` (`rtsc run [-g]`), FFI `extern "C"` protegido por capacidades (`cap`), exportação de cabeçalhos C/C++ (`--emit-header`) e suporte às plataformas **C99 Nativo**, **Unreal Engine 5**, **Unity** e **TypeScript SDK**.

---

## Passo 1 — Verifique a Instalação do Compilador

Confirme no terminal que o binário `rtsc` está disponível no `PATH`:

```bash
rtsc --version
# Saída esperada: rtsc 1.4.0-rts
```

*(Caso ainda não tenha instalado, consulte [INSTALACAO_E_USO.md](./INSTALACAO_E_USO.md)).*

---

## Passo 2 — Crie um Projeto Autônomo (`rtsc init`)

Use o comando `rtsc init` para gerar a estrutura padrão:

```bash
rtsc init meu_projeto
cd meu_projeto
```

Estrutura criada automaticamente:

```text
meu_projeto/
├── rts.toml          # Manifesto do projeto
├── .gitignore
├── src/
│   └── main.rts      # Entrypoint nativo com `pub fn main() -> i32`
└── build/            # Código C99, headers `.h` e binários compilados
```

> **Nota sobre codificação no Windows:** desde a `v1.1.0-rts`, o lexer do `rtsc` remove automaticamente o BOM UTF-8 (`\u{FEFF}`), evitando erros caso um editor salve com BOM (embora **UTF-8 sem BOM** continue recomendado).

---

## Passo 3 — Organize Múltiplos Módulos com `import`

Crie um módulo auxiliar em `src/meu_sensor.rts`:

```rust
module meu_sensor;
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

E conecte no entrypoint `src/main.rts`:

```rust
module app;
import core.io;
import meu_sensor;

pub fn main() -> i32 {
    let lim: Limites = Limites { max_temp: 85.0 };
    let i2c: cap I2CBus<0x48> = I2CBus::new(0x48);
    let cooler: cap PWMPin<4> = PWMPin::new(4);

    let res: Result<f32, FalhaSensor> = monitorar(lim, i2c, cooler);
    match res {
        Ok(temp) => {
            io::print_f32("[RustTS] Temperatura Nominal (C): ", temp);
            return 0;
        }
        Err(_) => {
            io::println("[RustTS] Falha ou superaquecimento detectado!");
            return 1;
        }
    }
}
```

---

## Passo 4 — Verifique, Compile e Execute (`rtsc check` / `rtsc run`)

### 4.1. Checagem Estática Multi-Arquivo (`< 15 ms`)
```bash
rtsc check src/main.rts
```
Saída esperada:
```text
[rtsc 1.4.0-rts] OK: 0 capability violations, 0 transitive leaks, 0 capability forgeries, 0 arena escapes.
```
*(Use `--json-diagnostics` para pipelines automatizados ou agentes de IA).*

### 4.2. Compilação Nativa & Execução Direta (`rtsc run`)
```bash
# Compila C99 via LLVM/Clang (-O3 -mavx2 -mfma -Wall -Wextra -Werror) e executa
rtsc run src/main.rts

# Com símbolos de depuração (-g -O0) e mapeamento `#line` direto para os arquivos `.rts`
rtsc run src/main.rts -g
```

---

## Passo 5 — Exportação de Artefatos e Integração com Engines / Hosts Externos

Quando quiser embutir o módulo `.rts` em outro projeto C/C++, engine comercial ou frontend web, utilize `rtsc build` com `-o` ou `--out-dir`:

- **Nativo C99 + Header ABI C/C++ (`.h` com `#pragma once` e `extern "C"`):**
  ```bash
  rtsc build src/main.rts --out-dir build --emit-header
  ```
- **Executável nativo direto com bibliotecas C externas (`-l`, `-L`, `-I`):**
  ```bash
  rtsc build src/main.rts --bin -o build/app.exe -l raylib -L ./libs -I ./include
  ```
- **Unreal Engine 5 (`A<Actor>.h` e `A<Actor>.cpp` compatíveis com UHT):**
  ```bash
  rtsc build src/meu_ator.rts --target=unreal --out-dir Source/MeuProjeto
  ```
- **Unity (`rtsc_plugin.rs` Rust C-ABI + `RtscNativeBridge.cs` `NativeArray<T>`):**
  ```bash
  rtsc build src/minha_fisica.rts --target=unity --out-dir Assets/Plugins/RustTS
  ```
- **Frontend Web (Cliente TypeScript Tipado para APIs `.rts`):**
  ```bash
  rtsc build src/servidor.rts --emit-ts-sdk -o frontend/src/sdk/client.ts
  ```
- **Gramática GBNF para LLMs Locais (`llama.cpp`, `vLLM`):**
  ```bash
  rtsc --export-gbnf > build/rustts.gbnf
  ```
- **Suporte no VS Code:** instale ou carregue o pacote oficial da extensão em [`editors/vscode-rustts/`](./editors/vscode-rustts/package.json).

---

## Exemplos e Showcase de Referência (`examples/`)

* **Showcase Multi-Arquivo Completo (`v1.4.0-rts`):** [`examples/kanori_zone_sim/src/main.rts`](./examples/kanori_zone_sim/src/main.rts) + [`examples/kanori_zone_sim/src/anomaly_math.rts`](./examples/kanori_zone_sim/src/anomaly_math.rts) (`rtsc run examples/kanori_zone_sim/src/main.rts -g`)
* **Hardware & `match` Exaustivo:** [`examples/thermal_control.rts`](./examples/thermal_control.rts) + [`examples/thermal_control_benchmark.c`](./examples/thermal_control_benchmark.c)
* **Simulação SIMD 10k Projéteis:** [`examples/ballistics_10k_benchmark.rts`](./examples/ballistics_10k_benchmark.rts) + [`examples/ballistics_10k_benchmark.c`](./examples/ballistics_10k_benchmark.c)
* **Física Vetorial `vec3` SoA:** [`examples/engine_physics.rts`](./examples/engine_physics.rts) + [`examples/engine_physics_benchmark.c`](./examples/engine_physics_benchmark.c)
* **Integração Unreal Engine 5:** [`examples/combat_unreal.rts`](./examples/combat_unreal.rts)
* **Backend HTTP + SDK TypeScript:** [`examples/telemetry_server.rts`](./examples/telemetry_server.rts) → [`examples/telemetry_client_sdk.ts`](./examples/telemetry_client_sdk.ts)
