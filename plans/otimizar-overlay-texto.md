# Plano: otimizar overlay para receber texto

## Objetivo

Permitir que os overlays recebam texto sem rasterizar cada linha para RGBA nem
enviar uma textura nova ao GPU a cada quadro. O conteúdo textual deve continuar
sendo atualizado por quadro quando necessário, mas a conversão de caracteres
para textura e o upload de pixels devem acontecer apenas quando a fonte, o
tamanho ou o conjunto de glifos mudar.

## Diagnóstico atual

- `gpu_window.rs` chama `append_text_overlay_line` para cada linha do overlay.
- `append_text_overlay_line` calcula dimensões, chama `debug_overlay_upload` e
  cria um `ImageUpdate` com o bitmap completo da linha.
- O caminho atual representa texto como `OverlayPrimitive::Image`, portanto
  cada linha é tratada como uma textura independente.
- Mesmo com revisão por hash, a rasterização CPU, a alocação do `Vec<u8>` e a
  preparação das atualizações acontecem durante a preparação do frame.
- A geometria também é reconstruída como um quad por linha.

## Alternativas

### A. Atlas de glifos persistente + comandos de texto (recomendada)

Manter no renderer uma textura atlas com os glifos da fonte bitmap usada hoje.
O overlay passa apenas uma lista de caracteres/glifos, posição, cor, escala e
camada. Um buffer de instâncias ou vértices referencia regiões do atlas. O
atlas é criado uma vez e só recebe upload quando um glifo ainda não existente
for requisitado.

Vantagens:

- elimina a rasterização de cada string e o upload de uma imagem por linha;
- preserva a fonte atual e permite atualizar somente posições/conteúdo;
- permite agrupar todas as linhas em uma chamada de desenho;
- torna o custo por quadro aproximadamente proporcional ao número de
  caracteres, não à área dos bitmaps.

Cuidados:

- definir contrato de ciclo de vida para o atlas quando o device/surface for
  recriado;
- decidir se o texto será ASCII limitado, Unicode com fallback ou uma fonte
  real no futuro;
- separar comandos de texto do contrato genérico de imagens para não forçar
  backends que não suportam GPU text.

### B. Uma textura bitmap única para todos os overlays

Rasterizar todas as linhas em um único bitmap e enviar no máximo uma textura
por quadro.

É uma melhoria intermediária simples, reduzindo a quantidade de updates e
draw calls, mas ainda paga a rasterização, alocação e upload de toda a área a
cada atualização. Deve ser usada apenas como baseline de medição ou fallback.

### C. Instâncias de glifos com atlas fixo

Variação mais explícita de A: gerar uma tabela fixa para os caracteres aceitos
(`glyph()` atual), criar o atlas no startup e enviar somente instâncias por
quadro. É a opção mais previsível para os overlays de diagnóstico atuais e
evita a complexidade de um atlas dinâmico.

### D. Renderizar texto fora do pipeline GPU

Usar a janela/UI nativa para desenhar texto. Pode reduzir o trabalho do
renderer, mas mistura dois sistemas de composição e não é adequado enquanto o
overlay precisa acompanhar exatamente o frame GPU, o viewport e o backend
atual.

## Direção proposta

Começar por C, com uma abstração de comandos que permita evoluir para A:

1. Introduzir `TextOverlay`/`TextRun` no contrato de renderização, contendo
   texto ou glifos, posição, dimensões, cor e camada.
2. Manter o atlas bitmap persistente no estado de `GpuWindow`/backend, com os
   glifos atualmente suportados por `renderer::glyph`.
3. Converter strings em uma lista compacta de instâncias (índice do glifo e
   posição), sem produzir RGBA por linha.
4. Criar um pipeline de texto que amostre o atlas e desenhe todas as
   instâncias dos overlays em lote.
5. Reutilizar buffers com capacidade crescente e escrever apenas a região
   necessária a cada frame.
6. Manter `OverlayPrimitive::Image` para envelopes e outros bitmaps; texto não
   deve mais passar pelo caminho de `ImageUpdate`.
7. Adicionar fallback temporário para o caminho antigo quando o backend não
   oferecer o pipeline de texto.

## Sequência TDD

### RED

- Testar que a preparação de uma linha textual produz comandos de glifo e
  nenhum `ImageUpdate`.
- Testar posicionamento, avanço fixo de 6 pixels, quebras de linha e
  caracteres sem glifo.
- Testar que duas preparações consecutivas reutilizam o atlas e não criam
  uploads de glifos novamente.
- Instrumentar contadores esperados: uploads de atlas, instâncias e draw
  calls.

### GREEN

- Implementar o contrato mínimo de texto, atlas fixo e geração de instâncias.
- Ligar primeiro um único grupo de overlay, mantendo os demais no caminho
  antigo até a cobertura estar estável.
- Implementar o pipeline WGPU e o fallback.

### REFACTOR

- Extrair a política de layout de texto do backend.
- Consolidar métricas e remover alocações por linha.
- Revisar nomes, ownership e ciclo de vida do atlas junto com a recriação do
  device.

## Critérios de aceitação

- Nenhuma linha textual usa `debug_overlay_upload` no caminho GPU otimizado.
- Em frames com conteúdo textual estável, não há `ImageUpdate` de texto nem
  upload de pixels do atlas.
- Todas as linhas atuais continuam visualmente legíveis e no mesmo
  posicionamento.
- O conteúdo alterado aparece no frame seguinte sem recriar a textura do
  atlas.
- Testes unitários cobrem layout, cache/atlas e comandos gerados.
- Métricas registram instâncias, uploads e draw calls para comparação antes e
  depois.

## Questões para decisão antes da implementação

1. O conjunto atual de caracteres ASCII da função `glyph()` é suficiente para
   os overlays, ou precisamos de UTF-8 completo desde o início?
2. A prioridade é minimizar uploads, draw calls ou latência CPU de preparação?
3. O contrato genérico deve expor `TextRun` para todos os backends ou o texto
   pode ser uma capacidade opcional específica do GPU?
4. Devemos preservar o caminho de imagem como fallback para testes/headless?

## Verificação manual

Após RED, GREEN e REFACTOR com testes passando, iniciar `cargo run --bin
sprite-demo` no worktree para validação visual do usuário, conforme o HIL
global do repositório.
