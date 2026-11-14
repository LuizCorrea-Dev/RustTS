<p align="center">
  <img src="./rustts_language_logo.png" alt="RustTS Logo" width="220">
</p>

---

## Apêndice A: Matriz de Escopo Operacional e Domínios de Aplicação (`rtsc v1.5.0-rts` — Guia Oficial para Devs e IAs)

A **RustTS (`.rts`)** **não é uma linguagem de propósito geral ("canivete suíço")**. Ela foi projetada especificamente para sistemas determinísticos de alta performance, controle estrito de memória sem *Garbage Collector* e segurança baseada em capacidades (`cap`). 

Tanto desenvolvedores humanos quanto agentes de Inteligência Artificial (Gemini, Claude, GPT, DeepSeek, Qwen) devem consultar esta matriz antes de iniciar qualquer implementação para garantir que a linguagem seja aplicada exclusivamente onde sua arquitetura entrega vantagem técnica real.

---

### 1. Matriz de Classificação de Domínios

| Classificação | Domínio de Aplicação | Modelo de Memória (Arenas) | Capacidades Típicas (`cap`) | Alvo de Compilação (`rtsc`) | Status |
|---|---|---|---|---|---|
| **Tier 1 (Núcleo)** | **Motores de Jogos (2D / 2.5D / 3D) e Simulação** | `StaticArena`, `LevelArena`, `FrameArena` + ECS (`SoA`) | `MutColumn<T>`, `ReadColumn<T>`, `GpuQueue`, `AudioBuffer` | Nativo (LLVM / SIMD + SPIR-V / DXIL) ou Transpilação Unreal/Unity | **Nativo / Prioridade Máxima** |
| **Tier 1 (Núcleo)** | **Hardware, Embarcados e Sistemas Bare-Metal** | `StaticArena` + Mapeamento Direto (MMIO) | `I2CPort`, `SPIBus`, `GPIOPin`, `DMAChannel`, `Interrupt` | Nativo Bare-Metal (ARM / RISC-V / x86 via LLVM + C ABI) | **Nativo / Prioridade Máxima** |
| **Tier 1 (Núcleo)** | **Servidores de Alta Performance, Multiplayer e APIs** | `StaticArena`, `SessionArena`, `RequestArena` | `HttpRoute`, `TcpSocket`, `UdpTick`, `SqlRead<T>`, `SqlWrite<T>` | Nativo (Linux/Windows Server) ou Sandbox WASI Isolada | **Nativo / Prioridade Máxima** |
| **Tier 2 (Exceção)** | **Frontend Gráfico no Navegador (WebGPU / WebGL)** | `StaticArena`, `FrameArena` (Memória Linear WASM) | `CanvasContext`, `WebGpuDevice`, `InputState` | `wasm32-unknown-wasi` + Bindings Diretos WebGPU / WebGL2 | **Suportado (Exceção Estrita)** |
| **Tier 0 (Banido)** | **Frontend Web Tradicional (DOM, HTML, CSS, UI)** | *Incompatível (Exige GC e Árvore DOM Mutável)* | *Nenhuma (Não existem capacidades de DOM na linguagem)* | *Rejeitado pelo Compilador* | **Estritamente Proibido** |

---

### 2. Especificação Técnica das Áreas de Operação

#### A. Motores de Jogos (2D, 2.5D, 3D) e Integração com Engines
A RustTS opera como linguagem principal para construção de engines próprias do zero ou como gerador de módulos críticos de performance para motores comerciais:
* **Engines Próprias (Do Zero):** Utiliza *Data-Oriented Design* (DOD) obrigatório. Entidades são processadas em arrays contíguos (`SoA`) para máxima eficiência de cache L1/L2 e vetorização automática SIMD. Toda alocação dinâmica de ciclo curto ocorre na `FrameArena`, zerada ao fim de cada quadro em tempo constante $O(1)$.
* **Unreal Engine (`--target=unreal-cpp`):** Transpila arquivos `.rts` diretamente para pares `.h` e `.cpp` compatíveis com o *Unreal Header Tool* (UHT), gerando boilerplate de `UCLASS`, replicação de rede e *Gameplay Ability System* (GAS) sem alucinações sintáticas.
* **Unity (`--target=unity-burst` / `c-abi`):** Compila algoritmos pesados (raycasting, física, geração procedural, pathfinding) para bibliotecas dinâmicas nativas ou código C# estrito compatível com *Burst Compiler* e `NativeArray`.

#### B. Hardware, Sistemas Embarcados e Operações de Baixo Nível
Substitui C e C++ em contextos onde um ponteiro inválido ou acesso indevido gerado por IA causaria falha física ou travamento de kernel:
* **Zero Autoridade Ambiental:** Nenhuma função acessa registradores, portas seriais ou endereços físicos sem receber a respectiva capacidade (`cap`) na assinatura.
* **Previsibilidade de Tempo Real (RTOS / Bare-Metal):** Ausência total de *Garbage Collector*, exceções ocultas ou alocações implícitas no *heap*, garantindo tempo de execução determinístico por ciclo de clock.

#### C. Servidores, APIs Críticas e Backends Multiplayer
Aplica a mesma disciplina de tempo de quadro dos motores de jogos ao processamento de pacotes de rede e transações de dados:
* **Gerenciamento via `RequestArena`:** Cada requisição HTTP/gRPC ou pacote de rede recebe um bloco de memória linear dedicado. Ao enviar a resposta ao cliente, toda a memória daquela requisição é liberada instantaneamente movendo um único ponteiro (`offset = 0`), eliminando pausas de *Garbage Collection* e vazamentos de memória em servidores de longa duração.
* **Servidores Dedicados de Jogos (Tick-Rate Fixo):** Ideal para servidores autoritativos (ex: 60Hz ou 128Hz) que exigem simulação física determinística e serialização binária ultrarrápida.
* **Auditoria Estática de Segurança:** Uma rota de API que recebe apenas `cap SqlRead<Usuarios>` é fisicamente incapaz de executar um `DROP`, `DELETE` ou `UPDATE` no banco de dados, blindando o backend contra injeções ou erros lógicos de agentes de IA.

#### D. A Única Exceção de Frontend: WebGPU e WebGL via `<canvas>`
A RustTS **só pode ser executada no navegador** quando o navegador atua exclusivamente como um monitor de vídeo e leitor de periféricos (teclado, mouse, gamepad):
* **Escopo Permitido:** Jogos rodando no navegador, motores de renderização 2D/2.5D/3D para web, simuladores físicos e ferramentas gráficas baseadas em viewport (CAD, editores de nível).
* **Arquitetura:** O código `.rts` é compilado para WebAssembly (WASM) com memória linear pré-alocada e envia buffers binários de vértices, texturas e *shaders* diretamente para a GPU do usuário via **WebGPU** ou **WebGL**, renderizando tudo dentro de uma única tag HTML `<canvas>`.

---

### 3. Regra de Exclusão: Frontend Web Tradicional e Ponte TypeScript

É **estritamente proibido** criar bibliotecas, bindings ou abstrações em RustTS para manipular HTML, CSS, árvores DOM ou substituir frameworks reativos (como React, Next.js, Vue ou Angular).

#### Diretriz Obrigatória para Desenvolvedores e Agentes de IA:
Quando um projeto exigir uma aplicação completa (Backend + Interface Web tradicional com telas, formulários, tabelas e roteamento no navegador), a arquitetura **deve** ser dividida da seguinte forma:

1. **No Backend / Core (`.rts`):** Escreva toda a API, regras de negócio, validação, acesso a banco de dados e processamento pesado em **RustTS**.
2. **Na Ponte de Compilação (`--emit-ts-sdk`):** Execute o compilador `rtsc` com a flag de exportação de contrato (`-o` ou `--out-dir`):
   ```bash
   rtsc build src/server.rts --emit-ts-sdk --out-dir ./web-app/src/sdk/
   ```
3. **No Frontend Web (`.ts` / `.tsx`):** O compilador `rtsc` gerará automaticamente um cliente TypeScript 100% tipado (`client.ts`) refletindo todas as `struct`, `enum` e rotas do servidor RustTS. Utilize **TypeScript + Next.js / React + Tailwind CSS** para construir a interface visual consumindo esse SDK gerado.