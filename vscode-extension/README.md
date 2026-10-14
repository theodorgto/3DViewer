# 3DViewer

Open STEP/STP, PLY, STL, OBJ, glTF and GLB directly in a VS Code editor.
Geometry is imported locally by the bundled Rust/OpenCASCADE executable and
rendered in a Three.js webview. No model data is uploaded.

## Requirements

Windows x64, VS Code 1.96 or newer, a trusted local workspace, and WebGL support.
Rust and Node.js are not required to use the installed extension. This is a
local-file editor; virtual files and remote workspace URIs are not supported.

## Open Models

Select a supported file in Explorer, use **3DViewer: Open Model**, or choose
**Reopen Editor With > 3DViewer** from the editor tab. The viewer is registered
as the default editor for supported formats unless another editor association
already takes precedence. Choose **Configure Default Editor** to override it.

Left drag orbits, right/middle drag pans, and the wheel zooms. The toolbar
provides fit/reset, reload/cancel, desktop launch, solid/wireframe/points,
orthographic view, Y/Z up, grid, colors, shading, and point size. F fits the model.
Opening the desktop viewer launches a separate native window.

## Settings

- `3dviewer.executablePath`: optional machine-local path to a compatible
	`model-viewer.exe`. Empty uses the executable bundled with this extension.
- `3dviewer.stepTolerance`: relative STEP tessellation tolerance, default
	`0.001`. Smaller values produce finer meshes. Reload to apply changes.

Imports run in a cancellable child process with a three-minute timeout. Closing
the editor terminates the active import. Errors retain the previous model.
File changes require the toolbar's Reload action.

## Geometry Limitations

This is a viewer, not a CAD editor or a universal format converter. STEP
assembly placements are applied; product trees and CAD colors are not retained.
Untriangulated meaningful surfaces produce a persistent incomplete-model warning.
Do not use incomplete models for measurement or CAD verification.
Textures, animation, skinning, transparency, OBJ materials, and glTF lines are
not rendered. Extents are in imported units (normally millimeters for STEP).

## Build And Install

From the parent Rust repository:

```powershell
npm --prefix vscode-extension ci
.\scripts\package-extension.ps1 -Install
```

This builds the native executable, bundles it into a Windows x64 VSIX, and
installs it locally. Reload VS Code after updating. Sample files and standalone
integration tests are not included in the repository or needed for installation.

## Third-Party Software

Three.js uses the MIT license; Lucide uses the ISC license. The native importer
uses OpenCASCADE and opencascade-rs under their LGPL licenses, with the
OpenCASCADE exception. Sources and licensing:
https://github.com/bschwind/opencascade-rs and
https://dev.opencascade.org/resources/licensing . Review source and relinking
requirements before redistributing the native-bundled VSIX. Native build source
and the patched bindings are in the parent repository.

## Release

0.1.0: local custom editor with mesh/STEP import, persistent geometry warnings,
cancellation, interactive rendering, and desktop launch.
