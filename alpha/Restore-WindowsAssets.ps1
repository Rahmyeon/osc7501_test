$ErrorActionPreference = 'Stop'
$sourceRoot = Split-Path $PSScriptRoot -Parent
$revision = 'e2a98e1b8af156988ad0ac778658ac27b60a55fb'
$assets = Get-Content (Join-Path $PSScriptRoot 'windows-assets.json') -Raw | ConvertFrom-Json
foreach ($asset in $assets) {
    $destination = Join-Path $sourceRoot $asset.path
    if ((Test-Path -LiteralPath $destination) -and (Get-FileHash -LiteralPath $destination).Hash -eq $asset.sha256) { continue }
    New-Item -ItemType Directory -Path (Split-Path $destination -Parent) -Force | Out-Null
    Invoke-WebRequest -Uri "https://raw.githubusercontent.com/wezterm/wezterm/$revision/$($asset.path)" -OutFile $destination
    if ((Get-FileHash -LiteralPath $destination).Hash -ne $asset.sha256) { throw "Asset hash mismatch: $($asset.path)" }
}
Write-Host 'Pinned Windows runtime assets restored and verified locally.'
