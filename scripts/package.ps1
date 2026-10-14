param([switch]$SkipBuild, [switch]$PortableOnly, [string]$InnoCompiler, [string]$MediaBundleDir)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'env.ps1')
$projectRoot = Split-Path -Parent $PSScriptRoot
$distDir = Join-Path $projectRoot 'dist'
$packageDir = Join-Path $distDir 'GenexDomPlayer'
$stageDir = Join-Path $distDir ('.package-' + [Guid]::NewGuid().ToString('N'))
$previousRustFlags = $env:RUSTFLAGS
if (-not $MediaBundleDir) { $MediaBundleDir = Join-Path $projectRoot '.tools\media-bundle\payload' }

# Only remove generated directories immediately inside this project's dist folder.
function Remove-PackageDirectory([string]$Path) {
    $resolved = [IO.Path]::GetFullPath($Path)
    if ([IO.Path]::GetDirectoryName($resolved) -ne [IO.Path]::GetFullPath($distDir) -or
        ([IO.Path]::GetFileName($resolved) -ne 'GenexDomPlayer' -and
         [IO.Path]::GetFileName($resolved) -notmatch '^\.package-[0-9a-f]{32}$')) {
        throw "Refusing to remove a directory outside the packaging area: $resolved"
    }
    if (Test-Path -LiteralPath $resolved) { Remove-Item -LiteralPath $resolved -Recurse -Force }
}

Push-Location -LiteralPath $projectRoot
try {
    $versionMatch = [regex]::Match((Get-Content Cargo.toml -Raw), '(?m)^version\s*=\s*"(\d+\.\d+\.\d+)"')
    if (-not $versionMatch.Success) { throw 'Cargo.toml must contain a three-part release version.' }
    $version = $versionMatch.Groups[1].Value
    $runtimeManifestPath = Join-Path $MediaBundleDir 'tools\runtime-manifest.json'
    if (-not (Test-Path -LiteralPath $runtimeManifestPath)) {
        throw 'Bundled media runtime missing. Run scripts\prepare-media-bundle.ps1 before packaging.'
    }
    $runtimeManifest = Get-Content -LiteralPath $runtimeManifestPath -Raw | ConvertFrom-Json
    foreach ($runtimeFile in $runtimeManifest.files) {
        $runtimePath = Join-Path $MediaBundleDir $runtimeFile.path
        if ((Get-FileHash -LiteralPath $runtimePath -Algorithm SHA256).Hash -ne $runtimeFile.sha256) {
            throw "Bundled runtime checksum mismatch: $($runtimeFile.path)"
        }
    }
    foreach ($tool in @('ffmpeg.exe','ffprobe.exe','yt-dlp.exe','deno.exe')) {
        if (-not (Test-Path -LiteralPath (Join-Path $MediaBundleDir "tools\$tool"))) { throw "Bundled tool missing: $tool" }
    }
    $buildTarget = ((& rustc -vV) | Select-String '^host: ').ToString().Substring(6)
    if ($LASTEXITCODE -ne 0 -or $buildTarget -notmatch '^x86_64-pc-windows-(gnu|gnullvm|msvc)$') {
        throw "Packaging requires an x86-64 Windows Rust toolchain; found $buildTarget"
    }
    $releaseDirectory = Join-Path $projectRoot 'target\release'
    if ($buildTarget -match '-msvc$') {
        # Link the C runtime into the application, so the package needs no VC++ installer.
        $releaseDirectory = Join-Path $projectRoot "target\$buildTarget\release"
    }
    if (-not $SkipBuild) {
        if ($buildTarget -match '-msvc$') {
            $env:RUSTFLAGS = ($previousRustFlags + ' -C target-feature=+crt-static').Trim()
            & cargo build --release --locked --target $buildTarget
        } else {
            & cargo build --release --locked
        }
        if ($LASTEXITCODE -ne 0) { throw 'Release build failed' }
    }
    $releaseExe = Join-Path $releaseDirectory 'GenexDomPlayer.exe'
    if (-not (Test-Path -LiteralPath $releaseExe)) { throw "Release executable missing: $releaseExe" }
    $actualVersion = & $releaseExe --version
    if ($LASTEXITCODE -ne 0 -or $actualVersion -ne "GenexDomPlayer $version") {
        throw 'The release executable does not match Cargo.toml. Rebuild without -SkipBuild.'
    }
    # Locate the compiler before replacing any existing distribution.
    if (-not $PortableOnly) {
        if (-not $InnoCompiler) {
            $candidates = @((Join-Path $projectRoot '.tools\inno\compiler\ISCC.exe'))
            foreach ($programDir in @(${env:ProgramFiles(x86)}, $env:ProgramFiles)) {
                if ($programDir) {
                    $candidates += Join-Path $programDir 'Inno Setup 7\ISCC.exe'
                    $candidates += Join-Path $programDir 'Inno Setup 6\ISCC.exe'
                }
            }
            $command = Get-Command ISCC.exe -ErrorAction SilentlyContinue
            if ($command) { $candidates += $command.Source }
            $InnoCompiler = $candidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
        }
        if (-not $InnoCompiler -or -not (Test-Path -LiteralPath $InnoCompiler)) {
            throw 'Inno Setup compiler missing. Run scripts\install-inno.ps1, specify -InnoCompiler, or use -PortableOnly.'
        }
        $InnoCompiler = (Resolve-Path -LiteralPath $InnoCompiler).Path
    }
    New-Item -ItemType Directory -Force -Path $stageDir | Out-Null
    Copy-Item -LiteralPath $releaseExe -Destination $stageDir
    Copy-Item -LiteralPath 'README.md','LICENSE' -Destination $stageDir
    Copy-Item -LiteralPath 'docs','assets' -Destination $stageDir -Recurse
    Copy-Item -LiteralPath 'docs\DISTRIBUTION.txt' -Destination (Join-Path $stageDir 'QUICK-START.txt')
    Copy-Item -LiteralPath 'scripts\distribution\Start-GenexDome.cmd' -Destination $stageDir
    Get-ChildItem -LiteralPath $MediaBundleDir | ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination $stageDir -Recurse }
    if ($buildTarget -match '-(gnu|gnullvm)$') {
        if (-not $linkerDir) { throw 'The MinGW runtime and licence folder is required for this toolchain.' }
        $licenceDir = Join-Path $stageDir 'licences'
        New-Item -ItemType Directory -Force -Path $licenceDir | Out-Null
        Copy-Item -LiteralPath (Join-Path $linkerDir.FullName 'LICENSE.TXT') -Destination (Join-Path $licenceDir 'LLVM-MinGW.txt')
        Copy-Item -LiteralPath (Join-Path $linkerDir.FullName 'x86_64-w64-mingw32\share\mingw32\COPYING.MinGW-w64-runtime.txt') -Destination $licenceDir
        foreach ($dll in @('libunwind.dll','libgcc_s_seh-1.dll','libwinpthread-1.dll','libstdc++-6.dll')) {
            $dllPath = Join-Path $linkerDir.FullName "bin\$dll"
            if (Test-Path -LiteralPath $dllPath) { Copy-Item -LiteralPath $dllPath -Destination $stageDir }
        }
        if ($buildTarget -match '-gnullvm$' -and -not (Test-Path (Join-Path $stageDir 'libunwind.dll'))) {
            throw 'The LLVM MinGW build requires libunwind.dll.'
        }
    }
    # Include notices for resolved Rust dependencies from the locked build.
    $metadataJson = & cargo metadata --locked --format-version 1 --filter-platform $buildTarget
    if ($LASTEXITCODE -ne 0) { throw 'Could not collect dependency licence metadata.' }
    $metadata = $metadataJson | ConvertFrom-Json
    $resolvedIds = @($metadata.resolve.nodes | ForEach-Object { $_.id })
    $notices = @("GenexDome $version - third-party dependencies", '', 'Font: assets\FONT-LICENSE.txt', 'Bundled media tools: licences\media\BUNDLED-MEDIA-NOTICES.txt', '')
    foreach ($dependency in ($metadata.packages | Where-Object { $_.source -and $_.id -in $resolvedIds } | Sort-Object name,version)) {
        $dependencyRoot = Split-Path -Parent $dependency.manifest_path
        $noticeFiles = @(Get-ChildItem -LiteralPath $dependencyRoot -File | Where-Object { $_.Name -match '^(LICEN[CS]E|COPYING|NOTICE|UNLICENSE)([.\-_]|$)' })
        $noticeDirs = @(Get-ChildItem -LiteralPath $dependencyRoot -Directory | Where-Object { $_.Name -match '^licen[cs]es?$' })
        $destination = Join-Path $stageDir ("licences\rust\" + $dependency.name + '-' + $dependency.version)
        if ($noticeFiles.Count -gt 0 -or $noticeDirs.Count -gt 0 -or $dependency.license_file) {
            New-Item -ItemType Directory -Force -Path $destination | Out-Null
            foreach ($file in $noticeFiles) { Copy-Item -LiteralPath $file.FullName -Destination $destination }
            foreach ($directory in $noticeDirs) { Copy-Item -LiteralPath $directory.FullName -Destination $destination -Recurse }
            if ($dependency.license_file) {
                Copy-Item -LiteralPath (Join-Path $dependencyRoot $dependency.license_file) -Destination $destination -Force
            }
        }
        $notices += "$($dependency.name) $($dependency.version) | $($dependency.license) | $($dependency.repository)"
    }
    Set-Content -LiteralPath (Join-Path $stageDir 'THIRD-PARTY-NOTICES.txt') -Value $notices -Encoding UTF8
    $manifest = [ordered]@{ name = 'GenexDome'; version = $version; target = $buildTarget; media_tools = 'bundled'; runtime_versions = $runtimeManifest.versions; files = @() }
    $manifest.files = @(Get-ChildItem -LiteralPath $stageDir -Recurse -File | Sort-Object FullName | ForEach-Object {
        [ordered]@{ path = $_.FullName.Substring($stageDir.Length + 1).Replace('\','/'); sha256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant() }
    })
    $manifest | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $stageDir 'package-manifest.json') -Encoding UTF8
    Remove-PackageDirectory $packageDir
    Move-Item -LiteralPath $stageDir -Destination $packageDir
    $portableZip = Join-Path $distDir "GenexDome-$version-windows-x64-portable.zip"
    $zipTemp = Join-Path $distDir ('.portable-' + [Guid]::NewGuid().ToString('N') + '.zip')
    try {
        Add-Type -AssemblyName System.IO.Compression.FileSystem
        [IO.Compression.ZipFile]::CreateFromDirectory($packageDir, $zipTemp, [IO.Compression.CompressionLevel]::Optimal, $true)
        Move-Item -LiteralPath $zipTemp -Destination $portableZip -Force
    } finally {
        if (Test-Path -LiteralPath $zipTemp) { Remove-Item -LiteralPath $zipTemp -Force }
    }
    $artifacts = @($portableZip)
    if (-not $PortableOnly) {
        & $InnoCompiler '/Qp' "/DAppVersion=$version" "/DPackageDir=$packageDir" "/DOutputDir=$distDir" (Join-Path $PSScriptRoot 'installer\GenexDome.iss')
        if ($LASTEXITCODE -ne 0) { throw 'Installer compilation failed' }
        $artifacts += Join-Path $distDir "GenexDome-$version-windows-x64-setup.exe"
    }
    $checksums = @($artifacts | ForEach-Object { (Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash.ToLowerInvariant() + '  ' + [IO.Path]::GetFileName($_) })
    Set-Content -LiteralPath (Join-Path $distDir 'SHA256SUMS.txt') -Value $checksums -Encoding ASCII
    Write-Output "Application folder: $packageDir"
    $artifacts | ForEach-Object { Write-Output "Distribution: $_" }
    Write-Output "Checksums: $(Join-Path $distDir 'SHA256SUMS.txt')"
} finally {
    $env:RUSTFLAGS = $previousRustFlags
    Remove-PackageDirectory $stageDir
    Pop-Location
}
