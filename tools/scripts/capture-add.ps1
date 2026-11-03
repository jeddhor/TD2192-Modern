<#
.SYNOPSIS
Allocates the next capture ID (C-NNNN) and/or appends a row to re-work\captures\MANIFEST.md.

.DESCRIPTION
Reference captures live outside the repository (ADG 3.3, 11.2). Every capture gets an ID and one
manifest row: ID, date, exe, episode/mission/scene, method, DOSBox build + cycles, description,
path (relative to re-work\captures). Repo documents refer to captures by ID only.

This script only edits MANIFEST.md in the RE work area. It never touches the repository.

.EXAMPLE
# What would the next ID be?
pwsh tools/scripts/capture-add.ps1 -NextId

.EXAMPLE
# Register a WAV recorded by hand in an interactive DOSBox-X session
pwsh tools/scripts/capture-add.ps1 -Exe file0001.exe -Scene 'E1 main menu music' -Method audio `
    -Build 'DOSBox-X 2026.10.01 f1c4414 +td2192-oracle; preset P1; cycles=fixed 20000' `
    -Description 'menu.cmf loop, 60 s' -Path 'audio\menu_000.wav'
#>
[CmdletBinding(DefaultParameterSetName = 'Add')]
param(
    [Parameter(ParameterSetName = 'Next', Mandatory)][switch]$NextId,
    [Parameter(ParameterSetName = 'Add')][string]$Id,
    [Parameter(ParameterSetName = 'Add', Mandatory)][string]$Exe,
    [Parameter(ParameterSetName = 'Add', Mandatory)][string]$Scene,
    [Parameter(ParameterSetName = 'Add', Mandatory)][string]$Method,
    [Parameter(ParameterSetName = 'Add', Mandatory)][string]$Build,
    [Parameter(ParameterSetName = 'Add', Mandatory)][string]$Description,
    [Parameter(ParameterSetName = 'Add', Mandatory)][string]$Path,
    [string]$Rework = $(if ($env:TD2192_REWORK) { $env:TD2192_REWORK } else { 'P:\msdos\TD2192\re-work' })
)

$ErrorActionPreference = 'Stop'
$manifest = Join-Path $Rework 'captures\MANIFEST.md'
if (-not (Test-Path $manifest)) { throw "Manifest not found: $manifest" }

function Get-NextCaptureId([string]$file) {
    $max = 0
    foreach ($m in [regex]::Matches((Get-Content -Raw $file), '^\|\s*C-(\d{4})\s*\|', 'Multiline')) {
        $n = [int]$m.Groups[1].Value
        if ($n -gt $max) { $max = $n }
    }
    return ('C-{0:D4}' -f ($max + 1))
}

if ($NextId) {
    Get-NextCaptureId $manifest
    return
}

if (-not $Id) { $Id = Get-NextCaptureId $manifest }
if ((Get-Content -Raw $manifest) -match "(?m)^\|\s*$([regex]::Escape($Id))\s*\|") {
    throw "$Id already exists in $manifest"
}
$cells = @($Id, (Get-Date -Format 'yyyy-MM-dd'), $Exe, $Scene, $Method, $Build, $Description, $Path) |
    ForEach-Object { ($_ -replace '\|', '/') -replace '\r?\n', ' ' }
$row = '| ' + ($cells -join ' | ') + ' |'
$text = Get-Content -Raw $manifest
if (-not $text.EndsWith("`n")) { $text += "`n" }
[IO.File]::WriteAllText($manifest, $text + $row + "`n", [Text.UTF8Encoding]::new($false))
Write-Output $Id
