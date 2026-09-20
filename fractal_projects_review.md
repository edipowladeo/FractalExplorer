mas # Fractal projects: legacy inventory for the Rust rewrite

Updated: 2026-09-09. Based on authenticated GitHub access, including private repositories, plus the local `fractalExplorer_kotlin2026` project. See [general_projects.md](../../general_projects.md) for account-wide coverage and unrelated projects.

**Decision already made:** build from scratch in Rust in `edipowladeo/FractalExplorer`, targeting WebGL, Android, Windows, Linux and Metal. Every pre-existing implementation below is legacy. This document preserves features and experiments; it does not recommend continuing any legacy architecture.

Status terminology: **implemented** means an active source path was inspected, **experimental** means standalone/partial code or a non-default variant, and **planned** means TODO/comment only. None of the programs or tests was run.

## Repository summary

| Repository | Visibility / reviewed revision | Quick description and completeness |
|---|---|---|
| **Fractal** | Private; `master` at `6d34cc8` | C++/SFML Mandelbrot explorer with tiled layers, CPU escape calculation, mouse/keyboard navigation, palette selection/cycling, interpolated shader coloring and diagnostic output. Old Code::Blocks/GCC project, committed object files and duplicated `rev8` source. Substantial experimental renderer; OpenCL call is commented out. |
| **FractalExplorer** | Private; rewrite `master` at `ff127b0`; legacy at `79fe6f3` | `master` now contains only the rewrite README. The preserved C++/SFML implementation adds window/debug controls, explicit draw/allocation/deallocation regions and OpenCL host/kernel experiments. Windows/Visual Studio project with bundled SFML. Startup runs an OpenCL test and waits for input before starting the explorer, so this is not a clean portable launch path. |
| **FractalExplorerAndroid** | Public; `master` at `0128720`, plus `origin/new` | Kotlin Android Mandelbrot explorer with pinch/drag navigation, progressive tiled CPU rendering, OpenGL ES coloring and resource pools. APK/AAB artifacts exist. Substantial mobile prototype with older build configuration, portrait restriction and known lifecycle/resource issues. |
| **FractalExplorerPC** | Public; `master` at `1d78194` | Kotlin/JVM LWJGL/GLFW Mandelbrot explorer, tiled CPU workers, integer iteration-buffer textures, separate palette buffer, smooth camera and keyboard navigation. Substantial desktop prototype; Windows dependencies, shader compatibility and packaging need work. |
| **MandelbrotPC** | **Private**; `master` at `522822e`, plus ten other remote branches | Broader Kotlin/LWJGL explorer: Mandelbrot-to-Julia windows, camera rotation, mouse controls, rich palettes, formula interface and additional formula implementations. Separate compute-shader demo. Important source of capabilities absent from the earlier review. Still experimental, with known focus/fullscreen/resource problems. |
| **Mandelbrot-android** | **Private**; empty | Description identifies a Mandelbrot viewer, but there are no commits or files. No implementation or unique feature is available to preserve. |
| **fractalExplorer_kotlin2026** | Local Kotlin/JVM project; working tree | Swing Mandelbrot explorer with 1–5-component floating-point expansion arithmetic, progressive 1/8–1x passes, final 2x supersampling, cursor-centered zoom, drag preview/pan, clipboard location exchange, configurable iterations/palette period and deep-zoom display. A focused CPU implementation with no GPU or mobile backend. |

There are **six populated fractal implementations**, plus the empty Android placeholder. `FractalExplorer` also holds the new rewrite starting point. The feature matrix is maintained in [feature_comparison.md](feature_comparison.md).

## 6. fractalExplorer_kotlin2026: Kotlin/Swing precision explorer

The local project is a Kotlin/JVM Swing application. Its renderer is CPU-based and renders a viewport through progressive 1/8, 1/4, 1/2 and 1x passes, then emits a final 2x supersampled image. A generation counter cancels stale background renders when the viewport or settings change.

- `ArbitraryPrecisionMath.MultiDouble` stores one to five binary64 components and implements compensated addition, subtraction, multiplication and division using error-free transforms. This extends usable precision beyond one `Double`, but it remains a floating-point expansion rather than arbitrary-precision or a proof of unlimited deep zoom.
- Mouse controls include left-click zoom in, right-click zoom out, wheel zoom around the cursor, and drag panning with a bitmap preview until release. Middle-click copies `x=[...];y=[...];zoom=[...]` to the clipboard; the location field parses the same format.
- Sliders expose click zoom factor, expansion precision, palette period and maximum iterations. The UI reports logarithmic zoom depth. Coloring is CPU-generated from normalized escape iterations and a periodic palette.

Completeness: the project has a Gradle Kotlin/JVM build and a README run instruction, but no test sources, GPU renderer, tile scheduler, mobile target or packaged release. Rendering is performed by daemon threads and the Swing panel receives completed images asynchronously. Treat the multi-component arithmetic and supersampling as useful references for the Rust core, with numerical accuracy and cancellation behavior still requiring dedicated tests.

## 1. Fractal: C++/SFML

Implemented paths in [main.cpp](../../../legacy-fractal-explorer-worktrees/Fractal/main.cpp), [Cjanela.cpp](../../../legacy-fractal-explorer-worktrees/Fractal/Cjanela.cpp), [Cpaleta.cpp](../../../legacy-fractal-explorer-worktrees/Fractal/Cpaleta.cpp) and [Ccelula.cpp](../../../legacy-fractal-explorer-worktrees/Fractal/Ccelula.cpp) show:

- CPU Mandelbrot escape calculation with normalized iteration values; tiled cells and magnification layers; coarse-resolution tile population and front/back task queues.
- Keyboard pan/zoom, mouse-wheel zoom around a cursor pivot, mouse dragging, and click-versus-drag handling for zoom interaction.
- Multiple predefined palettes plus a sine palette, next/previous palette controls, animated color cycling, and interpolated coloring in a fragment shader. Palette data is packed alongside tile data in extra texture rows.
- Runtime iteration-limit increase/decrease; comments state this affects newly created sublayers, so full reprocessing of existing tiles is not established.
- Texture-smoothing toggle, but existing-tile update code is commented out; this is a partial control rather than proven immediate full-view filtering.
- Diagnostic timings/task information and tile/layer management logic.

Completeness: Code::Blocks project with old SFML calls and external dependencies; no test suite found. A function named `calcula_matriz_iteracoes_opencl_temp` prepares buffers, but its host kernel invocation is commented out. Do not label this an active GPU fractal engine.

`rev8/` is worth recording as an embedded version, but all matching top-level `.c`, `.cpp` and `.h` files compared byte-identically with their `rev8` counterparts. No additional feature was found in that duplicate source set. `main.cpp.save-failed` and compiled artifacts are retained in Git but were not treated as independent working versions.

## 2. FractalExplorer: preserved C++ implementation and Rust home

The rewrite [README](README.md) records the target platforms. The old implementation is in the [local legacy worktree](../../../legacy-fractal-explorer-worktrees/FractalExplorer/ProjetoFractal) and at [legacy commit 79fe6f3](https://github.com/edipowladeo/FractalExplorer/tree/79fe6f36479ff12dee0dfe15a122cea02ffb592c/ProjetoFractal), also checked out at `C:\Users\lidia\edipinhu\legacy-fractal-explorer-worktrees\FractalExplorer`.

Besides the related C++ navigation/palette/tile behavior:

- `PropriedadesJanela` distinguishes drawing, allocation and deallocation regions, plus magnification ranges. Different retention bounds are useful reference behavior for avoiding repeated tile destruction/recomputation.
- Keyboard handlers include fullscreen, shader-use, cell/layer-debug and return-to-initial-view controls. Shader-use is captured by cells at creation, so toggling the setting is not proof that existing tiles instantly switch paths.
- Both shader-colored iteration textures and CPU-colored texture routines are present. Some alternate paths contain debugging output/workarounds; one full-resolution CPU routine overwrites its calculated color with white. Preserve the fallback concept, not that bug.
- OpenCL platform/device enumeration, host buffer execution and Mandelbrot kernels exist, but the tiled fractal host invocation is commented out. `main()` calls `testeOpenCL()` (a vector-add test), then `getchar()`, before `TarefaDesenho()`. The test assumes a second OpenCL platform, so startup depends on machine configuration.

**Branch finding:** `origin/OpenCL` and `legacy` resolve to the exact same commit, `79fe6f36479ff12dee0dfe15a122cea02ffb592c`. They are not two distinct implementations at their current tips. No code needs to be merged between those tips for feature preservation.

## 3. FractalExplorerAndroid

Source: [properties](../../../legacy-fractal-explorer-worktrees/FractalExplorerAndroid/app/src/main/java/com/exploradordefractais/FractalJanelaPropriedades.kt), [gesture view](../../../legacy-fractal-explorer-worktrees/FractalExplorerAndroid/app/src/main/java/com/exploradordefractais/android/MyGlSurfaceView.kt), [renderer](../../../legacy-fractal-explorer-worktrees/FractalExplorerAndroid/app/src/main/java/com/exploradordefractais/android/MyRenderer.kt), [iteration function](../../../legacy-fractal-explorer-worktrees/FractalExplorerAndroid/app/src/main/java/com/exploradordefractais/IteracoesNormalizada.kt).

- Double-precision CPU Mandelbrot calculation, normalized iterations, four workers and prioritized progressive layers. Defaults: 64×64 tiles, four extra preview layers, apparent pixel size 2, maximum 1,024 iterations.
- OpenGL ES 2 context, RGBA-packed iteration values and procedural sine coloring in the active shader; time-based palette animation.
- Pinch zoom and drag pan. Fling/double-tap handlers are stubs, not inertia or double-tap-zoom features.
- Pools for textures, iteration arrays, byte arrays and direct buffers. GL allocation/upload/deallocation queues with a frame-time budget; surface callback requests texture regeneration.
- Fullscreen portrait presentation on `master`; optional diagnostic text updated on interaction. Pause/resume forwarding exists.

Completeness limits: SDK 29-era build files, template tests only, documented runaway zoom tasks and texture lifetime/validity issues. The active shader's zero-iteration return leaves RGB uninitialized, and packed-iteration decoding needs validation. Existing release artifacts are evidence of past packaging, not proof that this source builds or behaves identically today.

**`origin/new`:** compared against `master`; 16 files differ. Adds application-owned `FractalResources`, centralizes pools/GL queues/window state, removes portrait locking, adds pool resizing/clearing and `regenAllTextures()`, and queues texture deletion through shared resources. This is an experimental lifecycle/orientation refactor with an out-of-memory TODO; do not describe rotation/context recovery as solved.

**[local_history.patch](../../../legacy-fractal-explorer-worktrees/FractalExplorerAndroid/local_history.patch):** experimental ES 3.2 unsigned-buffer shader conversion. Preserve as a reference; it is not the active Android rendering path.

## 4. FractalExplorerPC

Source: [window/core](../../../legacy-fractal-explorer-worktrees/FractalExplorerPC/src/main/kotlin/com/example/fractal/Janela.kt), [properties](../../../legacy-fractal-explorer-worktrees/FractalExplorerPC/src/main/kotlin/com/example/fractal/JanelaPropriedades.kt), [GLFW window](../../../legacy-fractal-explorer-worktrees/FractalExplorerPC/src/main/kotlin/com/example/fractal/windows/HelloWordWindow.kt), [palette](../../../legacy-fractal-explorer-worktrees/FractalExplorerPC/src/main/kotlin/com/example/fractal/Paleta.kt), [active fragment shader](../../../legacy-fractal-explorer-worktrees/FractalExplorerPC/src/shaders/RawTextureFragment.glsl).

- Tiled double-precision CPU Mandelbrot calculation; defaults: 128×128 tiles, five extra preview layers, apparent pixel size 1, maximum 1,024 iterations.
- CPU-count-minus-one workers, iteration-array pooling, background layer management and queued GL allocation/deallocation.
- `GL_R32UI` iteration buffer plus a separate 1,024-color RGBA palette buffer. Animated lookup-index cycling; palette scale/speed is partly hardcoded in the shader.
- WASD/arrows pan, Space/Z zoom, H resets the camera and Escape closes. Actual/desired camera interpolation includes logarithmic zoom smoothing. GLFW resize updates the viewport.
- Interfaces for backend textures, drawing and time, although core still imports GLFW. Diagnostic string generation exists without a visible text overlay implementation.

Completeness limits: Windows-only native dependency selection; relative `src/shaders` loading; wrapper properties but no wrapper launch scripts/JAR found; no source tests found. The active shader declares ES 3.2 while GLFW creates a desktop context without an ES request. The layer thread busy-loops; a single-core machine yields zero workers. Tile drawing enlarges rectangles 2% to hide seams; normalized iteration coloring adds a `+10` workaround.

## 5. MandelbrotPC: additional private features

Sources: [main window/actions](../../../legacy-fractal-explorer-worktrees/MandelbrotPC/src/main/kotlin/com/fractalExplorer/platformSpecific/windows/janelas/JanelaPrincipalFractal.kt), [application/window creation](../../../legacy-fractal-explorer-worktrees/MandelbrotPC/src/main/kotlin/com/fractalExplorer/platformSpecific/windows/WindowsApplication.kt), [formula implementations](../../../legacy-fractal-explorer-worktrees/MandelbrotPC/src/main/kotlin/com/fractalExplorer/fractal), [camera](../../../legacy-fractal-explorer-worktrees/MandelbrotPC/src/main/kotlin/com/fractalExplorer/viewPort/Camera.kt), [palette manager](../../../legacy-fractal-explorer-worktrees/MandelbrotPC/src/main/kotlin/com/fractalExplorer/paletas/GerenciadorPaletas.kt), [fragment shader](../../../legacy-fractal-explorer-worktrees/MandelbrotPC/src/main/resources/shaders/fractal/Fragment.glsl).

### Connected explorer features

- **Julia exploration:** J captures the cursor's complex-plane position in a Mandelbrot window; the application creates another fractal window with `Julia(c)`. This is an implemented multi-window flow, despite stale multi-window TODOs.
- **Mouse navigation:** drag pan and wheel zoom around a fixed cursor point. Click actions also exist, but the left-click factor is currently 1, so do not count it as effective zoom-in.
- **Camera rotation:** R/T change desired angle; interpolation and renderer rotation uniforms are connected. Coordinate display appears in the window title.
- **Palette control:** Q/E choose palettes; keypad controls change density, movement toggle, speed/direction and phase offset; N toggles removal of fractional normalization. A sine palette and multiple hardcoded color arrays are available. Shader interpolates adjacent palette colors linearly.
- **Rendering architecture:** shared CPU workers, window/GL thread management, tiled viewport, texture queues and pooled resources. Defaults include 128×128 tiles, five extra preview layers and maximum 5,000 iterations. GPU iteration buffers remain separate from palette data.

### Additional implementations and experiments

- Formula interface includes optimized Mandelbrot, a complex-number Mandelbrot implementation, **integer-power Mandelbrot**, **Julia**, **Newton**, and `Espiral`. Only Mandelbrot-to-Julia selection is connected to the inspected UI; other formulas can be supplied in code.
- Newton and `Espiral` are closely related hardcoded Newton-iteration experiments. One listed root of `z^3 - 1` has the wrong real-component sign; iteration limits are hardcoded. Treat these as feature/prototype references requiring mathematical validation, not a finished generic Newton solver or a separate proven spiral formula.
- A **GPU Mandelbrot compute demo** exists: [JanelaComputeShader.kt](../../../legacy-fractal-explorer-worktrees/MandelbrotPC/src/main/kotlin/com/playground/janelas/JanelaComputeShader.kt) dispatches [cshader1.glsl](../../../legacy-fractal-explorer-worktrees/MandelbrotPC/src/main/resources/shaders/compute/cshader1.glsl), writing a 1,024×1,024 image. Uses GLSL 4.30, float arithmetic and fixed view/iteration parameters. It is a separate playground window, not the default tiled explorer backend.
- Complex-number tests are present. There is no demonstrated renderer/backend conformance suite.

Completeness limits: old Kotlin/LWJGL configuration with Windows natives, desktop/ES shader compatibility concern, known fullscreen/Alt-Tab and GL-focus issues, memory/lifecycle TODOs, and incomplete formula parameterization. Shader max-iteration uniform names also differ between host and shader. Rotation and palette interpolation have active code despite TODO comments requesting them.

### Branch preservation

All ten non-default remote branches were inventoried and checked for commits absent from `master`.

| Branch(es) | Finding |
|---|---|
| `BasicTexture`, `imutabilityCustomTypes`, `spritePositionAtShader`, `threadoverhaul`, `umathreadporjanela`, `uniformWrapper` | Branch tips are ancestors of current `master`; their commit history is already reachable there. They mark older texture/type/shader/thread refactors. |
| `intermediateBranch` | Six commits not in `master`; inspected change summary shows basic-texture abstraction, dummy-texture, type-alias, worker and mouse cleanup. No additional user-facing formula/backend identified in those changes. |
| `tipos_especiais_classe` | Two divergent commits introducing `Arb` and adapting math/formula/camera code. **`Arb` wraps a `Double`; it is not arbitrary precision.** Some conversions are TODOs. Preserve as a numeric-abstraction experiment. |
| `tipos_experimentais_abs_class`, `tipos_experimentais_interface` | Two and one divergent commits respectively, adding playground numeric wrappers (`NumeroEspecial`, `TDouble`, `TInt`). Type-system experiments, not additional verified rendering features. |

## Consolidated feature checklist for Rust

The rewrite should explicitly retain, improve or defer each item. A legacy bug is not a compatibility requirement.

| Capability to account for | Best source references | Status to preserve |
|---|---|---|
| Normalized Mandelbrot iteration calculation | All five implementations | Implemented; compare initialization, bailout, iteration offsets and interior sentinels before defining a common result format. |
| Julia with a selected Mandelbrot parameter, separate windows | MandelbrotPC | Implemented UI flow. |
| Integer-power Mandelbrot; configurable formula interface | MandelbrotPC | Formula code exists; selection/parameters mostly source-level. |
| Newton root/basin exploration | MandelbrotPC | Experimental; correct roots, convergence and coloring before adopting. |
| Progressive tiles, multiple resolutions, prioritized work | All families | Implemented design; C++ also has coarse sampling within tiles. |
| Separate draw/allocate/retain bounds | C++ FractalExplorer properties and window management | Implemented region model; useful for cache retention and navigation responsiveness. |
| CPU fallback and GPU compute path | CPU paths throughout; C++ coloring alternatives; MandelbrotPC compute playground | CPU rendering implemented; GPU computation experimental, not an interchangeable production backend. |
| Keyboard, drag, fixed-point wheel zoom, touch pinch | C++/MandelbrotPC mouse paths; Android touch; PC keyboard | Implemented across different projects; consolidate input actions across platforms. |
| Smooth camera, home/reset and rotation | PC smoothing/home; C++ home; MandelbrotPC rotation | Implemented across variants. |
| Multiple palettes, animation controls, density/phase and interpolation | C++ palettes; MandelbrotPC palette/action/shader code | Implemented; keep procedural and lookup palettes and normalized/banded display options. |
| Runtime iteration limit and texture filtering controls | C++ key handlers | Partial behavior; new implementation should invalidate/recompute/update existing tiles correctly. |
| Unsigned iteration textures separate from palette | Both Kotlin PC projects | Implemented storage/display model; Android uses packed RGBA instead. |
| Resize/fullscreen, multi-window and mobile context recovery | C++, MandelbrotPC, Android and Android `new` branch | Mixed: active resize/fullscreen/window code; mobile restoration remains experimental. |
| Pools, cancellable work, GL upload budgets and shutdown | Kotlin/C++ task/resource code | Preserve intent; repair runaway queues, lifetime races, thread shutdown and memory growth. |
| Diagnostics: coordinates, timings, tile/layer display | C++ debug paths, MandelbrotPC title, Android text | Present at differing completeness levels. |

### Still planned or not established

- Arbitrary-precision arithmetic, perturbation/accelerated deep zoom: TODOs; numeric wrapper branches do not implement these.
- Saved locations/configuration, image import of coordinates, high-resolution image/video export, interactive palette editor: not found as completed paths in the reviewed code; several are explicit TODOs.
- Antialiasing/supersampling, seamless LOD blending, special lighting/distortion shaders and palette-to-palette blending: comments/properties/ideas are not evidence of a finished implementation. Texture filtering and palette interpolation are separate capabilities already represented above.
- Rust, WebGL and Metal backends: rewrite targets, not existing legacy backends. The Kotlin desktop projects hardcode Windows native dependencies; portable library choices alone do not prove Linux support.

Before archiving any legacy reference, capture representative output for these features and validate numerical results, pan/zoom/rotation behavior, palette changes, lifecycle recovery and backend parity in the Rust implementation. This static inventory supplies the cases to preserve; runtime verification remains future work.


