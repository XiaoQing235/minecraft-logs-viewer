param(
    [ValidateSet('windows-x64', 'macos-arm64', 'macos-x64', 'linux-x64')]
    [string]$Platform = 'windows-x64'
)
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
$version = (Get-Content Cargo.toml | Select-String '^version = "([^"]+)"$').Matches[0].Groups[1].Value
$name = "minecraft-logs-viewer-$version-$Platform"
$stage = Join-Path (Get-Location) "dist/$name"
if (Test-Path -LiteralPath $stage) { throw 'Package staging directory exists; use a fresh directory for this build.' }
New-Item -ItemType Directory -Path $stage -Force | Out-Null
$executable = if ($Platform -eq 'windows-x64') { 'minecraft-logs-viewer.exe' } else { 'minecraft-logs-viewer' }
$binary = Join-Path (Get-Location) "target/release/$executable"
if (!(Test-Path -LiteralPath $binary)) { throw 'Build Release before packaging.' }
if ($Platform.StartsWith('macos')) {
    $appRoot = Join-Path $stage 'Minecraft Logs Viewer.app/Contents'
    New-Item -ItemType Directory -Path "$appRoot/MacOS" -Force | Out-Null
    Copy-Item -LiteralPath $binary -Destination "$appRoot/MacOS/$executable"
    @"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>minecraft-logs-viewer</string>
<key>CFBundleIdentifier</key><string>org.minecraftlogsviewer.desktop</string>
<key>CFBundleName</key><string>Minecraft Logs Viewer</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>$version</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
"@ | Set-Content -LiteralPath "$appRoot/Info.plist" -Encoding utf8NoBOM
} else {
    Copy-Item -LiteralPath $binary -Destination "$stage/$executable"
}
Copy-Item README.md -Destination $stage
Copy-Item LICENSE -Destination $stage
$licenses = Join-Path $stage 'licenses'
New-Item -ItemType Directory -Path $licenses | Out-Null
$metadata = cargo metadata --locked --format-version 1 | ConvertFrom-Json
if ($LASTEXITCODE) { throw 'Cannot inspect locked dependency licenses.' }
$notices = [Collections.Generic.List[string]]::new()
foreach ($package in $metadata.packages | Where-Object { $_.source }) {
    $root = Split-Path $package.manifest_path
    $files = @(Get-ChildItem -LiteralPath $root -File | Where-Object { $_.Name -match '^(LICENSE|LICENCE|COPYING|UNLICENSE|NOTICE|COPYRIGHT)([.\-_]|$)' })
    if (!$files.Count -and !$package.license_file) { throw "Missing license text for $($package.name)" }
    $destination = Join-Path $licenses "$($package.name)-$($package.version)"
    New-Item -ItemType Directory -Path $destination | Out-Null
    foreach ($file in $files) { Copy-Item -LiteralPath $file.FullName -Destination $destination }
    if ($package.license_file) { Copy-Item -LiteralPath (Join-Path $root $package.license_file) -Destination $destination -Force }
    $notices.Add("$($package.name) $($package.version): $($package.license)")
    if ($package.name -eq 'fltk-sys') {
        Copy-Item -LiteralPath "$root/cfltk/fltk/COPYING" -Destination "$destination/FLTK-COPYING"
        Copy-Item -LiteralPath "$root/cfltk/fltk/CREDITS.txt" -Destination "$destination/FLTK-CREDITS.txt"
        Copy-Item -LiteralPath "$root/cfltk/fltk/src/xutf8/COPYING" -Destination "$destination/FLTK-UTF8-COPYING"
        Copy-Item -LiteralPath "$root/cfltk/fltk/src/xutf8/lcUniConv/COPYRIGHT" -Destination "$destination/FLTK-CONVERSION-COPYRIGHT"
    }
}
$triplet = switch ($Platform) {
    'windows-x64' { 'x64-windows-static-md' }
    'macos-arm64' { 'arm64-osx' }
    'macos-x64' { 'x64-osx' }
    'linux-x64' { 'x64-linux' }
}
foreach ($native in 'libarchive', 'zlib', 'liblzma', 'bzip2') {
    Copy-Item -LiteralPath "build/vcpkg/installed/$triplet/share/$native/copyright" -Destination "$licenses/$native.txt"
    $notices.Add("${native}: statically linked; license text in licenses/$native.txt")
}
$notices | Set-Content -LiteralPath "$stage/THIRD-PARTY.txt" -Encoding utf8NoBOM
$revision = git rev-parse HEAD
if ($LASTEXITCODE) { throw 'Cannot read source revision.' }
$dirty = @(git status --porcelain).Count -gt 0
$bodyBytes = (Get-ChildItem -LiteralPath $stage -File -Recurse | Measure-Object Length -Sum).Sum
if ($bodyBytes -ge 15000000) { throw "Application body exceeds 15 MB: $bodyBytes bytes" }
$archive = Join-Path (Get-Location) "dist/$name.zip"
if (Test-Path -LiteralPath $archive) { throw 'Package archive already exists.' }
if ($IsWindows) {
    Compress-Archive -LiteralPath $stage -DestinationPath $archive -CompressionLevel Optimal
} else {
    Push-Location (Split-Path $stage -Parent)
    try { & zip -r -9 -q $archive $name; if ($LASTEXITCODE) { throw 'zip packaging failed' } }
    finally { Pop-Location }
}
$packageBytes = (Get-Item -LiteralPath $archive).Length
if ($packageBytes -ge 15000000) { throw "Download exceeds 15 MB: $packageBytes bytes" }
[ordered]@{ version = $version; platform = $Platform; revision = $revision; dirty = $dirty; binaryBytes = (Get-Item -LiteralPath $binary).Length; bodyBytes = $bodyBytes; downloadBytes = $packageBytes; sha256 = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash } |
    ConvertTo-Json | Set-Content -LiteralPath "$archive.manifest.json" -Encoding utf8NoBOM
Get-Content -LiteralPath "$archive.manifest.json"
