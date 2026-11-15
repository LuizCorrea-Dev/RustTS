# Extensão Oficial RustTS (`.rts`) para VS Code e Antigravity IDE (`v2.1.0`)

Pacote oficial de realce de sintaxe (TextMate), ícones de arquivo (`.png`/`.ico`), configuração de linguagem e snippets para a linguagem determinística **RustTS (`.rts`)** (`rtsc v2.1.0-rts`).

## Recursos Inclusos
- **Colorização Semântica Dedicada:** Destaque em alto contraste para o modificador `cap`, domínios de capacidades (`I2CBus`, `PWMPin`, `StdIO`, `FrameArena`, `ReadFrameArena`, `MutColumn`, `ReadColumn`, `MonotonicClock`, `TimeStep`), atributos de compilador (`@component`, `@system`, `@parallel_system`, `@sealed_sig`, `@open_sig`, `@unreal_actor`, `@gpu_kernel`, `@shader_shared`) e operador de propagação `?`.
- **Tipos e Estruturas:** Suporte a tipos primitivos, aliases ergonômicos TypeScript/C (`int`, `uint`, `float`, `double`, `number`, `boolean`, `string`, `byte`, `int8..int64`, `uint8..uint64`), vetoriais SIMD (`vec2`, `vec3`, `vec4`, `quat`, `mat3`, `mat4`), tipos algébricos (`Result<T, E>`, `Option<T>`, `Handle<T>`) e tipos nominais (`struct` e `enum`).
- **Snippets Integrados:**
  - `comp` → Gera um `@component struct` alinhado em 64 bytes SoA.
  - `sys` → Gera um `@system` determinístico com `MutColumn`, `FrameArena` e `TimeStep`.
  - `externc` → Declara uma função FFI `extern "C" fn` protegida por capacidade `cap`.
  - `gpukernel` → Gera um Compute Shader GPU (`@gpu_kernel`) com `@shader_shared struct` para WebGPU (`.wgsl`) e Vulkan/DX12 SPIR-V (`.spvasm`/`.spv`).

## Como Instalar no Visual Studio Code e no Antigravity IDE

> **Atenção (Windows):** **NÃO dê duplo-clique** no arquivo `.vsix` pelo Windows Explorer. O Windows associa arquivos `.vsix` ao `VSIXInstaller.exe` do *Microsoft Visual Studio Build Tools* (IDE C++/C#), que exibirá o erro `MissingPackagePartException`. Instale sempre por dentro do VS Code ou do Antigravity IDE usando um dos 3 métodos abaixo.

### Método A — Instalar pelo Arquivo `.vsix` (Recomendado)
1. Abra o **VS Code** ou o **Antigravity IDE**.
2. Pressione **`Ctrl + Shift + P`** (ou `Cmd + Shift + P` no macOS).
3. Digite **`Extensions: Install from VSIX...`** e pressione **Enter**.
4. Selecione o arquivo [`editors/rustts-lang-2.1.0.vsix`](../rustts-lang-2.1.0.vsix) deste repositório.
5. Pressione **`Ctrl + Shift + P`** e execute **`Developer: Reload Window`**.

### Método B — Carregar Diretamente da Pasta (`Install Extension from Location`)
1. No **VS Code** ou **Antigravity IDE**, pressione **`Ctrl + Shift + P`**.
2. Digite **`Developer: Install Extension from Location...`** e pressione **Enter**.
3. Selecione a pasta `editors/vscode-rustts` deste repositório.

### Método C — Instalação via Terminal (PowerShell / Bash)

**Windows (PowerShell — instala simultaneamente no Antigravity IDE e no VS Code):**
```powershell
foreach ($base in @("$env:USERPROFILE\.antigravity\extensions", "$env:USERPROFILE\.vscode\extensions")) {
    if (Test-Path (Split-Path $base -Parent)) {
        $dest = "$base\rootszera.rustts-lang-2.1.0"
        Remove-Item $dest -Recurse -Force -ErrorAction SilentlyContinue
        New-Item -ItemType Directory -Force -Path $dest | Out-Null
        Copy-Item -Path "editors\vscode-rustts\*" -Destination $dest -Recurse -Force
    }
}
```

**Linux / macOS (Bash / Zsh):**
```bash
mkdir -p ~/.vscode/extensions/rootszera.rustts-lang-2.1.0
cp -r editors/vscode-rustts/* ~/.vscode/extensions/rootszera.rustts-lang-2.1.0/

mkdir -p ~/.antigravity/extensions/rootszera.rustts-lang-2.1.0
cp -r editors/vscode-rustts/* ~/.antigravity/extensions/rootszera.rustts-lang-2.1.0/
```