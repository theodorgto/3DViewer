param([Parameter(ValueFromRemainingArguments = $true)][string[]]$CargoArgs)
$ErrorActionPreference = 'Stop'
if (-not (Get-Command cmake -ErrorAction SilentlyContinue)) {
    $installer = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (Test-Path $installer) {
        $installation = & $installer -latest -products '*' -property installationPath
        $cmake = Join-Path $installation 'Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin'
        if (Test-Path (Join-Path $cmake 'cmake.exe')) { $env:PATH = "$cmake;$env:PATH" }
    }
}
if (-not (Get-Command cmake -ErrorAction SilentlyContinue)) {
    throw 'Install CMake or the C++ CMake tools component in Visual Studio Build Tools.'
}
if (-not $CargoArgs) { $CargoArgs = @('build', '--release') }
Push-Location (Split-Path -Parent $PSScriptRoot)
try { & cargo @CargoArgs; exit $LASTEXITCODE }
finally { Pop-Location }