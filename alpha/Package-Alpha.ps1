param([switch]$SkipBuild)
$ErrorActionPreference = 'Stop'
$sourceDirectory = Join-Path (Split-Path $PSScriptRoot -Parent) '.research/wezterm'
$exportScript = Join-Path (Split-Path $PSScriptRoot -Parent) '.research/export-alpha-patch.py'
if (-not $SkipBuild) {
    Push-Location $sourceDirectory
    try {
        & cargo build -p wezterm-gui -p wezterm --offline
        if ($LASTEXITCODE -ne 0) { throw 'Cargo build failed.' }
    }
    finally { Pop-Location }
}
& python $exportScript
if ($LASTEXITCODE -ne 0) { throw 'Source patch export or clean-baseline validation failed.' }
$binaryDirectory = Join-Path $PSScriptRoot 'bin'
[void](New-Item -ItemType Directory -Path $binaryDirectory -Force)
[void](New-Item -ItemType Directory -Path (Join-Path $binaryDirectory 'mesa') -Force)
foreach ($name in @('wezterm-gui.exe', 'wezterm.exe', 'conpty.dll', 'OpenConsole.exe', 'libEGL.dll', 'libGLESv2.dll', 'mesa/opengl32.dll')) {
    $builtFile = Join-Path $sourceDirectory "target/debug/$name"
    if (-not (Test-Path -LiteralPath $builtFile)) { throw "Missing build output: $builtFile" }
    Copy-Item -LiteralPath $builtFile -Destination (Join-Path $binaryDirectory $name) -Force
}
$revision = & git -C $sourceDirectory rev-parse HEAD
$manifest = @{
    name = 'WezTerm OSC 7501 local alpha'
    source_revision = [string]$revision
    source_directory = $sourceDirectory
    local_modifications = $true
    built_at_utc = [DateTime]::UtcNow.ToString('o')
    build_command = 'cargo build -p wezterm-gui -p wezterm --offline'
    remote_mux_records = $false
    binaries = @(Get-ChildItem -LiteralPath $binaryDirectory -Filter '*.exe' | ForEach-Object {
        @{ file = $_.Name; sha256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash }
    })
}
if (Test-Path -LiteralPath (Join-Path $PSScriptRoot 'osc7501-alpha.patch')) {
    $manifest.source_patch_sha256 = (Get-FileHash -LiteralPath (Join-Path $PSScriptRoot 'osc7501-alpha.patch') -Algorithm SHA256).Hash
}
$manifest | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $PSScriptRoot 'build-info.json')
Write-Host "Packaged the alpha in $binaryDirectory" -ForegroundColor Green
