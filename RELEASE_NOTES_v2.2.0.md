# RustTS v2.2.0-rts — Motor Gráfico Nativo 2D/3D Vulkan + Ponte Linguagem→GPU

> **Tag:** `2.2.0` (`v2.2.0`) | **Compilador & Engine:** `rtsc 2.2.0-rts` | **Testes:** `74/74 PASS` (`cargo test`)

A **RustTS `v2.2.0-rts`** unifica a linguagem de sistemas determinística (`.rts`) com um **motor gráfico 2D e 3D nativo em Vulkan 1.3** (`src/engine/*`), eliminando a necessidade de código C externo para renderizar jogos, texturas, áudio e shaders escritos diretamente na linguagem.

---

## Principais Novidades da `v2.2.0-rts`

### 1. Motor 2D Real (`SpriteBatch2D` + Câmera Ortográfica)
- Novo módulo [`src/engine/renderer_2d.rts`](./src/engine/renderer_2d.rts) com `SpriteBatch2D`, `sprite2d_begin`, `sprite2d_draw` (rotação em torno do centro e animação de sprite sheets via `sprite_anim`), `sprite2d_draw_rect`, `sprite2d_flush` e `sprite2d_set_texture` (auto-flush automático ao trocar de textura no frame).
- Pipeline Vulkan `PIPELINE_SPRITE_2D` (`4`) integrado à câmera ortográfica (`MODE_ORTHO_2D` + `PROJ_ORTHOGRAPHIC`).

### 2. Motor 3D Genérico (`Renderer3D` + Loader `.OBJ`)
- Novo módulo [`src/engine/renderer_3d.rts`](./src/engine/renderer_3d.rts) encapsulando `Renderer3D` (`r3d_init`, `r3d_begin`, `r3d_add_box`, `r3d_flush`, `r3d_submit_mesh_range`) sobre `VramArena` ($O(1)$).
- Loader nativo Wavefront `.OBJ` (`mesh_load_obj` / `rtsc_mesh_load_obj`) exposto diretamente no módulo [`src/engine/mesh.rts`](./src/engine/mesh.rts).

### 3. Texturas Reais na GPU (PNG / BMP / TGA via Staging DMA)
- Decodificador nativo C99 para **PNG** (zlib/deflate stored/fixed/dynamic Huffman + unfiltering `None/Sub/Up/Avg/Paeth`), **BMP** e **TGA** em `rtsc_vk_load_texture`, além de upload procedural `rhi_texture_create_rgba`.
- Criação real de `VkImage` (`DEVICE_LOCAL`), Staging Buffer DMA (`vkCmdCopyBufferToImage`), barreiras de layout, `VkSampler` e `VkDescriptorSet` vinculados via `rhi_cmd_bind_texture`.

### 4. Ponte Linguagem → GPU (Shaders Gráficos em `.rts`)
- Novos atributos `@vertex_shader` e `@fragment_shader` em `src/ast.rs`, `src/lexer.rs`, `src/parser.rs`, `src/sema.rs`, `src/gbnf_exporter.rs` e `src/codegen_gpu.rs`.
- Compilação direta de shaders `.rts` para **SPIR-V 1.5** (e WGSL) via CLI:
  ```bash
  rtsc build examples/shaders/custom_tri.rts --target spirv-vert --bin -o build/custom_vs.spv
  rtsc build examples/shaders/custom_tri.rts --target spirv-frag --bin -o build/custom_fs.spv
  ```
- Suporte a `PushConstant` (`Mat4` / `mvp` + `mul(mvp, pos)`), parâmetros vetoriais (`Vec2`/`Vec3`/`Vec4`/`Quat`), `Sampler2D` (`DescriptorSet 0, Binding 0` + `sample(tex, uv)`) e criação de pipelines em runtime via `rhi::rhi_create_graphics_pipeline`.

### 5. Plataforma, Input Completo, Relógio Real & Áudio
- [`src/engine/platform/window.rts`](./src/engine/platform/window.rts): teclas `KEY_TAB_BIT`, `KEY_SPACE_BIT`, `KEY_SHIFT_BIT`, `KEY_ESC_BIT`, `KEY_UP_BIT`, `KEY_DOWN_BIT`, `KEY_LEFT_BIT`, `KEY_RIGHT_BIT`, `KEY_ENTER_BIT` e cronômetro de alta precisão `rtsc_time_now_sec()`.
- [`src/engine/audio.rts`](./src/engine/audio.rts): playback assíncrono de áudio WAV (`audio_play_wav`, `audio_stop_all`) via `winmm.dll`.

### 6. Exemplos Inclusos & Limpeza do Repositório
- [`examples/demo_2d.rts`](./examples/demo_2d.rts) — Demo 2D com `SpriteBatch2D`, plataformas, animação de sprites e movimento.
- [`examples/demo_3d.rts`](./examples/demo_3d.rts) — Demo 3D desacoplado com `Renderer3D`, câmera orbital e `rtsc_time_now_sec()`.
- [`examples/demo_shader.rts`](./examples/demo_shader.rts) + [`examples/shaders/custom_tri.rts`](./examples/shaders/custom_tri.rts) — Shaders compilados em `.rts` carregados em tempo de execução no Vulkan.
- [`examples/village_editor.rts`](./examples/village_editor.rts) — Editor visual 3D completo com instancing, loader `.OBJ` e persistência binária `.pak` (movido para fora de `src/main.rts`, que agora atua como template limpo da Engine).

---

## Como Instalar e Testar

```bash
git clone https://github.com/LuizCorrea-Dev/RustTS.git
cd RustTS
cargo install --path .
rtsc --version
# rtsc 2.2.0-rts

cargo test
# 74 passed; 0 failed

rtsc run examples/demo_2d.rts
rtsc run examples/demo_3d.rts
rtsc build examples/shaders/custom_tri.rts --target spirv-vert --bin -o build/custom_vs.spv
rtsc build examples/shaders/custom_tri.rts --target spirv-frag --bin -o build/custom_fs.spv
rtsc run examples/demo_shader.rts
```
