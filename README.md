# 3DViewer

<img src="assets/3dviewer.png" width="96" alt="3DViewer logo">

A Windows desktop viewer and VS Code extension for STEP/STP, PLY, STL, OBJ,
glTF, and GLB files. Models are imported locally; nothing is uploaded.

## Requirements

- Windows x64 and graphics drivers supporting OpenGL 3.3.
- Rust 1.88+ with the MSVC toolchain.
- Visual Studio Build Tools: **Desktop development with C++**, Windows SDK,
  and **C++ CMake tools for Windows** (or a separate CMake installation).
- For the extension: Node.js 24 LTS and VS Code 1.96+ with WebGL support.

## Build And Install

```powershell
git clone https://github.com/theodorgto/3DViewer.git
cd 3DViewer
.\scripts\cargo.ps1 build --release
.\scripts\install-windows.ps1
```

The first build compiles OpenCASCADE and can take several minutes. Installation
is per-user, with no administrator rights required. Choose **3DViewer** in
Windows Default Apps to associate model files. Alternatively, run
`target\release\model-viewer.exe` directly.

To install the VS Code extension after building the viewer:

```powershell
npm --prefix vscode-extension ci
.\scripts\package-extension.ps1 -SkipNativeBuild -Install
```

Reload VS Code, trust the workspace, and open a model using **3DViewer: Open Model**
or **Reopen Editor With > 3DViewer**. The extension bundles the native viewer;
Rust and Node.js are only needed to build, not to use the installed apps.

Both viewers show a loading progress bar, the current import stage, elapsed
time, and cancellation. STEP progress advances at real stage boundaries; it is
not an estimate of time remaining. Other formats use an animated indicator
until geometry is ready. Cached STEP imports report cache reuse.

## STEP Performance

STEP meshing runs in parallel and extracts positions/triangles in bulk, avoiding
unused native normals and UVs. Healing, tolerances, assembly placement, display
normals, and incomplete-model warnings are unchanged. On a Ryzen 9 9950X, the
36.6 MB TRIOS mount's uncached median improved from 11.8 s to 10.9 s; its
meshing/extraction stage fell from 1.29 s to 0.58 s. Three real models produced
byte-identical output before and after these changes.

Desktop and VS Code share a local, checksum-verified STEP mesh cache. Steady
TRIOS repeat imports took 0.23 s; cached JSON exports took 0.25-0.31 s. These
are worker timings, not complete webview render timings. Startup outliers occur,
and a first import still performs full CAD processing. Cache keys include the
complete staged file contents, tolerance, and importer executable contents, so
file changes, quality changes, and software updates cannot reuse stale meshes.

The cache stores model geometry locally under
`%LOCALAPPDATA%\3DViewer\step-cache`, with a 512 MiB eviction budget and
128 MiB entry limit. Corrupt or unusable entries fall back to normal import.
Delete that directory to clear it. `MODEL_VIEWER_DISABLE_CACHE=1` bypasses
caching; `MODEL_VIEWER_CACHE_DIR` overrides its location. For cache and CAD phase
timings, set `MODEL_VIEWER_IMPORT_TRACE=1` before launching an import worker.

## Limitations And Licensing

Read-only geometry viewing, not CAD editing or validated measurement. Textures,
animation, and STEP product trees/colors are not supported. Do not use models
marked **INCOMPLETE MODEL** for measurement or CAD verification.

OpenCASCADE and the Rust bindings use LGPL licenses with an OpenCASCADE exception.
Review source and relinking requirements before redistributing binaries:
https://github.com/bschwind/opencascade-rs and
https://dev.opencascade.org/resources/licensing .