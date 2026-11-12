# Como Iniciar um Projeto com RustTS (`v1.0.0-rts`)

Na versão **`v1.0.0-rts`**, o compilador `rtsc` atua como um **transpilador e verificador estático de núcleo determinístico** (a CLI atual dispõe dos comandos `check`, `build` e `--export-gbnf`, sem subcomando `init`/`new`).

O fluxo de trabalho consiste em escrever seus módulos críticos em arquivos `.rts` — com contratos de **Capacidades (`cap`)**, **Arenas Escopadas ($O(1)$)** e tipos algébricos (`Result<T, E>`, `Option<T>`, `enum` exaustivo) — e integrar o código gerado ao seu projeto **host** (aplicação C99/C++, Unreal Engine 5, Unity ou frontend TypeScript/Next.js).

---

## Passo 1 — Verifique a Instalação do Compilador

Confirme no terminal que o binário `rtsc` está disponível no `PATH`:

```bash
rtsc --version
# Saída esperada: rtsc 1.0.0-rts
```

*(Caso ainda não tenha instalado, consulte [INSTALACAO_E_USO.md](./INSTALACAO_E_USO.md)).*

---

## Passo 2 — Crie a Estrutura de Pastas do Projeto

Para um projeto nativo C99 (hardware embarcado, simulação ECS ou motor próprio), organize os diretórios assim:

```text
meu_projeto/
├── src/
│   └── meu_modulo.rts     # Lógica RustTS (tipos, capacidades, arenas)
├── host/
│   └── main.c             # Entrypoint C que instancia capacidades/arenas e chama o módulo gerado
└── build/                 # Saída gerada pelo rtsc e executável compilado
```

No terminal:
```powershell
mkdir meu_projeto\src, meu_projeto\host, meu_projeto\build
cd meu_projeto
```

---

## Passo 3 — Escreva o Primeiro Módulo `.rts` (`src/meu_modulo.rts`)

Crie o arquivo `src/meu_modulo.rts`.

> **Atenção no Windows:** salve arquivos `.rts` em **UTF-8 sem BOM** (no PowerShell 5.1, `Set-Content -Encoding UTF8` adiciona um BOM `U+FEFF` que gera `RTS-E0101: Invalid lexical token` na linha 1:1; prefira VS Code ou `[System.IO.File]::WriteAllText(..., [System.Text.UTF8Encoding]::new($false))`).

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

---

## Passo 4 — Valide e Gere o Código para o Seu Alvo

### 4.1. Checagem Estática (`< 15 ms`)
```bash
rtsc check src/meu_modulo.rts
```
Saída esperada:
```text
[rtsc 1.0.0-rts] OK: 0 capability violations, 0 transitive leaks, 0 capability forgeries, 0 arena escapes.
```
*(Se um agente de IA estiver iterando sobre o código, use `--json-diagnostics` para receber o AST node e o patch sugerido em JSON).*

### 4.2. Geração de Código por Plataforma Alvo

- **Nativo C99 + AVX2/SoA (Hardware, Simulação, Engine Própria):**
  - **Linux / macOS / Git Bash / PowerShell 7+:**
    ```bash
    rtsc build src/meu_modulo.rts --target=native > build/meu_modulo.c
    ```
  - **Windows PowerShell 5.1** *(evita redirecionamento `>` em UTF-16 LE BOM, que o Clang rejeita)*:
    ```powershell
    rtsc build src/meu_modulo.rts --target=native | Out-File -Encoding ascii build/meu_modulo.c
    ```

- **Unreal Engine 5 (`.h` / `.cpp` compatíveis com UHT):**
  ```bash
  rtsc build src/meu_modulo.rts --target=unreal > Source/MeuModulo_UE5.cpp
  ```

- **Unity (`extern "C"` Rust ABI + C# `NativeArray<T>` Zero-Copy):**
  ```bash
  rtsc build src/meu_modulo.rts --target=unity > build/unity_bridge.txt
  ```

- **Frontend Web (Cliente TypeScript Tipado para APIs `.rts`):**
  ```bash
  rtsc build src/servidor.rts --emit-ts-sdk > frontend/src/sdk/client.ts
  ```

- **Fluxos com LLMs Locais (`llama.cpp`, `vLLM`):**
  ```bash
  rtsc --export-gbnf > build/rustts.gbnf
  ```

---

## Passo 5 — Conecte ao Host (`host/main.c`) e Compile (Target Nativo)

O `--target=native` emite funções com a assinatura `<module_com_underscores>_<nome_fn>` (no exemplo acima: `fabrica_meu_sensor_monitorar`) junto com os descritores de hardware gerados no topo do arquivo (`rts_i2c_bus_t`, `rts_pwm_pin_t`).

Crie `host/main.c`:

```c
#include <stdio.h>
#include "../build/meu_modulo.c"

int main(void) {
    Limites_t lim = { .max_temp = 85.0f };
    /* 1024 * 0.0625 = 64.00 C */
    rts_i2c_bus_t i2c = { .device_address = 0x48, .bus_online = true, .registers = { 1024 } };
    rts_pwm_pin_t cooler = { .channel = 4, .current_duty = 0 };

    rts_result_float_FalhaSensor_t_t res = fabrica_meu_sensor_monitorar(lim, &i2c, &cooler);
    if (res.is_ok) {
        printf("Temp: %.2f C | PWM: %u%%\n", res.val.ok, cooler.current_duty);
    }
    return 0;
}
```

Compile com **Clang** (ou GCC) habilitando C11 e instruções AVX2/FMA:

```bash
clang -std=c11 -O3 -mavx2 -mfma -Wall -Wextra -Werror host/main.c -o build/app.exe
./build/app.exe
```

Saída observada:
```text
Temp: 64.00 C | PWM: 64%
```

---

## Próximos Passos e Projetos Avançados de Referência

Quando quiser evoluir de funções simples para pipelines ECS com **SoA compactado (`64 B`)**, **Strip-Mining L1d (`256` elementos)** e acoplamento **Produtor (`FrameArena<T>`) → Consumidor (`ReadFrameArena<T>`)**, utilize os exemplos completos do repositório como base:

* **Hardware & `match` Exaustivo:** [`examples/thermal_control.rts`](./examples/thermal_control.rts) + [`examples/thermal_control_benchmark.c`](./examples/thermal_control_benchmark.c)
* **Simulação SIMD 10k Projéteis:** [`examples/ballistics_10k_benchmark.rts`](./examples/ballistics_10k_benchmark.rts) + [`examples/ballistics_10k_benchmark.c`](./examples/ballistics_10k_benchmark.c)
* **Física Vetorial `vec3` SoA:** [`examples/engine_physics.rts`](./examples/engine_physics.rts) + [`examples/engine_physics_benchmark.c`](./examples/engine_physics_benchmark.c)
* **Integração Unreal Engine 5:** [`examples/combat_unreal.rts`](./examples/combat_unreal.rts)
* **Backend HTTP + SDK TypeScript:** [`examples/telemetry_server.rts`](./examples/telemetry_server.rts) → [`examples/telemetry_client_sdk.ts`](./examples/telemetry_client_sdk.ts)

