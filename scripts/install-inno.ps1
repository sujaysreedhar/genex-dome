# Portable compiler bootstrap: no machine-wide install, shortcuts or file associations.
param([string]$CompilerDir)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$downloadDir = Join-Path $projectRoot '.tools\inno'
if (-not $CompilerDir) { $CompilerDir = Join-Path $downloadDir 'compiler' }
New-Item -ItemType Directory -Force -Path $downloadDir | Out-Null
$download = Join-Path $downloadDir 'innosetup-7.1.0-x64.exe'
$expectedHash = '0362a383ed217d4c4239b5933866dd96d3eb2102737da92f80f6057a4b40df2f'
if (-not (Test-Path -LiteralPath $download) -or
    (Get-FileHash -LiteralPath $download -Algorithm SHA256).Hash -ne $expectedHash) {
    Invoke-WebRequest -Uri 'https://github.com/jrsoftware/issrc/releases/download/is-7_1_0/innosetup-7.1.0-x64.exe' -OutFile $download
}
if ((Get-FileHash -LiteralPath $download -Algorithm SHA256).Hash -ne $expectedHash) {
    throw 'Inno Setup download checksum mismatch. The compiler was not executed.'
}
$process = Start-Process -FilePath $download -ArgumentList @(
    '/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART','/CURRENTUSER','/PORTABLE=1',
    ('/DIR="' + [IO.Path]::GetFullPath($CompilerDir) + '"'),
    ('/LOG="' + (Join-Path $downloadDir 'extract.log') + '"')
) -WindowStyle Hidden -Wait -PassThru
if ($process.ExitCode -ne 0) { throw "Inno Setup extraction failed with exit code $($process.ExitCode)" }
if (-not (Test-Path (Join-Path $CompilerDir 'ISCC.exe'))) { throw 'ISCC.exe was not extracted.' }
Write-Output "Compiler ready: $(Join-Path $CompilerDir 'ISCC.exe')"
