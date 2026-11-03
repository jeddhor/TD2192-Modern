<#
.SYNOPSIS
Builds a portable release folder and zip of TD2192-Modern for Windows x64 (M13-01, M13-03).

.DESCRIPTION
1. cargo build --release -p td2192 (and tdtool unless -NoTools).
2. Copies the binaries, LICENSE, README.md and THIRD_PARTY.md into
   <Out>\TD2192-Modern-<version>-windows-x64\, and generates THIRD_PARTY_NOTICES.txt with
   tools\scripts\gen-notices.py (licence texts of every linked third-party component).
3. Zips the folder next to it.

The package contains no game data: players supply their own copy of Traffic Department 2192
(ADR-0003). The output folder must be outside the repository. Code signing and the other
platforms wait for decision D6.

.EXAMPLE
pwsh tools/scripts/package.ps1 -Out P:\msdos\TD2192\dist
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Out,
    [switch]$NoTools
)

$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$outFull = [System.IO.Path]::GetFullPath($Out)
if ($outFull.StartsWith($repo, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing to write the package inside the repository ($repo)."
}

Push-Location $repo
try {
    $version = (cargo metadata --format-version 1 --no-deps | ConvertFrom-Json).packages |
        Where-Object name -eq 'td2192' | Select-Object -ExpandProperty version
    $packages = @('-p', 'td2192')
    if (-not $NoTools) { $packages += @('-p', 'tdtool') }
    cargo build --release @packages
    if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }

    $name = "TD2192-Modern-$version-windows-x64"
    $dir = Join-Path $outFull $name
    if (Test-Path $dir) { Remove-Item -Recurse -Force $dir -Confirm:$false }
    New-Item -ItemType Directory -Force $dir | Out-Null

    Copy-Item target\release\td2192.exe $dir
    if (-not $NoTools) { Copy-Item target\release\tdtool.exe $dir }
    Copy-Item LICENSE, README.md, THIRD_PARTY.md $dir
    python tools\scripts\gen-notices.py --out (Join-Path $dir 'THIRD_PARTY_NOTICES.txt')
    if ($LASTEXITCODE -ne 0) { throw 'gen-notices.py failed' }

    $zip = Join-Path $outFull "$name.zip"
    if (Test-Path $zip) { Remove-Item -Force $zip -Confirm:$false }
    Compress-Archive -Path $dir -DestinationPath $zip
    Get-ChildItem $dir | Select-Object Name, Length | Format-Table -AutoSize | Out-String | Write-Host
    Write-Host "Package: $zip"
}
finally {
    Pop-Location
}
