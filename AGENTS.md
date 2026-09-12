# Instruções para agentes

Estas instruções se aplicam a todo agente que atuar neste repositório.

## Controle global de HIL

- [x] HIL tests

Este é o único checkbox de HIL do repositório e é controlado exclusivamente pelo usuário.

- Quando estiver marcado, o agente deve disparar a aplicação ao concluir uma alteração funcional nova e estável, para que o usuário faça a validação manual.
- Uma alteração estável é aquela que concluiu RED, GREEN e REFACTOR, com os testes passando.
- Alterações sem mudança funcional, como documentação ou refatorações internas sem mudança de comportamento, não exigem esse disparo.
- Quando estiver desmarcado, o agente não deve disparar automaticamente a aplicação por causa de uma alteração funcional.
- O agente nunca deve marcar, desmarcar ou mover esse checkbox.

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
6. Toda nova tarefa solicitada pelo usuário deve ser tratada como se tivesse sido adicionada ao início de `TODO`, tornando-se a primeira tarefa ativa: analisar seu agrupamento com as demais tarefas, executar seguindo o fluxo TDD e registrá-la em `DONE` assim que os critérios de conclusão forem atendidos.

## Testes e validação manual

1. Escrever testes ao longo da implementação e executar os testes unitários rápidos durante os ciclos RED e GREEN.
2. Não executar verificações pesadas, benchmarks ou validações demoradas sem solicitação explícita do usuário.
3. Registrar no `TASKS.md` quais testes unitários foram executados e quais verificações manuais ficaram a cargo do usuário.

## Execução da aplicação na sessão

Para disparar a aplicação diretamente na sessão do terminal:

1. Usar um terminal PowerShell com diretório de trabalho na raiz do projeto.
2. Executar `cargo run --bin sprite-demo` diretamente.
3. Manter o processo do terminal em execução enquanto a janela do renderizador estiver aberta.
4. Considerar a aplicação encerrada quando a janela for fechada e o comando retornar ao prompt.

Esse procedimento não depende da configuração de depuração do VS Code nem do LLDB. A validação visual fica a cargo do usuário enquanto a sessão permanece ativa.

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
