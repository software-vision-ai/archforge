#!/usr/bin/env pwsh
# Windows: an installer with a wizard (WiX 5 with the UI and Util extensions) and a zip archive.
# Before packaging, the executable's machine type and subsystem are checked: the application uses
# the GUI subsystem. Unsigned by default; -SignOnly signs what an earlier run produced, in a signing
# environment only.
#
# WiX sources carry their XML namespace as __WIX_NAMESPACE__, filled from WIX_NAMESPACE or, when it
# is unset, WiX 5's schema namespace root, fixed by WiX.

param(
    [Parameter(Mandatory = $true)][string]$Version,
    [string]$Target = 'x86_64-pc-windows-msvc',
    [ValidateSet('x64', 'x86', 'arm64')][string]$Arch = 'x64',
    [switch]$SignOnly
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$Root = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$App = 'archforge'
$Out = Join-Path $Root 'dist/windows'
New-Item -ItemType Directory -Force -Path $Out | Out-Null

function Get-ArtefactName([string]$Ext) { "$App-$Version-windows-$Arch.$Ext" }

if ($SignOnly) {
    foreach ($name in 'WINDOWS_CERTIFICATE_PFX', 'WINDOWS_CERTIFICATE_PASSWORD', 'WINDOWS_TIMESTAMP_URL') {
        if (-not [Environment]::GetEnvironmentVariable($name)) { throw "$name is not set" }
    }
    $pfx = Join-Path $env:RUNNER_TEMP 'signing.pfx'
    [IO.File]::WriteAllBytes($pfx, [Convert]::FromBase64String($env:WINDOWS_CERTIFICATE_PFX))
    $files = Get-ChildItem $Out -Filter *.msi
    foreach ($zip in Get-ChildItem $Out -Filter *.zip) {
        $dir = Join-Path $env:RUNNER_TEMP $zip.BaseName
        Expand-Archive $zip.FullName -DestinationPath $dir -Force
        $exes = Get-ChildItem $dir -Filter *.exe -Recurse
        & signtool sign /f $pfx /p $env:WINDOWS_CERTIFICATE_PASSWORD /fd SHA256 /tr $env:WINDOWS_TIMESTAMP_URL /td SHA256 @($exes.FullName)
        if ($LASTEXITCODE -ne 0) { throw 'signtool failed' }
        Compress-Archive -Path (Join-Path $dir '*') -DestinationPath $zip.FullName -Force
    }
    & signtool sign /f $pfx /p $env:WINDOWS_CERTIFICATE_PASSWORD /fd SHA256 /tr $env:WINDOWS_TIMESTAMP_URL /td SHA256 @($files.FullName)
    if ($LASTEXITCODE -ne 0) { throw 'signtool failed' }
    Remove-Item $pfx
    return
}

# The installer lists its licence texts one by one; one added under licenses/ must be listed too,
# checked before the build.
$Wxs = Get-Content (Join-Path $Root "packaging/windows/$App.wxs") -Raw
foreach ($text in Get-ChildItem (Join-Path $Root 'licenses') -Recurse -File -Force) {
    $relative = $text.FullName.Substring($Root.Length + 1).Replace('/', '\')
    if (-not $Wxs.Contains("Source=`"`$(var.RootDir)\$relative`"")) { throw "packaging/windows/$App.wxs does not install $relative" }
}

& cargo build --release --locked --target $Target -p $App --manifest-path (Join-Path $Root 'Cargo.toml')
if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }
$TargetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $Root 'target' }
$Bin = Join-Path $TargetDir "$Target/release"

# Machine type and subsystem, read from the PE header.
$Machines = @{ 'x64' = 0x8664; 'x86' = 0x14c; 'arm64' = 0xAA64 }
function Test-Executable([string]$Path, [int]$Subsystem) {
    $bytes = [IO.File]::ReadAllBytes($Path)
    $pe = [BitConverter]::ToInt32($bytes, 0x3C)
    $machine = [BitConverter]::ToUInt16($bytes, $pe + 4)
    $sub = [BitConverter]::ToUInt16($bytes, $pe + 24 + 68)
    if ($machine -ne $Machines[$Arch]) { throw "$Path has machine type 0x$('{0:x}' -f $machine), expected $Arch" }
    if ($sub -ne $Subsystem) { throw "$Path has subsystem $sub, expected $Subsystem" }
}
Test-Executable (Join-Path $Bin "$App.exe") 2

& (Join-Path $PSScriptRoot 'check-icons.ps1') -Wxs (Join-Path $PSScriptRoot "$App.wxs")

# The installer, its version the numbers only (0.2.0 of 0.2.0-rc.1): Windows Installer takes no suffix.
if (-not $env:WIX_NAMESPACE) { $env:WIX_NAMESPACE = 'http://wixtoolset.org/schemas/v4' }
$Work = Join-Path $env:TEMP "$App-wix"
New-Item -ItemType Directory -Force -Path $Work | Out-Null
foreach ($f in "$App.wxs", "$App.wxl") {
    (Get-Content (Join-Path $PSScriptRoot $f) -Raw).Replace('__WIX_NAMESPACE__', $env:WIX_NAMESPACE) | Set-Content (Join-Path $Work $f) -NoNewline
}
$Icon = Join-Path $Root 'dist/icons/archforge.ico'
if (-not (Test-Path $Icon)) { throw "$Icon is missing: run packaging/icons.sh first; the application icon is drawn from assets/app-icon" }
& wix build -arch $Arch -ext WixToolset.UI.wixext -ext WixToolset.Util.wixext `
    -d "Version=$(($Version -split '-')[0])" -d "BinDir=$Bin" -d "IconFile=$Icon" -d "RootDir=$Root" `
    -loc (Join-Path $Work "$App.wxl") -o (Join-Path $Out (Get-ArtefactName 'msi')) (Join-Path $Work "$App.wxs")
if ($LASTEXITCODE -ne 0) { throw 'wix build failed' }

# The zip archive: the executable with its licence texts.
$Portable = Join-Path $env:TEMP "$App-portable"
Remove-Item $Portable -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $Portable | Out-Null
Copy-Item (Join-Path $Bin "$App.exe") $Portable
Copy-Item (Join-Path $Root 'LICENSE-MIT'), (Join-Path $Root 'LICENSE-APACHE'), (Join-Path $Root 'NOTICE'), (Join-Path $Root 'ATTRIBUTION.md') $Portable
Copy-Item (Join-Path $Root 'licenses') $Portable -Recurse
Compress-Archive -Path (Join-Path $Portable '*') -DestinationPath (Join-Path $Out (Get-ArtefactName 'zip')) -Force
Write-Output "windows: $Out"
