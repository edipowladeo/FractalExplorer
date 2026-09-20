# FractalExplorer

A fresh implementation in Rust. The previous implementations are considered legacy; this rewrite is not based on which legacy project was most complete.

## Intended targets

- Web browsers with WebGL
- Android
- Windows
- Linux
- Apple platforms with Metal

The intended direction is a shared Rust fractal core with platform and rendering backends. Backend architecture and dependencies have not been selected, and no Rust implementation has been added yet.

## First sprite renderer prototype

The first implementation uses a Rust CPU-side framebuffer and a Windows-capable `minifb` window to display a centered sprite. After installing the Rust toolchain, run these commands from the repository root:

```powershell
# Confirmar a toolchain ativa
rustup show active-toolchain

# Formatar o código
cargo fmt

# Verificar a compilação sem executar
cargo check

# Executar os testes
cargo test

# Executar a aplicação de demonstração
cargo run --bin sprite-demo
```

O comando `cargo run --bin sprite-demo` abre a janela de demonstração e exibe o sprite azul centralizado. Pressione `Esc` para fechar a janela.

Para compilar em modo release:

```powershell
cargo build --release
```

Para limpar os artefatos de compilação:

```powershell
cargo clean
```

O protótipo é acompanhado pela `T001` em [TASKS.md](TASKS.md) e permanece em `TODO` até que testes, refactor e verificação visual sejam concluídos.

## Dynamic configuration UI

The optional `native-ui` feature opens an `egui/eframe` configuration window alongside the `minifb` renderer window:

```powershell
cargo run --features native-ui --bin sprite-demo
```

The configuration window discovers TOML properties recursively. Boolean values use checkboxes, integer and floating-point values use spinners, and unsupported scalar values are displayed as read-only. Changes are sent to the renderer through a channel; compatible changes update the current frame, while width and height changes recreate the renderer window.

The default build remains framebuffer-only:

```powershell
cargo run --bin sprite-demo
```

Use a complete Windows native toolchain to run the optional UI feature. The project keeps `eframe` behind the feature so the configuration model and core tests can run without native GUI linker dependencies.

## Legacy implementations

All existing fractal implementations are legacy references for the Rust rewrite. The complete former C++/SFML implementation of this repository, including its history, is preserved on `legacy` at `79fe6f36479ff12dee0dfe15a122cea02ffb592c`. `OpenCL` points to the same commit and does not need a duplicate worktree.

The reference worktrees are under:

```text
C:\Users\lidia\edipinhu\legacy-fractal-explorer-worktrees
```

The following selection comes from the account-wide review of 28 repositories, including private repositories. It covers the legacy projects and useful branch variants. Links below open the local worktrees in this workspace layout. The complete feature matrix, including the newer local Kotlin/Swing implementation, is in [feature_comparison.md](../feature_comparison.md).

| Worktree | Source ref / pinned commit | Valuable content for the rewrite |
|---|---|---|
| [FractalExplorer](../../../legacy-fractal-explorer-worktrees/FractalExplorer) | `legacy` / `79fe6f3` | C++/SFML tile engine; separate draw, allocation and retention regions; mouse navigation, palettes, fullscreen and debug controls. OpenCL host/kernels are experimental, not an integrated fractal backend. |
| [Fractal](../../../legacy-fractal-explorer-worktrees/Fractal) | `origin/master` / `6d34cc8` | Earlier C++ tile/task design, coarse-to-fine tile calculation, cursor-pivot navigation, predefined palettes and shader interpolation. Runtime iteration/filtering controls reveal behavior to improve. |
| [FractalExplorerAndroid](../../../legacy-fractal-explorer-worktrees/FractalExplorerAndroid) | `origin/master` / `0128720` | Touch pinch/drag controls, OpenGL ES display, packed iteration textures, pooled resources, queued uploads and Android surface/activity lifecycle handling. |
| [FractalExplorerAndroid-lifecycle](../../../legacy-fractal-explorer-worktrees/FractalExplorerAndroid-lifecycle) | `origin/new` / `080c6d8` | Application-owned resources, texture regeneration, pool clearing and removal of portrait locking. An unfinished lifecycle refactor to study alongside the original Android implementation. |
| [FractalExplorerPC](../../../legacy-fractal-explorer-worktrees/FractalExplorerPC) | `origin/master` / `1d78194` | Desktop backend interfaces, unsigned integer iteration-buffer textures, separate palette buffer, smooth camera and keyboard navigation. |
| [MandelbrotPC](../../../legacy-fractal-explorer-worktrees/MandelbrotPC) | `origin/master` / `522822e` | Julia windows selected from Mandelbrot, multiple windows, rotation, mouse controls, formula abstraction, palette selection/density/phase/speed, shader interpolation and complex-number tests. Includes Newton/integer-power formula experiments and a separate GPU compute demo. |
| `fractalExplorer_kotlin2026` | local working tree | Kotlin/JVM Swing explorer with 1–5-component floating-point expansions, progressive 1/8–1x passes, 2x supersampling, cursor-centered zoom, drag preview, clipboard locations and configurable iterations/palette. |
| [MandelbrotPC-numeric-experiment](../../../legacy-fractal-explorer-worktrees/MandelbrotPC-numeric-experiment) | `origin/tipos_especiais_classe` / `e118fe1` | Numeric abstraction integrated into math, camera and formula code. `Arb` wraps `Double`; this is a design experiment, not arbitrary-precision arithmetic. |

`FractalExplorer` uses the existing `legacy` branch. The other worktrees use detached HEADs pinned to the commits above, so fetching a repository will not change the reviewed reference files. The original repositories remain under `C:\Users\lidia\edipinhu\repo`; worktrees share their Git object stores. Use Git's `worktree move` command if relocating them again.

Full commit IDs, source refs and local paths are recorded in [legacy-worktrees.json](legacy-worktrees.json).

### How to use the references

- Define shared numerical results and tile scheduling from the existing CPU implementations, then validate the new renderer backends against those results.
- Preserve the union of input, camera, formula and palette capabilities across projects. The [fractal feature review](fractal_projects_review.md) distinguishes connected features from partial controls, experiments and TODOs.
- Study Android context recovery and desktop thread/resource ownership together. Known lifetime, queue-growth and shader-compatibility bugs should be fixed rather than reproduced.
- Treat GPU computation, Newton/generalized formulas and numeric abstractions as experiments requiring validation. None establishes an existing Rust, WebGL, Metal or arbitrary-precision implementation.

### Other projects and variants considered

- `Mandelbrot-android`, `gmt-api`, `Solucoes` and `uav-projects` are empty; there is no implementation to check out.
- `Fractal/rev8` duplicates the matching C/C++ source at the root of its repository. It is already present inside the `Fractal` worktree.
- Android's `local_history.patch` is already inside its worktree; it is an experimental integer-buffer shader conversion.
- Six MandelbrotPC branch tips are ancestors of its current `master`. The divergent `intermediateBranch` contains texture/type/worker cleanup, and the two `tipos_experimentais_*` branches contain standalone numeric-wrapper playgrounds. Their history remains available in the source repository; no additional user-facing renderer capability was identified that warrants another worktree.
- `enxame` has a reusable 3D orbit-camera example and `artemis_orbit` has general animation/visualization structure. These are optional ideas for future 3D scope, not missing capabilities of the current 2D fractal explorer.
- The nonogram/OCR project, hardware/CAD tools, financial APIs and developer-tooling forks do not provide a specific fractal core or target-platform rendering backend. Their descriptions remain in the [general project review](../../general_projects.md).

Selection and source inspection are static; the legacy applications were not built or run. Worktree paths, registrations and pinned revisions were verified locally.
