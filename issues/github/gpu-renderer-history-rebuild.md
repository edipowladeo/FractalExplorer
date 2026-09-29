# Reconstrução do histórico do PR do renderizador GPU

Data: 2026-09-29

## Sintoma

O PR original do renderizador GPU apresentava a mensagem de que a branch não
podia ser rebased por conflitos. A API do GitHub, porém, retornava
simultaneamente:

- `mergeable: true`;
- `mergeable_state: clean`;
- `rebaseable: false`;
- 122 commits exclusivos de `gpu-renderer` em relação a `master`.

Isso caracteriza um problema de topologia/histórico, não um conflito de
conteúdo na árvore final. A branch original continha dois commits de merge de
`master`, o que impedia o rebase automático do GitHub mesmo quando o resultado
era mergeável.

## O que foi feito

1. Consultamos o PR e o histórico local/remoto antes de alterar a branch.
2. Mantivemos a branch original `gpu-renderer` intacta como referência e
   preservamos suas alterações locais não commitadas no worktree original.
3. Criamos `rebuild/gpu-renderer-history` a partir de `origin/master`.
4. Recriamos os 122 commits exclusivos em sequência linear, mantendo para cada
   commit a árvore, a mensagem, o autor e as datas originais.
5. Os dois commits que eram merges foram representados como commits lineares
   com seus snapshots originais. Não houve squash e não foi criado um único
   commit de merge.
6. Comparamos a árvore final da branch reconstruída com `origin/gpu-renderer`.

## Evidências

- commits recriados: `122`;
- commits de merge na nova sequência: `0`;
- comparação da árvore final com `origin/gpu-renderer`: sem diferenças;
- novo histórico começa em `origin/master` e termina no snapshot exato da
  branch original.

Os hashes mudaram porque os pais dos commits foram reconstruídos. Isso é
esperado em uma reconstrução de histórico; os commits individuais, suas
mensagens e seu conteúdo foram preservados.

## O que resolveu o problema

A investigação final mostrou duas condições distintas:

1. os merges antigos explicavam a topologia problemática e justificavam a
   reconstrução linear;
2. o bloqueio persistente do rebase no PR veio do limite do GitHub para
   `Rebase and merge`: uma PR com mais de 100 commits não pode usar esse método.

A branch linear com 123 commits continuou retornando `mergeable: true` e
`rebaseable: false`, confirmando que a linearização sozinha não era suficiente.
Por isso o histórico foi dividido em PRs sequenciais, cada uma abaixo do limite,
sem squash e sem perder os commits.

## Dicas para evitar recorrência

- Criar a branch de trabalho a partir de `origin/master` atualizado.
- Manter cada PR de rebase com menos de 100 commits; para históricos maiores,
  dividir em PRs sequenciais e preservar a ordem dos commits.
- Antes de publicar a branch, usar `git fetch origin` e rebasear a branch
  privada sobre `origin/master`.
- Evitar fazer merge de `master` dentro da branch que será usada como origem de
  um PR quando a rebaseabilidade da PR for necessária.
- Depois do primeiro push ou da abertura da PR, não reescrever o histórico sem
  coordenação; se a branch já tiver merges, criar uma branch linear de
  reconstrução documentada.
- Conferir antes de abrir a PR:
  `git rev-list --merges origin/master..HEAD` deve estar vazio quando a política
  exigir histórico linear.
- Conferir a árvore antes e depois de uma reconstrução com:
  `git diff --exit-code origin/branch-antiga HEAD`.
- Manter a identidade da conta que cria a PR separada da identidade dos
  autores dos commits quando as regras de aprovação dependerem do autor da PR.
