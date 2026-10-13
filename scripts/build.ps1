param(
    [ValidateSet('build', 'test', 'check', 'clippy', 'run', 'fmt')]
    [string]$Action = 'build',
    [switch]$Release,
    [switch]$Prepare
)
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
if ($Prepare) {
    $baseline = (Get-Content vcpkg.json -Raw | ConvertFrom-Json).'builtin-baseline'
    if (!(Test-Path build/vcpkg/.git)) {
        git clone https://github.com/microsoft/vcpkg.git build/vcpkg
        if ($LASTEXITCODE) { throw 'vcpkg clone failed' }
    }
    git -C build/vcpkg checkout $baseline
    if ($LASTEXITCODE) { throw 'vcpkg checkout failed' }
    if ($IsWindows) {
        & ./build/vcpkg/bootstrap-vcpkg.bat -disableMetrics
        $triplet = 'x64-windows-static-md'
        $vcpkg = './build/vcpkg/vcpkg.exe'
    } else {
        & ./build/vcpkg/bootstrap-vcpkg.sh -disableMetrics
        $triplet = if ($IsMacOS) {
            if ((uname -m) -eq 'arm64') { 'arm64-osx' } else { 'x64-osx' }
        } else { 'x64-linux' }
        $vcpkg = './build/vcpkg/vcpkg'
    }
    if ($LASTEXITCODE) { throw 'vcpkg bootstrap failed' }
    & $vcpkg install --triplet $triplet --x-install-root=build/vcpkg/installed
    if ($LASTEXITCODE) { throw 'native dependency build failed' }
}
$env:VCPKG_ROOT = Join-Path (Get-Location) 'build/vcpkg'
$nativeRoot = Join-Path $env:VCPKG_ROOT 'installed'
if ($IsWindows) {
    $env:VCPKGRS_TRIPLET = 'x64-windows-static-md'
    if (!(Get-Command cmake -ErrorAction SilentlyContinue)) {
        $tool = Get-ChildItem build/vcpkg/downloads/tools -Filter cmake.exe -Recurse | Select-Object -First 1
        if (!$tool) { throw 'CMake is required. Run with -Prepare or install CMake.' }
        $env:CMAKE = $tool.FullName
        $env:PATH = "$($tool.DirectoryName)$([IO.Path]::PathSeparator)$env:PATH"
    }
    if (!(Get-Command ninja -ErrorAction SilentlyContinue)) {
        $ninja = Get-ChildItem build/vcpkg/downloads/tools -Filter ninja.exe -Recurse | Select-Object -First 1
        if ($ninja) { $env:PATH = "$($ninja.DirectoryName)$([IO.Path]::PathSeparator)$env:PATH" }
    }
} else {
    $triplet = if ($IsMacOS) {
        if ((uname -m) -eq 'arm64') { 'arm64-osx' } else { 'x64-osx' }
    } else { 'x64-linux' }
    $env:PKG_CONFIG_PATH = Join-Path $nativeRoot "$triplet/lib/pkgconfig"
}
$cargoArgs = @($Action, '--locked')
if ($Action -eq 'fmt') { $cargoArgs = @('fmt', '--all', '--', '--check') }
elseif ($Release) { $cargoArgs += '--release' }
if ($Action -eq 'clippy') { $cargoArgs += @('--all-targets', '--', '-D', 'warnings') }
& cargo @cargoArgs
if ($LASTEXITCODE) { exit $LASTEXITCODE }
if ($Action -in @('build', 'test', 'run')) {
    & ./scripts/check-secrets.ps1
    if ($LASTEXITCODE) { exit $LASTEXITCODE }
}
