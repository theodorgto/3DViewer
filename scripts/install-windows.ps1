param(
    [string]$Executable = (Join-Path (Split-Path -Parent $PSScriptRoot) 'target\release\model-viewer.exe'),
    [string]$InstallDirectory = (Join-Path $env:LOCALAPPDATA 'Programs\3DViewer'),
    [switch]$NoSettings,
    [switch]$ValidateOnly,
    [switch]$Uninstall
)
$ErrorActionPreference = 'Stop'
$extensions = @('.step', '.stp', '.ply', '.stl', '.obj', '.gltf', '.glb')
$programId = '3DViewer.Model'
$registry = [Microsoft.Win32.Registry]::CurrentUser
$installed = Join-Path $InstallDirectory 'model-viewer.exe'
$backup = Join-Path $InstallDirectory 'associations.json'
function Set-RegistryString([string]$Path, [string]$Name, [string]$Value) {
    $key = $registry.CreateSubKey($Path)
    try { $key.SetValue($Name, $Value, [Microsoft.Win32.RegistryValueKind]::String) }
    finally { $key.Dispose() }
}
if ($Uninstall) {
    $previous = if (Test-Path $backup) { Get-Content $backup -Raw | ConvertFrom-Json } else { $null }
    foreach ($extension in $extensions) {
        $key = $registry.OpenSubKey("Software\Classes\$extension", $true)
        if ($key) {
            try {
                if ($key.GetValue('') -eq $programId) {
                    $value = if ($previous) { $previous.$extension } else { $null }
                    if ($null -eq $value) { $key.DeleteValue('', $false) }
                    else { $key.SetValue('', [string]$value) }
                }
                $openWith = $key.OpenSubKey('OpenWithProgids', $true)
                if ($openWith) { try { $openWith.DeleteValue($programId, $false) } finally { $openWith.Dispose() } }
            } finally { $key.Dispose() }
        }
    }
    $registry.DeleteSubKeyTree("Software\Classes\$programId", $false)
    $registry.DeleteSubKeyTree('Software\Classes\Applications\model-viewer.exe', $false)
    $registry.DeleteSubKeyTree('Software\3DViewer\Capabilities', $false)
    $registered = $registry.OpenSubKey('Software\RegisteredApplications', $true)
    if ($registered) { try { $registered.DeleteValue('3DViewer', $false) } finally { $registered.Dispose() } }
    Write-Output '3DViewer file registration removed. The installed executable was retained.'
    exit 0
}
if (-not (Test-Path -LiteralPath $Executable -PathType Leaf)) { throw 'Build the release executable before installing.' }
$Executable = (Resolve-Path -LiteralPath $Executable).Path
$command = '"' + $installed + '" "%1"'
if ($ValidateOnly) {
    Write-Output "Validated per-user installation: $installed"
    Write-Output "Open command: $command"
    Write-Output "Extensions: $($extensions -join ', ')"
    exit 0
}
New-Item -ItemType Directory -Path $InstallDirectory -Force | Out-Null
if ([IO.Path]::GetFullPath($Executable) -ne [IO.Path]::GetFullPath($installed)) {
    $sameBinary = (Test-Path -LiteralPath $installed -PathType Leaf) -and
        ((Get-FileHash -LiteralPath $Executable).Hash -eq (Get-FileHash -LiteralPath $installed).Hash)
    if (-not $sameBinary) {
        try { Copy-Item -LiteralPath $Executable -Destination $installed -Force }
        catch { throw "Cannot update $installed. Close all 3DViewer windows and retry. $($_.Exception.Message)" }
    }
}
if (-not (Test-Path $backup)) {
    $previous = @{}
    foreach ($extension in $extensions) {
        $key = $registry.OpenSubKey("Software\Classes\$extension")
        $previous[$extension] = if ($key) { try { $key.GetValue('') } finally { $key.Dispose() } } else { $null }
    }
    $previous | ConvertTo-Json | Set-Content -Path $backup -Encoding UTF8
}
Set-RegistryString "Software\Classes\$programId" '' '3D Model'
Set-RegistryString "Software\Classes\$programId\DefaultIcon" '' ('"' + $installed + '",0')
Set-RegistryString "Software\Classes\$programId\shell\open\command" '' $command
Set-RegistryString 'Software\Classes\Applications\model-viewer.exe' 'FriendlyAppName' '3DViewer'
Set-RegistryString 'Software\Classes\Applications\model-viewer.exe\shell\open\command' '' $command
Set-RegistryString 'Software\3DViewer\Capabilities' 'ApplicationName' '3DViewer'
Set-RegistryString 'Software\3DViewer\Capabilities' 'ApplicationDescription' 'View STEP, PLY, STL, OBJ and glTF models'
Set-RegistryString 'Software\RegisteredApplications' '3DViewer' 'Software\3DViewer\Capabilities'
foreach ($extension in $extensions) {
    Set-RegistryString "Software\Classes\$extension\OpenWithProgids" $programId ''
    Set-RegistryString 'Software\Classes\Applications\model-viewer.exe\SupportedTypes' $extension ''
    Set-RegistryString 'Software\3DViewer\Capabilities\FileAssociations' $extension $programId
    $key = $registry.CreateSubKey("Software\Classes\$extension")
    try { if (-not $key.GetValue('')) { $key.SetValue('', $programId) } }
    finally { $key.Dispose() }
}
if (-not ('ViewerAssociations' -as [type])) {
    Add-Type -TypeDefinition 'using System; using System.Runtime.InteropServices; public static class ViewerAssociations { [DllImport("shell32.dll")] public static extern void SHChangeNotify(uint eventId, uint flags, IntPtr first, IntPtr second); }'
}
[ViewerAssociations]::SHChangeNotify(0x08000000, 0, [IntPtr]::Zero, [IntPtr]::Zero)
Write-Output "Installed 3DViewer at $installed and registered all supported formats."
Write-Output 'In Windows Default Apps, select 3DViewer and assign the formats. Existing protected defaults require your confirmation.'
if (-not $NoSettings) { Start-Process 'ms-settings:defaultapps?registeredAppUser=3DViewer' }