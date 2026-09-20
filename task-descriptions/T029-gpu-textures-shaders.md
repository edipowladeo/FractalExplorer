# T029 — Renderização integral na GPU com texturas e shaders

## Objetivo

Substituir o caminho atual de composição CPU (`Sprite::draw_into_scaled` sobre um
framebuffer `u32`) por um renderer baseado em GPU, usando texturas para os tiles
e shaders para transformar, compor e apresentar a imagem.

O caminho CPU deve permanecer disponível durante a migração como referência de
correção e fallback. A primeira entrega deve preservar navegação, camadas,
tiles progressivos, overlays e dumps de instrumentação.

O renderer legado deve permanecer ativável por uma flag de configuração, pois o
backend GPU inicial será implementado para Windows. A API pública do renderer
deve continuar multiplataforma: tipos e contratos comuns não podem depender de
`minifb`, `winit` ou `wgpu`; essas dependências devem ficar isoladas nos
backends de plataforma.

## Plano de implementação

1. **Definir a fronteira do backend**
   - Introduzir um contrato de renderer com operações de inicialização,
     redimensionamento, upload/atualização de textura, composição e apresentação.
   - Separar o estado da janela do estado do backend gráfico.
   - Manter `CpuRenderer` como implementação de referência.
   - Introduzir seleção explícita, por exemplo `renderer.backend = "cpu"` ou
     `renderer.backend = "gpu"`, com `cpu` como fallback seguro.
   - Manter o contrato comum compilável nas plataformas sem o backend Windows.

2. **Escolher e configurar a API GPU**
   - Avaliar `wgpu` como backend multiplataforma, aproveitando Vulkan/Metal/DX12
     quando disponíveis e fallback compatível.
   - Criar dispositivo, fila, surface, swapchain/configuração de surface e
     tratamento de resize.
   - Não acoplar o orquestrador de tiles a tipos específicos de `wgpu`.
   - Isolar `winit`/`wgpu` em módulo/backend Windows; o núcleo deve depender
     somente de traits e estruturas próprias do projeto.

3. **Migrar o formato dos tiles para texturas**
   - Definir o formato de pixel e o contrato de cor/paleta.
   - Criar uma textura GPU por tile ou um atlas, conforme os testes de uso de
     memória e custo de atualização indicarem.
   - Implementar cache de texturas por identidade/versão do tile.
   - Invalidar e liberar recursos quando tiles saírem da alocação.

4. **Implementar shaders de composição**
   - Criar shader de vértices para um quad por tile, com transformação de tela.
   - Criar fragment shader para amostragem da textura, escala e composição das
     camadas na ordem atual.
   - Definir filtros, coordenadas UV, transparência e tratamento de bordas para
     evitar seams entre tiles.
   - Manter overlays em uma etapa compatível; migrá-los para GPU se o backend
     atual impedir a apresentação integral pela GPU.

5. **Integrar com o ciclo do renderer**
   - Trocar o loop que chama `draw_into_scaled` por comandos de render pass.
   - Fazer uploads somente para sprites novos ou alterados.
   - Preservar a semântica de tiles incompletos e a composição progressiva.
   - Adaptar apresentação e dumps para ler a superfície GPU somente quando
     explicitamente necessário, evitando readback por frame.

6. **Instrumentar e comparar desempenho**
   - Registrar separadamente tempo de upload, preparação de comandos, execução
     GPU quando disponível e apresentação.
   - Manter eventos tipados de frame; não filtrar desempenho por texto livre.
   - Adicionar modo de comparação CPU/GPU para a mesma câmera e conjunto de tiles.

7. **Estabilizar fallback e recursos**
   - Se a inicialização GPU falhar, usar o renderer CPU com diagnóstico claro.
   - Tratar perda/recriação de surface, resize e encerramento sem acessar recursos
     destruídos.
   - Documentar requisitos de driver e limitações por plataforma.

## Estratégia TDD

- **RED:** testes de contrato para transformação de tile, ordem de camadas,
  UV/bordas, invalidação de textura, resize e seleção CPU/GPU; teste de regressão
  que reproduza a composição CPU atual.
- **GREEN:** implementar primeiro os tipos/contratos e um backend mínimo; depois
  upload de uma textura, um quad, múltiplos tiles e camadas.
- **REFACTOR:** encapsular recursos GPU, reduzir uploads, remover acoplamento ao
  framebuffer CPU e manter o caminho de fallback legível.
- Testes de pixels devem aceitar tolerância explícita quando a GPU produzir
  diferenças de arredondamento; invariantes de coordenadas e ordem devem ser
  exatos.

## Critérios de aceitação

- A composição normal dos tiles ocorre na GPU por texturas e shaders.
- O loop não executa `Sprite::draw_into_scaled` para compor cada tile no caminho
  GPU.
- O caminho legado CPU pode ser selecionado explicitamente por flag e continua
  funcional como fallback.
- A seleção de backend e os contratos comuns compilam sem dependências Windows;
  a implementação GPU específica fica isolada por backend/condicional de
  plataforma.
- Navegação, camadas, resize, tiles progressivos e overlays preservam o
  comportamento existente.
- CPU e GPU produzem imagens equivalentes dentro da tolerância documentada.
- Falha de inicialização GPU usa fallback CPU sem panic.
- Testes unitários e de integração passam; benchmark/validação visual ficam
  separados dos testes rápidos.
- Alteração concluída com refactor, commit e push no branch do worktree GPU.

## Riscos e decisões pendentes

- `minifb` pode não expor uma surface adequada para `wgpu`; nesse caso será
  necessário substituir apenas a camada de janela/apresentação, sem mover a
  lógica de orquestração.
- A primeira integração de surface será Windows, mas não deve contaminar o
  contrato multiplataforma nem impedir futuros backends macOS, Linux, Web ou
  mobile.
- Renderização do fractal diretamente no fragment/compute shader é uma etapa
  posterior: esta tarefa começa acelerando a composição dos tiles existentes.
- A estratégia por textura individual versus atlas deve ser decidida após medir
  quantidade de tiles, uploads e limites de bind groups.
