<#
.SYNOPSIS
Builds a portable release folder and zip of TD2192-Modern for Windows x64 (M13-01, M13-03, M13-05).

.DESCRIPTION
1. Checks that CHANGELOG.md has a section for the workspace version (docs/RELEASE.md).
2. cargo build --release -p td2192 (and tdtool unless -NoTools) into <repo>\target\package,
   with the static C runtime (-C target-feature=+crt-static, and
   tools\scripts\msvc-static-crt.cmake for SDL 3's CMake build), so the program needs no Visual
   C++ Redistributable. Checks with dumpbin, when Visual Studio has it, that no VCRUNTIME/MSVCP
   DLL is imported.
3. Copies the binaries, LICENSE, README.md, CHANGELOG.md, THIRD_PARTY.md and the player guide
   (docs\player\README.md, as PLAYER-GUIDE.md) into
   <Out>\TD2192-Modern-<version>-windows-x64\, and generates THIRD_PARTY_NOTICES.txt (the
   licence texts of every linked third-party component) and COPYRIGHT-rust-std.html (the Rust
   standard library's notices) with tools\scripts\gen-notices.py.
4. Zips the folder next to it.

The package contains no game data: players supply their own copy of Traffic Department 2192
(ADR-0003). The output folder must be outside the repository. Decision D6: Windows x64 and
Linux x64, unsigned; the Linux package is tools/scripts/package.sh. The release checklist is
docs/RELEASE.md.

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
$savedEnv = @{}
foreach ($v in 'RUSTFLAGS', 'CMAKE_TOOLCHAIN_FILE', 'CARGO_TARGET_DIR') {
    $savedEnv[$v] = [Environment]::GetEnvironmentVariable($v)
}
try {
    $version = (cargo metadata --format-version 1 --no-deps | ConvertFrom-Json).packages |
        Where-Object name -eq 'td2192' | Select-Object -ExpandProperty version
    if (-not (Select-String -Path CHANGELOG.md -SimpleMatch "## [$version]" -Quiet)) {
        throw "CHANGELOG.md has no '## [$version]' section (docs/RELEASE.md)."
    }

    # The static C runtime, for Rust and cc-built C/C++ (the flag) and SDL's CMake (the file).
    $env:RUSTFLAGS = '-C target-feature=+crt-static'
    $env:CMAKE_TOOLCHAIN_FILE = (Join-Path $repo 'tools\scripts\msvc-static-crt.cmake').Replace('\', '/')
    $env:CARGO_TARGET_DIR = Join-Path $repo 'target\package'
    $packages = @('-p', 'td2192')
    if (-not $NoTools) { $packages += @('-p', 'tdtool') }
    cargo build --release @packages
    if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }
    $bin = Join-Path $env:CARGO_TARGET_DIR 'release'
    $exes = @('td2192.exe')
    if (-not $NoTools) { $exes += 'tdtool.exe' }

    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    $dumpbin = if (Test-Path $vswhere) {
        & $vswhere -latest -products * -find 'VC\Tools\MSVC\**\bin\Hostx64\x64\dumpbin.exe' |
            Select-Object -First 1
    }
    if ($dumpbin) {
        foreach ($exe in $exes) {
            $crt = & $dumpbin /dependents (Join-Path $bin $exe) |
                Select-String -Pattern '(vcruntime|msvcp)\d+.*\.dll' | ForEach-Object { $_.Line.Trim() }
            if ($crt) { throw "$exe imports the Visual C++ runtime ($($crt -join ', '))." }
        }
        Write-Host 'Runtime check: no Visual C++ Redistributable DLL imported.'
    }
    else {
        Write-Warning 'dumpbin not found: the Visual C++ runtime check was skipped.'
    }

    $name = "TD2192-Modern-$version-windows-x64"
    $dir = Join-Path $outFull $name
    if (Test-Path $dir) { Remove-Item -Recurse -Force $dir -Confirm:$false }
    New-Item -ItemType Directory -Force $dir | Out-Null

    foreach ($exe in $exes) { Copy-Item (Join-Path $bin $exe) $dir }
    Copy-Item LICENSE, README.md, CHANGELOG.md, THIRD_PARTY.md $dir
    Copy-Item docs\player\README.md (Join-Path $dir 'PLAYER-GUIDE.md')
    python tools\scripts\gen-notices.py --out (Join-Path $dir 'THIRD_PARTY_NOTICES.txt') `
        --target x86_64-pc-windows-msvc --std-copyright-out (Join-Path $dir 'COPYRIGHT-rust-std.html')
    if ($LASTEXITCODE -ne 0) { throw 'gen-notices.py failed' }

    $zip = Join-Path $outFull "$name.zip"
    if (Test-Path $zip) { Remove-Item -Force $zip -Confirm:$false }
    Compress-Archive -Path $dir -DestinationPath $zip
    Get-ChildItem $dir | Select-Object Name, Length | Format-Table -AutoSize | Out-String | Write-Host
    Write-Host "Package: $zip"
}
finally {
    foreach ($v in $savedEnv.Keys) { [Environment]::SetEnvironmentVariable($v, $savedEnv[$v]) }
    Pop-Location
}
