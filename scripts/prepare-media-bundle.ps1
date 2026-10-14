# This downloads only at build time. Setup and the portable application are offline.
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$cacheDir = Join-Path $projectRoot '.tools\media-bundle'
$payloadDir = Join-Path $cacheDir 'payload'
$stageDir = Join-Path $cacheDir ('.payload-' + [Guid]::NewGuid().ToString('N'))
$extractDir = Join-Path $stageDir 'extract'
$lockPath = Join-Path $PSScriptRoot 'media-runtime.lock.json'
$lock = Get-Content -LiteralPath $lockPath -Raw | ConvertFrom-Json

function Get-VerifiedDownload($Entry) {
    $path = Join-Path $cacheDir $Entry.file
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $path) | Out-Null
    if (-not (Test-Path -LiteralPath $path) -and $Entry.local_cache) {
        $local = Join-Path $projectRoot $Entry.local_cache
        if ((Test-Path -LiteralPath $local) -and (Get-FileHash -LiteralPath $local -Algorithm SHA256).Hash -eq $Entry.sha256) {
            Copy-Item -LiteralPath $local -Destination $path
        }
    }
    if (-not (Test-Path -LiteralPath $path) -or (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -ne $Entry.sha256) {
        Write-Host "Downloading pinned build input: $($Entry.file)"
        Invoke-WebRequest -Uri $Entry.url -OutFile $path
    }
    if ((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -ne $Entry.sha256) {
        throw "Checksum mismatch; refusing to bundle $($Entry.file)"
    }
    return $path
}

function Remove-BundleDirectory([string]$Path) {
    $resolved = [IO.Path]::GetFullPath($Path)
    if ([IO.Path]::GetDirectoryName($resolved) -ne [IO.Path]::GetFullPath($cacheDir) -or
        ([IO.Path]::GetFileName($resolved) -ne 'payload' -and
         [IO.Path]::GetFileName($resolved) -notmatch '^\.payload-[0-9a-f]{32}$')) {
        throw "Refusing to remove an unrelated folder: $resolved"
    }
    if (Test-Path -LiteralPath $resolved) { Remove-Item -LiteralPath $resolved -Recurse -Force }
}

try {
    New-Item -ItemType Directory -Force -Path $extractDir | Out-Null
    $toolsDir = Join-Path $stageDir 'tools'
    New-Item -ItemType Directory -Path $toolsDir | Out-Null
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    [IO.Compression.ZipFile]::ExtractToDirectory((Get-VerifiedDownload $lock.ffmpeg), (Join-Path $extractDir 'ffmpeg'))
    $ffmpegRoot = Get-ChildItem -LiteralPath (Join-Path $extractDir 'ffmpeg') -Directory | Select-Object -First 1
    foreach ($tool in @('ffmpeg.exe','ffprobe.exe')) {
        Copy-Item -LiteralPath (Join-Path $ffmpegRoot.FullName "bin\$tool") -Destination $toolsDir
    }
    $ffmpegNotices = Join-Path $stageDir 'licences\media\ffmpeg'
    New-Item -ItemType Directory -Force -Path $ffmpegNotices | Out-Null
    Copy-Item -LiteralPath (Join-Path $ffmpegRoot.FullName 'LICENSE.txt') -Destination $ffmpegNotices
    Copy-Item -LiteralPath (Get-VerifiedDownload $lock.yt_dlp) -Destination (Join-Path $toolsDir 'yt-dlp.exe')
    [IO.Compression.ZipFile]::ExtractToDirectory((Get-VerifiedDownload $lock.deno), (Join-Path $extractDir 'deno'))
    Copy-Item -LiteralPath (Join-Path $extractDir 'deno\deno.exe') -Destination $toolsDir
    foreach ($entry in @($lock.notices) + @($lock.sources)) {
        $destination = Join-Path $stageDir $entry.destination
        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $destination) | Out-Null
        Copy-Item -LiteralPath (Get-VerifiedDownload $entry) -Destination $destination
    }
    $ffmpegVersion = & (Join-Path $toolsDir 'ffmpeg.exe') -version
    if ($LASTEXITCODE -ne 0 -or $ffmpegVersion[0] -notmatch [regex]::Escape($lock.ffmpeg.version)) { throw 'Unexpected bundled FFmpeg version' }
    $ytDlpVersion = & (Join-Path $toolsDir 'yt-dlp.exe') --version
    if ($LASTEXITCODE -ne 0 -or $ytDlpVersion -ne $lock.yt_dlp.version) { throw 'Unexpected bundled yt-dlp version' }
    $denoVersion = & (Join-Path $toolsDir 'deno.exe') --version
    if ($LASTEXITCODE -ne 0 -or $denoVersion[0] -notmatch ('^deno ' + [regex]::Escape($lock.deno.version) + ' ')) { throw 'Unexpected bundled Deno version' }
    Set-Content -LiteralPath (Join-Path $ffmpegNotices 'BUILD-CONFIG.txt') -Value $ffmpegVersion -Encoding UTF8
    Copy-Item -LiteralPath $lockPath -Destination (Join-Path $stageDir 'licences\media\media-runtime.lock.json')
    Copy-Item -LiteralPath (Join-Path $projectRoot 'docs\BUNDLED-MEDIA-NOTICES.txt') -Destination (Join-Path $stageDir 'licences\media')
    # Extraction scratch files are outside the final payload; remove only this known child.
    $resolvedExtract = [IO.Path]::GetFullPath($extractDir)
    if ([IO.Path]::GetDirectoryName($resolvedExtract) -ne [IO.Path]::GetFullPath($stageDir)) { throw 'Unexpected extraction path' }
    Remove-Item -LiteralPath $resolvedExtract -Recurse -Force
    $manifest = [ordered]@{
        versions = [ordered]@{ ffmpeg = $lock.ffmpeg.version; yt_dlp = $lock.yt_dlp.version; deno = $lock.deno.version }
        files = @(Get-ChildItem -LiteralPath $stageDir -Recurse -File | Sort-Object FullName | ForEach-Object {
            [ordered]@{ path = $_.FullName.Substring($stageDir.Length + 1).Replace('\','/'); sha256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant() }
        })
    }
    $manifest | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $toolsDir 'runtime-manifest.json') -Encoding UTF8
    Remove-BundleDirectory $payloadDir
    Move-Item -LiteralPath $stageDir -Destination $payloadDir
    Write-Output "Verified bundled media runtime: $payloadDir"
} finally {
    Remove-BundleDirectory $stageDir
}
