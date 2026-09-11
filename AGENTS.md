# Instruções para agentes

Estas instruções se aplicam a todo agente que atuar neste repositório.

## Fluxo obrigatório de desenvolvimento

1. Sempre aplicar o fluxo completo de TDD:
   - **RED**: escrever ou ajustar um teste que falha e confirmar a falha pela razão esperada.
   - **GREEN**: implementar o mínimo necessário para fazer o teste passar.
   - **REFACTOR**: melhorar o design, a legibilidade e a manutenção sem alterar o comportamento; confirmar que os testes continuam passando.
2. Se a estratégia de testes não for óbvia, discutir a estratégia com o usuário antes de implementar.
3. Uma tarefa só pode ser considerada concluída depois de: testes em estado **GREEN**, refactor realizado, commit criado e push enviado ao remoto.
4. Mover a tarefa para `DONE` imediatamente após todas as condições do item anterior serem atendidas.

## Ordem e seleção de tarefas

1. Nunca selecionar uma tarefa diretamente de `BACKLOG`.
2. Sempre atuar na primeira tarefa listada em `TODO`.
3. Antes de iniciar a primeira tarefa, analisar as demais tarefas em `TODO` e confirmar com o usuário se alguma deve ser agrupada com ela. Não iniciar a implementação até essa confirmação quando houver um agrupamento plausível.
4. Se `TODO` estiver vazio, não puxar tarefas de `BACKLOG` automaticamente; pedir ao usuário para promover explicitamente uma tarefa para `TODO`.
5. Manter a ordem das tarefas e registrar dependências ou decisões relevantes na própria tarefa.

## Atualização da lista

- Usar `TASKS.md` como fonte da fila de trabalho.
- `TODO` contém apenas a fila ativa e ordenada.
- `BACKLOG` contém ideias ou tarefas ainda não promovidas para execução.
- `DONE` contém somente tarefas com testes GREEN, refactor, commit e push confirmados.
- Ao trabalhar em uma tarefa, registrar critérios de aceitação e evidências de verificação.

## Commits e colaboração

- Fazer commits pequenos, focados e com mensagem clara.
- Não misturar alterações não relacionadas à tarefa atual.
- Antes do push, verificar o diff, os testes e o branch/remoto de destino.
- Se commit ou push não puder ser feito por falta de acesso, registrar o bloqueio e não mover a tarefa para `DONE`.
