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

### T029 — Renderização modular com destinos plugáveis

- **Objetivo:** transformar o spike GPU validado em apenas mais um destino de
  renderização, preservando um único caminho comum para configuração, janela,
  input, canvas, tiles, overlays, instrumentação e encerramento.
- **Plano:** ver [plano de implementação de T029](task-descriptions/T029-gpu-textures-shaders.md).
- **Dependências:** T003, T005, T006 e T007; a primeira fatia pode reutilizar o
  processador CPU e os tiles atuais, migrando inicialmente apenas a composição.
- **Decisão arquitetural:** `RenderTarget` (apresentação) e `TileProcessor`
  (cálculo CPU/GPGPU) serão plugins independentes. Recursos gráficos serão
  encapsulados por handles, descritores, command lists e `GraphicsDevice`, sem
  tipos de `wgpu`, `winit`, `minifb`, Metal ou OpenCL no domínio.
- **Critérios de aceitação:** um único `ApplicationController` e `FrameBuilder`;
  CPU/GPU selecionáveis por factory; equivalência dentro de tolerância
  documentada; recursos comuns independentes do destino; buffers e texturas
  persistentes; renderer legado como fallback; Metal e GPGPU adicionáveis sem
  modificar canvas, UI, overlays ou instrumentação; testes GREEN, refactor,
  commits focados e push confirmados.
- **Passo 0 concluído:** a caracterização determinística cobre viewport
  reduzida, envelopes, ordem de camadas, tiles progressivos, overlays, input,
  resize, configuração e encerramento. O golden data do `PreparedTileBatch`
  preserva identidade, ordem e destino no `RenderFrame`; RED confirmou a
  ausência da conversão isolada e GREEN passou após a extração de
  `frame_tiles_from_batch`. `cargo fmt`, `git diff --check` e `cargo test
  --lib` passaram com 160 testes. Não foi necessária validação HILT.
- **Passo 1 concluído:** criado `PreparedFrame`, que mantém o `RenderFrame`
  lógico separado dos `ImageUpdate` em um snapshot imutável; os caminhos GPU e
  CPU publicam e consomem o mesmo contrato. No CPU, a conversão de sprites
  preserva identidade, revisão, ordem, destino e payload RGBA, e o rasterizador
  usa os destinos do snapshot. RED confirmou o tipo ausente e depois a unidade
  de preparação CPU ausente; GREEN passou após a implementação e a migração.
  `cargo fmt`, `git diff --check` e `cargo test --lib` passaram com 162 testes.
  Não foi necessária validação HILT.
- **Passo 2 concluído:** o `RenderTargetPipeline` agora centraliza a
  sequência de resize, uploads, render, eviction e recovery, coberta por fake
  determinística. O `CpuRenderTarget` passou a preservar a semântica de
  framebuffer anterior, cor de fundo e o loop CPU já usa resize/update/render
  pelo contrato comum. RED/GREEN cobriram a preservação de pixels; `cargo fmt`,
  `git diff --check` e `cargo test --lib` passaram com 164 testes. Não foi
  necessária validação HILT até aqui.
- **Fechamento do passo 2:** `RendererFactory` passou a concentrar a seleção
  CPU/GPU, o `main` consulta a factory em vez de comparar o enum de backend e
  o runner CPU recebe a construção do `CpuRenderTarget` pela factory. A
  factory CPU também é a implementação usada pelo contrato
  `RenderTargetFactory`, mantendo o fallback legado no mesmo ponto de
  composição. RED/GREEN: os testes de seleção CPU/GPU passaram; REFACTOR:
  `CpuRenderTargetFactory::create_cpu_target` eliminou a construção duplicada.
  `cargo fmt --all -- --check`, `git diff --check` e `cargo test --lib`
  passaram com 176 testes. A alteração não muda comportamento visual nem
  exige validação HILT.
- **Passo 3 concluido:** o controlador agora expoe uma fronteira unica para
  publicar e preparar `PreparedFrame`, usada pelos runtimes CPU e GPU. Tambem
  centraliza o ciclo comum de canvas em `prepare_canvas_common`; `GpuWindowApp`
  e o loop CPU deixam de duplicar begin frame, trim, cobertura e agendamento
  das camadas. A composicao de imagens continua especifica do adaptador.
  Shared canvas tile collection, sprite generation, stable identity, and logical
  frame descriptions are now consumed by CPU and GPU. The controller builds
  sequential PreparedFrame snapshots and owns shared pan/zoom behavior and
  semantic layer, queue, and worker overlay data. The adapters still rasterize
  those overlays. RED/GREEN: frame, input, and overlay-switch tests; cargo test
  --lib passed (180 tests), cargo check --bin sprite-demo, cargo fmt --all
  -- --check, and git diff --check passed. Visual HILT was confirmed by the
  user before continuing.
- Registro anterior do passo 3: `ApplicationController::prepare_frame` passou a
  retornar `PreparedFrame`, alinhando a API comum ao snapshot usado por CPU e
  GPU. RED confirmou que o controlador ainda retornava `RenderFrame`; GREEN
  passou após encapsular frame e updates, com 2 testes do controlador. O
  `publish_frame` agora recebe o snapshot do runtime GPU e o invalida em
  resize, também coberto por RED/GREEN. O controlador ainda precisa receber o
  canvas/scheduler e produzir os tiles e overlays reais; o loop CPU agora
  também publica e consome o snapshot pelo controlador, e resize, input,
  configuração e encerramento passam pela mesma normalização de eventos. O
  runtime GPU agora drena atualizações de configuração por canal e sinaliza o
  encerramento ao Config UI, ambos sem acesso à janela nos testes. O passo não
  foi concluído. O canal agora é criado por `gpu_window::runtime_channels`,
  conectado ao `main` e consumido por `GpuWindowApp`; o sinal de encerramento
  também é compartilhado com a Config UI. RED/GREEN: o teste
  `gpu_window::tests::runtime_channels_connect_config_updates_and_shutdown_signal`
  passou, `cargo check --bin sprite-demo` passou e `cargo test --lib` passou com
  168 testes; `cargo fmt --all` e `git diff --check` também passaram. O próximo
  gate — validar resize, configuração, overlays e shutdown numa janela GPU
  real — requer validação HILT.
- **HILT e correção do runtime:** a validação manual reproduziu o panic do
  `winit` quando o `EventLoop` era criado na thread GPU secundária no Windows.
  GREEN: o entrypoint GPU voltou a executar o event loop na thread principal;
  `cargo test --lib` passou com 170 testes e `cargo check --bin sprite-demo`
  passou. A Config UI não é aberta simultaneamente pelo entrypoint GPU até a
  integração ser migrada para um runtime único. Essa integração foi movida para
  o backlog como T035; criar outro event loop em thread secundária teria a
  mesma restrição de plataforma.
- **Passo 4 — adaptador de ciclo de vida concluído no escopo T029:** a tradução
  dos eventos de ciclo de vida do `winit`
  (`resize`, `redraw` e fechamento) foi extraída para
  `app_event_from_window_event`, deixando o adaptador GPU encaminhar eventos ao
  `ApplicationController` por uma fronteira pura e testável. RED confirmou a
  ausência do tradutor; GREEN passou com o teste
  `gpu_window::tests::translates_window_lifecycle_events_to_application_events`.
  `cargo fmt --all`, `git diff --check` e `cargo test --lib` passaram com 171
  testes. A redução de `AppEffect` para ações do runtime também foi extraída;
  agora resize, input e configuração encaminham `RequestRedraw` ao adaptador,
  em vez de descartar o efeito. RED/GREEN cobriram
  `app::tests::reduces_controller_effects_to_runtime_actions` e a suíte passou
  com 172 testes. Registro intermediário: o ciclo de redraw e o encerramento
  foram concluídos e validados nas atualizações abaixo.
- **Passo 4 — ciclo de redraw:** RED confirmou que `AppEffect::Render` era
  descartado por `reduce_effects`; GREEN preservou a ação e condicionou o
  caminho GPU de `RedrawRequested` à decisão do controlador. O callback
  `about_to_wait` agora também é representado por `AppEvent::AboutToWait`, com
  ações explícitas `PrepareFrame` e `RequestRedraw`; após `CloseRequested`, o
  controlador não agenda outro frame, evitando a preparação tardia que podia
  falhar com `InvalidFrame("application is closed")`. RED/GREEN cobriu a
  sequência do controlador. `cargo test --lib` passou com 183 testes,
  `cargo check --bin sprite-demo`, `cargo fmt --all -- --check` e
  `git diff --check` passaram. O teste fake de runtime cobre resize contínuo,
  input, configuração, redraw, fechamento e o sinal de shutdown compartilhado,
  sem janela real.
  HILT confirmou redraw, resize e fechamento sem panic. A integração visual da
  Config UI permanece explicitamente na T035 do backlog; composição `wgpu`
  migra nos Passos 5–6.
- **Passo 5 em andamento:** RED fixou o caminho do adaptador público e GREEN
  moveu `WgpuContext`, `WgpuGraphicsDevice`, construção do pipeline, layout de
  vértices, uploads e cache de texturas para `render::graphics::wgpu`, com
  wrappers `WgpuPipeline`, `WgpuSurface`, `WgpuBuffer` e `WgpuTexture`. O
  runtime GPU já consome esses wrappers para contexto, pipeline e texturas. Foi criado um
  `MockGraphicsDevice` de teste que valida ciclo de vida, dimensões/limites de
  uploads e a ordem de desenho/apresentação. Como o adaptador de recursos ainda
  não codifica render pass, agora rejeita `DrawTexture`/`Present` em vez de
  ignorá-los; o suporte de apresentação será fechado na migração do Passo 6.
  Os testes de contrato e cache foram migrados do `RecordingDevice` local para
  esse mock compartilhado.
  `cargo test --lib` passou com 187 testes; `cargo check --bin sprite-demo`,
  `cargo fmt --all -- --check` e `git diff --check` passaram. O módulo legado
  `gpu.rs` agora mantém somente reexports de compatibilidade para vértices,
  shaders e recursos de textura, sem duplicar suas implementações. O runtime
  ainda contém acessos `wgpu` diretos; a migração e o confinamento completo
  seguem pendentes. O adaptador também deixou de consultar constantes de
  shader/layout/formato pelo módulo legado. A suíte foi repetida após esse
  refactor: `cargo test --lib` (187 testes), `cargo check --bin sprite-demo`,
  `cargo fmt --all -- --check`, busca de dependências legadas e
  `git diff --check` passaram.
- **Diagnóstico de frames lentos:** adicionados eventos separados para o tempo
  entre a finalização de um frame e o próximo ciclo do event loop
  (`GpuEventLoopWait`) e para a latência entre `request_redraw` e
  `RedrawRequested` (`GpuRedrawLatency`). RED/GREEN cobriu os formatadores
  desses tempos; `cargo fmt --all`, `git diff --check` e `cargo test --lib`
  passaram com 173 testes. Um dump agora consegue distinguir espera fora do
  renderer de custo de aquisição, composição, submissão e apresentação GPU. A
  espera é registrada no frame anterior, antes do próximo `begin_frame`, para
  não misturar a preparação CPU do frame seguinte nessa medição.
- **Passo 4 — shutdown coordenado:** o sinal `renderer_closed` foi extraído
  para `signal_renderer_closed`, sem depender de uma janela real. RED/GREEN
  passou com `gpu_window::tests::signals_renderer_closed_without_requiring_a_window`;
  a suíte unitária permanece GREEN com 173 testes. O próximo avanço é o
  adaptador GPU real do Passo 5, que fica sujeito a novo gate HITL.
- **Worktree:** implementar em `FractalExplorer-gpu`, branch `gpu-renderer`.

### T026 — Investigar e melhorar a precisão para zoom profundo

- **Critério de aceitação:** a câmera, camadas e tiles preservam coordenadas e passo suficientes para zoom profundo; copiar uma localização e restaurá-la na mesma versão mantém a mesma visão. Ver [diretriz de precisão](task-descriptions/T026-deep-zoom-precision.md).
- **Atualização:** o zoom inicial da câmera foi separado da escala visual da camada semente. A camada permanece em escala configurada (`8` por padrão), enquanto o `delta` é ajustado para representar o zoom inicial profundo; isso evita converter `2^48` em tamanho de tile e elimina o overflow em `ensure_screen_coverage`.
- **RED/GREEN/REFACTOR:** `deep_starting_zoom_keeps_seed_layer_screen_size_bounded` falhou antes da função de parâmetros existir e passou após a separação; `cargo fmt -- --check`, `git diff --check` e `cargo test --bin sprite-demo` passaram com 2 testes.

### T025 — Unificar coordenada copiada, zoom e visão inicial

- **Critério de aceitação:** o clique do meio sempre deve copiar e registrar `x`, `y` e o zoom atual em uma única string; `renderer.starting_point` deve aceitar essa mesma string para restaurar a visão inicial. O relatório detalhado por camadas deve permanecer opcional, controlado somente por `renderer.debug.middle_click_coordinate_report`, cujo padrão é `false`.
- **RED/GREEN:** testes criados para a flag de relatório, a serialização da coordenada com zoom e o carregamento da visão inicial. RED confirmou os campos e APIs ausentes; GREEN: `cargo test` passou com 61 testes. A validação manual e o commit/push dependem de liberar o executável bloqueado.

### T024 — Atualizar o renderizador progressivamente durante o redimensionamento

- **Critério de aceitação:** a janela deve aceitar redimensionamento e o framebuffer, os envelopes de alocação e a renderização devem acompanhar cada tamanho informado pela janela durante o gesto, sem esperar a soltura da borda. Em cada frame do arrasto, os tiles devem manter seu tamanho correto e a cobertura deve adicionar/renderizar novos tiles progressivamente à medida que ficam prontos.
- **RED/GREEN:** `renderer::tests::render_surface_reallocates_the_framebuffer_for_each_live_window_size` falhou pela ausência de `RenderSurface`; após a implementação, confirma a realocação para `960x540`, o recálculo dos envelopes e a rejeição de tamanhos transitórios nulos. `cargo test` passou com 56 testes; `cargo fmt -- --check` e `git diff --check` passaram.
- **Validação manual:** ao aumentar a janela durante o arrasto, cada frame mostra a imagem anterior esticada, sem novos tiles; a cobertura só é atualizada após soltar o mouse, quando os tiles corretos aparecem. Isso não atende ao critério: investigar o ciclo de eventos/redimensionamento e garantir atualização, alocação e renderização progressivas durante o gesto antes do commit/push.

### T023 — Registrar percurso de navegação no relatório de coordenadas

- **Critério de aceitação:** o relatório de clique do meio deve informar todos os arrastos e zooms aplicados ao canvas, com dados suficientes para reproduzir a sequência em um teste de regressão de alinhamento das camadas.
- **Decisão:** registrar os comandos de navegação no próprio canvas, pois um retrato final das coordenadas não permite reconstruir de forma determinística a sequência de transformações.
- **Evidências:** RED confirmado para a ausência do percurso e para o uso incorreto da transformação global no relatório de cada camada. GREEN: `cargo test renderer::tests::` passou com 13 testes; `cargo fmt` e `git diff --check` passaram. Regressão RED criada em `orchestrator::tests::reported_navigation_keeps_all_layer_coordinates_under_the_cursor_aligned`: reproduz os 31 zooms informados e confirma que a camada 0 mapeia o cursor para `-1.5913955983658463x0.04982497959951243`, em vez do ponto global `-1.5912597012720062x0.04979363767123237`. O valor absoluto diverge do relatório por ele ainda não registrar o estado inicial do canvas, mas a sequência de comandos e a inconsistência de transformação são reproduzidas. `cargo test` completo também permanece RED no teste preexistente `orchestrator::tests::tiled_infinite_canvas_expands_by_one_layer_per_frame` (`esperado 360x265`, `obtido 362x267`); por isso a tarefa permanece em `TODO`, sem commit/push ou validação manual.
- **Atualização:** o relatório passou a registrar o estado inicial (`posição`, `tile`, `delta`, `tela`, `zoom_max` e `zoom_min`), e `cargo test renderer::tests::` passou com 14 testes. É necessário gerar um novo relatório para ajustar a regressão aos valores absolutos do caso visual.
- **Regressão mínima:** `orchestrator::tests::initial_layer_expansion_keeps_the_cursor_complex_coordinate_aligned` reproduz o estado inicial informado e cinco expansões de frame, sem pan ou zoom. RED confirmado na camada 3: `-1.5565625x0.0009375`, em vez de `-1.55625x0.00125`; a tolerância de `1e-12` elimina somente ruído de ponto flutuante.
- **Isolamento progressivo:** `orchestrator::tests::each_initial_layer_expansion_keeps_the_cursor_complex_coordinate_aligned` valida a invariante depois de cada expansão. As expansões 1, 2 e 3 passam; a primeira ruptura é a expansão 4, na camada 3 (`zoom=1`), com o mesmo desvio do relatório.
- **Diagnóstico visual:** o relatório marca uma camada divergente com `DESALINHADA dx=... dy=...` e informa `Discrepancias detectadas: N`. RED/GREEN: o teste do marcador falhou antes da implementação e `cargo test renderer::tests::` passou com 15 testes depois dela.
- **Causa isolada:** a regressão sem cobertura de tiles falha igualmente na expansão 4; `adjacent_layer(..., 0.5)` já retorna desalinhada antes da sincronização. O teste mais próximo da causa registra a origem da nova camada em `491x361`, enquanto `canvas.complex_to_screen(layer.position())` produz `491x360`.
- **Log de criação:** a criação e expansão de camadas imprimem e armazenam `Camada criada com delta: <expoente>` ou `Camada expandida, direcao de incremento: <maior|menor>, delta: <expoente>`. RED/GREEN: `orchestrator::tests::records_the_delta_exponent_and_direction_for_created_layers` passou após a implementação.
- **Overlays de texto:** adicionadas em `[renderer.debug]` as flags booleanas `text_overlay_global`, `text_overlay_workers`, `text_overlay_layers` e `text_overlay_queue`, todas com padrão `true` e registradas em `config.toml`. RED/GREEN: `cargo test config::tests::` passou com 4 testes e `cargo test renderer::tests::` passou com 15 testes.
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
### T021 — Separar técnica, nível de precisão e método de renderização

- **Implementação local concluída:** `PrecisionDecisionManager` separa técnica
  (`float`/`fixed`) e nível; `direct` usa esse plano e `perturbation` usa o
  mesmo plano na semente, com delta fixo `float64` nível 1. Níveis float acima
  de 1 são normalizados para 1 sem erro. A `TileLayer` recebe o plano e o
  orquestrador o aplica em todos os tiles.
- **Bloqueio:** testes e compilação passaram, commit local `6264431` criado,
  mas o push para `origin/multiprecisao` foi recusado pela política do ambiente.

## BACKLOG

### T035 — Integrar Config UI e renderer GPU em runtime único

- Resolver a limitação documentada em
  [T035-gpu-config-ui-runtime.md](task-descriptions/T035-gpu-config-ui-runtime.md).
- Manter `winit::EventLoop` na thread principal e permitir configuração ao vivo
  no modo GPU sem abrir um segundo event loop em thread secundária.
- Promover para `TODO` somente quando a implementação for explicitamente
  priorizada.

### T030 - Filtrar tiles totalmente sobrepostos na composicao

- Identificar tiles cuja area visivel esteja completamente coberta por tiles de
  camadas superiores ja incluidos no batch.
- Remover esses tiles do batch de renderizacao sem alterar o resultado visual,
  respeitando ordem de camadas, opacidade, viewport reduzida e envelope de
  debug.
- Manter o filtro independente do destino de renderizacao, para que CPU e GPU
  recebam a mesma selecao logica de tiles.
- Cobrir com testes a sobreposicao parcial, total, transparencia, camadas
  invertidas e a ausencia de tiles cobertos no batch final.

### T031 - Reorganizar a documentacao do repositorio

- Criar as pastas `docs/`, `skills/` e `plans/`.
- Separar as instrucoes atualmente concentradas em `AGENTS.md` entre
  documentacao geral em `docs/` e instrucoes operacionais reutilizaveis em
  `skills/`.
- Mover os planos de implementacao para `plans/`.
- Garantir que todos os arquivos `.md` do repositorio fiquem em `docs/`,
  `skills/` ou `plans/`, preservando links relativos e referencias usadas por
  ferramentas e agentes.
- Adicionar uma verificacao automatica que falhe quando um novo `.md` for
  criado fora dessas pastas.

### T027 — Representar `delta` como expoente inteiro positivo

- Fazer com que `delta` seja representado exclusivamente por um expoente inteiro positivo na camada, no tile e em todos os demais pontos em que essa informação for necessária.
- Definir as conversões para o passo numérico real apenas nas fronteiras de cálculo e renderização.
- Revisar serialização, logs, invariantes e testes para garantir que o expoente permaneça positivo e consistente.

### T028 — Implementar precisão arbitrária baseada em lista de inteiros

- Implementar uma representação de precisão arbitrária usando uma lista de inteiros.
- Usar essa representação para coordenadas, passos e demais valores que precisem preservar precisão em zoom profundo.
- Depois, criar funções otimizadas que aproveitem a representação inteira de `delta`, evitando conversões e operações desnecessárias.
- Cobrir conversões, operações aritméticas, invariantes e desempenho com testes específicos.

### Nota — Remover artefatos que remetem ao estado local

- Avaliar a remoção ou realocação de `fractal_projects_review.md` e `legacy-worktrees.json`, pois ambos registram caminhos, worktrees e inventário específicos da máquina local. Preservar antes qualquer informação que deva virar documentação portátil do projeto.

### T022 — Diagnosticar alinhamento das camadas com envelopes de tiles

- Adicionar retângulos de debug ao redor dos tiles.
- Usar uma cor aleatória por camada, consistente para todos os tiles daquela camada.
- Verificar visualmente se as camadas se alinham como níveis equivalentes de uma quadtree.
- Usar o diagnóstico para orientar a futura conversão de coordenadas da camada pela câmera.

### T029 — Resolver desalinhamento de tiles usando tipos pequenos

- Investigar e corrigir o desalinhamento entre camadas sem implementar conversão de cada tile do plano complexo para a tela durante o desenho.
- Reavaliar a solução atual de posicionamento relativo mantendo o custo e a representação numérica reduzidos quando possível.
- Usar sempre o menor tipo numérico adequado para índices, dimensões e posições de tela, sem alterar desnecessariamente os tipos de coordenadas complexas.

### T020 — Cobrir invariantes da sequência de camadas

- Validar a sequência completa de zooms, por exemplo `8, 4, 2, 1`.
- Garantir que cada camada adjacente tenha exatamente o dobro ou a metade do zoom e do delta da ponta anterior.
- Verificar ausência de zooms repetidos após zoom in/out, retração e expansão.

### T019 — Verificar camada vazia no ciclo de retração e expansão

- Avaliar se aplicar a ordem de retração às camadas e aos tiles pode deixar uma camada com zero tiles.
- Validar o ciclo: retrair → verificar se ficou vazia → criar uma camada com um tile → expandir.
- Confirmar que a mesma sequência é segura tanto para `TiledInfiniteCanvas`/camadas quanto para `TileLayer`/tiles.

### T030 — Revisar invariância do pivô no zoom

- Revisar o fluxo de zoom para garantir que o ponto do plano complexo sob o cursor permaneça na mesma posição do botão do mouse durante a operação.
- Verificar a interação entre conversão tela/plano, transformação da camada, expansão/desalocação de tiles e processamento assíncrono.
- Criar testes que cubram zoom in/out, múltiplas operações e cursor fora do centro.

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

### T031 — Integrar multiprecisão na camada e corrigir navegação (reconstrução histórica)

- **Resultado:** a integração foi preservada para a reconstrução histórica; a
  implementação compatível com o canvas atual será reaplicada pelo merge
  `4bc041b`.

### T032 — Integrar multiprecisão na camada e corrigir navegação

- **Resultado:** `Orchestrator::from_config` aplica `multiprecision` ou
  `perturbation` a todos os tiles, inclusive os criados por expansão; o loop do
  renderer processa pan/zoom antes de recalcular cobertura e renderização.
- **Evidências:** RED confirmou a ausência do orquestrador configurável;
  GREEN passou com 44 testes, `cargo check --bin sprite-demo`, `cargo fmt` e
  `git diff --check`. A validação visual da navegação fica a cargo do usuário.

### T033 — Implementar núcleo `Fixed<N>` com limbs `u64`

- **Resultado:** implementados `Fixed<1>`, `Fixed<2>` e `Fixed<N>` com limbs
  `u64`, escala fracionária `32 * N`, cálculo Mandelbrot direto por pixel,
  órbita de referência, seleção em grade, perturbação e fallback configurável.
- **Configuração:** `renderer.rendering_method` aceita `f64`,
  `multiprecision` e `perturbation`; `renderer.perturbation_fallback` começa
  desabilitado para preservar artefatos instáveis.
- **Evidências:** `cargo test` passou com 28 testes; `cargo check --bin
  sprite-demo` passou; `cargo fmt` e `git diff --check` passaram. A validação
  visual da janela fica a cargo do usuário.

### T034 — Investigar e planejar a multiprecisão u64

- **Resultado:** investigação do `fractalExplorer_kotlin2026` registrada em
  `PLANO_IMPLEMENTACAO_MULTIPRECISAO.md`, cobrindo full multiprecision,
  seleção/órbita de referência, perturbação, snippets e o plano para
  `Fixed<1>`, `Fixed<2>` e `Fixed<N>` com limbs `u64`.
- **Decisão registrada:** `perturbation_fallback = false` por padrão, com opção
  funcional para habilitar fallback e permitir inspeção dos artefatos quando a
  perturbação perder estabilidade.
- **Evidências:** `git diff --check` passou; não foram executados testes pesados
  ou benchmarks nesta etapa.
- **Commit:** `9edf964`, enviado para `origin/multiprecisao`.

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
