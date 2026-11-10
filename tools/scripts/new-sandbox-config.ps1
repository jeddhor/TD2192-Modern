<#
.SYNOPSIS
Prepares a Windows Sandbox config; does not enable features or launch Sandbox.
.DESCRIPTION
Copies only the archive into a dedicated sibling .files folder and maps it read-only.
An optional game folder/ZIP is mapped read-only, separately. No game data is copied.
The Sandbox starts PowerShell, unpacks the candidate, checks --version/--help, and
opens Explorer for the manual first-run/audio/input/save checks in docs/RELEASE.md.
Networking, clipboard, microphone and printer sharing are disabled.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Archive,
    [Parameter(Mandatory)][string]$Out,
    [string]$Data
)
$ErrorActionPreference = 'Stop'
$archiveFull = (Resolve-Path -LiteralPath $Archive).Path
if ((Split-Path -Leaf $archiveFull) -notmatch '^TD2192-Modern-\d+\.\d+\.\d+-windows-x64\.zip$') {
    throw 'Archive must be a Windows x64 release zip.'
}
$outFull = [System.IO.Path]::GetFullPath($Out)
if ([System.IO.Path]::GetExtension($outFull) -ne '.wsb') { throw 'Out must end in .wsb.' }
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
if ($outFull.StartsWith($repo + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw 'Keep generated Sandbox files outside the repository.'
}
$bundle = [System.IO.Path]::ChangeExtension($outFull, '.files')
if ((Test-Path -LiteralPath $outFull) -or (Test-Path -LiteralPath $bundle)) {
    throw 'Output already exists. Choose a new Out path; this helper never replaces existing files.'
}
$dataFull = if ($Data) { (Resolve-Path -LiteralPath $Data).Path }
New-Item -ItemType Directory -Path $bundle -Force | Out-Null
Copy-Item -LiteralPath $archiveFull -Destination $bundle
$archiveName = (Split-Path -Leaf $archiveFull).Replace("'", "''")
$scriptLines = @(
    '$ErrorActionPreference = ''Stop'''
    '$dest = Join-Path $env:USERPROFILE ''Desktop\TD2192-Test'''
    'New-Item -ItemType Directory -Path $dest | Out-Null'
    ("Expand-Archive -LiteralPath 'C:\TD2192-Package\$archiveName' -DestinationPath " + '$dest')
    '$game = Get-ChildItem -LiteralPath $dest -Filter td2192.exe -Recurse | Select-Object -First 1'
    'if (-not $game) { throw ''No td2192.exe in package.'' }'
    'function Test-Cli([string]$Exe, [string]$Option) {'
    '    $info = New-Object System.Diagnostics.ProcessStartInfo'
    '    $info.FileName = $Exe; $info.Arguments = $Option'
    '    $info.UseShellExecute = $false; $info.CreateNoWindow = $true'
    '    $info.RedirectStandardOutput = $true; $info.RedirectStandardError = $true'
    '    $process = [System.Diagnostics.Process]::Start($info)'
    '    try {'
    '        $stdout = $process.StandardOutput.ReadToEndAsync()'
    '        $stderr = $process.StandardError.ReadToEndAsync()'
    '        if (-not $process.WaitForExit(60000)) { $process.Kill(); throw "CLI timed out: $Exe $Option" }'
    '        Write-Host $stdout.Result; Write-Host $stderr.Result'
    '        if ($process.ExitCode -ne 0) { throw "CLI failed ($($process.ExitCode)): $Exe $Option" }'
    '    } finally { $process.Dispose() }'
    '}'
    'Test-Cli $game.FullName ''--version'''
    'Test-Cli $game.FullName ''--help'''
    '$tool = Join-Path $game.DirectoryName ''tdtool.exe'''
    'if (Test-Path -LiteralPath $tool) { Test-Cli $tool ''--version'' }'
    'Write-Host ''Now double-click td2192.exe in Explorer and follow docs/RELEASE.md section 5.'''
    'Write-Host ''Game data, when provided, is read-only under C:\TD2192-Data.'''
    'Write-Host ''Record results before closing Sandbox; its files are disposable.'''
    'Start-Process explorer.exe -ArgumentList $game.DirectoryName'
)
$scriptLines | Set-Content -LiteralPath (Join-Path $bundle 'start-test.ps1') -Encoding utf8
$xml = [System.Xml.XmlDocument]::new()
$config = $xml.CreateElement('Configuration')
$null = $xml.AppendChild($config)
function Add-Value($Parent, [string]$Name, [string]$Value) {
    $node = $xml.CreateElement($Name)
    $node.InnerText = $Value
    $null = $Parent.AppendChild($node)
}
Add-Value $config 'Networking' 'Disable'
Add-Value $config 'ClipboardRedirection' 'Disable'
Add-Value $config 'AudioInput' 'Disable'
Add-Value $config 'PrinterRedirection' 'Disable'
$mapped = $xml.CreateElement('MappedFolders')
$null = $config.AppendChild($mapped)
function Add-Mapping([string]$HostPath, [string]$GuestPath) {
    $node = $xml.CreateElement('MappedFolder')
    $null = $mapped.AppendChild($node)
    Add-Value $node 'HostFolder' $HostPath
    Add-Value $node 'SandboxFolder' $GuestPath
    Add-Value $node 'ReadOnly' 'true'
}
Add-Mapping $bundle 'C:\TD2192-Package'
if ($dataFull) {
    $dataItem = Get-Item -LiteralPath $dataFull
    $hostDir = if ($dataItem.PSIsContainer) { $dataFull } else { $dataItem.DirectoryName }
    Add-Mapping $hostDir 'C:\TD2192-Data'
    if (-not $dataItem.PSIsContainer) {
        Write-Host 'The ZIP parent folder is shared read-only; use a dedicated folder to share only that ZIP.'
    }
}
$logon = $xml.CreateElement('LogonCommand')
$null = $config.AppendChild($logon)
Add-Value $logon 'Command' 'powershell.exe -NoProfile -NoExit -ExecutionPolicy Bypass -File C:\TD2192-Package\start-test.ps1'
$xml.Save($outFull)
Write-Host "Prepared: $outFull"
Write-Host 'No feature was changed and Sandbox was not launched. The owner opens the .wsb when ready.'
