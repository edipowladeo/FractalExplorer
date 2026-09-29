# T026 — Precisão para zoom profundo

## Objetivo

Investigar e introduzir a precisão necessária para que a navegação e o cálculo continuem distinguindo pixels em zoom profundo, preservando o alinhamento entre camadas corrigido em `f1eed38`.

## Diagnóstico atual

As coordenadas complexas, `delta`, escala da câmera e coordenadas dos tiles usam `f64`. Com a configuração atual (`delta ≈ 4/1400` e zoom inicial `8`), restam aproximadamente 40 bits de zoom binário antes que um deslocamento de um pixel possa deixar de alterar uma coordenada próxima de `-1,5`.

O branch `multiprecisao` possui cálculo com `Fixed`, mas ainda cria a coordenada e o passo do tile a partir de `f64`; isso não recupera os bits já perdidos na câmera.

## Diretriz

1. Manter tela, dimensões, índices, framebuffer e paleta nos tipos atuais (`i32`, `u32`, `usize`, `u64` e `f64` quando estritamente visual).
2. Migrar para um tipo multipreciso a posição complexa da câmera, o passo complexo por pixel, a coordenada e o `delta` de cada tile, e os cálculos de criação de camada adjacente.
3. Fazer conversões tela→plano em multiprecisão; arredondar para `i32` somente ao posicionar pixels na tela.
4. Para cálculo direto, executar `c` e a órbita `z` na precisão escolhida. Para perturbation, manter `c_ref` e a órbita de referência multiprecisos e usar um delta local de menor custo somente enquanto for seguro, com fallback.
5. Separar profundidade de zoom da escala visual, preferindo mantissa+expoente ou `complex_per_pixel` multipreciso a um único `f64` crescente.
6. O formato de copiar/restaurar localização não é contrato final: cada versão deve apenas garantir que sua própria saída possa ser lida pela sua própria entrada sem reduzir prematuramente a precisão.

## Verificação esperada

- Testes de regressão para pan, zoom e criação de camadas em uma profundidade que exceda a resolução útil de `f64`.
- Testes de round-trip: localização copiada → configuração → visão restaurada, sem perda além da precisão selecionada.
- Testes de alinhamento entre câmera, camadas e tiles mantidos para as representações nova e `f64`.
- Testes de cálculo direto e de perturbation com fallback em pontos profundos representativos.
