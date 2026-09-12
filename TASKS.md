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

### T018 — Investigar e planejar a multiprecisão u64

- Investigar `fractalExplorer_kotlin2026` e registrar os caminhos de
  multiprecisão direta e perturbação.
- Definir o plano para `Fixed<1>`, `Fixed<2>` e `Fixed<N>` com limbs `u64`.
- Trazer snippets, decisões de representação, riscos e estratégia de testes
  para `PLANO_IMPLEMENTACAO_MULTIPRECISAO.md`.
- Registrar que o fallback da perturbação será configurável e começará
  desabilitado por padrão para permitir a inspeção dos artefatos instáveis.
- **Critérios de aceitação:** investigação registrada; full multiprecision,
  órbita de referência e perturbação identificadas; plano TDD definido.
- **Evidências:** análise registrada em
  `PLANO_IMPLEMENTACAO_MULTIPRECISAO.md`; nenhum teste pesado executado nesta
  etapa.

## BACKLOG

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

### T016 — Substituir glyph bitmap temporário da barra de status

- Avaliar substituir o `glyph` bitmap implementado no renderer por uma biblioteca de fonte bitmap, um renderer com suporte nativo a texto ou widgets nativos da plataforma.
- Preservar a barra de status e sua atualização em tempo real durante a migração.
- Depende da estabilização da barra de status.

## DONE

### T001 — Desenhar um sprite na tela e exibi-lo

- **Resultado:** janela/renderizador Rust para Windows criado com `minifb`; sprite preenchido com o fractal de Mandelbrot CPU `f64`, centralizado em `(0, 0)`.
- **Evidências:** RED confirmou a ausência do módulo; GREEN passou com 5 testes; `cargo fmt -- --check` foi aplicado; `cargo test` passou com 5 testes; `cargo run --bin sprite-demo` compilou e iniciou sem erro.
- **Commit:** registrado e enviado ao remoto após a implementação.

### T002 — Separar cálculo, orquestração e renderização

- **Resultado:** `src/mandelbrot.rs` calcula somente as iterações de um ponto `(x, y)`; `src/orchestrator.rs` percorre largura/altura e produz `IterationBuffer`; `src/renderer.rs` converte as iterações em sprite e gerencia a janela `minifb`; `src/main.rs` apenas compõe esses módulos.
- **Evidências:** RED confirmou os módulos ausentes; GREEN passou com 7 testes; `cargo fmt` foi aplicado; `cargo test` passou com 7 testes.
- **Commit:** registrado e enviado ao remoto após a implementação.

### T017 — Corrigir o atalho do Codex no Windows

- **Resultado:** criado `scripts/launch-codex.ps1`, que resolve o repositório pela própria localização, posiciona o PowerShell nele e invoca `codex.exe`; o atalho da área de trabalho foi atualizado para usar `-File` com esse launcher.
- **Evidências:** RED confirmou o launcher ausente; GREEN passou com `tests/launch-codex.tests.ps1`; `cargo test` passou com 15 testes; `git diff --check` passou; os argumentos completos do `.lnk` foram lidos novamente sem truncamento. A abertura visual do atalho fica para validação manual do usuário.
- **Commit:** `72e7164`, registrado e enviado para `origin/master`.
