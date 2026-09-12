# Plano de implementação — UI dinâmica para configuração TOML

## Objetivo

Adicionar uma janela de configuração gerada dinamicamente a partir das propriedades do TOML, mantendo inicialmente a janela de renderização baseada em `minifb`.

## Arquitetura proposta

```text
Janela egui/eframe
        │
        │ comandos de configuração
        ▼
     canal de mensagens
        │
        ▼
Thread da janela minifb ──> recalcula o fractal
```

- `AppConfig` continua sendo a fonte tipada da configuração.
- `toml::Value` será usado para percorrer a configuração recursivamente.
- Cada propriedade escalar será representada por um `ConfigField` com caminho, valor e tipo de controle.
- A UI não acessará diretamente o estado do renderer; alterações serão enviadas por mensagens.
- O renderer poderá invalidar o resultado atual quando uma propriedade relevante for alterada.

## Escopo inicial

- Booleanos: `Checkbox`.
- Inteiros: spinner com decremento e incremento.
- Pontos flutuantes: spinner com passo configurável inicialmente fixo.
- Strings, enums, arrays e outros tipos: visíveis como somente leitura.
- Caminhos de propriedades: formato pontilhado, por exemplo `renderer.debug.reduced_viewport`.

## Etapas TDD

### 1. Modelo de propriedades

- RED: testar descoberta recursiva, caminhos pontilhados e classificação dos tipos.
- GREEN: implementar `ConfigUi`, `ConfigField` e `ControlKind` usando `toml::Value`.
- REFACTOR: separar travessia TOML, conversão de valores e estado editável.

### 2. Atualização tipada

- RED: testar alternância de booleanos, incremento/decremento numérico e rejeição de tipos incompatíveis.
- GREEN: implementar operações de edição e aplicação de volta ao `AppConfig`.
- REFACTOR: centralizar validação, limites e passos numéricos.

### 3. Janela de configuração

- RED: testar a transformação de cada `ConfigField` no widget esperado.
- GREEN: criar a janela `egui/eframe` com checkbox, spinner e indicação de somente leitura.
- REFACTOR: organizar a UI por tabelas/seções TOML e adicionar rolagem.

### 4. Comunicação com o renderer

- RED: testar que uma alteração gera a mensagem correta e que mensagens desconhecidas são ignoradas com segurança.
- GREEN: conectar a janela de configuração e a thread do renderer por canais.
- REFACTOR: definir comandos e eventos independentes da biblioteca de UI.

### 5. Re-renderização

- RED: testar a invalidação para alterações que afetam o resultado.
- GREEN: reiniciar ou atualizar o cálculo quando a configuração mudar.
- REFACTOR: separar alterações que exigem novo cálculo das que afetam apenas apresentação.

## Critérios de aceitação

- As propriedades booleanas do TOML aparecem como checkboxes.
- As propriedades inteiras aparecem como spinners inteiros.
- As propriedades de ponto flutuante aparecem como spinners decimais.
- Propriedades aninhadas são descobertas sem codificar seus nomes individualmente.
- Propriedades ainda não suportadas continuam visíveis e não editáveis.
- Alterações feitas na UI chegam ao renderer sem acesso compartilhado não controlado.
- Os testes unitários passam após o refactor.
- A verificação manual confirma que as duas janelas abrem, respondem a eventos e encerram corretamente.

## Riscos e decisões pendentes

- Dois loops de eventos (`eframe` e `minifb`) podem exigir execução em threads distintas.
- O comportamento de encerramento e limpeza das duas janelas precisa ser validado no Windows.
- Alterações em largura/altura podem exigir recriação da janela ou do framebuffer.
- Se a convivência entre as bibliotecas se mostrar frágil, a alternativa será migrar ambas as janelas para `eframe`.

## Verificação e entrega

- Executar somente os testes unitários rápidos durante RED e GREEN.
- Aplicar `cargo fmt` e conferir `git diff --check`.
- Fazer a validação visual manual da aplicação com o usuário.
- Registrar testes e validações no T018 de `TASKS.md`.
- Criar commits pequenos e focados no branch `feature/dynamic-config-ui`.
- Conferir diff, branch e remoto antes do push.
- Só mover T018 para `DONE` depois de testes GREEN, refactor, commit e push confirmados.
