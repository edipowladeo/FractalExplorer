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
   - **Exceção:** alterações exclusivamente de configuração ou de documentação de configuração não exigem o fluxo TDD.
2. Se a estratégia de testes não for óbvia, discutir a estratégia com o usuário antes de implementar.
3. Uma tarefa só pode ser considerada concluída depois de: testes em estado **GREEN**, refactor realizado, commit criado e push enviado ao remoto.
4. Mover a tarefa para `DONE` imediatamente após todas as condições do item anterior serem atendidas.

## Ordem e seleção de tarefas

1. Instruções dadas diretamente pelo usuário devem ser implementadas fora do fluxo de `TASKS.md`.
2. Só criar ou promover uma tarefa em `TASKS.md` quando o usuário pedir explicitamente.
3. Para tarefas explicitamente criadas ou promovidas, nunca selecionar diretamente de `BACKLOG`.
4. Para tarefas explicitamente criadas ou promovidas, sempre atuar na primeira tarefa listada em `TODO`.
5. Antes de iniciar uma tarefa explicitamente criada ou promovida, analisar as demais tarefas em `TODO` e confirmar com o usuário se alguma deve ser agrupada com ela. Não iniciar a implementação até essa confirmação quando houver um agrupamento plausível.
6. Se `TODO` estiver vazio, não puxar tarefas de `BACKLOG` automaticamente; pedir ao usuário para promover explicitamente uma tarefa para `TODO`.
7. Manter a ordem das tarefas e registrar dependências ou decisões relevantes na própria tarefa.
8. Toda tarefa explicitamente criada ou promovida pelo usuário deve ser tratada como se tivesse sido adicionada ao início de `TODO`, tornando-se a primeira tarefa ativa: analisar seu agrupamento com as demais, executar seguindo o fluxo TDD e registrá-la em `DONE` assim que os critérios de conclusão forem atendidos.

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

### Política de sincronização entre branches

- Regra de isolamento: **1 agente = 1 worktree = 1 branch**. Um agente não
  deve trabalhar simultaneamente em mais de um worktree ou branch, e dois
  agentes não devem compartilhar a mesma branch de trabalho.
- A `develop` é mantida por um único agente neste fluxo. Outros agentes devem
  criar uma branch própria a partir do estado atualizado e trabalhar em um
  worktree próprio.
- `develop` é uma branch longa, porém exclusiva do agente responsável por ela;
  não fazer rebase nem force-push nela depois que commits forem publicados.
- Para atualizar `develop` com `master`, usar merge explícito:

  ```powershell
  git fetch origin
  git switch develop
  git merge --no-ff origin/master
  git push origin develop
  ```

- Para PRs de `develop` para `master`, preferir merge commit. Evitar rebase-and-
  merge e squash-and-merge nessa branch longa, pois podem gerar divergências
  históricas em PRs posteriores.
- Branches `feature/*` podem ser rebaseadas enquanto privadas. Depois do
  primeiro push ou uso por outro agente, não reescrever seu histórico sem
  coordenação.
- Após o merge de uma PR, a PR é fechada pelo GitHub. Uma nova PR só deve ser
  criada quando `develop` tiver commits novos em relação a `master`.
- Antes de abrir ou atualizar uma PR, verificar a divergência e a possibilidade
  de merge:

  ```powershell
  git fetch origin
  git rev-list --left-right --count origin/master...origin/develop
  gh pr view <numero> --json mergeable,mergeStateStatus
  ```
