# Plano de implementação: multiprecisão e perturbação

## Objetivo

Investigar e portar para o FractalExplorer Rust os dois caminhos existentes em
`C:\Users\edipo\repo\fractalrenderers\fractalExplorer_kotlin2026`:

1. cálculo direto de todos os pixels com aritmética multiprecisão;
2. método da perturbação, com órbita de referência em alta precisão e cálculo
   dos deltas por pixel.

A implementação Rust não deve reproduzir `MultiDouble` baseado em vários
`Double`. A representação escolhida é um ponto fixo com limbs `u64`, começando
por `Fixed<1>`, `Fixed<2>` e `Fixed<N>`:

```rust
struct Fixed<const N: usize> {
    limbs: [u64; N],
}
```

## Tarefa ativa

### T018 — Investigar e planejar a multiprecisão u64

**Critérios de aceitação da investigação:**

- identificar no Kotlin o cálculo direto multiprecisão por pixel;
- identificar a seleção da referência, a órbita de referência e a fórmula da
  perturbação;
- registrar snippets portáveis e diferenças relevantes para limbs `u64`;
- definir a ordem de implementação de `Fixed<1>`, `Fixed<2>` e `Fixed<N>`;
- definir testes unitários e testes de conformidade contra a implementação
  atual `f64`.

## Resultado da investigação do Kotlin

### Aritmética `MultiDouble`

Arquivo: `src/main/kotlin/ArbitraryPrecisionMath.kt`.

`MultiDouble` guarda componentes `Double` em ordem do mais significativo para
os resíduos:

```kotlin
class MultiDouble private constructor(
    private val components: DoubleArray
) {
    val precision: Int get() = components.size
    val high: Double get() = components[0]
}
```

As operações constroem termos e normalizam a expansão. Soma usa `twoSum`, e o
produto usa `twoProduct`/Dekker para conservar o erro do produto binário:

```kotlin
val result = twoProduct(a, b)
if (result.product != 0.0) terms += result.product
if (result.error != 0.0) terms += result.error
```

No Rust, a responsabilidade equivalente será carregar, somar e normalizar
limbs inteiros, com carry/borrow explícito. O formato precisa definir antes:

- qual limb é o menos significativo;
- posição do ponto binário;
- representação do sinal;
- arredondamento/truncamento ao reduzir a precisão;
- comportamento de overflow, underflow e conversões.

### Cálculo direto multiprecisão de cada pixel

Arquivos: `MandelbrotOrbit.kt` e `MandelbrotRenderer.kt`.

O cálculo de uma órbita multiprecisão é a recorrência padrão de Mandelbrot,
mantendo `zr`, `zi`, `cr` e `ci` na mesma precisão:

```kotlin
var zr = MultiDouble.zero(precision)
var zi = MultiDouble.zero(precision)

while (iteration < maxIterations) {
    val zr2 = zr.square()
    val zi2 = zi.square()
    if ((zr2 + zi2).compareToDouble(4.0) > 0) {
        return MandelbrotOrbitResult(iteration, escaped = true)
    }

    val newZr = zr2 - zi2 + cr
    zi = zr * zi * 2.0 + ci
    zr = newZr
    iteration++
}
```

Esse é o caminho de referência para o Rust: cada pixel usa `Fixed<N>` do
começo ao fim, sem converter coordenadas profundas para `f64`. A primeira
versão deve preservar a semântica de escape `norm_squared > 4` e retornar o
limite de iterações para pontos que não escapam.

### Seleção e órbita de referência

Arquivos: `MandelbrotReference.kt` e `MandelbrotReferenceOrbit.kt`.

O Kotlin amostra uma grade, calcula todos os candidatos em multiprecisão e
seleciona o ponto com mais iterações; em empate, escolhe o mais próximo do
centro normalizado:

```kotlin
if (
    bestResult == null ||
    result.iterations > bestResult.iterations ||
    result.iterations == bestResult.iterations && distance < bestDistance
) {
    best = candidate
    bestResult = result
    bestDistance = distance
}
```

A órbita selecionada é armazenada inteira em arrays de alta precisão:

```kotlin
real[iteration + 1] = zr2 - zi2 + reference.coordinateReal
imag[iteration + 1] = zr * zi * 2.0 + reference.coordinateImag
```

No Rust, a órbita deve usar `Vec<Fixed<N>>` para as partes real e imaginária,
com `valid_iterations` para distinguir uma órbita interrompida por escape de
uma órbita completa.

### Método da perturbação

Arquivo: `MandelbrotRenderer.kt`, função
`calculatePixelPerturbation`.

Para um pixel `c = c_ref + delta_c`, o Kotlin converte apenas `delta_c` para
`Double` e propaga `delta_z` ao redor da órbita de referência:

```kotlin
val deltaCReal = (cr - referenceCandidate.coordinateReal).toDouble()
val deltaCImag = (ci - referenceCandidate.coordinateImag).toDouble()

var deltaZReal = 0.0
var deltaZImag = 0.0
for (iteration in 0 until settings.maxIterations) {
    val referenceReal = reference.real[iteration].toDouble()
    val referenceImag = reference.imag[iteration].toDouble()

    val real = referenceReal + deltaZReal
    val imag = referenceImag + deltaZImag
    if (real * real + imag * imag > 4.0) return iteration

    val nextReal =
        2.0 * (referenceReal * deltaZReal - referenceImag * deltaZImag) +
        (deltaZReal * deltaZReal - deltaZImag * deltaZImag) + deltaCReal
    val nextImag =
        2.0 * (referenceReal * deltaZImag + referenceImag * deltaZReal) +
        2.0 * deltaZReal * deltaZImag + deltaCImag

    deltaZReal = nextReal
    deltaZImag = nextImag
}
```

O Kotlin rejeita o resultado direto e faz fallback quando os deltas não são
finitos ou ficam grandes demais. O teste de regressão compara os dois métodos
em uma grade `100x100` e exige zero diferenças de iteração. No Rust, essa
comparação será o teste de conformidade principal, mas a política de precisão
precisa ser explícita: a órbita de referência deve permanecer em `Fixed<N>`;
somente a variante otimizada do delta poderá usar uma representação reduzida
se os testes demonstrarem que ela é segura.

### Fallback configurável para inspeção de artefatos

O comportamento será exposto como uma opção funcional de configuração, por
exemplo `perturbation_fallback`:

```toml
# Default intencional: permite observar artefatos quando a perturbação perde estabilidade.
perturbation_fallback = false
```

- `false` (default): não substituir automaticamente o resultado instável pelo
  cálculo direto; registrar/produzir o pixel da perturbação para inspeção dos
  artefatos;
- `true`: fazer fallback para full multiprecision quando a perturbação perder
  estabilidade, mantendo a imagem robusta.

O teste deve cobrir os dois modos e verificar que a opção realmente altera o
  caminho de execução, sem esconder a causa da instabilidade.

## Decisões de design para `Fixed<const N: usize>`

### Representação inicial proposta

- `limbs[0]` será o limb menos significativo, para simplificar carry e
  multiplicação escolar;
- inteiros sem sinal armazenarão o módulo; o sinal será separado na struct;
- a escala binária (`FRACTION_BITS`) será fixa por tipo lógico;
- `Fixed<1>`, `Fixed<2>` e `Fixed<N>` compartilharão a mesma API;
- operações intermediárias usarão largura dupla (`u128`) para `u64 * u64`;
- resultados que excederem a capacidade serão tratados por uma política
  documentada e testada, não por conversão silenciosa para `f64`.

Exemplo de forma pública esperada:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Fixed<const N: usize> {
    pub(crate) negative: bool,
    pub(crate) limbs: [u64; N],
}

impl<const N: usize> Fixed<N> {
    pub fn zero() -> Self;
    pub fn from_i64(value: i64) -> Self;
    pub fn from_f64(value: f64) -> Self;
    pub fn add(self, rhs: Self) -> Self;
    pub fn sub(self, rhs: Self) -> Self;
    pub fn mul(self, rhs: Self) -> Self;
    pub fn square(self) -> Self;
    pub fn compare(self, rhs: Self) -> core::cmp::Ordering;
}
```

O layout exato, a escala e a política de overflow devem ser fechados no
primeiro ciclo RED antes de conectar o tipo ao Mandelbrot.

## Ordem de implementação

1. **RED da aritmética:** testes para zero, sinal, soma/subtração, carry,
   borrow, multiplicação, quadrado, comparação e conversão.
2. **GREEN `Fixed<1>`:** implementação mínima com a mesma escala fixa.
3. **REFACTOR:** separar helpers de limbs, normalização e operações assinadas;
   manter os testes verdes.
4. **RED/GREEN `Fixed<2>`:** validar resultados que atravessam o limite do
   primeiro limb e multiplicação que exige limbs superiores.
5. **REFACTOR genérico:** promover a implementação para `Fixed<N>` sem duplicar
   código e adicionar casos para `N = 1`, `2` e valores maiores.
6. **Mandelbrot direto:** adaptar ponto complexo, viewport e cálculo de todos
   os pixels para `Fixed<N>`; comparar com `f64` em regiões rasas.
7. **Órbita de referência:** selecionar candidatos e armazenar a órbita em
   `Fixed<N>`.
8. **Perturbação:** implementar a recorrência de `delta_z`, detecção de
   instabilidade e fallback ao cálculo direto.
9. **Conformidade:** comparar full multiprecision e perturbação pixel a pixel
   em grades pequenas e em uma localização de deep zoom determinística.

## Testes planejados

- propriedades de soma/subtração com carry e borrow;
- produto `u64::MAX * u64::MAX` e propagação por vários limbs;
- identidade `x.square() == x * x`;
- comparação de números positivos, negativos e zero;
- conversão de coordenadas inteiras e frações conhecidas;
- Mandelbrot: origem permanece dentro e ponto `2 + 0i` escapa;
- igualdade de iterações entre `Fixed<1>` e `f64` em pontos representativos;
- igualdade entre cálculo direto e perturbação quando a perturbação for aceita;
- fallback obrigatório quando a perturbação for numericamente instável.

## Riscos e pontos a decidir antes do código

- Um ponto fixo puro precisa de escala suficiente para representar tanto a
  região inicial quanto deep zoom; `N` aumenta a faixa/precisão, mas a escala
  deve ser compatível com os limites da viewport.
- A fórmula de perturbação do Kotlin usa `Double` nos deltas. Isso não é um
  porte integral da multiprecisão; deve ser tratado como otimização opcional e
  validado contra o caminho direto.
- A conversão de uma coordenada arbitrária para `f64` não pode ser usada para
  decidir pixels em deep zoom.
- A aritmética assinada, arredondamento e overflow precisam ser definidos antes
  de medir desempenho ou escolher a quantidade padrão de limbs.

## Evidências da investigação

- Full multiprecision: `ArbitraryPrecisionMath.kt`, `MandelbrotOrbit.kt` e
  `MandelbrotRenderer.kt` (`calculatePixelArbitraryPrecision`).
- Referência: `MandelbrotReference.kt` e `MandelbrotReferenceOrbit.kt`.
- Perturbação: `MandelbrotRenderer.kt` (`calculatePixelPerturbation`).
- Conformidade: `MandelbrotPerturbationRegressionTest.kt`, grade `100x100`,
  exigindo zero diferenças de iteração.
- Não foram executados testes pesados ou benchmarks nesta etapa; a validação
  de implementação ficará para os ciclos TDD correspondentes.
