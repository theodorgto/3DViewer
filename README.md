# Model Viewer

<img src="assets/3dviewer.png" width="96" alt="3DViewer logo">

A native Rust desktop viewer for Windows, with GPU rendering and a statically
linked OpenCASCADE CAD importer. No browser, Python, FreeCAD, or conversion service is
required. A sample model is visible at startup.

## Build And Run

Install Rust using rustup and the **Visual Studio Build Tools**, including
**Desktop development with C++** and a Windows SDK. Use the MSVC Rust toolchain.
Also install CMake, or Visual Studio's **C++ CMake tools for Windows** component.
Rust 1.88 or later is required. The build wrapper locates Visual Studio's CMake.
The first build compiles OpenCASCADE and can take several minutes.

```powershell
.\scripts\cargo.ps1 run
.\scripts\cargo.ps1 build --release
.\target\release\model-viewer.exe
```

The Windows executable is `target/release/model-viewer.exe`. You can move it
outside the project and launch it directly. The viewer requires a graphics driver
with OpenGL 3.3 support. It does not need Rust installed on the destination PC;
some Windows installations may need the Microsoft Visual C++ runtime.

Open a model immediately with a command-line argument:

```powershell
.\target\release\model-viewer.exe "C:\Models\part.step"
```

The cube-and-orbit logo is embedded in the Windows executable and app window,
and shared with the VS Code extension. Regenerate the PNG/ICO artwork with
`.\scripts\generate-logo.ps1`, then rebuild and reinstall to update it.

## Windows File Associations

After building the release executable, install a stable per-user copy and
register the supported file formats:

```powershell
.\scripts\install-windows.ps1
```

The application is installed in `%LOCALAPPDATA%\Programs\3DViewer`, so moving
this repository does not break double-click opening. No administrator rights
are required. The script opens Windows **Default Apps**; select **3DViewer**
and assign the supported extensions. Windows protects existing default-app
choices, so these require your confirmation. The installer never bypasses
Windows' `UserChoice` protection or overwrites another existing default.
You can also right-click a model, choose **Open with > Choose another app**,
select 3DViewer, and select **Always**.

Re-run the installer after rebuilding to update the installed executable.
`-ValidateOnly` checks the installation command without registry changes;
`-NoSettings` suppresses opening Settings. Remove the registration with:

```powershell
.\scripts\install-windows.ps1 -Uninstall
```

Uninstall restores installer-owned extension defaults and removes the viewer's
registration; it retains the executable and does not alter protected user choices.

## VS Code Model Editor

The `vscode-extension` directory contains a local Windows x64 custom editor
with an embedded Three.js viewport. Install Node.js for building, then run:

```powershell
npm --prefix vscode-extension ci
.\scripts\package-extension.ps1 -Install
```

Reload VS Code after installation or updates. Open a supported model in Explorer,
use **3DViewer: Open Model**, or choose **Reopen Editor With > 3DViewer** on its
tab. If an existing editor association takes precedence, select **Configure
Default Editor** there and choose 3DViewer. The extension is local-only and
requires a trusted workspace and WebGL. The packaged executable is included;
Rust/Node.js are not needed on the machine using the installed extension.

The editor supports orbit/pan/zoom, fit/reset, solid/wireframe/points, grid,
orthographic view, Y/Z up, colors, shading, point size, reload/cancel, and desktop
launch. STEP warnings remain visible. `3dviewer.stepTolerance` controls import
quality; `3dviewer.executablePath` optionally overrides the bundled executable.
The model is read-only; file changes require Reload. Imports are cancellable
native child processes with a three-minute timeout and temporary-file cleanup.

Sample files, standalone tests, examples, and browser-preview tooling are not
included in the repository and are not needed to build or install either app.

## Supported Formats

| Format | Support |
| --- | --- |
| STEP / STP | OpenCASCADE STEP transfer, shape healing, assembly placements and surface tessellation; case-insensitive extensions |
| PLY | ASCII and little-/big-endian binary; polygon meshes and point clouds; RGB vertex colors |
| STL | ASCII and binary triangle meshes |
| OBJ | Mesh geometry; multiple objects combined into one view |
| glTF / GLB | Static scene geometry, hierarchy transforms, triangle strips/fans, vertex colors and base-color factors |

**This is not a universal importer.** FBX, IGES, 3MF, USD, native SolidWorks files,
and other proprietary formats are not supported. Export those files to one of
the supported mesh formats using the originating application.

STEP assembly placements are applied, but colors and the product tree are not
preserved. OpenCASCADE normally transfers STEP length units into millimeters.
Degenerate zero-area faces are ignored. If a real surface cannot be tessellated
even after a finer face-local pass, the viewer displays the available geometry
with a persistent **INCOMPLETE MODEL** warning and skipped-surface count.
An incomplete model must not be used for CAD verification or measurement.
This is a geometry viewer, not a CAD editor or a substitute for validated CAD
measurements. Export a mesh from the originating CAD application when required.

Texture images, transparency, animation, skinning, morph targets, and glTF line
primitives are not rendered. OBJ material libraries are not applied. File colors
are currently supplied by PLY RGB or glTF vertex/material colors.

Mesh extents are reported in input coordinate units; STEP extents use the CAD
importer's transferred units (normally millimeters). Geometry is centered
and normalized for rendering only; the listed extents retain their original size.

## Controls

- **Open model**, Ctrl+O, drag-and-drop, or a command-line file argument.
- Left mouse drag: orbit.
- Right/middle mouse drag or Shift+drag: pan.
- Mouse wheel: zoom.
- **Fit view** or F: center and fit the model.
- **Reset view**, Front, Right, Top, and Isometric camera presets.
- Solid, Wireframe, and Points display modes.
- Orthographic/perspective, Y/Z up axis, grid, flat/smooth shading, colors,
  background, and point-size controls.
- STEP tolerance controls the relative tessellation accuracy. Smaller values
  produce finer meshes. Change the value and **Reload model** to apply it.

Imports run in an isolated child process with elapsed-time and STEP-stage feedback.
**Cancel** terminates the importer, and a three-minute timeout stops stuck imports.
Closing the viewer also terminates any active import. Import errors keep the
previous model visible. Only one import runs at a time.
The model is displayed as a combined mesh; there
is no individual part selection or scene-tree editor.

## STEP Import Performance

An October 2026 investigation found that the native CMake helper overwrote
MSVC's Release compiler flags after stripping optimization flags. The Rust
executable was optimized, but its OpenCASCADE kernel was not. The project's
MSVC CMake toolchain now explicitly enables `/O2 /Ob2 /DNDEBUG`, retaining the
existing `/MD` runtime. Stale unoptimized OpenCASCADE CMake caches are
automatically regenerated on the next build. Other platforms are unaffected.

Sequential before/after measurements on an AMD Ryzen 9 9950X, using fresh
release worker processes and the unchanged default STEP tolerance of `0.001`:

| Model | Before | After | Runs per build |
| --- | --- | --- | --- |
| 10055884 TRIOS IN ROBOT MOUNT (for mercury) | 42.9-51.2 s | 11.4-12.3 s | 3 |
| CAD-02039-BaterryBase | 4.7-5.1 s | 1.0 s | 2 |
| BallThrowerHead | 15.0-27.1 s | 6.3-6.6 s | 2 |

The TRIOS median improved from **45.3 s to 11.6 s**, about **3.9x faster**
(74% less waiting). There is no model cache; Windows filesystem caches were
not flushed. All three models' binary outputs were byte-for-byte identical
before and after, including geometry, normals, colors, dimensions, and warnings.
No healing, precision, or incomplete-model safeguards were disabled.

For TRIOS, a separate optimized VS Code JSON-export run took 11.8 s:
parsing 1.3 s, CAD transfer 6.0 s, additional healing 3.0 s, and meshing/extraction
about 1.3 s. The 17.4 MB JSON contained 122,310 vertices and 162,888 triangles,
with no warnings. Node.js file reading plus JSON parsing took about 76 ms;
webview messaging and GPU upload were not included in that measurement.

To inspect import phases yourself:

```powershell
$env:MODEL_VIEWER_IMPORT_TRACE = '1'
New-Item -ItemType Directory -Path target/timing -Force | Out-Null
Measure-Command {
    .\target\release\model-viewer.exe --export-model "C:\Models\part.step" 0.001 target/timing/model.json | Out-Host
}
Remove-Item Env:MODEL_VIEWER_IMPORT_TRACE
```

The pipe makes PowerShell wait for the Windows GUI-subsystem executable.
Use distinct output directories for concurrent workers: STEP input staging is
local to the output directory. Native `--check-model` also reports total loading
time and mesh counts. Set VS Code's `3dviewer.executablePath` to the newly built
executable, or package and reinstall the extension, to use the optimized importer;
rebuilding alone does not update an already installed extension or desktop copy.

Further opportunities, not implemented here:

- Cache tessellated STEP models for repeat opens, keyed by file contents,
  tolerance, and importer version, with bounded storage and preserved warnings.
- Investigate conditional extra healing. It costs about 3 s here, but removing
  it unconditionally risks regressions on previously problematic CAD files.
- Avoid the preliminary bounds mesh and evaluate parallel meshing. Meshing is
  now a smaller cost. The current path caps coarse deflection at 0.05, so raising
  the relative tolerance alone does not speed this large model's meshing.

## Native Dependency Notes

The local `vendor/opencascade-sys` copy fixes the upstream binding's MSVC handle
allocations and handle-parameter signatures, watches C++ headers for rebuilds,
heals STEP topology, and reports untessellated nondegenerate surfaces. OpenCASCADE
is built statically by the binding's `builtin` feature. The project config enables
CMake 4 compatibility with the dependency's older policy declarations.

OpenCASCADE and the Rust bindings carry LGPL licenses; OpenCASCADE also has an
additional exception. Review their license and source/relinking requirements
before redistributing a statically linked executable, especially in a closed-source
product. Sources: https://github.com/bschwind/opencascade-rs and
https://dev.opencascade.org/resources/licensing . No Gmsh backend is included.