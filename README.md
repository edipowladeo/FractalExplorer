# FractalExplorer

A fresh implementation in Rust. The previous implementations are considered legacy; this rewrite is not based on which legacy project was most complete.

## Intended targets

- Web browsers with WebGL
- Android
- Windows
- Linux
- Apple platforms with Metal

The intended direction is a shared Rust fractal core with platform and rendering backends. Backend architecture and dependencies have not been selected, and no Rust implementation has been added yet.

## Legacy implementations

The complete former C++/SFML implementation of this repository is preserved on the `legacy` branch at commit `79fe6f36479ff12dee0dfe15a122cea02ffb592c`, including its history. The existing `OpenCL` branch remains available separately.

The local legacy worktree is `C:\legacy-fractal-explorer-worktrees\FractalExplorer`.

The separate FractalExplorerAndroid and FractalExplorerPC repositories are also considered legacy references. Their features should inform the rewrite without requiring their architecture to be retained.
