<#
.SYNOPSIS
Runs one of our Ghidra scripts (tools/ghidra) headless against the canonical TD2192 project.

.DESCRIPTION
Read-only and without re-analysis by default. Pass -Write only while you hold the Ghidra lock
(docs/STATUS.md, ADG 14.4). JAVA_HOME is set for the child process only.
Script output lines are echoed; the full Ghidra log goes to -LogDir.

.EXAMPLE
tools/scripts/ghidra-headless.ps1 -Program file0001.exe -Script ExportFunctionIndex.java

.EXAMPLE
tools/scripts/ghidra-headless.ps1 -Program all -Script SetupBp7Program.java -Write

.EXAMPLE
tools/scripts/ghidra-headless.ps1 -Program file0001.exe -Script ExportDecompiled.java -ScriptArgs 'filter=^3A9A:'
#>
[CmdletBinding()]
param(
    # Program name in the project (file0001.exe, td2.exe, ...) or 'all'.
    [Parameter(Mandatory = $true)][string]$Program,
    # Script file name in tools/ghidra (for example FindPortIO.java).
    [Parameter(Mandatory = $true)][string]$Script,
    [string[]]$ScriptArgs = @(),
    # Save changes to the project (requires the Ghidra lock).
    [switch]$Write,
    # Run Ghidra auto-analysis after the script (default: -noanalysis).
    [switch]$Analyze,
    # Echo the whole Ghidra console output, not just script lines.
    [switch]$Raw,
    [string]$GhidraHome = $(if ($env:GHIDRA_HOME) { $env:GHIDRA_HOME } else { 'C:\Users\jason\Desktop\ghidra_12.2_PUBLIC' }),
    [string]$JavaHome = 'C:\Program Files\Eclipse Adoptium\jdk-25.0.5.8-hotspot',
    [string]$ProjectDir = 'P:\msdos\TD2192\re-work\ghidra',
    [string]$ProjectName = 'TD2192',
    [string]$LogDir = 'P:\msdos\TD2192\re-work\logs'
)

$ErrorActionPreference = 'Stop'
$headless = Join-Path $GhidraHome 'support\analyzeHeadless.bat'
if (-not (Test-Path $headless)) { throw "analyzeHeadless.bat not found under $GhidraHome (set -GhidraHome or GHIDRA_HOME)" }
if (-not (Test-Path (Join-Path $JavaHome 'bin\java.exe'))) {
    if ($env:JAVA_HOME -and (Test-Path (Join-Path $env:JAVA_HOME 'bin\java.exe'))) { $JavaHome = $env:JAVA_HOME }
    else { throw "No JDK at $JavaHome (set -JavaHome)" }
}
$scriptDir = (Resolve-Path (Join-Path $PSScriptRoot '..\ghidra')).Path
if (-not (Test-Path (Join-Path $scriptDir $Script))) { throw "No script $Script in $scriptDir" }
New-Item -ItemType Directory -Force $LogDir | Out-Null

$stem = if ($Program -eq 'all') { 'all' } else { [IO.Path]::GetFileNameWithoutExtension($Program) }
$log = Join-Path $LogDir ("ghidra-{0}-{1}.log" -f [IO.Path]::GetFileNameWithoutExtension($Script), $stem)

$argv = @($ProjectDir, $ProjectName, '-process')
if ($Program -ne 'all') { $argv += $Program }
$argv += @('-scriptPath', $scriptDir, '-postScript', $Script) + $ScriptArgs + @('-log', $log)
if (-not $Analyze) { $argv += '-noanalysis' }
if (-not $Write) { $argv += '-readOnly' }
else { Write-Warning 'Writing to the canonical Ghidra project: make sure you hold the Ghidra lock (docs/STATUS.md).' }

$env:JAVA_HOME = $JavaHome   # process scope only
$scriptName = [IO.Path]::GetFileName($Script)
& $headless @argv 2>&1 | ForEach-Object {
    $line = "$_"
    if ($Raw) { $line }
    elseif ($line -match "$([regex]::Escape($scriptName))> (.*?)( \(GhidraScript\))?\s*$") { $Matches[1] }
    elseif ($line -match 'ERROR|Exception') { $line }
}
exit $LASTEXITCODE
