# Lista de tarefas

Fila de trabalho do rewrite do FractalExplorer em Rust. A ordem de `TODO` é deliberada: agentes devem atuar sempre na primeira tarefa e nunca puxar itens diretamente de `BACKLOG`.

## TODO

### T001 — Definir contratos do núcleo e a estratégia de testes

- **Objetivo:** transformar os requisitos levantados nas referências legadas em contratos verificáveis para o núcleo Rust, começando por representação numérica, ponto no plano complexo, câmera, fórmula de escape-time e resultado de iteração.
- **Critérios de aceitação:** contratos documentados; casos-limite identificados; estratégia de testes unitários, propriedades e referências numéricas definida; escopo de qualquer agrupamento com esta tarefa confirmado pelo usuário antes da implementação.
- **TDD:** iniciar com testes RED para os contratos escolhidos, depois GREEN e REFACTOR.
- **Dependências:** nenhuma.

## BACKLOG

### T002 — Criar o workspace Rust e os módulos iniciais do core

- Definir a estrutura de crates/módulos compartilhados e comandos de build/teste.
- Depende de T001.

### T003 — Implementar o motor CPU determinístico

- Renderizar amostras do plano complexo em uma representação independente de plataforma.
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

### T008 — Avaliar backend WebGL e abstração de renderização

- Selecionar a arquitetura e validar a primeira integração multiplataforma.
- Depende de T007.

### T009 — Avaliar backend Metal e targets desktop/mobile

- Validar limites de portabilidade, ciclo de vida e recuperação de recursos.
- Depende de T008.

### T010 — Investigar deep zoom e precisão arbitrária

- Avaliar aritmética de precisão estendida, perturbation e aceleração; não assumir que os wrappers legados já resolvem o problema.
- Depende de T001 e T007.

### T011 — Adicionar persistência, importação e exportação

- Locais/configurações salvos, importação de coordenadas e exportação de imagem/vídeo.
- Depende de T004 e T008.

## DONE

_Nenhuma tarefa concluída._
