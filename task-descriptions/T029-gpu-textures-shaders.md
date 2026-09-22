# T029 — Renderização modular com destinos plugáveis

## Objetivo revisado

Transformar a implementação GPU já validada em apenas mais um destino de
renderização. O ciclo da aplicação, a configuração, a navegação, o canvas, o
agendamento de tiles, a composição lógica do frame, os overlays, a
instrumentação e o encerramento devem seguir um único caminho, independente do
destino escolhido.

O caminho comum deve aceitar:

- destino CPU legado;
- destino GPU baseado em `wgpu` (GL, Vulkan, DX12 ou Metal conforme a
  plataforma e o driver);
- futura implementação Metal nativa;
- futuros destinos WebGL e mobile;
- processadores de tiles CPU ou GPGPU, escolhidos independentemente do destino
  de apresentação.

Tipos de `minifb`, `winit`, `wgpu`, Metal, OpenGL, DirectX ou OpenCL não podem
atravessar a fronteira dos respectivos adaptadores.

## Diagnóstico do estado atual

O spike GPU validou texturas, shaders, cache, apresentação e instrumentação,
mas criou um segundo caminho vertical. Hoje `GpuWindowApp` acumula quatro
responsabilidades:

1. ciclo de vida da janela e tradução de eventos;
2. preparação do frame e atualização do canvas;
3. gerenciamento de recursos gráficos;
4. codificação, submissão e apresentação.

O renderer CPU mantém versões próprias das duas primeiras responsabilidades.
Essa duplicação explica as divergências observadas na UI de configuração, na
viewport reduzida, nos overlays e no encerramento.

A migração não deve acrescentar condicionais CPU/GPU a esses recursos. Ela deve
extrair o comportamento comum e reduzir os backends a adaptadores de entrada e
destinos de saída.

## Arquitetura-alvo

```text
Config UI / arquivo
        |
        v
PlatformRuntime ---- eventos normalizados ----> ApplicationController
 (winit/minifb)                              / configuração, input,
                                            / canvas, scheduler,
TileProcessor -----------------------------/ instrumentação e frame lógico
 (CPU/GPGPU)                                      |
                                                  v
                                             RenderFrame
                                                  |
                                     RenderTarget (plugin)
                                      /                 \
                              CpuRenderTarget       GpuRenderTarget
                                                        |
                                                GraphicsDevice
                                               /       |       \
                                            wgpu    Metal    futuro
```

### Regra de dependências

As dependências apontam para dentro:

- `app_core` conhece apenas contratos e modelos próprios;
- `render_core` conhece `RenderFrame`, descritores e handles próprios;
- adaptadores de plataforma conhecem `winit` ou `minifb`;
- adaptadores gráficos conhecem `wgpu`, Metal ou outra API;
- `main` e as factories fazem a composição concreta;
- nenhum módulo de domínio escolhe backend com `match`.

## Contratos principais

Os nomes abaixo são orientativos; durante o refactor podem ser ajustados sem
alterar as responsabilidades.

### 1. Modelo lógico do frame

`RenderFrame` é um snapshot imutável, independente de API gráfica:

```rust
pub struct RenderFrame {
    pub frame_id: FrameId,
    pub viewport: Viewport,
    pub clear_color: Color,
    pub tiles: Vec<TileDraw>,
    pub overlays: Vec<OverlayPrimitive>,
}

pub struct TileDraw {
    pub image: ImageId,
    pub revision: ImageRevision,
    pub destination: Rect,
    pub source: UvRect,
    pub layer: LayerOrder,
    pub opacity: f32,
}
```

O frame referencia imagens por identidade e revisão. Pixels novos são
publicados separadamente por `ImageUpdate`; frames sem alteração reutilizam o
mesmo recurso. Isso impede uploads causados apenas por redesenho ou mudança de
FPS.

### 2. Destino de renderização

```rust
pub trait RenderTarget: Send {
    fn capabilities(&self) -> RenderCapabilities;
    fn resize(&mut self, viewport: Viewport) -> Result<(), RenderError>;
    fn update_images(&mut self, updates: &[ImageUpdate]) -> Result<(), RenderError>;
    fn render(&mut self, frame: &RenderFrame) -> Result<FrameOutcome, RenderError>;
    fn evict_images(&mut self, images: &[ImageId]);
    fn recover(&mut self, reason: SurfaceFailure) -> Result<(), RenderError>;
}
```

`CpuRenderTarget`, `WgpuRenderTarget` e um futuro `MetalRenderTarget`
implementam o mesmo contrato. A factory seleciona a implementação a partir da
configuração e aplica fallback; o controlador não conhece a seleção.

### 3. Runtime de plataforma

Loops de eventos nativos possuem inversão de controle diferente. Em vez de
tentar escondê-los atrás de um `run()` artificial, cada adaptador traduz seus
eventos para `AppEvent` e chama o mesmo `ApplicationController`:

```rust
pub trait ApplicationController {
    fn handle_event(&mut self, event: AppEvent) -> Vec<AppEffect>;
    fn prepare_frame(&mut self) -> Result<PreparedFrame, AppError>;
}
```

`AppEvent` cobre resize, teclado, ponteiro, redraw, atualização de configuração
e encerramento. `AppEffect` cobre solicitar redraw, alterar cursor, copiar texto
e sair. Recursos exclusivos da plataforma ficam em serviços pequenos, como
`ClipboardPort`, e não no renderer.

### 4. Wrappers gráficos de baixo nível

O destino acelerado usa uma camada interna e object-safe. Ela abstrai recursos
sem vazar objetos nativos:

```rust
pub trait GraphicsDevice: Send {
    fn capabilities(&self) -> GraphicsCapabilities;
    fn create_buffer(&mut self, desc: BufferDescriptor) -> Result<BufferHandle, GfxError>;
    fn write_buffer(&mut self, handle: BufferHandle, offset: u64, data: &[u8])
        -> Result<(), GfxError>;
    fn create_texture(&mut self, desc: TextureDescriptor)
        -> Result<TextureHandle, GfxError>;
    fn write_texture(&mut self, handle: TextureHandle, update: TextureWrite<'_>)
        -> Result<(), GfxError>;
    fn create_pipeline(&mut self, desc: PipelineDescriptor)
        -> Result<PipelineHandle, GfxError>;
    fn execute(&mut self, commands: &CommandList) -> Result<PresentOutcome, GfxError>;
    fn destroy(&mut self, resource: ResourceHandle);
}
```

`BufferHandle`, `TextureHandle` e `PipelineHandle` são IDs opacos, com geração,
para detectar handles expirados. `CommandList` contém comandos próprios do
projeto (`BeginPass`, `BindPipeline`, `BindTexture`, `BindBuffer`, `Draw`,
`EndPass`, `Present`). Descritores usam enums e flags do projeto.

Esta camada não tentará representar toda API gráfica. Só ganhará operações
necessárias aos renderers do projeto. Extensões específicas serão expostas por
`GraphicsCapabilities`, nunca por downcast no domínio.

### 5. Registro e wrappers de recursos

`ResourceRegistry` será responsável por:

- associar `ImageId + ImageRevision` a `TextureHandle`;
- manter buffers persistentes e aumentar capacidade geometricamente;
- atualizar somente intervalos alterados;
- reter recursos enquanto houver frames em voo;
- invalidar recursos após perda de device/surface;
- destruir recursos por RAII no wrapper ou por coleta explícita e testável;
- fornecer métricas de criação, atualização, reutilização e descarte.

O cache do envelope de debug deixa de ser uma exceção do backend e passa a usar
o mesmo mecanismo de identidade/revisão de qualquer imagem de overlay.

### 6. Processamento de tiles independente

`TileProcessor` é uma porta separada de `RenderTarget`:

```rust
pub trait TileProcessor: Send + Sync {
    fn submit(&self, request: TileRequest) -> TileJob;
    fn poll(&self, job: TileJob) -> TileJobState;
    fn cancel(&self, job: TileJob);
}
```

CPU, OpenCL, compute via `wgpu` e Metal compute poderão implementar essa porta.
Inicialmente todos publicam um `TileImage` canônico em memória do host. Uma
segunda etapa poderá acrescentar `ExternalImageLease` para zero-copy quando
processador e renderer compartilharem device e formato; o fallback obrigatório
será readback/upload pelo formato canônico. Assim GPGPU não fica acoplado ao
backend que apresenta a janela.

## Estrutura de módulos pretendida

```text
src/
  app/
    controller.rs          # ciclo comum e AppEvent/AppEffect
    frame_builder.rs       # canvas -> RenderFrame/ImageUpdate
  render/
    mod.rs                 # RenderTarget e tipos públicos
    resources.rs           # IDs, revisões e ResourceRegistry
    cpu.rs                 # destino CPU
    gpu.rs                 # composição acelerada, sem API nativa
    graphics/
      mod.rs               # GraphicsDevice e descritores
      wgpu.rs              # wrappers de wgpu
      metal.rs             # futuro adaptador Metal nativo
  platform/
    mod.rs                 # eventos e serviços portáveis
    winit.rs               # runtime nativo principal
    minifb.rs              # runtime legado/fallback enquanto necessário
  processing/
    mod.rs                 # TileProcessor
    cpu.rs
    wgpu_compute.rs        # futuro
    metal_compute.rs       # futuro
```

Dependências opcionais devem ser controladas por features (`renderer-wgpu`,
`renderer-cpu`, `graphics-metal`, `processor-wgpu`, `processor-metal`, por
exemplo). Builds sem uma feature não compilam nem importam o SDK correspondente.

## Plano incremental de implementação

Cada passo termina em RED, GREEN e REFACTOR, com testes rápidos registrados.
Enquanto a migração estiver em curso, CPU e GPU devem continuar selecionáveis.

### Passo 0 — Congelar o comportamento validado

- Criar testes de caracterização para viewport reduzida, envelopes, ordem de
  camadas, tiles progressivos, overlays, input, resize, atualização de config e
  encerramento.
- Registrar golden data do `PreparedTileBatch` atual sem depender de `wgpu`.
- Documentar quais diferenças CPU/GPU são toleradas e quais são bugs.

**Saída:** uma suíte capaz de detectar nova divergência antes da extração.

Os testes devem separar comportamento determinístico de comportamento dependente
de plataforma. Viewport, envelopes, ordem de camadas, input normalizado,
configuração, shutdown e sequência de frames devem ser cobertos com fakes e
golden data. A abertura de janela, desenho efetivo, resize durante o gesto e
qualidade visual ficam para uma validação HITL curta e explícita.

### Passo 1 — Introduzir `RenderFrame` e identidades estáveis

- Criar `ImageId`, `ImageRevision`, `ImageUpdate`, `TileDraw`,
  `OverlayPrimitive`, `Viewport` e `RenderFrame`.
- Extrair um `FrameBuilder` puro do código CPU/GPU atual.
- Garantir que o mesmo estado de canvas gere o mesmo `RenderFrame` nos dois
  caminhos.

**Teste RED principal:** CPU e GPU atuais produzem descrições lógicas idênticas
para a mesma cena, inclusive viewport reduzida e envelope.

### Passo 2 — Criar `RenderTarget` e adaptar primeiro o CPU

- Introduzir o contrato de alto nível e uma fake determinística para testes.
- Envolver o renderer CPU existente em `CpuRenderTarget` sem alterar pixels.
- Mover fallback e seleção para `RenderTargetFactory`.
- Fazer `main` depender da factory, não de enums concretos espalhados.

**Teste RED principal:** o controlador completo funciona com um target fake e
produz a sequência esperada de resize, updates, render, eviction e recovery.

Este passo não exige HITL para ser concluído. Uma execução visual CPU pode ser
usada apenas como smoke test depois que o contrato estiver GREEN.

### Passo 3 — Extrair `ApplicationController`

- Unificar configuração, canvas, agendamento, input, instrumentação, overlays e
  finalização no controlador.
- Converter atualização da Config UI em `AppEvent::ConfigurationChanged`.
- Fazer CPU e GPU receberem o mesmo `PreparedFrame`.
- Remover essas responsabilidades de `GpuWindowApp` e do loop CPU.

**Teste RED principal:** uma sequência normalizada de eventos produz os mesmos
efeitos e frames, independentemente do runtime e do target fake usados.

O runtime falso deve cobrir resize contínuo, redraw, configuração e fechamento;
nenhum desses casos deve depender de uma janela real.

### Passo 4 — Normalizar o runtime de janela

- Transformar `GpuWindowApp` em adaptador `WinitRuntime` fino.
- Encaminhar `window_event`, `about_to_wait` e redraw ao controlador.
- Restaurar a Config UI pelo canal comum e shutdown coordenado.
- Manter `MinifbRuntime` apenas como adaptador legado enquanto o target CPU
  precisar dele; preferir posteriormente um único runtime `winit` para ambos.

**Teste RED principal:** fake runtime cobre resize contínuo, redraw, fechamento
da janela de renderer e fechamento da Config UI sem conhecer CPU/GPU.

HITL fica reservado para confirmar que o adaptador traduz corretamente os
eventos reais da janela, especialmente resize durante o arrasto, redraw e
encerramento coordenado.

### Passo 5 — Introduzir wrappers `GraphicsDevice`

- Criar handles, descritores, `CommandList`, capabilities e erros portáveis.
- Criar `MockGraphicsDevice` que valida ciclo de vida e sequência de comandos.
- Implementar wrappers `WgpuGraphicsDevice`, `WgpuBuffer`, `WgpuTexture`,
  `WgpuPipeline` e surface/presentação.
- Confinar todo tipo `wgpu::*` ao adaptador `render/graphics/wgpu.rs`.

**Teste RED principal:** a composição de uma cena gera o mesmo command list no
mock e no adaptador `wgpu`, sem exigir uma GPU nos testes unitários.

Handles, descritores, cache, invalidação e ordem de comandos devem permanecer
testáveis exclusivamente com `MockGraphicsDevice`. A criação real de device e
surface é uma verificação opcional de integração, não um requisito do ciclo
RED/GREEN.

### Passo 6 — Migrar o destino GPU para os contratos

- Implementar `GpuRenderTarget<D: GraphicsDevice>`.
- Migrar shader, quads, cache de texturas, buffers e apresentação do spike.
- Substituir criações por frame por buffers persistentes e writes incrementais.
- Integrar o envelope e o timing overlay ao modelo comum de imagens/revisões.
- Manter instrumentação nas fronteiras lógicas e permitir métricas extras do
  adaptador.

**Teste RED principal:** frames sem mudança não criam nem enviam texturas ou
buffers; mudança de posição atualiza somente instâncias; mudança de imagem
atualiza somente a textura cuja revisão mudou.

O comportamento de recursos deve ser fechado com `MockGraphicsDevice` antes da
validação HITL. A validação manual deste passo fica limitada a uma cena GPU
real: abrir, renderizar, redimensionar, usar pan/zoom, alternar overlays e
fechar. Stalls, sincronização de driver, apresentação e qualidade dos shaders
não devem ser simulados além do que o mock consegue afirmar.

#### Plano operacional de diagnóstico e contenção de stalls GPU

1. Corrigir as flags do `config.toml` efetivamente carregado pelo worktree GPU,
   mantendo `show_allocation_envelope = false` e `text_overlay_frames = false`
   durante a medição da linha de base.
2. Usar um ring buffer de três slots para os vértices dos tiles, evitando
   sobrescrever um buffer que ainda pode estar em uso pela GPU.
3. Usar o mesmo ring buffer de três slots para os vértices do overlay e do
   envelope, com invalidação completa no resize ou na perda da superfície.
4. Instrumentar a seleção dos slots, indicando slot ativo, crescimento do
   buffer, reutilização e eventual espera observada durante a escrita.
5. Separar a instrumentação de submissão em encoder finalizado, `queue.submit`,
   `device.poll` e apresentação, para distinguir espera de upload e espera de
   execução/apresentação.
6. Comparar a linha de base sem overlays com as configurações de ring buffer de
   dois e três slots, usando os mesmos eventos e a mesma cena.
7. Registrar os resultados de CPU, backend GL e demais backends disponíveis,
   classificando o custo como CPU, transferência, sincronização do driver ou
   apresentação antes de escolher a otimização definitiva.

### Passo 7 — Eliminar os caminhos verticais duplicados

- Remover preparação de canvas, overlays e configuração de `gpu_window.rs`.
- Remover composição lógica duplicada de `renderer/mod.rs`.
- Fazer CPU e GPU diferirem somente a partir de `RenderTarget::render` e dos
  adaptadores inevitáveis de apresentação.
- Dividir ou remover `gpu_window.rs` após seus últimos consumidores migrarem.

**Gate arquitetural:** busca estática e testes impedem imports de `wgpu`,
`winit` ou `minifb` em `app_core`, `orchestrator` e contratos de renderização.

O gate pode ser verificado automaticamente com busca de dependências, fake
runtime e fake target. HITL serve apenas para confirmar que a experiência CPU e
GPU continua equivalente nos fluxos principais.

### Passo 8 — Conformidade, fallback e recuperação

- Comparar CPU e GPU por cenas representativas e tolerância explícita.
- Testar falha de inicialização, perda de surface/device, resize nulo,
  reconstrução de cache e fallback.
- Fazer a seleção distinguir destino (`cpu`, `gpu`) de API (`auto`, `gl`,
  `vulkan`, `dx12`, `metal`).
- Expor capabilities e diagnóstico da implementação efetivamente escolhida.

**Teste RED principal:** qualquer falha recuperável preserva o estado lógico da
aplicação e troca/recria somente o adaptador afetado.

Perda de surface/device, fallback e recriação de cache devem ser simulados com
erros injetáveis. HITL é necessária somente para confirmar recuperação em um
backend real quando a plataforma permitir provocar ou observar essa falha.

### Passo 9 — Preparar GPGPU sem acoplar apresentação

- Extrair o processador CPU atual para `TileProcessor`.
- Criar fake processor assíncrono para testar prioridade, cancelamento e
  conclusão progressiva.
- Definir o formato canônico de `TileImage` e o contrato opcional de
  `ExternalImageLease`.
- Provar as quatro combinações em testes de contrato: CPU→CPU, CPU→GPU,
  GPGPU→CPU e GPGPU→GPU.

**Observação:** este passo prepara a arquitetura; implementar o kernel GPGPU
continua sendo tarefa própria e usa a suíte de conformidade.

As quatro combinações devem ser cobertas primeiro com fake processor e fake
target. Execução GPGPU real e medições de transferência são validações de
hardware opcionais, não critérios para os testes unitários.

### Passo 10 — Provar a substituição com Metal

- Compilar o backend `wgpu`/Metal em macOS sem mudanças no domínio.
- Se necessário, implementar `MetalGraphicsDevice` nativo usando os mesmos
  handles, descritores e command lists.
- Implementar Metal compute separadamente como `TileProcessor`.
- Manter interop/zero-copy como capability opcional, nunca como requisito do
  frame lógico.

**Critério:** adicionar Metal exige novos adaptadores e configuração, mas não
alterações no canvas, controlador, frame builder, overlays ou Config UI.

Compilação, contratos e seleção podem ser verificados sem hardware Metal; a
execução visual e a integração com o device Metal exigem HITL em macOS.

## Estratégia TDD e pirâmide de testes

- **Unitários, sempre rápidos:** frame builder, controlador, resource registry,
  cache, handles, command list, seleção/fallback e máquinas de estado.
- **Testes de contrato:** todo `RenderTarget`, `GraphicsDevice` e
  `TileProcessor` roda a mesma suíte reutilizável.
- **Conformidade de imagem:** cenas pequenas CPU/GPU com tolerância explícita e
  sem depender de temporização.
- **Integração sem hardware:** fake runtime + fake target + mock device.
- **Integração com hardware:** opt-in, separada, nunca necessária para o ciclo
  unitário RED/GREEN.
- **HILT mínimo:** uma verificação por backend real cobrindo inicialização,
  renderização, resize, pan/zoom, configuração, overlays e encerramento.
- **HILT adicional:** shaders, apresentação, stalls, perda de device/surface e
  Metal real somente quando a alteração tocar esses limites.
- **HILT:** somente quando o checkbox global for marcado pelo usuário. Enquanto
  estiver desmarcado, a aplicação não será iniciada automaticamente.

O objetivo é manter a maior parte da migração rápida, determinística e
reproduzível. HITL não deve substituir testes de contrato nem ser usado para
validar lógica que pode ser exercitada por fakes, mocks ou golden data.

Cada passo registra no `TASKS.md` o teste que falhou no RED, os testes GREEN e
o refactor realizado. Verificações pesadas e benchmarks só serão executados por
solicitação explícita.

## Critérios de aceitação finais

- Há um único `ApplicationController` e um único `FrameBuilder` para CPU/GPU.
- Config UI, input, viewport reduzida, envelopes, overlays, instrumentação,
  resize, shutdown e renderização progressiva independem do target.
- CPU e GPU são selecionados por factory e implementam `RenderTarget`.
- `wgpu` está confinado ao adaptador correspondente; o mesmo vale para APIs
  futuras.
- Buffers, texturas, pipelines e command lists são acessados por wrappers e
  handles próprios, com ciclo de vida testável.
- Frames sem mudança não recriam nem reenviam recursos.
- O processador de tiles é selecionável independentemente do target.
- Metal ou outro backend pode ser adicionado sem modificar domínio e recursos
  comuns da aplicação.
- O caminho CPU permanece como fallback e referência de conformidade.
- Testes ficam GREEN, o refactor é concluído, commits são focados e o branch é
  enviado ao remoto antes de T029 ir para `DONE`.

## Decisões e limites

- Não será criada uma abstração universal de GPU; somente as operações usadas
  pelo projeto entram em `GraphicsDevice`.
- APIs gráficas diferentes ficam atrás de adaptadores. Não serão espalhadas
  condicionais de plataforma pelo núcleo.
- Render e compute são plugins separados mesmo quando usam o mesmo device.
- Zero-copy é uma otimização opcional negociada por capability; a correção não
  depende dele.
- O spike existente será migrado incrementalmente, preservando o backend
  funcional em todos os passos; não haverá reescrita total de uma só vez.
