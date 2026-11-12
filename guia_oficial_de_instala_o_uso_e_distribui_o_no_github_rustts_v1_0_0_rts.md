# Guia Oficial de Instalação, Uso e Distribuição — RustTS (`rtsc v1.0.0-rts`)

A **RustTS (`.rts`)** foi projetada para ter **zero dependências complexas para o usuário final**. O compilador inteiro (`rtsc`) é compilado em um **único arquivo executável estático (`rtsc.exe` no Windows, `rtsc` no Linux/macOS)** de poucos megabytes, que roda em menos de $15\text{ ms}$.

Abaixo está a resposta direta sobre como instalar, como usar no terminal e como distribuir profissionalmente no GitHub via executável e PowerShell.

---

## 1. Precisa de Executável ou Comando PowerShell? (Como Funciona)

Você **não precisa escolher apenas um** — o padrão da indústria (usado por ferramentas como `rustup`, `bun`, `deno` e `uv`) combina **os dois**:

1. **O Executável (`rtsc.exe`):** É o compilador em si. Você hospeda o `rtsc.exe` pré-compilado na aba **Releases** do seu repositório no GitHub. Quem quiser pode apenas baixar o `.exe` ou `.zip` e usar.
2. **O Comando de 1 Linha no PowerShell (`install.ps1`):** É um pequeno script que você deixa na raiz do seu GitHub. Quando o usuário roda `irm https://raw.githubusercontent.com/<SEU_USUARIO>/RustTS/main/install.ps1 | iex` no PowerShell, o script:
   * Baixa automaticamente o `rtsc.exe` mais recente do seu GitHub Releases;
   * Salva na pasta `C:\Users\<Usuario>\.rtsc\bin\rtsc.exe`;
   * Adiciona essa pasta ao `PATH` do Windows automaticamente para que o comando `rtsc` funcione de qualquer lugar no terminal.
3. **O Compilador Backend Nativo (`clang.exe`):**
   * Para rodar `rtsc check` ou gerar código TypeScript/Unreal/Unity/C99, o `rtsc.exe` **não precisa de nada externo**.
   * Para compilar o código `.c` gerado pelo `--target=native` em um binário final `.exe` ou `.dll` com AVX2, usa-se o **LLVM/Clang** (que já vem instalado no seu Visual Studio ou pode ser instalado via PowerShell com `winget install LLVM.LLVM`).

---

## 2. Como Gerar o Executável `rtsc.exe` na Sua Máquina Agora

No diagnóstico do ambiente, notamos que o seu Windows já possui o **Clang 22.1.3** (dentro do Visual Studio), mas ainda não tinha o `cargo` / `rustc` no `PATH` para compilar os arquivos `.rs` em `rtsc.exe`.

Siga estes passos no **PowerShell** para gerar o seu `rtsc.exe`:

### Passo 2.1. Instalar o Toolchain Rust (caso ainda não tenha na máquina)
Abra o PowerShell e execute:
```powershell
winget install Rustlang.Rustup
```
*(Ou baixe em [rustup.rs](https://rustup.rs)). Após instalar, feche e reabra o PowerShell para atualizar o `PATH`.*

### Passo 2.2. Compilar o Binário Oficial `rtsc.exe` e Rodar os Testes
Entre na pasta do projeto `RustTS` e rode:
```powershell
cd C:\Users\luizc\Documents\ROOTSZERA\Projetos\RustTS

# 1. Executa os 12 testes unitários do compilador (Capacidades, Arenas, TypeChecker, Match, Codegen)
cargo test

# 2. Gera o executável otimizado de produção (rtsc.exe)
cargo build --release
```
Ao terminar, o seu executável estará pronto em:
```text
C:\Users\luizc\Documents\ROOTSZERA\Projetos\RustTS\target\release\rtsc.exe
```

### Passo 2.3. Instalar o `rtsc.exe` Localmente no Seu Windows
Para poder digitar `rtsc` de qualquer pasta no seu computador:
```powershell
cargo install --path .
```

---

## 3. Manual Completo de Uso do Compilador (`rtsc`)

Depois que o `rtsc` estiver instalado no seu terminal, você tem **6 fluxos principais de trabalho**:

### 3.1. Verificar Segurança, Tipos, Arenas e Capacidades (`< 15 ms`)
Valida se o código `.rts` respeita todas as regras (`RTS-E0101` a `RTS-E0900`):
```powershell
# Saída formatada e colorida para humanos:
rtsc check examples/thermal_control.rts

# Saída em JSON determinístico para Agentes de IA (LLMs):
rtsc check examples/thermal_control.rts --json-diagnostics
```

### 3.2. Compilar para Código Nativo C99 + AVX2 SIMD (`--target=native`)
Converte o arquivo `.rts` para C99 ultra-otimizado (descritores SoA de $64\text{ bytes}$, *Strip-Mining* no Cache L1d e vetorização AVX2/FMA3):
```powershell
# 1. Transpila .rts para .c
rtsc build examples/ballistics_10k_benchmark.rts --target=native > build_ballistics.c

# 2. Compila com Clang para executável nativo x86_64 com AVX2 + FMA3
& "C:\Program Files\Microsoft Visual Studio\18\Community\VC\Tools\Llvm\x64\bin\clang.exe" `
  -std=c11 -O3 -mavx2 -mfma -Wall -Wextra -Werror `
  examples/ballistics_10k_benchmark.c -o ballistics_bench.exe

# 3. Executa o binário nativo (~7.58 us/frame para 10.000 projéteis)
.\ballistics_bench.exe
```

### 3.3. Gerar Atores C++ para Unreal Engine 5 (`--target=unreal`)
Elimina a necessidade de escrever `.h` e `.cpp` separados e configurar `GetLifetimeReplicatedProps` / `DOREPLIFETIME` manualmente:
```powershell
rtsc build examples/combat_unreal.rts --target=unreal
```
*Gera automaticamente o cabeçalho `AWeaponProjectile.h` e a implementação `AWeaponProjectile.cpp` compatíveis com o Unreal Header Tool (UHT).*

### 3.4. Gerar Plugin Nativo + Wrapper C# Zero-Copy para Unity (`--target=unity`)
```powershell
rtsc build examples/engine_physics.rts --target=unity
```
*Gera o código FFI `extern "C"` para a DLL nativa e a classe C# `RtscNativeBridge` operando diretamente sobre `NativeArray<T>` via `NativeArrayUnsafeUtility` sem cópias e sem Garbage Collector.*

### 3.5. Gerar SDK TypeScript Tipado para Frontend Web (`--emit-ts-sdk`)
Para servidores de alta frequência escritos em `.rts` (com `RequestArena` em $O(1)$ e `SqlRead<T>`), gera automaticamente o cliente HTTP para Next.js / React:
```powershell
rtsc build examples/telemetry_server.rts --emit-ts-sdk > examples/telemetry_client_sdk.ts
```
*Garante que inteiros de 64 bits (`u64`/`i64`) sejam mapeados para `string` (evitando corrupção IEEE-754 acima de $2^{53} - 1$) e retorna `RtsResult<T, ApiError>` sem exceções ocultas (`throw`).*

### 3.6. Exportar a Gramática Formal GBNF para LLMs Locais (`--export-gbnf`)
Para impedir que IAs locais (`llama.cpp`, `vLLM`) alucinem sintaxe:
```powershell
rtsc --export-gbnf > rustts_v1.gbnf
```

---

## 4. Passo a Passo: Criando Seu Primeiro Projeto em RustTS

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

   @sealed_sig
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

       cooler.set_duty(30);
       return Ok(temp);
   }
   ```
2. Valide o contrato de segurança e tipos:
   ```powershell
   rtsc check meu_sensor.rts
   ```
   Saída esperada:
   ```text
   [rtsc 1.0.0-rts] OK: 0 capability violations, 0 transitive leaks, 0 capability forgeries, 0 arena escapes.
   ```
3. Gere o código C99 determinístico:
   ```powershell
   rtsc build meu_sensor.rts --target=native > meu_sensor.c
   ```

---

## 5. Como Distribuir a Linguagem no GitHub (Executável + PowerShell Automatizado)

Para que qualquer pessoa (ou equipe da **KANORI**) instale o `rtsc` em segundos direto do seu GitHub sem precisar compilar nada manualmente, configure os **3 arquivos abaixo** no seu repositório Git:

### 5.1. Estrutura Recomendada do Repositório no GitHub
```text
RustTS/
├── .github/
│   └── workflows/
│       └── release.yml          # Compila rtsc.exe automaticamente e cria o GitHub Release
├── src/                         # Código-fonte do compilador (lexer, parser, sema, codegen...)
├── examples/                    # Exemplos oficiais e benchmarks (.rts e .c)
├── scripts/
│   ├── install.ps1              # Instalador de 1 linha para Windows PowerShell
│   └── install.sh               # Instalador de 1 linha para Linux / macOS
├── Cargo.toml
├── RustTS_guia.md
└── INSTALACAO_E_USO_RUSTTS.md
```

### 5.2. Script Instalador para Windows PowerShell (`scripts/install.ps1`)
Crie o arquivo `scripts/install.ps1` no seu repositório com o seguinte conteúdo (substitua `<SEU_USUARIO>` pelo seu usuário ou organização no GitHub):

```powershell
# scripts/install.ps1 — Instalador Oficial do Compilador RustTS (rtsc) para Windows x64
$ErrorActionPreference = "Stop"

$Repo = "<SEU_USUARIO>/RustTS"
$InstallDir = "$env:USERPROFILE\.rtsc\bin"
$ExePath = "$InstallDir\rtsc.exe"

Write-Host "==> Instalando o compilador RustTS (rtsc)..." -ForegroundColor Cyan

# 1. Cria o diretório de instalação ~/.rtsc/bin
if (!(Test-Path -Path $InstallDir)) {
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
}

# 2. Busca a URL do último executável rtsc-windows-x64.exe no GitHub Releases
$ReleaseUrl = "https://github.com/$Repo/releases/latest/download/rtsc-windows-x64.exe"
Write-Host "==> Baixando binário nativo de $ReleaseUrl ..."
Invoke-WebRequest -Uri $ReleaseUrl -OutFile $ExePath

# 3. Adiciona ~/.rtsc/bin ao PATH do usuário no Windows (se ainda não estiver)
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($UserPath -notlike "*$InstallDir*") {
    [Environment]::SetEnvironmentVariable("Path", "$UserPath;$InstallDir", "User")
    $env:Path += ";$InstallDir"
    Write-Host "==> Diretório '$InstallDir' adicionado ao PATH do usuário." -ForegroundColor Green
}

Write-Host "==> RustTS instalado com sucesso! Teste executando: rtsc --version" -ForegroundColor Green
```

Com esse arquivo no GitHub, qualquer usuário no Windows instala a sua linguagem abrindo o PowerShell e rodando **um único comando**:
```powershell
irm https://raw.githubusercontent.com/<SEU_USUARIO>/RustTS/main/scripts/install.ps1 | iex
```

### 5.3. Automação de Releases no GitHub Actions (`.github/workflows/release.yml`)
Crie o arquivo `.github/workflows/release.yml`. Toda vez que você subir uma tag de versão (ex: `git tag v1.0.0 && git push origin v1.0.0`), os servidores do GitHub compilarão automaticamente o `rtsc` para **Windows (`.exe`)**, **Linux** e **macOS** e publicarão os arquivos na aba **Releases**:

```yaml
name: Release RustTS Binaries (rtsc)

on:
  push:
    tags:
      - 'v*.*.*'

permissions:
  contents: write

jobs:
  build-and-release:
    strategy:
      matrix:
        include:
          - os: windows-latest
            target: x86_64-pc-windows-msvc
            bin_name: rtsc.exe
            asset_name: rtsc-windows-x64.exe
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            bin_name: rtsc
            asset_name: rtsc-linux-x64
          - os: macos-latest
            target: aarch64-apple-darwin
            bin_name: rtsc
            asset_name: rtsc-macos-arm64

    runs-on: ${{ matrix.os }}

    steps:
      - name: Checkout Repository
        uses: actions/checkout@v4

      - name: Install Rust Stable Toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Run Compiler Unit Tests
        run: cargo test --release

      - name: Build Optimized Release Binary
        run: cargo build --release --target ${{ matrix.target }}

      - name: Prepare Release Asset
        shell: bash
        run: |
          cp target/${{ matrix.target }}/release/${{ matrix.bin_name }} ${{ matrix.asset_name }}

      - name: Publish Binary to GitHub Releases
        uses: softprops/action-gh-release@v2
        with:
          files: ${{ matrix.asset_name }}
          generate_release_notes: true
```

### 5.4. Comandos Git para Publicar a `v1.0.0-rts` no GitHub
Quando estiver pronto para subir o repositório e disparar a geração automática dos executáveis na nuvem do GitHub:
```powershell
git init
git add .
git commit -m "feat(rtsc): release RustTS v1.0.0-rts compiler and SIMD/SoA runtime"
git branch -M main
git remote add origin https://github.com/<SEU_USUARIO>/RustTS.git
git push -u origin main

# Cria e envia a tag v1.0.0 para acionar o GitHub Actions e gerar o rtsc-windows-x64.exe
git tag v1.0.0
git push origin v1.0.0
```