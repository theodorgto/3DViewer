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

## Limitations And Licensing

Read-only geometry viewing, not CAD editing or validated measurement. Textures,
animation, and STEP product trees/colors are not supported. Do not use models
marked **INCOMPLETE MODEL** for measurement or CAD verification.

OpenCASCADE and the Rust bindings use LGPL licenses with an OpenCASCADE exception.
Review source and relinking requirements before redistributing binaries:
https://github.com/bschwind/opencascade-rs and
https://dev.opencascade.org/resources/licensing .