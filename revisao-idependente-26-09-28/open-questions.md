# Open questions — review do renderer WGPU

Perguntas que precisam de resposta antes de fechar o plano do T029. O contexto
técnico completo está em [relatorio-resposta-review-wgpu-renderer.md](relatorio-resposta-review-wgpu-renderer.md).

## Nível de telemetria

### Contexto

O caminho legado informa separadamente composição, apresentação e finalização
do encoder. O caminho novo já registra eventos agregados, mas o review aponta
que a migração pode reduzir a observabilidade necessária para diagnosticar
stalls e distinguir custo de codificação, submissão e apresentação.

### Opções

- **Preservar a telemetria detalhada:** manter eventos de início/fim para
  composição, apresentação e codificação também no destino novo.
  - Prós: continuidade das métricas, diagnóstico mais preciso e comparação
    direta entre caminhos.
  - Contras: contrato de instrumentação maior e mais manutenção nos adaptadores.
- **Aceitar telemetria agregada:** considerar suficientes os eventos de
  submissão/apresentação já existentes.
  - Prós: contrato menor e implementação mais simples.
  - Contras: perde separação entre etapas e pode dificultar investigar stalls
    ou regressões de driver.

**Resposta:**

Contexto detalhado: [seção 1 — Pipelines duplicadas e paridade](relatorio-resposta-review-wgpu-renderer.md#1-pipelines-duplicadas-e-paridade-legado--novo).

## Modelo de erro

### Contexto

`WgpuContext` ainda retorna `Result<_, String>` em inicialização, criação de
surface e `poll`, enquanto a fronteira `GraphicsDevice` usa `DeviceError`.
Depois da remoção do legado, essa inconsistência continuará no caminho novo.

### Opções

- **Ampliar `DeviceError`:** representar falhas de contexto diretamente no erro
  portável do dispositivo.
  - Prós: uma única fronteira de erro para runtime, fallback e testes.
  - Contras: o enum pode acumular casos específicos de inicialização WGPU e
    ficar menos coeso.
- **Criar `WgpuContextError`:** manter detalhes no adaptador e converter para
  `DeviceError` somente na fronteira `GraphicsDevice`.
  - Prós: separação mais clara entre erro específico e erro portável; melhor
    preservação de causa e contexto.
  - Contras: exige conversões adicionais e pode dificultar diagnóstico se a
    conversão descartar informação.

**Resposta:**

Contexto detalhado: [seção 6 — Erros stringly-typed](relatorio-resposta-review-wgpu-renderer.md#6-erros-stringly-typed-em-wgpucontext).
