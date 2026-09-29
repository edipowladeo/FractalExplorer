# T035 — Integrar Config UI e renderer GPU em runtime único

## Contexto

O renderer GPU usa `winit::EventLoop`. No Windows, o `winit` exige que o
event loop seja criado e executado na thread principal. A Config UI atual usa
`eframe::run_native`, que também controla um event loop próprio. Tentar manter
o GPU na thread secundária para deixar a Config UI na thread principal causa o
panic:

```text
Initializing the event loop outside of the main thread
```

A correção imediata mantém o GPU funcionando na thread principal, mas deixa a
Config UI desabilitada simultaneamente no modo GPU. Este documento registra as
alternativas para resolver essa limitação sem usar `any_thread` como paliativo.

## Opções

### 1. Painel de configuração dentro da janela GPU — recomendada para a primeira entrega

Migrar o estado e os controles da Config UI para um painel `egui` renderizado
na própria janela GPU.

Vantagens:

- um único `winit::EventLoop` e uma única janela;
- nenhuma comunicação entre threads ou processos;
- reutiliza o `wgpu::Device`, surface e ciclo de redraw existentes;
- menor risco de sincronização e encerramento;
- configuração ao vivo pode gerar diretamente `AppEvent::ConfigurationChanged`.

Tradeoffs:

- muda a experiência visual da Config UI;
- exige integrar `egui-winit` e `egui-wgpu` ao renderer GPU;
- o painel compartilha o espaço da cena, salvo implementação de docking ou
  overlay redimensionável.

### 2. Segunda janela `winit` no mesmo event loop — recomendada se a janela separada for requisito

Criar uma janela de configuração adicional com `winit`, mantendo o renderer e
a Config UI sob o mesmo `ApplicationHandler`. O estado `egui` de cada janela
é atualizado e desenhado pelo mesmo ciclo.

Vantagens:

- preserva a experiência de uma janela separada;
- continua com apenas um event loop na thread principal;
- resize, fechamento e shutdown podem ser coordenados pelo mesmo controlador.

Tradeoffs:

- exige gerenciar dois estados `egui`, duas superfícies e dois ciclos de
  renderização;
- aumenta a complexidade de foco, DPI, fechamento e recuperação de surface;
- requer separar a Config UI de `eframe::run_native` e usar as camadas
  `egui-winit`/`egui-wgpu` diretamente.

### 3. Config UI em processo separado

Executar a Config UI como outro processo. Cada processo cria seu próprio event
loop na sua thread principal; a configuração é enviada por IPC, arquivo
temporário ou socket local.

Vantagens:

- cada processo pode continuar usando seu framework de janela atual;
- isolamento de falhas e de ciclo de vida;
- não exige integrar `eframe` ao renderer GPU.

Tradeoffs:

- IPC, reconexão, sincronização e propagação de encerramento;
- distribuição e descoberta de um segundo executável;
- maior latência e mais estados inconsistentes possíveis;
- solução desproporcional para a configuração local da aplicação.

### 4. `EventLoopBuilderExtWindows::any_thread` — não recomendada

Permitir explicitamente um event loop fora da thread principal no Windows.

Vantagens:

- mudança pequena no código atual;
- preserva as duas janelas e os dois loops existentes.

Tradeoffs:

- contorna uma restrição de portabilidade do `winit`;
- deixa o comportamento dependente do backend e do sistema operacional;
- pode falhar em outros caminhos de janela, DPI, composição ou shutdown;
- não resolve a coordenação conceitual de dois event loops;
- não deve ser usada como arquitetura padrão.

## Decisão recomendada

Implementar primeiro a opção 1, com um painel de configuração dentro da janela
GPU. Depois, se a janela separada for necessária, evoluir para a opção 2. A
opção 3 fica reservada para uma necessidade explícita de isolamento de processo;
a opção 4 deve permanecer rejeitada.

## Critérios de aceitação

- nenhum `EventLoop` é criado fora da thread principal no Windows;
- a Config UI GPU altera configuração e o renderer aplica a mudança sem
  reiniciar a aplicação;
- resize, foco, redraw e fechamento são coordenados;
- o fechamento do renderer encerra corretamente a UI e vice-versa;
- CPU continua funcionando sem suporte GPU;
- testes com fake runtime cobrem eventos e efeitos sem abrir janelas;
- HILT cobre uma janela GPU, alteração de configuração, resize e shutdown.

## Estratégia TDD

1. RED: testar no fake runtime a criação do painel/janela de configuração,
   propagação de `ConfigurationChanged` e efeitos de fechamento.
2. GREEN: integrar o estado da UI ao event loop principal com a menor mudança
   possível.
3. REFACTOR: separar estado, tradução de eventos e desenho da UI dos detalhes
   do renderer.
4. HILT: confirmar o comportamento em uma sessão GPU real no Windows.
