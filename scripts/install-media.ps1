$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$runtimeDir = Join-Path $projectRoot '.tools\media'
$downloadDir = Join-Path $projectRoot '.tools\media-download'
New-Item -ItemType Directory -Force -Path $runtimeDir,$downloadDir | Out-Null
Invoke-WebRequest -Uri 'https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip' -OutFile (Join-Path $downloadDir 'ffmpeg.zip')
Expand-Archive -LiteralPath (Join-Path $downloadDir 'ffmpeg.zip') -DestinationPath (Join-Path $downloadDir 'extracted') -Force
Get-ChildItem -LiteralPath (Join-Path $downloadDir 'extracted') -Recurse -Filter '*.exe' | Copy-Item -Destination $runtimeDir
Invoke-WebRequest -Uri 'https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe' -OutFile (Join-Path $runtimeDir 'yt-dlp.exe')
Write-Output "Installed media tools in $runtimeDir"

