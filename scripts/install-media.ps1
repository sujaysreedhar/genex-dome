param([switch]$YouTubeOnly, [string]$RuntimeDir)

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
if (-not $RuntimeDir) { $RuntimeDir = Join-Path $projectRoot '.tools\media' }
$RuntimeDir = [IO.Path]::GetFullPath($RuntimeDir)
$downloadDir = Join-Path $RuntimeDir 'downloads'
New-Item -ItemType Directory -Force -Path $runtimeDir,$downloadDir | Out-Null
if (-not $YouTubeOnly) {
    Invoke-WebRequest -Uri 'https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip' -OutFile (Join-Path $downloadDir 'ffmpeg.zip')
    Expand-Archive -LiteralPath (Join-Path $downloadDir 'ffmpeg.zip') -DestinationPath (Join-Path $downloadDir 'extracted') -Force
    Get-ChildItem -LiteralPath (Join-Path $downloadDir 'extracted') -Recurse -Filter '*.exe' | Copy-Item -Destination $runtimeDir
}
Invoke-WebRequest -Uri 'https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe' -OutFile (Join-Path $runtimeDir 'yt-dlp.exe')
Invoke-WebRequest -Uri 'https://github.com/denoland/deno/releases/latest/download/deno-x86_64-pc-windows-msvc.zip' -OutFile (Join-Path $downloadDir 'deno.zip')
$denoDir = Join-Path $downloadDir 'deno'
Expand-Archive -LiteralPath (Join-Path $downloadDir 'deno.zip') -DestinationPath $denoDir -Force
Copy-Item -LiteralPath (Join-Path $denoDir 'deno.exe') -Destination $runtimeDir
Write-Output "Installed media tools in $runtimeDir"
Write-Output 'YouTube uses portable Deno; Node.js 22+ in PATH is also supported.'

