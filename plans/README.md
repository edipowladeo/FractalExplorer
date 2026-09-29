# Organização dos planos

Os planos de implementação ficam versionados dentro de `plans/`, separados
por branch ou linha de trabalho. Isso permite trabalhar em várias branches sem
misturar planos incompletos.

## Convenção

- `plans/renderer-gpu/`: planos da branch `gpu-renderer` e da implementação do
  renderizador GPU.
- `plans/multiprecisao/`: planos da linha de trabalho de multiprecisão e zoom
  profundo.
- `plans/develop/`: fila geral de desenvolvimento, decisões compartilhadas e
  planos que não pertencem a uma branch específica.

Uma pasta específica pode existir mesmo quando a branch correspondente não
está presente neste clone. O nome da pasta deve seguir a branch ou a linha de
trabalho que o plano acompanha.

Documentos de processo do repositório, como `AGENTS.md`, `README.md` e
`WINDOWS_SETUP.md`, não são planos e permanecem na raiz. Revisões históricas
ou inventários também permanecem fora de `plans/` até serem convertidos em um
plano identificável.

Ao criar ou mover um plano:

1. coloque o arquivo na pasta da branch correspondente;
2. atualize todos os links relativos que apontem para ele;
3. confirme com uma varredura que nenhum plano ficou fora de `plans/` e que não
   há referências antigas quebradas.
