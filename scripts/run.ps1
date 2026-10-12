param([Parameter(ValueFromRemainingArguments=$true)][string[]]$AppArgs)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'env.ps1')
$projectRoot = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $projectRoot
$releaseExe = Join-Path $projectRoot 'target\release\GenexDomPlayer.exe'
$debugExe = Join-Path $projectRoot 'target\debug\GenexDomPlayer.exe'
if (Test-Path $releaseExe) { & $releaseExe @AppArgs }
elseif (Test-Path $debugExe) { & $debugExe @AppArgs }
else { cargo run -- @AppArgs }

