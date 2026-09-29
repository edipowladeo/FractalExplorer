# Relatório de resposta ao review do backend WGPU

## Escopo e conclusão executiva

O arquivo `review-wgpu-renderer.md` foi recebido de `C:\Users\edipo\Downloads` e
migrado para a raiz deste worktree (`gpu-renderer`). Este relatório cruza cada
um dos sete pontos do review com:

- `plans/renderer-gpu/T029-gpu-textures-shaders.md`;
- o estado registrado em `plans/develop/TASKS.md`;
- a implementação atual em `src/render/graphics/wgpu.rs`,
  `src/render/gpu.rs` e `src/gpu_window.rs`.

Conclusão: os pontos 2 e 3 são essencialmente legado e devem acompanhar a
remoção planejada; os pontos 4, 5, 6 e 7 são lacunas reais que precisam entrar
no trabalho do T029; o ponto 1 está parcialmente resolvido pelo frame comum,
mas ainda exige decisões e um fechamento explícito antes da remoção do
fallback legado. O ponto 1b depende de confirmar o estado operacional desejado.

## Classificação consolidada

| Ponto | Classificação | Situação no plano atual | Ação recomendada |
|---|---|---|---|
| 1. Pipelines duplicadas e paridade | legado → novo | Parcialmente coberto nos Passos 3, 6 e 7 | Fechar telemetria e confirmar o contrato de overlays antes do Passo 7 |
| 1b. Textura duplicada/dessincronia | compartilhado/transitório | Não há critério explícito de exclusão operacional | Confirmar se os caminhos podem coexistir para o mesmo conteúdo |
| 2. Modelos de textura paralelos | só legado | Coberto implicitamente pela remoção do legado no Passo 7 | Não implementar otimização separada |
| 3. `content_hash` sem cache legado | só legado | Contradito pelo cache/revisão do modelo novo e coberto pela limpeza do Passo 7 | Não investir no cache antigo |
| 4. Validação duplicada em `submit` | só novo | Não coberto explicitamente | Adicionar ao Passo 6 como refactor/teste unitário imediato |
| 5. `draw` como god-method | só novo | Não coberto explicitamente | Adicionar ao Passo 6 como refactor após estabilizar a execução |
| 6. Erros `String` no `WgpuContext` | compartilhado | Não coberto explicitamente | Adicionar ao Passo 6/8, alinhando com `DeviceError` |
| 7. Vértices acoplados a tipos legados | compartilhado/misto | Parcialmente coberto pelo objetivo de confinar/remover legado | Adicionar ao Passo 7, ou antecipar ao refactor do Passo 6 |

## 1. Pipelines duplicadas e paridade legado → novo

### Já coberto pelo plano atual

O plano já prevê:

- responsabilidades comuns, incluindo overlays e instrumentação, no Passo 3;
- envelope e timing overlay no modelo comum de imagens/revisões no Passo 6;
- remoção da composição lógica duplicada e dos caminhos verticais no Passo 7;
- conformidade de overlays, inclusive posição, ancoragem e ordem, no Passo 0;
- instrumentação adicional do adaptador e separação de aquisição, composição,
  submissão, `poll` e apresentação no diagnóstico operacional do Passo 6.

O código atual confirma que a lacuna de camadas extras foi parcialmente
resolvida: `PreparedFrame`/`RenderFrame` carregam `OverlayPrimitive`, o runtime
GPU cria `GpuRenderTarget<WgpuGraphicsDevice>` e submete o frame comum por
`submit_prepared_frame`. Portanto, envelope e overlays já possuem representação
no caminho novo; não devem ser reintroduzidos como parâmetros específicos do
`encode_frame`.

### Decisão incorporada ao plano

`OverlayPrimitive` é a representação definitiva de envelope, texto e ferramentas
de debug. O caminho novo (`WgpuGraphicsDevice::draw`) deve ser mantido como a
única composição do frame; os parâmetros equivalentes de `encode_frame` serão
removidos junto com o caminho legado.

Isso deve ser tratado como paridade de observabilidade, não como motivo para
duplicar o modelo de camadas. O plano agora exige telemetria detalhada na
fronteira do destino GPU, com avaliação explícita de custo/complexidade e
possibilidade de desativação ou remoção em build de release.

### Decisão registrada

`envelope` e `overlay` do `encode_frame` estão definitivamente substituídos por
`OverlayPrimitive` no contrato comum. O plano também passa a incluir um
rasterizador de texto otimizado por renderer, reaproveitando do contrato comum
posição e conteúdo, ou uma lista de strings.

## 1b. Risco de duas identidades de textura

### Evidência atual

O caminho legado usa `GpuTextureStore`, `TextureKey` e `GpuTileTexture`. O
caminho novo usa `TextureHandle` e `WgpuTexture`. O runtime ainda contém o
fallback legado, embora, após a inicialização do destino comum, a submissão
retorne pelo `GpuRenderTarget`.

### Situação no plano

O plano prevê coexistência temporária, mas não define um teste ou uma
invariante que impeça os dois caminhos de receberem a mesma imagem ao mesmo
tempo. A remoção do legado no Passo 7 elimina o risco estruturalmente, mas até
lá o comportamento precisa ser explícito.

### Lacuna para sua resposta

Precisamos confirmar uma destas opções:

1. o caminho legado é apenas fallback de inicialização e nunca deve compor o
   mesmo frame que o target novo;
2. ambos podem ser usados simultaneamente para comparação/diagnóstico;
3. ambos podem existir, mas cada um recebe conteúdos distintos.

Se a opção 1 for a intenção, recomendo registrar uma asserção/teste de estado
e tratar o caminho legado como temporário, sem investir em sincronização de
cache entre os dois modelos.

## 2. Dois modelos de textura paralelos

### Já coberto pelo plano atual

É um efeito da migração incremental. O Passo 5 introduziu `WgpuTexture` e
handles; o Passo 6 usa o `TextureResourceCache`; o Passo 7 remove os
consumidores legados. A diferença de encapsulamento não representa uma
regressão funcional isolada.

### Decisão

Classificar como **já coberto pela limpeza final**. Não implementar uma revisão
de encapsulamento em `GpuTileTexture` enquanto ele estiver destinado à remoção.

## 3. `content_hash` sem cache no caminho legado

### Já coberto pelo plano atual

O plano novo especifica identidade/revisão de imagens, atualizações separadas
do frame e reutilização de recursos. O estado do T029 registra cache do
`GpuRenderTarget` e descarte de imagens fora do frame. Isso atende ao objetivo
no caminho novo.

### Decisão

Classificar como **a resolver pela remoção do legado**, não como item a
implementar agora. Conectar o cache antigo seria trabalho descartável e poderia
prolongar indevidamente a convivência dos dois modelos.

## 4. Validação inconsistente em `submit`

### Evidência atual

`submit` chama `validate_commands` para ordem, mas mantém inline a validação de:

- limites de `WriteBuffer`;
- dimensões e tamanho de bytes de `WriteTexture`;
- existência da textura, dimensões não nulas e opacidade de `DrawTexture`.

### Situação no plano

O Passo 5 exige que handles, descritores, command lists e ordem permaneçam
testáveis com mock. O Passo 6 exige testes do comportamento de recursos. Porém,
o plano não explicita a extração da validação de conteúdo nem uma suíte própria
para ela.

### A adicionar ao plano

Adicionar ao Passo 6, antes de novos refactors de apresentação:

- extrair `validate_command_resources` (nome provisório);
- manter `submit` como orquestração: valida ordem, valida recursos, aplica
  writes e desenha;
- testar buffer inexistente, overflow de offset, textura inexistente,
  dimensões incompatíveis, payload com tamanho incorreto, dimensões nulas,
  `NaN`, infinito e opacidade fora de `[0, 1]`;
- confirmar que nenhuma chamada à fila ocorre quando a validação falha.

## 5. `draw` como god-method

### Evidência atual

`WgpuGraphicsDevice::draw` concentra descoberta dos draws, resize/recriação da
composição, preparação do ring de vértices, aquisição com retry, composição,
pass de apresentação, submissão, `present` e atualização de
`surface_initialized`.

### Situação no plano

O plano cobre funcionalmente essas responsabilidades nos Passos 5, 6 e 8,
mas não exige explicitamente decomposição estrutural. O review identifica um
risco de manutenção real e independente da remoção do legado.

### A adicionar ao plano

Adicionar ao refactor do Passo 6, depois de haver testes de comportamento:

- `ensure_composition_target`;
- `acquire_frame_with_retry`;
- `record_composition_pass`;
- `record_present_pass`;
- uma etapa clara para submissão/apresentação e atualização do estado.

Cada helper deve manter erro em `DeviceError` e receber apenas os dados que
usa. Os testes devem continuar exercitando a máquina de estado via mock/fakes;
não é necessário criar testes unitários dependentes de uma GPU real.

## 6. Erros stringly-typed em `WgpuContext`

### Evidência atual

`WgpuContext::poll_device`, `initialize` e `create_surface` retornam
`Result<_, String>`, enquanto a fronteira `GraphicsDevice` usa `DeviceError`.
O contexto é dependência do caminho WGPU novo, portanto esse problema não
desaparece com `encode_frame`.

### Situação no plano

O contrato do Passo 5 já define erros portáveis (`GfxError` no texto do plano;
`DeviceError` no código atual), e o Passo 8 exige testes de falha de
inicialização, perda de surface/device e fallback. Falta ligar explicitamente
as APIs de contexto a esse erro tipado.

### A adicionar ao plano

Adicionar ao Passo 6 ou ao início do Passo 8:

- definir variantes/conversões para inicialização, superfície, device e
  `poll`, preservando a causa quando possível;
- eliminar `Result<_, String>` da API pública de `WgpuContext`;
- adaptar logs no runtime para formatar o erro tipado na borda;
- testar mapeamento de erro e preservar fallback/recuperação.

A decisão entre ampliar `DeviceError` e criar `WgpuContextError` deve respeitar
a regra de que tipos WGPU não atravessam o adaptador.

### Decisão registrada

Adotar `WgpuContextError`, com um campo genérico para preservar a causa
específica como texto ou objeto pequeno quando isso for útil. A API pública de
`WgpuContext` não deve continuar expondo `Result<_, String>`.

## 7. Vértices acoplados a tipos legados

### Evidência atual

`render_vertices_for_commands` converte cada `Command::DrawTexture` em um
`TileDrawCommand` com `TextureKey` artificial e `content_hash: 0` apenas para
reutilizar `tile_vertices_for_commands`. A matemática de conversão para
vértices não depende de identidade de textura nem de hash.

### Situação no plano

O Passo 7 exige eliminar o acoplamento residual aos caminhos verticais e
remover tipos legados. O Passo 6 ainda é um ponto seguro para preparar essa
separação, pois o adaptador novo já está ativo.

### A adicionar ao plano

Extrair uma função neutra, por exemplo `tile_vertices_for_rects`, que receba
somente posição, tamanho, viewport e eventualmente opacidade. Então:

- `tile_vertices_for_commands` pode adaptar comandos legados à função neutra;
- `render_vertices_for_commands` pode usar a mesma função sem criar
  `TileDrawCommand`/`TextureKey` falsos;
- os testes de vértices devem cobrir as duas entradas e manter os mesmos
  resultados;
- depois, a função e os tipos legados podem ser removidos no Passo 7 quando
  não houver mais consumidores.

## Plano revisado de encaixe

### Passo 6 — prioridade imediata

1. Fechar o caminho comum GPU com telemetria equivalente à antiga.
2. Extrair e testar a validação de conteúdo de comandos.
3. Decompor `draw` após estabilizar o comportamento.
4. Tipar os erros do `WgpuContext`.
5. Confirmar que overlays/envelope são somente `OverlayPrimitive` no caminho
   novo.

### Passo 7 — limpeza e remoção

1. Remover `encode_frame`, `GpuTextureStore`, `GpuTileTexture`, `TextureKey`
   e compatibilidades que não tenham consumidores.
2. Remover o cache legado não utilizado.
3. Remover a adaptação de vértices via tipos legados.
4. Executar busca estática para confirmar que não restou caminho vertical
   duplicado.

### Passo 8 — recuperação e critérios operacionais

1. Exercitar erros tipados de inicialização, surface/device e `poll`.
2. Confirmar que o fallback não executa os dois caminhos para o mesmo frame.
3. Validar a preservação do estado lógico e a reconstrução de recursos.

## Estado das decisões

As questões de representação de overlays/envelope, coexistência dos caminhos,
nível de telemetria, tipagem de erros e prioridade foram respondidas e
incorporadas ao plano T029. O arquivo separado de perguntas abertas deixou de
ser necessário.

## Evidências consultadas

- `review-wgpu-renderer.md` — review externo recebido;
- `plans/renderer-gpu/T029-gpu-textures-shaders.md` — plano incremental e gates;
- `plans/develop/TASKS.md` — checkpoint atual do T029 e verificações registradas;
- `src/render/graphics/wgpu.rs` — `WgpuContext`, `WgpuGraphicsDevice`,
  validação, `draw` e adaptação de vértices;
- `src/render/gpu.rs` — integração do target GPU ao contrato comum;
- `src/gpu_window.rs` — runtime, overlays, telemetria e fallback legado.

## Estado da entrega

- Review original movido de `C:\Users\edipo\Downloads` para a raiz deste
  worktree.
- Relatório de resposta criado neste arquivo.
- Nenhum código funcional foi alterado.
- Não foram executados testes, pois esta entrega é documental e não altera
  comportamento.
