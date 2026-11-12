# Guia de Inicialização e Prompt Mestre — Projeto RustTS

Este documento contém o protocolo de ativação para iniciar a construção prática do compilador **`rtsc`** e da linguagem **RustTS** em qualquer LLM de ponta (Gemini, Claude, GPT, DeepSeek, Qwen).

---

## Como Usar Este Pacote de Arquivos

1. Abra uma nova sessão no seu ambiente de IA ou agente de código (Cursor, Claude Code, Gemini Canvas, Windsurf ou API local).
2. Anexe ou cole no contexto da conversa os arquivos **`RustTS_guia.md`** e **`agenteRTS.md`**.
3. Copie e envie o **Prompt de Inicialização (Bloco 1)** abaixo para construir a base funcional do compilador.
4. Utilize os **Prompts de Expansão Modular (Bloco 2)** nas etapas seguintes conforme o projeto avança.

---

## Bloco 1: Prompt Mestre de Inicialização (Bootstrap do Compilador `rtsc`)

> Copie todo o texto dentro da caixa abaixo e envie como sua primeira mensagem após carregar os arquivos `RustTS_guia.md` e `agenteRTS.md`:

```text
Assuma integralmente a identidade, as competências e as restrições operacionais definidas no arquivo `agenteRTS.md` e utilize o arquivo `RustTS_guia.md` como a Constituição Técnica inviolável do nosso projeto.

Estamos iniciando oficialmente a construção do compilador `rtsc` (RustTS Compiler) escrito em Rust. Nosso objetivo no MVP (Fase 1) é criar um pipeline funcional ponta a ponta capaz de ler código `.rts`, validar capacidades (`cap`) e tipos, emitir diagnósticos em JSON para IAs e transpilar para código nativo (Rust/C99) e Unreal Engine C++.

Para começar agora no [MODO: COMPILER-DEV], execute as seguintes entregas estruturadas:

1. ARQUITETURA DO WORKSPACE CARGO:
   Defina a estrutura de diretórios e o `Cargo.toml` do compilador `rtsc`, mantendo dependências enxutas e de alta performance.

2. NÚCLEO DA AST E TOKENS (`ast.rs` e `lexer.rs`):
   Implemente em Rust a definição completa dos Tokens e dos nós da AST (Abstract Syntax Tree) da RustTS cobrindo:
   - Declarações de `module`, `struct`, `enum`, e atributos (`@component`, `@system`, `@unreal_actor`, `@gpu_kernel`).
   - Assinaturas de funções `fn` com suporte explícito ao modificador de capacidade `cap` nos parâmetros (ex: `i2c: cap I2CBus<0x48>`, `pos: cap MutColumn<Transform>`, `arena: cap FrameArena<Event>`).
   - Tipos primitivos, tipos matemáticos nativos (`vec2`, `vec3`, `vec4`, `mat4`), `Option<T>`, `Result<T, E>` e `Handle<T>`.

3. VERIFICADOR DE CAPACIDADES E DIAGNÓSTICOS JSON (`sema.rs` e `diagnostics.rs`):
   Implemente o protótipo funcional do analisador semântico que verifica se chamadas sensíveis dentro de uma função possuem o token `cap` correspondente declarado na assinatura, gerando a saída exata do protocolo `--json-diagnostics` com sugestões determinísticas de correção.

4. TESTE DE MESA COM CÓDIGO `.rts`:
   Forneça um caso de teste demonstrando o compilador aceitando uma função válida de física/hardware e bloqueando (com o respectivo JSON de erro) uma função onde uma IA tentou acessar um recurso não autorizado.
```

---

## Bloco 2: Prompts Modulares para as Etapas Seguintes

Após validar a base gerada pelo Bloco 1, utilize estes prompts sequenciais para construir cada subsistema da linguagem:

### Passo 2: Construir o Parser Completo e o Exportador GBNF (Anti-Alucinação)
```text
[MODO: COMPILER-DEV]
Com base na nossa AST definida anteriormente e nas regras do `RustTS_guia.md`, implemente agora:
1. O Parser completo (`parser.rs`) convertendo o fluxo de tokens em nós da AST com rastreamento exato de linha e coluna (`Span`).
2. O módulo `gbnf_exporter.rs` (comando `rtsc --export-gbnf`), que gera o arquivo de gramática formal `.gbnf` da RustTS pronto para ser usado em motores de Constrained Decoding (llama.cpp, vLLM, Structured Outputs), garantindo que qualquer LLM fique matematicamente impedido de cometer erros de sintaxe ao gerar código `.rts`.
```

### Passo 3: Construir o Backend de Engine Própria (Transpilador C99 / Rust + Arenas + ECS)
```text
[MODO: COMPILER-DEV]
Implemente agora o módulo `codegen_native.rs` (`rtsc --target native`) focado em criação de engines de jogos do zero e controle de hardware:
1. Implemente em código nativo o runtime mínimo de memória contendo `StaticArena`, `LevelArena`, `FrameArena` (com reset em O(1)) e `Handle<T>` com índice geracional.
2. Faça o transpilador converter estruturas `@component` em layouts contíguos Structure of Arrays (SoA) alinhados em 64 bytes para SIMD.
3. Faça o transpilador converter funções `@system` em loops otimizados e gere automaticamente o escalonador multithread (Job Graph) baseado nas assinaturas `cap ReadColumn<T>` e `cap MutColumn<T>`.
```

### Passo 4: Construir o Backend Unreal Engine 5 e Unity
```text
[MODO: COMPILER-DEV]
Implemente agora os módulos de integração com engines comerciais:
1. `codegen_unreal.rs` (`rtsc --target unreal`): Recebe uma AST contendo `@unreal_actor` e `@unreal_impl` e emite determinísticamente o par de arquivos `.h` e `.cpp` com todas as macros do Unreal Header Tool (`UCLASS`, `UPROPERTY`, `UFUNCTION`, `GENERATED_BODY`) e funções de replicação de rede prontas para compilar no Unreal Build Tool.
2. `codegen_unity.rs` (`rtsc --target unity`): Emite a interface C-ABI (`extern "C"`) para compilação de DLL nativa e gera automaticamente o arquivo `.cs` com bindings `DllImport` e suporte zero-copy via `NativeArray<T>`.
```