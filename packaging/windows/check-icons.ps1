#!/usr/bin/env pwsh
# Every icon identifier in the WiX source carries a file extension (.ico or .exe); the installer
# rejects shortcuts whose icon identifier lacks one.
param([Parameter(Mandatory = $true)][string]$Wxs)

$ErrorActionPreference = 'Stop'
$text = Get-Content $Wxs -Raw
# Always arrays, so one icon or none still has a Count under the caller's strict mode.
$ids = @([regex]::Matches($text, '<Icon\s+Id="([^"]+)"') | ForEach-Object { $_.Groups[1].Value })
$bad = @($ids | Where-Object { $_ -notmatch '\.(ico|exe)$' })
if ($bad.Count -gt 0) {
    Write-Error "icon identifiers without an extension in ${Wxs}: $($bad -join ', ')"
    exit 1
}
Write-Output "check-icons: $($ids.Count) icon identifier(s), all with an extension"
