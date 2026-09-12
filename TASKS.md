# Lista de tarefas

Fila de trabalho do rewrite do FractalExplorer em Rust. A ordem de `TODO` é deliberada: agentes devem atuar sempre na primeira tarefa e nunca puxar itens diretamente de `BACKLOG`.

## Decisões de arquitetura

- **Fractal inicial:** Mandelbrot.
- **Orquestrador:** Rust.
- **Processador:** arquitetura plugável e multiplataforma.
- **Primeiro processador:** CPU Rust usando `f64`.
- **Renderizador:** arquitetura plugável e multiplataforma.
- **Primeiro renderizador:** biblioteca Rust para Windows usando sprites.
- **Processadores no backlog:** GPGPU com OpenCL e GPGPU com Metal.
- **Renderizadores no backlog:** WebGL, macOS, Android e iOS.

Estas decisões definem a primeira fatia vertical, mas não antecipam a implementação dos itens em `BACKLOG`.

## TODO

_Nenhuma tarefa ativa._
- **Progresso:** implementação inicial criada em `src/lib.rs` e `src/main.rs`; RED/GREEN, refactor e verificação visual estão pendentes porque `cargo`/`rustc` não estão disponíveis neste ambiente.

## BACKLOG

### T002 — Extrair e estabilizar o workspace Rust e os módulos do core

- Definir a estrutura de crates/módulos compartilhados e comandos de build/teste.
- Depende de T001.

### T003 — Ampliar o processador CPU determinístico

- Expandir o processador CPU `f64` inicial para uma representação independente de plataforma.
- Incluir cancelamento e comportamento determinístico para permitir conformance tests.
- Depende de T001 e T002.

### T004 — Implementar câmera e navegação com zoom centrado no cursor

- Cobrir pan, zoom, limites e conversão tela/plano com testes.
- Depende de T001 e T002.

### T005 — Implementar paletas e coloração interpolada

- Preservar os comportamentos úteis identificados nas referências legadas.
- Depende de T001 e T003.

### T006 — Definir e implementar o agendador de tiles/progressive rendering

- Separar cálculo, retenção e desenho; cobrir prioridade, cancelamento e ausência de seams.
- Depende de T003.

### T007 — Criar uma suíte de conformidade entre implementações/backend

- Comparar resultados CPU e futuros backends em pontos e imagens representativos.
- Depende de T003, T005 e T006.

### T008 — Adicionar processador GPGPU com OpenCL

- Implementar OpenCL como processador plugável e comparar sua saída com a referência CPU.
- Depende de T007.

### T009 — Adicionar processador GPGPU com Metal

- Implementar Metal como processador plugável e comparar sua saída com a referência CPU.
- Depende de T007.

### T010 — Adicionar renderizador WebGL

- Integrar o contrato de renderização plugável ao alvo WebGL.
- Depende de T007.

### T011 — Adicionar renderizador macOS

- Implementar o alvo macOS respeitando o contrato plugável.
- Depende de T007.

### T012 — Adicionar renderizador Android

- Implementar o alvo Android e validar ciclo de vida e recuperação de recursos.
- Depende de T007.

### T013 — Adicionar renderizador iOS

- Implementar o alvo iOS respeitando o contrato plugável.
- Depende de T007.

### T014 — Investigar deep zoom e precisão arbitrária

- Avaliar aritmética de precisão estendida, perturbation e aceleração; não assumir que os wrappers legados já resolvem o problema.
- Depende de T001 e T007.

### T015 — Adicionar persistência, importação e exportação

- Locais/configurações salvos, importação de coordenadas e exportação de imagem/vídeo.
- Depende de T004 e T010.

## DONE

### T001 — Desenhar um sprite na tela e exibi-lo

- **Resultado:** janela/renderizador Rust para Windows criado com `minifb`; sprite preenchido com o fractal de Mandelbrot CPU `f64`, centralizado em `(0, 0)`.
- **Evidências:** RED confirmou a ausência do módulo; GREEN passou com 5 testes; `cargo fmt -- --check` foi aplicado; `cargo test` passou com 5 testes; `cargo run --bin sprite-demo` compilou e iniciou sem erro.
- **Commit:** registrado e enviado ao remoto após a implementação.
