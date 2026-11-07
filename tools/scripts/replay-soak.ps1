<#
.SYNOPSIS
Replay soak (M12-06): runs every committed replay with `tdtool replay --verify` many times, in
parallel processes, and checks that every run of a replay prints the same result.

.DESCRIPTION
Each round starts one `tdtool replay <file> --verify` process per replay in tests\replays, at
most -Parallel at a time, and records its exit code and output (the verified line ends with the
final state hash). A replay passes when every run verified and all runs printed the same line.
A replay whose checkpoints come from a capture's world (the e3f10-hangar runs, checked by
td-sim/tests/oracle_e3_full.rs with the capture's muzzle bytes) fails --verify in a direct
start; it is reported as such, and its runs must still agree with each other.

Needs a release build of tdtool (cargo build --release -p tdtool) and the game data
(-Data, else TD2192_DATA). Writes nothing.

.EXAMPLE
pwsh tools/scripts/replay-soak.ps1 -Rounds 20 -Parallel 16
#>
[CmdletBinding()]
param(
    [int]$Rounds = 10,
    [int]$Parallel = [Environment]::ProcessorCount,
    [string]$Data = $env:TD2192_DATA
)

$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$exe = Join-Path $repo 'target\release\tdtool.exe'
if (-not (Test-Path $exe)) { $exe = Join-Path $repo 'target/release/tdtool' }
if (-not (Test-Path $exe)) { throw "No release tdtool; run: cargo build --release -p tdtool" }
if (-not $Data) { throw 'Pass -Data or set TD2192_DATA.' }
$files = Get-ChildItem (Join-Path $repo 'tests\replays') -Filter '*.tdr' | Sort-Object Name
$jobs = foreach ($r in 1..$Rounds) { foreach ($f in $files) { [pscustomobject]@{ Round = $r; File = $f.FullName } } }
Write-Host "$($files.Count) replays x $Rounds rounds = $($jobs.Count) runs, $Parallel at a time"
$start = Get-Date
$results = $jobs | ForEach-Object -ThrottleLimit $Parallel -Parallel {
    $out = & $using:exe replay $_.File --verify --data $using:Data 2>&1 | Out-String
    [pscustomobject]@{
        Round = $_.Round
        Name  = Split-Path $_.File -Leaf
        Exit  = $LASTEXITCODE
        # The first line names the file by the path given; keep the result lines only.
        Text  = (($out -split "`r?`n") | Where-Object { $_ -and $_ -notmatch '\.tdr: episode' }) -join ' | '
    }
}
$elapsed = (Get-Date) - $start
$bad = 0
foreach ($g in ($results | Group-Object Name | Sort-Object Name)) {
    $distinct = @($g.Group | Select-Object Exit, Text -Unique)
    $ok = ($g.Group | Where-Object Exit -eq 0).Count
    if ($distinct.Count -ne 1) {
        $bad++
        Write-Host "DIFFERS  $($g.Name): $($distinct.Count) different results over $($g.Count) runs"
        $distinct | ForEach-Object { Write-Host "         exit $($_.Exit): $($_.Text)" }
    } elseif ($ok -ne $g.Count) {
        Write-Host "SAME, NOT VERIFIED  $($g.Name) ($($g.Count) runs, exit $($distinct[0].Exit)): $($distinct[0].Text)"
    } else {
        Write-Host "SAME  $($g.Name) ($($g.Count) runs): $($distinct[0].Text -replace '^.*final hash ', 'final ')"
    }
}
Write-Host ("{0} runs in {1:N1} s; {2} replays differ between runs" -f $results.Count, $elapsed.TotalSeconds, $bad)
if ($bad -gt 0) { exit 1 }
