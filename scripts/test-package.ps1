param([switch]$TestInstaller, [string]$YouTubeUrl)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$version = [regex]::Match((Get-Content (Join-Path $projectRoot 'Cargo.toml') -Raw), '(?m)^version\s*=\s*"([^"]+)"').Groups[1].Value
$distDir = Join-Path $projectRoot 'dist'
$testRoot = Join-Path $projectRoot ('test-output\package-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force -Path $testRoot | Out-Null
$previousPath = $env:PATH
$previousDenoDir = $env:DENO_DIR

function Assert-Payload([string]$Directory) {
    $manifest = Get-Content -LiteralPath (Join-Path $Directory 'package-manifest.json') -Raw | ConvertFrom-Json
    if ($manifest.version -ne $version) { throw 'Unexpected package version' }
    if ($manifest.media_tools -ne 'bundled') { throw 'Package is missing the bundled media runtime' }
    foreach ($file in $manifest.files) {
        $path = Join-Path $Directory $file.path
        if ((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -ne $file.sha256) {
            throw "Payload checksum mismatch: $($file.path)"
        }
    }
    $exe = Join-Path $Directory 'GenexDomPlayer.exe'
    & $exe --version
    if ($LASTEXITCODE -ne 0) { throw 'Packaged executable could not start' }
    & $exe check-show --session (Join-Path $Directory 'assets\show.genexdomplayer.json')
    if ($LASTEXITCODE -ne 0) { throw 'Packaged demo assets are invalid' }
    & $exe pattern --kind polar --size 64 --output (Join-Path $testRoot 'polar.png')
    if ($LASTEXITCODE -ne 0) { throw 'Packaged pattern generation failed' }
    & $exe export --input (Join-Path $Directory 'assets\show\panorama.png') --output (Join-Path $testRoot 'fisheye.png') --width 64 --height 64
    if ($LASTEXITCODE -ne 0) { throw 'Packaged projection export failed' }
    $toolsDir = Join-Path $Directory 'tools'
    & (Join-Path $toolsDir 'ffmpeg.exe') -version | Select-Object -First 1
    if ($LASTEXITCODE -ne 0) { throw 'Bundled FFmpeg could not start' }
    & (Join-Path $toolsDir 'ffprobe.exe') -version | Select-Object -First 1
    if ($LASTEXITCODE -ne 0) { throw 'Bundled FFprobe could not start' }
    & (Join-Path $toolsDir 'yt-dlp.exe') --version
    if ($LASTEXITCODE -ne 0) { throw 'Bundled yt-dlp could not start' }
    $denoResult = & (Join-Path $toolsDir 'deno.exe') eval 'console.log(6 * 7)'
    if ($LASTEXITCODE -ne 0 -or $denoResult -ne '42') { throw 'Bundled Deno could not execute JavaScript' }
    $doctor = & $exe doctor
    if ($LASTEXITCODE -ne 0 -or ($doctor -join "`n") -match '\bmissing\b') { throw 'Application could not discover bundled media tools' }
    $clip = Join-Path $testRoot 'bundled-runtime-test.mp4'
    & (Join-Path $toolsDir 'ffmpeg.exe') -hide_banner -loglevel error -y -f lavfi -i 'testsrc=size=64x64:rate=12' -f lavfi -i 'sine=frequency=440:sample_rate=48000' -t 1 -c:v libx264 -pix_fmt yuv420p -c:a aac $clip
    if ($LASTEXITCODE -ne 0) { throw 'Bundled FFmpeg could not generate the H.264/AAC smoke clip' }
    & $exe export --input $clip --output (Join-Path $testRoot 'movie-preview.png') --width 64 --height 64
    if ($LASTEXITCODE -ne 0) { throw 'Application could not load a movie using its bundled tools' }
    $benchmarkOutput = & $exe benchmark --input $clip --seconds 2 --max-height 64
    if ($LASTEXITCODE -ne 0) { throw 'Bundled video decoding failed' }
    $benchmarkOutput | Set-Content -LiteralPath (Join-Path $testRoot 'bundled-decode.json') -Encoding UTF8
    if ($YouTubeUrl) {
        & $exe export --input $YouTubeUrl --output (Join-Path $testRoot 'youtube-preview.png') --width 64 --height 64
        if ($LASTEXITCODE -ne 0) { throw 'YouTube preview failed using the bundled resolver and runtime' }
    }
    Write-Output "Verified $($manifest.files.Count) payload checksums, bundled tools, JavaScript and movie decoding."
}

function Invoke-Setup([string]$Installer, [string]$Directory, [string]$LogName) {
    $process = Start-Process -FilePath $Installer -ArgumentList @(
        '/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART','/NOICONS','/TASKS=',
        ('/DIR="' + $Directory + '"'), ('/LOG="' + (Join-Path $testRoot $LogName) + '"')
    ) -WindowStyle Hidden -Wait -PassThru
    if ($process.ExitCode -ne 0) { throw "Installer failed: $($process.ExitCode). See $LogName" }
}

Push-Location -LiteralPath $testRoot
try {
    # Prevent project-local tools, linker DLLs or Cargo from masking missing files.
    $env:PATH = "$env:SystemRoot\System32;$env:SystemRoot"
    $env:DENO_DIR = Join-Path $testRoot 'deno-cache'
    foreach ($line in (Get-Content -LiteralPath (Join-Path $distDir 'SHA256SUMS.txt'))) {
        $parts = $line -split '  ', 2
        if ($parts.Count -ne 2 -or (Get-FileHash -LiteralPath (Join-Path $distDir $parts[1]) -Algorithm SHA256).Hash -ne $parts[0]) {
            throw 'Distribution checksum mismatch'
        }
    }
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $portableRoot = Join-Path $testRoot 'Portable folder with spaces'
    [IO.Compression.ZipFile]::ExtractToDirectory((Join-Path $distDir "GenexDome-$version-windows-x64-portable.zip"), $portableRoot)
    Assert-Payload (Join-Path $portableRoot 'GenexDomPlayer')

    if ($TestInstaller) {
        $uninstallKey = 'HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\{C1A9D85B-851D-40B9-B85E-39F14839AFD9}_is1'
        & "$env:SystemRoot\System32\reg.exe" query $uninstallKey /reg:64 2>$null | Out-Null
        if ($LASTEXITCODE -eq 0) { throw 'An existing GenexDome installation is registered. Installer testing was skipped to preserve it.' }
        $installer = Join-Path $distDir "GenexDome-$version-windows-x64-setup.exe"
        $installDir = Join-Path $testRoot 'Installed folder with spaces'
        try {
            Invoke-Setup $installer $installDir 'install.log'
            Assert-Payload $installDir
            $userFile = Join-Path $installDir 'my-show.genexdomplayer.json'
            Set-Content -LiteralPath $userFile -Value '{"user":"preserve"}' -Encoding ASCII
            $toolsDir = Join-Path $installDir 'tools'
            New-Item -ItemType Directory -Force -Path $toolsDir | Out-Null
            Set-Content -LiteralPath (Join-Path $toolsDir 'user-tool.txt') -Value 'preserve' -Encoding ASCII
            Invoke-Setup $installer $installDir 'reinstall.log'
            Assert-Payload $installDir
            if (-not (Test-Path -LiteralPath $userFile)) { throw 'Reinstall removed user data' }
        } finally {
            $uninstaller = Join-Path $installDir 'unins000.exe'
            if (Test-Path -LiteralPath $uninstaller) {
                $process = Start-Process -FilePath $uninstaller -ArgumentList @(
                    '/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART',
                    ('/LOG="' + (Join-Path $testRoot 'uninstall.log') + '"')
                ) -WindowStyle Hidden -Wait -PassThru
                if ($process.ExitCode -ne 0) { throw "Uninstall failed: $($process.ExitCode)" }
            }
        }
        if (Test-Path -LiteralPath (Join-Path $installDir 'GenexDomPlayer.exe')) { throw 'Uninstall left the application executable' }
        foreach ($tool in @('ffmpeg.exe','ffprobe.exe','yt-dlp.exe','deno.exe')) {
            if (Test-Path -LiteralPath (Join-Path $toolsDir $tool)) { throw "Uninstall left a bundled tool: $tool" }
        }
        if (-not (Test-Path -LiteralPath $userFile) -or -not (Test-Path -LiteralPath (Join-Path $toolsDir 'user-tool.txt'))) {
            throw 'Uninstall removed user-created files'
        }
        & "$env:SystemRoot\System32\reg.exe" query $uninstallKey /reg:64 2>$null | Out-Null
        if ($LASTEXITCODE -eq 0) { throw 'Uninstall left its registration' }
        Write-Output 'Silent install, reinstall and uninstall passed; user-created files were preserved.'
    }
    Write-Output "Package validation passed. Logs and images: $testRoot"
} finally {
    $env:PATH = $previousPath
    $env:DENO_DIR = $previousDenoDir
    Pop-Location
}
