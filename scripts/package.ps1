$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'env.ps1')
$projectRoot = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $projectRoot
cargo build --release --locked
if ($LASTEXITCODE -ne 0) { throw 'Release build failed' }
$packageDir = Join-Path $projectRoot 'dist\GenexDomPlayer'
New-Item -ItemType Directory -Force -Path $packageDir | Out-Null
Copy-Item -LiteralPath (Join-Path $projectRoot 'target\release\GenexDomPlayer.exe') -Destination $packageDir
Copy-Item -LiteralPath (Join-Path $projectRoot 'README.md'),(Join-Path $projectRoot 'LICENSE') -Destination $packageDir
Copy-Item -LiteralPath (Join-Path $projectRoot 'docs'),(Join-Path $projectRoot 'assets') -Destination $packageDir -Recurse -Force
if ($linkerDir) {
    Copy-Item -LiteralPath (Join-Path $linkerDir.FullName 'bin\libunwind.dll') -Destination $packageDir
    $licenceDir = Join-Path $packageDir 'licences'
    New-Item -ItemType Directory -Force -Path $licenceDir | Out-Null
    Copy-Item -LiteralPath (Join-Path $linkerDir.FullName 'LICENSE.TXT') -Destination (Join-Path $licenceDir 'LLVM-MinGW.txt')
    Copy-Item -LiteralPath (Join-Path $linkerDir.FullName 'x86_64-w64-mingw32\share\mingw32\COPYING.MinGW-w64-runtime.txt') -Destination $licenceDir
}
Write-Output "Standalone app: $packageDir"
Write-Output 'Media tools remain separate: put ffmpeg.exe, ffprobe.exe and yt-dlp.exe beside the executable or in PATH.'
