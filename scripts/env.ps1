$projectRoot = Split-Path -Parent $PSScriptRoot
if (Test-Path (Join-Path $projectRoot '.tools\cargo\bin')) {
    $env:CARGO_HOME = Join-Path $projectRoot '.tools\cargo'
    $env:RUSTUP_HOME = Join-Path $projectRoot '.tools\rustup'
    $env:PATH = "$env:CARGO_HOME\bin;$env:PATH"
}
$linkerDir = Get-ChildItem -LiteralPath (Join-Path $projectRoot '.tools\mingw') -Directory -ErrorAction SilentlyContinue | Select-Object -First 1
if ($linkerDir) {
    $env:PATH = "$($linkerDir.FullName)\bin;$env:PATH"
    $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = Join-Path $linkerDir.FullName 'bin\x86_64-w64-mingw32-gcc.exe'
    $env:CC_x86_64_pc_windows_gnu = $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER
    $env:AR_x86_64_pc_windows_gnu = Join-Path $linkerDir.FullName 'bin\llvm-ar.exe'
    $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNULLVM_LINKER = Join-Path $linkerDir.FullName 'bin\x86_64-w64-mingw32-clang.exe'
    $env:CC_x86_64_pc_windows_gnullvm = $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNULLVM_LINKER
    $env:AR_x86_64_pc_windows_gnullvm = Join-Path $linkerDir.FullName 'bin\llvm-ar.exe'
}
$env:PATH = "$(Join-Path $projectRoot '.tools\media');$env:PATH"

