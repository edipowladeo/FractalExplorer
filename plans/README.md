# Organização dos planos

Os planos de implementação ficam versionados dentro de `plans/`, separados
por branch ou linha de trabalho. Isso evita misturar planos incompletos entre
agentes e worktrees.

## Convenção

- `plans/text-overlay/`: plano específico da branch `text-overlay`.
- `plans/renderer-gpu/`: planos da implementação do renderer GPU.
- `plans/multiprecisao/`: planos da linha de trabalho de multiprecisão e zoom profundo.
- `plans/develop/`: fila geral, decisões compartilhadas e planos sem branch específica.

Documentos de processo, reviews e incidentes permanecem fora de `plans/`.
Toda referência a um plano deve usar seu novo caminho relativo.
