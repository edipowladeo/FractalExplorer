# Review técnico — backend wgpu (renderer plugável, WIP CPU/GPU)

## Instrução para o agente

Este arquivo é um review de código feito externamente sobre o estado atual do
backend `wgpu`, no contexto da migração para renderer plugável (trait
`GraphicsDevice`) com convivência temporária entre o caminho legado
(`encode_frame` / `GpuTextureStore` / `TileDrawCommand` / `TextureKey` /
`GpuTileTexture`) e o caminho novo (`WgpuGraphicsDevice` implementando
`GraphicsDevice`).

Antes de agir:
1. Cruze cada ponto abaixo com o seu arquivo de plano próprio. Marque
   explicitamente quais itens **já estavam no radar** (e em que etapa do
   plano) e quais são **novos**.
2. Para os itens novos, proponha em qual etapa do plano eles se encaixam
   (parte da paridade legado→novo, parte de "limpeza final", ou item
   ortogonal que pode ser resolvido a qualquer momento).
3. Não implemente nada ainda — devolva primeiro o diagnóstico de
   sobreposição com o plano, para alinharmos prioridade antes de mexer em
   código.

---

## Classificação geral

Os itens abaixo estão marcados como:
- **[SÓ LEGADO]** — some sozinho quando `encode_frame`/`GpuTextureStore`
  forem removidos. Não vale investir agora se o plano já prevê a remoção.
- **[SÓ NOVO]** — vive inteiramente em `WgpuGraphicsDevice`/`submit`/`draw`.
  Sobrevive à limpeza tal como está hoje. Prioridade real.
- **[COMPARTILHADO]** — infraestrutura usada pelos dois caminhos
  (`WgpuContext`) ou utilitário que o caminho novo ainda depende de tipos do
  legado para reaproveitar. Não desaparece automaticamente com a remoção do
  legado; precisa de ação própria.

---

## 1. Duas pipelines de render duplicadas — **[legado vs. novo, esperado no WIP]**

`encode_frame()` (função livre, caminho legado) e
`WgpuGraphicsDevice::draw()` (caminho novo) implementam essencialmente a
mesma sequência de decisões (load op condicional a
`preserve_previous_frame`/`surface_initialized`, composição em textura
intermediária, segundo pass de presentation) para fontes de dados
diferentes. É o formato normal de uma migração em andamento — mas duas
lacunas de paridade específicas precisam fechar antes de remover
`encode_frame`:

- **Telemetria ausente no novo** — `encode_frame` recebe um callback
  `report_stage` (tempo de composição/apresentação); `draw()` não tem
  equivalente. Se isso não for portado, a remoção do legado é uma regressão
  silenciosa de observabilidade.
- **Camadas extra sem paridade** — `encode_frame` aceita `envelope` e
  `overlay` como camadas adicionais de composição; `Command`/`CommandList`
  (vocabulário do trait novo) não tem conceito equivalente. Confirmar se
  isso é intencionalmente fora de escopo do novo modelo ou se falta migrar.

**Risco operacional, independente do WIP:** se os dois caminhos puderem
rodar simultaneamente sobre o *mesmo conteúdo* (mesma imagem/tile), a
textura pode acabar subindo duas vezes à GPU sob identidades diferentes
(`TextureKey` de um lado, `TextureHandle` do outro), sem nenhuma relação
entre si — risco de VRAM duplicada e dessincronia. Confirmar se os dois
caminhos coexistem ativos ao mesmo tempo para o mesmo conteúdo, ou se um já
está órfão.

## 2. Dois modelos de textura paralelos — **[SÓ LEGADO, mas revisar encapsulamento]**

`GpuTileTexture` (legado, todos os campos `pub`) vs. `WgpuTexture` (novo,
campo prefixado `_texture` por convenção de "só mantém o recurso vivo").
Filosofias de encapsulamento diferentes entre os dois — não é bug, mas
reforça que os dois foram escritos em momentos diferentes sem revisão
cruzada. Desaparece com a remoção do legado; não priorizar agora.

## 3. `content_hash` nunca usado para cache — **[SÓ LEGADO]**

`TextureKey`/`TextureUpload` têm `content_hash`, desenhado para permitir
pular reupload de textura inalterada, mas `GpuTextureStore::upload` sempre
recria a textura incondicionalmente — a otimização nunca foi conectada.
Como é tipo exclusivo do caminho legado, **não vale implementar** se o
plano já prevê descontinuar esse caminho — seria trabalho descartável.

## 4. Validação de comandos inconsistente dentro do `submit()` — **[SÓ NOVO — prioridade real]**

Dentro do próprio caminho novo, `submit()` faz duas passadas de validação
com padrões diferentes:
- `validate_commands()` — valida ordem (write→draw→present); **extraída em
  função nomeada e testada**.
- Um segundo loop manual, inline dentro de `submit()` — valida conteúdo
  (bounds de buffer, existência de resource, range de opacidade);
  **não extraído, sem teste próprio**.

Isso não é resíduo de migração — é inconsistência de padrão dentro do
código recém-escrito, justamente na parte mais sensível (bounds check de
memória antes de tocar a GPU). Recomendação: extrair o segundo loop para
uma função nomeada e testável, no mesmo padrão de `validate_commands()`.

## 5. `draw()` como god-method — **[SÓ NOVO — prioridade real]**

`WgpuGraphicsDevice::draw()` concentra: gestão de ciclo de vida da
composição (recriação em resize), retry de acquire de surface
(outdated/lost), dois render passes, submit e present, e mutação de estado
(`surface_initialized`) — tudo em ~150 linhas sem quebra em etapas
nomeadas. É a versão "função grande demais" do mesmo smell do arquivo como
um todo. Sobrevive à remoção do legado tal como está. Recomendação: quebrar
em métodos privados por etapa (ex.: `ensure_composition_target`,
`acquire_frame_with_retry`, `record_composition_pass`,
`record_present_pass`).

## 6. Erros stringly-typed em `WgpuContext` — **[COMPARTILHADO — não desaparece sozinho]**

`WgpuContext::initialize`, `create_surface` e `poll_device` retornam
`Result<_, String>`, perdendo tipagem de erro para o chamador. O restante
do sistema novo usa `DeviceError` (enum) via trait `GraphicsDevice`.
`WgpuContext` é usado tanto pelo legado quanto pelo novo — então esse
descompasso de tipagem **continua existindo mesmo depois de remover o
legado**, porque o `GraphicsDevice`/`WgpuGraphicsDevice` novo também
depende de `WgpuContext` por baixo. Precisa de ação própria: propagar
`DeviceError` (ou um erro específico de inicialização) até essa camada, em
vez de manter `String`.

## 7. Acoplamento do utilitário de vértices a tipo do legado — **[COMPARTILHADO/misto]**

`render_vertices_for_commands` (usada dentro do `draw()` **novo**) monta um
`TileDrawCommand`/`TextureKey` "fake" (com `content_hash: 0` sempre) só
para reaproveitar `tile_vertices_for_commands`, que é matemática de vértice
agnóstica de backend/versão. O caminho novo empacotando dados em um tipo do
legado só para reusar uma função é acoplamento desnecessário que **não
desaparece automaticamente** ao remover `encode_frame`/`GpuTextureStore` —
a função utilitária vai precisar ser refatorada para operar sobre um tipo
neutro (ex.: posição+tamanho puro, sem depender de `TextureKey`) antes ou
durante a remoção do legado.

---

## Resumo de prioridade sugerida

| # | Item | Escopo | Desaparece com remoção do legado? | Ação agora? |
|---|------|--------|-----------------------------------|-------------|
| 1 | Paridade `encode_frame` vs `draw()` (telemetria, envelope/overlay) | legado→novo | Parcial — só se paridade for fechada antes | Sim, como pré-requisito da remoção |
| 1b | Risco de textura duplicada/dessincronia se ambos ativos | legado+novo | Sim, ao remover legado | Confirmar estado atual (coexistem ativos?) |
| 2 | Dois modelos de textura, encapsulamento distinto | legado | Sim | Não priorizar |
| 3 | `content_hash` sem cache implementado | legado | Sim | Não implementar (trabalho descartável) |
| 4 | Validação inconsistente em `submit()` | **novo** | Não | **Sim, prioridade real** |
| 5 | `draw()` god-method | **novo** | Não | **Sim, prioridade real** |
| 6 | Erros `String` em `WgpuContext` | compartilhado | Não | Sim, ação própria necessária |
| 7 | Utilitário de vértices acoplado a tipo legado | compartilhado/misto | Não | Sim, refatorar ao mexer no legado |
