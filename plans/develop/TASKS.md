# Lista de tarefas

## TODO

### T037 — Centralizar logs e gerar dump ao final da execução

- **Objetivo:** fazer todos os logs da aplicação passarem por uma classe de
  logging comum, persistir os registros em arquivo de texto e gerar um dump
  final quando a execução terminar.
- **Escopo:** substituir chamadas diretas de saída/log por uma API única;
  preservar saída no console quando configurado; registrar nível, timestamp,
  thread e contexto do evento; definir caminho e política de rotação/limite do
  arquivo; garantir flush e dump final em encerramento normal e em falhas
  controladas.
- **Critérios de aceitação:** nenhum caminho de runtime relevante usa saída
  direta fora da classe de logging; `InvalidFrame` e demais erros GPU incluem
  frame, recurso e contexto no arquivo; o arquivo é fechado e o dump final é
  emitido ao encerrar; testes cobrem concorrência básica, flush, encerramento
  e falha de escrita sem travar o renderer.
- **Dependências:** reutilizar a telemetria e os eventos de frame do T029,
  sem alterar o contrato de renderização nem reabrir o caminho legado.
- **Decisão de agrupamento:** mantida como tarefa separada de T029 porque
  atravessa CPU, GPU, UI e inicialização/encerramento da aplicação.
- **Estado:** aguardando confirmação do agrupamento e posterior execução pelo
  fluxo RED/GREEN/REFACTOR.

## DONE

