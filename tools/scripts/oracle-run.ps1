<#
.SYNOPSIS
Runs the original Traffic Department 2192 unattended in the oracle build of DOSBox-X and collects
logs, framebuffer dumps, memory dumps and (optionally) WAV / video / OPL recordings.

.DESCRIPTION
This script only launches dosbox-x.exe with arguments and an oracle configuration file. It does
not contain or link DOSBox-X code. The oracle build, its options and its file formats are
documented in P:\msdos\TD2192\re-work\dosbox\ORACLE.md; the procedure is in ADG section 11.

What it does:
  1. Picks the output folder (-Out, or re-work\captures\oracle\C-NNNN with -Register).
  2. Writes <out>\oracle.ini (read by the oracle build through the DBX_ORACLE variable).
  3. Runs DOSBox-X headless (SDL dummy video/audio) with ref-play.conf + ref-oracle.conf, a fixed
     cycles value and a fixed guest clock, and the DOS command (default TD1.EXE in C:\TD).
  4. Stops after -Frames video frames or -Seconds emulated seconds (or a scenario 'exit').
  5. Optionally converts framebuffer dumps to PNG and appends a MANIFEST row (-Register).

Runs are deterministic: same build + configs + scenario + options => identical outputs. The
run is not paced to the wall clock (turbo), unless -RealTime is given.

The game runs from the writable copy re-work\dosbox\c\TD, mounted as C:. Never point this at the
original install.

.PARAMETER Scenario
Scenario file: one '<when> <action> [args]' per line (keys, fb / mem / vram dumps, marks, exit).
See ORACLE.md. Optional.

.PARAMETER Frames
Exit when this video frame starts (about 70.086 frames per emulated second in mode 13h).

.PARAMETER Seconds
Exit at this emulated time in seconds (alternative to -Frames).

.PARAMETER Record
Also record with DOSBox-X's DX-CAPTURE: wav, video (ZMBV AVI), opl (DRO), av (video+wav), all.

.EXAMPLE
# 60 emulated seconds of the intro, a framebuffer every second, PNGs, registered as a capture
pwsh tools/scripts/oracle-run.ps1 -Seconds 60 -FbEvery 70 -Png -Register `
    -Exe file0000.exe -SceneDesc 'intro' -Description 'intro, fb every 70 frames'

.EXAMPLE
# A scenario to mission 1 with audio, into a scratch folder
pwsh tools/scripts/oracle-run.ps1 -Scenario P:\msdos\TD2192\re-work\dosbox\scenarios\e1-to-m1.txt `
    -Frames 12000 -Record wav -Out P:\msdos\TD2192\re-work\scratch\run1
#>
[CmdletBinding()]
param(
    [string]$Scenario,
    [int]$Frames = 0,
    [double]$Seconds = 0,
    [string]$Out,
    [string]$Command = 'TD1.EXE',
    [int]$Cycles = 20000,
    [int]$FbEvery = 0,
    [int]$FbStart = 0,
    [ValidateSet('files', 'all', 'off')][string]$Int21 = 'files',
    [switch]$NoPit,
    [switch]$NoFrames,
    [string]$FixedTime = '1994-11-01 09:00:00',
    [ValidateSet('none', 'wav', 'video', 'opl', 'av', 'all')][string]$Record = 'none',
    [switch]$Window,
    [switch]$RealTime,
    [switch]$Png,
    [switch]$Register,
    [string]$Exe = 'td1.exe',
    [string]$SceneDesc = '',
    [string]$Description = '',
    [string[]]$Set = @(),
    [string]$Rework = $(if ($env:TD2192_REWORK) { $env:TD2192_REWORK } else { 'P:\msdos\TD2192\re-work' }),
    [string]$DosboxExe = 'P:\msdos\TD2192\dosbox-x\bin\x64\Release\dosbox-x.exe',
    [string]$Build = 'DOSBox-X 2026.10.01 f1c4414 +td2192-oracle 1'
)

$ErrorActionPreference = 'Stop'
$here = $PSScriptRoot

if ($Frames -le 0 -and $Seconds -le 0 -and -not $Window) {
    throw 'Give -Frames or -Seconds (unattended runs must be bounded), or use -Window.'
}
if (-not (Test-Path $DosboxExe)) { throw "DOSBox-X not found: $DosboxExe" }
$dosboxDir = Join-Path $Rework 'dosbox'
$playConf = Join-Path $dosboxDir 'ref-play.conf'
$oracleConf = Join-Path $dosboxDir 'ref-oracle.conf'
foreach ($f in @($playConf, $oracleConf)) { if (-not (Test-Path $f)) { throw "Missing $f" } }

# Safety: the mounted C: must be the writable copy and must not contain DOS\LABEL.EXE.
$cRoot = Join-Path $dosboxDir 'c'
if (Test-Path (Join-Path $cRoot 'DOS\LABEL.EXE')) { throw "Refusing to run: $cRoot\DOS\LABEL.EXE exists." }
if (-not (Test-Path (Join-Path $cRoot 'TD\FILE0001.EXE'))) { throw "Game copy not found under $cRoot\TD" }

$captureId = $null
if ($Register) {
    $captureId = & (Join-Path $here 'capture-add.ps1') -NextId -Rework $Rework
    if (-not $Out) { $Out = Join-Path $Rework "captures\oracle\$captureId" }
}
if (-not $Out) { $Out = Join-Path $Rework ('scratch\oracle-' + (Get-Date -Format 'yyyyMMdd-HHmmss')) }
if (Test-Path (Join-Path $Out 'run.txt')) { throw "Output folder already holds a run: $Out" }
New-Item -ItemType Directory -Force -Path $Out | Out-Null
$Out = (Resolve-Path $Out).Path
$capDir = Join-Path $Out 'capture'
New-Item -ItemType Directory -Force -Path $capDir | Out-Null

# Oracle configuration (read by the oracle build through DBX_ORACLE).
$ini = @(
    '# written by tools/scripts/oracle-run.ps1'
    "out = $Out"
    "int21 = $Int21"
    "frames = $(if ($NoFrames) { 'off' } else { 'on' })"
    "pit = $(if ($NoPit) { 'off' } else { 'on' })"
    "fb_every = $FbEvery"
    "fb_start = $FbStart"
    "exit_frame = $Frames"
    "exit_ms = $([int]($Seconds * 1000))"
    "fixed_time = $FixedTime"
)
if ($Scenario) {
    $scenarioPath = (Resolve-Path $Scenario).Path
    Copy-Item $scenarioPath (Join-Path $Out 'scenario.txt')
    $ini += "script = $(Join-Path $Out 'scenario.txt')"
}
$iniPath = Join-Path $Out 'oracle.ini'
[IO.File]::WriteAllLines($iniPath, [string[]]$ini, [Text.UTF8Encoding]::new($false))

# DOS command, optionally wrapped in DX-CAPTURE (/-D: no wall-clock post-exit delay).
$dosCmd = $Command
$recFlags = @{ wav = '/A'; video = '/V'; opl = '/O'; av = '/V /A'; all = '/V /A /O' }
if ($Record -ne 'none') { $dosCmd = "DX-CAPTURE /-D $($recFlags[$Record]) $Command" }

# Backstop in emulated seconds, in case the scenario never reaches its exit.
$limit = if ($Seconds -gt 0) { $Seconds + 5 } elseif ($Frames -gt 0) { [math]::Ceiling($Frames / 70.0) + 10 } else { 0 }

$argv = @(
    '-conf', $playConf,
    '-conf', $oracleConf,
    '-set', "cpu cycles=fixed $Cycles",
    '-set', "dosbox captures=$capDir",
    '-set', "log logfile=$(Join-Path $Out 'dosbox.log')",
    '-nopromptfolder', '-noconsole', '-fastlaunch'
)
if ($RealTime) { $argv += @('-set', 'cpu turbo=false') }
foreach ($s in $Set) { $argv += @('-set', $s) }
if ($limit -gt 0) { $argv += @('-time-limit', "$limit") }
$argv += @('-c', $dosCmd)
if (-not $Window) { $argv += '-exit' }

$cmdline = "DBX_ORACLE=$iniPath`n" + ($argv | ForEach-Object { if ($_ -match '\s') { "`"$_`"" } else { $_ } }) -join ' '
[IO.File]::WriteAllText((Join-Path $Out 'cmdline.txt'), "$DosboxExe $cmdline`n")

$saved = @{ DBX_ORACLE = $env:DBX_ORACLE; SDL_VIDEODRIVER = $env:SDL_VIDEODRIVER; SDL_AUDIODRIVER = $env:SDL_AUDIODRIVER }
try {
    $env:DBX_ORACLE = $iniPath
    if (-not $Window) { $env:SDL_VIDEODRIVER = 'dummy'; $env:SDL_AUDIODRIVER = 'dummy' }
    $sw = [Diagnostics.Stopwatch]::StartNew()
    & $DosboxExe @argv 2>&1 | Out-Null
    $code = $LASTEXITCODE
    $sw.Stop()
}
finally {
    foreach ($k in $saved.Keys) { Set-Item "env:$k" -Value $saved[$k] -ErrorAction SilentlyContinue; if ($null -eq $saved[$k]) { Remove-Item "env:$k" -ErrorAction SilentlyContinue } }
}

# Summary
function Get-DataRows([string]$file) {
    if (-not (Test-Path $file)) { return 0 }
    $n = 0
    foreach ($line in [IO.File]::ReadLines($file)) { $n++ }
    return [math]::Max(0, $n - 1)
}
$fb = @(Get-ChildItem (Join-Path $Out 'fb') -Filter *.vga -ErrorAction SilentlyContinue)
$frameRows = Get-DataRows (Join-Path $Out 'frames.tsv')
$int21Rows = Get-DataRows (Join-Path $Out 'int21.tsv')
$exitLine = Select-String -Path (Join-Path $Out 'events.tsv') -Pattern "`tEXIT`t" -ErrorAction SilentlyContinue | Select-Object -Last 1
$captures = @(Get-ChildItem $capDir -File -ErrorAction SilentlyContinue)
if (-not (Test-Path (Join-Path $Out 'run.txt'))) {
    Write-Warning 'run.txt missing: the oracle did not activate (is this the td2192-oracle build?).'
}

if ($Png -and $fb.Count -gt 0) {
    & python (Join-Path $here 'oracle-fb2png.py') (Join-Path $Out 'fb') --sheet 8
}

if ($Register) {
    $methods = @('oracle')
    if ($fb.Count) { $methods += 'screenshot(fb)' }
    if ($Record -ne 'none') { $methods += $Record }
    $rel = $Out.Substring((Join-Path $Rework 'captures').Length).TrimStart('\')
    $desc = if ($Description) { $Description } else { "oracle run: $Command" }
    & (Join-Path $here 'capture-add.ps1') -Id $captureId -Exe $Exe -Scene $SceneDesc -Method ($methods -join '+') `
        -Build "$Build; preset P1; cycles=fixed $Cycles; clock $FixedTime" -Description $desc -Path $rel -Rework $Rework | Out-Null
}

[pscustomobject]@{
    CaptureId    = $captureId
    Out          = $Out
    ExitCode     = $code
    WallSeconds  = [math]::Round($sw.Elapsed.TotalSeconds, 1)
    Frames       = $frameRows
    Int21Rows    = $int21Rows
    FbDumps      = $fb.Count
    Recordings   = ($captures.Name -join ', ')
    ExitReason   = if ($exitLine) { ($exitLine.Line -split "`t")[3..4] -join ' ' } else { 'time limit / program end' }
}
