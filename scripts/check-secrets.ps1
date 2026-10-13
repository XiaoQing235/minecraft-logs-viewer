$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
if (!(Get-Command betterleaks -ErrorAction SilentlyContinue)) { throw 'BetterLeaks is required' }
$scanFiles = @(git -c core.quotepath=false ls-files --cached --others --exclude-standard)
if ($LASTEXITCODE) { throw 'Cannot enumerate the working tree' }
$scanFiles = @($scanFiles | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf })
if (!$scanFiles.Count) { throw 'No project files found' }
& betterleaks dir @scanFiles --redact --regex-engine stdlib --no-banner --no-color
exit $LASTEXITCODE
