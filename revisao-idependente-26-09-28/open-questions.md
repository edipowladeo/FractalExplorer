# Open questions — review do renderer WGPU

Perguntas que precisam de resposta antes de fechar o plano do T029. O contexto
técnico completo está em [relatorio-resposta-review-wgpu-renderer.md](relatorio-resposta-review-wgpu-renderer.md).

## 1. Overlays e envelope

`envelope` e `overlay` do caminho legado devem ser considerados definitivamente
substituídos por `OverlayPrimitive` no frame comum?

Contexto: [seção 1 — Pipelines duplicadas e paridade](relatorio-resposta-review-wgpu-renderer.md#1-pipelines-duplicadas-e-paridade-legado--novo).

## 2. Coexistência dos caminhos

O caminho legado deve funcionar apenas como fallback temporário, sem compor o
mesmo frame ou conteúdo que o caminho novo?

Contexto: [seção 1b — Risco de duas identidades de textura](relatorio-resposta-review-wgpu-renderer.md#1b-risco-de-duas-identidades-de-textura).

## 3. Nível de telemetria

O caminho novo precisa preservar o nível detalhado de telemetria — início e fim
da composição, apresentação e codificação — ou os eventos agregados de
submissão/apresentação são suficientes?

Contexto: [seção 1 — Pipelines duplicadas e paridade](relatorio-resposta-review-wgpu-renderer.md#1-pipelines-duplicadas-e-paridade-legado--novo).

## 4. Modelo de erro

Os erros do `WgpuContext` devem ampliar `DeviceError` ou usar um tipo específico
como `WgpuContextError`, convertido na fronteira do adaptador?

Contexto: [seção 6 — Erros stringly-typed](relatorio-resposta-review-wgpu-renderer.md#6-erros-stringly-typed-em-wgpucontext).

## 5. Prioridade dos itens novos

Os itens 4–7 do review devem ser resolvidos dentro do fechamento do T029 antes
de qualquer trabalho novo, ou algum deles deve virar tarefa independente?

Contexto: [Plano revisado de encaixe](relatorio-resposta-review-wgpu-renderer.md#plano-revisado-de-encaixe).
