# RustTS v2.3.0-rts — Backend Godot 4 (GDExtension/SCons), HAL FPS/Áudio/Rede, Vulkan PBR RHI & Ferramentas Nativas

> **Tag:** `v2.3.0-rts` | **Compilador & Engine:** `rtsc 2.3.0-rts` | **Testes:** `80/80 PASS` (`cargo test`)

A **RustTS `v2.3.0-rts`** amplia o ecossistema de multi-compilação com o **backend completo para Godot Engine 4** (`--target=godot` e `--target=godot-module`), consolida a camada HAL nativa (entrada FPS, áudio multicanal/microfone, rede TCP/UDP, sistema de arquivos e decodificação PNG nos backends C99 e `--llvm`), introduz módulos utilitários em `src/engine/*` e promove conversores/empacotadores de assets diretamente para a CLI `rtsc`.

---

## Principais Novidades da `v2.3.0-rts`

### 1. Backend Godot Engine 4 (`--target=godot` & `--target=godot-module`)
- Novos atributos declarativos em [`src/ast.rs`](./src/ast.rs), [`src/lexer.rs`](./src/lexer.rs), [`src/parser.rs`](./src/parser.rs) e gramática GBNF ([`src/gbnf_exporter.rs`](./src/gbnf_exporter.rs)):
  - `@godot_node(base = "...", category = "...")`
  - `@godot_impl(StructName)`
  - `@gdexport` / `@gdexport(hint = "range", hint_string = "...")`
- Novo gerador [`src/codegen_godot.rs`](./src/codegen_godot.rs) seguindo a arquitetura oficial Godot 4:
  - **`--target=godot` (GDExtension C++17 / `godot-cpp`)**: gera classes `GDCLASS`, getters/setters e `ADD_PROPERTY(...)` no `_bind_methods()`, `register_types.h` / `register_types.cpp` (`GDREGISTER_CLASS`, `godot::GDExtensionBinding::InitObject`), script `SConstruct` e manifesto `.gdextension` (`compatibility_minimum = "4.2"`).
  - **`--target=godot-module` (Módulo Customizado SCons)**: gera headers internos da engine (`scene/3d/physics/character_body_3d.h`, `core/object/class_db.h`, `modules/register_module_types.h`), `ClassDB::register_class<T>()`, `SCsub` (`add_source_files`) e `config.py` para compilação integrada via `custom_modules`.
  - **Fallback automático**: módulos sem `@godot_node` expõem suas funções `pub fn` em uma classe `RefCounted` via `ClassDB::bind_static_method(...)`.
- Novo exemplo incluso: [`examples/combat_godot.rts`](./examples/combat_godot.rts).

### 2. Janela, Entrada FPS & Primitivas 2D (`WindowHost` / `InputDevice`)
- `WindowHost`: adicionados `open_windowed`, `set_fullscreen`, `set_cursor_locked`, `set_cursor_visible`, `draw_rect`, `draw_text_scaled` e `draw_blit_rgba` em C99 e `--llvm` (`src/sema.rs`, `src/codegen_native.rs`, `src/codegen_llvm.rs`).
- `InputDevice`: adicionados `mouse_dx`, `mouse_dy`, `mouse_wheel_delta` (com suporte a `WM_MOUSEWHEEL`), `mouse_right_down` e `mouse_right_clicked`.

### 3. FileSystem, Decodificador PNG Nativo & `core.mem` / `core.math`
- `FileSystem`: adicionados `exists`, `file_size`, `read_text`, `read_raw_slice`, `write_raw_slice`, `list_dir` e decodificador PNG nativo `load_image_rgba` em ambos os runtimes (C99 e LLVM IR com DEFLATE stored/fixed/dynamic Huffman + unfiltering).
- `core.mem` & `core.math`: adicionados `mem::f32_from_bits`/`to_bits` e `math::tan`, `asin`, `acos`, `atan`, `exp`, `log`, `pow`, `f32_from_bits`, `f32_to_bits`.

### 4. Memória & Sistema de Tipos (`StaticArena` em `rts.toml` & `@view struct`)
- **`StaticArena` configurável:** suporte à seção `[memory] static_arena_mb = <N>` em `rts.toml` (`read_static_arena_mb_from_rts_toml`, propagado para `emit_c99_with_arena_mb` e `emit_llvm_ir_with_arena_mb`), além de `arena.used_bytes()` e `arena.capacity_bytes()`.
- **`@view struct` & `MutSlice<Struct>`:** novo atributo `@view` permitindo encapsular `MutSlice<T>`/`ReadSlice<T>` em structs escopadas com bloqueio de escape via `RTS-E0501`, somado ao lowering completo de `MutSlice<Struct>` no C99 e `--llvm`.

### 5. Áudio Multicanal, Microfone (`AudioInput`) & Rede (`NetworkSocket`)
- **`AudioDevice` Mixer:** mixer PCM de 16 canais (`mixer_init`, `mixer_load_wav`, `mixer_play`, `mixer_set_channel`, `mixer_mix_frame`, `mixer_stop_channel`) com ganho, panning estéreo e filtro low-pass.
- **`AudioInput` & `NetworkSocket`:** novos domínios infalsificáveis de capacidade (`RTS-E0404` / `RTS-E0402`), incluindo leitura PCM de microfone com mute instantâneo (`open`, `set_muted`, `is_muted`, `read_pcm`, `close`) e sockets UDP/TCP não-bloqueantes via `ws2_32.dll` (`udp_bind`, `udp_send_to`, `udp_recv_from`, `tcp_connect`, `tcp_listen`, `tcp_accept`, `tcp_send`, `tcp_recv`, `close_socket`).

### 6. Vulkan RHI PBR
- Adicionados `PIPELINE_PBR_3D`, `rhi_cmd_bind_pbr_textures`, `rhi_cmd_bind_lights` e `rhi_cmd_bind_bones` em [`src/engine/gpu/rhi.rts`](./src/engine/gpu/rhi.rts) e [`src/engine/gpu/vulkan_defs.rts`](./src/engine/gpu/vulkan_defs.rts) com implementação C99 e stubs LLVM.

### 7. Novos Módulos da Stdlib / Engine (`src/engine/*`)
- [`src/engine/json.rts`](./src/engine/json.rts) — parser e escritor JSON zero-heap (`JsonWriter`, `json_get_i32`, `json_get_bool`, `json_find_key_value_offset`).
- [`src/engine/collections.rts`](./src/engine/collections.rts) — coleções sobre arena (`ArenaVecU32`, `ArenaVecF32`, `ArenaQueueU32`, `ArenaMapU32`).
- [`src/engine/threads.rts`](./src/engine/threads.rts) — particionamento paralelo determinístico (`ThreadRange`, `parallel_chunk_range`, `parallel_worker_count`).
- [`src/engine/zpmesh.rts`](./src/engine/zpmesh.rts) — leitor binário `.zpmesh` v1/v2 (`MAGIC_ZPM2`, `ZpmeshV2Header`, `ZpmeshVertex`, conversão ponto fixo Q16.16).
- [`src/engine/diagnostics.rts`](./src/engine/diagnostics.rts) — diagnóstico e registro JSON sob demanda (`DiagErrorInfo`, `error_registrar_se_houver`).

### 8. Novas Ferramentas Nativas na CLI `rtsc`
- **`rtsc bake-mesh <in.fbx> -o <out.zpmesh>`** ([`src/bake_mesh.rs`](./src/bake_mesh.rs)): parser nativo de Kaydara FBX Binary (7400/7500) com cálculo de tangentes e exportação binária `.zpmesh` v2 em ponto fixo Q16.16.
- **`rtsc bake-textures <input_dir> -o <output>`** ([`src/bake_textures.rs`](./src/bake_textures.rs)): varredura recursiva de texturas PNG PBR (`*_Color.png` + `*_AmbientOcclusion.png`), composição AO × Color e exportação para `.rts` / `.zptex`.
- **`rtsc check <file.rts> --verify-parity`**: validação conjunta do check estático e emissão simultânea C99 + LLVM IR.

### 9. Paridade C99 / `--llvm` & Diagnósticos
- Diretivas `#line` multi-arquivo com `FnDecl.source_file`, correções de `-Wsign-compare`, `pub @attr ...`, paridade booleana/mista em `codegen_llvm.rs`, decodificação real de sequências de escape em literais string no pool LLVM (`unescape_rts_string`) e suíte ampliada para **`80/80 PASS`** (`test_70`–`test_76`).

---

## Como Instalar e Testar

```bash
git clone https://github.com/LuizCorrea-Dev/RustTS.git
cd RustTS
cargo install --path .
rtsc --version
# rtsc 2.3.0-rts

cargo test
# 80 passed; 0 failed

# Verificação estática e de paridade C99 / LLVM IR
rtsc check src/main.rts --verify-parity

# Gerar GDExtension C++17 (godot-cpp + SConstruct + .gdextension) ou módulo Godot 4
rtsc build examples/combat_godot.rts --target=godot --out-dir build/godot
rtsc build examples/combat_godot.rts --target=godot-module --out-dir build/godot_module

# Pipeline nativo de meshes e texturas PBR
rtsc bake-mesh assets/model.fbx -o assets/model.zpmesh
rtsc bake-textures assets/textures -o build/baked_textures.rts
```
