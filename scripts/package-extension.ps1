param([switch]$SkipNativeBuild, [switch]$Install)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
if (-not $SkipNativeBuild) {
    & (Join-Path $PSScriptRoot 'cargo.ps1') -CargoArgs @('build', '--release')
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}
$extension = Join-Path $root 'vscode-extension'
$manifest = Get-Content (Join-Path $extension 'package.json') -Raw | ConvertFrom-Json
$vsix = Join-Path $extension "3dviewer-win32-x64-$($manifest.version).vsix"
$bin = Join-Path $extension 'bin'
New-Item -ItemType Directory -Path $bin -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $root 'target\release\model-viewer.exe') -Destination (Join-Path $bin 'model-viewer.exe') -Force
Push-Location $extension
try {
    & npx vsce package --target win32-x64 --out $vsix --no-dependencies --allow-missing-repository --skip-license
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    if ($Install) {
        & code --install-extension $vsix --force
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    }
} finally { Pop-Location }